//! YAML ingestion with strict JSON-compatible normalization and parser budgets.
use serde_json::Value;
use serde_saphyr::granit_parser::{Event, Parser, ScalarStyle};

use crate::{ParseError, yaml_value::JsonValue};

pub(crate) fn parse(text: &str) -> Result<(Value, usize), ParseError> {
    let options = serde_saphyr::options! {
        strict_booleans: true,
        no_schema: true,
        reject_unsupported_tags: true,
    };
    let documents: Vec<JsonValue> = serde_saphyr::from_multiple_with_options(text, options)
        .map_err(|error| ParseError::Yaml(Box::new(error)))?;
    // The stream API deliberately omits null documents. Recover those slots
    // from parser events, not a line-based splitter (markers can occur in strings).
    let slots = document_slots(text)?;
    let mut values = documents.into_iter().map(|value| value.0);
    let documents = slots
        .into_iter()
        .map(|null| {
            if null {
                Ok(Value::Null)
            } else {
                values
                    .next()
                    .ok_or_else(|| unsupported("Could not resolve YAML document boundaries"))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.next().is_some() {
        return Err(unsupported("Could not resolve YAML document boundaries"));
    }
    let count = documents.len();
    let value = if count == 1 {
        documents.into_iter().next().unwrap_or(Value::Null)
    } else if count == 0 {
        Value::Null
    } else {
        Value::Array(documents)
    };
    Ok((value, count))
}

fn unsupported(message: impl Into<String>) -> ParseError {
    ParseError::YamlValue {
        message: message.into(),
    }
}

fn document_slots(text: &str) -> Result<Vec<bool>, ParseError> {
    let mut slots = Vec::new();
    let mut root_pending = false;
    for event in Parser::new_from_str(text) {
        let (event, span) = event.map_err(|error| unsupported(error.to_string()))?;
        match event {
            Event::DocumentStart(..) => root_pending = true,
            Event::Scalar(value, style, _, tag) => {
                let tag = tag.as_deref();
                let plain = style == ScalarStyle::Plain;
                let null = tag.is_some_and(|tag| tag.is_yaml_core_schema_tag("null"))
                    || (tag.is_none()
                        && plain
                        && (value.is_empty()
                            || value == "~"
                            || value.eq_ignore_ascii_case("null")));
                if root_pending {
                    slots.push(null);
                    root_pending = false;
                }
                // deserialize_any falls back to f64 outside i64/u64. Reject
                // oversized integer tokens rather than silently rounding them.
                if ((plain && tag.is_none())
                    || tag.is_some_and(|tag| tag.is_yaml_core_schema_tag("int")))
                    && oversized_integer(&value)
                {
                    return Err(unsupported(format!(
                        "Integer is outside the supported i64/u64 range at line {}, column {}; quote it to retain it as a string",
                        span.start.line(),
                        span.start.col() + 1
                    )));
                }
            }
            Event::SequenceStart(..) | Event::MappingStart(..) | Event::Alias(..)
                if root_pending =>
            {
                slots.push(false);
                root_pending = false;
            }
            _ => {}
        }
    }
    Ok(slots)
}

fn oversized_integer(text: &str) -> bool {
    let clean = text.replace('_', "");
    let negative = clean.starts_with('-');
    let digits = clean.strip_prefix(['+', '-']).unwrap_or(&clean);
    let (radix, digits) = if let Some(digits) = digits.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = digits.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = digits.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, digits)
    };
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_digit(radix)) {
        return false;
    }
    u128::from_str_radix(digits, radix).map_or(true, |value| {
        value
            > if negative {
                (i64::MAX as u128) + 1
            } else {
                u64::MAX as u128
            }
    })
}
