use ravel_core::{Document, Format};
use serde_json::json;

#[test]
fn detects_both_yaml_extensions_case_insensitively() {
    for name in ["config.yaml", "config.yml", "config.YAML", "config.YmL"] {
        assert_eq!(Format::from_filename(name), Format::Yaml);
    }
}

#[test]
fn yaml_maps_sequences_scalars_and_member_order() {
    let doc = Document::parse(
        b"name: Ravel\nenabled: true\ncount: 3\nratio: 1.5\nitems: [null, false, '42']\n",
        Format::Yaml,
    )
    .unwrap();
    assert_eq!(
        doc.value_at(""),
        Some(
            &json!({"name":"Ravel", "enabled":true, "count":3, "ratio":1.5, "items":[null,false,"42"]})
        )
    );
    assert_eq!(
        doc.root()
            .children
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        ["name", "enabled", "count", "ratio", "items"]
    );
    assert_eq!(doc.document_count(), 1);
    assert_eq!(doc.statistics().arrays, 1);
}

#[test]
fn yaml_uses_strict_booleans_and_keeps_dates_as_strings() {
    let doc = Document::parse(
        b"country: NO\nyes_value: yes\non_value: on\ndate: 2026-01-01\n",
        Format::Yaml,
    )
    .unwrap();
    assert_eq!(doc.value_at("/country"), Some(&json!("NO")));
    assert_eq!(doc.value_at("/yes_value"), Some(&json!("yes")));
    assert_eq!(doc.value_at("/on_value"), Some(&json!("on")));
    assert_eq!(doc.value_at("/date"), Some(&json!("2026-01-01")));
}

#[test]
fn yaml_resolves_aliases_and_merge_overrides() {
    let doc = Document::parse(b"defaults: &base\n  name: Alice\n  active: true\nuser:\n  <<: *base\n  name: Bob\ncopy: *base\n", Format::Yaml).unwrap();
    assert_eq!(doc.value_at("/user/name"), Some(&json!("Bob")));
    assert_eq!(doc.value_at("/user/active"), Some(&json!(true)));
    assert_eq!(doc.value_at("/copy/name"), Some(&json!("Alice")));
    assert!(doc.value_at("/user/<<").is_none());
}

#[test]
fn yaml_handles_multiline_strings_and_escaped_pointers() {
    let doc = Document::parse("a/b~c: |\n  héllo\n  world\n".as_bytes(), Format::Yaml).unwrap();
    assert_eq!(doc.value_at("/a~1b~0c"), Some(&json!("héllo\nworld\n")));
}

#[test]
fn yaml_streams_are_arrays_only_when_multiple_documents_exist() {
    let doc = Document::parse(b"---\nname: first\n---\n[1, 2]\n---\nnull\n", Format::Yaml).unwrap();
    assert_eq!(doc.document_count(), 3);
    assert_eq!(doc.value_at("/0/name"), Some(&json!("first")));
    assert_eq!(doc.value_at("/1/1"), Some(&json!(2)));
    assert_eq!(doc.value_at("/2"), Some(&json!(null)));
    let single = Document::parse(b"---\n[1, 2]\n...\n", Format::Yaml).unwrap();
    assert_eq!(single.document_count(), 1);
    assert_eq!(single.value_at("/1"), Some(&json!(2)));
}

#[test]
fn yaml_handles_empty_input_scalar_roots_bom_and_crlf() {
    for input in ["", "# comment\n"] {
        let doc = Document::parse(input.as_bytes(), Format::Yaml).unwrap();
        assert_eq!(doc.value_at(""), Some(&json!(null)));
        assert_eq!(doc.document_count(), 0);
    }
    let doc = Document::parse(b"\xef\xbb\xbfname: Alice\r\n", Format::Yaml).unwrap();
    assert_eq!(doc.value_at("/name"), Some(&json!("Alice")));
    assert_eq!(
        Document::parse(b"42", Format::Yaml).unwrap().value_at(""),
        Some(&json!(42))
    );
    assert!(Document::parse(b"\xff", Format::Yaml).is_err());
}

#[test]
fn yaml_rejects_unsupported_values_without_silent_coercion() {
    for input in [
        "1: value",
        "true: value",
        "? [a, b]\n: value",
        "x: .nan",
        "x: .inf",
        "x: -.inf",
        "x: !custom hello",
        "x: 1\nx: 2",
        "x: *missing",
        "x: &loop [*loop]",
    ] {
        assert!(
            Document::parse(input.as_bytes(), Format::Yaml).is_err(),
            "Unexpectedly accepted {input:?}"
        );
    }
}

#[test]
fn yaml_preserves_null_and_empty_document_slots() {
    let doc = Document::parse(b"---\nnull\n---\n---\n~\n---\n!!str null\n", Format::Yaml).unwrap();
    assert_eq!(doc.document_count(), 4);
    assert_eq!(doc.value_at(""), Some(&json!([null, null, null, "null"])));
    let explicit = Document::parse(b"---\n", Format::Yaml).unwrap();
    assert_eq!(explicit.document_count(), 1);
    assert_eq!(explicit.value_at(""), Some(&json!(null)));
}

#[test]
fn yaml_mapping_keys_do_not_invoke_json_private_number_protocol() {
    let doc = Document::parse(b"'$serde_json::private::Number': '123'\n", Format::Yaml).unwrap();
    assert_eq!(
        doc.value_at(""),
        Some(&json!({"$serde_json::private::Number":"123"}))
    );
}

#[test]
fn yaml_keeps_document_markers_inside_block_strings() {
    let doc = Document::parse(b"text: |\n  ---\n  ...\n---\ntext: second\n", Format::Yaml).unwrap();
    assert_eq!(doc.document_count(), 2);
    assert_eq!(doc.value_at("/0/text"), Some(&json!("---\n...\n")));
}

#[test]
fn yaml_rejects_excessive_nesting_before_building_the_tree() {
    let text = format!("{}null{}", "[".repeat(100), "]".repeat(100));
    assert!(Document::parse(text.as_bytes(), Format::Yaml).is_err());
}

#[test]
fn yaml_syntax_errors_include_location() {
    let error = Document::parse(b"name: Alice\nitems: [1, 2\n", Format::Yaml)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("Invalid YAML:"));
    assert!(error.contains("line") || error.contains("-->"), "{error}");
}

#[test]
fn yaml_rejects_oversized_integers_instead_of_rounding() {
    let error = Document::parse(b"number: 123456789012345678901234567890\n", Format::Yaml)
        .unwrap_err()
        .to_string();
    assert!(error.contains("i64/u64"), "{error}");
    let doc = Document::parse(
        b"number: 18446744073709551615\nnegative: -9223372036854775808\n",
        Format::Yaml,
    )
    .unwrap();
    assert_eq!(
        doc.value_at("/number").unwrap().to_string(),
        "18446744073709551615"
    );
    assert_eq!(
        doc.value_at("/negative").unwrap().to_string(),
        "-9223372036854775808"
    );
}
