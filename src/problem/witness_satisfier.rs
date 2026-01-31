use crate::problem::graph::Graph;
use crate::problem::witness_state::WitnessState;
use crate::satisfier::{Satisfier, SatisfierChain};
use std::sync::Arc;

pub struct WitnessSatisfier;

impl WitnessSatisfier {
    pub fn build(g: Arc<Graph>) -> SatisfierChain<WitnessState> {
        SatisfierChain::new().add(Box::new(LineRule { graph: g.clone() }))
    }
}

pub struct LineRule {
    pub graph: Arc<Graph>,
}

impl Satisfier<WitnessState> for LineRule {
    fn is_satisfied(&self, s: &WitnessState) -> bool {
        if s.head == self.graph.end
            && s.degrees.iter().enumerate().all(|(idx, &d)| {
                ((idx == self.graph.start || idx == self.graph.end) && d == 1) || (d % 2 == 0)
            })
        {
            return true;
        }
        false
    }
}
