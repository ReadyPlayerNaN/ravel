use gtk::{gdk, glib};
use libadwaita::{self as adw, prelude::*};
use serde_json::{Value, json};

use crate::{key_hover::KeyHover, ui_tests::wait_for};

pub(crate) fn controller(widget: &impl IsA<gtk::Widget>, name: &str) -> gtk::EventController {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items())
        .find_map(|index| {
            let controller = controllers
                .item(index)?
                .downcast::<gtk::EventController>()
                .ok()?;
            (controller.name().as_deref() == Some(name)).then_some(controller)
        })
        .unwrap()
}

pub(crate) fn offsets(buffer: &gtk::TextBuffer, token: &str) -> Vec<i32> {
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    text.match_indices(token)
        .map(|(byte, _)| i32::try_from(text[..byte].chars().count()).unwrap())
        .collect()
}

pub(crate) fn motion_at(view: &sourceview5::View, motion: &gtk::EventController, offset: i32) {
    let rect = view.iter_location(&view.buffer().iter_at_offset(offset));
    let (x, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, rect.x(), rect.y());
    motion.emit_by_name::<()>(
        "motion",
        &[
            &(f64::from(x) + f64::from(rect.width()) / 2.0),
            &(f64::from(y) + f64::from(rect.height()) / 2.0),
        ],
    );
}

fn show(hover: &KeyHover, value: &Value) {
    hover.set_content(value, "", &serde_json::to_string_pretty(value).unwrap());
}

pub fn run(app: &adw::Application) {
    let view = crate::preview::create();
    let scroll = gtk::ScrolledWindow::builder().child(&view).build();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .default_width(700)
        .default_height(480)
        .content(&scroll)
        .build();
    let hover = KeyHover::install(&view, &window);
    let motion = controller(&view, "key-hover-motion");
    let modifiers = controller(&window, "key-hover-modifiers");
    let buffer = view.buffer();
    let tag = buffer.tag_table().lookup("ravel-key-hover").unwrap();
    let document =
        json!({"users": [{"name":"A"}, {"name":"B"}], "metadata":{"name":"C"}, "value":"name"});
    show(&hover, &document);
    window.present();
    wait_for(|| view.is_mapped() && view.visible_rect().height() > 100);
    let names = offsets(&buffer, "\"name\"");
    let tagged = || {
        names
            .iter()
            .map(|offset| buffer.iter_at_offset(offset + 1).has_tag(&tag))
            .collect::<Vec<_>>()
    };

    motion_at(&view, &motion, names[0] + 1);
    wait_for(|| tagged() == [true, true, true, false]);
    // Shift changes the existing hover without moving the pointer or focusing the view.
    // Native GDK events carry the modifier state BEFORE the Shift transition.
    modifiers.emit_by_name::<bool>(
        "key-pressed",
        &[&gdk::Key::Shift_L, &50_u32, &gdk::ModifierType::empty()],
    );
    assert_eq!(tagged(), [true, true, false, false]);
    modifiers.emit_by_name::<bool>("modifiers", &[&gdk::ModifierType::SHIFT_MASK]);
    modifiers.emit_by_name::<bool>(
        "key-pressed",
        &[&gdk::Key::Shift_R, &62_u32, &gdk::ModifierType::SHIFT_MASK],
    );
    modifiers.emit_by_name::<()>(
        "key-released",
        &[&gdk::Key::Shift_L, &50_u32, &gdk::ModifierType::SHIFT_MASK],
    );
    assert_eq!(tagged(), [true, true, false, false]);
    modifiers.emit_by_name::<()>(
        "key-released",
        &[&gdk::Key::Shift_R, &62_u32, &gdk::ModifierType::SHIFT_MASK],
    );
    assert_eq!(tagged(), [true, true, true, false]);
    motion_at(&view, &motion, names[3] + 1); // Same text in a string value is not a key.
    assert_eq!(tagged(), [false; 4]);
    motion_at(&view, &motion, names[0] + 6); // Colon immediately after the quoted key.
    assert_eq!(tagged(), [false; 4]);
    motion_at(&view, &motion, names[0] + 1);
    motion.emit_by_name::<()>("motion", &[&650.0_f64, &20.0_f64]); // Blank margin.
    assert_eq!(tagged(), [false; 4]);
    motion_at(&view, &motion, names[0] + 1);
    motion.emit_by_name::<()>("leave", &[]);
    assert_eq!(tagged(), [false; 4]);

    let styles = adw::StyleManager::default();
    let original_scheme = styles.color_scheme();
    styles.set_color_scheme(adw::ColorScheme::ForceLight);
    let light = tag.background_rgba().unwrap();
    styles.set_color_scheme(adw::ColorScheme::ForceDark);
    assert_ne!(tag.background_rgba().unwrap(), light);
    styles.set_color_scheme(original_scheme);

    let unicode = json!({"users": [{"é\"\\": "A"}, {"é\"\\": "B"}], "metadata": {"é\"\\": "C"}});
    show(&hover, &unicode);
    let escaped = offsets(&buffer, "\"é\\\"\\\\\"");
    assert_eq!(escaped.len(), 3);
    // Theme/content changes invalidate text layout; wait for the new line geometry.
    wait_for(|| {
        view.iter_location(&buffer.iter_at_offset(escaped[2])).y()
            > view.iter_location(&buffer.iter_at_offset(escaped[0])).y()
    });
    motion_at(&view, &motion, escaped[0] + 1);
    assert!(
        escaped
            .iter()
            .all(|offset| buffer.iter_at_offset(*offset).has_tag(&tag))
    );
    modifiers.emit_by_name::<bool>(
        "key-pressed",
        &[&gdk::Key::Shift_L, &50_u32, &gdk::ModifierType::empty()],
    );
    assert!(!buffer.iter_at_offset(escaped[2]).has_tag(&tag));
    modifiers.emit_by_name::<()>(
        "key-released",
        &[&gdk::Key::Shift_L, &50_u32, &gdk::ModifierType::SHIFT_MASK],
    );

    let many = json!({"users": (0..50).map(|index| json!({"name":index})).collect::<Vec<_>>(), "padding":"x".repeat(2000)});
    show(&hover, &many);
    let names = offsets(&buffer, "\"name\"");
    wait_for(|| {
        view.iter_location(&buffer.iter_at_offset(*names.last().unwrap()))
            .y()
            > view.visible_rect().height()
    });
    motion_at(&view, &motion, names[0] + 1);
    assert!(buffer.iter_at_offset(names[0]).has_tag(&tag));
    assert!(!buffer.iter_at_offset(*names.last().unwrap()).has_tag(&tag));
    let origin = view.iter_location(&buffer.iter_at_offset(names[0]));
    let target = view.iter_location(&buffer.iter_at_offset(names[10]));
    scroll
        .vadjustment()
        .set_value(f64::from(target.y() - origin.y()));
    wait_for(|| buffer.iter_at_offset(names[10]).has_tag(&tag));
    assert!(!buffer.iter_at_offset(names[0]).has_tag(&tag));
    assert!(!buffer.iter_at_offset(*names.last().unwrap()).has_tag(&tag));

    scroll.hadjustment().set_value(400.0);
    wait_for(|| {
        names
            .iter()
            .all(|offset| !buffer.iter_at_offset(*offset).has_tag(&tag))
    });
    scroll.hadjustment().set_value(0.0);
    wait_for(|| buffer.iter_at_offset(names[10]).has_tag(&tag));
    // Changing the selected value clears active hover and indexes the new content.
    show(&hover, &json!({"other": 1}));
    assert!((0..buffer.char_count()).all(|offset| !buffer.iter_at_offset(offset).has_tag(&tag)));
    show(&hover, &document);
    scroll.vadjustment().set_value(0.0);
    let names = offsets(&buffer, "\"name\"");
    wait_for(|| {
        view.iter_location(&buffer.iter_at_offset(names[0]))
            .height()
            > 0
    });
    motion_at(&view, &motion, names[2] + 1);
    modifiers.emit_by_name::<bool>("modifiers", &[&gdk::ModifierType::SHIFT_MASK]);
    assert!(buffer.iter_at_offset(names[2]).has_tag(&tag));
    assert!(!buffer.iter_at_offset(names[0]).has_tag(&tag));
    window.close();
    drop(hover);
    // Weak captures remain safe if a window controller outlives its hover state.
    modifiers.emit_by_name::<bool>("modifiers", &[&gdk::ModifierType::empty()]);
    let _ = glib::MainContext::default().iteration(false);
}
