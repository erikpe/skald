//! Independent encoding, secured-value and success-edge validation.
use super::super::super::{
    numeric::{Cell, Domain, Numeric, Recipe},
    Instruction, Opcode, Site,
};
use crate::backend::{
    graph::{SelectedBlockId, SelectedValueId},
    lir::{Constant, ScalarCheck, ScalarDomainEvidence},
    plan::ScalarType,
    selected::{SelectedDraft, SelectedFact},
};
use std::collections::{BTreeMap, BTreeSet};
mod recipes;

mod facts;
use facts::Facts;

pub(in crate::backend::x86_64_sysv::native::selected::verify) fn graph(
    draft: &SelectedDraft<'_, Instruction>,
) -> Result<(), &'static str> {
    let mut facts = Facts {
        defs: BTreeMap::new(),
        terms: BTreeMap::new(),
        entry: None,
        guards: BTreeMap::new(),
        parameters: BTreeMap::new(),
        sites: BTreeMap::new(),
        source_sites: BTreeMap::new(),
        representations: BTreeMap::new(),
    };
    let mut operations = vec![];
    let mut origins = BTreeMap::new();
    draft.visit(|fact| {
        match fact {
            SelectedFact::Entry { entry, .. } => facts.entry = entry,
            SelectedFact::Value {
                id, representation, ..
            } => {
                facts.representations.insert(id, representation);
            }
            SelectedFact::Origins { blocks, .. } => {
                origins.extend(blocks.iter().map(|(a, b)| (*a, *b)))
            }
            SelectedFact::Block { id, parameters, .. } => {
                for (ordinal, value) in parameters.iter().enumerate() {
                    facts.parameters.insert(*value, (id, ordinal));
                }
            }
            SelectedFact::Instruction { block, payload, .. } => {
                for (out, is_definition) in payload.operands() {
                    if is_definition {
                        facts.defs.insert(out.value, &payload.opcode);
                        facts.sites.insert(out.value, block);
                        facts.source_sites.insert(out.value, payload.origin.site);
                    }
                }
                operations.push((block, payload));
            }
            SelectedFact::Terminal {
                block,
                payload: Some(node),
                edges,
            } => {
                facts.terms.insert(block, (node, edges.to_vec()));
            }
            _ => {}
        }
        Ok::<_, &'static str>(())
    })?;
    for (block, (node, _)) in &facts.terms {
        if matches!(node.opcode, Opcode::CheckBranch { .. }) {
            if let Site::Terminator(source) = node.origin.site {
                if let Some(original) = origins.get(&source) {
                    if facts.guards.insert(*original, *block).is_some() {
                        return Err("ambiguous native guard origin");
                    }
                }
            }
        }
    }
    for (block, (node, _)) in &facts.terms {
        if let Opcode::CheckBranch {
            condition,
            relation,
        } = &node.opcode
        {
            if !facts.check_condition(condition.value, relation) {
                return Err("native check does not test its secured source");
            }
            use crate::backend::failure::FailureMessage::*;
            let valid_failure = match relation {
                ScalarCheck::NonZeroDivisor { .. } => matches!(
                    facts.failure_reason(*block),
                    Some(IntegerDivisionByZero | IntegerRemainderByZero)
                ),
                ScalarCheck::ShiftCountBelowWidth { .. } => {
                    facts.failure_reason(*block) == Some(ShiftCountOutOfRange)
                }
                ScalarCheck::FiniteTruncatedF64InIntegerRange { .. } => {
                    facts.failure_reason(*block) == Some(PrimitiveCastOutOfRange)
                }
            };
            if !valid_failure {
                return Err("native check failure path is bypassed or substituted");
            }
        }
    }
    let associations = operations
        .iter()
        .filter_map(|(_, node)| match &node.opcode {
            Opcode::Numeric(Numeric::Boundary(recipe)) => Some((node.origin.site, recipe)),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (block, node) in operations {
        let Opcode::Numeric(n) = &node.opcode else {
            continue;
        };
        if matches!(n, Numeric::Divide { .. } | Numeric::CheckedTruncate { .. })
            && !associations.iter().any(|(site, recipe)| {
                *site == node.origin.site
                    && matches!(
                        (n, recipe),
                        (Numeric::Divide { .. }, Recipe::Division { .. })
                            | (
                                Numeric::CheckedTruncate { .. },
                                Recipe::Conversion {
                                    conversion: crate::backend::lir::Conversion::TruncateFloat,
                                    ..
                                }
                            )
                    )
            })
        {
            return Err("numeric cell lacks its semantic association");
        }
        if n.domain().is_some_and(|d| !facts.domain(block, d)) {
            return Err("native numeric domain is bypassed or substituted");
        }
        match n {
            Numeric::Boundary(recipe) => {
                let mut recipe = recipe.clone();
                if recipe
                    .values_mut()
                    .iter()
                    .any(|v| facts.representations.get(&v.value) != Some(&v.representation))
                {
                    return Err("foreign numeric recipe value");
                }
                facts.recipe(&recipe, node.origin.site)?;
            }
            Numeric::Shift {
                count,
                input,
                domain,
                ..
            } => {
                let ScalarCheck::ShiftCountBelowWidth { count: full, width } = domain.relation
                else {
                    return Err("wrong shift domain");
                };
                if facts
                    .sites
                    .get(&count.value)
                    .is_none_or(|site| !facts.domain(*site, domain))
                {
                    return Err("CL narrowed before full-width guard");
                }
                if u16::from(width) != input.representation.bits()
                    || !matches!(facts.defs.get(&count.value),Some(Opcode::Numeric(Numeric::Cell {cell:Cell::Narrow,input,..}))if input.value==full)
                {
                    return Err("native CL does not narrow the guarded full count");
                }
            }
            Numeric::Divide {
                signed,
                low,
                high,
                divisor,
                domain,
                overflow,
                ..
            } => {
                let ScalarCheck::NonZeroDivisor {
                    ty,
                    divisor: original,
                } = domain.relation
                else {
                    return Err("wrong division domain");
                };
                if facts
                    .sites
                    .get(&high.value)
                    .is_none_or(|site| !facts.domain(*site, domain))
                {
                    return Err("dividend setup precedes its domain guard");
                }
                let setup_matches = match facts.defs.get(&high.value) {
                    Some(Opcode::Numeric(Numeric::Dividend {
                        signed: setup_signed,
                        low: setup_low,
                        ..
                    })) => setup_signed == signed && setup_low.value == low.value,
                    _ => false,
                };
                if *signed != (ty == ScalarType::I64)
                    || !facts.widened(divisor.value, original)
                    || !setup_matches
                {
                    return Err("native dividend setup or divisor association mismatch");
                }
                if *signed {
                    let Some(guard) = overflow else {
                        return Err("signed divide lacks overflow exclusion");
                    };
                    use crate::primitive_comparison::PrimitiveComparisonPredicate::Equal;
                    let valid = facts.terms.get(guard).is_some_and(|(node, _)| {
                        let Opcode::Branch { condition } = node.opcode else {
                            return false;
                        };
                        facts.and(condition.value).is_some_and(|(a, b)| {
                            facts.cmp(a, Equal, low.value, Constant::I64(i64::MIN), true)
                                && facts.cmp(b, Equal, original, Constant::I64(-1), true)
                        })
                    });
                    if !valid || !facts.protected(*guard, 1, block) {
                        return Err("signed divide overflow exclusion is bypassed or substituted");
                    }
                } else if overflow.is_some() {
                    return Err("unsigned divide has extraneous overflow exclusion");
                }
            }
            Numeric::CheckedTruncate {
                input,
                source,
                target,
                domain,
                ..
            } => {
                if domain.relation
                    != (ScalarCheck::FiniteTruncatedF64InIntegerRange {
                        source: source.value,
                        target: *target,
                    })
                {
                    return Err("native conversion has a substituted source domain");
                }
                if *target == ScalarType::U64 {
                    let reduced = match facts.defs.get(&input.value) {
                        Some(Opcode::Alu {
                            operation: crate::backend::lir::BinaryOperation::Subtract,
                            left,
                            right,
                            ..
                        }) => {
                            left.value == source.value
                                && facts.constant(right.value)
                                    == Some(Constant::F64(0x43e0_0000_0000_0000))
                        }
                        _ => false,
                    };
                    if !((input.value == source.value && facts.upper_arm(block, source.value, 0))
                        || (reduced && facts.upper_arm(block, source.value, 1)))
                    {
                        return Err("unsigned float correction arm is bypassed or substituted");
                    }
                } else if input.value != source.value {
                    return Err("signed or byte float conversion source mismatch");
                }
            }
            _ => {}
        }
    }
    Ok(())
}
