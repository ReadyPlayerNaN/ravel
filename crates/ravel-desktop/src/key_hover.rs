use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

use gtk::{gdk, glib};
use libadwaita::{self as adw, prelude::*};
use serde_json::Value;

use crate::key_index::{Key, KeyIndex};

pub struct KeyHover {
    view: glib::WeakRef<sourceview5::View>,
    buffer: gtk::TextBuffer,
    tag: gtk::TextTag,
    index: RefCell<KeyIndex>,
    position: Cell<Option<(f64, f64)>>,
    shift: Cell<bool>,
    pressed_shifts: Cell<u8>,
    ticking: Cell<bool>,
    applied: RefCell<Vec<Range<i32>>>,
}

impl KeyHover {
    pub fn install(view: &sourceview5::View, window: &adw::ApplicationWindow) -> Rc<Self> {
        let buffer = view.buffer();
        let tag = gtk::TextTag::builder().name("ravel-key-hover").build();
        buffer.tag_table().add(&tag);
        adw::StyleManager::default()
            .bind_property("dark", &tag, "background-rgba")
            .transform_to(|_, dark: bool| {
                Some(if dark {
                    gdk::RGBA::new(0.35, 0.55, 0.95, 0.35)
                } else {
                    gdk::RGBA::new(0.1, 0.4, 0.9, 0.2)
                })
            })
            .sync_create()
            .build();

        let hover = Rc::new(Self {
            view: view.downgrade(),
            buffer,
            tag,
            index: RefCell::new(KeyIndex::default()),
            position: Cell::new(None),
            shift: Cell::new(false),
            pressed_shifts: Cell::new(0),
            ticking: Cell::new(false),
            applied: RefCell::new(Vec::new()),
        });
        let motion = gtk::EventControllerMotion::new();
        motion.set_name(Some("key-hover-motion"));
        motion.connect_enter(glib::clone!(
            #[weak]
            hover,
            move |controller, x, y| {
                hover.motion(x, y, controller.current_event_state());
            }
        ));
        motion.connect_motion(glib::clone!(
            #[weak]
            hover,
            move |controller, x, y| {
                hover.motion(x, y, controller.current_event_state());
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak]
            hover,
            move |_| {
                hover.position.set(None);
                hover.apply(Vec::new());
            }
        ));
        view.add_controller(motion);

        // Capture at the window so Shift works even when search/tree has focus.
        let keys = gtk::EventControllerKey::new();
        keys.set_name(Some("key-hover-modifiers"));
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_modifiers(glib::clone!(
            #[weak]
            hover,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, modifiers| {
                hover
                    .shift
                    .set(modifiers.contains(gdk::ModifierType::SHIFT_MASK));
                hover.refresh();
                glib::Propagation::Proceed
            }
        ));
        // Modifier notifications use the pre-event mask on Shift press/release.
        // Handle those transitions explicitly, including both Shift keys together.
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            hover,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                let bit = shift_bit(key);
                if bit != 0 {
                    hover.pressed_shifts.set(hover.pressed_shifts.get() | bit);
                    hover.shift.set(true);
                    hover.refresh();
                }
                glib::Propagation::Proceed
            }
        ));
        keys.connect_key_released(glib::clone!(
            #[weak]
            hover,
            move |_, key, _, _| {
                let bit = shift_bit(key);
                if bit != 0 {
                    hover.pressed_shifts.set(hover.pressed_shifts.get() & !bit);
                    hover.shift.set(hover.pressed_shifts.get() != 0);
                    hover.refresh();
                }
            }
        ));
        window.add_controller(keys);
        hover.buffer.connect_changed(glib::clone!(
            #[weak]
            hover,
            move |_| hover.reset()
        ));
        hover
    }

    pub fn set_content(&self, document: &Value, pointer: &str, text: &str) {
        self.buffer.set_text(text);
        *self.index.borrow_mut() = KeyIndex::new(document, pointer, text);
    }

    fn reset(&self) {
        self.position.set(None);
        *self.index.borrow_mut() = KeyIndex::default();
        self.applied.borrow_mut().clear();
        self.buffer.remove_tag(
            &self.tag,
            &self.buffer.start_iter(),
            &self.buffer.end_iter(),
        );
    }

    fn motion(self: &Rc<Self>, x: f64, y: f64, modifiers: gdk::ModifierType) {
        self.position.set(Some((x, y)));
        self.shift
            .set(modifiers.contains(gdk::ModifierType::SHIFT_MASK));
        if !self.shift.get() {
            self.pressed_shifts.set(0);
        }
        self.refresh();
        // Track scrolling and relayout beneath a stationary pointer, only while hovered.
        if !self.ticking.replace(true)
            && let Some(view) = self.view.upgrade()
        {
            let weak = Rc::downgrade(self);
            view.add_tick_callback(move |_, _| {
                let Some(hover) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if hover.position.get().is_none() {
                    hover.ticking.set(false);
                    return glib::ControlFlow::Break;
                }
                hover.refresh();
                glib::ControlFlow::Continue
            });
        }
    }

    fn refresh(&self) {
        let Some(view) = self.view.upgrade() else {
            return;
        };
        let Some((x, y)) = self.position.get() else {
            return;
        };
        let (x, y) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let visible = view.visible_rect();
        if !contains(&visible, x, y) {
            self.apply(Vec::new());
            return;
        }
        let index = self.index.borrow();
        let hovered = view.iter_at_position(x, y).and_then(|(iter, _)| {
            let rect = view.iter_location(&iter);
            // GTK returns the nearest glyph; blank space is not a key hit.
            if contains(&rect, x, y) {
                index.at(iter.offset())
            } else {
                None
            }
        });
        let spans = hovered
            .map(|hovered| {
                let mut start = view
                    .iter_at_location(0, visible.y())
                    .unwrap_or_else(|| self.buffer.start_iter());
                start.set_line_offset(0);
                let mut end = view
                    .iter_at_location(0, visible.y() + visible.height())
                    .unwrap_or_else(|| self.buffer.end_iter());
                end.forward_to_line_end();
                index
                    .in_range(start.offset()..end.offset())
                    .iter()
                    .filter(|key| key.matches(hovered, self.shift.get()))
                    .filter(|key| intersects(&key_rect(&view, &self.buffer, key), &visible))
                    .map(|key| key.span.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.apply(spans);
    }

    fn apply(&self, spans: Vec<Range<i32>>) {
        let mut applied = self.applied.borrow_mut();
        if *applied == spans {
            return;
        }
        for span in applied.iter() {
            self.buffer.remove_tag(
                &self.tag,
                &self.buffer.iter_at_offset(span.start),
                &self.buffer.iter_at_offset(span.end),
            );
        }
        for span in &spans {
            self.buffer.apply_tag(
                &self.tag,
                &self.buffer.iter_at_offset(span.start),
                &self.buffer.iter_at_offset(span.end),
            );
        }
        *applied = spans;
    }
}

fn shift_bit(key: gdk::Key) -> u8 {
    match key {
        gdk::Key::Shift_L => 1,
        gdk::Key::Shift_R => 2,
        _ => 0,
    }
}

fn key_rect(view: &sourceview5::View, buffer: &gtk::TextBuffer, key: &Key) -> gdk::Rectangle {
    let start = view.iter_location(&buffer.iter_at_offset(key.span.start));
    let end = view.iter_location(&buffer.iter_at_offset(key.span.end - 1));
    gdk::Rectangle::new(
        start.x().min(end.x()),
        start.y(),
        (end.x() + end.width()).max(start.x() + start.width()) - start.x().min(end.x()),
        start.height(),
    )
}

fn contains(rect: &gdk::Rectangle, x: i32, y: i32) -> bool {
    x >= rect.x() && x < rect.x() + rect.width() && y >= rect.y() && y < rect.y() + rect.height()
}

fn intersects(a: &gdk::Rectangle, b: &gdk::Rectangle) -> bool {
    a.x() < b.x() + b.width()
        && a.x() + a.width() > b.x()
        && a.y() < b.y() + b.height()
        && a.y() + a.height() > b.y()
}
