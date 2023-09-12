use indexmap::IndexMap;
use crate::hashable_value::Value;
/// A representation of all key types typical Value types will assume.
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum Key {
    /// An array index
    Index(usize),
    /// A string index for mappings
    String(String),
}

fn value_items<'a>(value: &'a Value) -> Option<Box<dyn Iterator<Item = (Key, &'a Value)> + 'a>> {
    match *value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
            None
        }
        Value::Array(ref inner) => Some(Box::new(
            inner.iter().enumerate().map(|(i, v)| (Key::Index(i), v)),
        )),
        Value::Object(ref inner) => Some(Box::new(
            inner.iter().map(|(s, v)| (Key::String(s.clone()), v)),
        )),
    }

}

struct PatchDiffer {
    path: String,
    patch: super::Patch,
    shift: usize,
}

impl PatchDiffer {
    fn new() -> Self {
        Self {
            path: "".to_string(),
            patch: super::Patch(Vec::new()),
            shift: 0,
        }
    }
}

fn tdiff<'a>(l: &'a Value, r: &'a Value, d: &mut PatchDiffer) {
    match (value_items(l), value_items(r)) {
        // two scalars, equal
        (None, None) if l == r => d.unchanged(l),
        // two scalars, different
        (None, None) => d.modified(l, r),
        // two objects, equal
        (Some(_), Some(_)) if l == r => d.unchanged(l),
        // object and scalar
        (Some(_), None) | (None, Some(_)) => d.modified(l, r),
        // two objects, different
        (Some(li), Some(ri)) => {
            let mut ml = IndexMap::new();
            ml.extend(li);
            let mut mr = IndexMap::new();
            mr.extend(ri);
            for k in mr.keys().filter(|k| ml.contains_key(*k)) {
                let v1 = ml.get(k).expect("key to exist in map");
                let v2 = mr.get(k).expect("key to exist in map");
                d.push(k);
                tdiff(*v1, *v2, d);
                d.pop();
            }
            for k in mr.keys().filter(|k| !ml.contains_key(*k)) {
                d.added(k, mr.get(k).expect("key to exist in map"));
            }
            for k in ml.keys().filter(|k| !mr.contains_key(*k)) {
                d.removed(k, ml.get(k).expect("key to exist in map"));
            }
        }
    }
}


impl<'a> PatchDiffer {
    fn push(&mut self, key: &Key) {
        use std::fmt::Write;
        self.path.push('/');
        match *key {
            Key::Index(idx) => write!(self.path, "{}", idx - self.shift).unwrap(),
            Key::String(ref key) => append_path(&mut self.path, key),
        }
    }

    fn pop(&mut self) {
        let pos = self.path.rfind('/').unwrap_or(0);
        self.path.truncate(pos);
        self.shift = 0;
    }

    fn removed<'b>(&mut self, k: &'b Key, _v: &'a Value) {
        let len = self.path.len();
        self.push(k);
        self.patch
            .0
            .push(super::PatchOperation::Remove(super::RemoveOperation {
                path: self.path.clone(),
            }));
        // Shift indices, we are deleting array elements
        if let Key::Index(_) = k {
            self.shift += 1;
        }
        self.path.truncate(len);
    }

    fn added(&mut self, k: &Key, v: &Value) {
        let len = self.path.len();
        self.push(k);
        self.patch
            .0
            .push(super::PatchOperation::Add(super::AddOperation {
                path: self.path.clone(),
                value: v.clone().into_serde(),
            }));
        self.path.truncate(len);
    }

    fn modified(&mut self, _old: &'a Value, new: &'a Value) {
        self.patch
            .0
            .push(super::PatchOperation::Replace(super::ReplaceOperation {
                path: self.path.clone(),
                value: new.clone().into_serde(),
            }));
    }
    fn unchanged(&mut self, _v: &'a Value) {}
}

fn append_path(path: &mut String, key: &str) {
    path.reserve(key.len());
    for ch in key.chars() {
        if ch == '~' {
            *path += "~0";
        } else if ch == '/' {
            *path += "~1";
        } else {
            path.push(ch);
        }
    }
}

/// Diff two JSON documents and generate a JSON Patch (RFC 6902).
///
/// # Example
/// Diff two JSONs:
///
/// ```rust
/// #[macro_use]
/// use json_patch::{Patch, patch, diff};
/// use serde_json::{json, from_value};
///
/// # pub fn main() {
/// let left = json!({
///   "title": "Goodbye!",
///   "author" : {
///     "givenName" : "John",
///     "familyName" : "Doe"
///   },
///   "tags":[ "example", "sample" ],
///   "content": "This will be unchanged"
/// });
///
/// let right = json!({
///   "title": "Hello!",
///   "author" : {
///     "givenName" : "John"
///   },
///   "tags": [ "example" ],
///   "content": "This will be unchanged",
///   "phoneNumber": "+01-123-456-7890"
/// });
///
/// let p = diff(&left, &right);
/// assert_eq!(p, from_value::<Patch>(json!([
///   { "op": "remove", "path": "/author/familyName" },
///   { "op": "remove", "path": "/tags/1" },
///   { "op": "replace", "path": "/title", "value": "Hello!" },
///   { "op": "add", "path": "/phoneNumber", "value": "+01-123-456-7890" },
/// ])).unwrap());
///
/// let mut doc = left.clone();
/// patch(&mut doc, &p).unwrap();
/// assert_eq!(doc, right);
///
/// # }
/// ```
pub fn diff(left: &serde_json::Value, right: &serde_json::Value) -> super::Patch {
    let left = Value::from_serde(left.clone());
    let right = Value::from_serde(right.clone());
    let mut differ = PatchDiffer::new();
    tdiff(&left, &right, &mut differ);
    differ.patch
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    #[test]
    pub fn replace_all() {
        let left = json!({"title": "Hello!"});
        let p = super::diff(&left, &Value::Null);
        assert_eq!(
            p,
            serde_json::from_value(json!([
                { "op": "replace", "path": "", "value": null },
            ]))
            .unwrap()
        );
        let mut left = json!({"title": "Hello!"});
        crate::patch(&mut left, &p).unwrap();
    }

    #[test]
    pub fn diff_empty_key() {
        let left = json!({"title": "Something", "": "Hello!"});
        let right = json!({"title": "Something", "": "Bye!"});
        let p = super::diff(&left, &right);
        assert_eq!(
            p,
            serde_json::from_value(json!([
                { "op": "replace", "path": "/", "value": "Bye!" },
            ]))
            .unwrap()
        );
        let mut left_patched = json!({"title": "Something", "": "Hello!"});
        crate::patch(&mut left_patched, &p).unwrap();
        assert_eq!(left_patched, right);
    }

    #[test]
    pub fn add_all() {
        let right = json!({"title": "Hello!"});
        let p = super::diff(&Value::Null, &right);
        assert_eq!(
            p,
            serde_json::from_value(json!([
                { "op": "replace", "path": "", "value": { "title": "Hello!" } },
            ]))
            .unwrap()
        );
    }

    #[test]
    pub fn remove_all() {
        let left = json!(["hello", "bye"]);
        let right = json!([]);
        let p = super::diff(&left, &right);
        assert_eq!(
            p,
            serde_json::from_value(json!([
                { "op": "remove", "path": "/0" },
                { "op": "remove", "path": "/0" },
            ]))
            .unwrap()
        );
    }

    #[test]
    pub fn remove_tail() {
        let left = json!(["hello", "bye", "hi"]);
        let right = json!(["hello"]);
        let p = super::diff(&left, &right);
        assert_eq!(
            p,
            serde_json::from_value(json!([
                { "op": "remove", "path": "/1" },
                { "op": "remove", "path": "/1" },
            ]))
            .unwrap()
        );
    }
    #[test]
    pub fn replace_object() {
        let left = json!(["hello", "bye"]);
        let right = json!({"hello": "bye"});
        let p = super::diff(&left, &right);
        assert_eq!(
            p,
            serde_json::from_value(json!([
                { "op": "add", "path": "/hello", "value": "bye" },
                { "op": "remove", "path": "/0" },
                { "op": "remove", "path": "/0" },
            ]))
            .unwrap()
        );
    }

    #[test]
    fn escape_json_keys() {
        let mut left = json!({
            "/slashed/path/with/~": 1
        });
        let right = json!({
            "/slashed/path/with/~": 2,
        });
        let patch = super::diff(&left, &right);

        crate::patch(&mut left, &patch).unwrap();
        assert_eq!(left, right);
    }
}
