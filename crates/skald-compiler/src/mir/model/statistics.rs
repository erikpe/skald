//! Stable aggregate shape statistics for executable MIR products.

use super::{MirDefinitionRef, MirProgram, PreliminaryMirProgram};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirProgramStatistics {
    definitions: u64,
    blocks: u64,
    instructions: u64,
}

impl MirProgramStatistics {
    fn from_definitions<'mir>(definitions: impl Iterator<Item = MirDefinitionRef<'mir>>) -> Self {
        let mut statistics = Self::default();
        for definition in definitions {
            statistics.definitions = statistics.definitions.saturating_add(1);
            for block in &definition.body().blocks {
                statistics.blocks = statistics.blocks.saturating_add(1);
                statistics.instructions = statistics
                    .instructions
                    .saturating_add(count(block.instructions.len()));
            }
        }
        statistics
    }

    pub(crate) const fn definitions(self) -> u64 {
        self.definitions
    }

    pub(crate) const fn blocks(self) -> u64 {
        self.blocks
    }

    pub(crate) const fn instructions(self) -> u64 {
        self.instructions
    }
}

impl MirProgram {
    pub(crate) fn reporting_statistics(&self) -> MirProgramStatistics {
        MirProgramStatistics::from_definitions(self.executable_definitions())
    }
}

impl PreliminaryMirProgram {
    pub(crate) fn reporting_statistics(&self) -> MirProgramStatistics {
        MirProgramStatistics::from_definitions(self.executable_definitions())
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use crate::{mir::lower_preliminary_hir, test_support::type_check_source};

    #[test]
    fn statistics_count_every_executable_definition_block_and_instruction() {
        let checked = type_check_source(
            "fn helper() -> i64 { return 1; } fn main() -> i64 { return helper(); }",
        );
        let hir = checked.hir.unwrap();
        let preliminary = lower_preliminary_hir(&hir);
        let statistics = preliminary.reporting_statistics();

        assert_eq!(statistics.definitions(), 2);
        assert_eq!(statistics.blocks(), 2);
        assert!(statistics.instructions() >= 1);
    }
}
