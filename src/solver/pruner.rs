use crate::solver::state::SearchState;

pub trait Pruner<S: SearchState>: Send + Sync {
    fn should_prune(&self, s: &S, ctx: &S::Ctx) -> bool;
}

/// Compose multiple pruners — prunes if ANY rule says to prune (short-circuit OR).
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
    fn should_prune(&self, s: &S, ctx: &S::Ctx) -> bool {
        self.rules.iter().any(|p| p.should_prune(s, ctx))
    }
}

/// No-op pruner — never prunes.
impl<S: SearchState> Pruner<S> for () {
    fn should_prune(&self, _s: &S, _ctx: &S::Ctx) -> bool {
        false
    }
}
