use crate::error::{ErrorCode, RuntimeError};
use super::value::StackValue;

pub fn check_arity(
    method: &str,
    args: &[StackValue],
    expected: usize,
    line: usize,
) -> Result<(), RuntimeError> {
    if args.len() != expected {
        return Err(RuntimeError::new(
            ErrorCode::R008,
            line,
            0,
            format!("`{method}` expects {expected} argument(s), got {}", args.len()),
        ));
    }
    Ok(())
}

pub fn require_float(sv: StackValue, line: usize) -> Result<f64, RuntimeError> {
    match sv {
        StackValue::Float(f) => Ok(f),
        StackValue::Bool(b) => Ok(if b { 1.0 } else { 0.0 }),
        _ => Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected float")),
    }
}

pub fn require_heap_ref(sv: StackValue, line: usize) -> Result<u32, RuntimeError> {
    match sv {
        StackValue::HeapRef(idx) => Ok(idx),
        _ => Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected heap object")),
    }
}

pub struct ColorDef {
    pub name: &'static str,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

pub const COLOR_CONSTANTS: &[ColorDef] = &[
    ColorDef { name: "red",         r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
    ColorDef { name: "green",       r: 0.0, g: 1.0, b: 0.0, a: 1.0 },
    ColorDef { name: "blue",        r: 0.0, g: 0.0, b: 1.0, a: 1.0 },
    ColorDef { name: "white",       r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
    ColorDef { name: "black",       r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
    ColorDef { name: "transparent", r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
];

#[must_use] 
pub fn lookup_color(name: &str) -> Option<&'static ColorDef> {
    COLOR_CONSTANTS.iter().find(|c| c.name == name)
}
