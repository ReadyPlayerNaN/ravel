use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};
use webkit6::prelude::*;

use super::{eval, ready};
use crate::{
    tree_navigation::visible_rows,
    ui_tests::{find, wait_for},
};

fn aligned(view: &webkit6::WebView, native: &gtk::Widget, id: usize) -> bool {
    let Some(bounds) = native.compute_bounds(view) else {
        return false;
    };
    let rect = eval(view, &format!("JSON.stringify((() => {{ const r = document.querySelector('[data-json-block=\"{id}\"] .json-structured-slot').getBoundingClientRect(); return [r.x,r.y,r.width,r.height]; }})())")).to_string();
    let rect: Vec<f64> = serde_json::from_str(&rect).unwrap();
    [bounds.x(), bounds.y(), bounds.width(), bounds.height()]
        .iter()
        .zip(rect)
        .all(|(native, html)| (f64::from(*native) - html * view.zoom_level()).abs() <= 2.0)
}

fn slot_height(view: &webkit6::WebView, id: usize) -> f64 {
    eval(view, &format!("document.querySelector('[data-json-block=\"{id}\"] .json-structured-slot').getBoundingClientRect().height")).to_double()
}

fn fitted(view: &webkit6::WebView, native: &gtk::Widget, id: usize) -> bool {
    let content = find::<gtk::Box>(native, Some("structured-view")).unwrap();
    let (_, natural, _, _) = content.measure(gtk::Orientation::Vertical, content.width());
    let expected = (f64::from(natural) / view.zoom_level())
        .ceil()
        .clamp(48.0, 640.0);
    (slot_height(view, id) - expected).abs() < 1.0 && aligned(view, native, id)
}

fn revealer(row: &gtk::Widget) -> gtk::Revealer {
    row.next_sibling().unwrap().downcast().unwrap()
}

pub(super) fn run(app: &adw::Application) {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let path = examples.join("json-blocks.md");
    let source = std::fs::read_to_string(&path).unwrap();
    let (window, viewer) = super::super::build(app);
    window.present();
    viewer.load(gio::File::for_path(&path));
    wait_for(|| !viewer.spinner.is_spinning());
    let view = viewer.markdown.view().unwrap();
    ready(&view);
    wait_for(|| {
        eval(
            &view,
            "document.querySelectorAll('.json-view-toggle').length === 2",
        )
        .to_boolean()
    });
    assert!(eval(&view, "Array.from(document.querySelectorAll('[data-json-block]')).every(b => b.querySelector('pre').hidden && !b.querySelector('.json-structured-slot').hidden && b.querySelector('button').textContent === 'Raw')").to_boolean());
    assert!(eval(&view, "Array.from(document.querySelectorAll('.code-block:not([data-json-block])')).every(b => !b.querySelector('pre').hidden && !b.querySelector('button'))").to_boolean());
    eval(
        &view,
        "document.querySelector('[data-json-block=\"0\"]').scrollIntoView()",
    );
    let container = viewer.markdown.widget();
    wait_for(|| {
        find::<gtk::Widget>(container.upcast_ref(), Some("markdown-json-0"))
            .is_some_and(|widget| widget.is_mapped())
    });
    let native = find::<gtk::Widget>(container.upcast_ref(), Some("markdown-json-0")).unwrap();
    wait_for(|| fitted(&view, &native, 0));
    let initial_height = slot_height(&view, 0);
    let following_top = eval(
        &view,
        "document.querySelector('[data-json-block=\"1\"]').getBoundingClientRect().top + scrollY",
    )
    .to_double();
    let rows = visible_rows(&native);
    assert_eq!(rows.len(), 5); // Same root-expanded native component as a JSON file.
    let users = rows[2].clone().downcast::<gtk::Button>().unwrap();
    users.emit_clicked();
    wait_for(|| revealer(users.upcast_ref()).is_child_revealed() && fitted(&view, &native, 0));
    assert_eq!(visible_rows(&native).len(), 7);
    let expanded_height = slot_height(&view, 0);
    assert!(expanded_height > initial_height && expanded_height < 640.0);
    let shifted_top = eval(
        &view,
        "document.querySelector('[data-json-block=\"1\"]').getBoundingClientRect().top + scrollY",
    )
    .to_double();
    assert!((shifted_top - following_top - (expanded_height - initial_height)).abs() < 2.0);

    // Expanding deeper branches hits the cap and enables internal scrolling.
    let records = visible_rows(&revealer(users.upcast_ref()).child().unwrap());
    for row in &records {
        row.downcast_ref::<gtk::Button>().unwrap().emit_clicked();
    }
    wait_for(|| {
        records.iter().all(|row| revealer(row).is_child_revealed()) && fitted(&view, &native, 0)
    });
    assert_eq!(slot_height(&view, 0), 640.0);
    let adjustment = native
        .downcast_ref::<gtk::ScrolledWindow>()
        .unwrap()
        .vadjustment();
    wait_for(|| adjustment.upper() > adjustment.page_size());
    adjustment.set_value(adjustment.upper() - adjustment.page_size());
    assert!(adjustment.value() > 0.0);

    // Shrinking must use natural content size, not the old scroll viewport size.
    for row in &records {
        row.downcast_ref::<gtk::Button>().unwrap().emit_clicked();
    }
    wait_for(|| {
        records.iter().all(|row| !revealer(row).is_child_revealed()) && fitted(&view, &native, 0)
    });
    assert_eq!(slot_height(&view, 0), expanded_height);
    wait_for(|| adjustment.value() == 0.0);
    let root = rows[0].downcast_ref::<gtk::Button>().unwrap();
    root.emit_clicked();
    wait_for(|| !revealer(root.upcast_ref()).is_child_revealed() && fitted(&view, &native, 0));
    assert!(slot_height(&view, 0) < initial_height);
    root.emit_clicked();
    wait_for(|| revealer(root.upcast_ref()).is_child_revealed() && fitted(&view, &native, 0));
    assert_eq!(slot_height(&view, 0), expanded_height);

    // CSS-pixel sizing remains fitted when WebKit zoom changes independently of GTK fonts.
    view.set_zoom_level(1.25);
    wait_for(|| native.is_mapped() && fitted(&view, &native, 0));
    view.set_zoom_level(1.0);
    eval(
        &view,
        "document.querySelector('[data-json-block=\"0\"]').scrollIntoView()",
    );
    wait_for(|| native.is_mapped() && fitted(&view, &native, 0));
    assert_eq!(slot_height(&view, 0), expanded_height);

    // Only the slot intercepts pointer events; the surrounding page remains WebKit.
    let bounds = native.compute_bounds(&window).unwrap();
    let picked = window
        .pick(
            f64::from(bounds.x() + 30.0),
            f64::from(bounds.y() + 20.0),
            gtk::PickFlags::DEFAULT,
        )
        .unwrap();
    assert!(picked == native || picked.is_ancestor(&native));
    let origin = view
        .compute_point(&window, &gtk::graphene::Point::new(5.0, 5.0))
        .unwrap();
    assert_eq!(
        window
            .pick(
                f64::from(origin.x()),
                f64::from(origin.y()),
                gtk::PickFlags::DEFAULT
            )
            .unwrap(),
        view
    );

    // Unrelated window actions still resolve through the block's local action group.
    let forwarded = std::rc::Rc::new(std::cell::Cell::new(false));
    let parent_action = gio::SimpleAction::new("test-parent", None);
    parent_action.connect_activate({
        let forwarded = std::rc::Rc::clone(&forwarded);
        move |_, _| forwarded.set(true)
    });
    window.add_action(&parent_action);
    native.activate_action("win.test-parent", None).unwrap();
    assert!(forwarded.get());

    // Block-local copy and jq actions must never target the Markdown document or another block.
    native
        .activate_action("win.node-copy", Some(&"/count".to_variant()))
        .unwrap();
    assert_eq!(
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        "123456789012345678901234567890"
    );
    native
        .activate_action("win.node-pointer", Some(&"/a~1b/~0key".to_variant()))
        .unwrap();
    assert_eq!(
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        "/a~1b/~0key"
    );
    native
        .activate_action(
            "win.node-jq",
            Some(&("/users", "length", "Length").to_variant()),
        )
        .unwrap();
    wait_for(|| window.visible_dialog().is_some());
    let dialog = window.visible_dialog().unwrap();
    let output = find::<sourceview5::View>(dialog.upcast_ref(), None)
        .unwrap()
        .buffer();
    assert_eq!(
        output.text(&output.start_iter(), &output.end_iter(), true),
        "2"
    );
    dialog.close();
    assert!(!window.lookup_action("node-jq").unwrap().is_enabled());

    let raw = eval(
        &view,
        "document.querySelector('[data-json-block=\"0\"] pre code').textContent",
    )
    .to_string();
    assert!(raw.starts_with("{\n  \"name\": \"Ravel\",\n"));
    eval(
        &view,
        "document.querySelector('[data-json-block=\"0\"] button').click()",
    );
    wait_for(|| !native.is_visible());
    assert!(eval(&view, "!document.querySelector('[data-json-block=\"0\"] pre').hidden && document.querySelector('[data-json-block=\"0\"] button').textContent === 'Structured'").to_boolean());
    assert_eq!(
        eval(
            &view,
            "document.querySelector('[data-json-block=\"0\"] pre code').textContent"
        )
        .to_string(),
        raw
    );
    assert!(
        eval(
            &view,
            "document.querySelector('[data-json-block=\"1\"] pre').hidden"
        )
        .to_boolean()
    );
    // Zoom changes while Raw is visible are applied when the native tree remaps.
    view.set_zoom_level(1.25);
    eval(
        &view,
        "document.querySelector('[data-json-block=\"0\"] button').click()",
    );
    wait_for(|| native.is_mapped() && fitted(&view, &native, 0));
    assert_eq!(slot_height(&view, 0), (expanded_height / 1.25).ceil());
    view.set_zoom_level(1.0);
    wait_for(|| fitted(&view, &native, 0));
    assert_eq!(slot_height(&view, 0), expanded_height);
    assert_eq!(visible_rows(&native).len(), 7); // Expansion survives Raw/Structured toggles.

    // Switching the whole document to Raw Markdown hides overlays, even across resize.
    viewer.view_stack.set_visible_child_name("raw");
    wait_for(|| !view.is_mapped());
    assert!(!native.is_visible());
    window.set_default_size(900, 740);
    viewer.view_stack.set_visible_child_name("formatted");
    wait_for(|| view.is_mapped() && native.is_mapped() && aligned(&view, &native, 0));
    assert_eq!(visible_rows(&native).len(), 7);

    // DOM focus enters the native tree; Shift+Tab returns to its own header button.
    assert!(view.grab_focus());
    eval(
        &view,
        "document.querySelector('[data-json-block=\"0\"] .json-structured-slot').focus()",
    );
    wait_for(|| {
        gtk::prelude::RootExt::focus(&window).is_some_and(|focus| focus.is_ancestor(&native))
    });
    if std::env::var("GDK_BACKEND").as_deref() == Ok("x11")
        && std::process::Command::new("xdotool")
            .args(["key", "shift+Tab"])
            .status()
            .is_ok_and(|status| status.success())
    {
        wait_for(|| {
            eval(&view, "document.activeElement === document.querySelector('[data-json-block=\"0\"] button')").to_boolean()
                && gtk::prelude::RootExt::focus(&window).as_ref() == Some(view.upcast_ref())
        });
        eval(
            &view,
            "document.querySelector('[data-json-block=\"0\"] .json-structured-slot').focus()",
        );
        wait_for(|| {
            gtk::prelude::RootExt::focus(&window).is_some_and(|focus| focus.is_ancestor(&native))
        });
        assert!(
            std::process::Command::new("xdotool")
                .args(["key", "Tab"])
                .status()
                .unwrap()
                .success()
        );
        wait_for(|| {
            eval(
                &view,
                "document.activeElement.textContent === 'An ordinary link'",
            )
            .to_boolean()
                && gtk::prelude::RootExt::focus(&window).as_ref() == Some(view.upcast_ref())
        });
    }
    // Wheel scrolling at the tree's lower boundary continues through the Markdown page.
    let adjustment = native
        .downcast_ref::<gtk::ScrolledWindow>()
        .unwrap()
        .vadjustment();
    adjustment.set_value(adjustment.upper() - adjustment.page_size());
    let controllers = native.observe_controllers();
    let scroll = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|object| object.downcast::<gtk::EventControllerScroll>().ok())
        .find(|controller| controller.name().as_deref() == Some("markdown-json-scroll"))
        .unwrap();
    let old_scroll = eval(&view, "scrollY").to_double();
    assert!(scroll.emit_by_name::<bool>("scroll", &[&0.0_f64, &1.0_f64]));
    wait_for(|| eval(&view, "scrollY").to_double() > old_scroll);
    window.set_default_size(800, 740);
    wait_for(|| eval(&view, "innerWidth <= 800").to_boolean() && aligned(&view, &native, 0));
    eval(
        &view,
        "window.scrollBy(0, document.querySelector('[data-json-block=\"0\"] .json-structured-slot').getBoundingClientRect().top + 40)",
    );
    wait_for(|| native.is_mapped() && aligned(&view, &native, 0));
    assert!(native.compute_bounds(&view).unwrap().y() < 0.0);
    let header_point = view
        .compute_point(&window, &gtk::graphene::Point::new(30.0, -20.0))
        .unwrap();
    assert!(
        !window
            .pick(
                f64::from(header_point.x()),
                f64::from(header_point.y()),
                gtk::PickFlags::DEFAULT
            )
            .unwrap()
            .is_ancestor(&native)
    );
    eval(
        &view,
        "document.querySelector('[data-json-block=\"1\"]').scrollIntoView()",
    );
    wait_for(|| {
        find::<gtk::Widget>(container.upcast_ref(), Some("markdown-json-1"))
            .is_some_and(|widget| widget.is_mapped())
    });
    let second = find::<gtk::Widget>(container.upcast_ref(), Some("markdown-json-1")).unwrap();
    wait_for(|| aligned(&view, &second, 1));
    second
        .activate_action("win.node-copy", Some(&"/0/second".to_variant()))
        .unwrap();
    assert_eq!(
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        "true"
    );
    eval(&view, "window.scrollTo(0, document.body.scrollHeight)");
    wait_for(|| !native.is_visible());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    let buffer = viewer.text.buffer();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        source
    );

    // A stopped web process must not leave floating native trees over the error view.
    view.terminate_web_process();
    wait_for(|| {
        find::<gtk::Label>(container.upcast_ref(), Some("markdown-error"))
            .is_some_and(|error| error.is_visible())
    });
    assert!(!native.is_visible() && !second.is_visible());

    // Pending old-page layout messages cannot retain overlays after document replacement.
    viewer.load(gio::File::for_path(examples.join("sample.json")));
    wait_for(|| !viewer.spinner.is_spinning());
    assert!(view.parent().is_none());
    assert!(native.parent().is_none());
    assert!(second.parent().is_none());
    assert!(viewer.markdown.view().is_none());
    window.close();
}
