use super::value::{HeapObject, StackValue};

/// GC-free arena heap. Objects are never freed during execution; the entire heap
/// is dropped when the VM is done.
pub struct Heap {
    objects: Vec<HeapObject>,
}

impl Heap {
    /// Create an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    /// Allocate a heap object and return a `StackValue::HeapRef` pointing to it.
    pub fn alloc(&mut self, obj: HeapObject) -> StackValue {
        let idx = self.objects.len() as u32;
        self.objects.push(obj);
        StackValue::HeapRef(idx)
    }

    /// Borrow an object by its heap index.
    ///
    /// # Panics
    /// Panics if the index is out of bounds.
    #[must_use]
    pub fn get(&self, idx: u32) -> &HeapObject {
        &self.objects[idx as usize]
    }

    /// Mutably borrow an object by its heap index.
    ///
    /// # Panics
    /// Panics if the index is out of bounds.
    pub fn get_mut(&mut self, idx: u32) -> &mut HeapObject {
        &mut self.objects[idx as usize]
    }

    /// Total number of objects allocated.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// True if no objects have been allocated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
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
    fn alloc_and_retrieve() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::Str("hello".into()));
        assert_eq!(h.len(), 1);
        assert!(!h.is_empty());

        if let StackValue::HeapRef(idx) = r {
            match h.get(idx) {
                HeapObject::Str(s) => assert_eq!(s, "hello"),
                other => panic!("unexpected variant: {:?}", other),
            }
        } else {
            panic!("expected HeapRef");
        }
    }

    #[test]
    fn mutate_via_get_mut() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::Str("old".into()));
        if let StackValue::HeapRef(idx) = r {
            *h.get_mut(idx) = HeapObject::Str("new".into());
            match h.get(idx) {
                HeapObject::Str(s) => assert_eq!(s, "new"),
                other => panic!("unexpected: {:?}", other),
            }
        } else {
            panic!("expected HeapRef");
        }
    }

    #[test]
    fn multiple_allocations_have_correct_indices() {
        let mut h = Heap::new();
        let r0 = h.alloc(HeapObject::Str("zero".into()));
        let r1 = h.alloc(HeapObject::Str("one".into()));
        let r2 = h.alloc(HeapObject::Vec2(1.0, 2.0));

        assert_eq!(h.len(), 3);

        let idx0 = if let StackValue::HeapRef(i) = r0 { i } else { panic!() };
        let idx1 = if let StackValue::HeapRef(i) = r1 { i } else { panic!() };
        let idx2 = if let StackValue::HeapRef(i) = r2 { i } else { panic!() };

        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(idx2, 2);

        assert!(matches!(h.get(idx0), HeapObject::Str(s) if s == "zero"));
        assert!(matches!(h.get(idx1), HeapObject::Str(s) if s == "one"));
        assert!(matches!(h.get(idx2), HeapObject::Vec2(1.0, 2.0)));
    }

    #[test]
    fn default_is_empty() {
        let h = Heap::default();
        assert!(h.is_empty());
    }
}
