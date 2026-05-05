use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;

use super::value::Value;
use crate::syntax::ast::{Param, Stmt, Type};

/// Trait for dynamically-dispatched object types (structs, enums in future).
pub trait RustleObject: fmt::Debug {
    fn type_name(&self) -> &str;
    fn get_field(&self, name: &str) -> Option<Value>;
    fn set_field(&mut self, name: &str, val: Value) -> bool;
    fn clone_deep(&self) -> Box<dyn RustleObject>;
    fn field_names(&self) -> Vec<&str>;
    fn display(&self) -> String;
}

/// Visibility of a struct method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// A method definition stored in the shared method table.
#[derive(Debug, Clone)]
pub struct StructMethodDef {
    pub visibility: Visibility,
    pub params: Arc<[Param]>,
    pub body: Arc<[Stmt]>,
    pub return_ty: Option<Type>,
}

/// Shared method table — all instances of the same struct type share this.
pub type StructMethods = HashMap<String, StructMethodDef>;

/// A struct instance at runtime.
#[derive(Debug)]
pub struct StructInstance {
    pub type_name: String,
    pub fields: HashMap<String, Value>,
    pub methods: Rc<StructMethods>,
}

impl RustleObject for StructInstance {
    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn get_field(&self, name: &str) -> Option<Value> {
        self.fields.get(name).cloned()
    }

    fn set_field(&mut self, name: &str, val: Value) -> bool {
        if self.fields.contains_key(name) {
            self.fields.insert(name.to_string(), val);
            true
        } else {
            false
        }
    }

    fn clone_deep(&self) -> Box<dyn RustleObject> {
        let cloned_fields = self.fields.iter().map(|(k, v)| {
            (k.clone(), deep_clone_value(v))
        }).collect();
        Box::new(StructInstance {
            type_name: self.type_name.clone(),
            fields: cloned_fields,
            methods: self.methods.clone(),
        })
    }

    fn field_names(&self) -> Vec<&str> {
        self.fields.keys().map(String::as_str).collect()
    }

    fn display(&self) -> String {
        let fields: Vec<String> = self.fields.iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        format!("{} {{ {} }}", self.type_name, fields.join(", "))
    }
}

/// Deep-clone a Value, recursively cloning Object and List internals.
pub fn deep_clone_value(v: &Value) -> Value {
    match v {
        Value::Object(rc) => {
            let cloned = rc.borrow().clone_deep();
            Value::Object(Rc::new(RefCell::new(cloned)))
        }
        Value::List(rc) => {
            let cloned_items: Vec<Value> = rc.borrow().iter().map(deep_clone_value).collect();
            Value::List(Rc::new(RefCell::new(cloned_items)))
        }
        other => other.clone(),
    }
}
