use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::{SerializeSeq}};


pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(1))?;
    seq.serialize_element(value)?;
    seq.end()
}
pub fn deserialize<'de, T: Deserialize<'de>, D>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    // Try to deserialize as a sequence first
    let [elem] = <[T; 1]>::deserialize(deserializer)?;
    Ok(elem)
}