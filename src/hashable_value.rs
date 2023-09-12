#![allow(missing_docs)]

use std::fmt::{Debug};
use std::hash::{Hash, Hasher};
// use serde_json::{Map, Number};
use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),

    Array(Vec<Value>),
    Object(IndexMap<String, Value>),
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Null => state.write_u8(0),
            Value::Bool(boolean) => {
                state.write_u8(1);
                boolean.hash(state);
            }
            Value::Number(number) => {
                state.write_u8(2);
                number.hash(state);
            }
            Value::String(string) => {
                state.write_u8(3);
                string.hash(state);
            }
            Value::Array(vec) => {
                state.write_u8(4);
                vec.hash(state);
            }
            Value::Object(map) => {
                state.write_u8(5);
                map.iter().for_each(|(k, v)| {
                    k.hash(state);
                    v.hash(state);
                })
            }
        }
    }
}


impl Value {
    pub fn from_serde(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(boolean) => Value::Bool(boolean),
            serde_json::Value::Number(number) => Value::Number(number),
            serde_json::Value::String(string) => Value::String(string),
            serde_json::Value::Array(vec) => Value::Array(vec.into_iter().map(Value::from_serde).collect()),
            serde_json::Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, Value::from_serde(v))).collect()),
        }
    }

    pub fn into_serde(self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(boolean) => serde_json::Value::Bool(boolean),
            Value::Number(number) => serde_json::Value::Number(number),
            Value::String(string) => serde_json::Value::String(string),
            Value::Array(vec) => serde_json::Value::Array(vec.into_iter().map(Value::into_serde).collect()),
            Value::Object(map) => serde_json::Value::Object(map.into_iter().map(|(k, v)| (k, Value::into_serde(v))).collect()),
        }
    }
}