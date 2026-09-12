//! Executable exits, checked protocols, and ordered control-flow targets.

macro_rules! define_terminator_traversal {
    (($($mir_mutability:tt)*)) => {
        pub(crate) fn map_terminator<M: MirLocalIdentityMapper>(
            terminator: &$($mir_mutability)* MirTerminator,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            match terminator {
                MirTerminator::Return { value, span: _ } => {
                    if let Some(value) = value {
                        map_value_use(mapper, site, MirValueUseRole::OrdinaryReturn, value)?;
                    }
                    Ok(())
                }
                MirTerminator::ReturnShared { owner, span: _ }
                | MirTerminator::ReturnOptionalShared { owner, span: _ } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, owner)
                }
                MirTerminator::Panic { message, span: _ } => map_place(
                    message,
                    mapper,
                    site,
                    MirPlaceUseContext::OtherExecutable,
                ),
                MirTerminator::Goto { target, span: _ } => map_block(mapper, site, target),
                MirTerminator::Branch {
                    condition,
                    true_target,
                    false_target,
                    span: _,
                } => {
                    map_value_use(mapper, site, MirValueUseRole::OrdinaryBranch, condition)?;
                    map_block(mapper, site, true_target)?;
                    map_block(mapper, site, false_target)
                }
                MirTerminator::ShiftCountCheck {
                    check,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    let MirShiftCountCheck {
                        operation: _,
                        left,
                        count,
                        result,
                    } = check;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, left)?;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, count)?;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, result)?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::IntegerDivisorCheck {
                    check,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    let MirIntegerDivisorCheck {
                        operation: _,
                        dividend,
                        divisor,
                        result,
                    } = check;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, dividend)?;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, divisor)?;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, result)?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::PrimitiveCastRangeCheck {
                    check,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    let MirPrimitiveCastRangeCheck {
                        relation: _,
                        source,
                        result,
                    } = check;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, source)?;
                    map_storage_use(mapper, site, MirStorageUseRole::CheckedProtocol, result)?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::CheckedCast {
                    binding,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    map_checked_view_binding(binding, mapper, site)?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::SharedCast {
                    cast,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    map_shared_cast(cast, mapper, site)?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::OptionalUnwrap {
                    source,
                    destination,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::OptionalSharedUnwrap {
                    unwrap,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    let MirOptionalSharedUnwrap {
                        optional: _,
                        source,
                        destination,
                        target: _,
                        span: _,
                    } = unwrap;
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OwnershipOrLifecycle,
                    )?;
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        destination,
                    )?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::BeginOptionalView {
                    begin,
                    success_target,
                    absent_target,
                    overflow_target,
                    span: _,
                } => {
                    map_optional_view_begin(begin, mapper, site)?;
                    map_block_triple(mapper, site, success_target, absent_target, overflow_target)
                }
                MirTerminator::BeginOptionalBoxView {
                    begin,
                    success_target,
                    absent_target,
                    overflow_target,
                    span: _,
                } => {
                    map_optional_box_view_begin(begin, mapper, site)?;
                    map_block_triple(mapper, site, success_target, absent_target, overflow_target)
                }
                MirTerminator::CheckOptionalMutation {
                    source,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    map_place(
                        source,
                        mapper,
                        site,
                        MirPlaceUseContext::OtherExecutable,
                    )?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::ArrayPositionCheck {
                    position,
                    kind: _,
                    success_target,
                    failure_target,
                    span: _,
                } => {
                    map_storage_use(mapper, site, MirStorageUseRole::OtherExecutable, position)?;
                    map_block_pair(mapper, site, success_target, failure_target)
                }
                MirTerminator::ArrayOperationCheck {
                    failure: _,
                    success_target,
                    failure_target,
                    span: _,
                } => map_block_pair(mapper, site, success_target, failure_target),
                MirTerminator::ArrayLoop {
                    backing,
                    index,
                    length,
                    kind,
                    body_target,
                    complete_target,
                    span: _,
                } => {
                    map_storage_use(
                        mapper,
                        site,
                        MirStorageUseRole::OwnershipOrLifecycle,
                        backing,
                    )?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, index)?;
                    map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, length)?;
                    if let crate::mir::MirArrayLoopKind::Indexed { binding } = kind {
                        map_storage_use(mapper, site, MirStorageUseRole::OtherExecutable, binding)?;
                    }
                    map_block_pair(mapper, site, body_target, complete_target)
                }
                MirTerminator::Terminate { reason: _, span: _ } => Ok(()),
            }
        }

        fn map_block_pair<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            first: &$($mir_mutability)* BlockId,
            second: &$($mir_mutability)* BlockId,
        ) -> Result<(), M::Error> {
            map_block(mapper, site, first)?;
            map_block(mapper, site, second)
        }

        fn map_block_triple<M: MirLocalIdentityMapper>(
            mapper: &mut M,
            site: MirLocalIdentitySite,
            first: &$($mir_mutability)* BlockId,
            second: &$($mir_mutability)* BlockId,
            third: &$($mir_mutability)* BlockId,
        ) -> Result<(), M::Error> {
            map_block(mapper, site, first)?;
            map_block(mapper, site, second)?;
            map_block(mapper, site, third)
        }

    };
}

pub(super) use define_terminator_traversal;
