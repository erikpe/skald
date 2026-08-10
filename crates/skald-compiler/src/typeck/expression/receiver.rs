//! Complete-object provenance for alias views and method receivers.

use crate::{
    hir::{HirObjectOrigin, HirObjectPath, HirObjectPlace, HirViewTarget, Type},
    identity::{BindingId, CallableId},
    object_path::ObjectProjection,
    resolve::ResolvedParameterBindingMode,
};

use super::CallableChecker;

impl CallableChecker<'_, '_> {
    pub(super) fn object_origin(&self, place: &HirObjectPlace) -> HirObjectOrigin {
        let root_type = self.binding_type(place.root());
        if let Type::Shared(crate::hir::HirSharedTarget::Class(static_class)) = root_type {
            return super::shared_pointee::CheckedSharedPointee::stable(
                place.root(),
                HirViewTarget::Class(static_class),
                place.access,
                place.projections().to_vec(),
                place.span(),
            )
            .origin();
        }
        if let Some((projection_index, field)) = place
            .projections()
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, projection)| match projection {
                ObjectProjection::Field(field) => Some((index, *field)),
                ObjectProjection::Base(_) => None,
            })
        {
            let Type::Class(dynamic_class) = self
                .program
                .field(field)
                .map(|declaration| {
                    super::super::program::lower_type(self.program, &declaration.type_syntax)
                })
                .expect("object-place field projection must reference a field")
            else {
                unreachable!("an object-place field projection must have class type")
            };
            return HirObjectOrigin::Exact {
                complete: HirObjectPlace {
                    path: HirObjectPath {
                        root: place.root(),
                        projections: place.projections()[..=projection_index].to_vec(),
                        class: dynamic_class,
                        span: place.span(),
                    },
                    access: place.access,
                },
                dynamic_class,
            };
        }

        let static_class = match root_type {
            Type::Class(class) => class,
            _ => unreachable!("a class object place must have a class-capable root"),
        };
        if self.binding_carries_dynamic_origin(place.root()) {
            return HirObjectOrigin::Forwarded {
                binding: place.root(),
                static_target: HirViewTarget::Class(static_class),
                access: place.access,
                dispatch_limit: (matches!(place.root(), BindingId::Receiver(_))
                    && matches!(self.callable, CallableId::Destructor(_)))
                .then_some(static_class),
                span: place.span(),
            };
        }

        HirObjectOrigin::Exact {
            complete: HirObjectPlace {
                path: HirObjectPath {
                    root: place.root(),
                    projections: Vec::new(),
                    class: static_class,
                    span: place.span(),
                },
                access: place.access,
            },
            dynamic_class: static_class,
        }
    }

    fn binding_carries_dynamic_origin(&self, binding: BindingId) -> bool {
        match binding {
            BindingId::Receiver(_) => true,
            BindingId::Parameter(parameter) => !matches!(
                self.parameter(parameter).binding_mode,
                ResolvedParameterBindingMode::Value
            ),
            BindingId::Local(_) => false,
        }
    }
}
