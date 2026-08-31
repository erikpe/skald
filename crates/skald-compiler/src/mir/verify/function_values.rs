//! Canonical function-value metadata and callable-address verification.

use std::collections::HashSet;

use crate::{
    identity::CallableId,
    mir::{
        MirBasicBlock, MirCallableAddress, MirDefinitionRef, MirFunctionLinkage, MirMethodKind,
        MirParameter, MirParameterMode, MirType,
    },
};

use super::context::Verifier;

mod provenance;

impl Verifier<'_> {
    pub(super) fn verify_function_type_declarations(&mut self) {
        let declarations = self
            .program
            .function_types
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut seen = HashSet::new();
        for (index, function) in declarations.iter().enumerate() {
            if function.id.index() != index {
                self.program_error(format!(
                    "function-type table index {index} contains {}",
                    function.id
                ));
            }
            if !seen.insert(function.id) {
                self.program_error(format!("duplicate function type {}", function.id));
            }
            for (parameter_index, parameter) in function.parameters.iter().enumerate() {
                if parameter.mode != MirParameterMode::Value
                    && matches!(parameter.ty, MirType::Function(_))
                {
                    self.program_error(format!(
                        "function type {} parameter {parameter_index} cannot alias a function-value slot",
                        function.id
                    ));
                }
                if let MirType::Function(child) = parameter.ty {
                    if child.index() >= function.id.index() {
                        self.program_error(format!(
                            "function type {} parameter {parameter_index} does not reference bottom-up canonical metadata",
                            function.id
                        ));
                    }
                }
            }
            if let MirType::Function(child) = function.result {
                if child.index() >= function.id.index() {
                    self.program_error(format!(
                        "function type {} result does not reference bottom-up canonical metadata",
                        function.id
                    ));
                }
            }
        }
    }

    pub(super) fn verify_callable_address(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        address: MirCallableAddress,
        rvalue_type: MirType,
    ) {
        if rvalue_type != MirType::Function(address.function_type) {
            self.block_error(
                function.callable(),
                block.id,
                "callable address result type differs from its canonical function type",
            );
        }
        let Some(expected) = self.program.function_type(address.function_type) else {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "callable address references undeclared function type {}",
                    address.function_type
                ),
            );
            return;
        };
        let expected_parameters = expected.parameters.clone();
        let expected_result = expected.result;
        let Some((parameters, result)) =
            self.eligible_callable_signature(function, block, address.target, "callable address")
        else {
            return;
        };
        if parameters != expected_parameters || result != expected_result {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "callable address target {} does not match function type {}",
                    address.target, address.function_type
                ),
            );
        }
    }

    fn eligible_callable_signature(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: CallableId,
        operation: &str,
    ) -> Option<(Vec<MirParameter>, MirType)> {
        match target {
            CallableId::Function(id) => {
                let Some(declaration) = self.program.declarations.get(id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{operation} target {target} is not declared"),
                    );
                    return None;
                };
                if declaration.linkage != MirFunctionLinkage::Internal {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{operation} target {target} is not an internal function"),
                    );
                    return None;
                }
                Some((declaration.parameters.clone(), declaration.return_type))
            }
            CallableId::Method(id) => {
                let Some(declaration) = self.program.method(id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{operation} target {target} is not declared"),
                    );
                    return None;
                };
                if declaration.kind != MirMethodKind::Static {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{operation} target {target} is not a static method"),
                    );
                    return None;
                }
                Some((declaration.parameters.clone(), declaration.return_type))
            }
            _ => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("{operation} target {target} is not eligible for a function value"),
                );
                None
            }
        }
    }
}
