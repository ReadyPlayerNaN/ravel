use ravel_core::markdown::Markdown;

fn render(source: &str) -> String {
    Markdown::parse(source.as_bytes())
        .unwrap()
        .html()
        .to_owned()
}

#[test]
fn links_are_clickable_but_active_schemes_are_blocked() {
    let html = render(
        "[web](https://example.org/?a=1&b=2) [mail](mailto:a@example.org) [relative](other.md#title) [bad](javascript:alert%281%29) [data](data:text/html,evil)",
    );
    assert!(html.contains("href=\"https://example.org/?a=1&amp;b=2\""));
    assert!(html.contains("href=\"mailto:a@example.org\""));
    assert!(html.contains("href=\"other.md#title\""));
    assert_eq!(html.matches("class=\"blocked-link\"").count(), 2);
    assert!(!html.contains("href=\"javascript:"));
}

#[test]
fn bare_web_urls_are_linked_without_surrounding_punctuation() {
    let html = render(
        "See https://example.org/a_(b). Then (http://example.org/path), www.example.org and HTTPS://example.org/日本語!",
    );
    for url in [
        "https://example.org/a_(b)",
        "http://example.org/path",
        "https://www.example.org",
        "HTTPS://example.org/日本語",
    ] {
        assert!(html.contains(&format!("href=\"{url}\"")), "{html}");
    }
    assert!(html.contains("a_(b)</a>. Then ("), "{html}");
    assert!(html.contains("/path</a>),"), "{html}");
    assert!(html.contains(">www.example.org</a>"), "{html}");
}

#[test]
fn bare_url_entities_are_merged_and_escaped_without_changing_source() {
    let source = "https://example.org/?a=1&amp;b=2 and https://example.org/\\\"quoted";
    let document = Markdown::parse(source.as_bytes()).unwrap();
    assert!(
        document
            .html()
            .contains("href=\"https://example.org/?a=1&amp;b=2\""),
        "{}",
        document.html()
    );
    assert!(
        !document
            .html()
            .contains("href=\"https://example.org/\"quoted")
    );
    assert_eq!(document.source(), source);
}

#[test]
fn bare_urls_leave_code_html_math_images_and_existing_links_alone() {
    let html = render(
        "`https://inline.example.org`\n\n```\nhttps://fenced.example.org\n```\n\n    https://indented.example.org\n\n<div>https://html.example.org</div>\n\n<span title=\"https://attribute.example.org\">literal</span>\n\n$https://math.example.org$\n\n![https://alt.example.org](image.png)\n\n[https://label.example.org](https://destination.example.org)\n\n[https://blocked.example.org](javascript:evil)\n\n<https://autolink.example.org>",
    );
    assert_eq!(html.matches("<a ").count(), 2, "{html}");
    assert!(html.contains("href=\"https://destination.example.org\""));
    assert!(html.contains("href=\"https://autolink.example.org\""));
}

#[test]
fn bare_urls_do_not_enable_unsupported_schemes_or_guess_domains() {
    let html = render(
        "javascript:alert(1) javascript://example.org ftp://example.org data:text/html,evil file:///tmp/file example.org sample.md",
    );
    assert!(!html.contains("<a "), "{html}");
}

#[test]
fn headings_have_unique_unicode_slugs_explicit_ids_and_a_contents_index() {
    let html = render(
        "# Hello, *world*!\n## Hello, world!\n# Hello world-1\n# 日本語\n# Named {#custom .ignored onclick=evil}\n# Explicit again {#custom}\n",
    );
    for id in [
        "hello-world",
        "hello-world-1",
        "hello-world-1-1",
        "日本語",
        "custom",
        "custom-1",
    ] {
        assert!(html.contains(&format!("id=\"{id}\"")), "{html}");
        assert!(html.contains(&format!("href=\"#{id}\"")), "{html}");
    }
    assert!(html.starts_with("<div class=\"reader-layout has-contents\">"));
    assert_eq!(html.matches("<nav ").count(), 1);
    assert!(html.contains("<details class=\"contents\"><summary>Contents</summary>"));
    assert!(!html.contains("onclick="));
    assert!(!html.contains("class=\"ignored\""));
}

#[test]
fn documents_without_headings_have_no_contents_or_sidebar_column() {
    let html = render("A short document without headings.");
    assert!(html.starts_with("<div class=\"reader-layout\"><article>"));
    assert!(!html.contains("<nav "));
}

#[test]
fn html_remains_literal_including_scripts_event_handlers_and_svg() {
    let html = render(
        "<script>alert(1)</script>\n\nInline <img src=x onerror=evil> and <svg onload=evil />\n",
    );
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("&lt;img src=x onerror=evil&gt;"));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<svg"));
}

#[test]
fn media_supports_relative_remote_reference_and_explicit_type_urls() {
    let html = render(
        "![Photo](images/cat.png \"A cat\")\n\n![Movie](movie.WEBM?x=1)\n\n![Song][audio]\n\n[audio]: https://example.org/stream \"audio\"\n\n![Live](https://example.org/stream \"video\")",
    );
    assert!(html.contains("<img src=\"images/cat.png\" alt=\"Photo\" title=\"A cat\""));
    assert!(html.contains("<video src=\"movie.WEBM?x=1\" controls preload=\"metadata\""));
    assert!(html.contains("<audio src=\"https://example.org/stream\" controls"));
    assert!(html.contains("<video src=\"https://example.org/stream\" controls"));
    assert!(!html.contains("autoplay"));
}

#[test]
fn media_attributes_are_escaped_and_unsafe_sources_rejected() {
    let html = render(
        "![\" alt <b>](image.png \"title \\\" quoted\") ![bad](javascript:evil) ![svg](data:image/svg+xml,evil)",
    );
    assert!(html.contains("alt=\"&quot; alt &lt;b&gt;\""), "{html}");
    assert!(html.contains("title=\"title &quot; quoted\""), "{html}");
    assert_eq!(html.matches("[Media unavailable:").count(), 2);
    assert!(!html.contains("src=\"javascript:"));
}

#[test]
fn footnotes_move_to_the_end_with_numbered_references_and_multiple_backlinks() {
    let html = render(
        "[^note]: First paragraph.\n\n    Another **paragraph**.\n\n# Document\n\nOne[^note], again[^note], and two[^second].\n\n[^second]: Other note.",
    );
    assert!(html.find("class=\"footnotes\"").unwrap() > html.find("One").unwrap());
    assert_eq!(html.matches("href=\"#ravel-fn-1\"").count(), 2);
    assert!(html.contains("id=\"ravel-ref-1-2\""));
    assert!(html.contains("href=\"#ravel-ref-1-1\""));
    assert!(html.contains("href=\"#ravel-ref-1-2\""));
    assert!(html.contains("Another <strong>paragraph</strong>"));
    assert!(html.contains("id=\"ravel-fn-2\""));
}

#[test]
fn heading_ids_cannot_shadow_footnote_targets() {
    let html = render("# Heading {#ravel-fn-1}\n\nText[^x].\n\n[^x]: Note");
    assert!(html.contains("<h1 id=\"ravel-fn-1-1\""));
    assert_eq!(html.matches("id=\"ravel-fn-1\"").count(), 1);
}

#[test]
fn math_placeholders_preserve_tex_and_do_not_parse_code_as_math() {
    let html = render(
        "Inline $\\frac{a}{b}$ and `$not math$`.\n\n$$\\int_0^1 x^2 dx$$\n\n```tex\n$literal$\n```\n",
    );
    assert!(html.contains("class=\"math math-inline\" data-tex=\"\\frac{a}{b}\""));
    assert!(html.contains("class=\"math math-display\" data-tex=\"\\int_0^1 x^2 dx\""));
    assert_eq!(html.matches("data-tex=").count(), 2);
    assert!(html.contains("<code>$not math$</code>"));
}

#[test]
fn task_checkboxes_are_interactive_without_changing_source() {
    let source = "- [x] Done\n- [ ] Pending\n  - [ ] Child";
    let document = Markdown::parse(source.as_bytes()).unwrap();
    assert_eq!(document.html().matches("type=\"checkbox\"").count(), 3);
    assert_eq!(document.html().matches(" checked>").count(), 1);
    assert!(!document.html().contains("disabled"));
    assert_eq!(document.source(), source);
}

#[test]
fn media_inventory_is_deduplicated_and_never_includes_source_html_or_links() {
    let document = Markdown::parse(b"![a](media/movie.webm) ![b](media/movie.webm) [link](other.md) ![blocked](javascript:evil)\n\n<img src=\"secret.png\">\n").unwrap();
    assert_eq!(document.media_sources(), ["media/movie.webm"]);
}

#[test]
fn tables_honor_alignment_hints() {
    let html = render("| Left | Center | Right |\n| :--- | :---: | ---: |\n| a | b | c |");
    assert!(html.contains("<th style=\"text-align:center\">Center</th>"));
    assert!(html.contains("<td style=\"text-align:right\">c</td>"));
}
