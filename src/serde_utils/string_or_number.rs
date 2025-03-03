//! String or number serialization/deserialization
//!
//! This module provides utilities for handling values that can be
//! either strings or numbers in serialized form.

use serde::{Deserializer, Serializer};
use std::fmt;

/// Deserialize a value that could be either a string or a number into a f64
pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> serde::de::Visitor<'de> for StringOrNumberVisitor {
        type Value = f64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or number")
        }

        fn visit_str<E>(self, value: &str) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            value.parse::<f64>().map_err(serde::de::Error::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            value.parse::<f64>().map_err(serde::de::Error::custom)
        }

        fn visit_f64<E>(self, value: f64) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            Ok(value as f64)
        }

        fn visit_u64<E>(self, value: u64) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            Ok(value as f64)
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}

/// Serialize a number as a string
pub fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}