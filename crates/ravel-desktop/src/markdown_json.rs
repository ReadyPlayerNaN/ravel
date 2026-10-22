//! Native structured trees embedded over bounded, document-local WebKit slots.
//! The bridge accepts only slot geometry/focus IDs, never paths, code or queries.
use std::{cell::{Cell, RefCell}, rc::Rc, sync::Arc};

use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};
use ravel_core::Document;
use webkit6::{self as webkit, prelude::*};

mod imp {
    use gtk::{glib, subclass::prelude::*};

    #[derive(Default)]
    pub struct JsonLayer;

    #[glib::object_subclass]
    impl ObjectSubclass for JsonLayer {
        const NAME: &'static str = "RavelMarkdownJsonLayer";
        type Type = super::JsonLayer;
        type ParentType = gtk::Fixed;
    }
    impl ObjectImpl for JsonLayer {}
    impl WidgetImpl for JsonLayer {
        // GTK picks descendants first. Empty parts of this layer must pass
        // pointer events through to the WebView, rather than cover the page.
        fn contains(&self, _x: f64, _y: f64) -> bool { false }
    }
    impl FixedImpl for JsonLayer {}
}

glib::wrapper! {
    pub struct JsonLayer(ObjectSubclass<imp::JsonLayer>)
        @extends gtk::Fixed, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

struct Slot {
    document: Arc<Document>,
    height: i32,
    widget: RefCell<Option<gtk::Widget>>,
}

pub struct JsonBlocks {
    pub overlay: gtk::Overlay,
    layer: JsonLayer,
    view: glib::WeakRef<webkit::WebView>,
    window: glib::WeakRef<adw::ApplicationWindow>,
    slots: Vec<Slot>,
    active: Cell<bool>,
}

impl JsonBlocks {
    pub fn new(view: &webkit::WebView, window: &adw::ApplicationWindow, documents: &[Arc<Document>]) -> Rc<Self> {
        let overlay = gtk::Overlay::builder().child(view).vexpand(true).hexpand(true).build();
        let layer: JsonLayer = glib::Object::new();
        layer.set_overflow(gtk::Overflow::Hidden);
        layer.set_widget_name("markdown-json-layer");
        overlay.add_overlay(&layer);
        overlay.set_clip_overlay(&layer, true);
        let slots = documents.iter().map(|document| {
            let children = document.value_at("").map_or(0, |value| match value {
                serde_json::Value::Object(values) => values.len(),
                serde_json::Value::Array(values) => values.len(),
                _ => 0,
            });
            Slot { document: Arc::clone(document), height: ((children + 1).clamp(2, 9) * 34 + 24) as i32, widget: RefCell::new(None) }
        }).collect();
        let blocks = Rc::new(Self { overlay, layer, view: view.downgrade(), window: window.downgrade(), slots, active: Cell::new(false) });
        let Some(manager) = view.user_content_manager() else { return blocks; };
        if !manager.register_script_message_handler("ravelJson", None) {
            return blocks; // Raw blocks remain usable if the bridge cannot be registered.
        }
        manager.connect_script_message_received(Some("ravelJson"), glib::clone!(
            #[weak]
            blocks,
            move |_, message| blocks.message(&message.to_string())
        ));
        view.connect_load_changed(glib::clone!(
            #[weak]
            blocks,
            move |view, event| {
                if event == webkit::LoadEvent::Finished {
                    blocks.active.set(true);
                    let heights: Vec<_> = blocks.slots.iter().map(|slot| slot.height).collect();
                    run_script(view, &format!("window.ravelJson?.enable({heights:?});"));
                } else if event == webkit::LoadEvent::Started {
                    blocks.deactivate();
                }
            }
        ));
        view.connect_web_process_terminated(glib::clone!(#[weak] blocks, move |_, _| blocks.deactivate()));
        view.connect_unmap(glib::clone!(#[weak] blocks, move |_| blocks.hide()));
        view.connect_map(|view| run_script(view, "window.dispatchEvent(new Event('resize'));"));
        blocks
    }

    fn hide(&self) {
        for slot in &self.slots {
            if let Some(widget) = slot.widget.borrow().as_ref() { widget.set_visible(false); }
        }
    }

    fn deactivate(&self) {
        self.active.set(false);
        self.hide();
    }

    pub fn clear(&self) {
        self.deactivate();
        if let Some(manager) = self.view.upgrade().and_then(|view| view.user_content_manager()) {
            manager.unregister_script_message_handler("ravelJson", None);
        }
        self.overlay.set_child(None::<&gtk::Widget>);
    }

    fn message(&self, text: &str) {
        if !self.active.get() || text.len() > 8192 { return; }
        let Ok(message) = serde_json::from_str::<serde_json::Value>(text) else { return; };
        let Some(view) = self.view.upgrade().filter(|view| view.is_mapped()) else { return; };
        if let Some(id) = message.get("focus").and_then(|value| value.as_u64()).and_then(|id| usize::try_from(id).ok()) {
            if let Some(slot) = self.slots.get(id)
                && let Some(widget) = slot.widget.borrow().as_ref()
                && widget.is_visible()
                && let Some(row) = crate::tree_navigation::visible_rows(widget).into_iter().find(|row| row.is_focusable())
            {
                crate::tree_navigation::focus(&row);
            }
            return;
        }
        let Some(rows) = message.get("layout").and_then(|value| value.as_array()) else { return; };
        if rows.len() > self.slots.len() { return; }
        let mut visible = vec![false; self.slots.len()];
        for row in rows {
            let Some((id, rect)) = geometry(row, self.slots.len()) else { continue; };
            let [x, y, width, height] = rect.map(|value| value * view.zoom_level());
            if x + width <= 0.0 || y + height <= 0.0 || x >= f64::from(view.width()) || y >= f64::from(view.height()) { continue; }
            let widget = self.widget(id);
            widget.set_size_request(width.round() as i32, height.round() as i32);
            self.layer.move_(&widget, x, y);
            widget.set_visible(true);
            visible[id] = true;
        }
        for (slot, visible) in self.slots.iter().zip(visible) {
            if !visible && let Some(widget) = slot.widget.borrow().as_ref() { widget.set_visible(false); }
        }
    }

    fn widget(&self, id: usize) -> gtk::Widget {
        let slot = &self.slots[id];
        if let Some(widget) = slot.widget.borrow().as_ref() { return widget.clone(); }
        let (widget, tree) = crate::structured::create_embedded();
        tree.set_content(&slot.document, "");
        widget.set_widget_name(&format!("markdown-json-{id}"));
        widget.add_css_class("view");
        install_actions(&widget, &slot.document, &self.window);
        install_navigation(&widget, &self.view, id);
        self.layer.put(&widget, 0.0, 0.0);
        *slot.widget.borrow_mut() = Some(widget.clone());
        widget
    }
}

fn geometry(value: &serde_json::Value, count: usize) -> Option<(usize, [f64; 4])> {
    let row = value.as_array()?;
    if row.len() != 5 { return None; }
    let id = usize::try_from(row[0].as_u64()?).ok()?;
    if id >= count { return None; }
    let rect = [row[1].as_f64()?, row[2].as_f64()?, row[3].as_f64()?, row[4].as_f64()?];
    if rect.iter().any(|number| !number.is_finite() || number.abs() > 100_000.0)
        || rect[2] < 1.0 || rect[2] > 16384.0 || rect[3] < 1.0 || rect[3] > 1024.0
    { return None; }
    Some((id, rect))
}

fn run_script(view: &webkit::WebView, script: &str) {
    view.evaluate_javascript(script, None, None, None::<&gio::Cancellable>, |_| {});
}

fn install_navigation(widget: &gtk::Widget, view: &glib::WeakRef<webkit::WebView>, id: usize) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        widget,
        #[strong]
        view,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, modifiers| {
            if !matches!(key, gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab)
                || modifiers.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK | gtk::gdk::ModifierType::SUPER_MASK)
                || widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus.ancestor(gtk::Popover::static_type()).is_some())
            { return glib::Propagation::Proceed; }
            if let Some(view) = view.upgrade() {
                let backward = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                // Move DOM focus off the slot before refocusing WebKit, otherwise
                // its focus event can immediately send us back into the tree.
                view.evaluate_javascript(
                    &format!("window.ravelJson?.exit({id}, {backward});"),
                    None, None, None::<&gio::Cancellable>,
                    glib::clone!(#[weak] view, move |_| { view.grab_focus(); }),
                );
            }
            glib::Propagation::Stop
        }
    ));
    widget.add_controller(keys);
    // At the inner scroll boundary, continue scrolling the surrounding Markdown.
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    scroll.set_name(Some("markdown-json-scroll"));
    scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    scroll.connect_scroll(glib::clone!(
        #[weak]
        widget,
        #[strong]
        view,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, _, dy| {
            let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() else { return glib::Propagation::Proceed; };
            let adjustment = scroll.vadjustment();
            let at_edge = if dy < 0.0 { adjustment.value() <= adjustment.lower() } else { adjustment.value() >= adjustment.upper() - adjustment.page_size() };
            if at_edge && dy.is_finite() && let Some(view) = view.upgrade() {
                run_script(&view, &format!("window.scrollBy(0, {});", (dy * 40.0).clamp(-1000.0, 1000.0)));
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    ));
    widget.add_controller(scroll);
}

fn install_actions(widget: &gtk::Widget, document: &Arc<Document>, window: &glib::WeakRef<adw::ApplicationWindow>) {
    let actions = gio::SimpleActionGroup::new();
    for (name, pointer_only) in [("node-copy", false), ("node-pointer", true)] {
        let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
        action.connect_activate(glib::clone!(
            #[weak]
            widget,
            #[strong]
            document,
            move |_, target| {
                let Some(pointer) = target.and_then(|value| value.str()) else { return; };
                let text = if pointer_only { Some(pointer.to_owned()) } else { document.value_at(pointer).and_then(|value| serde_json::to_string_pretty(value).ok()) };
                if let Some(text) = text { widget.clipboard().set_text(&text); }
            }
        ));
        actions.add_action(&action);
    }
    let jq = gio::SimpleAction::new("node-jq", Some(&<(String, String, String)>::static_variant_type()));
    jq.connect_activate(glib::clone!(
        #[strong]
        document,
        #[strong]
        window,
        move |_, target| {
            let Some((pointer, program, label)) = target.and_then(|value| value.get::<(String, String, String)>()) else { return; };
            let Some(input) = document.value_at(&pointer).and_then(|value| serde_json::to_string(value).ok()) else { return; };
            let window = window.clone();
            glib::spawn_future_local(async move {
                let result = crate::jq::run(&program, &input).await;
                if let Some(window) = window.upgrade() {
                    match result {
                        Ok(output) => crate::jq::show_result(&window, &format!("jq: {label}"), &output),
                        Err(error) => {
                            let dialog = adw::AlertDialog::new(Some("Could not run jq"), Some(&error));
                            dialog.add_response("close", "Close");
                            dialog.present(Some(&window));
                        }
                    }
                }
            });
        }
    ));
    actions.add_action(&jq);
    widget.insert_action_group("win", Some(&actions));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bridge_geometry_rejects_unknown_ids_and_unbounded_or_malformed_rectangles() {
        assert_eq!(geometry(&json!([0, 10, -20, 400, 300]), 1), Some((0, [10.0, -20.0, 400.0, 300.0])));
        for value in [json!([1, 0, 0, 100, 100]), json!([0, 0, 0, -1, 100]), json!([0, 0, 0, 100, 100000]), json!([0, 0, 0]), json!({"path":"/etc/passwd"})] {
            assert!(geometry(&value, 1).is_none());
        }
    }
}
