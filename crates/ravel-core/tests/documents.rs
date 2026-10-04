use ravel_core::{Document, Format, Kind, ParseError};
use serde_json::json;

#[test]
fn json_supports_nested_objects_and_arrays() {
    let document = Document::parse(br#"{"items":[{"name":"Ravel"}]}"#, Format::Json).unwrap();
    assert_eq!(document.value_at("/items/0/name"), Some(&json!("Ravel")));
}

#[test]
fn json_accepts_scalar_roots() {
    let document = Document::parse(b"true", Format::Json).unwrap();
    assert_eq!(document.root().kind, Kind::Boolean);
}

#[test]
fn json_rejects_empty_input() {
    assert!(matches!(
        Document::parse(b"", Format::Json),
        Err(ParseError::Json(_))
    ));
}

#[test]
fn json_rejects_trailing_values() {
    assert!(Document::parse(b"{} {}", Format::Json).is_err());
}

#[test]
fn json_rejects_invalid_utf8() {
    assert!(Document::parse(b"\"\xff\"", Format::Json).is_err());
}

#[test]
fn json_accepts_a_utf8_bom() {
    let document = Document::parse(b"\xef\xbb\xbf{}", Format::Json).unwrap();
    assert_eq!(document.value_at(""), Some(&json!({})));
}

#[test]
fn json_lines_accepts_crlf_blank_lines_and_scalar_records() {
    let document = Document::parse(b"{}\r\n \r\n42\r\nnull\r\n", Format::JsonLines).unwrap();
    assert_eq!(document.value_at(""), Some(&json!([{}, 42, null])));
}

#[test]
fn json_lines_reports_physical_line_after_blank_records() {
    let error = Document::parse(b"{}\n\n{invalid}\n", Format::JsonLines).unwrap_err();
    assert!(matches!(error, ParseError::Record { line: 3, .. }));
}

#[test]
fn json_lines_does_not_accept_multiple_values_on_one_line() {
    assert!(matches!(
        Document::parse(b"{} {}\n", Format::JsonLines),
        Err(ParseError::Record { line: 1, .. })
    ));
}

#[test]
fn json_lines_rejects_invalid_utf8() {
    assert!(matches!(
        Document::parse(b"\xff", Format::JsonLines),
        Err(ParseError::Encoding(_))
    ));
}

#[test]
fn json_lines_accepts_zero_records() {
    let document = Document::parse(b"\n \n", Format::JsonLines).unwrap();
    assert_eq!(document.value_at(""), Some(&json!([])));
}

#[test]
fn format_detection_is_case_insensitive() {
    assert_eq!(Format::from_filename("events.JSONL"), Format::JsonLines);
}

#[test]
fn a_filename_without_an_extension_defaults_to_json() {
    assert_eq!(Format::from_filename("jsonl"), Format::Json);
}

#[test]
fn pointers_escape_slash_and_tilde_in_keys() {
    let document = Document::parse(br#"{"a/b":{"~key":7}}"#, Format::Json).unwrap();
    let node = &document.root().children[0].children[0];
    assert_eq!(document.value_at(&node.pointer), Some(&json!(7)));
}

#[test]
fn an_empty_key_has_a_distinct_pointer_from_the_root() {
    let document = Document::parse(br#"{"":false}"#, Format::Json).unwrap();
    assert_eq!(document.root().children[0].pointer, "/");
}

#[test]
fn search_retains_ancestors_and_prunes_unrelated_children() {
    let document = Document::parse(
        br#"{"user":{"name":"Alice","age":42},"other":null}"#,
        Format::Json,
    )
    .unwrap();
    let root = document.filter("ALICE", None).unwrap();
    let pointers: Vec<_> = root
        .children
        .iter()
        .flat_map(|child| &child.children)
        .map(|node| node.pointer.as_str())
        .collect();
    assert_eq!(pointers, ["/user/name"]);
}

#[test]
fn search_matches_keys() {
    let document = Document::parse(br#"{"needle":null,"other":false}"#, Format::Json).unwrap();
    let root = document.filter("needle", None).unwrap();
    assert_eq!(root.children[0].pointer, "/needle");
}

#[test]
fn search_matches_json_pointers() {
    let document = Document::parse(br#"{"data":[3]}"#, Format::Json).unwrap();
    assert!(document.filter("/data/0", None).is_some());
}

#[test]
fn search_matches_text_beyond_the_truncated_preview() {
    let source = json!({"text": format!("{}needle", "x".repeat(100))}).to_string();
    let document = Document::parse(source.as_bytes(), Format::Json).unwrap();
    assert!(document.filter("needle", Some(Kind::String)).is_some());
}

#[test]
fn type_filter_keeps_only_matching_values_and_their_ancestors() {
    let document = Document::parse(br#"{"a":1,"b":"text","c":[true,2]}"#, Format::Json).unwrap();
    let root = document.filter("", Some(Kind::Number)).unwrap();
    assert_eq!(
        root.children
            .iter()
            .map(|node| node.pointer.as_str())
            .collect::<Vec<_>>(),
        ["/a", "/c"]
    );
}

#[test]
fn search_and_type_filter_combine_using_and() {
    let document = Document::parse(br#"{"number":42}"#, Format::Json).unwrap();
    assert!(document.filter("42", Some(Kind::String)).is_none());
}

#[test]
fn no_search_matches_returns_no_tree() {
    let document = Document::parse(b"{}", Format::Json).unwrap();
    assert!(document.filter("missing", None).is_none());
}

#[test]
fn clearing_filters_reuses_the_original_tree() {
    let document = Document::parse(b"{}", Format::Json).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &document.root(),
        &document.filter("  ", None).unwrap()
    ));
}

#[test]
fn filtering_does_not_change_the_underlying_document() {
    let document = Document::parse(br#"{"keep":1,"hide":2}"#, Format::Json).unwrap();
    let _ = document.filter("keep", None);
    assert_eq!(document.value_at("/hide"), Some(&json!(2)));
}

#[test]
fn large_numbers_keep_their_precision_when_displayed() {
    let document = Document::parse(b"123456789012345678901234567890", Format::Json).unwrap();
    assert_eq!(
        document.value_at("").unwrap().to_string(),
        "123456789012345678901234567890"
    );
}

#[test]
fn object_members_keep_their_source_order() {
    let document = Document::parse(br#"{"z":1,"a":2}"#, Format::Json).unwrap();
    assert_eq!(
        document
            .root()
            .children
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
}

#[test]
fn scalar_previews_are_single_line() {
    let document = Document::parse(br#""hello\nworld""#, Format::Json).unwrap();
    assert_eq!(document.root().preview, "hello world");
}
