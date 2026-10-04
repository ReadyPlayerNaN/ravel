use gtk::{gdk, gio, glib};
use libadwaita::{self as adw, prelude::*};
use sourceview5::prelude::*;

use crate::{
    hover_tests::{controller, motion_at, offsets},
    ui_tests::{find, wait_for},
};

fn text(buffer: &gtk::TextBuffer, hidden: bool) -> glib::GString {
    buffer.text(&buffer.start_iter(), &buffer.end_iter(), hidden)
}

fn toggle(view: &sourceview5::View, renderer: &sourceview5::GutterRendererText, line: i32) {
    let iter = view.buffer().iter_at_line(line).unwrap();
    let rect = view.iter_location(&iter);
    assert!(renderer.query_activatable(&iter, &rect));
    GutterRendererExt::activate(renderer, &iter, &rect, 1, gdk::ModifierType::empty(), 1);
}

fn clipboard(view: &sourceview5::View) -> glib::GString {
    glib::MainContext::default()
        .block_on(view.clipboard().read_text_future())
        .unwrap()
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
    let view = find::<sourceview5::View>(&widget, None).unwrap();
    let renderer = find::<sourceview5::GutterRendererText>(&widget, Some("code-folds")).unwrap();
    let buffer = view.buffer();
    let original = text(&buffer, true);
    let names = offsets(&buffer, "\"name\"");
    let first_object_line = buffer.iter_at_offset(names[0]).line() - 1;
    let users_line = first_object_line - 1;
    let motion = controller(&view, "key-hover-motion");
    let tag = buffer.tag_table().lookup("ravel-key-hover").unwrap();
    wait_for(|| view.is_mapped() && view.visible_rect().height() > 200);

    toggle(&view, &renderer, first_object_line);
    assert!(!text(&buffer, false).contains("Alice"));
    assert!(text(&buffer, false).contains("Bob"));
    assert_eq!(text(&buffer, true), original);
    // Hidden keys do not participate in hover highlighting, even though their
    // logical offsets remain between visible lines in the buffer.
    wait_for(|| {
        view.iter_location(&buffer.iter_at_offset(names[1])).y()
            < view.iter_location(&buffer.iter_at_offset(names[2])).y()
    });
    motion_at(&view, &motion, names[1] + 1);
    wait_for(|| buffer.iter_at_offset(names[1]).has_tag(&tag));
    assert!(!buffer.iter_at_offset(names[0]).has_tag(&tag));
    assert!(buffer.iter_at_offset(names[2]).has_tag(&tag));

    toggle(&view, &renderer, users_line);
    assert!(text(&buffer, false).contains("\"users\": [\n  ]"));
    assert!(!text(&buffer, false).contains("Bob"));
    toggle(&view, &renderer, users_line);
    assert!(!text(&buffer, false).contains("Alice")); // Child remains folded.
    assert!(text(&buffer, false).contains("Bob"));

    toggle(&view, &renderer, 0);
    assert_eq!(text(&buffer, false), "{\n}");
    // Hidden child openers must not leave phantom arrows on the closing line.
    let child = buffer.iter_at_line(users_line).unwrap();
    assert!(!renderer.query_activatable(&child, &view.iter_location(&child)));
    renderer.emit_by_name::<()>(
        "query-data",
        &[
            &buffer.clone().upcast::<glib::Object>(),
            &(users_line as u32),
        ],
    );
    assert_eq!(renderer.text().as_deref(), Some(""));
    assert_eq!(text(&buffer, true), original);
    let copy = find::<gtk::Button>(&widget, Some("copy-json")).unwrap();
    copy.emit_clicked();
    assert_eq!(clipboard(&view), original);
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    view.emit_by_name::<()>("copy-clipboard", &[]);
    assert_eq!(clipboard(&view), original);
    toggle(&view, &renderer, 0);
    assert!(!text(&buffer, false).contains("Alice"));
    toggle(&view, &renderer, first_object_line);
    assert_eq!(text(&buffer, false), original);

    // Tag lifetime must not accumulate across repeated folds.
    let tag_count = buffer.tag_table().size();
    for _ in 0..10 {
        toggle(&view, &renderer, 0);
        toggle(&view, &renderer, 0);
    }
    assert_eq!(buffer.tag_table().size(), tag_count);
    toggle(&view, &renderer, 0);
    let list = find::<gtk::ListView>(&widget, Some("structure-tree")).unwrap();
    let selection = list
        .model()
        .unwrap()
        .downcast::<gtk::SingleSelection>()
        .unwrap();
    selection.set_selected(1); // The users array replaces the folded root.
    assert!(text(&buffer, false).contains("Alice"));
    assert_eq!(text(&buffer, false), text(&buffer, true));
    toggle(&view, &renderer, 0);
    assert_eq!(text(&buffer, false), "[\n]");
    selection.set_selected(3); // A string scalar has no fold control.
    assert_eq!(text(&buffer, false), "\"name\"");
    let start = buffer.start_iter();
    assert!(!renderer.query_activatable(&start, &view.iter_location(&start)));
    assert!(!view.is_editable());

    let events =
        crate::window::create(app, Some(gio::File::for_path(samples.join("events.jsonl"))));
    wait_for(|| {
        events
            .title()
            .is_some_and(|title| title.contains("events.jsonl"))
    });
    let events_widget = events.clone().upcast::<gtk::Widget>();
    let events_view = find::<sourceview5::View>(&events_widget, None).unwrap();
    let events_renderer =
        find::<sourceview5::GutterRendererText>(&events_widget, Some("code-folds")).unwrap();
    let events_buffer = events_view.buffer();
    let events_original = text(&events_buffer, true);
    toggle(&events_view, &events_renderer, 0);
    assert_eq!(text(&events_buffer, false), "[\n]");
    toggle(&events_view, &events_renderer, 0);
    assert_eq!(text(&events_buffer, false), events_original);
    events.close();
    window.close();
}
