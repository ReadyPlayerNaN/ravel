//! Native Open menu, built from GTK widgets rather than editor-specific code.
use std::{path::Path, rc::Rc};

use gtk::{gio, glib, prelude::*};

use crate::recent::History;

pub(crate) fn create(history: &Rc<History>) -> gtk::MenuButton {
    let button = gtk::MenuButton::builder()
        .label("Open")
        .always_show_arrow(true)
        .tooltip_text("Open a recent document or browse files (Ctrl+O)")
        .build();
    button.set_widget_name("open-menu");
    let popover = gtk::Popover::builder().position(gtk::PositionType::Bottom).build();
    button.set_popover(Some(&popover));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_width_request(360);
    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search documents")
        .hexpand(true)
        .build();
    search.set_widget_name("recent-search");
    let browse = gtk::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Browse files (Ctrl+O)")
        .action_name("win.open")
        .build();
    browse.set_widget_name("recent-browse");
    browse.connect_clicked(glib::clone!(
        #[weak]
        popover,
        move |_| popover.popdown()
    ));
    toolbar.append(&search);
    toolbar.append(&browse);
    content.append(&toolbar);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let filter = gtk::CustomFilter::new(glib::clone!(
        #[weak]
        search,
        #[upgrade_or]
        false,
        move |item| {
            let Some(file) = item.downcast_ref::<gio::File>() else {
                return false;
            };
            matches(file, &search.text())
        }
    ));
    let filtered = gtk::FilterListModel::new(Some(history.model.clone()), Some(filter.clone()));
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["navigation-sidebar"])
        .build();
    list.set_widget_name("recent-list");
    let empty = gtk::Label::builder()
        .label("No recent documents")
        .margin_top(24)
        .margin_bottom(24)
        .css_classes(["dim-label"])
        .build();
    list.set_placeholder(Some(&empty));
    list.bind_model(Some(&filtered), glib::clone!(
        #[weak]
        history,
        #[weak]
        search,
        #[upgrade_or_else]
        || gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        move |item| {
            let Some(file) = item.downcast_ref::<gio::File>() else {
                return gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast();
            };
            let (name, directory) = labels(file);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            row.set_tooltip_text(Some(&file.parse_name()));
            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_hexpand(true);
            text.append(&gtk::Label::builder()
                .label(&name)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .build());
            text.append(&gtk::Label::builder()
                .label(&directory)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["dim-label", "caption"])
                .build());
            row.append(&text);
            let remove = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text(format!("Remove {name} from recent documents"))
                .valign(gtk::Align::Center)
                .css_classes(["flat", "circular"])
                .build();
            remove.set_widget_name("recent-remove");
            remove.connect_clicked(glib::clone!(
                #[weak]
                history,
                #[weak]
                search,
                #[strong]
                file,
                move |_| {
                    history.remove(&file);
                    search.grab_focus();
                }
            ));
            row.append(&remove);
            row.upcast()
        }
    ));
    list.connect_row_activated(glib::clone!(
        #[weak]
        popover,
        #[weak]
        filtered,
        move |_, row| {
            if let Some(file) = filtered.item(row.index() as u32).and_downcast::<gio::File>() {
                open(&popover, &file);
            }
        }
    ));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(80)
        .max_content_height(520)
        .propagate_natural_height(true)
        .child(&list)
        .build();
    content.append(&scroll);
    popover.set_child(Some(&content));

    // Use immediate editable changes so Enter always opens the current results,
    // even before SearchEntry's debounced search-changed signal would run.
    search.connect_changed(glib::clone!(
        #[weak]
        filter,
        #[weak]
        empty,
        move |search| {
            empty.set_label(if search.text().trim().is_empty() {
                "No recent documents"
            } else {
                "No matching documents"
            });
            filter.changed(gtk::FilterChange::Different);
        }
    ));
    search.connect_activate(glib::clone!(
        #[weak]
        popover,
        #[weak]
        filtered,
        move |_| {
            if let Some(file) = filtered.item(0).and_downcast::<gio::File>() {
                open(&popover, &file);
            }
        }
    ));
    search.connect_stop_search(glib::clone!(
        #[weak]
        popover,
        move |_| popover.popdown()
    ));
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        list,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if matches!(key, gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down)
                && let Some(row) = list.row_at_index(0)
            {
                list.select_row(Some(&row));
                row.grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    search.add_controller(keys);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        list,
        #[weak]
        search,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if matches!(key, gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up)
                && let Some(row) = list.row_at_index(0)
                && let Some(focus) = list.root().and_then(|root| root.focus())
                && (focus == row || focus.is_ancestor(&row))
            {
                search.grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    list.add_controller(keys);
    popover.connect_show(glib::clone!(
        #[weak]
        search,
        #[weak]
        list,
        #[weak]
        scroll,
        move |_| {
            search.set_text("");
            list.unselect_all();
            scroll.vadjustment().set_value(0.0);
            search.grab_focus();
        }
    ));
    button
}

fn open(popover: &gtk::Popover, file: &gio::File) {
    if let Err(error) = popover.activate_action("win.open-recent", Some(&file.uri().to_variant())) {
        glib::g_warning!("ravel", "Could not open recent document: {error}");
    }
    popover.popdown();
}

fn labels(file: &gio::File) -> (String, String) {
    let name = file.basename()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.parse_name().into());
    let directory = file.parent().map(|parent| {
        parent.path().map(|path| abbreviated_directory(&path, &glib::home_dir()))
            .unwrap_or_else(|| parent.parse_name().into())
    }).unwrap_or_default();
    (name, directory)
}

fn abbreviated_directory(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(relative) if relative.as_os_str().is_empty() => "~".into(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn matches(file: &gio::File, query: &str) -> bool {
    glib::casefold(file.parse_name()).contains(glib::casefold(query.trim()).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_case_insensitive_filename_and_directory() {
        let file = gio::File::for_path("/tmp/Projects/Žluťoučký.json");
        assert!(matches(&file, "ŽLUŤOUČKÝ"));
        assert!(matches(&file, " projects "));
        assert!(matches(&file, ""));
        assert!(!matches(&file, "missing"));
    }

    #[test]
    fn home_abbreviation_respects_path_boundaries() {
        let home = Path::new("/home/user");
        assert_eq!(abbreviated_directory(home, home), "~");
        assert_eq!(abbreviated_directory(Path::new("/home/user/Downloads"), home), "~/Downloads");
        assert_eq!(abbreviated_directory(Path::new("/home/username"), home), "/home/username");
    }
}
