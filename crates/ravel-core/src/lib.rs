//! GTK-independent parsing and structural search for JSON and YAML documents.
//!
//! YAML is normalized to JSON-compatible values. Multi-document YAML streams
//! become synthetic arrays; comments, source formatting and tags are not retained.
//!
//! JSON Lines is represented as a synthetic array. Paths are RFC 6901 JSON
//! Pointers into that array (record indices, not physical line numbers).
#![deny(missing_docs)]

use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

mod yaml;
mod yaml_value;

/// Supported document encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// A single JSON value, including scalar roots.
    Json,
    /// One JSON value per nonblank line.
    JsonLines,
    /// YAML mappings, sequences and scalars; multiple documents form an array.
    Yaml,
}

impl Format {
    /// Detects `.jsonl`, `.yaml` and `.yml` case-insensitively; defaults to JSON.
    pub fn from_filename(name: &str) -> Self {
        match name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("jsonl") => Self::JsonLines,
            Some("yaml" | "yml") => Self::Yaml,
            _ => Self::Json,
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
    /// Classifies a JSON value into its [`Kind`].
    pub fn of(value: &Value) -> Self {
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
    /// Invalid YAML, including source diagnostics or unsupported values.
    #[error("Invalid YAML: {0}")]
    Yaml(#[source] Box<serde_saphyr::Error>),
    /// YAML data cannot be represented faithfully by the viewer.
    #[error("Invalid YAML: {message}")]
    YamlValue {
        /// Source diagnostic, including location when available.
        message: String,
    },
    /// Input text must be UTF-8.
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

/// Whole-document counts, including the root (and JSON Lines' synthetic array).
/// Depth is the maximum number of edges from the root; a scalar root has depth 0.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Statistics {
    /// Total JSON values, including containers.
    pub values: usize,
    /// Object containers.
    pub objects: usize,
    /// Array containers.
    pub arrays: usize,
    /// Object members, counting repeated names in different objects separately.
    pub keys: usize,
    /// String values (not object keys).
    pub strings: usize,
    /// Numeric values.
    pub numbers: usize,
    /// Boolean values.
    pub booleans: usize,
    /// Null values.
    pub nulls: usize,
    /// Maximum depth below the root.
    pub max_depth: usize,
}

/// A parsed, read-only document and its structural index.
#[derive(Debug)]
pub struct Document {
    value: Value,
    root: Arc<Node>,
    format: Format,
    document_count: usize,
}

impl Document {
    /// Parses UTF-8 JSON, JSON Lines or YAML and builds a browsable tree.
    /// YAML aliases and merges are resolved; multiple YAML documents form an
    /// array. Empty YAML becomes null. Custom tags and non-JSON values are rejected.
    ///
    /// A leading UTF-8 BOM is accepted. Blank JSON Lines records are skipped;
    /// an empty JSON Lines file is a valid zero-record document.
    ///
    /// # Errors
    /// Returns an encoding or syntax error; malformed JSON Lines reports the
    /// original physical line, even when earlier lines were blank.
    pub fn parse(bytes: &[u8], format: Format) -> Result<Self, ParseError> {
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        let (value, document_count) = match format {
            Format::Json => (serde_json::from_slice(bytes)?, 1),
            Format::Yaml => yaml::parse(std::str::from_utf8(bytes)?)?,
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
                let count = records.len();
                (Value::Array(records), count)
            }
        };
        let root = Node::build(&value, String::new(), "$".into());
        Ok(Self {
            value,
            root,
            format,
            document_count,
        })
    }

    /// Number of source documents: one for JSON, nonblank records for JSON Lines,
    /// and documents in a YAML stream (zero for empty/comment-only YAML).
    pub fn document_count(&self) -> usize {
        self.document_count
    }

    /// Returns the unfiltered tree's root.
    pub fn root(&self) -> Arc<Node> {
        Arc::clone(&self.root)
    }

    /// Counts all values in the unfiltered document, independent of UI state.
    pub fn statistics(&self) -> Statistics {
        let mut stats = Statistics::default();
        let mut pending = vec![(&*self.root, 0)];
        while let Some((node, depth)) = pending.pop() {
            stats.values += 1;
            stats.max_depth = stats.max_depth.max(depth);
            match node.kind {
                Kind::Object => {
                    stats.objects += 1;
                    stats.keys += node.children.len();
                }
                Kind::Array => stats.arrays += 1,
                Kind::String => stats.strings += 1,
                Kind::Number => stats.numbers += 1,
                Kind::Boolean => stats.booleans += 1,
                Kind::Null => stats.nulls += 1,
            }
            pending.extend(node.children.iter().map(|child| (&**child, depth + 1)));
        }
        stats
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
