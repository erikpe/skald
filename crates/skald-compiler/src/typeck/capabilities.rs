//! Neutral-guided materialization of concrete HIR copy plans.

use crate::{
    hir::{
        HirBaseCopy, HirCopyCapability, HirSelectedCopyOperation, HirSynthesizedCopy,
        HirSynthesizedFieldCopy, HirUserCopy,
    },
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId},
    resolve::{ResolvedClassDeclaration, ResolvedCopyOperation, ResolvedProgram, ResolvedTypeKind},
    type_capabilities::{LifecyclePathElement, ResolvedLifecycleCapabilities},
};

#[cfg(test)]
use crate::type_capabilities::LifecycleComputationReport;

#[derive(Debug)]
pub(super) struct CopyCapabilities {
    lifecycle: ResolvedLifecycleCapabilities,
    plans: CopyCapabilityPlans,
    array_types: crate::hir::HirArrayTypeTable,
}

#[derive(Debug)]
pub(in crate::typeck) struct CopyCapabilityPlans {
    constructors: Vec<HirCopyCapability<CopyConstructorId>>,
    assignments: Vec<HirCopyCapability<CopyAssignmentId>>,
}

#[cfg(test)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CopyCapabilityComputationReport {
    pub(super) neutral_computations: usize,
    pub(super) neutral_lifecycle: LifecycleComputationReport,
    pub(super) constructor_rounds: usize,
    pub(super) assignment_rounds: usize,
    pub(super) cloned_constructor_records: usize,
    pub(super) cloned_assignment_records: usize,
    pub(super) provisional_array_builds: usize,
    pub(super) provisional_array_entries: usize,
    pub(super) final_array_builds: usize,
    pub(super) final_array_entries: usize,
    pub(super) final_publication_clones: usize,
    pub(super) final_publication_entries: usize,
    pub(super) final_publication_moves: usize,
    pub(super) final_publication_moved_entries: usize,
    pub(super) constructor_plan_constructions: usize,
    pub(super) assignment_plan_constructions: usize,
}

impl CopyCapabilities {
    pub(super) fn compute(program: &ResolvedProgram) -> Self {
        #[cfg(test)]
        {
            Self::compute_internal(program, None)
        }
        #[cfg(not(test))]
        {
            Self::compute_internal(program)
        }
    }

    #[cfg(test)]
    pub(super) fn compute_with_report(
        program: &ResolvedProgram,
    ) -> (Self, CopyCapabilityComputationReport) {
        let mut report = CopyCapabilityComputationReport::default();
        let capabilities = Self::compute_internal(program, Some(&mut report));
        (capabilities, report)
    }

    fn compute_internal(
        program: &ResolvedProgram,
        #[cfg(test)] mut report: Option<&mut CopyCapabilityComputationReport>,
    ) -> Self {
        #[cfg(test)]
        let lifecycle = if let Some(report) = report.as_deref_mut() {
            let (lifecycle, lifecycle_report) =
                ResolvedLifecycleCapabilities::compute_with_report(program);
            report.neutral_computations = report.neutral_computations.saturating_add(1);
            report.neutral_lifecycle = lifecycle_report;
            lifecycle
        } else {
            ResolvedLifecycleCapabilities::compute(program)
        };
        #[cfg(not(test))]
        let lifecycle = ResolvedLifecycleCapabilities::compute(program);

        let constructors = materialize_plans(
            program,
            &lifecycle,
            PlanOperation {
                name: "copy constructor",
                resolved: |class| class.copy_constructor,
                available: ResolvedLifecycleCapabilities::constructor,
                array_available: ResolvedLifecycleCapabilities::array_copy,
                required_constructors: None,
            },
        );
        #[cfg(test)]
        record_count(
            &mut report,
            |report| &mut report.constructor_plan_constructions,
            constructors.len(),
        );

        let assignments = materialize_plans(
            program,
            &lifecycle,
            PlanOperation {
                name: "copy assignment",
                resolved: |class| class.copy_assignment,
                available: ResolvedLifecycleCapabilities::assignment,
                array_available: ResolvedLifecycleCapabilities::array_assignment,
                required_constructors: Some(&constructors),
            },
        );
        #[cfg(test)]
        record_count(
            &mut report,
            |report| &mut report.assignment_plan_constructions,
            assignments.len(),
        );

        let plans = CopyCapabilityPlans {
            constructors,
            assignments,
        };
        #[cfg(test)]
        {
            record_count(&mut report, |report| &mut report.final_array_builds, 1);
            record_count(
                &mut report,
                |report| &mut report.final_array_entries,
                program.array_types.len(),
            );
        }
        let array_types = crate::typeck::arrays::lower_array_types(program, &plans, &lifecycle);
        let capabilities = Self {
            lifecycle,
            plans,
            array_types,
        };
        capabilities.assert_consistent(program);
        capabilities
    }

    pub(super) fn constructor(&self, class: ClassId) -> &HirCopyCapability<CopyConstructorId> {
        self.plans.constructor(class)
    }

    pub(super) fn assignment(&self, class: ClassId) -> &HirCopyCapability<CopyAssignmentId> {
        self.plans.assignment(class)
    }

    pub(super) fn array(&self, array: ArrayTypeId) -> &crate::hir::HirArrayType {
        self.array_types
            .get(array)
            .expect("resolved array identity must have typed lifecycle metadata")
    }

    pub(super) fn into_array_types(self) -> crate::hir::HirArrayTypeTable {
        self.array_types
    }

    #[cfg(test)]
    pub(super) fn into_array_types_with_report(
        self,
        report: &mut CopyCapabilityComputationReport,
    ) -> crate::hir::HirArrayTypeTable {
        report.final_publication_moves = report.final_publication_moves.saturating_add(1);
        report.final_publication_moved_entries = report
            .final_publication_moved_entries
            .saturating_add(self.array_types.len());
        self.into_array_types()
    }

    pub(crate) fn constructor_failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.lifecycle.constructor_failure(class)
    }

    pub(crate) fn assignment_failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.lifecycle.assignment_failure(class)
    }

    fn assert_consistent(&self, program: &ResolvedProgram) {
        for class in program.classes.iter() {
            assert_eq!(
                self.lifecycle.constructor(class.id),
                self.constructor(class.id).selected().is_some(),
                "neutral lifecycle and HIR copy-constructor plan diverged for {}",
                class.id,
            );
            assert_eq!(
                self.lifecycle.assignment(class.id),
                self.assignment(class.id).selected().is_some(),
                "neutral lifecycle and HIR copy-assignment plan diverged for {}",
                class.id,
            );
        }
        for array in program.array_types.iter() {
            let hir = self.array(array.id);
            assert_eq!(
                self.lifecycle.array_copy(array.id),
                hir.lifecycle.copy.is_some(),
                "neutral lifecycle and HIR array copy plan diverged for {}",
                array.id,
            );
            assert_eq!(
                self.lifecycle.array_assignment(array.id),
                hir.lifecycle.assignment.is_some(),
                "neutral lifecycle and HIR array assignment plan diverged for {}",
                array.id,
            );
        }
    }

    #[cfg(test)]
    pub(super) fn lifecycle_for_test(&self) -> &ResolvedLifecycleCapabilities {
        &self.lifecycle
    }
}

impl CopyCapabilityPlans {
    pub(in crate::typeck) fn constructor(
        &self,
        class: ClassId,
    ) -> &HirCopyCapability<CopyConstructorId> {
        &self.constructors[class.index()]
    }

    pub(in crate::typeck) fn assignment(
        &self,
        class: ClassId,
    ) -> &HirCopyCapability<CopyAssignmentId> {
        &self.assignments[class.index()]
    }
}

#[cfg(test)]
fn record_count(
    report: &mut Option<&mut CopyCapabilityComputationReport>,
    select: impl FnOnce(&mut CopyCapabilityComputationReport) -> &mut usize,
    count: usize,
) {
    if let Some(report) = report.as_deref_mut() {
        let value = select(report);
        *value = value.saturating_add(count);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Complete,
}

#[derive(Clone, Copy)]
struct PlanOperation<'a, I> {
    name: &'static str,
    resolved: fn(&ResolvedClassDeclaration) -> ResolvedCopyOperation<I>,
    available: fn(&ResolvedLifecycleCapabilities, ClassId) -> bool,
    array_available: fn(&ResolvedLifecycleCapabilities, ArrayTypeId) -> bool,
    required_constructors: Option<&'a [HirCopyCapability<CopyConstructorId>]>,
}

fn materialize_plans<I: Copy>(
    program: &ResolvedProgram,
    lifecycle: &ResolvedLifecycleCapabilities,
    operation: PlanOperation<'_, I>,
) -> Vec<HirCopyCapability<I>> {
    PlanMaterializer::new(program, lifecycle, operation).materialize_all()
}

#[cfg(test)]
pub(in crate::typeck) fn materialize_constructor_plans_for_test(
    program: &ResolvedProgram,
    lifecycle: &ResolvedLifecycleCapabilities,
) -> Vec<HirCopyCapability<CopyConstructorId>> {
    materialize_plans(
        program,
        lifecycle,
        PlanOperation {
            name: "copy constructor",
            resolved: |class| class.copy_constructor,
            available: ResolvedLifecycleCapabilities::constructor,
            array_available: ResolvedLifecycleCapabilities::array_copy,
            required_constructors: None,
        },
    )
}

struct PlanMaterializer<'a, I> {
    program: &'a ResolvedProgram,
    lifecycle: &'a ResolvedLifecycleCapabilities,
    operation: PlanOperation<'a, I>,
    plans: Vec<Option<HirCopyCapability<I>>>,
    states: Vec<VisitState>,
}

impl<'a, I: Copy> PlanMaterializer<'a, I> {
    fn new(
        program: &'a ResolvedProgram,
        lifecycle: &'a ResolvedLifecycleCapabilities,
        operation: PlanOperation<'a, I>,
    ) -> Self {
        Self {
            program,
            lifecycle,
            operation,
            plans: vec![None; program.classes.len()],
            states: vec![VisitState::Unvisited; program.classes.len()],
        }
    }

    fn materialize_all(mut self) -> Vec<HirCopyCapability<I>> {
        for class in self.program.classes.iter() {
            self.materialize_class(class.id);
        }
        self.plans
            .into_iter()
            .map(|plan| plan.expect("every class copy plan must be materialized"))
            .collect()
    }

    fn materialize_class(&mut self, class_id: ClassId) -> Option<HirSelectedCopyOperation<I>> {
        match self.states[class_id.index()] {
            VisitState::Complete => {
                return self.plans[class_id.index()]
                    .as_ref()
                    .expect("complete class copy plan must exist")
                    .selected();
            }
            VisitState::Visiting => panic!(
                "neutral lifecycle marked {} available through recursive class dependency at {}",
                self.operation.name, class_id,
            ),
            VisitState::Unvisited => {}
        }

        if !(self.operation.available)(self.lifecycle, class_id) {
            self.plans[class_id.index()] = Some(HirCopyCapability::Unavailable);
            self.states[class_id.index()] = VisitState::Complete;
            return None;
        }
        self.states[class_id.index()] = VisitState::Visiting;

        let program = self.program;
        let class = program
            .class(class_id)
            .expect("copy-plan class must be resolved");
        let base = class.direct_base.map(|direct_base| HirBaseCopy {
            base: direct_base.class,
            operation: self.require_class_plan(direct_base.class),
        });
        let plan = match (self.operation.resolved)(class) {
            ResolvedCopyOperation::User(selected) => HirCopyCapability::User(HirUserCopy {
                operation: selected,
                base,
            }),
            ResolvedCopyOperation::Synthesized(_) => {
                let fields = self.materialize_fields(class);
                HirCopyCapability::Synthesized(HirSynthesizedCopy {
                    class: class_id,
                    base,
                    fields,
                    final_fields: if self.operation.required_constructors.is_some() {
                        class
                            .fields
                            .iter()
                            .filter(|field| field.final_span.is_some())
                            .map(|field| field.id)
                            .collect()
                    } else {
                        Vec::new()
                    },
                })
            }
            ResolvedCopyOperation::Unavailable => panic!(
                "neutral lifecycle marked unavailable resolved {} available for {}",
                self.operation.name, class_id,
            ),
        };
        let selected = plan
            .selected()
            .expect("neutral-available class copy plan must select an operation");
        self.plans[class_id.index()] = Some(plan);
        self.states[class_id.index()] = VisitState::Complete;
        Some(selected)
    }

    fn require_class_plan(&mut self, class: ClassId) -> HirSelectedCopyOperation<I> {
        self.materialize_class(class).unwrap_or_else(|| {
            panic!(
                "neutral-available {} requires unavailable class plan {}",
                self.operation.name, class,
            )
        })
    }

    fn materialize_fields(
        &mut self,
        class: &ResolvedClassDeclaration,
    ) -> Vec<HirSynthesizedFieldCopy<I>> {
        let mut fields = Vec::with_capacity(class.fields.len());
        for field in &class.fields {
            let plan = match field.type_syntax.kind {
                ResolvedTypeKind::Class(target) => HirSynthesizedFieldCopy::Class {
                    field: field.id,
                    operation: self.require_class_plan(target),
                },
                ResolvedTypeKind::Shared(_) => HirSynthesizedFieldCopy::Shared { field: field.id },
                ResolvedTypeKind::Optional(optional) => {
                    self.materialize_optional_field(field.id, optional)
                }
                ResolvedTypeKind::Array(array) => {
                    self.require_array_fact(array, class.id);
                    HirSynthesizedFieldCopy::Array {
                        field: field.id,
                        array,
                    }
                }
                _ => HirSynthesizedFieldCopy::Scalar { field: field.id },
            };
            fields.push(plan);
        }
        fields
    }

    fn materialize_optional_field(
        &mut self,
        field: crate::identity::FieldId,
        optional: crate::identity::OptionalTypeId,
    ) -> HirSynthesizedFieldCopy<I> {
        let program = self.program;
        let payload = program
            .optional_types
            .get(optional)
            .expect("resolved optional identity must name metadata")
            .payload
            .kind;
        match payload {
            ResolvedTypeKind::I64 => optional_primitive(field, crate::hir::HirPrimitiveType::I64),
            ResolvedTypeKind::U64 => optional_primitive(field, crate::hir::HirPrimitiveType::U64),
            ResolvedTypeKind::U8 => optional_primitive(field, crate::hir::HirPrimitiveType::U8),
            ResolvedTypeKind::F64 => optional_primitive(field, crate::hir::HirPrimitiveType::F64),
            ResolvedTypeKind::Bool => optional_primitive(field, crate::hir::HirPrimitiveType::Bool),
            ResolvedTypeKind::Shared(target) => HirSynthesizedFieldCopy::OptionalShared {
                field,
                target: crate::typeck::shared::lower_shared_target(target),
            },
            ResolvedTypeKind::Class(class) => {
                self.require_optional_constructor(class, field);
                let selected = self.require_class_plan(class);
                HirSynthesizedFieldCopy::OptionalClass {
                    field,
                    class,
                    operation: selected,
                }
            }
            ResolvedTypeKind::Optional(_) => {
                self.require_optional_leaf(optional, field);
                HirSynthesizedFieldCopy::Optional { field, optional }
            }
            ResolvedTypeKind::Array(array) => {
                self.require_array_fact(array, field.class());
                HirSynthesizedFieldCopy::Optional { field, optional }
            }
            ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_)
            | ResolvedTypeKind::Function(_) => panic!(
                "neutral lifecycle marked {} available through invalid optional field {}",
                self.operation.name, field,
            ),
        }
    }

    fn require_optional_leaf(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        field: crate::identity::FieldId,
    ) {
        match optional_leaf(self.program, optional) {
            ResolvedTypeKind::Class(class) => {
                self.require_optional_constructor(class, field);
                self.require_class_plan(class);
            }
            ResolvedTypeKind::Array(array) => {
                self.require_array_fact(array, field.class());
            }
            ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_)
            | ResolvedTypeKind::Function(_) => panic!(
                "neutral lifecycle marked {} available through invalid nested optional field {}",
                self.operation.name, field,
            ),
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Shared(_)
            | ResolvedTypeKind::Optional(_) => {}
        }
    }

    fn require_optional_constructor(&self, class: ClassId, field: crate::identity::FieldId) {
        let Some(constructors) = self.operation.required_constructors else {
            return;
        };
        assert!(
            constructors[class.index()].selected().is_some(),
            "neutral-available {} for optional field {} requires unavailable constructor {}",
            self.operation.name,
            field,
            class,
        );
    }

    fn require_array_fact(&self, array: ArrayTypeId, class: ClassId) {
        assert!(
            (self.operation.array_available)(self.lifecycle, array),
            "neutral-available {} for {} requires unavailable array plan {}",
            self.operation.name,
            class,
            array,
        );
    }
}

fn optional_primitive<I>(
    field: crate::identity::FieldId,
    payload: crate::hir::HirPrimitiveType,
) -> HirSynthesizedFieldCopy<I> {
    HirSynthesizedFieldCopy::OptionalPrimitive { field, payload }
}

fn optional_leaf(
    program: &ResolvedProgram,
    mut optional: crate::identity::OptionalTypeId,
) -> ResolvedTypeKind {
    loop {
        let kind = program
            .optional_types
            .get(optional)
            .expect("nested optional identity must exist")
            .payload
            .kind;
        match kind {
            ResolvedTypeKind::Optional(nested) => optional = nested,
            leaf => return leaf,
        }
    }
}
