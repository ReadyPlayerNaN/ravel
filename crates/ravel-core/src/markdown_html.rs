//! Generated markup only: source HTML and every attribute value are escaped.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;

use linkify::{LinkFinder, LinkKind};
use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd, TextMergeStream,
};

use super::{MAX_EVENTS, MAX_NESTING, MAX_RENDERED_BYTES, MarkdownError};

type Result<T> = std::result::Result<T, MarkdownError>;

pub(super) fn render(source: &str) -> Result<(String, Vec<String>)> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
        | Options::ENABLE_HEADING_ATTRIBUTES;
    let mut events = Vec::new();
    let mut depth = 0;
    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            _ => {}
        }
        if events.len() >= MAX_EVENTS || depth > MAX_NESTING {
            return Err(MarkdownError::ComplexityLimit);
        }
        events.push(event);
    }
    // Entities and escaped punctuation can split a URL across adjacent text
    // events. Merge only after enforcing the original parser-event budget.
    let events: Vec<_> = TextMergeStream::new(events.into_iter()).collect();
    let mut renderer = Renderer::new(&events);
    renderer.events(&events)?;
    renderer.footnotes(&events)?;
    let mut result = String::new();
    if !renderer.headings.is_empty() {
        result.push_str("<nav aria-label=\"Table of contents\"><details class=\"contents\"><summary>Contents</summary><ol>");
        for (level, id, title) in &renderer.headings {
            result.push_str(&format!(
                "<li class=\"toc-level-{level}\"><a href=\"#{}\">{}</a></li>",
                escape(id),
                escape(title)
            ));
        }
        result.push_str("</ol></details></nav>");
    }
    result.push_str("<article>");
    result.push_str(&renderer.output);
    result.push_str("</article>");
    check_size(&result)?;
    Ok((result, renderer.media_sources.into_iter().collect()))
}

fn check_size(text: &str) -> Result<()> {
    if text.len() > MAX_RENDERED_BYTES {
        Err(MarkdownError::ComplexityLimit)
    } else {
        Ok(())
    }
}

fn escape(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            '\0' => output.push('\u{fffd}'),
            _ => output.push(character),
        }
    }
    output
}

fn allowed_destination(destination: &str, media: bool) -> bool {
    if destination.trim() != destination || destination.chars().any(char::is_control) {
        return false;
    }
    let Some((scheme, _)) = destination.split_once(':') else {
        return !media || !destination.is_empty();
    };
    if !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return true; // Relative paths may contain a colon after their first segment.
    }
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" | "file" => true,
        "mailto" => !media,
        "data" if media => [
            "data:image/png;",
            "data:image/jpeg;",
            "data:image/gif;",
            "data:image/webp;",
            "data:image/avif;",
        ]
        .iter()
        .any(|prefix| destination.to_ascii_lowercase().starts_with(prefix)),
        _ => false,
    }
}

struct Note {
    number: usize,
    references: usize,
    seen: usize,
    definition: Option<Range<usize>>,
}

#[derive(Default)]
struct Renderer {
    output: String,
    media_sources: BTreeSet<String>,
    headings: Vec<(u8, String, String)>,
    ids: HashSet<String>,
    slug_suffixes: HashMap<String, usize>,
    notes: HashMap<String, Note>,
    note_order: Vec<String>,
    links: Vec<bool>,
    link_finder: LinkFinder,
    in_code_block: bool,
    table_alignments: Vec<Alignment>,
    table_head: bool,
    table_column: usize,
}

impl Renderer {
    fn new(events: &[Event<'_>]) -> Self {
        let mut renderer = Self::default();
        renderer
            .link_finder
            .kinds(&[LinkKind::Url])
            .url_must_have_scheme(false);
        for event in events {
            if let Event::FootnoteReference(label) = event {
                let number = renderer.notes.len() + 1;
                let note = renderer.notes.entry(label.to_string()).or_insert_with(|| {
                    renderer.note_order.push(label.to_string());
                    Note {
                        number,
                        references: 0,
                        seen: 0,
                        definition: None,
                    }
                });
                note.references += 1;
                renderer.ids.insert(format!("ravel-fn-{}", note.number));
                renderer
                    .ids
                    .insert(format!("ravel-ref-{}-{}", note.number, note.references));
            }
        }
        for (index, event) in events.iter().enumerate() {
            if let Event::Start(Tag::FootnoteDefinition(label)) = event
                && let Some(note) = renderer.notes.get_mut(label.as_ref())
            {
                let end = events[index + 1..]
                    .iter()
                    .position(|event| matches!(event, Event::End(TagEnd::FootnoteDefinition)))
                    .map_or(events.len(), |offset| index + 1 + offset);
                note.definition = Some(index + 1..end);
            }
        }
        renderer
    }

    fn heading_id(&mut self, title: &str, explicit: Option<&str>) -> String {
        let base = explicit.map(str::to_owned).unwrap_or_else(|| {
            title
                .to_lowercase()
                .chars()
                .filter_map(|c| {
                    if c.is_alphanumeric() || c == '_' || c == '-' {
                        Some(c)
                    } else if c.is_whitespace() {
                        Some('-')
                    } else {
                        None
                    }
                })
                .collect()
        });
        let base = if base.is_empty() {
            "section".to_owned()
        } else {
            base
        };
        let mut id = base.clone();
        let suffix = self.slug_suffixes.entry(base.clone()).or_insert(0);
        while !self.ids.insert(id.clone()) {
            *suffix += 1;
            id = format!("{base}-{suffix}");
        }
        id
    }

    fn events(&mut self, events: &[Event<'_>]) -> Result<()> {
        let mut index = 0;
        while let Some(event) = events.get(index) {
            match event {
                Event::Start(Tag::FootnoteDefinition(_)) => {
                    while index < events.len() && !matches!(events[index], Event::End(TagEnd::FootnoteDefinition)) {
                        index += 1;
                    }
                }
                Event::Start(Tag::Image { dest_url, title, .. }) => {
                    let start = index + 1;
                    let mut depth = 1;
                    index += 1;
                    while index < events.len() {
                        match events[index] {
                            Event::Start(Tag::Image { .. }) => depth += 1,
                            Event::End(TagEnd::Image) => depth -= 1,
                            _ => {}
                        }
                        if depth == 0 { break; }
                        index += 1;
                    }
                    self.media(dest_url, title, &plain_text(&events[start..index]));
                }
                Event::Start(Tag::Heading { level, id, .. }) => {
                    let end = events[index + 1..].iter()
                        .position(|event| matches!(event, Event::End(TagEnd::Heading(_))))
                        .map_or(events.len(), |offset| index + 1 + offset);
                    let title = plain_text(&events[index + 1..end]);
                    let id = self.heading_id(&title, id.as_deref());
                    self.output.push_str(&format!("<h{} id=\"{}\" tabindex=\"-1\">", *level as u8, escape(&id)));
                    self.headings.push((*level as u8, id, title));
                }
                Event::Start(tag) => self.start(tag),
                Event::End(tag) => self.end(*tag),
                Event::Text(text) => self.text(text)?,
                Event::Html(text) => self.output.push_str(&escape(text)),
                Event::Code(text) | Event::InlineHtml(text) => {
                    self.output.push_str(&format!("<code>{}</code>", escape(text)));
                }
                Event::InlineMath(tex) | Event::DisplayMath(tex) => {
                    let display = matches!(event, Event::DisplayMath(_));
                    self.output.push_str(&format!(
                        "<span class=\"math {}\" data-tex=\"{}\">{}</span>",
                        if display { "math-display" } else { "math-inline" }, escape(tex), escape(tex)
                    ));
                }
                Event::FootnoteReference(label) => {
                    if let Some(note) = self.notes.get_mut(label.as_ref()) {
                        note.seen += 1;
                        self.output.push_str(&format!(
                            "<sup id=\"ravel-ref-{0}-{1}\"><a href=\"#ravel-fn-{0}\" role=\"doc-noteref\" aria-label=\"Footnote {0}\">{0}</a></sup>",
                            note.number, note.seen
                        ));
                    }
                }
                Event::SoftBreak => self.output.push('\n'),
                Event::HardBreak => self.output.push_str("<br>\n"),
                Event::Rule => self.output.push_str("<hr>"),
                Event::TaskListMarker(checked) => self.output.push_str(if *checked {
                    "<input type=\"checkbox\" class=\"task\" aria-label=\"Task status (changes are not saved)\" checked> "
                } else {
                    "<input type=\"checkbox\" class=\"task\" aria-label=\"Task status (changes are not saved)\"> "
                }),
            }
            check_size(&self.output)?;
            index += 1;
        }
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<()> {
        if self.in_code_block || !self.links.is_empty() {
            self.output.push_str(&escape(text));
            return check_size(&self.output);
        }
        let mut end = 0;
        for link in self.link_finder.links(text) {
            let url = link.as_str();
            let prefix = if url.get(..4).is_some_and(|s| s.eq_ignore_ascii_case("www.")) {
                "https://"
            } else if url.split_once("://").is_some_and(|(scheme, _)| {
                scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
            }) {
                ""
            } else {
                continue;
            };
            let destination = format!("{prefix}{url}");
            if !allowed_destination(&destination, false) {
                continue;
            }
            self.output.push_str(&escape(&text[end..link.start()]));
            self.output.push_str(&format!(
                "<a href=\"{}\" rel=\"noreferrer noopener\">{}</a>",
                escape(&destination),
                escape(url)
            ));
            check_size(&self.output)?;
            end = link.end();
        }
        self.output.push_str(&escape(&text[end..]));
        check_size(&self.output)
    }

    fn start(&mut self, tag: &Tag<'_>) {
        match tag {
            Tag::Paragraph => self.output.push_str("<p>"),
            Tag::BlockQuote(_) => self.output.push_str("<blockquote>"),
            Tag::CodeBlock(kind) => {
                self.in_code_block = true;
                self.output.push_str("<pre><code");
                if let CodeBlockKind::Fenced(language) = kind {
                    self.output
                        .push_str(&format!(" title=\"{}\"", escape(language)));
                }
                self.output.push('>');
            }
            Tag::HtmlBlock => self.output.push_str("<pre class=\"literal-html\">"),
            Tag::List(Some(start)) => self.output.push_str(&format!("<ol start=\"{start}\">")),
            Tag::List(None) => self.output.push_str("<ul>"),
            Tag::Item => self.output.push_str("<li>"),
            Tag::Emphasis => self.output.push_str("<em>"),
            Tag::Strong => self.output.push_str("<strong>"),
            Tag::Strikethrough => self.output.push_str("<del>"),
            Tag::Link {
                dest_url, title, ..
            } => {
                let allowed = allowed_destination(dest_url, false);
                self.links.push(allowed);
                if allowed {
                    self.output.push_str(&format!(
                        "<a href=\"{}\" title=\"{}\" rel=\"noreferrer noopener\">",
                        escape(dest_url),
                        escape(title)
                    ));
                } else {
                    self.output.push_str(
                        "<span class=\"blocked-link\" title=\"Unsupported link scheme\">",
                    );
                }
            }
            Tag::Table(alignments) => {
                self.table_alignments.clone_from(alignments);
                self.output.push_str("<div class=\"table-scroll\"><table>");
            }
            Tag::TableHead => {
                self.table_head = true;
                self.table_column = 0;
                self.output.push_str("<thead><tr>");
            }
            Tag::TableRow => {
                self.table_column = 0;
                self.output.push_str("<tr>");
            }
            Tag::TableCell => {
                let cell = if self.table_head { "th" } else { "td" };
                let alignment = match self.table_alignments.get(self.table_column) {
                    Some(Alignment::Center) => "center",
                    Some(Alignment::Right) => "right",
                    _ => "left",
                };
                self.output
                    .push_str(&format!("<{cell} style=\"text-align:{alignment}\">"));
                self.table_column += 1;
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        let text = match tag {
            TagEnd::Paragraph => "</p>\n",
            TagEnd::Heading(level) => {
                self.output.push_str(&format!("</h{}>\n", level as u8));
                return;
            }
            TagEnd::BlockQuote(_) => "</blockquote>\n",
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                "</code></pre>\n"
            }
            TagEnd::HtmlBlock => "</pre>\n",
            TagEnd::List(true) => "</ol>\n",
            TagEnd::List(false) => "</ul>\n",
            TagEnd::Item => "</li>\n",
            TagEnd::Emphasis => "</em>",
            TagEnd::Strong => "</strong>",
            TagEnd::Strikethrough => "</del>",
            TagEnd::Link => {
                if self.links.pop().unwrap_or(false) {
                    "</a>"
                } else {
                    "</span>"
                }
            }
            TagEnd::Table => "</tbody></table></div>\n",
            TagEnd::TableHead => {
                self.table_head = false;
                "</tr></thead><tbody>"
            }
            TagEnd::TableRow => "</tr>\n",
            TagEnd::TableCell => {
                if self.table_head {
                    "</th>"
                } else {
                    "</td>"
                }
            }
            _ => "",
        };
        self.output.push_str(text);
    }

    fn media(&mut self, destination: &str, title: &str, alt: &str) {
        if !allowed_destination(destination, true) {
            self.output.push_str(&format!(
                "<span class=\"media-error\">[Media unavailable: {}]</span>",
                escape(alt)
            ));
            return;
        }
        self.media_sources.insert(destination.to_owned());
        let path = destination.split(['?', '#']).next().unwrap_or(destination);
        let extension = path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase());
        let kind = match (title, extension.as_deref()) {
            ("video", _) | (_, Some("mp4" | "m4v" | "webm" | "ogv" | "mov" | "mkv")) => "video",
            ("audio", _)
            | (_, Some("mp3" | "m4a" | "ogg" | "oga" | "opus" | "flac" | "wav" | "aac")) => "audio",
            _ => "img",
        };
        let url = escape(destination);
        let alt = escape(alt);
        self.output.push_str("<span class=\"media\">");
        if kind == "img" {
            self.output.push_str(&format!("<img src=\"{url}\" alt=\"{alt}\" title=\"{}\" loading=\"lazy\" referrerpolicy=\"no-referrer\">", escape(title)));
        } else {
            self.output.push_str(&format!("<{kind} src=\"{url}\" controls preload=\"metadata\" aria-label=\"{alt}\" title=\"{alt}\">{alt}</{kind}>"));
        }
        self.output.push_str("<span class=\"media-error\" hidden>Media could not be loaded or its format is unsupported.</span></span>");
    }

    fn footnotes(&mut self, events: &[Event<'_>]) -> Result<()> {
        if self.note_order.is_empty() {
            return Ok(());
        }
        self.output.push_str(
            "<section class=\"footnotes\" role=\"doc-endnotes\" aria-label=\"Footnotes\"><hr><ol>",
        );
        for label in self.note_order.clone() {
            let note = &self.notes[&label];
            let (number, references, definition) =
                (note.number, note.references, note.definition.clone());
            self.output
                .push_str(&format!("<li id=\"ravel-fn-{number}\" tabindex=\"-1\">"));
            if let Some(range) = definition {
                self.events(&events[range])?;
            }
            for reference in 1..=references {
                self.output.push_str(&format!(" <a href=\"#ravel-ref-{number}-{reference}\" role=\"doc-backlink\" aria-label=\"Back to reference {reference}\">↩{reference}</a>"));
            }
            self.output.push_str("</li>");
            check_size(&self.output)?;
        }
        self.output.push_str("</ol></section>");
        Ok(())
    }
}

fn plain_text(events: &[Event<'_>]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            Event::Text(value)
            | Event::Code(value)
            | Event::InlineMath(value)
            | Event::DisplayMath(value)
            | Event::InlineHtml(value) => text.push_str(value),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            _ => {}
        }
    }
    text
}
