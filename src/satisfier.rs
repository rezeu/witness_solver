use crate::state::SearchState;

pub trait Satisfier<S: SearchState> {
    fn is_satisfied(&self, _s: &S) -> bool {
        false
    }

}

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
    fn is_satisfied(&self, s: &S) -> bool {
        self.rules.iter().any(|p| p.is_satisfied(s))
    }
}



