use crate::solver::undo::UndoStack;

/// Generic search state for DFS-based solvers.
///
/// `Ctx` is the shared, immutable problem context (e.g. a graph).
/// `Move` represents a single action in the search.
/// `UndoEntry` captures what to reverse when backtracking.
pub trait SearchState: Sized + Clone + Send {
    type Move: Copy + Send;
    type UndoEntry;
    type Ctx: Send + Sync;

    fn gen_moves(&self, ctx: &Self::Ctx, out: &mut Vec<Self::Move>);
    fn apply_move(&mut self, ctx: &Self::Ctx, mv: Self::Move, undo: &mut UndoStack<Self>);
    fn apply_undo(&mut self, entry: Self::UndoEntry);
}
