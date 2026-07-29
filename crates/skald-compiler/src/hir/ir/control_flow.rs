//! Composable control outcomes for structured HIR.

use std::collections::BTreeSet;

use crate::identity::LoopId;

/// Possible ways execution can leave a structured HIR operation.
///
/// This is an outcome set rather than a single state: different conditional
/// paths can fall through, exit the function, diverge, or transfer to
/// different enclosing loops.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirControlEffects {
    falls_through: bool,
    exits_function: bool,
    diverges: bool,
    breaks: BTreeSet<LoopId>,
    continues: BTreeSet<LoopId>,
}

impl HirControlEffects {
    pub fn fallthrough() -> Self {
        Self {
            falls_through: true,
            ..Self::default()
        }
    }

    pub fn function_exit() -> Self {
        Self {
            exits_function: true,
            ..Self::default()
        }
    }

    pub fn divergence() -> Self {
        Self {
            diverges: true,
            ..Self::default()
        }
    }

    pub fn break_to(target: LoopId) -> Self {
        Self {
            breaks: BTreeSet::from([target]),
            ..Self::default()
        }
    }

    pub fn continue_to(target: LoopId) -> Self {
        Self {
            continues: BTreeSet::from([target]),
            ..Self::default()
        }
    }

    pub const fn can_fall_through(&self) -> bool {
        self.falls_through
    }

    pub const fn can_exit_function(&self) -> bool {
        self.exits_function
    }

    pub const fn can_diverge(&self) -> bool {
        self.diverges
    }

    pub fn can_break_to(&self, target: LoopId) -> bool {
        self.breaks.contains(&target)
    }

    pub fn can_continue_to(&self, target: LoopId) -> bool {
        self.continues.contains(&target)
    }

    /// Compose a statement sequence.
    ///
    /// Only fallthrough paths from `self` reach `next`; all other outcomes
    /// bypass it and remain outcomes of the complete sequence.
    pub fn then(mut self, next: Self) -> Self {
        if self.falls_through {
            self.falls_through = false;
            self.extend(next);
        }
        self
    }

    /// Combine alternative control-flow paths.
    pub fn union(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }

    /// Summarize a structured loop.
    ///
    /// Transfers targeting this loop are consumed. The condition-false path
    /// makes every loop conservatively fall through, while effects targeting
    /// outer loops and function-level outcomes propagate unchanged.
    pub fn through_loop(mut self, loop_id: LoopId) -> Self {
        self.breaks.remove(&loop_id);
        self.continues.remove(&loop_id);
        self.falls_through = true;
        self
    }

    fn extend(&mut self, other: Self) {
        self.falls_through |= other.falls_through;
        self.exits_function |= other.exits_function;
        self.diverges |= other.diverges;
        self.breaks.extend(other.breaks);
        self.continues.extend(other.continues);
    }
}

#[cfg(test)]
mod tests {
    use crate::identity::FunctionId;

    use super::*;

    #[test]
    fn sequences_only_fallthrough_paths_and_unions_alternatives() {
        let function = FunctionId::new(0);
        let target = LoopId::new(function, 0);
        let sequence = HirControlEffects::fallthrough()
            .union(HirControlEffects::function_exit())
            .then(HirControlEffects::break_to(target));

        assert!(!sequence.can_fall_through());
        assert!(sequence.can_exit_function());
        assert!(sequence.can_break_to(target));

        let unreachable =
            HirControlEffects::divergence().then(HirControlEffects::continue_to(target));
        assert!(unreachable.can_diverge());
        assert!(!unreachable.can_continue_to(target));
    }

    #[test]
    fn loops_consume_only_their_own_targeted_effects() {
        let function = FunctionId::new(0);
        let inner = LoopId::new(function, 0);
        let outer = LoopId::new(function, 1);
        let effects = HirControlEffects::break_to(inner)
            .union(HirControlEffects::continue_to(inner))
            .union(HirControlEffects::break_to(outer))
            .union(HirControlEffects::function_exit())
            .through_loop(inner);

        assert!(effects.can_fall_through());
        assert!(!effects.can_break_to(inner));
        assert!(!effects.can_continue_to(inner));
        assert!(effects.can_break_to(outer));
        assert!(effects.can_exit_function());
    }
}
