//! GTK-independent parsing and structural search for JSON documents.
//!
//! JSON Lines is represented as a synthetic array. Paths are RFC 6901 JSON
//! Pointers into that array (record indices, not physical line numbers).
#![deny(missing_docs)]

use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

/// Supported document encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// A single JSON value, including scalar roots.
    Json,
    /// One JSON value per nonblank line.
    JsonLines,
}

impl Format {
    /// Detects JSON Lines by a case-insensitive `.jsonl` extension; defaults to JSON.
    pub fn from_filename(name: &str) -> Self {
        if name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("jsonl"))
        {
            Self::JsonLines
        } else {
            Self::Json
        }
    }
}

/// A JSON value's type, also used as a structural filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An object with named members.
    Object,
    /// An ordered array.
    Array,
    /// A Unicode string.
    String,
    /// A JSON number.
    Number,
    /// A boolean.
    Boolean,
    /// JSON null.
    Null,
}

impl Kind {
    fn of(value: &Value) -> Self {
        match value {
            Value::Object(_) => Self::Object,
            Value::Array(_) => Self::Array,
            Value::String(_) => Self::String,
            Value::Number(_) => Self::Number,
            Value::Bool(_) => Self::Boolean,
            Value::Null => Self::Null,
        }
    }
}

/// Input errors with source location preserved.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Invalid JSON, including line and column information.
    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// JSON Lines must be UTF-8 text.
    #[error("The document is not UTF-8: {0}")]
    Encoding(#[from] std::str::Utf8Error),
    /// A malformed record at its physical, one-based line number.
    #[error("Invalid JSON Lines record on line {line}: {source}")]
    Record {
        /// Physical line number in the source file.
        line: usize,
        /// Underlying JSON parser error.
        #[source]
        source: serde_json::Error,
    },
}

/// An immutable tree entry; children are shared rather than deep-cloned.
#[derive(Debug)]
pub struct Node {
    /// RFC 6901 pointer; the root has an empty pointer.
    pub pointer: String,
    /// Object key, array index, or `$` for the root.
    pub label: String,
    /// JSON value type.
    pub kind: Kind,
    /// Single-line, truncated value preview or container size.
    pub preview: String,
    /// Direct child entries, possibly pruned by a search.
    pub children: Vec<Arc<Node>>,
    search_text: String,
}

impl Node {
    fn build(value: &Value, pointer: String, label: String) -> Arc<Self> {
        let children = match value {
            Value::Object(values) => values
                .iter()
                .map(|(key, value)| {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    Self::build(value, format!("{pointer}/{escaped}"), key.clone())
                })
                .collect(),
            Value::Array(values) => values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    Self::build(value, format!("{pointer}/{index}"), format!("[{index}]"))
                })
                .collect(),
            _ => Vec::new(),
        };
        let description = match value {
            Value::Object(values) => format!("{{}} · {} members", values.len()),
            Value::Array(values) => format!("[] · {} items", values.len()),
            Value::String(text) => text.clone(),
            _ => value.to_string(),
        };
        let search_text = format!("{label}\n{pointer}\n{description}").to_lowercase();
        let mut preview: String = description
            .chars()
            .take(80)
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect();
        if description.chars().count() > 80 {
            preview.push('…');
        }
        Arc::new(Self {
            pointer,
            label,
            kind: Kind::of(value),
            preview,
            children,
            search_text,
        })
    }

    fn filtered(self: &Arc<Self>, query: &str, kind: Option<Kind>) -> Option<Arc<Self>> {
        let children: Vec<_> = self
            .children
            .iter()
            .filter_map(|child| child.filtered(query, kind))
            .collect();
        let matches = kind.is_none_or(|kind| kind == self.kind) && self.search_text.contains(query);
        if !matches && children.is_empty() {
            return None;
        }
        Some(Arc::new(Self {
            pointer: self.pointer.clone(),
            label: self.label.clone(),
            kind: self.kind,
            preview: self.preview.clone(),
            children,
            search_text: self.search_text.clone(),
        }))
    }
}

/// A parsed, read-only document and its structural index.
#[derive(Debug)]
pub struct Document {
    value: Value,
    root: Arc<Node>,
    format: Format,
}

impl Document {
    /// Parses UTF-8 JSON or JSON Lines and builds a browsable tree.
    ///
    /// A leading UTF-8 BOM is accepted. Blank JSON Lines records are skipped;
    /// an empty JSON Lines file is a valid zero-record document.
    ///
    /// # Errors
    /// Returns an encoding or syntax error; malformed JSON Lines reports the
    /// original physical line, even when earlier lines were blank.
    pub fn parse(bytes: &[u8], format: Format) -> Result<Self, ParseError> {
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        let value = match format {
            Format::Json => serde_json::from_slice(bytes)?,
            Format::JsonLines => {
                let mut records = Vec::new();
                for (index, line) in std::str::from_utf8(bytes)?.lines().enumerate() {
                    if !line.trim().is_empty() {
                        records.push(serde_json::from_str(line).map_err(|source| {
                            ParseError::Record {
                                line: index + 1,
                                source,
                            }
                        })?);
                    }
                }
                Value::Array(records)
            }
        };
        let root = Node::build(&value, String::new(), "$".into());
        Ok(Self {
            value,
            root,
            format,
        })
    }

    /// Returns the unfiltered tree's root.
    pub fn root(&self) -> Arc<Node> {
        Arc::clone(&self.root)
    }

    /// Returns the document's input format.
    pub fn format(&self) -> Format {
        self.format
    }

    /// Resolves an RFC 6901 pointer to a value; `""` selects the entire document.
    pub fn value_at(&self, pointer: &str) -> Option<&Value> {
        self.value.pointer(pointer)
    }

    /// Searches keys, pointers and full scalar values with case-insensitive
    /// substring matching. Type filtering combines with search using AND.
    /// Ancestors remain visible to preserve structure; unrelated descendants
    /// are pruned even when their parent matches. No matches returns `None`.
    pub fn filter(&self, query: &str, kind: Option<Kind>) -> Option<Arc<Node>> {
        if query.trim().is_empty() && kind.is_none() {
            return Some(self.root());
        }
        self.root.filtered(&query.trim().to_lowercase(), kind)
    }
}
