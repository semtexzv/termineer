//! Element array serialization/deserialization
//!
//! This module provides utilities for handling elements that can be either
//! serialized as a single item or as an array with a single element.

use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

/// Serialize a value as a single-element array
pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(1))?;
    seq.serialize_element(value)?;
    seq.end()
}

/// Deserialize a value from a single-element array
pub fn deserialize<'de, T: Deserialize<'de>, D>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    // Try to deserialize as a sequence first
    let [elem] = <[T; 1]>::deserialize(deserializer)?;
    Ok(elem)
}