use crate::solver::state::SearchState;

pub trait Satisfier<S: SearchState>: Send + Sync {
    fn is_satisfied(&self, s: &S, ctx: &S::Ctx) -> bool;
}

/// Compose multiple satisfiers — satisfied only if ALL agree (short-circuit AND).
pub struct SatisfierChain<S: SearchState> {
    rules: Vec<Box<dyn Satisfier<S>>>,
}

impl<S: SearchState> SatisfierChain<S> {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add(mut self, p: Box<dyn Satisfier<S>>) -> Self {
        self.rules.push(p);
        self
    }
}

impl<S: SearchState> Satisfier<S> for SatisfierChain<S> {
    fn is_satisfied(&self, s: &S, ctx: &S::Ctx) -> bool {
        self.rules.iter().all(|p| p.is_satisfied(s, ctx))
    }
}

/// No-op satisfier — always satisfied (use as default when no rules needed).
impl<S: SearchState> Satisfier<S> for () {
    fn is_satisfied(&self, _s: &S, _ctx: &S::Ctx) -> bool {
        true
    }
}
