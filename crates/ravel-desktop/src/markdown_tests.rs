use super::build;
use crate::ui_tests::{find, wait_for};
use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};
use sourceview5::prelude::*;

pub(crate) fn run(app: &adw::Application) {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let (window, viewer) = build(app);
    window.present();
    viewer.load(gio::File::for_path(examples.join("sample.json")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.json"))
    });
    viewer.load(gio::File::for_path(examples.join("sample.md")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.md"))
    });

    let widget = window.clone().upcast::<gtk::Widget>();
    let stack = find::<adw::ViewStack>(&widget, Some("view-stack")).unwrap();
    assert_eq!(stack.visible_child_name().as_deref(), Some("formatted"));
    assert!(!viewer.structured_page.is_visible());
    assert!(viewer.markdown_page.is_visible());
    let tree = find::<gtk::Box>(&widget, Some("structured-view")).unwrap();
    assert!(
        tree.first_child().is_none(),
        "Old structured nodes must be released"
    );
    let formatted = find::<gtk::TextView>(&widget, Some("markdown-formatted")).unwrap();
    wait_for(|| formatted.is_mapped());
    assert!(!formatted.is_editable());
    assert_eq!(formatted.wrap_mode(), gtk::WrapMode::WordChar);
    let buffer = formatted.buffer();
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    assert!(text.starts_with("Ravel Markdown\n"));
    assert!(!text.contains("# Ravel"));
    let heading = buffer.tag_table().lookup("heading-1").unwrap();
    assert!(buffer.start_iter().has_tag(&heading));
    assert!(heading.scale() > 1.0);
    let bold = text.find("read-only").unwrap();
    assert!(
        buffer
            .iter_at_offset(text[..bold].chars().count() as i32)
            .has_tag(&buffer.tag_table().lookup("strong").unwrap())
    );
    assert!(text.contains("<script>not executed</script>"));
    assert!(text.contains("[Image: Example image] (https://example.org/image.png)"));
    let cell = text.find("Feature\tStatus").unwrap();
    assert!(
        buffer
            .iter_at_offset(text[..cell].chars().count() as i32)
            .tags()
            .iter()
            .any(|tag| tag.tabs().is_some())
    );

    let styles = adw::StyleManager::default();
    let original_scheme = styles.color_scheme();
    let code = buffer.tag_table().lookup("code-block").unwrap();
    styles.set_color_scheme(adw::ColorScheme::ForceLight);
    let light = code.paragraph_background_rgba().unwrap();
    styles.set_color_scheme(adw::ColorScheme::ForceDark);
    assert_ne!(light, code.paragraph_background_rgba().unwrap());
    styles.set_color_scheme(original_scheme);

    for name in ["node-copy", "node-pointer", "node-jq"] {
        assert!(!window.lookup_action(name).unwrap().is_enabled());
    }
    assert!(!viewer.pointer.is_visible());
    assert!(viewer.status.text().starts_with("Markdown · Read only"));
    assert_eq!(
        viewer.copy.tooltip_text().as_deref(),
        Some("Copy Markdown source")
    );
    viewer.copy.emit_clicked();
    let source = std::fs::read_to_string(examples.join("sample.md")).unwrap();
    assert_eq!(
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        source
    );
    buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
    formatted.emit_by_name::<()>("copy-clipboard", &[]);
    assert_eq!(
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        text
    );

    stack.set_visible_child_name("raw");
    wait_for(|| viewer.text.is_mapped());
    let raw = viewer
        .text
        .buffer()
        .downcast::<sourceview5::Buffer>()
        .unwrap();
    assert_eq!(raw.language().unwrap().id(), "markdown");
    assert!(!viewer.text.is_editable());
    assert_eq!(raw.text(&raw.start_iter(), &raw.end_iter(), true), source);
    assert_eq!(viewer.raw_page.title().as_deref(), Some("Raw Markdown"));
    let folds = find::<sourceview5::GutterRendererText>(&widget, Some("code-folds")).unwrap();
    assert!(!folds.is_visible());

    // Failed UTF-8 opens preserve the existing Markdown document and selected tab.
    let directory = std::env::temp_dir().join(format!(
        "ravel-markdown-tests-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let invalid = directory.join("invalid.md");
    std::fs::write(&invalid, [0xff]).unwrap();
    viewer.load(gio::File::for_path(&invalid));
    wait_for(|| window.visible_dialog().is_some());
    let alert = window
        .visible_dialog()
        .unwrap()
        .downcast::<adw::AlertDialog>()
        .unwrap();
    assert!(alert.body().contains("not UTF-8"));
    assert!(window.title().unwrap().contains("sample.md"));
    assert_eq!(stack.visible_child_name().as_deref(), Some("raw"));
    assert_eq!(
        viewer
            .text
            .buffer()
            .text(&raw.start_iter(), &raw.end_iter(), true),
        source
    );
    alert.close();

    // Switching back restores JSON-only behavior and releases Markdown text.
    viewer.load(gio::File::for_path(examples.join("sample.json")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.json"))
    });
    assert_eq!(stack.visible_child_name().as_deref(), Some("structured"));
    assert!(!viewer.markdown_page.is_visible());
    assert!(viewer.structured_page.is_visible());
    assert_eq!(formatted.buffer().char_count(), 0);
    assert!(viewer.pointer.is_visible());
    assert!(folds.is_visible());
    assert_eq!(raw.language().unwrap().id(), "json");
    assert!(window.lookup_action("node-jq").unwrap().is_enabled());
    assert_eq!(viewer.copy.tooltip_text().as_deref(), Some("Copy JSON"));

    let empty = directory.join("empty.MARKDOWN");
    std::fs::write(&empty, b"\xef\xbb\xbf").unwrap();
    // A newer open wins even if an earlier parse is still pending.
    viewer.load(gio::File::for_path(examples.join("sample.md")));
    viewer.load(gio::File::for_path(&empty));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("empty.MARKDOWN"))
    });
    assert_eq!(formatted.buffer().char_count(), 0);
    assert_eq!(stack.visible_child_name().as_deref(), Some("formatted"));
    assert_eq!(viewer.text.buffer().char_count(), 0);
    viewer.load(gio::File::for_path(examples.join("sample.yaml")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.yaml"))
    });
    assert_eq!(stack.visible_child_name().as_deref(), Some("structured"));
    assert_eq!(viewer.raw_page.title().as_deref(), Some("Raw YAML"));
    assert!(!folds.is_visible());
    window.close();
    std::fs::remove_dir_all(directory).unwrap();
}
