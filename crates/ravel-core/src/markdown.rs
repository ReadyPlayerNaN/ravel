//! Safe, toolkit-independent Markdown presentation, separate from JSON trees.
//!
//! CommonMark has both a plain-text/style-range fallback and a rich HTML
//! presentation with headings, links, media, footnotes, math and task controls.
//! Source HTML always stays literal. No files or network resources are read.

#[path = "markdown_html.rs"]
mod rich;

use std::ops::Range;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};
use thiserror::Error;

const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_RENDERED_BYTES: usize = 32 * 1024 * 1024;
const MAX_EVENTS: usize = 1_000_000;
const MAX_NESTING: usize = 128;

/// Semantic formatting for a range of rendered characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Heading level, from one to six.
    Heading(u8),
    /// Strong emphasis.
    Strong,
    /// Italic emphasis.
    Emphasis,
    /// Deleted text.
    Strikethrough,
    /// Inline code or literal HTML.
    Code,
    /// Preformatted code block.
    CodeBlock,
    /// Quoted paragraphs.
    Quote,
    /// A list item at a one-based nesting depth.
    ListItem(u8),
    /// Inert link label and destination.
    Link,
    /// Tab-separated table rows; alignment hints are not retained.
    Table,
}

/// A style range in Unicode scalar offsets, not UTF-8 byte offsets.
#[derive(Debug)]
pub struct Span {
    /// Half-open range into [`Markdown::text`].
    pub range: Range<usize>,
    /// Formatting to apply, possibly overlapping other spans.
    pub style: Style,
}

/// Invalid encoding or a document exceeding presentation budgets.
#[derive(Debug, Error)]
pub enum MarkdownError {
    /// Markdown source must be UTF-8.
    #[error("The Markdown document is not UTF-8: {0}")]
    Encoding(#[from] std::str::Utf8Error),
    /// Bound source size before parsing.
    #[error("The Markdown document exceeds the 16 MiB source limit")]
    SourceLimit,
    /// Bound nesting, parser events and expanded link destinations.
    #[error("The Markdown document exceeds the rendering complexity limit")]
    ComplexityLimit,
}

/// An immutable Markdown source and its formatted presentation.
#[derive(Debug)]
pub struct Markdown {
    source: String,
    text: String,
    spans: Vec<Span>,
    html: String,
    media_sources: Vec<String>,
}

impl Markdown {
    /// Recognizes `.md` and `.markdown` case-insensitively.
    pub fn matches_filename(name: &str) -> bool {
        name.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
    }

    /// Parses Markdown without executing HTML or resolving external resources.
    /// Empty and incomplete Markdown are valid. A leading UTF-8 BOM is removed.
    ///
    /// # Errors
    /// Returns an encoding error or a budget error for sources above 16 MiB,
    /// over 128 nested elements, over one million events, or output above 32 MiB.
    pub fn parse(bytes: &[u8]) -> Result<Self, MarkdownError> {
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(MarkdownError::SourceLimit);
        }
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        let source = std::str::from_utf8(bytes)?;
        let mut renderer = Renderer::default();
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        for (index, event) in Parser::new_ext(source, options).enumerate() {
            if index >= MAX_EVENTS {
                return Err(MarkdownError::ComplexityLimit);
            }
            renderer.event(event);
            if renderer.text.len() > MAX_RENDERED_BYTES || renderer.frames.len() > MAX_NESTING {
                return Err(MarkdownError::ComplexityLimit);
            }
        }
        let (html, media_sources) = rich::render(source)?;
        Ok(Self {
            source: source.to_owned(),
            text: renderer.text,
            spans: renderer.spans,
            html,
            media_sources,
        })
    }

    /// Original source, except for an optional leading BOM.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Plain-text fallback without Markdown formatting delimiters.
    /// Links and images retain visible destinations; rich extensions are in [`Self::html`].
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Safe HTML presentation with a heading index, links, media, footnotes,
    /// task controls and TeX math placeholders. Source HTML is escaped, never
    /// passed through. The desktop supplies styling and trusted math typesetting.
    /// No resource is resolved or loaded by the core.
    pub fn html(&self) -> &str {
        &self.html
    }

    /// Distinct allowed media destinations, unresolved and without I/O. The
    /// desktop uses these to grant read-only sandbox access to local media only.
    pub fn media_sources(&self) -> &[String] {
        &self.media_sources
    }

    /// Semantic formatting ranges for the rendered text.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }
}

struct Frame<'a> {
    start: usize,
    style: Option<Style>,
    destination: Option<CowStr<'a>>,
}

#[derive(Default)]
struct Renderer<'a> {
    text: String,
    chars: usize,
    spans: Vec<Span>,
    frames: Vec<Frame<'a>>,
    lists: Vec<Option<u64>>,
    table_column: usize,
}

impl<'a> Renderer<'a> {
    fn push(&mut self, text: &str) {
        self.text.push_str(text);
        self.chars += text.chars().count();
    }

    fn newline(&mut self, count: usize) {
        if self.text.is_empty() {
            return;
        }
        let existing = self
            .text
            .bytes()
            .rev()
            .take_while(|&ch| ch == b'\n')
            .count();
        for _ in existing..count {
            self.push("\n");
        }
    }

    fn styled(&mut self, text: &str, style: Style) {
        let start = self.chars;
        self.push(text);
        self.spans.push(Span {
            range: start..self.chars,
            style,
        });
    }

    fn event(&mut self, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.push(&text),
            Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.styled(&text, Style::Code);
            }
            Event::SoftBreak => self.push(" "),
            Event::HardBreak => self.push("\n"),
            Event::Rule => {
                self.newline(2);
                self.push("────────────────────\n\n");
            }
            Event::TaskListMarker(checked) => self.push(if checked { "☑ " } else { "☐ " }),
            // These events require options that this renderer does not enable.
            Event::FootnoteReference(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                self.push(&text)
            }
        }
    }

    fn start(&mut self, tag: Tag<'a>) {
        let end = tag.to_end();
        let mut destination = None;
        let style = match tag {
            Tag::Heading { level, .. } => {
                self.newline(2);
                Some(Style::Heading(level as u8))
            }
            Tag::BlockQuote(_) => {
                self.newline(2);
                Some(Style::Quote)
            }
            Tag::CodeBlock(_) => {
                self.newline(2);
                Some(Style::CodeBlock)
            }
            Tag::HtmlBlock => {
                self.newline(2);
                Some(Style::CodeBlock)
            }
            Tag::List(first) => {
                self.newline(if self.lists.is_empty() { 2 } else { 1 });
                self.lists.push(first);
                None
            }
            Tag::Item => {
                self.newline(1);
                Some(Style::ListItem(self.lists.len().min(128) as u8))
            }
            Tag::Emphasis => Some(Style::Emphasis),
            Tag::Strong | Tag::TableHead => Some(Style::Strong),
            Tag::Strikethrough => Some(Style::Strikethrough),
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                destination = Some(dest_url);
                Some(Style::Link)
            }
            Tag::Table(_) => {
                self.newline(2);
                Some(Style::Table)
            }
            Tag::TableCell => {
                if self.table_column > 0 {
                    self.push("\t");
                }
                self.table_column += 1;
                None
            }
            _ => None,
        };
        self.frames.push(Frame {
            start: self.chars,
            style,
            destination,
        });
        match end {
            TagEnd::Item => {
                let marker = match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}. ");
                        *number = number.saturating_add(1);
                        marker
                    }
                    _ => "• ".to_owned(),
                };
                self.push(&marker);
            }
            TagEnd::Image => self.push("[Image: "),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        if tag == TagEnd::Image {
            self.push("]");
        }
        if let Some(destination) = frame.destination {
            self.push(" (");
            self.push(&destination);
            self.push(")");
        }
        match tag {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::Table => self.newline(2),
            TagEnd::Item => self.newline(1),
            TagEnd::List(_) => {
                self.lists.pop();
                self.newline(if self.lists.is_empty() { 2 } else { 1 });
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                self.newline(1);
                self.table_column = 0;
            }
            _ => {}
        }
        if let Some(style) = frame.style {
            self.spans.push(Span {
                range: frame.start..self.chars,
                style,
            });
        }
    }
}
