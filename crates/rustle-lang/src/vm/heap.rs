use super::value::{HeapObject, StackValue};

/// Bit flag: indices with this bit set live in the frame arena.
const FRAME_BIT: u32 = 0x4000_0000;

/// Two-arena heap for the Rustle VM.
///
/// - **Persistent arena**: objects from `on_init`, globals, constants, and anything
///   promoted from the frame arena via state/list mutation.  Survives across ticks.
/// - **Frame arena**: objects allocated during `on_update`.  Bulk-reset at the start
///   of each tick (zero per-object cost).
///
/// Index encoding: persistent indices are `0..persistent.len()`.  Frame indices
/// have `FRAME_BIT` set: the raw frame offset is `idx & !FRAME_BIT`.
#[derive(Debug)]
pub struct Heap {
    persistent: Vec<HeapObject>,
    frame: Vec<HeapObject>,
    in_frame_mode: bool,
}

impl Heap {
    #[must_use]
    pub fn new() -> Self {
        Self {
            persistent: Vec::new(),
            frame: Vec::new(),
            in_frame_mode: false,
        }
    }

    pub fn enter_frame_mode(&mut self) {
        self.in_frame_mode = true;
    }

    pub fn exit_frame_mode(&mut self) {
        self.in_frame_mode = false;
    }

    pub fn reset_frame(&mut self) {
        self.frame.clear();
    }

    pub fn alloc(&mut self, obj: HeapObject) -> StackValue {
        if self.in_frame_mode {
            let idx = self.frame.len() as u32 | FRAME_BIT;
            self.frame.push(obj);
            StackValue::HeapRef(idx)
        } else {
            let idx = self.persistent.len() as u32;
            self.persistent.push(obj);
            StackValue::HeapRef(idx)
        }
    }

    #[must_use]
    pub fn get(&self, idx: u32) -> &HeapObject {
        if idx & FRAME_BIT != 0 {
            &self.frame[(idx & !FRAME_BIT) as usize]
        } else {
            &self.persistent[idx as usize]
        }
    }

    pub fn get_mut(&mut self, idx: u32) -> &mut HeapObject {
        if idx & FRAME_BIT != 0 {
            &mut self.frame[(idx & !FRAME_BIT) as usize]
        } else {
            &mut self.persistent[idx as usize]
        }
    }

    pub fn read_alloc<F>(&mut self, idx: u32, f: F) -> StackValue
    where
        F: FnOnce(&HeapObject) -> HeapObject,
    {
        let new_obj = if idx & FRAME_BIT != 0 {
            f(&self.frame[(idx & !FRAME_BIT) as usize])
        } else {
            f(&self.persistent[idx as usize])
        };
        self.alloc(new_obj)
    }

    pub fn read_then<F, E>(&mut self, idx: u32, f: F) -> Result<StackValue, E>
    where
        F: FnOnce(&HeapObject) -> Result<HeapObject, E>,
    {
        let new_obj = if idx & FRAME_BIT != 0 {
            f(&self.frame[(idx & !FRAME_BIT) as usize])?
        } else {
            f(&self.persistent[idx as usize])?
        };
        Ok(self.alloc(new_obj))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.persistent.len() + self.frame.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.persistent.is_empty() && self.frame.is_empty()
    }

    pub fn truncate(&mut self, watermark: usize) {
        self.persistent.truncate(watermark);
    }

    /// Returns true if `idx` points into the frame arena.
    #[must_use]
    pub fn is_frame_ref(idx: u32) -> bool {
        idx & FRAME_BIT != 0
    }

    /// Promote a frame-arena object (and its entire reachable graph) to the
    /// persistent arena.  Returns the new persistent index.
    ///
    /// If `sv` is not a frame HeapRef, returns it unchanged.
    pub fn promote(&mut self, sv: StackValue) -> StackValue {
        match sv {
            StackValue::HeapRef(idx) if Self::is_frame_ref(idx) => {
                let new_idx = self.promote_recursive(idx);
                StackValue::HeapRef(new_idx)
            }
            other => other,
        }
    }

    fn promote_recursive(&mut self, frame_idx: u32) -> u32 {
        let raw = (frame_idx & !FRAME_BIT) as usize;
        let obj = self.frame[raw].clone();
        let promoted = self.promote_object(obj);
        let new_idx = self.persistent.len() as u32;
        self.persistent.push(promoted);
        new_idx
    }

    fn promote_object(&mut self, obj: HeapObject) -> HeapObject {
        match obj {
            HeapObject::List(items) => {
                let promoted: Vec<StackValue> = items.into_iter()
                    .map(|sv| self.promote(sv))
                    .collect();
                HeapObject::List(promoted)
            }
            HeapObject::State(s) => {
                let promoted: Vec<StackValue> = s.fields.into_iter()
                    .map(|sv| self.promote(sv))
                    .collect();
                HeapObject::State(super::value::StateObj { fields: promoted })
            }
            HeapObject::Object(mut o) => {
                o.fields = o.fields.into_iter()
                    .map(|sv| self.promote(sv))
                    .collect();
                HeapObject::Object(o)
            }
            HeapObject::EnumVariant(mut e) => {
                e.field_values = e.field_values.into_iter()
                    .map(|sv| self.promote(sv))
                    .collect();
                HeapObject::EnumVariant(e)
            }
            HeapObject::Closure(mut c) => {
                c.upvalues = c.upvalues.into_iter()
                    .map(|sv| self.promote(sv))
                    .collect();
                HeapObject::Closure(c)
            }
            HeapObject::ResOk(inner) => HeapObject::ResOk(self.promote(inner)),
            other => other,
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::value::HeapObject;

    #[test]
    fn new_heap_is_empty() {
        let h = Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn persistent_alloc_and_retrieve() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::Str("hello".into()));
        assert_eq!(h.len(), 1);
        if let StackValue::HeapRef(idx) = r {
            assert!(!Heap::is_frame_ref(idx));
            match h.get(idx) {
                HeapObject::Str(s) => assert_eq!(s, "hello"),
                other => panic!("unexpected variant: {other:?}"),
            }
        } else {
            panic!("expected HeapRef");
        }
    }

    #[test]
    fn frame_alloc_and_retrieve() {
        let mut h = Heap::new();
        h.enter_frame_mode();
        let r = h.alloc(HeapObject::Str("frame".into()));
        if let StackValue::HeapRef(idx) = r {
            assert!(Heap::is_frame_ref(idx));
            match h.get(idx) {
                HeapObject::Str(s) => assert_eq!(s, "frame"),
                other => panic!("unexpected: {other:?}"),
            }
        } else {
            panic!("expected HeapRef");
        }
    }

    #[test]
    fn frame_reset_clears_frame_only() {
        let mut h = Heap::new();
        h.alloc(HeapObject::Str("persistent".into()));
        h.enter_frame_mode();
        h.alloc(HeapObject::Str("temp".into()));
        assert_eq!(h.len(), 2);
        h.reset_frame();
        assert_eq!(h.len(), 1);
        // persistent object still accessible
        assert!(matches!(h.get(0), HeapObject::Str(s) if s == "persistent"));
    }

    #[test]
    fn promote_moves_to_persistent() {
        let mut h = Heap::new();
        h.enter_frame_mode();
        let sv = h.alloc(HeapObject::Str("promote_me".into()));
        let idx = if let StackValue::HeapRef(i) = sv { i } else { panic!() };
        assert!(Heap::is_frame_ref(idx));

        let promoted = h.promote(sv);
        let new_idx = if let StackValue::HeapRef(i) = promoted { i } else { panic!() };
        assert!(!Heap::is_frame_ref(new_idx));
        assert!(matches!(h.get(new_idx), HeapObject::Str(s) if s == "promote_me"));
    }

    #[test]
    fn promote_deep_list() {
        let mut h = Heap::new();
        h.enter_frame_mode();
        let inner = h.alloc(HeapObject::Str("inside".into()));
        let list_sv = h.alloc(HeapObject::List(vec![inner, StackValue::Float(1.0)]));

        let promoted = h.promote(list_sv);
        let idx = if let StackValue::HeapRef(i) = promoted { i } else { panic!() };
        assert!(!Heap::is_frame_ref(idx));
        match h.get(idx) {
            HeapObject::List(items) => {
                assert_eq!(items.len(), 2);
                if let StackValue::HeapRef(str_idx) = items[0] {
                    assert!(!Heap::is_frame_ref(str_idx));
                    assert!(matches!(h.get(str_idx), HeapObject::Str(s) if s == "inside"));
                } else {
                    panic!("expected HeapRef in promoted list");
                }
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn promote_non_frame_is_noop() {
        let mut h = Heap::new();
        let sv = h.alloc(HeapObject::Str("already_persistent".into()));
        let promoted = h.promote(sv);
        assert_eq!(
            sv.as_heap_ref().unwrap(),
            promoted.as_heap_ref().unwrap(),
        );
    }

    #[test]
    fn mutate_via_get_mut() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::Str("old".into()));
        if let StackValue::HeapRef(idx) = r {
            *h.get_mut(idx) = HeapObject::Str("new".into());
            match h.get(idx) {
                HeapObject::Str(s) => assert_eq!(s, "new"),
                other => panic!("unexpected: {other:?}"),
            }
        } else {
            panic!("expected HeapRef");
        }
    }

    #[test]
    fn default_is_empty() {
        let h = Heap::default();
        assert!(h.is_empty());
    }
}
