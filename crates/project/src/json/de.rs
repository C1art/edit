use serde::de::{self, DeserializeOwned, IntoDeserializer, Visitor};
use serde::forward_to_deserialize_any;

use super::Value;
use crate::ProjectError;

pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, ProjectError> {
    T::deserialize(value)
}

impl<'de> IntoDeserializer<'de, ProjectError> for Value {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de> de::Deserializer<'de> for Value {
    type Error = ProjectError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, ProjectError> {
        match self {
            Self::Null => visitor.visit_unit(),
            Self::Bool(flag) => visitor.visit_bool(flag),
            Self::Integer(number) => {
                if let Ok(small) = i64::try_from(number) {
                    visitor.visit_i64(small)
                } else if let Ok(large) = u64::try_from(number) {
                    visitor.visit_u64(large)
                } else {
                    visitor.visit_i128(number)
                }
            }
            Self::String(text) => visitor.visit_string(text),
            Self::Array(items) => visitor.visit_seq(Items(items.into_iter())),
            Self::Object(fields) => visitor.visit_map(Fields {
                fields: fields.into_iter(),
                value: None,
            }),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, ProjectError> {
        match self {
            Self::Null => visitor.visit_none(),
            value => visitor.visit_some(value),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, ProjectError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, ProjectError> {
        let variant = match self {
            Self::String(name) => Some(Variant { name, value: None }),
            Self::Object(fields) => {
                let mut fields = fields.into_iter();
                match (fields.next(), fields.next()) {
                    (Some((name, value)), None) => Some(Variant {
                        name,
                        value: Some(value),
                    }),
                    _ => None,
                }
            }
            _ => None,
        };
        let variant = variant.ok_or_else(|| {
            ProjectError::Invalid("expected an enum variant name or a one-field object".to_owned())
        })?;
        visitor.visit_enum(variant)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

struct Items(std::vec::IntoIter<Value>);

impl<'de> de::SeqAccess<'de> for Items {
    type Error = ProjectError;

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, ProjectError> {
        self.0.next().map(|item| seed.deserialize(item)).transpose()
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.0.len())
    }
}

struct Fields {
    fields: std::vec::IntoIter<(String, Value)>,
    value: Option<Value>,
}

impl<'de> de::MapAccess<'de> for Fields {
    type Error = ProjectError;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, ProjectError> {
        let Some((name, value)) = self.fields.next() else {
            return Ok(None);
        };
        self.value = Some(value);
        seed.deserialize(Value::String(name)).map(Some)
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, ProjectError> {
        let value = self
            .value
            .take()
            .ok_or_else(|| ProjectError::Invalid("field value read twice".to_owned()))?;
        seed.deserialize(value)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.fields.len())
    }
}

struct Variant {
    name: String,
    value: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for Variant {
    type Error = ProjectError;
    type Variant = Payload;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Payload), ProjectError> {
        Ok((
            seed.deserialize(Value::String(self.name))?,
            Payload(self.value),
        ))
    }
}

/// The data of an enum variant; unit variants have none.
struct Payload(Option<Value>);

impl<'de> de::VariantAccess<'de> for Payload {
    type Error = ProjectError;

    fn unit_variant(self) -> Result<(), ProjectError> {
        match self.0 {
            None | Some(Value::Null) => Ok(()),
            Some(_) => Err(ProjectError::Invalid(
                "unexpected data for a unit variant".to_owned(),
            )),
        }
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, ProjectError> {
        seed.deserialize(self.0.unwrap_or(Value::Null))
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, ProjectError> {
        de::Deserializer::deserialize_seq(self.0.unwrap_or(Value::Null), visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, ProjectError> {
        de::Deserializer::deserialize_map(self.0.unwrap_or(Value::Null), visitor)
    }
}
