use ravel_core::{Document, Format, Statistics};

#[test]
fn counts_all_types_keys_and_depth() {
    let document = Document::parse(
        br#"{"a":[1,true,null,"x",{}],"b":{"a":false}}"#,
        Format::Json,
    )
    .unwrap();
    assert_eq!(
        document.statistics(),
        Statistics {
            values: 9,
            objects: 3,
            arrays: 1,
            keys: 3,
            strings: 1,
            numbers: 1,
            booleans: 2,
            nulls: 1,
            max_depth: 2,
        }
    );
}

#[test]
fn scalar_and_empty_roots_have_depth_zero() {
    for input in ["null", "{}", "[]", "\"text\"", "123"] {
        let stats = Document::parse(input.as_bytes(), Format::Json)
            .unwrap()
            .statistics();
        assert_eq!(stats.values, 1);
        assert_eq!(stats.max_depth, 0);
        assert_eq!(stats.keys, 0);
    }
}

#[test]
fn json_lines_counts_synthetic_array_and_nonblank_records() {
    let document = Document::parse(b"\n{\"x\":1}\n\nfalse\n", Format::JsonLines).unwrap();
    let stats = document.statistics();
    assert_eq!(stats.values, 4);
    assert_eq!(stats.arrays, 1);
    assert_eq!(stats.objects, 1);
    assert_eq!(stats.keys, 1);
    assert_eq!(stats.max_depth, 2);
    let empty = Document::parse(b"\n", Format::JsonLines)
        .unwrap()
        .statistics();
    assert_eq!(empty.values, 1);
    assert_eq!(empty.arrays, 1);
    assert_eq!(empty.max_depth, 0);
}
