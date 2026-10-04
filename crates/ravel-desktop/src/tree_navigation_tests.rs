use crate::{
    tree_navigation::visible_rows,
    ui_tests::{find, wait_for},
};
use gtk::gdk;
use libadwaita::{self as adw, prelude::*};
use std::sync::Arc;

fn press(controller: &gtk::EventControllerKey, key: gdk::Key) -> bool {
    controller.emit_by_name::<bool>("key-pressed", &[&key, &0_u32, &gdk::ModifierType::empty()])
}
fn focused(window: &adw::ApplicationWindow) -> gtk::Widget {
    gtk::prelude::RootExt::focus(window).unwrap()
}
fn reveal(row: &gtk::Widget) -> gtk::Revealer {
    row.next_sibling().unwrap().downcast().unwrap()
}

pub fn run(app: &adw::Application) {
    let (view, structured) = crate::structured::create();
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let before = gtk::Button::with_label("Before tree");
    let after = gtk::Button::with_label("After tree");
    view.set_vexpand(true);
    outer.append(&before);
    outer.append(&view);
    outer.append(&after);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .content(&outer)
        .default_width(400)
        .default_height(180)
        .build();
    let document = Arc::new(
        ravel_core::Document::parse(
            br#"{"list":[{"name":"Alice"},2],"empty":[],"last":null}"#,
            ravel_core::Format::Json,
        )
        .unwrap(),
    );
    structured.set_content(&document, "");
    window.present();
    let column = find::<gtk::Box>(&view, Some("structured-view")).unwrap();
    wait_for(|| column.is_mapped());
    let keys = (0..column.observe_controllers().n_items())
        .filter_map(|i| column.observe_controllers().item(i))
        .find_map(|object| object.downcast::<gtk::EventControllerKey>().ok())
        .unwrap();
    let root = visible_rows(column.upcast_ref())[0].clone();
    wait_for(|| root.height() > 0 && root.is_mapped() && window.is_active());
    assert!(before.grab_focus());
    assert!(window.child_focus(gtk::DirectionType::TabForward));
    assert_eq!(focused(&window), root);
    assert!(press(&keys, gdk::Key::Right));
    let rows = visible_rows(column.upcast_ref());
    assert_eq!(rows.len(), 4);
    let list = rows[1].clone();
    assert_eq!(focused(&window), list); // One Right opens AND enters.
    assert!(reveal(&root).reveals_child());
    press(&keys, gdk::Key::Right);
    let rows = visible_rows(column.upcast_ref());
    let record = rows[2].clone();
    assert_eq!(focused(&window), record);
    press(&keys, gdk::Key::Right);
    let name = visible_rows(column.upcast_ref())[3].clone();
    assert_eq!(focused(&window), name);
    press(&keys, gdk::Key::Right);
    assert_eq!(focused(&window), name); // Leaves have no children.
    press(&keys, gdk::Key::Left);
    assert_eq!(focused(&window), record);
    press(&keys, gdk::Key::Left);
    assert!(!reveal(&record).reveals_child());
    assert_eq!(focused(&window), record);
    press(&keys, gdk::Key::Down);
    let two = visible_rows(column.upcast_ref())[3].clone();
    assert_eq!(focused(&window), two); // Hidden name is skipped immediately.
    press(&keys, gdk::Key::Up);
    assert_eq!(focused(&window), record);
    press(&keys, gdk::Key::Left);
    assert_eq!(focused(&window), list);
    press(&keys, gdk::Key::Left);
    assert_eq!(visible_rows(column.upcast_ref()).len(), 4);
    press(&keys, gdk::Key::Down);
    let empty = visible_rows(column.upcast_ref())[2].clone();
    assert_eq!(focused(&window), empty);
    press(&keys, gdk::Key::Right);
    assert_eq!(focused(&window), empty);
    press(&keys, gdk::Key::End);
    let last = visible_rows(column.upcast_ref()).last().unwrap().clone();
    assert_eq!(focused(&window), last);
    press(&keys, gdk::Key::Down);
    assert_eq!(focused(&window), last);
    wait_for(|| last.height() > 0 && last.is_mapped());
    if std::env::var("GDK_BACKEND").as_deref() == Ok("x11")
        && std::process::Command::new("xdotool")
            .args(["key", "Tab"])
            .status()
            .is_ok_and(|status| status.success())
    {
        wait_for(|| focused(&window) == after);
        assert!(
            std::process::Command::new("xdotool")
                .args(["key", "shift+Tab"])
                .status()
                .unwrap()
                .success()
        );
        wait_for(|| focused(&window) == last);
        assert!(
            std::process::Command::new("xdotool")
                .args(["key", "Home"])
                .status()
                .unwrap()
                .success()
        );
        wait_for(|| focused(&window) == root);
        assert!(
            std::process::Command::new("xdotool")
                .args(["key", "Right"])
                .status()
                .unwrap()
                .success()
        );
        wait_for(|| focused(&window) == list);
    }
    press(&keys, gdk::Key::Home);
    assert_eq!(focused(&window), root);
    press(&keys, gdk::Key::Left);
    assert_eq!(visible_rows(column.upcast_ref()).len(), 1);
    press(&keys, gdk::Key::Left);
    assert_eq!(focused(&window), root);
    press(&keys, gdk::Key::space);
    assert_eq!(visible_rows(column.upcast_ref()).len(), 4);
    press(&keys, gdk::Key::Return);
    assert_eq!(visible_rows(column.upcast_ref()).len(), 1);
    assert!(!press(&keys, gdk::Key::Tab));
    assert!(!keys.emit_by_name::<bool>(
        "key-pressed",
        &[&gdk::Key::Right, &0_u32, &gdk::ModifierType::CONTROL_MASK]
    ));
    // Reopening preserves nested expansion states, but only visible rows are tab stops.
    press(&keys, gdk::Key::Right);
    press(&keys, gdk::Key::Right);
    press(&keys, gdk::Key::Right);
    root.downcast_ref::<gtk::Button>().unwrap().emit_clicked();
    assert_eq!(focused(&window), root); // Mouse collapse never strands focus.
    assert_eq!(visible_rows(column.upcast_ref()).len(), 1);
    press(&keys, gdk::Key::Right);
    assert_eq!(visible_rows(column.upcast_ref()).len(), 7);
    assert_eq!(
        visible_rows(column.upcast_ref())
            .iter()
            .filter(|row| row.is_focusable())
            .count(),
        1
    );
    // Keyboard menus retain their own arrow navigation.
    let menu_keys = (0..list.observe_controllers().n_items())
        .filter_map(|index| list.observe_controllers().item(index))
        .find_map(|object| object.downcast::<gtk::EventControllerKey>().ok())
        .unwrap();
    assert!(menu_keys.emit_by_name::<bool>(
        "key-pressed",
        &[&gdk::Key::F10, &0_u32, &gdk::ModifierType::SHIFT_MASK]
    ));
    let menu = find::<gtk::PopoverMenu>(&list, None).unwrap();
    wait_for(|| menu.is_mapped());
    assert!(
        !press(&keys, gdk::Key::Down),
        "Tree arrows must not steal menu navigation"
    );
    menu.popdown();
    // Replacing the document starts with one fresh root focus target.
    structured.set_content(&document, "/empty");
    assert_eq!(visible_rows(column.upcast_ref()).len(), 1);
    assert!(visible_rows(column.upcast_ref())[0].is_focusable());
    let long_json = serde_json::to_vec(&(0..40).collect::<Vec<_>>()).unwrap();
    let long_document =
        Arc::new(ravel_core::Document::parse(&long_json, ravel_core::Format::Json).unwrap());
    structured.set_content(&long_document, "");
    visible_rows(column.upcast_ref())[0].grab_focus();
    press(&keys, gdk::Key::Right);
    press(&keys, gdk::Key::End);
    let scroll = view.clone().downcast::<gtk::ScrolledWindow>().unwrap();
    wait_for(|| scroll.vadjustment().value() > 0.0);
    assert_eq!(
        focused(&window),
        visible_rows(column.upcast_ref()).last().unwrap().clone()
    );
    window.close();
}
