use std::sync::atomic::{AtomicU64, Ordering};

use crate::solver::state::SearchState;

pub trait Pruner<S: SearchState>: Send + Sync {
    fn should_prune(&self, s: &S, ctx: &S::Ctx) -> bool;

    fn hit_stats(&self) -> Vec<PrunerHitStats> {
        Vec::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PrunerHitStats {
    pub name: &'static str,
    pub hits: u64,
}

struct PrunerEntry<S: SearchState> {
    name: &'static str,
    hits: AtomicU64,
    rule: Box<dyn Pruner<S>>,
}

/// Compose multiple pruners — prunes if ANY rule says to prune (short-circuit OR).
pub struct PrunerChain<S: SearchState> {
    rules: Vec<PrunerEntry<S>>,
}

impl<S: SearchState> PrunerChain<S> {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn with(mut self, p: Box<dyn Pruner<S>>) -> Self {
        self.rules.push(PrunerEntry {
            name: "unnamed",
            hits: AtomicU64::new(0),
            rule: p,
        });
        self
    }

    pub fn with_named(mut self, name: &'static str, p: Box<dyn Pruner<S>>) -> Self {
        self.rules.push(PrunerEntry {
            name,
            hits: AtomicU64::new(0),
            rule: p,
        });
        self
    }

    fn collect_hit_stats(&self) -> Vec<PrunerHitStats> {
        self.rules
            .iter()
            .map(|entry| PrunerHitStats {
                name: entry.name,
                hits: entry.hits.load(Ordering::Relaxed),
            })
            .collect()
    }
}

impl<S: SearchState> Default for PrunerChain<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: SearchState> Pruner<S> for PrunerChain<S> {
    fn should_prune(&self, s: &S, ctx: &S::Ctx) -> bool {
        for entry in &self.rules {
            if entry.rule.should_prune(s, ctx) {
                entry.hits.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    fn hit_stats(&self) -> Vec<PrunerHitStats> {
        self.collect_hit_stats()
    }
}

/// No-op pruner — never prunes.
impl<S: SearchState> Pruner<S> for () {
    fn should_prune(&self, _s: &S, _ctx: &S::Ctx) -> bool {
        false
    }
}
