//! Application-wide recent local documents. History is state, not configuration
//! or cache; no document contents are stored here.
use std::{
    cell::{Cell, RefCell},
    fs::{DirBuilder, File},
    io::{self, Read},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

use gtk::{gio, glib, prelude::*};
use libadwaita as adw;

const MAX_ENTRIES: usize = 100;
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_URI_BYTES: usize = 8192;

thread_local! {
    // All windows share one main-thread model and one serialized writer.
    static SHARED: RefCell<Weak<History>> = const { RefCell::new(Weak::new()) };
}

pub(crate) struct History {
    pub(crate) model: gio::ListStore,
    path: Option<PathBuf>,
    app: glib::WeakRef<adw::Application>,
    entries: RefCell<Vec<String>>,
    pending: RefCell<Vec<Change>>,
    loaded: Cell<bool>,
    saving: Cell<bool>,
    dirty: Cell<bool>,
}

#[derive(Clone)]
enum Change {
    Open(String),
    Remove(String),
}

impl History {
    pub(crate) fn shared(app: &adw::Application) -> Rc<Self> {
        SHARED.with_borrow_mut(|shared| {
            if let Some(history) = shared.upgrade() {
                return history;
            }
            // Smoke tests must never read or overwrite the user's real history.
            let path = if cfg!(test) {
                None
            } else {
                Some(state_path(
                    std::env::var_os("XDG_STATE_HOME").map(PathBuf::from),
                    &glib::home_dir(),
                ))
            };
            let history = Self::new(app, path);
            *shared = Rc::downgrade(&history);
            history
        })
    }

    pub(crate) fn new(app: &adw::Application, path: Option<PathBuf>) -> Rc<Self> {
        let history = Rc::new(Self {
            model: gio::ListStore::new::<gio::File>(),
            path,
            app: app.downgrade(),
            entries: RefCell::new(Vec::new()),
            pending: RefCell::new(Vec::new()),
            loaded: Cell::new(false),
            saving: Cell::new(false),
            dirty: Cell::new(false),
        });
        let Some(path) = history.path.clone() else {
            history.loaded.set(true);
            return history;
        };
        let hold = app.hold();
        glib::spawn_future_local({
            let history = Rc::clone(&history);
            async move {
                let _hold = hold;
                let result = gio::spawn_blocking(move || read_history(&path)).await;
                let mut entries = match result {
                    Ok(Ok(entries)) => entries,
                    Ok(Err(error)) => {
                        glib::g_warning!("ravel", "Could not read recent documents: {error}");
                        Vec::new()
                    }
                    Err(_) => {
                        glib::g_warning!("ravel", "Recent documents reader failed");
                        Vec::new()
                    }
                };
                for change in history.pending.take() {
                    apply(&mut entries, &change);
                }
                *history.entries.borrow_mut() = entries;
                history.loaded.set(true);
                history.refresh();
                history.save();
            }
        });
        history
    }

    pub(crate) fn record(self: &Rc<Self>, file: &gio::File) {
        let uri = file.uri().to_string();
        // Keep credentials and remote resource addresses out of persistent state.
        if file.path().is_some() && valid_uri(&uri) {
            self.change(Change::Open(uri));
        }
    }

    pub(crate) fn remove(self: &Rc<Self>, file: &gio::File) {
        self.change(Change::Remove(file.uri().to_string()));
    }

    fn change(self: &Rc<Self>, change: Change) {
        if !self.loaded.get() {
            self.pending.borrow_mut().push(change.clone());
        }
        apply(&mut self.entries.borrow_mut(), &change);
        self.dirty.set(true);
        self.refresh();
        self.save();
    }

    fn refresh(&self) {
        let files: Vec<_> = self
            .entries
            .borrow()
            .iter()
            .map(|uri| gio::File::for_uri(uri))
            .collect();
        self.model.splice(0, self.model.n_items(), &files);
    }

    fn save(self: &Rc<Self>) {
        if !self.loaded.get() || !self.dirty.get() || self.saving.get() {
            return;
        }
        let Some(path) = self.path.clone() else {
            self.dirty.set(false);
            return;
        };
        self.saving.set(true);
        let hold = self.app.upgrade().map(|app| app.hold());
        glib::spawn_future_local({
            let history = Rc::clone(self);
            async move {
                // Closing the last window must not interrupt a pending write.
                let _hold = hold;
                while history.dirty.replace(false) {
                    let entries = history.entries.borrow().clone();
                    let path = path.clone();
                    match gio::spawn_blocking(move || write_history(&path, &entries)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            glib::g_warning!("ravel", "Could not save recent documents: {error}");
                        }
                        Err(_) => glib::g_warning!("ravel", "Recent documents writer failed"),
                    }
                }
                history.saving.set(false);
            }
        });
    }
}

fn state_path(state_home: Option<PathBuf>, home: &Path) -> PathBuf {
    state_home
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/state"))
        .join("global.zak.ravel/recent-files.json")
}

fn valid_uri(uri: &str) -> bool {
    uri.len() <= MAX_URI_BYTES
        && url::Url::parse(uri).is_ok_and(|url| url.to_file_path().is_ok())
}

fn apply(entries: &mut Vec<String>, change: &Change) {
    let (Change::Open(uri) | Change::Remove(uri)) = change;
    entries.retain(|entry| entry != uri);
    if matches!(change, Change::Open(_)) {
        entries.insert(0, uri.clone());
        entries.truncate(MAX_ENTRIES);
    }
}

fn read_history(path: &Path) -> io::Result<Vec<String>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "History is too large"));
    }
    let entries: Vec<String> = serde_json::from_slice(&bytes)?;
    let mut result = Vec::new();
    for uri in entries {
        if valid_uri(&uri) && !result.contains(&uri) {
            result.push(uri);
            if result.len() == MAX_ENTRIES {
                break;
            }
        }
    }
    Ok(result)
}

fn write_history(path: &Path, entries: &[String]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| io::Error::other("Missing state directory"))?;
    DirBuilder::new().recursive(true).mode(0o700).create(parent)?;
    let bytes = serde_json::to_vec(entries)?;
    // Atomic replacement, private permissions, and no backup containing forgotten files.
    glib::file_set_contents_full(
        path,
        &bytes,
        glib::FileSetContentsFlags::CONSISTENT | glib::FileSetContentsFlags::DURABLE,
        0o600,
    )
    .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    struct StateDir(PathBuf);

    impl StateDir {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("ravel-history-{}", glib::uuid_string_random())))
        }

        fn file(&self) -> PathBuf {
            self.0.join("global.zak.ravel/recent-files.json")
        }
    }

    impl Drop for StateDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn state_path_uses_only_absolute_xdg_state_home() {
        let home = Path::new("/home/test");
        assert_eq!(state_path(Some("/state".into()), home), Path::new("/state/global.zak.ravel/recent-files.json"));
        for state_home in [None, Some("".into()), Some("relative".into())] {
            assert_eq!(state_path(state_home, home), Path::new("/home/test/.local/state/global.zak.ravel/recent-files.json"));
        }
    }

    #[test]
    fn reopening_moves_to_front_without_duplicates() {
        let mut entries = vec!["file:///b.json".into(), "file:///a.json".into()];
        apply(&mut entries, &Change::Open("file:///a.json".into()));
        assert_eq!(entries, ["file:///a.json", "file:///b.json"]);
    }

    #[test]
    fn history_is_bounded() {
        let mut entries = Vec::new();
        for index in 0..MAX_ENTRIES + 10 {
            apply(&mut entries, &Change::Open(format!("file:///{index}.json")));
        }
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries.last().unwrap(), "file:///10.json");
    }

    #[test]
    fn removal_does_not_touch_the_document() {
        let dir = StateDir::new();
        fs::create_dir_all(&dir.0).unwrap();
        let document = dir.0.join("keep.json");
        fs::write(&document, "{}").unwrap();
        let uri = url::Url::from_file_path(&document).unwrap().to_string();
        let mut entries = vec![uri.clone()];
        apply(&mut entries, &Change::Remove(uri));
        write_history(&dir.file(), &entries).unwrap();
        assert!(read_history(&dir.file()).unwrap().is_empty());
        assert_eq!(fs::read_to_string(document).unwrap(), "{}");
    }

    #[test]
    fn history_round_trips_with_private_permissions() {
        let dir = StateDir::new();
        let entries = vec!["file:///tmp/spaces%20and%20%C5%BE.yaml".into()];
        write_history(&dir.file(), &entries).unwrap();
        assert_eq!(read_history(&dir.file()).unwrap(), entries);
        assert_eq!(fs::metadata(dir.file()).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(dir.file().parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        write_history(&dir.file(), &[]).unwrap();
        assert!(read_history(&dir.file()).unwrap().is_empty());
    }

    #[test]
    fn missing_history_is_empty_and_does_not_create_directories() {
        let dir = StateDir::new();
        assert!(read_history(&dir.file()).unwrap().is_empty());
        assert!(!dir.0.exists());
    }

    #[test]
    fn malformed_and_oversized_history_return_errors() {
        let dir = StateDir::new();
        fs::create_dir_all(dir.file().parent().unwrap()).unwrap();
        fs::write(dir.file(), b"not json").unwrap();
        assert!(read_history(&dir.file()).is_err());
        fs::write(dir.file(), vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(read_history(&dir.file()).is_err());
    }

    #[test]
    fn history_discards_duplicates_and_nonlocal_uris() {
        let dir = StateDir::new();
        let entries = ["file:///a.json", "https://example.org/a", "file:///a.json", "invalid", "file:///b.yaml"];
        write_history(&dir.file(), &entries.map(String::from)).unwrap();
        assert_eq!(read_history(&dir.file()).unwrap(), ["file:///a.json", "file:///b.yaml"]);
    }
}
