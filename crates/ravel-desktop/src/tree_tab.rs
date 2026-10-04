//! Treat the custom structured tree as one tab stop in the surrounding UI.
use gtk::{gdk, glib};
use libadwaita::prelude::*;

pub fn install(column: &gtk::Box) {
    let Some(root) = column
        .root()
        .and_then(|root| root.dynamic_cast::<gtk::Widget>().ok())
    else {
        return;
    };
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        column,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, modifiers| {
            let backward = modifiers == gdk::ModifierType::SHIFT_MASK;
            if !((key == gdk::Key::Tab && modifiers.is_empty())
                || ((key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab) && backward))
            {
                return glib::Propagation::Proceed;
            }
            let Some(root) = column.root() else {
                return glib::Propagation::Proceed;
            };
            let Some(current) = root.focus() else {
                return glib::Propagation::Proceed;
            };
            if current.ancestor(gtk::Popover::static_type()).is_some() {
                return glib::Propagation::Proceed;
            }
            let Some(widget) = root.dynamic_cast_ref::<gtk::Widget>() else {
                return glib::Propagation::Proceed;
            };
            let mut stops = Vec::new();
            collect(widget, column.upcast_ref(), &mut stops);
            let inside =
                current == *column.upcast_ref::<gtk::Widget>() || current.is_ancestor(&column);
            let anchor = if inside {
                column.upcast_ref::<gtk::Widget>()
            } else {
                &current
            };
            let Some(index) = stops.iter().position(|stop| stop == anchor) else {
                return glib::Propagation::Proceed;
            };
            let next = if backward {
                (index + stops.len() - 1) % stops.len()
            } else {
                (index + 1) % stops.len()
            };
            let target = &stops[next];
            if target == column.upcast_ref::<gtk::Widget>() {
                if let Some(row) = crate::tree_navigation::visible_rows(column.upcast_ref())
                    .into_iter()
                    .find(|row| row.is_focusable())
                {
                    crate::tree_navigation::focus(&row);
                }
            } else if inside {
                target.grab_focus();
            } else {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        }
    ));
    root.add_controller(keys);
}

fn collect(widget: &gtk::Widget, tree: &gtk::Widget, stops: &mut Vec<gtk::Widget>) {
    if !widget.is_mapped() || !widget.is_sensitive() || !widget.can_focus() {
        return;
    }
    if widget == tree {
        stops.push(widget.clone());
        return;
    }
    if widget.is_focusable() {
        stops.push(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        collect(&widget, tree, stops);
        child = widget.next_sibling();
    }
}
