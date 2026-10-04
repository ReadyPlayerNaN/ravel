use crate::ui_tests::{find, wait_for};
use gtk::gio;
use libadwaita::{self as adw, prelude::*};
use sourceview5::prelude::*;

pub fn run(app: &adw::Application) {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let window =
        crate::window::create(app, Some(gio::File::for_path(examples.join("sample.yaml"))));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.yaml"))
    });
    let widget = window.clone().upcast::<gtk::Widget>();
    let stack = find::<adw::ViewStack>(&widget, Some("view-stack")).unwrap();
    assert_eq!(stack.visible_child_name().as_deref(), Some("structured"));
    let status = find::<gtk::Label>(&widget, Some("document-statistics")).unwrap();
    assert!(
        status
            .text()
            .starts_with("YAML · 1 document · Normalized to JSON")
    );
    let source = find::<sourceview5::View>(&widget, None).unwrap();
    let buffer = source.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["country"], "NO");
    assert_eq!(value["production"]["enabled"], true);
    assert_eq!(value["production"]["retries"], 5);
    assert!(!source.is_editable());
    let scroll = source.ancestor(gtk::ScrolledWindow::static_type()).unwrap();
    assert_eq!(stack.page(&scroll).title().as_deref(), Some("Raw (JSON)"));
    assert_eq!(
        buffer
            .downcast::<sourceview5::Buffer>()
            .unwrap()
            .language()
            .unwrap()
            .id(),
        "json"
    );
    gtk::prelude::WidgetExt::activate_action(
        &window,
        "win.node-jq",
        Some(&("/users", "length", "Length").to_variant()),
    )
    .unwrap();
    wait_for(|| window.visible_dialog().is_some());
    let dialog = window.visible_dialog().unwrap();
    assert_eq!(dialog.title(), "jq: Length");
    let result = find::<sourceview5::View>(&dialog.clone().upcast(), None)
        .unwrap()
        .buffer();
    assert_eq!(
        result.text(&result.start_iter(), &result.end_iter(), true),
        "2"
    );
    dialog.close();
    window.close();

    let stream = crate::window::create(
        app,
        Some(gio::File::for_path(examples.join("resources.yml"))),
    );
    wait_for(|| {
        stream
            .title()
            .is_some_and(|title| title.contains("resources.yml"))
    });
    let widget = stream.clone().upcast::<gtk::Widget>();
    let status = find::<gtk::Label>(&widget, Some("document-statistics")).unwrap();
    assert!(status.text().starts_with("YAML · 3 documents"));
    let source = find::<sourceview5::View>(&widget, None).unwrap();
    let buffer = source.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 3);
    assert_eq!(value[1]["spec"]["ports"][0]["port"], 8080);
    assert!(value[2].is_null());
    stream.close();

    // Parser diagnostics reach the native alert without showing a partial tree.
    let directory = std::env::temp_dir().join(format!(
        "ravel-yaml-tests-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("invalid.yaml");
    std::fs::write(&path, "items: [1, 2\n").unwrap();
    let invalid = crate::window::create(app, Some(gio::File::for_path(&path)));
    wait_for(|| invalid.visible_dialog().is_some());
    let alert = invalid
        .visible_dialog()
        .unwrap()
        .downcast::<adw::AlertDialog>()
        .unwrap();
    assert!(alert.body().starts_with("Invalid YAML:"));
    assert!(
        find::<gtk::Widget>(&invalid.clone().upcast(), Some("structured-view"))
            .is_none_or(|tree| !tree.is_mapped())
    );
    alert.close();
    invalid.close();
    std::fs::remove_dir_all(directory).unwrap();
}
