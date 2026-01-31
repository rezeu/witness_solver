use crate::pruner::{Pruner,PrunerChain};
use crate::problem::witness_state::WitnessState;
use crate::problem::graph::Graph;
use std::sync::Arc;

pub struct WitnessPruner;

impl WitnessPruner {
    pub fn build(g:Arc<Graph>) -> PrunerChain<WitnessState> {
        PrunerChain::new()
    }
}
