//! Foldable JSON containers in a rendered preview, using GTK character offsets.

use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub struct Block {
    pub line: u32,
    /// Hide complete body lines, preserving the opener and indented closing line.
    pub body: Range<i32>,
}

pub fn blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut stack = Vec::new();
    let mut line = 0_u32;
    let line_starts: Vec<_> = std::iter::once(0)
        .chain(
            text.chars()
                .enumerate()
                .filter(|(_, ch)| *ch == '\n')
                .map(|(offset, _)| offset + 1),
        )
        .collect();
    let mut string = false;
    let mut escape = false;
    for ch in text.chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
        }
        if string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                string = false;
            }
            continue;
        }
        match ch {
            '"' => string = true,
            '{' | '[' => stack.push((ch, line)),
            '}' | ']' => {
                let Some((open, start_line)) = stack.pop() else {
                    return Vec::new();
                };
                if !matches!((open, ch), ('{', '}') | ('[', ']')) {
                    return Vec::new();
                }
                if start_line < line
                    && let (Ok(start), Ok(end)) = (
                        i32::try_from(line_starts[start_line as usize + 1]),
                        i32::try_from(line_starts[line as usize]),
                    )
                    && start < end
                {
                    blocks.push(Block {
                        line: start_line,
                        body: start..end,
                    });
                }
            }
            _ => {}
        }
    }
    if string || !stack.is_empty() {
        return Vec::new();
    }
    blocks.sort_unstable_by_key(|block| block.line);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn indexes_root_objects_arrays_and_nested_containers() {
        let text = serde_json::to_string_pretty(&json!({"users": [{"name": "A"}, {"name": "B"}]}))
            .unwrap();
        assert_eq!(
            blocks(&text)
                .iter()
                .map(|block| block.line)
                .collect::<Vec<_>>(),
            [0, 1, 2, 5]
        );
    }

    #[test]
    fn collapsed_interiors_leave_valid_delimiters_visible() {
        let text = "{\n  \"users\": [\n    1\n  ]\n}";
        let block = &blocks(text)[1];
        let visible: String = text
            .chars()
            .enumerate()
            .filter(|(offset, _)| !block.body.contains(&(*offset as i32)))
            .map(|(_, ch)| ch)
            .collect();
        assert_eq!(visible, "{\n  \"users\": [\n  ]\n}");
    }

    #[test]
    fn ignores_delimiters_and_escaped_quotes_inside_strings() {
        let text =
            serde_json::to_string_pretty(&json!({"{[\\\"": "}\"[\\", "nested": {"key": "[]"}}))
                .unwrap();
        assert_eq!(blocks(&text).len(), 2);
    }

    #[test]
    fn offsets_count_unicode_characters_not_bytes() {
        let text = "{\n  \"é\": {\n    \"name\": 1\n  }\n}";
        let block = &blocks(text)[1];
        assert_eq!(text.chars().nth(block.body.start as usize - 1), Some('\n'));
        assert_eq!(
            text.chars()
                .skip(block.body.end as usize)
                .collect::<String>(),
            "  }\n}"
        );
    }

    #[test]
    fn inline_empty_containers_and_scalars_have_nothing_to_fold() {
        assert!(blocks("{}\n[]\ntrue\n\"[]\"").is_empty());
    }

    #[test]
    fn handles_json_lines_synthetic_root_arrays() {
        let text = serde_json::to_string_pretty(&json!([{"name": "A"}, {"name": "B"}])).unwrap();
        assert_eq!(blocks(&text).len(), 3);
    }

    #[test]
    fn malformed_or_incomplete_delimiters_have_no_folds() {
        for text in ["{\n]", "{\n", "}\n", "\"{\n"] {
            assert!(blocks(text).is_empty(), "Unexpected folds for {text:?}");
        }
    }
}
