use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};

use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};

use crate::{
    recent::History,
    ui_tests::{find, wait_for},
};

struct StateDir(PathBuf);

impl StateDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ravel-open-{}", glib::uuid_string_random()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for StateDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn uris(history: &History) -> Vec<String> {
    (0..history.model.n_items())
        .map(|index| {
            history
                .model
                .item(index)
                .and_downcast::<gio::File>()
                .unwrap()
                .uri()
                .into()
        })
        .collect()
}

fn search_focused(window: &adw::ApplicationWindow, search: &gtk::SearchEntry) -> bool {
    gtk::prelude::RootExt::focus(window)
        .is_some_and(|focus| focus == *search || focus.is_ancestor(search))
}

fn press(widget: &impl IsA<gtk::Widget>, key: gtk::gdk::Key) {
    let controllers = widget.observe_controllers();
    for index in 0..controllers.n_items() {
        if let Some(controller) = controllers
            .item(index)
            .and_downcast::<gtk::EventControllerKey>()
        {
            controller.emit_by_name::<bool>(
                "key-pressed",
                &[&key, &0_u32, &gtk::gdk::ModifierType::empty()],
            );
        }
    }
}

pub(crate) fn run(app: &adw::Application) {
    let state = StateDir::new();
    let path = state.0.join("state/recent-files.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = gio::File::for_path(state.0.join("old.json"));
    let first = gio::File::for_path(state.0.join("Žluťoučký.json"));
    let second = gio::File::for_path(state.0.join("second.yaml"));
    fs::write(first.path().unwrap(), "{}").unwrap();
    fs::write(second.path().unwrap(), "value: true").unwrap();
    fs::write(&path, serde_json::to_vec(&[old.uri().as_str()]).unwrap()).unwrap();
    let history = History::new(app, Some(path.clone()));
    // Mutations before the initial worker has returned must not be overwritten.
    history.record(&first);
    history.record(&second);
    history.remove(&old);
    history.record(&gio::File::for_uri("https://example.org/private.json"));
    let expected = vec![second.uri().to_string(), first.uri().to_string()];
    wait_for(|| {
        fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            == Some(expected.clone())
    });
    assert_eq!(uris(&history), expected);
    let restored = History::new(app, Some(path.clone()));
    wait_for(|| uris(&restored) == expected);
    drop(restored);

    let menu = crate::open_popover::create(&history);
    let other_menu = crate::open_popover::create(&history);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&menu);
    content.append(&other_menu);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .content(&content)
        .default_width(500)
        .default_height(600)
        .build();
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let action = gio::SimpleAction::new("open-recent", Some(glib::VariantTy::STRING));
    action.connect_activate({
        let opened = Rc::clone(&opened);
        move |_, value| {
            opened
                .borrow_mut()
                .push(value.unwrap().str().unwrap().to_owned())
        }
    });
    window.add_action(&action);
    let browsed = Rc::new(std::cell::Cell::new(false));
    let browse_action = gio::SimpleAction::new("open", None);
    browse_action.connect_activate({
        let browsed = Rc::clone(&browsed);
        move |_, _| browsed.set(true)
    });
    window.add_action(&browse_action);
    window.present();
    let popover = menu.popover().unwrap();
    let search = find::<gtk::SearchEntry>(popover.upcast_ref(), Some("recent-search")).unwrap();
    let list = find::<gtk::ListBox>(popover.upcast_ref(), Some("recent-list")).unwrap();
    let other_list = find::<gtk::ListBox>(other_menu.upcast_ref(), Some("recent-list")).unwrap();
    menu.popup();
    wait_for(|| popover.is_mapped() && search_focused(&window, &search));
    assert!(list.row_at_index(1).is_some());
    search.set_text("ŽLUŤOUČKÝ");
    assert!(list.row_at_index(0).is_some());
    assert!(list.row_at_index(1).is_none());
    search.emit_by_name::<()>("activate", &[]);
    wait_for(|| !popover.is_visible());
    assert_eq!(*opened.borrow(), [first.uri().to_string()]);

    menu.popup();
    wait_for(|| popover.is_mapped() && search_focused(&window, &search));
    assert!(search.text().is_empty());
    press(&search, gtk::gdk::Key::Down);
    assert_eq!(
        gtk::prelude::RootExt::focus(&window).unwrap(),
        list.row_at_index(0).unwrap()
    );
    press(&list, gtk::gdk::Key::Up);
    assert!(search_focused(&window, &search));
    if std::env::var("GDK_BACKEND").as_deref() == Ok("x11") && native_key("Down") {
        wait_for(|| {
            gtk::prelude::RootExt::focus(&window) == list.row_at_index(0).map(|row| row.upcast())
        });
        assert!(native_key("Down"));
        wait_for(|| {
            gtk::prelude::RootExt::focus(&window) == list.row_at_index(1).map(|row| row.upcast())
        });
        assert!(native_key("Return"));
        wait_for(|| !popover.is_visible());
        assert_eq!(opened.borrow().last().unwrap(), first.uri().as_str());
        opened.borrow_mut().pop();
        menu.popup();
        wait_for(|| popover.is_mapped() && search_focused(&window, &search));
        assert!(native_key("Escape"));
        wait_for(|| !popover.is_visible());
        menu.popup();
        wait_for(|| popover.is_mapped() && search_focused(&window, &search));
    }
    search.set_text("does not exist");
    assert!(list.row_at_index(0).is_none());
    let empty = find::<gtk::Box>(list.upcast_ref(), Some("recent-empty")).unwrap();
    let empty_title = find::<gtk::Label>(empty.upcast_ref(), None).unwrap();
    assert_eq!(empty_title.text(), "No matching documents");
    wait_for(|| empty.is_mapped());
    search.emit_by_name::<()>("activate", &[]);
    assert_eq!(opened.borrow().len(), 1);
    assert!(popover.is_visible());
    search.set_text("");
    let remove = find::<gtk::Button>(
        list.row_at_index(0).unwrap().upcast_ref(),
        Some("recent-remove"),
    )
    .unwrap();
    remove.emit_clicked();
    assert_eq!(uris(&history), [first.uri().to_string()]);
    assert!(second.path().unwrap().exists());
    assert_eq!(opened.borrow().len(), 1);
    assert!(other_list.row_at_index(1).is_none());
    assert!(search_focused(&window, &search));
    let row = list.row_at_index(0).unwrap();
    list.emit_by_name::<()>("row-activated", &[&row]);
    assert_eq!(opened.borrow().last().unwrap(), first.uri().as_str());
    wait_for(|| !popover.is_visible());

    menu.popup();
    wait_for(|| popover.is_mapped());
    search.emit_by_name::<()>("stop-search", &[]);
    wait_for(|| !popover.is_visible());
    menu.popup();
    wait_for(|| popover.is_mapped());
    find::<gtk::Button>(popover.upcast_ref(), Some("recent-browse"))
        .unwrap()
        .emit_clicked();
    assert!(browsed.get());
    wait_for(|| !popover.is_visible());
    // Several changes while a writer is pending must persist the newest state.
    history.record(&second);
    history.remove(&first);
    history.remove(&second);
    wait_for(|| fs::read(&path).ok().as_deref() == Some(b"[]"));
    menu.popup();
    wait_for(|| popover.is_mapped());
    assert!(list.row_at_index(0).is_none());
    wait_for(|| empty.is_mapped());
    assert_eq!(empty_title.text(), "No recent documents");
    assert!(empty_title.has_css_class("title-2"));
    assert!(empty.has_css_class("dim-label"));
    let icon = find::<gtk::Image>(empty.upcast_ref(), None).unwrap();
    assert_eq!(
        icon.icon_name().as_deref(),
        Some("document-open-recent-symbolic")
    );
    assert_eq!(icon.pixel_size(), 96);
    assert_eq!(icon.halign(), gtk::Align::Center);
    assert_eq!(empty.spacing(), 32);
    assert_eq!(empty.margin_top(), 32);
    assert_eq!(empty.margin_bottom(), 32);
    assert!(search.is_sensitive());
    assert!(
        find::<gtk::Button>(popover.upcast_ref(), Some("recent-browse"))
            .unwrap()
            .is_sensitive()
    );
    history.record(&first);
    wait_for(|| !empty.is_mapped());
    assert!(list.row_at_index(0).is_some());
    history.remove(&first);
    wait_for(|| empty.is_mapped());
    wait_for(|| fs::read(&path).ok().as_deref() == Some(b"[]"));
    menu.popdown();
    window.close();
    drop(history);

    viewer_integration(app, &state);
}

fn native_key(key: &str) -> bool {
    std::process::Command::new("xdotool")
        .args(["key", key])
        .status()
        .is_ok_and(|status| status.success())
}

fn viewer_integration(app: &adw::Application, state: &StateDir) {
    let valid = gio::File::for_path(state.0.join("valid.json"));
    fs::write(valid.path().unwrap(), r#"{"readOnly":true}"#).unwrap();
    let invalid = gio::File::for_path(state.0.join("broken.json"));
    fs::write(invalid.path().unwrap(), "[").unwrap();
    let missing = gio::File::for_path(state.0.join("missing.json"));
    let (window, viewer) = super::build(app);
    window.present();
    let history = Rc::clone(&viewer.history);
    let (second_window, second_viewer) = super::build(app);
    assert!(Rc::ptr_eq(&history, &second_viewer.history));
    viewer.load(valid.clone());
    wait_for(|| !viewer.spinner.is_spinning());
    assert_eq!(uris(&history).first().unwrap(), valid.uri().as_str());
    viewer.load(invalid.clone());
    wait_for(|| !viewer.spinner.is_spinning());
    assert!(!uris(&history).contains(&invalid.uri().to_string()));
    if let Some(dialog) = window.visible_dialog() {
        dialog.close();
    }
    // Missing recent files still produce the normal error, leaving the current view intact.
    history.record(&missing);
    gtk::prelude::WidgetExt::activate_action(
        &window,
        "win.open-recent",
        Some(&missing.uri().to_variant()),
    )
    .unwrap();
    wait_for(|| !viewer.spinner.is_spinning());
    assert_eq!(viewer.source_file.borrow().as_ref().unwrap(), &valid);
    if let Some(dialog) = window.visible_dialog() {
        dialog.close();
    }
    history.remove(&missing);
    // Even a valid stale request must not add a history entry.
    let stale = gio::File::for_path(state.0.join("stale.json"));
    fs::write(stale.path().unwrap(), "{}").unwrap();
    viewer.load(stale.clone());
    viewer.load(valid.clone());
    wait_for(|| !viewer.spinner.is_spinning());
    assert!(!uris(&history).contains(&stale.uri().to_string()));
    assert!(!viewer.text.is_editable());
    second_window.close();
    window.close();
}
