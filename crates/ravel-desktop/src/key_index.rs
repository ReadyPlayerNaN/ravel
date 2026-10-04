//! Key positions in a serde_json-rendered preview, using GTK character offsets.

use std::ops::Range;

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
enum PathPart {
    Member(String),
    ArrayItem,
}

#[derive(Debug)]
pub struct Key {
    pub span: Range<i32>,
    name: String,
    path: Vec<PathPart>,
}

impl Key {
    pub fn matches(&self, other: &Self, same_path: bool) -> bool {
        self.name == other.name && (!same_path || self.path == other.path)
    }
}

#[derive(Default)]
pub struct KeyIndex {
    keys: Vec<Key>,
}

impl KeyIndex {
    /// `text` must be serde_json's rendering of `document.pointer(pointer)`.
    pub fn new(document: &Value, pointer: &str, text: &str) -> Self {
        let Some(value) = document.pointer(pointer) else {
            return Self::default();
        };
        let Some(mut path) = path_at(document, pointer) else {
            return Self::default();
        };
        let mut members = Vec::new();
        collect_members(value, &mut path, &mut members);
        // The serializer emits keys in the same depth-first order as Value's members.
        let spans = key_spans(text);
        if spans.len() != members.len() {
            return Self::default();
        }
        Self {
            keys: spans
                .into_iter()
                .zip(members)
                .map(|(span, (name, path))| Key { span, name, path })
                .collect(),
        }
    }

    pub fn at(&self, offset: i32) -> Option<&Key> {
        let index = self.keys.partition_point(|key| key.span.end <= offset);
        self.keys
            .get(index)
            .filter(|key| key.span.contains(&offset))
    }

    pub fn in_range(&self, range: Range<i32>) -> &[Key] {
        let start = self.keys.partition_point(|key| key.span.end <= range.start);
        let end = self.keys.partition_point(|key| key.span.start < range.end);
        &self.keys[start..end.max(start)]
    }
}

fn path_at(mut value: &Value, pointer: &str) -> Option<Vec<PathPart>> {
    let mut path = Vec::new();
    if pointer.is_empty() {
        return Some(path);
    }
    for token in pointer.strip_prefix('/')?.split('/') {
        let member = token.replace("~1", "/").replace("~0", "~");
        match value {
            Value::Object(object) => {
                value = object.get(&member)?;
                path.push(PathPart::Member(member));
            }
            Value::Array(array) => {
                value = array.get(member.parse::<usize>().ok()?)?;
                path.push(PathPart::ArrayItem);
            }
            _ => return None,
        }
    }
    Some(path)
}

fn collect_members(
    value: &Value,
    path: &mut Vec<PathPart>,
    members: &mut Vec<(String, Vec<PathPart>)>,
) {
    match value {
        Value::Object(object) => {
            for (name, value) in object {
                path.push(PathPart::Member(name.clone()));
                members.push((name.clone(), path.clone()));
                collect_members(value, path, members);
                path.pop();
            }
        }
        Value::Array(array) => {
            path.push(PathPart::ArrayItem);
            for value in array {
                collect_members(value, path, members);
            }
            path.pop();
        }
        _ => {}
    }
}

fn key_spans(text: &str) -> Vec<Range<i32>> {
    let mut spans = Vec::new();
    let mut chars = text.chars().enumerate().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        while let Some((end, ch)) = chars.next() {
            if ch == '\\' {
                chars.next();
            } else if ch == '"' {
                while chars.peek().is_some_and(|(_, ch)| ch.is_whitespace()) {
                    chars.next();
                }
                if chars.peek().is_some_and(|(_, ch)| *ch == ':')
                    && let (Ok(start), Ok(end)) = (i32::try_from(start), i32::try_from(end + 1))
                {
                    spans.push(start..end);
                }
                break;
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn index(value: &Value, pointer: &str) -> KeyIndex {
        KeyIndex::new(
            value,
            pointer,
            &serde_json::to_string_pretty(value.pointer(pointer).unwrap()).unwrap(),
        )
    }

    #[test]
    fn hover_matches_names_across_unrelated_paths() {
        let value = json!({"users": [{"name": "A"}, {"name": "B"}], "metadata": {"name": "C"}});
        let index = index(&value, "");
        let names: Vec<_> = index.keys.iter().filter(|key| key.name == "name").collect();
        assert!(names.iter().all(|key| names[0].matches(key, false)));
    }

    #[test]
    fn shift_hover_matches_array_siblings_but_not_other_objects() {
        let value = json!({"users": [{"name": "A"}, {"name": "B"}], "metadata": {"name": "C"}});
        let index = index(&value, "");
        let names: Vec<_> = index.keys.iter().filter(|key| key.name == "name").collect();
        assert_eq!(
            names
                .iter()
                .filter(|key| names[0].matches(key, true))
                .count(),
            2
        );
    }

    #[test]
    fn key_names_are_case_sensitive() {
        let index = index(&json!({"name": 1, "Name": 2}), "");
        assert!(!index.keys[0].matches(&index.keys[1], false));
    }

    #[test]
    fn literal_wildcard_members_are_distinct_from_array_items() {
        let value =
            json!({"users": [{"children": [{"name": "A"}]}, {"children": {"*": {"name": "B"}}}]});
        let index = index(&value, "");
        let names: Vec<_> = index.keys.iter().filter(|key| key.name == "name").collect();
        assert!(!names[0].matches(names[1], true));
    }

    #[test]
    fn numeric_object_keys_are_not_array_wildcards() {
        let value = json!({"users": {"0": {"name": "A"}, "1": {"name": "B"}}});
        let index = index(&value, "");
        let names: Vec<_> = index.keys.iter().filter(|key| key.name == "name").collect();
        assert!(!names[0].matches(names[1], true));
    }

    #[test]
    fn shift_hover_wildcards_nested_arrays() {
        let value = json!({"users": [{"groups": [{"name": "A"}]}, {"groups": [{"name": "B"}, {"name": "C"}]}]});
        let index = index(&value, "");
        let names: Vec<_> = index.keys.iter().filter(|key| key.name == "name").collect();
        assert!(names.iter().all(|key| names[0].matches(key, true)));
    }

    #[test]
    fn selected_subtrees_keep_their_absolute_structural_path() {
        let value = json!({"users": [{"name": "A"}, {"name": "B"}], "metadata": {"name": "C"}});
        let first = index(&value, "/users/0");
        let second = index(&value, "/users/1");
        let other = index(&value, "/metadata");
        assert_eq!(
            (
                first.keys[0].matches(&second.keys[0], true),
                first.keys[0].matches(&other.keys[0], true)
            ),
            (true, false)
        );
    }

    #[test]
    fn escaped_pointer_members_are_unambiguous() {
        let value =
            json!({"a/b": {"~": [{"name": 1}, {"name": 2}]}, "a": {"b": {"~": [{"name": 3}]}}});
        let index = index(&value, "/a~1b/~0");
        assert_eq!(
            index.keys[0].path,
            vec![
                PathPart::Member("a/b".into()),
                PathPart::Member("~".into()),
                PathPart::ArrayItem,
                PathPart::Member("name".into())
            ]
        );
    }

    #[test]
    fn positions_count_unicode_characters_and_skip_string_values() {
        let value = json!({"é": "\"name\":", "name": "é", "quote\"\\": "ok", "": 1});
        let text = serde_json::to_string_pretty(&value).unwrap();
        let index = KeyIndex::new(&value, "", &text);
        let slices: Vec<String> = index
            .keys
            .iter()
            .map(|key| {
                text.chars()
                    .skip(key.span.start as usize)
                    .take((key.span.end - key.span.start) as usize)
                    .collect()
            })
            .collect();
        assert_eq!(slices, ["\"é\"", "\"name\"", "\"quote\\\"\\\\\"", "\"\""]);
    }

    #[test]
    fn hit_testing_includes_quotes_but_excludes_colons_and_values() {
        let index = index(&json!({"name": "name"}), "");
        let span = &index.keys[0].span;
        assert_eq!(
            (
                index.at(span.start).is_some(),
                index.at(span.end - 1).is_some(),
                index.at(span.end).is_some(),
                index.at(span.end + 3).is_some()
            ),
            (true, true, false, false)
        );
    }

    #[test]
    fn viewport_ranges_include_only_intersecting_keys() {
        let index = index(&json!({"name": 1, "other": 2, "last": 3}), "");
        assert_eq!(index.in_range(index.keys[1].span.clone()).len(), 1);
    }

    #[test]
    fn json_lines_root_arrays_use_the_same_structural_path() {
        let index = index(&json!([{"name": 1}, {"name": 2}]), "");
        assert!(index.keys[0].matches(&index.keys[1], true));
    }

    #[test]
    fn scalar_roots_and_invalid_pointers_have_no_keys() {
        assert!(index(&json!("name"), "").keys.is_empty());
        assert!(KeyIndex::new(&json!({}), "/missing", "{}").keys.is_empty());
    }
}
