//! Apply the same GtkSourceView JSON token colors to native GTK labels.
use gtk::{glib, pango};
use sourceview5::prelude::*;

pub fn apply_key(label: &gtk::Label) {
    apply(label, "{\"key\":null}", 2, usize::MAX);
}

pub fn apply(label: &gtk::Label, json: &str, start: usize, end: usize) {
    let view = crate::preview::create();
    let Some(buffer) = view.buffer().downcast::<sourceview5::Buffer>().ok() else {
        return;
    };
    buffer.set_text(json);
    update(label, &buffer, start, end);
    buffer.connect_style_scheme_notify(glib::clone!(
        #[weak]
        label,
        move |buffer| {
            update(&label, buffer, start, end);
        }
    ));
    // Retain the buffer for the label's lifetime, without a reference cycle.
    label.connect_destroy(move |_| {
        let _ = &buffer;
    });
}

fn update(label: &gtk::Label, buffer: &sourceview5::Buffer, start: usize, end: usize) {
    buffer.ensure_highlight(&buffer.start_iter(), &buffer.end_iter());
    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    let attributes = pango::AttrList::new();
    for (offset, (byte, ch)) in text.char_indices().enumerate() {
        if byte < start || byte >= end {
            continue;
        }
        let Some(color) = buffer
            .iter_at_offset(offset as i32)
            .tags()
            .into_iter()
            .filter(|tag| tag.is_foreground_set())
            .filter_map(|tag| tag.foreground_rgba())
            .next()
        else {
            continue;
        };
        let channel = |value: f32| (value * 65535.0).round() as u16;
        let mut attribute = pango::AttrColor::new_foreground(
            channel(color.red()),
            channel(color.green()),
            channel(color.blue()),
        );
        attribute.set_start_index((byte - start) as u32);
        attribute.set_end_index(if end == usize::MAX {
            u32::MAX
        } else {
            (byte - start + ch.len_utf8()) as u32
        });
        attributes.insert(attribute);
        if end == usize::MAX {
            break;
        }
    }
    label.set_attributes(Some(&attributes));
}
