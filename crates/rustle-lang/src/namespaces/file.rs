use crate::syntax::ast::Type;
use crate::error::{ErrorCode, RuntimeError};
use crate::Value;
use std::collections::HashMap;
use super::{Export, ExportKind, NamespaceInfo, NamespaceProvider, RuntimeState, check_argc};

fn ffn(name: &'static str, params: Vec<Type>, ret: Type) -> Export {
    Export { name, kind: ExportKind::Function, ty: Type::Fn(params, Some(Box::new(ret))) }
}

pub struct FileNamespace;

impl NamespaceInfo for FileNamespace {
    fn name(&self) -> &'static str { "file" }

    fn exports(&self) -> Vec<Export> {
        vec![
            ffn("read", vec![Type::String], Type::Res(Box::new(Type::String))),
            ffn("read_lines", vec![Type::String], Type::Res(Box::new(Type::List(Box::new(Type::String))))),
            ffn("write", vec![Type::String, Type::String], Type::Res(Box::new(Type::Bool))),
            ffn("append", vec![Type::String, Type::String], Type::Res(Box::new(Type::Bool))),
        ]
    }
}

impl NamespaceProvider for FileNamespace {
    fn call(
        &self,
        name: &str,
        args: &[Value],
        _named: &HashMap<String, Value>,
        _state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        match name {
            "read" => {
                check_argc(name, args, 1, line)?;
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => return Ok(Some(Value::ResErr("read: expected string path".into()))),
                };
                match std::fs::read_to_string(&path) {
                    Ok(content) => Ok(Some(Value::ResOk(Box::new(Value::Str(content))))),
                    Err(e) => Ok(Some(Value::ResErr(e.to_string()))),
                }
            }
            "read_lines" => {
                check_argc(name, args, 1, line)?;
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => return Ok(Some(Value::ResErr("read_lines: expected string path".into()))),
                };
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let lines: Vec<Value> = content.lines().map(|l| Value::Str(l.to_string())).collect();
                        Ok(Some(Value::ResOk(Box::new(Value::List(
                            std::rc::Rc::new(std::cell::RefCell::new(lines)),
                        )))))
                    }
                    Err(e) => Ok(Some(Value::ResErr(e.to_string()))),
                }
            }
            "write" => {
                check_argc(name, args, 2, line)?;
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => return Ok(Some(Value::ResErr("write: expected string path".into()))),
                };
                let content = match &args[1] {
                    Value::Str(s) => s.clone(),
                    _ => return Ok(Some(Value::ResErr("write: expected string content".into()))),
                };
                match std::fs::write(&path, &content) {
                    Ok(()) => Ok(Some(Value::ResOk(Box::new(Value::Bool(true))))),
                    Err(e) => Ok(Some(Value::ResErr(e.to_string()))),
                }
            }
            "append" => {
                check_argc(name, args, 2, line)?;
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => return Ok(Some(Value::ResErr("append: expected string path".into()))),
                };
                let content = match &args[1] {
                    Value::Str(s) => s.clone(),
                    _ => return Ok(Some(Value::ResErr("append: expected string content".into()))),
                };
                use std::io::Write;
                let result = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .and_then(|mut f| f.write_all(content.as_bytes()));
                match result {
                    Ok(()) => Ok(Some(Value::ResOk(Box::new(Value::Bool(true))))),
                    Err(e) => Ok(Some(Value::ResErr(e.to_string()))),
                }
            }
            _ => Ok(None),
        }
    }

    fn get_constant(&self, _name: &str) -> Option<Value> { None }
}
