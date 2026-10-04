use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};

use crate::ui_tests::{find, wait_for};

fn toggles(widget: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    let mut stack: Vec<gtk::Widget> = vec![widget.clone()];
    while let Some(current) = stack.pop() {
        if let Ok(button) = current.clone().downcast::<gtk::Button>()
            && button.widget_name() == "node-toggle"
        {
            found.push(button);
        }
        let mut child = current.first_child();
        while let Some(widget) = child {
            stack.push(widget.clone());
            child = widget.next_sibling();
        }
    }
    found
}

fn key_text(toggle: &gtk::Button) -> glib::GString {
    toggle
        .child()
        .and_downcast::<gtk::Box>()
        .and_then(|row| row.first_child().and_then(|arrow| arrow.next_sibling()))
        .and_downcast::<gtk::Label>()
        .map(|label| label.text())
        .unwrap_or_default()
}

fn revealer_of(toggle: &gtk::Button) -> gtk::Revealer {
    let parent = toggle.parent().unwrap().downcast::<gtk::Box>().unwrap();
    parent
        .first_child()
        .and_then(|sibling| sibling.next_sibling())
        .and_downcast::<gtk::Revealer>()
        .unwrap()
}

pub fn run(app: &adw::Application) {
    let samples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let window = crate::window::create(app, Some(gio::File::for_path(samples.join("hover.json"))));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("hover.json"))
    });
    let widget = window.clone().upcast::<gtk::Widget>();
    let stack = find::<adw::ViewStack>(&widget, Some("view-stack")).unwrap();
    assert_eq!(stack.visible_child_name().as_deref(), Some("structured"));
    let structured = find::<gtk::Widget>(&widget, Some("structured-view")).unwrap();
    wait_for(|| structured.is_mapped() && !toggles(&structured).is_empty());

    // Only the root is built initially; children appear after the animated expand.
    assert_eq!(toggles(&structured).len(), 1);
    let root = toggles(&structured)[0].clone();
    let revealer = revealer_of(&root);
    assert_eq!(revealer.transition_duration(), 250);
    assert_eq!(
        revealer.transition_type(),
        gtk::RevealerTransitionType::SlideDown
    );
    assert!(!revealer.reveals_child());
    root.emit_clicked();
    wait_for(|| {
        revealer.reveals_child() && revealer.is_child_revealed() && toggles(&structured).len() == 3
    });
    let users = toggles(&structured)
        .into_iter()
        .find(|toggle| key_text(toggle) == "users")
        .unwrap();
    let users_revealer = revealer_of(&users);
    let count = users
        .child()
        .unwrap()
        .last_child()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert_eq!(count.text(), "[] · 2 items");
    assert!(count.is_visible());
    users.emit_clicked();
    assert!(
        count.is_visible(),
        "Expanded arrays must retain their element count"
    );
    wait_for(|| toggles(&structured).len() == 5); // Root, users, metadata, and two record objects.
    // Collapsing hides children but keeps them built; a second expand is instant.
    users.emit_clicked();
    wait_for(|| !users_revealer.reveals_child() && !users_revealer.is_child_revealed());
    assert_eq!(toggles(&structured).len(), 5);
    assert!(count.is_visible());
    users.emit_clicked();
    wait_for(|| users_revealer.reveals_child());

    let events =
        crate::window::create(app, Some(gio::File::for_path(samples.join("events.jsonl"))));
    wait_for(|| {
        events
            .title()
            .is_some_and(|title| title.contains("events.jsonl"))
    });
    let events_widget = events.clone().upcast::<gtk::Widget>();
    let events_structured = find::<gtk::Widget>(&events_widget, Some("structured-view")).unwrap();
    wait_for(|| !toggles(&events_structured).is_empty());
    let events_root = toggles(&events_structured)[0].clone();
    events_root.emit_clicked();
    wait_for(|| toggles(&events_structured).len() == 4); // Root plus three records.
    events.close();
    // Exercise the real menu activation path, not just the window action.
    let controllers = root.observe_controllers();
    let gesture = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|object| {
            object
                .downcast::<gtk::GestureClick>()
                .ok()
                .filter(|click| click.button() == gtk::gdk::BUTTON_SECONDARY)
        })
        .unwrap();
    gesture.emit_by_name::<()>("pressed", &[&1_i32, &0_f64, &0_f64]);
    let popover = find::<gtk::PopoverMenu>(&root.clone().upcast(), None).unwrap();
    fn activate_type(widget: &gtk::Widget) -> bool {
        if widget.type_().name() == "GtkModelButton"
            && find::<gtk::Label>(widget, None).is_some_and(|label| label.text() == "Type")
        {
            widget.emit_by_name::<()>("clicked", &[]);
            return true;
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            if activate_type(&widget) {
                return true;
            }
            child = widget.next_sibling();
        }
        false
    }
    assert!(activate_type(&popover.upcast()));
    wait_for(|| window.visible_dialog().is_some());
    let menu_result = window.visible_dialog().unwrap();
    assert_eq!(menu_result.title(), "jq: Type");
    let menu_view = find::<sourceview5::View>(&menu_result.clone().upcast(), None).unwrap();
    let menu_buffer = menu_view.buffer();
    assert_eq!(
        menu_buffer.text(&menu_buffer.start_iter(), &menu_buffer.end_iter(), true),
        "\"object\""
    );
    menu_result.close();
    gtk::prelude::WidgetExt::activate_action(
        &window,
        "win.node-jq",
        Some(&("/users", "length", "Length").to_variant()),
    )
    .unwrap();
    wait_for(|| window.visible_dialog().is_some());
    let dialog = window.visible_dialog().unwrap();
    assert_eq!(dialog.title(), "jq: Length");
    let result = find::<sourceview5::View>(&dialog.clone().upcast::<gtk::Widget>(), None).unwrap();
    let buffer = result.buffer();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        "2"
    );
    assert!(!result.is_editable());
    dialog.close();
    window.close();
}
