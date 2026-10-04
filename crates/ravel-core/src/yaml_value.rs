//! JSON-compatible YAML values without serde_json's private numeric map protocol.
use std::fmt;

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};

pub(crate) struct JsonValue(pub Value);

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON-compatible YAML value with string mapping keys")
    }

    fn visit_unit<E: de::Error>(self) -> Result<JsonValue, E> {
        Ok(JsonValue(Value::Null))
    }
    fn visit_none<E: de::Error>(self) -> Result<JsonValue, E> {
        self.visit_unit()
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<JsonValue, E> {
        Ok(JsonValue(Value::Bool(value)))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<JsonValue, E> {
        Ok(JsonValue(Value::Number(value.into())))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<JsonValue, E> {
        Ok(JsonValue(Value::Number(value.into())))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<JsonValue, E> {
        Number::from_f64(value)
            .map(|number| JsonValue(Value::Number(number)))
            .ok_or_else(|| E::custom("Non-finite numbers cannot be represented as JSON"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<JsonValue, E> {
        self.visit_string(value.to_owned())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<JsonValue, E> {
        Ok(JsonValue(Value::String(value)))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<JsonValue, A::Error> {
        let mut values = Vec::new();
        while let Some(JsonValue(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(JsonValue(Value::Array(values)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<JsonValue, A::Error> {
        let mut values = Map::new();
        while let Some((key, JsonValue(value))) = mapping.next_entry::<String, JsonValue>()? {
            values.insert(key, value);
        }
        Ok(JsonValue(Value::Object(values)))
    }
}
