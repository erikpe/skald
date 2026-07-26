//! Path-sensitive optional payload guards and pinned-mutation permits.

use std::collections::{BTreeMap, HashSet, VecDeque};

use super::super::{
    super::model::{
        MirBasicBlock, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, MirTerminator,
        OptionalGuardId,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_optional_guards(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        if function.body().entry.index() >= function.body().blocks.len() {
            return;
        }
        let mut incoming = vec![None; function.body().blocks.len()];
        incoming[function.body().entry.index()] = Some(OptionalGuardState::default());
        let mut pending = VecDeque::from([function.body().entry]);
        let mut reported_joins = HashSet::new();

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            let Some(mut state) = incoming[block_id.index()].clone() else {
                continue;
            };
            for instruction in &block.instructions {
                self.verify_guarded_payload_instruction(function, block, instruction, &state);
                if !state.mutation_permits.is_empty()
                    && !matches!(
                        instruction,
                        MirInstruction::ClassOptionalAssign(_)
                            | MirInstruction::ClassOptionalCleanup(_)
                    )
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional mutation check is not immediately followed by its transition",
                    );
                    state.mutation_permits.clear();
                }
                match instruction {
                    MirInstruction::EndOptionalView(end) => {
                        let expected = (end.source.clone(), end.class);
                        let ordered = state.order.last() == Some(&end.guard);
                        if !ordered || state.active.remove(&end.guard) != Some(expected) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "optional view must end its matching active guard in reverse begin order",
                            );
                        }
                        if ordered {
                            state.order.pop();
                        }
                    }
                    MirInstruction::ClassOptionalAssign(assignment) => {
                        let self_copy = matches!(
                            &assignment.source,
                            crate::mir::MirClassOptionalSource::Copy(source)
                                if source == &assignment.destination
                        );
                        let array_element =
                            assignment.destination.projections.iter().any(|projection| {
                                matches!(
                                    projection,
                                    crate::mir::MirPlaceProjection::ArrayElement { .. }
                                )
                            });
                        if !self_copy
                            && !array_element
                            && !state.mutation_permits.remove(&assignment.destination)
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "class optional assignment lacks a matching mutation check",
                            );
                        }
                    }
                    MirInstruction::ClassOptionalCleanup(cleanup) => {
                        if !state.mutation_permits.remove(&cleanup.destination) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "class optional cleanup lacks a matching mutation check",
                            );
                        }
                    }
                    MirInstruction::SharedRelease(release)
                        if state.active.values().any(|(source, _)| {
                            matches!(
                                source.base,
                                crate::mir::MirPlaceBase::SharedPointee(owner)
                                    if owner == release.owner
                            )
                        }) =>
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "optional payload guard outlives its shared container anchor",
                        );
                    }
                    _ => {}
                }
            }
            if !state.mutation_permits.is_empty() {
                self.block_error(
                    function.callable(),
                    block.id,
                    "optional mutation check has no immediate transition",
                );
                state.mutation_permits.clear();
            }

            match &block.terminator {
                Some(MirTerminator::BeginOptionalView {
                    begin,
                    success_target,
                    absent_target,
                    overflow_target,
                    ..
                }) => {
                    self.require_active_payload_guards(function, block, &begin.source, &state);
                    let mut success = state.clone();
                    if success
                        .active
                        .insert(begin.guard, (begin.source.clone(), begin.class))
                        .is_some()
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "optional guard begins more than once",
                        );
                    } else {
                        success.order.push(begin.guard);
                    }
                    merge_optional_guard_state(
                        self,
                        function,
                        *success_target,
                        &success,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                    for target in [*absent_target, *overflow_target] {
                        merge_optional_guard_state(
                            self,
                            function,
                            target,
                            &state,
                            &mut incoming,
                            &mut pending,
                            &mut reported_joins,
                        );
                    }
                }
                Some(MirTerminator::CheckOptionalMutation {
                    source,
                    success_target,
                    failure_target,
                    ..
                }) => {
                    self.require_active_payload_guards(function, block, source, &state);
                    let mut success = state.clone();
                    success.mutation_permits.insert(source.clone());
                    merge_optional_guard_state(
                        self,
                        function,
                        *success_target,
                        &success,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                    merge_optional_guard_state(
                        self,
                        function,
                        *failure_target,
                        &state,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                }
                Some(
                    MirTerminator::Return { .. }
                    | MirTerminator::ReturnShared { .. }
                    | MirTerminator::ReturnOptionalShared { .. },
                ) => {
                    if !state.active.is_empty() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "optional payload guard remains active on normal return",
                        );
                    }
                }
                Some(terminator) => {
                    if let MirTerminator::CheckedCast { binding, .. } = terminator {
                        self.require_active_payload_guards(
                            function,
                            block,
                            &binding.view.source,
                            &state,
                        );
                    }
                    for target in terminator.successors() {
                        merge_optional_guard_state(
                            self,
                            function,
                            target,
                            &state,
                            &mut incoming,
                            &mut pending,
                            &mut reported_joins,
                        );
                    }
                }
                None => {}
            }
        }
    }

    fn verify_guarded_payload_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirInstruction,
        state: &OptionalGuardState,
    ) {
        match instruction {
            MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
                MirRvalueKind::Load(source) | MirRvalueKind::OptionalPresence { source, .. } => {
                    self.require_active_payload_guards(function, block, source, state);
                }
                MirRvalueKind::TypeTest { source, .. } => {
                    self.require_active_payload_guards(function, block, &source.source, state);
                }
                _ => {}
            },
            MirInstruction::Call(call) => {
                if let Some(receiver) = &call.receiver {
                    let place = match receiver {
                        crate::mir::MirCallReceiver::Method(receiver) => &receiver.place,
                        crate::mir::MirCallReceiver::Interface(view) => &view.source,
                    };
                    self.require_active_payload_guards(function, block, place, state);
                }
                for argument in &call.arguments {
                    let place = match argument {
                        crate::mir::MirArgument::Place(place)
                        | crate::mir::MirArgument::OwnedPlace(place) => Some(place),
                        crate::mir::MirArgument::View(view) => Some(&view.source),
                        crate::mir::MirArgument::Value(_)
                        | crate::mir::MirArgument::SharedOwner(_) => None,
                    };
                    if let Some(place) = place {
                        self.require_active_payload_guards(function, block, place, state);
                    }
                }
            }
            MirInstruction::Store(store) => {
                self.require_active_payload_guards(function, block, &store.destination, state);
            }
            MirInstruction::Initialize(initialize) => {
                self.require_guarded_nested_destination(
                    function,
                    block,
                    &initialize.destination,
                    state,
                );
            }
            MirInstruction::CopyConstruct(copy) => {
                self.require_active_payload_guards(function, block, &copy.source, state);
                self.require_guarded_nested_destination(function, block, &copy.destination, state);
            }
            MirInstruction::CopyAssign(copy) => {
                self.require_active_payload_guards(function, block, &copy.source, state);
                self.require_active_payload_guards(function, block, &copy.destination, state);
            }
            MirInstruction::BindCheckedView(binding) => {
                self.require_active_payload_guards(function, block, &binding.view.source, state);
            }
            MirInstruction::EndOptionalView(end) => {
                self.require_active_payload_guards(function, block, &end.source, state);
            }
            _ => {}
        }
    }

    fn require_guarded_nested_destination(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
        state: &OptionalGuardState,
    ) {
        if place
            .projections
            .iter()
            .enumerate()
            .any(|(index, projection)| {
                matches!(
                    projection,
                    crate::mir::MirPlaceProjection::OptionalPayload(_)
                ) && index + 1 < place.projections.len()
            })
        {
            self.require_active_payload_guards(function, block, place, state);
        }
    }

    fn require_active_payload_guards(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
        state: &OptionalGuardState,
    ) {
        let mut root = MirPlace {
            base: place.base,
            projections: Vec::new(),
        };
        for projection in &place.projections {
            if let crate::mir::MirPlaceProjection::OptionalPayload(class) = projection {
                if !state
                    .active
                    .values()
                    .any(|(source, guarded_class)| source == &root && guarded_class == class)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional payload place is used without its matching active guard",
                    );
                }
            }
            root.projections.push(*projection);
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct OptionalGuardState {
    active: BTreeMap<OptionalGuardId, (MirPlace, crate::identity::ClassId)>,
    order: Vec<OptionalGuardId>,
    mutation_permits: HashSet<MirPlace>,
}

fn merge_optional_guard_state(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    target: crate::mir::BlockId,
    state: &OptionalGuardState,
    incoming: &mut [Option<OptionalGuardState>],
    pending: &mut VecDeque<crate::mir::BlockId>,
    reported_joins: &mut HashSet<crate::mir::BlockId>,
) {
    if target.callable() != function.callable() || target.index() >= incoming.len() {
        return;
    }
    match &incoming[target.index()] {
        None => {
            incoming[target.index()] = Some(state.clone());
            pending.push_back(target);
        }
        Some(existing) if existing != state => {
            if reported_joins.insert(target) {
                verifier.block_error(
                    function.callable(),
                    target,
                    "optional guard state differs across control-flow paths",
                );
            }
        }
        Some(_) => {}
    }
}
