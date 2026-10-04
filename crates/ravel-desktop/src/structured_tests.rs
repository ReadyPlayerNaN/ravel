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
    users.emit_clicked();
    wait_for(|| toggles(&structured).len() == 5); // Root, users, metadata, and two record objects.
    // Collapsing hides children but keeps them built; a second expand is instant.
    users.emit_clicked();
    wait_for(|| !users_revealer.reveals_child() && !users_revealer.is_child_revealed());
    assert_eq!(toggles(&structured).len(), 5);
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
    window.close();
}
