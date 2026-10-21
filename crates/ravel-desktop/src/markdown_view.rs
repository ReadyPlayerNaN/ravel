//! Native read-only Markdown presentation. No HTML engine or URL activation.

use gtk::{pango, prelude::*};
use libadwaita as adw;
use ravel_core::markdown::{Markdown, Style};

pub fn create() -> gtk::TextView {
    let view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(32)
        .right_margin(32)
        .top_margin(24)
        .bottom_margin(24)
        .pixels_above_lines(2)
        .pixels_below_lines(2)
        .build();
    view.set_widget_name("markdown-formatted");
    view
}

pub fn set_content(view: &gtk::TextView, document: &Markdown) {
    // Replacing the buffer also releases all prior document-specific tags.
    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(document.text());
    install_tags(&buffer);
    for span in document.spans() {
        // The core's rendering budget keeps these Unicode offsets below i32::MAX.
        let start = buffer.iter_at_offset(span.range.start as i32);
        let end = buffer.iter_at_offset(span.range.end as i32);
        if span.style == Style::Table {
            let text = buffer.text(&start, &end, true);
            let tag = gtk::TextTag::builder()
                .family("monospace")
                .wrap_mode(gtk::WrapMode::None)
                .tabs(&table_tabs(view, &text))
                .build();
            buffer.tag_table().add(&tag);
            buffer.apply_tag(&tag, &start, &end);
        } else {
            buffer.apply_tag_by_name(&tag_name(span.style), &start, &end);
        }
    }
    buffer.set_modified(false);
    view.set_buffer(Some(&buffer));
}

fn tag_name(style: Style) -> String {
    match style {
        Style::Heading(level) => format!("heading-{level}"),
        Style::ListItem(depth) => format!("list-{depth}"),
        Style::Strong => "strong".into(),
        Style::Emphasis => "emphasis".into(),
        Style::Strikethrough => "strike".into(),
        Style::Code => "code".into(),
        Style::CodeBlock => "code-block".into(),
        Style::Quote => "quote".into(),
        Style::Link => "link".into(),
        Style::Table => "table".into(),
    }
}

fn install_tags(buffer: &gtk::TextBuffer) {
    let table = buffer.tag_table();
    table.add(
        &gtk::TextTag::builder()
            .name("quote")
            .style(pango::Style::Italic)
            .left_margin(56)
            .build(),
    );
    // Deeper lists have higher tag priority than their enclosing items.
    for depth in 1..=128 {
        table.add(
            &gtk::TextTag::builder()
                .name(format!("list-{depth}"))
                .left_margin(32 + depth * 24)
                .indent(-20)
                .build(),
        );
    }
    for (level, scale) in [2.0, 1.7, 1.4, 1.2, 1.1, 1.0].into_iter().enumerate() {
        table.add(
            &gtk::TextTag::builder()
                .name(format!("heading-{}", level + 1))
                .weight(700)
                .scale(scale)
                .build(),
        );
    }
    table.add(&gtk::TextTag::builder().name("strong").weight(700).build());
    table.add(
        &gtk::TextTag::builder()
            .name("emphasis")
            .style(pango::Style::Italic)
            .build(),
    );
    table.add(
        &gtk::TextTag::builder()
            .name("strike")
            .strikethrough(true)
            .build(),
    );
    table.add(
        &gtk::TextTag::builder()
            .name("link")
            .underline(pango::Underline::Single)
            .build(),
    );
    for (name, block) in [("code", false), ("code-block", true)] {
        let tag = gtk::TextTag::builder()
            .name(name)
            .family("monospace")
            .style(pango::Style::Normal)
            .build();
        if block {
            tag.set_wrap_mode(gtk::WrapMode::None);
        }
        // Native foreground colors and a subtle adaptive background work in both
        // themes. GObject bindings are weak and disappear with the buffer's tags.
        adw::StyleManager::default()
            .bind_property(
                "dark",
                &tag,
                if block {
                    "paragraph-background-rgba"
                } else {
                    "background-rgba"
                },
            )
            .transform_to(|_, dark: bool| {
                Some(if dark {
                    gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 0.08)
                } else {
                    gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 0.06)
                })
            })
            .sync_create()
            .build();
        table.add(&tag);
    }
}

fn table_tabs(view: &gtk::TextView, text: &str) -> pango::TabArray {
    let layout = view.create_pango_layout(None);
    let mut font = view.pango_context().font_description().unwrap_or_default();
    font.set_family("monospace");
    layout.set_font_description(Some(&font));
    let mut widths: Vec<i32> = Vec::new();
    for line in text.lines() {
        for (column, cell) in line.split('\t').enumerate() {
            layout.set_text(cell);
            let width = layout.pixel_size().0;
            if let Some(maximum) = widths.get_mut(column) {
                *maximum = (*maximum).max(width);
            } else {
                widths.push(width);
            }
        }
    }
    let mut tabs = pango::TabArray::new(widths.len() as i32, true);
    let mut position: i32 = 0;
    for (column, width) in widths.into_iter().enumerate() {
        position = position.saturating_add(width).saturating_add(32);
        tabs.set_tab(column as i32, pango::TabAlign::Left, position);
    }
    tabs
}
