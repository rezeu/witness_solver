use crate::state::SearchState;
use smallvec::SmallVec;
const UNDO_STACK_INLINE_CAPACITY: usize = 64;

pub struct UndoStack<S: SearchState> {
    stack: SmallVec<[S::UndoEntry; UNDO_STACK_INLINE_CAPACITY]>,
    marks: Vec<usize>,
}

impl<S: SearchState> UndoStack<S> {
    pub fn new() -> Self {
        UndoStack {
            stack: SmallVec::new(),
            marks: Vec::new(),
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, entry: S::UndoEntry) {
        self.stack.push(entry);
    }

    pub fn pop(&mut self) -> Option<S::UndoEntry> {
        self.stack.pop()
    }

    pub fn mark(&mut self) {
        self.marks.push(self.len());
    }

    pub fn rollback(&mut self, state: &mut S) {
        if let Some(target_len) = self.marks.pop() {
            while self.len() > target_len {
                let e = self.pop().unwrap_or_else(|| panic!("Undo stack underflow"));
                state.apply_undo(e);
            }
        }
    }
    pub fn draw(&self) {
        println!("Undo Stack (size={}):", self.len());
        for (i, entry) in self.stack.iter().enumerate() {
            println!("  [{}]: {}", i, entry);
        }
    }
}

