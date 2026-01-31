use crate::state::SearchState;

pub trait Pruner<S: SearchState> {
    fn should_prune(&self, _s: &S) -> bool {
        false
    }
}

pub struct PrunerChain<S: SearchState> {
    rules: Vec<Box<dyn Pruner<S>>>,
}

impl<S: SearchState> PrunerChain<S> {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add(mut self, p: Box<dyn Pruner<S>>) -> Self {
        self.rules.push(p);
        self
    }
}

impl<S: SearchState> Pruner<S> for PrunerChain<S> {
    fn should_prune(&self, s: &S) -> bool {
        self.rules.iter().any(|p| p.should_prune(s))
    }
}
