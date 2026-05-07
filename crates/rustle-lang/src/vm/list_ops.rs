//! Higher-order list operations for the bytecode VM.
//!
//! These functions need access to `Vm::call_closure_sync`, so they live in a
//! separate module as free functions that take `&mut Vm`.

use crate::error::{ErrorCode, RuntimeError};
use super::value::{is_truthy, values_equal, HeapObject, StackValue};
use super::vm::Vm;

/// Dispatch a list higher-order function call.
///
/// `list_idx` is the heap index of a `HeapObject::List`.
pub fn exec_list_hof(
    vm: &mut Vm,
    list_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    match method {
        "map" => list_map(vm, list_idx, args, line),
        "filter" => list_filter(vm, list_idx, args, line),
        "any" => list_any(vm, list_idx, args, line),
        "all" => list_all(vm, list_idx, args, line),
        "sort" => {
            if args.is_empty() {
                list_sort_default(vm, list_idx, line)
            } else {
                list_sort_custom(vm, list_idx, args, line)
            }
        }
        "search" => list_search(vm, list_idx, args, line),
        "bsearch" => list_bsearch(vm, list_idx, args, line),
        "take" => list_take(vm, list_idx, args, line),
        "drop" => list_drop(vm, list_idx, args, line),
        "cut" => list_cut(vm, list_idx, args, line),
        "paste" => list_paste(vm, list_idx, args, line),
        _ => Err(RuntimeError::new(
            ErrorCode::R004,
            line,
            0,
            format!("method `{method}` not found on list"),
        )),
    }
}

// ─── Closure-based operations ────────────────────────────────────────────────

fn list_map(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("map", args, 1, line)?;
    let closure = args[0];
    let items = get_list_items(&vm.heap, list_idx, line)?;
    let mut result = Vec::with_capacity(items.len());
    for item in &items {
        let val = vm.call_closure_sync(closure, &[*item], line)?;
        result.push(val);
    }
    Ok(vm.heap.alloc(HeapObject::List(result)))
}

fn list_filter(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("filter", args, 1, line)?;
    let closure = args[0];
    let items = get_list_items(&vm.heap, list_idx, line)?;
    let mut result = Vec::new();
    for item in &items {
        let val = vm.call_closure_sync(closure, &[*item], line)?;
        if is_truthy(val, &vm.heap) {
            result.push(*item);
        }
    }
    Ok(vm.heap.alloc(HeapObject::List(result)))
}

fn list_any(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("any", args, 1, line)?;
    let closure = args[0];
    let items = get_list_items(&vm.heap, list_idx, line)?;
    for item in &items {
        let val = vm.call_closure_sync(closure, &[*item], line)?;
        if is_truthy(val, &vm.heap) {
            return Ok(StackValue::Bool(true));
        }
    }
    Ok(StackValue::Bool(false))
}

fn list_all(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("all", args, 1, line)?;
    let closure = args[0];
    let items = get_list_items(&vm.heap, list_idx, line)?;
    for item in &items {
        let val = vm.call_closure_sync(closure, &[*item], line)?;
        if !is_truthy(val, &vm.heap) {
            return Ok(StackValue::Bool(false));
        }
    }
    Ok(StackValue::Bool(true))
}

fn list_sort_default(
    vm: &mut Vm,
    list_idx: u32,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    // Extract items so the mutable borrow on heap ends, allowing immutable reads
    // for string comparison during sort.
    let mut vec = match vm.heap.get_mut(list_idx) {
        HeapObject::List(items) => std::mem::take(items),
        _ => return Err(expected_list(line)),
    };
    vec.sort_by(|a, b| {
        match (a, b) {
            (StackValue::Float(af), StackValue::Float(bf)) => {
                af.partial_cmp(bf).unwrap_or(std::cmp::Ordering::Equal)
            }
            (StackValue::HeapRef(ai), StackValue::HeapRef(bi)) => {
                match (vm.heap.get(*ai), vm.heap.get(*bi)) {
                    (HeapObject::Str(sa), HeapObject::Str(sb)) => sa.cmp(sb),
                    _ => std::cmp::Ordering::Equal,
                }
            }
            _ => std::cmp::Ordering::Equal,
        }
    });
    match vm.heap.get_mut(list_idx) {
        HeapObject::List(items) => *items = vec,
        _ => unreachable!(),
    }
    Ok(StackValue::Float(0.0))
}

fn list_sort_custom(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("sort", args, 1, line)?;
    let closure = args[0];
    let mut vec = match vm.heap.get_mut(list_idx) {
        HeapObject::List(items) => std::mem::take(items),
        _ => return Err(expected_list(line)),
    };
    let mut sort_error: Option<RuntimeError> = None;
    vec.sort_by(|a, b| {
        if sort_error.is_some() {
            return std::cmp::Ordering::Equal;
        }
        match vm.call_closure_sync(closure, &[*a, *b], line) {
            Ok(StackValue::Float(f)) => {
                if f < 0.0 {
                    std::cmp::Ordering::Less
                } else if f > 0.0 {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            }
            Ok(_) => std::cmp::Ordering::Equal,
            Err(e) => {
                sort_error = Some(e);
                std::cmp::Ordering::Equal
            }
        }
    });
    match vm.heap.get_mut(list_idx) {
        HeapObject::List(items) => *items = vec,
        _ => unreachable!(),
    }
    if let Some(e) = sort_error {
        return Err(e);
    }
    Ok(StackValue::Float(0.0))
}

// ─── Non-closure operations ──────────────────────────────────────────────────

fn list_search(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("search", args, 1, line)?;
    let target = args[0];
    let items = match vm.heap.get(list_idx) {
        HeapObject::List(items) => items,
        _ => return Err(expected_list(line)),
    };
    for (i, item) in items.iter().enumerate() {
        if values_equal(*item, target, &vm.heap) {
            return Ok(StackValue::Float(i as f64));
        }
    }
    Ok(StackValue::Float(-1.0))
}

fn list_bsearch(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("bsearch", args, 1, line)?;
    let target = args[0];
    let items = match vm.heap.get(list_idx) {
        HeapObject::List(items) => items.clone(),
        _ => return Err(expected_list(line)),
    };
    let result = items.binary_search_by(|item| match (item, &target) {
        (StackValue::Float(a), StackValue::Float(b)) => {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (StackValue::HeapRef(ai), StackValue::HeapRef(bi)) => {
            match (vm.heap.get(*ai), vm.heap.get(*bi)) {
                (HeapObject::Str(sa), HeapObject::Str(sb)) => sa.cmp(sb),
                _ => std::cmp::Ordering::Equal,
            }
        }
        _ => std::cmp::Ordering::Equal,
    });
    match result {
        Ok(idx) => Ok(StackValue::Float(idx as f64)),
        Err(_) => Ok(StackValue::Float(-1.0)),
    }
}

fn list_take(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("take", args, 2, line)?;
    let start = require_usize(args[0], line)?;
    let end = require_usize(args[1], line)?;
    let items = match vm.heap.get(list_idx) {
        HeapObject::List(items) => items,
        _ => return Err(expected_list(line)),
    };
    let len = items.len();
    let start = start.min(len);
    let end = end.min(len);
    let slice = items[start..end].to_vec();
    Ok(vm.heap.alloc(HeapObject::List(slice)))
}

fn list_drop(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("drop", args, 2, line)?;
    let start = require_usize(args[0], line)?;
    let end = require_usize(args[1], line)?;
    let items = match vm.heap.get(list_idx) {
        HeapObject::List(items) => items,
        _ => return Err(expected_list(line)),
    };
    let len = items.len();
    let start = start.min(len);
    let end = end.min(len);
    let mut result = items[..start].to_vec();
    result.extend_from_slice(&items[end..]);
    Ok(vm.heap.alloc(HeapObject::List(result)))
}

fn list_cut(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("cut", args, 2, line)?;
    let start = require_usize(args[0], line)?;
    let end = require_usize(args[1], line)?;
    let items = match vm.heap.get_mut(list_idx) {
        HeapObject::List(items) => items,
        _ => return Err(expected_list(line)),
    };
    let len = items.len();
    let start = start.min(len);
    let end = end.min(len);
    let removed: Vec<StackValue> = items.drain(start..end).collect();
    Ok(vm.heap.alloc(HeapObject::List(removed)))
}

fn list_paste(
    vm: &mut Vm,
    list_idx: u32,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    check_arity("paste", args, 2, line)?;
    let idx = require_usize(args[0], line)?;
    let to_insert = args[1];

    // Check if inserting a list or single value
    let insert_items: Vec<StackValue> = if let StackValue::HeapRef(other_idx) = to_insert {
        match vm.heap.get(other_idx) {
            HeapObject::List(other) => other.clone(),
            _ => vec![to_insert],
        }
    } else {
        vec![to_insert]
    };

    let items = match vm.heap.get_mut(list_idx) {
        HeapObject::List(items) => items,
        _ => return Err(expected_list(line)),
    };
    let idx = idx.min(items.len());
    for (i, item) in insert_items.into_iter().enumerate() {
        items.insert(idx + i, item);
    }
    Ok(StackValue::Float(0.0))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn check_arity(
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

fn require_usize(sv: StackValue, line: usize) -> Result<usize, RuntimeError> {
    match sv {
        StackValue::Float(f) => {
            if f < 0.0 {
                return Err(RuntimeError::new(
                    ErrorCode::R011,
                    line,
                    0,
                    "index must be non-negative",
                ));
            }
            Ok(f as usize)
        }
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            "expected float",
        )),
    }
}

fn expected_list(line: usize) -> RuntimeError {
    RuntimeError::new(ErrorCode::R001, line, 0, "expected list")
}

/// Clone list items out of the heap so the borrow ends before closure calls.
fn get_list_items(
    heap: &super::heap::Heap,
    list_idx: u32,
    line: usize,
) -> Result<Vec<StackValue>, RuntimeError> {
    match heap.get(list_idx) {
        HeapObject::List(items) => Ok(items.clone()),
        _ => Err(expected_list(line)),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{
        chunk::Chunk,
        natives::native_table,
        opcode::Op,
        value::{ClosureObj, HeapObject, StackValue},
        CompiledProgram,
    };
    use std::sync::Arc;

    // ── Test helpers ─────────────────────────────────────────────────────────

    /// Build a minimal VM with custom chunks.
    fn make_vm(chunks: Vec<Chunk>, constants: Vec<StackValue>) -> Vm {
        let program = Arc::new(CompiledProgram {
            chunks,
            constants,
            strings: vec![],
            natives: vec![],
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
            state_fields: vec![],
            struct_defs: vec![],
            enum_defs: vec![],
            global_count: 0,
        });
        Vm::new(program, native_table())
    }

    /// Create a closure chunk that computes `arg0 * constant` and returns it.
    /// Chunk has 1 param (local 0), multiplies by the constant at `const_idx`.
    fn make_mul_chunk(const_idx: u16) -> Chunk {
        let mut c = Chunk::new("<test_mul>");
        c.param_count = 1;
        c.local_count = 1;
        c.emit(Op::LoadLocal(0), 1); // push arg0
        c.emit(Op::Const(const_idx), 1); // push constant
        c.emit(Op::Mul, 1);
        c.emit(Op::Return, 1);
        c
    }

    /// Create a closure chunk that returns `arg0 > constant` as bool.
    fn make_gt_chunk(const_idx: u16) -> Chunk {
        let mut c = Chunk::new("<test_gt>");
        c.param_count = 1;
        c.local_count = 1;
        c.emit(Op::LoadLocal(0), 1);
        c.emit(Op::Const(const_idx), 1);
        c.emit(Op::Gt, 1);
        c.emit(Op::Return, 1);
        c
    }

    /// Create a comparator closure chunk: returns `arg0 - arg1`.
    fn make_cmp_chunk() -> Chunk {
        let mut c = Chunk::new("<test_cmp>");
        c.param_count = 2;
        c.local_count = 2;
        c.emit(Op::LoadLocal(0), 1);
        c.emit(Op::LoadLocal(1), 1);
        c.emit(Op::Sub, 1);
        c.emit(Op::Return, 1);
        c
    }

    /// Allocate a closure on the heap pointing to the given chunk index.
    fn alloc_closure(vm: &mut Vm, chunk_index: u16) -> StackValue {
        vm.heap.alloc(HeapObject::Closure(ClosureObj {
            chunk_index,
            upvalues: vec![],
        }))
    }

    /// Allocate a list of floats on the heap.
    fn alloc_float_list(vm: &mut Vm, vals: &[f64]) -> (StackValue, u32) {
        let items: Vec<StackValue> = vals.iter().map(|&v| StackValue::Float(v)).collect();
        let sv = vm.heap.alloc(HeapObject::List(items));
        let idx = sv.as_heap_ref().unwrap();
        (sv, idx)
    }

    /// Read the list items from the heap.
    fn read_list(vm: &Vm, idx: u32) -> Vec<StackValue> {
        match vm.heap.get(idx) {
            HeapObject::List(items) => items.clone(),
            _ => panic!("expected list"),
        }
    }

    /// Extract floats from a list of StackValues.
    fn as_floats(items: &[StackValue]) -> Vec<f64> {
        items.iter().map(|sv| sv.as_float().unwrap()).collect()
    }

    // ── map ──────────────────────────────────────────────────────────────────

    #[test]
    fn map_doubles_elements() {
        // Chunk 0 = top-level (unused), Chunk 1 = closure: arg0 * 2.0
        let top = Chunk::new("<top>");
        let mul_chunk = make_mul_chunk(0);
        let mut vm = make_vm(vec![top, mul_chunk], vec![StackValue::Float(2.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "map", &[closure], 1).unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert_eq!(as_floats(&items), vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn map_empty_list() {
        let top = Chunk::new("<top>");
        let mul_chunk = make_mul_chunk(0);
        let mut vm = make_vm(vec![top, mul_chunk], vec![StackValue::Float(2.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "map", &[closure], 1).unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert!(items.is_empty());
    }

    #[test]
    fn map_arity_error() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);
        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0]);

        let err = exec_list_hof(&mut vm, list_idx, "map", &[], 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    // ── filter ───────────────────────────────────────────────────────────────

    #[test]
    fn filter_keeps_matching() {
        // Closure: arg0 > 2.0
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(2.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0, 4.0]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "filter", &[closure], 1).unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert_eq!(as_floats(&items), vec![3.0, 4.0]);
    }

    #[test]
    fn filter_empty_list() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(0.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "filter", &[closure], 1).unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert!(items.is_empty());
    }

    // ── any ──────────────────────────────────────────────────────────────────

    #[test]
    fn any_true_when_match_exists() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(2.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "any", &[closure], 1).unwrap();
        assert!(matches!(result, StackValue::Bool(true)));
    }

    #[test]
    fn any_false_when_no_match() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(5.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "any", &[closure], 1).unwrap();
        assert!(matches!(result, StackValue::Bool(false)));
    }

    #[test]
    fn any_empty_is_false() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(0.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "any", &[closure], 1).unwrap();
        assert!(matches!(result, StackValue::Bool(false)));
    }

    // ── all ──────────────────────────────────────────────────────────────────

    #[test]
    fn all_true_when_all_match() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(0.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "all", &[closure], 1).unwrap();
        assert!(matches!(result, StackValue::Bool(true)));
    }

    #[test]
    fn all_false_when_some_fail() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(2.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "all", &[closure], 1).unwrap();
        assert!(matches!(result, StackValue::Bool(false)));
    }

    #[test]
    fn all_empty_is_true() {
        let top = Chunk::new("<top>");
        let gt_chunk = make_gt_chunk(0);
        let mut vm = make_vm(vec![top, gt_chunk], vec![StackValue::Float(0.0)]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[]);
        let closure = alloc_closure(&mut vm, 1);

        let result = exec_list_hof(&mut vm, list_idx, "all", &[closure], 1).unwrap();
        assert!(matches!(result, StackValue::Bool(true)));
    }

    // ── sort (default) ───────────────────────────────────────────────────────

    #[test]
    fn sort_default_floats() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[3.0, 1.0, 2.0]);

        let ret = exec_list_hof(&mut vm, list_idx, "sort", &[], 1).unwrap();
        assert!(matches!(ret, StackValue::Float(f) if f == 0.0));

        let items = read_list(&vm, list_idx);
        assert_eq!(as_floats(&items), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn sort_default_strings() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let s_c = vm.heap.alloc(HeapObject::Str("cherry".into()));
        let s_a = vm.heap.alloc(HeapObject::Str("apple".into()));
        let s_b = vm.heap.alloc(HeapObject::Str("banana".into()));
        let list_sv = vm.heap.alloc(HeapObject::List(vec![s_c, s_a, s_b]));
        let list_idx = list_sv.as_heap_ref().unwrap();

        exec_list_hof(&mut vm, list_idx, "sort", &[], 1).unwrap();

        let items = read_list(&vm, list_idx);
        let names: Vec<String> = items
            .iter()
            .map(|sv| {
                let idx = sv.as_heap_ref().unwrap();
                match vm.heap.get(idx) {
                    HeapObject::Str(s) => s.clone(),
                    _ => panic!("expected str"),
                }
            })
            .collect();
        assert_eq!(names, vec!["apple", "banana", "cherry"]);
    }

    // ── sort (custom) ────────────────────────────────────────────────────────

    #[test]
    fn sort_custom_comparator() {
        // Comparator: arg0 - arg1 (ascending)
        let top = Chunk::new("<top>");
        let cmp_chunk = make_cmp_chunk();
        let mut vm = make_vm(vec![top, cmp_chunk], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[3.0, 1.0, 2.0]);
        let closure = alloc_closure(&mut vm, 1);

        let ret = exec_list_hof(&mut vm, list_idx, "sort", &[closure], 1).unwrap();
        assert!(matches!(ret, StackValue::Float(f) if f == 0.0));

        let items = read_list(&vm, list_idx);
        assert_eq!(as_floats(&items), vec![1.0, 2.0, 3.0]);
    }

    // ── search ───────────────────────────────────────────────────────────────

    #[test]
    fn search_found() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[10.0, 20.0, 30.0]);

        let result =
            exec_list_hof(&mut vm, list_idx, "search", &[StackValue::Float(20.0)], 1).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 1.0));
    }

    #[test]
    fn search_not_found() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[10.0, 20.0, 30.0]);

        let result =
            exec_list_hof(&mut vm, list_idx, "search", &[StackValue::Float(99.0)], 1).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == -1.0));
    }

    // ── bsearch ──────────────────────────────────────────────────────────────

    #[test]
    fn bsearch_found() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[10.0, 20.0, 30.0]);

        let result =
            exec_list_hof(&mut vm, list_idx, "bsearch", &[StackValue::Float(20.0)], 1).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 1.0));
    }

    #[test]
    fn bsearch_not_found() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[10.0, 20.0, 30.0]);

        let result =
            exec_list_hof(&mut vm, list_idx, "bsearch", &[StackValue::Float(15.0)], 1).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == -1.0));
    }

    // ── take ─────────────────────────────────────────────────────────────────

    #[test]
    fn take_subrange() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0, 4.0]);

        let result = exec_list_hof(
            &mut vm,
            list_idx,
            "take",
            &[StackValue::Float(1.0), StackValue::Float(3.0)],
            1,
        )
        .unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert_eq!(as_floats(&items), vec![2.0, 3.0]);
    }

    #[test]
    fn take_clamps_indices() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0]);

        let result = exec_list_hof(
            &mut vm,
            list_idx,
            "take",
            &[StackValue::Float(0.0), StackValue::Float(100.0)],
            1,
        )
        .unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert_eq!(as_floats(&items), vec![1.0, 2.0]);
    }

    // ── drop ─────────────────────────────────────────────────────────────────

    #[test]
    fn drop_removes_range() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0, 4.0]);

        let result = exec_list_hof(
            &mut vm,
            list_idx,
            "drop",
            &[StackValue::Float(1.0), StackValue::Float(3.0)],
            1,
        )
        .unwrap();
        let result_idx = result.as_heap_ref().unwrap();
        let items = read_list(&vm, result_idx);
        assert_eq!(as_floats(&items), vec![1.0, 4.0]);
    }

    // ── cut ──────────────────────────────────────────────────────────────────

    #[test]
    fn cut_removes_and_returns() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0, 4.0]);

        let removed = exec_list_hof(
            &mut vm,
            list_idx,
            "cut",
            &[StackValue::Float(1.0), StackValue::Float(3.0)],
            1,
        )
        .unwrap();

        // Removed elements
        let removed_idx = removed.as_heap_ref().unwrap();
        let removed_items = read_list(&vm, removed_idx);
        assert_eq!(as_floats(&removed_items), vec![2.0, 3.0]);

        // Original list mutated
        let remaining = read_list(&vm, list_idx);
        assert_eq!(as_floats(&remaining), vec![1.0, 4.0]);
    }

    // ── paste ────────────────────────────────────────────────────────────────

    #[test]
    fn paste_inserts_list() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);
        let (to_insert, _) = alloc_float_list(&mut vm, &[10.0, 20.0]);

        let ret = exec_list_hof(
            &mut vm,
            list_idx,
            "paste",
            &[StackValue::Float(1.0), to_insert],
            1,
        )
        .unwrap();
        assert!(matches!(ret, StackValue::Float(f) if f == 0.0));

        let items = read_list(&vm, list_idx);
        assert_eq!(as_floats(&items), vec![1.0, 10.0, 20.0, 2.0, 3.0]);
    }

    #[test]
    fn paste_inserts_single_value() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);

        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0, 2.0, 3.0]);

        exec_list_hof(
            &mut vm,
            list_idx,
            "paste",
            &[StackValue::Float(1.0), StackValue::Float(99.0)],
            1,
        )
        .unwrap();

        let items = read_list(&vm, list_idx);
        assert_eq!(as_floats(&items), vec![1.0, 99.0, 2.0, 3.0]);
    }

    // ── arity errors ─────────────────────────────────────────────────────────

    #[test]
    fn filter_arity_error() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);
        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0]);

        let err = exec_list_hof(&mut vm, list_idx, "filter", &[], 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    #[test]
    fn search_arity_error() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);
        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0]);

        let err = exec_list_hof(&mut vm, list_idx, "search", &[], 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    #[test]
    fn take_arity_error() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);
        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0]);

        let err = exec_list_hof(
            &mut vm,
            list_idx,
            "take",
            &[StackValue::Float(0.0)],
            1,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    #[test]
    fn negative_index_error() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);
        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0]);

        let err = exec_list_hof(
            &mut vm,
            list_idx,
            "take",
            &[StackValue::Float(-1.0), StackValue::Float(1.0)],
            1,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn unknown_method_error() {
        let top = Chunk::new("<top>");
        let mut vm = make_vm(vec![top], vec![]);
        let (_, list_idx) = alloc_float_list(&mut vm, &[1.0]);

        let err = exec_list_hof(&mut vm, list_idx, "nonexistent", &[], 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R004);
    }
}
