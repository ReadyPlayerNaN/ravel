use ravel_core::markdown::Markdown;

#[test]
fn tagged_json_blocks_share_normal_document_parsing_and_keep_raw_source() {
    let source = "```JSON extra-info\n{ \"a/b\": [null, 123456789012345678901234567890] }\n```\n\n```json\nfalse\n```\n";
    let markdown = Markdown::parse(source.as_bytes()).unwrap();
    assert_eq!(markdown.source(), source);
    assert_eq!(markdown.json_blocks().len(), 2);
    assert_eq!(
        markdown.json_blocks()[0]
            .value_at("/a~1b/1")
            .unwrap()
            .to_string(),
        "123456789012345678901234567890"
    );
    assert_eq!(markdown.json_blocks()[1].value_at("").unwrap(), false);
    assert!(markdown.html().contains("data-json-block=\"0\""));
    assert!(markdown.html().contains("data-json-block=\"1\""));
    assert!(
        markdown
            .html()
            .contains("{ &quot;a/b&quot;: [null, 123456789012345678901234567890] }\n</code>")
    );
}

#[test]
fn invalid_untagged_and_other_languages_remain_plain_blocks() {
    let source = "```json\n{broken\n```\n\n```\n{}\n```\n\n```javascript\n{}\n```\n\n`{}`\n";
    let markdown = Markdown::parse(source.as_bytes()).unwrap();
    assert!(markdown.json_blocks().is_empty());
    assert!(!markdown.html().contains("data-json-block"));
    assert!(markdown.html().contains("{broken"));
}

#[test]
fn embedded_json_budgets_leave_the_full_raw_block_available() {
    let too_wide = serde_json::to_string(&vec![0; 129]).unwrap();
    let too_large = serde_json::to_string(&"x".repeat(65536)).unwrap();
    for json in [too_wide, too_large] {
        let source = format!("```json\n{json}\n```\n");
        let markdown = Markdown::parse(source.as_bytes()).unwrap();
        assert!(markdown.json_blocks().is_empty());
        assert_eq!(markdown.source(), source);
        assert!(
            markdown
                .html()
                .contains("<pre><code data-language=\"json\"")
        );
    }
    let many = Markdown::parse("```json\n{}\n```\n\n".repeat(17).as_bytes()).unwrap();
    assert_eq!(many.json_blocks().len(), 16);
    assert_eq!(many.html().matches("<pre><code").count(), 17);
}

#[test]
fn aggregate_and_node_budgets_bound_embedded_json_trees() {
    let large_string = serde_json::to_string(&"x".repeat(60000)).unwrap();
    let source = format!("```json\n{large_string}\n```\n\n").repeat(5);
    assert_eq!(
        Markdown::parse(source.as_bytes())
            .unwrap()
            .json_blocks()
            .len(),
        4
    );
    let nested = serde_json::to_string(&vec![vec![0; 128]; 20]).unwrap();
    let source = format!("```json\n{nested}\n```\n");
    assert!(
        Markdown::parse(source.as_bytes())
            .unwrap()
            .json_blocks()
            .is_empty()
    );
}

#[test]
fn footnote_json_blocks_get_their_own_native_slot_ids() {
    let source =
        "```json\n{}\n```\n\nA note[^n].\n\n[^n]: Note\n\n    ```json\n    [1,2]\n    ```\n";
    let markdown = Markdown::parse(source.as_bytes()).unwrap();
    assert_eq!(markdown.json_blocks().len(), 2);
    assert_eq!(markdown.html().matches("data-json-block=\"1\"").count(), 1);
}
