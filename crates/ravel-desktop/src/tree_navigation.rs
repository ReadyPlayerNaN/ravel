//! Keyboard focus navigation for the lazy structured widget tree.
use gtk::{gdk, glib};
use libadwaita::prelude::*;

pub fn install(column: &gtk::Box) {
    let installed = std::cell::Cell::new(false);
    column.connect_map(move |column| {
        if !installed.replace(true) {
            crate::tree_tab::install(column);
        }
    });
    // A stable entry target lets GTK enter even when the active row is offscreen.
    column.set_focusable(true);
    column.connect_has_focus_notify(|column| {
        if column.has_focus()
            && let Some(row) = visible_rows(column.upcast_ref())
                .into_iter()
                .find(|row| row.is_focusable())
        {
            focus(&row);
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        column,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, modifiers| {
            if modifiers.intersects(
                gdk::ModifierType::CONTROL_MASK
                    | gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SUPER_MASK
                    | gdk::ModifierType::SHIFT_MASK,
            ) {
                return glib::Propagation::Proceed;
            }
            // Popovers are parented to rows but own their keyboard interaction.
            if column
                .root()
                .and_then(|root| root.focus())
                .is_some_and(|focus| focus.ancestor(gtk::Popover::static_type()).is_some())
            {
                return glib::Propagation::Proceed;
            }
            let rows = visible_rows(column.upcast_ref());
            let Some(index) = rows.iter().position(has_focus) else {
                return glib::Propagation::Proceed;
            };
            let current = &rows[index];
            let target = match key {
                gdk::Key::Down => rows.get(index + 1).cloned(),
                gdk::Key::Up => index
                    .checked_sub(1)
                    .and_then(|index| rows.get(index))
                    .cloned(),
                gdk::Key::Home => rows.first().cloned(),
                gdk::Key::End => rows.last().cloned(),
                gdk::Key::Right => {
                    if let Some(revealer) = revealer(current) {
                        if !revealer.reveals_child() {
                            toggle(current);
                        }
                        revealer
                            .child()
                            .and_then(|children| visible_rows(&children).first().cloned())
                    } else {
                        None
                    }
                }
                gdk::Key::Left => {
                    if revealer(current).is_some_and(|revealer| revealer.reveals_child()) {
                        toggle(current);
                        None
                    } else {
                        parent_row(current)
                    }
                }
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => {
                    toggle(current);
                    None
                }
                _ => return glib::Propagation::Proceed,
            };
            if let Some(target) = target {
                focus(&target);
            }
            glib::Propagation::Stop
        }
    ));
    column.add_controller(keys);
}

pub fn install_row(row: &gtk::Widget, root: bool) {
    row.set_focusable(root);
    row.set_can_focus(root);
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    click.connect_pressed(glib::clone!(
        #[weak]
        row,
        move |_, _, _, _| {
            focus(&row);
        }
    ));
    row.add_controller(click);
}

pub fn focus(row: &gtk::Widget) {
    if let Some(column) = tree_column(row) {
        for other in visible_rows(&column) {
            other.set_focusable(other == *row);
            other.set_can_focus(other == *row);
        }
    }
    row.set_focusable(true);
    row.set_can_focus(true);
    row.grab_focus();
    // Follow layout during reveal animations; stop afterward so manual scrolling stays free.
    let started = std::time::Instant::now();
    row.add_tick_callback(move |row, _| {
        if !has_focus(row) || started.elapsed().as_millis() > 300 {
            return glib::ControlFlow::Break;
        }
        if let Some(column) = tree_column(row)
            && let Some(scroll) = column
                .parent()
                .and_then(|parent| parent.ancestor(gtk::ScrolledWindow::static_type()))
                .and_downcast::<gtk::ScrolledWindow>()
            && let Some(bounds) = row.compute_bounds(&column)
        {
            let adjustment = scroll.vadjustment();
            adjustment.clamp_page(
                f64::from(bounds.y()),
                f64::from(bounds.y() + bounds.height()),
            );
        }
        glib::ControlFlow::Continue
    });
}

pub fn recover_before_collapse(row: &gtk::Widget, revealer: &gtk::Revealer) {
    if revealer.reveals_child() && has_focus(revealer.upcast_ref()) {
        focus(row);
    }
}

fn has_focus(widget: &gtk::Widget) -> bool {
    widget
        .root()
        .and_then(|root| root.focus())
        .is_some_and(|focus| focus == *widget || focus.is_ancestor(widget))
}

fn tree_column(widget: &gtk::Widget) -> Option<gtk::Widget> {
    let mut ancestor = Some(widget.clone());
    while let Some(widget) = ancestor {
        if widget.widget_name() == "structured-view" {
            return Some(widget);
        }
        ancestor = widget.parent();
    }
    None
}

fn toggle(row: &gtk::Widget) {
    if let Some(button) = row.downcast_ref::<gtk::Button>() {
        button.emit_clicked();
    }
}

fn revealer(row: &gtk::Widget) -> Option<gtk::Revealer> {
    if row.widget_name() != "node-toggle" {
        return None;
    }
    row.next_sibling().and_downcast()
}

fn parent_row(row: &gtk::Widget) -> Option<gtk::Widget> {
    let mut parent = row.parent();
    while let Some(widget) = parent {
        if widget.widget_name() == "node-container"
            && let Some(header) = widget.first_child()
            && header != *row
        {
            return Some(header);
        }
        parent = widget.parent();
    }
    None
}

pub(crate) fn visible_rows(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut rows = Vec::new();
    collect(widget, &mut rows);
    rows
}

fn collect(widget: &gtk::Widget, rows: &mut Vec<gtk::Widget>) {
    match widget.widget_name().as_str() {
        "node-row" => {
            rows.push(widget.clone());
            return;
        }
        "node-container" => {
            if let Some(header) = widget.first_child() {
                rows.push(header.clone());
                if let Some(revealer) =
                    revealer(&header).filter(|revealer| revealer.reveals_child())
                    && let Some(children) = revealer.child()
                {
                    collect(&children, rows);
                }
            }
            return;
        }
        _ => {}
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        collect(&widget, rows);
        child = widget.next_sibling();
    }
}
