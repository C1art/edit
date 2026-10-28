use serde::ser::{self, Serialize};

use super::Value;
use crate::ProjectError;

pub fn to_value<T: ?Sized + Serialize>(value: &T) -> Result<Value, ProjectError> {
    value.serialize(ValueSerializer)
}

struct ValueSerializer;

fn unsupported(what: &str) -> ProjectError {
    ProjectError::Invalid(format!("{what} cannot be saved in a project"))
}

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = ProjectError;
    type SerializeSeq = Sequence;
    type SerializeTuple = Sequence;
    type SerializeTupleStruct = Sequence;
    type SerializeTupleVariant = Variant<Sequence>;
    type SerializeMap = Map;
    type SerializeStruct = Map;
    type SerializeStructVariant = Variant<Map>;

    fn serialize_bool(self, v: bool) -> Result<Value, ProjectError> {
        Ok(Value::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_i128(self, v: i128) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<Value, ProjectError> {
        Ok(Value::Integer(v.into()))
    }
    fn serialize_u128(self, v: u128) -> Result<Value, ProjectError> {
        i128::try_from(v)
            .map(Value::Integer)
            .map_err(|_| unsupported("a number this large"))
    }
    fn serialize_f32(self, _: f32) -> Result<Value, ProjectError> {
        Err(unsupported("a fractional number"))
    }
    fn serialize_f64(self, _: f64) -> Result<Value, ProjectError> {
        Err(unsupported("a fractional number"))
    }
    fn serialize_char(self, v: char) -> Result<Value, ProjectError> {
        Ok(Value::String(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<Value, ProjectError> {
        Ok(Value::String(v.to_owned()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Value, ProjectError> {
        Ok(Value::Array(
            v.iter().map(|&b| Value::Integer(b.into())).collect(),
        ))
    }
    fn serialize_none(self) -> Result<Value, ProjectError> {
        Ok(Value::Null)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Value, ProjectError> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Value, ProjectError> {
        Ok(Value::Null)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Value, ProjectError> {
        Ok(Value::Null)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Value, ProjectError> {
        Ok(Value::String(variant.to_owned()))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Value, ProjectError> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, ProjectError> {
        Ok(tagged(variant, to_value(value)?))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Sequence, ProjectError> {
        Ok(Sequence(Vec::with_capacity(len.unwrap_or(0))))
    }
    fn serialize_tuple(self, len: usize) -> Result<Sequence, ProjectError> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Sequence, ProjectError> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Variant<Sequence>, ProjectError> {
        Ok(Variant {
            name: variant,
            inner: Sequence(Vec::with_capacity(len)),
        })
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Map, ProjectError> {
        Ok(Map {
            fields: Vec::with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Map, ProjectError> {
        self.serialize_map(Some(len))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Variant<Map>, ProjectError> {
        Ok(Variant {
            name: variant,
            inner: self.serialize_map(Some(len))?,
        })
    }
}

/// Enum variants with data are written as a one-field object named after the variant.
fn tagged(variant: &str, value: Value) -> Value {
    Value::Object(vec![(variant.to_owned(), value)])
}

pub struct Sequence(Vec<Value>);

impl ser::SerializeSeq for Sequence {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), ProjectError> {
        self.0.push(to_value(value)?);
        Ok(())
    }
    fn end(self) -> Result<Value, ProjectError> {
        Ok(Value::Array(self.0))
    }
}

impl ser::SerializeTuple for Sequence {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), ProjectError> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, ProjectError> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for Sequence {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), ProjectError> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, ProjectError> {
        ser::SerializeSeq::end(self)
    }
}

pub struct Map {
    fields: Vec<(String, Value)>,
    key: Option<String>,
}

impl ser::SerializeMap for Map {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), ProjectError> {
        self.key = Some(match to_value(key)? {
            Value::String(text) => text,
            Value::Integer(number) => number.to_string(),
            _ => return Err(unsupported("a map key that isn't text or a number")),
        });
        Ok(())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), ProjectError> {
        let key = self
            .key
            .take()
            .ok_or_else(|| ProjectError::Invalid("map value without a key".to_owned()))?;
        self.fields.push((key, to_value(value)?));
        Ok(())
    }
    fn end(self) -> Result<Value, ProjectError> {
        Ok(Value::Object(self.fields))
    }
}

impl ser::SerializeStruct for Map {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), ProjectError> {
        self.fields.push((key.to_owned(), to_value(value)?));
        Ok(())
    }
    fn end(self) -> Result<Value, ProjectError> {
        Ok(Value::Object(self.fields))
    }
}

pub struct Variant<T> {
    name: &'static str,
    inner: T,
}

impl ser::SerializeTupleVariant for Variant<Sequence> {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), ProjectError> {
        ser::SerializeSeq::serialize_element(&mut self.inner, value)
    }
    fn end(self) -> Result<Value, ProjectError> {
        Ok(tagged(self.name, ser::SerializeSeq::end(self.inner)?))
    }
}

impl ser::SerializeStructVariant for Variant<Map> {
    type Ok = Value;
    type Error = ProjectError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), ProjectError> {
        ser::SerializeStruct::serialize_field(&mut self.inner, key, value)
    }
    fn end(self) -> Result<Value, ProjectError> {
        Ok(tagged(self.name, ser::SerializeStruct::end(self.inner)?))
    }
}
