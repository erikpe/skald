//! Callable body sequencing and proof-metadata traversal.

macro_rules! define_body_traversal {
    (($($mir_mutability:tt)*)) => {
        pub(crate) fn map_body_local_identities<M: MirLocalIdentityMapper>(
            body: &$($mir_mutability)* MirBody,
            mapper: &mut M,
        ) -> Result<(), M::Error> {
            let MirBody {
                entry,
                blocks,
                path_conditions,
                logical_expressions,
            } = body;
            map_block(mapper, MirLocalIdentitySite::BodyEntry, entry)?;
            for (block_index, block) in blocks.into_iter().enumerate() {
                let MirBasicBlock {
                    id,
                    instructions,
                    terminator,
                    span: _,
                } = block;
                map_block(
                    mapper,
                    MirLocalIdentitySite::BlockDeclaration(block_index),
                    id,
                )?;
                for (instruction_index, instruction) in instructions.into_iter().enumerate() {
                    map_instruction(
                        instruction,
                        mapper,
                        MirLocalIdentitySite::Instruction {
                            block: block_index,
                            instruction: instruction_index,
                        },
                    )?;
                }
                if let Some(terminator) = terminator {
                    map_terminator(
                        terminator,
                        mapper,
                        MirLocalIdentitySite::Terminator(block_index),
                    )?;
                }
            }
            for (index, condition) in path_conditions.into_iter().enumerate() {
                map_path_condition_metadata(
                    condition,
                    mapper,
                    MirLocalIdentitySite::PathCondition(index),
                )?;
            }
            for (index, expression) in logical_expressions.into_iter().enumerate() {
                map_logical_expression(
                    expression,
                    mapper,
                    MirLocalIdentitySite::LogicalExpression(index),
                )?;
            }
            Ok(())
        }

        pub(crate) fn map_path_condition_metadata<M: MirLocalIdentityMapper>(
            condition: &$($mir_mutability)* MirPathCondition,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirPathCondition {
                id,
                parent,
                activation,
                active_predecessor,
                inactive_predecessor,
                merge,
                span: _,
            } = condition;
            map_path_condition(mapper, site, id)?;
            if let Some(parent) = parent {
                map_path_condition(mapper, site, parent)?;
            }
            map_storage_use(mapper, site, MirStorageUseRole::ProofMetadata, activation)?;
            map_block(mapper, site, active_predecessor)?;
            map_block(mapper, site, inactive_predecessor)?;
            map_block(mapper, site, merge)
        }

        pub(crate) fn map_logical_expression<M: MirLocalIdentityMapper>(
            expression: &$($mir_mutability)* MirLogicalExpression,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirLogicalExpression {
                operation: _,
                condition,
                result,
                left_result,
                split,
                selection,
                right_entry,
                right_exit,
                right_result,
                short,
                join,
                selected_result,
                span: _,
            } = expression;
            map_path_condition(mapper, site, condition)?;
            map_storage_use(mapper, site, MirStorageUseRole::ProofMetadata, result)?;
            map_value_use(mapper, site, MirValueUseRole::ProofMetadata, left_result)?;
            map_block(mapper, site, split)?;
            map_block(mapper, site, selection)?;
            map_block(mapper, site, right_entry)?;
            map_block(mapper, site, right_exit)?;
            map_value_use(mapper, site, MirValueUseRole::ProofMetadata, right_result)?;
            map_block(mapper, site, short)?;
            map_block(mapper, site, join)?;
            map_value_use(
                mapper,
                site,
                MirValueUseRole::ProofMetadata,
                selected_result,
            )
        }
    };
}

pub(super) use define_body_traversal;
