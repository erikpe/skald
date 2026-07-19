//! Structural and type verifier for MIR.

use std::{collections::HashSet, fmt};

use crate::resolve::{BindingId, FunctionId};

use super::model::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVerificationError {
    pub function: Option<FunctionId>,
    pub block: Option<BlockId>,
    pub message: String,
}

impl fmt::Display for MirVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.function, self.block) {
            (_, Some(block)) => write!(formatter, "MIR {block}: {}", self.message),
            (Some(function), None) => write!(formatter, "MIR {function}: {}", self.message),
            (None, None) => write!(formatter, "MIR program: {}", self.message),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVerificationErrors {
    errors: Vec<MirVerificationError>,
}

impl MirVerificationErrors {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirVerificationError> {
        self.errors.iter()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for MirVerificationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            error.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for MirVerificationErrors {}

pub fn verify_mir(program: &MirProgram) -> Result<(), MirVerificationErrors> {
    let mut verifier = Verifier {
        program,
        errors: Vec::new(),
    };
    verifier.verify();
    if verifier.errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors {
            errors: verifier.errors,
        })
    }
}

struct Verifier<'mir> {
    program: &'mir MirProgram,
    errors: Vec<MirVerificationError>,
}

impl Verifier<'_> {
    fn verify(&mut self) {
        let entry_declaration = self.program.declarations.get(self.program.entry_function);
        if entry_declaration.is_none() {
            self.program_error(format!(
                "entry function {} is not declared",
                self.program.entry_function
            ));
        } else {
            if !matches!(
                entry_declaration.map(|declaration| &declaration.linkage),
                Some(MirFunctionLinkage::Internal)
            ) {
                self.program_error("entry function must have internal linkage");
            }
            if self
                .program
                .definitions
                .get(self.program.entry_function)
                .is_none()
            {
                self.program_error(format!(
                    "entry function {} has no definition",
                    self.program.entry_function
                ));
            }
        }

        let declarations: Vec<_> = self.program.declarations.iter().collect();
        let mut seen = HashSet::new();
        for (index, declaration) in declarations.iter().enumerate() {
            if declaration.id.index() != index {
                self.function_error(
                    declaration.id,
                    format!(
                        "function declaration table index {index} contains {}",
                        declaration.id
                    ),
                );
            }
            if !seen.insert(declaration.id) {
                self.function_error(declaration.id, "duplicate function declaration ID");
            }
        }

        let mut defined_functions = HashSet::new();
        for (index, definition) in self.program.definitions.slots().iter().enumerate() {
            let Some(definition) = definition else {
                continue;
            };
            if definition.function.index() != index {
                self.function_error(
                    definition.function,
                    format!(
                        "function definition table index {index} contains {}",
                        definition.function
                    ),
                );
            }
            if !defined_functions.insert(definition.function) {
                self.function_error(definition.function, "duplicate function definition");
            }
            let Some(declaration) = self.program.declarations.get(definition.function) else {
                self.function_error(
                    definition.function,
                    "function definition has no declaration",
                );
                continue;
            };
            if !matches!(declaration.linkage, MirFunctionLinkage::Internal) {
                self.function_error(
                    definition.function,
                    "external function must not have a Skald definition",
                );
            }
            self.verify_function(declaration, definition);
        }

        for declaration in declarations {
            match (
                &declaration.linkage,
                self.program.definitions.get(declaration.id),
            ) {
                (MirFunctionLinkage::Internal, None) => {
                    self.function_error(declaration.id, "internal function has no definition");
                }
                (MirFunctionLinkage::External { .. }, Some(_)) => {
                    // Reported while walking definition slots above.
                }
                _ => {}
            }
        }
    }

    fn verify_function(
        &mut self,
        declaration: &MirFunctionDeclaration,
        function: &MirFunctionDefinition,
    ) {
        self.verify_storage(function);
        self.verify_values(function);
        self.verify_parameters(declaration, function);

        if function.block(function.body.entry).is_none() {
            self.function_error(
                function.function,
                format!("entry block {} is not declared", function.body.entry),
            );
        }

        let mut defined = HashSet::new();
        let mut seen_blocks = HashSet::new();
        for (index, block) in function.body.blocks.iter().enumerate() {
            if block.id.function() != function.function {
                self.block_error(
                    function.function,
                    block.id,
                    "block is owned by another function",
                );
            }
            if block.id.index() != index {
                self.block_error(
                    function.function,
                    block.id,
                    format!("block table index {index} contains {}", block.id),
                );
            }
            if !seen_blocks.insert(block.id) {
                self.block_error(function.function, block.id, "duplicate block ID");
            }
            self.verify_block(declaration, function, block, &mut defined);
        }

        for value in &function.values {
            if !defined.contains(&value.id) {
                self.function_error(
                    function.function,
                    format!("value {} has no definition", value.id),
                );
            }
        }
    }

    fn verify_storage(&mut self, function: &MirFunctionDefinition) {
        let mut sources = HashSet::new();
        for (index, storage) in function.storage.iter().enumerate() {
            if storage.id.function() != function.function {
                self.function_error(
                    function.function,
                    format!("storage {} is owned by another function", storage.id),
                );
            }
            if storage.id.index() != index {
                self.function_error(
                    function.function,
                    format!("storage table index {index} contains {}", storage.id),
                );
            }
            if storage.source.function() != function.function {
                self.function_error(
                    function.function,
                    format!(
                        "storage {} has a source binding from another function",
                        storage.id
                    ),
                );
            }
            if !sources.insert(storage.source) {
                self.function_error(
                    function.function,
                    format!(
                        "source binding {} has multiple storage slots",
                        storage.source
                    ),
                );
            }
        }
    }

    fn verify_values(&mut self, function: &MirFunctionDefinition) {
        for (index, value) in function.values.iter().enumerate() {
            if value.id.function() != function.function {
                self.function_error(
                    function.function,
                    format!("value {} is owned by another function", value.id),
                );
            }
            if value.id.index() != index {
                self.function_error(
                    function.function,
                    format!("value table index {index} contains {}", value.id),
                );
            }
        }
    }

    fn verify_parameters(
        &mut self,
        declaration: &MirFunctionDeclaration,
        function: &MirFunctionDefinition,
    ) {
        if function.parameters.len() != declaration.parameter_types.len() {
            self.function_error(
                function.function,
                format!(
                    "definition has {} parameters but declaration requires {}",
                    function.parameters.len(),
                    declaration.parameter_types.len()
                ),
            );
        }
        let mut seen = HashSet::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            let Some(storage) = function.storage(*parameter) else {
                self.function_error(
                    function.function,
                    format!("parameter storage {parameter} is not declared"),
                );
                continue;
            };
            if !seen.insert(*parameter) {
                self.function_error(
                    function.function,
                    format!("duplicate parameter storage {parameter}"),
                );
            }
            if storage.kind != MirStorageKind::Parameter
                || !matches!(storage.source, BindingId::Parameter(_))
            {
                self.function_error(
                    function.function,
                    format!("parameter {parameter} does not identify parameter storage"),
                );
            }
            if !matches!(storage.source, BindingId::Parameter(id) if id.index() == index) {
                self.function_error(
                    function.function,
                    format!("parameter position {index} has mismatched source binding"),
                );
            }
            if declaration
                .parameter_types
                .get(index)
                .is_some_and(|ty| *ty != storage.ty)
            {
                self.function_error(
                    function.function,
                    format!("parameter position {index} type differs from declaration"),
                );
            }
        }
    }

    fn verify_block(
        &mut self,
        declaration: &MirFunctionDeclaration,
        function: &MirFunctionDefinition,
        block: &MirBasicBlock,
        defined: &mut HashSet<ValueId>,
    ) {
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::Assign(assignment) => {
                    let Some(result) = function.value(assignment.result) else {
                        self.block_error(
                            function.function,
                            block.id,
                            format!("assignment result {} is not declared", assignment.result),
                        );
                        continue;
                    };
                    if defined.contains(&assignment.result) {
                        self.block_error(
                            function.function,
                            block.id,
                            format!("value {} is defined more than once", assignment.result),
                        );
                    }
                    if result.ty != assignment.rvalue.ty {
                        self.block_error(
                            function.function,
                            block.id,
                            format!("assignment type does not match value {}", assignment.result),
                        );
                    }
                    self.verify_rvalue(function, block, &assignment.rvalue, defined);
                    defined.insert(assignment.result);
                }
                MirInstruction::Call(call) => {
                    self.verify_call(function, block, call, defined);
                }
                MirInstruction::Store(store) => {
                    let storage_ty = function.storage(store.storage).map(|storage| storage.ty);
                    if storage_ty.is_none() {
                        self.block_error(
                            function.function,
                            block.id,
                            format!("store target {} is not declared", store.storage),
                        );
                    }
                    let value_ty = self.verify_value_use(function, block, store.value, defined);
                    if storage_ty.is_some() && value_ty.is_some() && storage_ty != value_ty {
                        self.block_error(
                            function.function,
                            block.id,
                            "store operand type mismatch",
                        );
                    }
                }
            }
        }

        match &block.terminator {
            Some(MirTerminator::Return { value, .. }) => {
                if let Some(ty) = self.verify_value_use(function, block, *value, defined) {
                    if ty != declaration.return_type {
                        self.block_error(
                            function.function,
                            block.id,
                            "return operand type mismatch",
                        );
                    }
                }
            }
            None => self.block_error(function.function, block.id, "block has no terminator"),
        }
    }

    fn verify_call(
        &mut self,
        function: &MirFunctionDefinition,
        block: &MirBasicBlock,
        call: &MirCall,
        defined: &mut HashSet<ValueId>,
    ) {
        for argument in &call.arguments {
            self.verify_value_use(function, block, *argument, defined);
        }

        let result_ty = match call.result {
            Some(result) => {
                let metadata = function.value(result);
                if metadata.is_none() {
                    self.block_error(
                        function.function,
                        block.id,
                        format!("call result {result} is not declared"),
                    );
                }
                if !defined.insert(result) {
                    self.block_error(
                        function.function,
                        block.id,
                        format!("value {result} is defined more than once"),
                    );
                }
                metadata.map(|metadata| metadata.ty)
            }
            None => None,
        };

        let MirCallTarget::Direct(target_id) = call.target;
        let Some(target) = self.program.declarations.get(target_id) else {
            self.block_error(
                function.function,
                block.id,
                format!("call target {target_id} is not declared"),
            );
            return;
        };

        if call.arguments.len() != target.parameter_types.len() {
            self.block_error(
                function.function,
                block.id,
                format!(
                    "call to {target_id} has {} arguments but requires {}",
                    call.arguments.len(),
                    target.parameter_types.len()
                ),
            );
        }
        for (index, argument) in call.arguments.iter().enumerate() {
            let argument_ty = function.value(*argument).map(|value| value.ty);
            let parameter_ty = target.parameter_types.get(index).copied();
            if argument_ty.is_some() && parameter_ty.is_some() && argument_ty != parameter_ty {
                self.block_error(
                    function.function,
                    block.id,
                    format!("call argument {index} type mismatch"),
                );
            }
        }

        match result_ty {
            Some(result_ty) if result_ty != target.return_type => {
                self.block_error(function.function, block.id, "call result type mismatch")
            }
            None => self.block_error(
                function.function,
                block.id,
                "value-returning call has no result",
            ),
            _ => {}
        }
    }

    fn verify_rvalue(
        &mut self,
        function: &MirFunctionDefinition,
        block: &MirBasicBlock,
        rvalue: &MirRvalue,
        defined: &HashSet<ValueId>,
    ) {
        match &rvalue.kind {
            MirRvalueKind::ConstantI64(_) => {
                if rvalue.ty != MirType::I64 {
                    self.block_error(function.function, block.id, "integer constant is not `i64`");
                }
            }
            MirRvalueKind::Load(storage) => match function.storage(*storage) {
                Some(storage) if storage.ty != rvalue.ty => {
                    self.block_error(function.function, block.id, "load result type mismatch")
                }
                None => self.block_error(
                    function.function,
                    block.id,
                    format!("load source {storage} is not declared"),
                ),
                _ => {}
            },
            MirRvalueKind::Unary { operand, .. } => {
                self.verify_i64_operand(function, block, *operand, defined);
            }
            MirRvalueKind::Binary { left, right, .. } => {
                self.verify_i64_operand(function, block, *left, defined);
                self.verify_i64_operand(function, block, *right, defined);
            }
        }
    }

    fn verify_i64_operand(
        &mut self,
        function: &MirFunctionDefinition,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != MirType::I64 {
                self.block_error(
                    function.function,
                    block.id,
                    "arithmetic operand is not `i64`",
                );
            }
        }
    }

    fn verify_value_use(
        &mut self,
        function: &MirFunctionDefinition,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) -> Option<MirType> {
        let Some(metadata) = function.value(value) else {
            self.block_error(
                function.function,
                block.id,
                format!("value {value} is not declared in this function"),
            );
            return None;
        };
        if !defined.contains(&value) {
            self.block_error(
                function.function,
                block.id,
                format!("value {value} is used before it is defined"),
            );
        }
        Some(metadata.ty)
    }

    fn program_error(&mut self, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            function: None,
            block: None,
            message: message.into(),
        });
    }

    fn function_error(&mut self, function: FunctionId, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            function: Some(function),
            block: None,
            message: message.into(),
        });
    }

    fn block_error(&mut self, function: FunctionId, block: BlockId, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            function: Some(function),
            block: Some(block),
            message: message.into(),
        });
    }
}
