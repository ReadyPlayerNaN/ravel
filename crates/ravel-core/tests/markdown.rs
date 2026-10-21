use ravel_core::markdown::{Markdown, MarkdownError, Style};

fn styled_text(document: &Markdown, style: Style) -> Vec<String> {
    document
        .spans()
        .iter()
        .filter(|span| span.style == style)
        .map(|span| {
            document
                .text()
                .chars()
                .skip(span.range.start)
                .take(span.range.len())
                .collect()
        })
        .collect()
}

#[test]
fn recognizes_markdown_extensions_only() {
    for name in ["README.md", "notes.MD", "doc.MarkDown"] {
        assert!(Markdown::matches_filename(name));
    }
    for name in ["md", "README", "data.json", "doc.md.json"] {
        assert!(!Markdown::matches_filename(name));
    }
}

#[test]
fn formats_unicode_and_nested_inline_styles() {
    let document =
        Markdown::parse("# Héllo 🌍\n\n**bold _é_** and ~~old~~ `x < y`".as_bytes()).unwrap();
    assert_eq!(styled_text(&document, Style::Heading(1)), ["Héllo 🌍\n\n"]);
    assert_eq!(styled_text(&document, Style::Strong), ["bold é"]);
    assert_eq!(styled_text(&document, Style::Emphasis), ["é"]);
    assert_eq!(styled_text(&document, Style::Strikethrough), ["old"]);
    assert_eq!(styled_text(&document, Style::Code), ["x < y"]);
}

#[test]
fn formats_lists_quotes_code_and_breaks() {
    let source = "3. first\n4. second\n   - nested\n\n- [x] done\n- [ ] pending\n\n> quote\n\n```rust\nlet x = 1;\n  x\n```\n\nsoft\nline  \nhard\n\n---\n";
    let document = Markdown::parse(source.as_bytes()).unwrap();
    assert!(document.text().contains("3. first\n4. second\n• nested"));
    assert!(document.text().contains("• ☑ done\n• ☐ pending"));
    assert_eq!(styled_text(&document, Style::Quote), ["quote\n\n"]);
    assert_eq!(
        styled_text(&document, Style::CodeBlock),
        ["let x = 1;\n  x\n\n"]
    );
    assert!(document.text().contains("soft line\nhard"));
    assert!(document.text().contains("────────────────────"));
    assert!(!styled_text(&document, Style::ListItem(2)).is_empty());
}

#[test]
fn loose_list_keeps_marker_beside_paragraph() {
    let document = Markdown::parse(b"- first\n\n- second\n\n  continuation").unwrap();
    assert!(
        document
            .text()
            .contains("• first\n\n• second\n\ncontinuation")
    );
}

#[test]
fn renders_tables_as_separate_rows_and_cells() {
    let document =
        Markdown::parse(b"| Name | Value |\n| --- | ---: |\n| Alice | **42** |\n").unwrap();
    assert_eq!(document.text(), "Name\tValue\nAlice\t42\n\n");
    assert_eq!(
        styled_text(&document, Style::Strong),
        ["Name\tValue\n", "42"]
    );
    assert_eq!(styled_text(&document, Style::Table), [document.text()]);
}

#[test]
fn keeps_html_images_and_link_destinations_inert_and_visible() {
    let source = "[site][ref] ![alt *text*](file:///private.png)\n\n[ref]: javascript:alert(1)\n\n<script>do_bad_things()</script>\n\ninline <b>HTML</b> &amp; text";
    let document = Markdown::parse(source.as_bytes()).unwrap();
    assert!(document.text().contains("site (javascript:alert(1))"));
    assert!(
        document
            .text()
            .contains("[Image: alt text] (file:///private.png)")
    );
    assert!(document.text().contains("<script>do_bad_things()</script>"));
    assert!(document.text().contains("inline <b>HTML</b> & text"));
    assert_eq!(document.source(), source);
}

#[test]
fn accepts_empty_incomplete_and_bom_prefixed_documents() {
    assert!(Markdown::parse(b"").unwrap().text().is_empty());
    let document = Markdown::parse(b"\xef\xbb\xbf# title\r\n\r\n**unfinished").unwrap();
    assert_eq!(document.source(), "# title\r\n\r\n**unfinished");
    assert!(document.text().contains("**unfinished"));
}

#[test]
fn rejects_invalid_utf8() {
    assert!(matches!(
        Markdown::parse(&[0xff]),
        Err(MarkdownError::Encoding(_))
    ));
}

#[test]
fn bounds_source_size_and_nesting() {
    assert!(matches!(
        Markdown::parse(&vec![b'a'; 16 * 1024 * 1024 + 1]),
        Err(MarkdownError::SourceLimit)
    ));
    let source = format!("{}text", "> ".repeat(150));
    assert!(matches!(
        Markdown::parse(source.as_bytes()),
        Err(MarkdownError::ComplexityLimit)
    ));
}

#[test]
fn all_ranges_are_valid_unicode_scalar_offsets() {
    let document =
        Markdown::parse("# é\n\n> **🦀** and [λ](https://example.org)\n\n- _一_\n".as_bytes())
            .unwrap();
    let length = document.text().chars().count();
    assert!(
        document
            .spans()
            .iter()
            .all(|span| span.range.start <= span.range.end && span.range.end <= length)
    );
}
