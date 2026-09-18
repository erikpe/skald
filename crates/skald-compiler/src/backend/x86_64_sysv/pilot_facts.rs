//! Narrow projection of existing checked services; no machine products escape.

use super::{layout, runtime_trace};
use crate::{
    backend::{
        pilot::{TraceContext, TraceFacts, TraceLocation, TraceRequest},
        plan::{DataKey, LayoutDisposition, LayoutFact},
        BackendError, BackendInput, RuntimeTracePolicy,
    },
    mir::MirType,
};
use std::collections::BTreeMap;

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn project_layouts(
    input: BackendInput<'_>,
    types: &[MirType],
) -> Result<Vec<LayoutFact>, BackendError> {
    let layouts = layout::DataLayout::compute(input.program())?;
    types
        .iter()
        .map(|ty| {
            let disposition = match ty {
                MirType::Unit => LayoutDisposition::ElidedUnit,
                MirType::Obj | MirType::Interface(_) => LayoutDisposition::ElidedMetadata,
                _ => LayoutDisposition::Addressable,
            };
            if disposition != LayoutDisposition::Addressable {
                return Ok(LayoutFact {
                    size: 0,
                    alignment: 1,
                    disposition,
                });
            }
            let layout = layouts.ty(*ty)?;
            Ok(LayoutFact {
                size: layout.size(),
                alignment: layout.alignment(),
                disposition,
            })
        })
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn project_trace(
    input: BackendInput<'_>,
) -> Result<TraceFacts, BackendError> {
    if input.runtime_trace() == RuntimeTracePolicy::Omitted {
        return Ok(TraceFacts::default());
    }
    let metadata = runtime_trace::Metadata::new(input);
    let mut requests = Vec::new();
    for definition in input.program().executable_definitions() {
        let callable = definition.callable();
        for span in std::iter::once(definition.span()).chain(
            definition.body().blocks.iter().flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|i| i.span())
                    .chain(block.terminator.iter().map(|t| t.span()))
            }),
        ) {
            let key = metadata
                .request_location(callable, span)?
                .expect("enabled metadata");
            requests.push((callable, span, key));
        }
    }
    let metadata = metadata.finish();
    let strings = metadata
        .strings
        .iter()
        .enumerate()
        .map(|(index, string)| (string.symbol.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let contexts = metadata
        .contexts
        .iter()
        .enumerate()
        .map(|(index, context)| (context.symbol.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let locations = metadata
        .locations
        .iter()
        .enumerate()
        .map(|(index, location)| (location.symbol.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    Ok(TraceFacts {
        record_layout: Some(LayoutFact {
            size: super::frame::TRACE_RECORD_SIZE,
            alignment: super::frame::TRACE_RECORD_ALIGNMENT,
            disposition: LayoutDisposition::Addressable,
        }),
        strings: metadata.strings.iter().map(|s| s.bytes.clone()).collect(),
        contexts: metadata
            .contexts
            .iter()
            .map(|context| TraceContext {
                name: DataKey::TraceBytes(strings[context.name_symbol.as_str()]),
                path: DataKey::TraceBytes(strings[context.path_symbol.as_str()]),
            })
            .collect(),
        locations: metadata
            .locations
            .iter()
            .map(|location| TraceLocation {
                context: DataKey::TraceContext(contexts[location.context_symbol.as_str()]),
                line: location.line,
                column: location.column,
            })
            .collect(),
        requests: requests
            .into_iter()
            .map(|(callable, span, key)| TraceRequest {
                callable,
                span,
                location: DataKey::TraceLocation(locations[key.as_str()]),
            })
            .collect(),
    })
}
