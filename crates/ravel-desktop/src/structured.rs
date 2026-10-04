//! Interactive JSON rendering with one GTK widget per node.

use std::{cell::Cell, rc::Rc, sync::Arc};

use gtk::{glib, pango};
use libadwaita::prelude::*;
use ravel_core::{Document, Kind};
use serde_json::Value;

/// Animated expand/collapse duration in milliseconds.
const EXPAND_MS: u32 = 250;
/// Horizontal indent per nesting level, in pixels.
const INDENT: i32 = 20;

pub struct Structured {
    column: gtk::Box,
}

/// Creates the scrollable structured view and its content handle.
pub fn create() -> (gtk::Widget, Rc<Structured>) {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(6)
        .margin_end(6)
        .build();
    column.set_widget_name("structured-view");
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&column)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    (scrolled.upcast(), Rc::new(Structured { column }))
}

impl Structured {
    /// Renders `document.pointer(pointer)` as a widget tree, replacing prior content.
    pub fn set_content(&self, document: &Arc<Document>, pointer: &str) {
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        if document.value_at(pointer).is_some() {
            self.column
                .append(&node_widget(document, pointer, &label_for(pointer), 0));
        }
    }
}

fn node_widget(document: &Arc<Document>, pointer: &str, label: &str, depth: i32) -> gtk::Widget {
    let widget = match document.value_at(pointer) {
        Some(Value::Object(members)) if !members.is_empty() => container_widget(
            document,
            pointer,
            label,
            depth,
            &format!("{{}} · {} members", members.len()),
        ),
        Some(Value::Array(items)) if !items.is_empty() => container_widget(
            document,
            pointer,
            label,
            depth,
            &format!("[] · {} items", items.len()),
        ),
        _ => scalar_widget(document, pointer, label, depth),
    };
    if let Some(kind) = document.value_at(pointer).map(Kind::of) {
        // Container menus belong to the header, not its descendants.
        let target = widget
            .first_child()
            .filter(|child| child.is::<gtk::Button>())
            .unwrap_or_else(|| widget.clone());
        crate::jq::attach_menu(&target, kind, pointer);
    }
    widget
}

fn container_widget(
    document: &Arc<Document>,
    pointer: &str,
    label: &str,
    depth: i32,
    summary: &str,
) -> gtk::Widget {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let toggle = gtk::Button::builder()
        .css_classes(["flat"])
        .halign(gtk::Align::Start)
        .build();
    toggle.set_widget_name("node-toggle");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_margin_start(depth * INDENT);
    let arrow = gtk::Image::from_icon_name("pan-end-symbolic");
    arrow.set_widget_name("node-arrow");
    let key = key_label(label);
    let count = gtk::Label::builder()
        .label(summary)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    row.append(&arrow);
    row.append(&key);
    row.append(&count);
    toggle.set_child(Some(&row));

    // Children are built on first expansion so closed branches stay cheap.
    let children = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let revealer = gtk::Revealer::builder()
        .child(&children)
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(EXPAND_MS)
        .reveal_child(false)
        .build();
    let built = Rc::new(Cell::new(false));
    let is_array = document.value_at(pointer).is_some_and(Value::is_array);
    let pointer = pointer.to_owned();
    toggle.connect_clicked(glib::clone!(
        #[weak]
        children,
        #[weak]
        revealer,
        #[strong]
        document,
        #[strong]
        pointer,
        #[strong]
        built,
        move |_| {
            if !built.get() {
                built.set(true);
                for (child_pointer, child_label) in member_pointers(&document, &pointer) {
                    children.append(&node_widget(
                        &document,
                        &child_pointer,
                        &child_label,
                        depth + 1,
                    ));
                }
            }
            revealer_toggle(&revealer);
        }
    ));
    revealer.connect_reveal_child_notify(glib::clone!(
        #[weak]
        arrow,
        #[weak]
        count,
        move |revealer| {
            arrow.set_icon_name(Some(if revealer.reveals_child() {
                "pan-down-symbolic"
            } else {
                "pan-end-symbolic"
            }));
            // Array lengths remain useful context even while their items are shown.
            count.set_visible(is_array || !revealer.reveals_child());
        }
    ));

    column.append(&toggle);
    column.append(&revealer);
    column.upcast()
}

fn scalar_widget(document: &Arc<Document>, pointer: &str, label: &str, depth: i32) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_margin_start(depth * INDENT);
    row.set_margin_top(2);
    row.set_margin_bottom(2);
    let key = key_label(label);
    let preview = document
        .value_at(pointer)
        .map(single_line)
        .unwrap_or_default();
    let value = gtk::Label::builder()
        .label(&preview)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(pango::EllipsizeMode::End)
        .tooltip_text(&preview)
        .selectable(true)
        .build();
    crate::syntax_label::apply(&value, &preview, 0, preview.len());
    row.append(&key);
    row.append(&value);
    row.upcast()
}

fn revealer_toggle(revealer: &gtk::Revealer) {
    revealer.set_reveal_child(!revealer.reveals_child());
}

fn key_label(label: &str) -> gtk::Label {
    let widget = gtk::Label::builder()
        .label(format!("<b>{}</b>", glib::markup_escape_text(label)))
        .use_markup(true)
        .xalign(0.0)
        .width_chars(6)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    crate::syntax_label::apply_key(&widget);
    widget
}

fn member_pointers(document: &Arc<Document>, pointer: &str) -> Vec<(String, String)> {
    match document.value_at(pointer) {
        Some(Value::Object(members)) => members
            .keys()
            .map(|key| {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                (format!("{pointer}/{escaped}"), key.clone())
            })
            .collect(),
        Some(Value::Array(items)) => (0..items.len())
            .map(|index| (format!("{pointer}/{index}"), format!("[{index}]")))
            .collect(),
        _ => Vec::new(),
    }
}

fn label_for(pointer: &str) -> String {
    match pointer.rsplit_once('/') {
        None => "$".into(),
        Some((_, token)) if token.chars().all(|ch| ch.is_ascii_digit()) => format!("[{token}]"),
        Some((_, token)) => token.replace("~1", "/").replace("~0", "~"),
    }
}

fn single_line(value: &Value) -> String {
    match value {
        Value::Object(members) => format!("{{}} · {} members", members.len()),
        Value::Array(items) => format!("[] · {} items", items.len()),
        other => other.to_string(),
    }
}
