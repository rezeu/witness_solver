use crate::solver::state::SearchState;
use smallvec::SmallVec;

const INLINE_CAP: usize = 64;

pub struct UndoStack<S: SearchState> {
    stack: SmallVec<[S::UndoEntry; INLINE_CAP]>,
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

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    #[inline]
    pub fn push(&mut self, entry: S::UndoEntry) {
        self.stack.push(entry);
    }

    #[inline]
    pub fn mark(&mut self) {
        self.marks.push(self.len());
    }

    pub fn rollback(&mut self, state: &mut S) {
        if let Some(target) = self.marks.pop() {
            while self.len() > target {
                let e = self.stack.pop().expect("undo stack underflow");
                state.apply_undo(e);
            }
        }
    }
}

impl<S: SearchState> Default for UndoStack<S> {
    fn default() -> Self {
        Self::new()
    }
}
