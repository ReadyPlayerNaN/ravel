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
    let tree = find::<gtk::Widget>(&widget, Some("structured-view")).unwrap();
    let rows = crate::tree_navigation::visible_rows(&tree);
    assert_eq!(rows.len(), 9); // Root plus eight YAML keys, without expanding nested mappings.
    let root_revealer = rows[0]
        .next_sibling()
        .unwrap()
        .downcast::<gtk::Revealer>()
        .unwrap();
    assert!(root_revealer.reveals_child());
    let status = find::<gtk::Label>(&widget, Some("document-statistics")).unwrap();
    assert!(status.text().starts_with("YAML · 1 document"));
    let source = find::<sourceview5::View>(&widget, None).unwrap();
    let buffer = source.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    assert_eq!(
        text.as_str(),
        std::fs::read_to_string(examples.join("sample.yaml")).unwrap()
    );
    assert!(
        text.contains("# YAML") && text.contains("&defaults") && text.contains("<<: *defaults")
    );
    assert!(!source.is_editable());
    let scroll = source.ancestor(gtk::ScrolledWindow::static_type()).unwrap();
    assert_eq!(stack.page(&scroll).title().as_deref(), Some("Raw YAML"));
    stack.set_property("visible-child-name", "raw");
    wait_for(|| source.is_mapped());
    assert_eq!(
        buffer
            .downcast::<sourceview5::Buffer>()
            .unwrap()
            .language()
            .unwrap()
            .id(),
        "yaml"
    );
    let syntax = source.buffer().downcast::<sourceview5::Buffer>().unwrap();
    syntax.ensure_highlight(&syntax.start_iter(), &syntax.end_iter());
    let colored = |needle: &str| {
        let byte = text.find(needle).unwrap();
        syntax
            .iter_at_offset(text[..byte].chars().count() as i32)
            .tags()
            .into_iter()
            .any(|tag| tag.is_foreground_set())
    };
    assert!(colored("# YAML"));
    assert!(colored("true"));
    let folds = find::<sourceview5::GutterRendererText>(&widget, Some("code-folds")).unwrap();
    assert!(
        !folds.is_visible(),
        "JSON delimiter folding must not operate on YAML source"
    );
    let copy = find::<gtk::Button>(&widget, Some("copy-json")).unwrap();
    assert_eq!(copy.tooltip_text().as_deref(), Some("Copy YAML source"));
    copy.emit_clicked();
    assert_eq!(
        gtk::glib::MainContext::default()
            .block_on(source.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        text
    );
    gtk::prelude::WidgetExt::activate_action(
        &window,
        "win.node-copy",
        Some(&"/production".to_variant()),
    )
    .unwrap();
    let node_json = gtk::glib::MainContext::default()
        .block_on(source.clipboard().read_text_future())
        .unwrap()
        .unwrap();
    let node: serde_json::Value = serde_json::from_str(&node_json).unwrap();
    assert_eq!(node["enabled"], true);
    assert_eq!(node["retries"], 5);
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
    let tree = find::<gtk::Widget>(&widget, Some("structured-view")).unwrap();
    assert_eq!(crate::tree_navigation::visible_rows(&tree).len(), 4); // Root plus three stream slots.
    let status = find::<gtk::Label>(&widget, Some("document-statistics")).unwrap();
    assert!(status.text().starts_with("YAML · 3 documents"));
    let source = find::<sourceview5::View>(&widget, None).unwrap();
    let buffer = source.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    assert_eq!(
        text.as_str(),
        std::fs::read_to_string(examples.join("resources.yml")).unwrap()
    );
    assert_eq!(text.matches("---").count(), 3);
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
