//! Ordinary values, calls, lifecycle operations, shared owners, and strings.

macro_rules! define_core_operation_traversal {
    (($($mir_mutability:tt)*)) => {
        fn map_assignment<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirAssignment,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirAssignment {
                result,
                rvalue,
                span: _,
            } = instruction;
            map_value_definition(mapper, site, result)?;
            map_rvalue(rvalue, mapper, site)
        }

        fn map_rvalue<M: MirLocalIdentityMapper>(
            rvalue: &$($mir_mutability)* MirRvalue,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirRvalue { kind, ty: _ } = rvalue;
            match kind {
                MirRvalueKind::ConstantI64(_)
                | MirRvalueKind::ConstantU64(_)
                | MirRvalueKind::ConstantU8(_)
                | MirRvalueKind::ConstantF64Bits(_)
                | MirRvalueKind::ConstantBool(_)
                | MirRvalueKind::CallableAddress(_) => Ok(()),
                MirRvalueKind::PathCondition(condition) => {
                    let MirPathConditionValue {
                        condition,
                        activation,
                    } = condition;
                    map_path_condition(mapper, site, condition)?;
                    map_storage_use(mapper, site, MirStorageUseRole::ProofMetadata, activation)
                }
                MirRvalueKind::Load(place) => {
                    map_place(place, mapper, site, MirPlaceUseContext::OrdinaryRead)
                }
                MirRvalueKind::Unary {
                    operation: _,
                    operand,
                } => map_value_use(
                    mapper,
                    site,
                    MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::UnaryOperand),
                    operand,
                ),
                MirRvalueKind::PrimitiveCast {
                    operation: _,
                    operand,
                } => map_value_use(
                    mapper,
                    site,
                    MirValueUseRole::OrdinaryPrimitiveCast,
                    operand,
                ),
                MirRvalueKind::CheckedF64ToInteger {
                    relation: _,
                    operand,
                } => map_value_use(
                    mapper,
                    site,
                    MirValueUseRole::CheckedProtocol,
                    operand,
                ),
                MirRvalueKind::Binary {
                    operation: _,
                    left,
                    right,
                } => {
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::BinaryLeft),
                        left,
                    )?;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::BinaryRight),
                        right,
                    )
                }
                MirRvalueKind::PrimitiveComparison {
                    operation: _,
                    left,
                    right,
                } => {
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::ComparisonLeft),
                        left,
                    )?;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OrdinaryScalarRvalue(MirScalarValueUse::ComparisonRight),
                        right,
                    )
                }
                MirRvalueKind::IntegerDivision {
                    operation: _,
                    dividend: left,
                    divisor: right,
                }
                | MirRvalueKind::Shift {
                    operation: _,
                    left,
                    count: right,
                } => {
                    map_value_use(mapper, site, MirValueUseRole::CheckedProtocol, left)?;
                    map_value_use(mapper, site, MirValueUseRole::CheckedProtocol, right)
                }
                MirRvalueKind::TypeTest { source, target: _ } => map_object_view(source, mapper, site),
                MirRvalueKind::OptionalPresence { source, kind: _ } => map_place(
                    source,
                    mapper,
                    site,
                    MirPlaceUseContext::OtherExecutable,
                ),
                MirRvalueKind::OptionalBoxPresence {
                    owner,
                    target: _,
                    layer: _,
                    kind: _,
                } => map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, owner),
                MirRvalueKind::ArrayLength { source, array: _ } => map_place(
                    source,
                    mapper,
                    site,
                    MirPlaceUseContext::OtherExecutable,
                ),
            }
        }

        fn map_call<M: MirLocalIdentityMapper>(
            call: &$($mir_mutability)* MirCall,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirCall {
                target,
                receiver,
                arguments,
                result,
                shared_result,
                destination,
                span: _,
            } = call;
            match target {
                MirCallTarget::Direct(_)
                | MirCallTarget::Static(_)
                | MirCallTarget::Method(_)
                | MirCallTarget::Interface(_) => {}
                MirCallTarget::Indirect(target) => {
                    let MirIndirectCallTarget {
                        callee,
                        function_type: _,
                    } = target;
                    map_value_use(
                        mapper,
                        site,
                        MirValueUseRole::OrdinaryCall(MirCallValueUse::Target),
                        callee,
                    )?;
                }
            }
            if let Some(receiver) = receiver {
                match receiver {
                    MirCallReceiver::Method(receiver) => map_method_receiver(receiver, mapper, site)?,
                    MirCallReceiver::Interface(view) => map_object_view(view, mapper, site)?,
                }
            }
            for (index, argument) in arguments.into_iter().enumerate() {
                map_argument(argument, mapper, site, ArgumentUseContext::OrdinaryCall(index))?;
            }
            if let Some(result) = result {
                map_value_definition(mapper, site, result)?;
            }
            map_optional_storage(mapper, site, MirStorageUseRole::Call, shared_result)?;
            if let Some(destination) = destination {
                map_place(destination, mapper, site, MirPlaceUseContext::Call)?;
            }
            Ok(())
        }

        fn map_argument<M: MirLocalIdentityMapper>(
            argument: &$($mir_mutability)* MirArgument,
            mapper: &mut M,
            site: MirLocalIdentitySite,
            context: ArgumentUseContext,
        ) -> Result<(), M::Error> {
            match argument {
                MirArgument::Value(value) => map_value_use(mapper, site, context.role(), value),
                MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
                    map_place(place, mapper, site, context.place_context())
                }
                MirArgument::View(view) => map_object_view(view, mapper, site),
                MirArgument::SharedOwner(owner) => {
                    map_storage_use(mapper, site, context.storage_role(), owner)
                }
            }
        }

        #[derive(Clone, Copy)]
        enum ArgumentUseContext {
            OrdinaryCall(usize),
            OwnershipOrLifecycle,
        }

        impl ArgumentUseContext {
            const fn role(self) -> MirValueUseRole {
                match self {
                    Self::OrdinaryCall(index) => {
                        MirValueUseRole::OrdinaryCall(MirCallValueUse::Argument(index))
                    }
                    Self::OwnershipOrLifecycle => MirValueUseRole::OwnershipOrLifecycle,
                }
            }

            const fn place_context(self) -> MirPlaceUseContext {
                match self {
                    Self::OrdinaryCall(_) => MirPlaceUseContext::Call,
                    Self::OwnershipOrLifecycle => MirPlaceUseContext::OwnershipOrLifecycle,
                }
            }

            const fn storage_role(self) -> MirStorageUseRole {
                match self {
                    Self::OrdinaryCall(_) => MirStorageUseRole::Call,
                    Self::OwnershipOrLifecycle => MirStorageUseRole::OwnershipOrLifecycle,
                }
            }
        }

        fn map_cleanup<M: MirLocalIdentityMapper>(
            cleanup: &$($mir_mutability)* MirCleanup,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirCleanup {
                destination,
                target: _,
                span: _,
            } = cleanup;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )
        }

        fn map_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirInitialize {
                destination,
                target: _,
                arguments,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            for argument in arguments {
                map_argument(
                    argument,
                    mapper,
                    site,
                    ArgumentUseContext::OwnershipOrLifecycle,
                )?;
            }
            Ok(())
        }

        fn map_store<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirStore,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirStore {
                destination,
                value,
                authorization,
                final_authorization,
                span: _,
            } = instruction;
            let authorization = match (authorization.is_some(), final_authorization.is_some()) {
                (false, false) => MirStorageWriteAuthorization::None,
                (true, false) => MirStorageWriteAuthorization::Cell,
                (false, true) => MirStorageWriteAuthorization::Final,
                (true, true) => MirStorageWriteAuthorization::CellAndFinal,
            };
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OrdinaryWrite(authorization),
            )?;
            map_value_use(mapper, site, MirValueUseRole::OrdinaryStore, value)
        }

        fn map_copy_construction<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirCopyConstruction,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirCopyConstruction {
                destination,
                source,
                class: _,
                operation: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_place(
                source,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )
        }

        fn map_copy_assignment<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirCopyAssignment,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirCopyAssignment {
                destination,
                source,
                class: _,
                operation: _,
                authorization: _,
                final_authorization: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_place(
                source,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )
        }

        fn map_checked_view_binding<M: MirLocalIdentityMapper>(
            binding: &$($mir_mutability)* MirCheckedViewBinding,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirCheckedViewBinding {
                destination,
                view,
                span: _,
            } = binding;
            map_storage_use(mapper, site, MirStorageUseRole::Alias, destination)?;
            map_object_view(view, mapper, site)
        }

        fn map_shared_allocate<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirSharedAllocate,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirSharedAllocate {
                allocation,
                target: _,
                origin: _,
                mode,
                span: _,
            } = instruction;
            map_storage_use(
                mapper,
                site,
                MirStorageUseRole::OwnershipOrLifecycle,
                allocation,
            )?;
            match mode {
                MirSharedAllocationMode::Initialize => Ok(()),
                MirSharedAllocationMode::Copy { source } => map_place(
                    source,
                    mapper,
                    site,
                    MirPlaceUseContext::OwnershipOrLifecycle,
                ),
                MirSharedAllocationMode::OptionalBox { completion: _ } => Ok(()),
            }
        }

        fn map_shared_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirSharedInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirSharedInitialize {
                allocation,
                target: _,
                arguments,
                span: _,
            } = instruction;
            map_storage_use(
                mapper,
                site,
                MirStorageUseRole::OwnershipOrLifecycle,
                allocation,
            )?;
            for argument in arguments {
                map_argument(
                    argument,
                    mapper,
                    site,
                    ArgumentUseContext::OwnershipOrLifecycle,
                )?;
            }
            Ok(())
        }

        fn map_shared_cast<M: MirLocalIdentityMapper>(
            cast: &$($mir_mutability)* MirSharedCast,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirSharedCast {
                destination,
                source,
                target: _,
                transfer: _,
                exact_dynamic_class: _,
                span: _,
            } = cast;
            map_storage_use(
                mapper,
                site,
                MirStorageUseRole::OwnershipOrLifecycle,
                destination,
            )?;
            match source {
                MirSharedCastSource::Owner { storage, target: _ } => map_storage_use(
                    mapper,
                    site,
                    MirStorageUseRole::OwnershipOrLifecycle,
                    storage,
                ),
                MirSharedCastSource::Field { place, target: _ } => map_place(
                    place,
                    mapper,
                    site,
                    MirPlaceUseContext::OwnershipOrLifecycle,
                ),
            }
        }

        fn map_string_initialize<M: MirLocalIdentityMapper>(
            instruction: &$($mir_mutability)* MirStringInitialize,
            mapper: &mut M,
            site: MirLocalIdentitySite,
        ) -> Result<(), M::Error> {
            let MirStringInitialize {
                destination,
                data: _,
                backing,
                class: _,
                storage_field: _,
                start_field: _,
                length_field: _,
                hash_code_field: _,
                start: _,
                length: _,
                span: _,
            } = instruction;
            map_place(
                destination,
                mapper,
                site,
                MirPlaceUseContext::OwnershipOrLifecycle,
            )?;
            map_storage_use(mapper, site, MirStorageUseRole::OwnershipOrLifecycle, backing)
        }

    };
}

pub(in crate::mir::rewrite::map) use define_core_operation_traversal;
