use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gtk::glib;
use sourceview5::{self as sourceview, prelude::*};

use crate::fold_index::{self, Block};

struct Region {
    block: Block,
    tag: Option<gtk::TextTag>,
}

pub struct Folding {
    buffer: gtk::TextBuffer,
    renderer: glib::WeakRef<sourceview::GutterRendererText>,
    regions: RefCell<Vec<Region>>,
    enabled: Cell<bool>,
}

impl Folding {
    pub fn install(view: &sourceview::View) -> Rc<Self> {
        let renderer = sourceview::GutterRendererText::builder()
            .text("▼")
            .xalign(0.5)
            .xpad(4)
            .width_request(24)
            .alignment_mode(sourceview::GutterRendererAlignmentMode::First)
            .tooltip_text("Click an arrow to collapse or expand a JSON object or array")
            .build();
        renderer.set_widget_name("code-folds");
        sourceview::prelude::ViewExt::gutter(view, gtk::TextWindowType::Left)
            .insert(&renderer, -20);
        let folding = Rc::new(Self {
            buffer: view.buffer(),
            renderer: renderer.downgrade(),
            regions: RefCell::new(Vec::new()),
            enabled: Cell::new(true),
        });
        renderer.connect_query_data(glib::clone!(
            #[weak]
            folding,
            move |renderer, _, line| {
                if !folding.line_visible(line) {
                    renderer.set_text("");
                    return;
                }
                let regions = folding.regions.borrow();
                let region = regions
                    .binary_search_by_key(&line, |region| region.block.line)
                    .ok()
                    .map(|index| &regions[index]);
                renderer.set_text(match region {
                    Some(region) if region.tag.is_some() => "▶",
                    Some(_) => "▼",
                    None => "",
                });
            }
        ));
        renderer.connect_query_activatable(glib::clone!(
            #[weak]
            folding,
            #[upgrade_or]
            false,
            move |_, iter, _| {
                folding.line_visible(iter.line() as u32)
                    && folding
                        .regions
                        .borrow()
                        .binary_search_by_key(&(iter.line() as u32), |region| region.block.line)
                        .is_ok()
            }
        ));
        renderer.connect_activate(glib::clone!(
            #[weak]
            folding,
            move |_, iter, _, button, _, presses| {
                if button == 1 && presses == 1 {
                    folding.toggle(iter.line() as u32);
                }
            }
        ));
        folding.buffer.connect_changed(glib::clone!(
            #[weak]
            folding,
            move |_| folding.rebuild()
        ));
        folding.rebuild();
        folding
    }

    /// JSON delimiter folding is not valid for original YAML source.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        if let Some(renderer) = self.renderer.upgrade() {
            renderer.set_visible(enabled);
        }
        self.rebuild();
    }

    fn line_visible(&self, line: u32) -> bool {
        i32::try_from(line)
            .ok()
            .and_then(|line| self.buffer.iter_at_line(line))
            .is_some_and(|iter| {
                !iter
                    .tags()
                    .iter()
                    .any(|tag| tag.is_invisible_set() && tag.is_invisible())
            })
    }

    fn rebuild(&self) {
        let mut regions = self.regions.borrow_mut();
        for region in regions.drain(..) {
            if let Some(tag) = region.tag {
                self.buffer.tag_table().remove(&tag);
            }
        }
        let text = self
            .buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true);
        if self.enabled.get() {
            *regions = fold_index::blocks(&text)
                .into_iter()
                .map(|block| Region { block, tag: None })
                .collect();
        }
        if let Some(renderer) = self.renderer.upgrade() {
            renderer.queue_draw();
        }
    }

    fn toggle(&self, line: u32) {
        let mut regions = self.regions.borrow_mut();
        let Ok(index) = regions.binary_search_by_key(&line, |region| region.block.line) else {
            return;
        };
        let region = &mut regions[index];
        let start = self.buffer.iter_at_offset(region.block.body.start);
        let end = self.buffer.iter_at_offset(region.block.body.end);
        if let Some(tag) = region.tag.take() {
            self.buffer.remove_tag(&tag, &start, &end);
            self.buffer.tag_table().remove(&tag);
        } else {
            // A separate tag per collapsed block preserves overlapping child folds.
            let tag = gtk::TextTag::builder().invisible(true).build();
            self.buffer.tag_table().add(&tag);
            self.buffer.apply_tag(&tag, &start, &end);
            region.tag = Some(tag);
        }
        if let Some(renderer) = self.renderer.upgrade() {
            renderer.queue_draw();
        }
    }
}
