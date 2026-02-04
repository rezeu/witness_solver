use std::fmt::Display;

use crate::undo::UndoStack;
use crate::problem::graph::Graph;
pub trait SearchState: Sized{
    type Move:Display + Clone+ Copy;
    type UndoEntry: Display;

    fn gen_moves(&self,g: &Graph, out : &mut Vec<Self::Move>);
    fn apply_move(&mut self, g: &Graph, mv : Self::Move, undost : &mut UndoStack<Self>);
    fn apply_undo(&mut self, entry : Self::UndoEntry);
    fn draw(&self, g: &Graph);
}
