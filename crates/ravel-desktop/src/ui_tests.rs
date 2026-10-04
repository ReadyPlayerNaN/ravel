use std::time::{Duration, Instant};

use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};
use sourceview5::prelude::*;

pub(crate) fn find<T: IsA<gtk::Widget> + glib::object::ObjectType>(
    root: &gtk::Widget,
    name: Option<&str>,
) -> Option<T> {
    if name.is_none_or(|name| root.widget_name() == name)
        && let Ok(widget) = root.clone().downcast::<T>()
    {
        return Some(widget);
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

#[track_caller]
pub(crate) fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(
            Instant::now() < deadline,
            "UI did not reach the expected state"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn color_at(buffer: &sourceview5::Buffer, needle: &str) -> gtk::gdk::RGBA {
    buffer.ensure_highlight(&buffer.start_iter(), &buffer.end_iter());
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    let byte_offset = text.find(needle).unwrap();
    let offset = i32::try_from(text[..byte_offset].chars().count()).unwrap();
    buffer
        .iter_at_offset(offset)
        .tags()
        .into_iter()
        .find(|tag| tag.is_foreground_set())
        .unwrap_or_else(|| panic!("No syntax color applied to {needle:?}"))
        .foreground_rgba()
        .unwrap()
}

#[test]
#[ignore = "Requires a display; run with xvfb-run -a dbus-run-session -- cargo test -p ravel-desktop -- --ignored"]
fn opens_documents_searches_filters_and_displays_selected_json() {
    adw::init().unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.ravel.Ravel.Tests")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let samples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let window =
        crate::window::create(&app, Some(gio::File::for_path(samples.join("sample.json"))));
    let widget = window.clone().upcast::<gtk::Widget>();
    let text = find::<sourceview5::View>(&widget, None).unwrap();
    let search = find::<gtk::SearchEntry>(&widget, None).unwrap();
    let kind = find::<gtk::DropDown>(&widget, None).unwrap();
    let list = find::<gtk::ListView>(&widget, Some("structure-tree")).unwrap();
    let selection = list
        .model()
        .unwrap()
        .downcast::<gtk::SingleSelection>()
        .unwrap();
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.json"))
    });
    assert!(!text.is_editable());
    let buffer = text.buffer().downcast::<sourceview5::Buffer>().unwrap();
    assert_eq!(buffer.language().unwrap().id(), "json");
    assert!(buffer.is_highlight_syntax());
    assert!(
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .contains("Alice")
    );

    let styles = adw::StyleManager::default();
    let original_scheme = styles.color_scheme();
    styles.set_color_scheme(adw::ColorScheme::ForceLight);
    assert_eq!(buffer.style_scheme().unwrap().id(), "Adwaita");
    let light_string_color = color_at(&buffer, "Ravel sample document");
    assert_ne!(color_at(&buffer, "title"), light_string_color);
    color_at(&buffer, "32");
    color_at(&buffer, "true");
    color_at(&buffer, "false");
    color_at(&buffer, "null");
    styles.set_color_scheme(adw::ColorScheme::ForceDark);
    assert_eq!(buffer.style_scheme().unwrap().id(), "Adwaita-dark");
    assert_ne!(
        color_at(&buffer, "Ravel sample document"),
        light_string_color
    );

    search.set_text("alice");
    wait_for(|| selection.n_items() == 5); // Description also contains Alice.
    selection.set_selected(4);
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false),
        "\"Alice\""
    );
    color_at(&buffer, "Alice");

    kind.set_selected(4); // Numbers cannot match Alice.
    wait_for(|| selection.n_items() == 0);
    assert!(
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .is_empty()
    );
    search.set_text("");
    wait_for(|| selection.n_items() > 0);
    kind.set_selected(0);
    wait_for(|| selection.n_items() == 6); // Root plus five top-level fields.

    let events = crate::window::create(
        &app,
        Some(gio::File::for_path(samples.join("events.jsonl"))),
    );
    wait_for(|| {
        events
            .title()
            .is_some_and(|title| title.contains("events.jsonl"))
    });
    let events_widget = events.clone().upcast::<gtk::Widget>();
    let events_list = find::<gtk::ListView>(&events_widget, Some("structure-tree")).unwrap();
    assert_eq!(events_list.model().unwrap().n_items(), 4);
    let events_view = find::<sourceview5::View>(&events_widget, None).unwrap();
    let events_buffer = events_view
        .buffer()
        .downcast::<sourceview5::Buffer>()
        .unwrap();
    assert_eq!(events_buffer.language().unwrap().id(), "json");
    assert_eq!(events_buffer.style_scheme().unwrap().id(), "Adwaita-dark");
    color_at(&events_buffer, "warning");
    color_at(&events_buffer, "2");

    let escaped_view = crate::preview::create();
    let escaped_buffer = escaped_view
        .buffer()
        .downcast::<sourceview5::Buffer>()
        .unwrap();
    let escaped_json = r#"{"key":"é \\ \" \u0041","number":-12.5e+2}"#;
    escaped_buffer.set_text(escaped_json);
    color_at(&escaped_buffer, "é");
    color_at(&escaped_buffer, r#"\u0041"#);
    color_at(&escaped_buffer, "-12.5e+2");
    assert_eq!(
        escaped_buffer.text(
            &escaped_buffer.start_iter(),
            &escaped_buffer.end_iter(),
            false
        ),
        escaped_json
    );
    styles.set_color_scheme(adw::ColorScheme::ForceLight);
    assert_eq!(events_buffer.style_scheme().unwrap().id(), "Adwaita");
    assert_eq!(buffer.style_scheme().unwrap().id(), "Adwaita");
    styles.set_color_scheme(original_scheme);
    window.close();
    events.close();
    crate::hover_tests::run(&app);
    crate::fold_tests::run(&app);
}
