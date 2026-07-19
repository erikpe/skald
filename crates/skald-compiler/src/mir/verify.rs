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
        if self
            .program
            .functions
            .get(self.program.entry_function)
            .is_none()
        {
            self.program_error(format!(
                "entry function {} is not declared",
                self.program.entry_function
            ));
        }

        let functions: Vec<_> = self.program.functions.iter().collect();
        let mut seen = HashSet::new();
        for (index, function) in functions.iter().enumerate() {
            if function.id.index() != index {
                self.function_error(
                    function.id,
                    format!("function table index {index} contains {}", function.id),
                );
            }
            if !seen.insert(function.id) {
                self.function_error(function.id, "duplicate function ID");
            }
        }
        for function in functions {
            self.verify_function(function);
        }
    }

    fn verify_function(&mut self, function: &MirFunction) {
        self.verify_storage(function);
        self.verify_values(function);
        self.verify_parameters(function);

        if function.block(function.body.entry).is_none() {
            self.function_error(
                function.id,
                format!("entry block {} is not declared", function.body.entry),
            );
        }

        let mut defined = HashSet::new();
        let mut seen_blocks = HashSet::new();
        for (index, block) in function.body.blocks.iter().enumerate() {
            if block.id.function() != function.id {
                self.block_error(function.id, block.id, "block is owned by another function");
            }
            if block.id.index() != index {
                self.block_error(
                    function.id,
                    block.id,
                    format!("block table index {index} contains {}", block.id),
                );
            }
            if !seen_blocks.insert(block.id) {
                self.block_error(function.id, block.id, "duplicate block ID");
            }
            self.verify_block(function, block, &mut defined);
        }

        for value in &function.values {
            if !defined.contains(&value.id) {
                self.function_error(function.id, format!("value {} has no definition", value.id));
            }
        }
    }

    fn verify_storage(&mut self, function: &MirFunction) {
        let mut sources = HashSet::new();
        for (index, storage) in function.storage.iter().enumerate() {
            if storage.id.function() != function.id {
                self.function_error(
                    function.id,
                    format!("storage {} is owned by another function", storage.id),
                );
            }
            if storage.id.index() != index {
                self.function_error(
                    function.id,
                    format!("storage table index {index} contains {}", storage.id),
                );
            }
            if storage.source.function() != function.id {
                self.function_error(
                    function.id,
                    format!(
                        "storage {} has a source binding from another function",
                        storage.id
                    ),
                );
            }
            if !sources.insert(storage.source) {
                self.function_error(
                    function.id,
                    format!(
                        "source binding {} has multiple storage slots",
                        storage.source
                    ),
                );
            }
        }
    }

    fn verify_values(&mut self, function: &MirFunction) {
        for (index, value) in function.values.iter().enumerate() {
            if value.id.function() != function.id {
                self.function_error(
                    function.id,
                    format!("value {} is owned by another function", value.id),
                );
            }
            if value.id.index() != index {
                self.function_error(
                    function.id,
                    format!("value table index {index} contains {}", value.id),
                );
            }
        }
    }

    fn verify_parameters(&mut self, function: &MirFunction) {
        let mut seen = HashSet::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            let Some(storage) = function.storage(*parameter) else {
                self.function_error(
                    function.id,
                    format!("parameter storage {parameter} is not declared"),
                );
                continue;
            };
            if !seen.insert(*parameter) {
                self.function_error(
                    function.id,
                    format!("duplicate parameter storage {parameter}"),
                );
            }
            if storage.kind != MirStorageKind::Parameter
                || !matches!(storage.source, BindingId::Parameter(_))
            {
                self.function_error(
                    function.id,
                    format!("parameter {parameter} does not identify parameter storage"),
                );
            }
            if !matches!(storage.source, BindingId::Parameter(id) if id.index() == index) {
                self.function_error(
                    function.id,
                    format!("parameter position {index} has mismatched source binding"),
                );
            }
        }
    }

    fn verify_block(
        &mut self,
        function: &MirFunction,
        block: &MirBasicBlock,
        defined: &mut HashSet<ValueId>,
    ) {
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::Assign(assignment) => {
                    let Some(result) = function.value(assignment.result) else {
                        self.block_error(
                            function.id,
                            block.id,
                            format!("assignment result {} is not declared", assignment.result),
                        );
                        continue;
                    };
                    if defined.contains(&assignment.result) {
                        self.block_error(
                            function.id,
                            block.id,
                            format!("value {} is defined more than once", assignment.result),
                        );
                    }
                    if result.ty != assignment.rvalue.ty {
                        self.block_error(
                            function.id,
                            block.id,
                            format!("assignment type does not match value {}", assignment.result),
                        );
                    }
                    self.verify_rvalue(function, block, &assignment.rvalue, defined);
                    defined.insert(assignment.result);
                }
                MirInstruction::Store(store) => {
                    let storage_ty = function.storage(store.storage).map(|storage| storage.ty);
                    if storage_ty.is_none() {
                        self.block_error(
                            function.id,
                            block.id,
                            format!("store target {} is not declared", store.storage),
                        );
                    }
                    let value_ty = self.verify_value_use(function, block, store.value, defined);
                    if storage_ty.is_some() && value_ty.is_some() && storage_ty != value_ty {
                        self.block_error(function.id, block.id, "store operand type mismatch");
                    }
                }
            }
        }

        match &block.terminator {
            Some(MirTerminator::Return { value, .. }) => {
                if let Some(ty) = self.verify_value_use(function, block, *value, defined) {
                    if ty != function.return_type {
                        self.block_error(function.id, block.id, "return operand type mismatch");
                    }
                }
            }
            None => self.block_error(function.id, block.id, "block has no terminator"),
        }
    }

    fn verify_rvalue(
        &mut self,
        function: &MirFunction,
        block: &MirBasicBlock,
        rvalue: &MirRvalue,
        defined: &HashSet<ValueId>,
    ) {
        match &rvalue.kind {
            MirRvalueKind::ConstantI64(_) => {
                if rvalue.ty != MirType::I64 {
                    self.block_error(function.id, block.id, "integer constant is not `i64`");
                }
            }
            MirRvalueKind::Load(storage) => match function.storage(*storage) {
                Some(storage) if storage.ty != rvalue.ty => {
                    self.block_error(function.id, block.id, "load result type mismatch")
                }
                None => self.block_error(
                    function.id,
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
            MirRvalueKind::DirectCall {
                function: target_id,
                arguments,
            } => {
                let Some(target) = self.program.functions.get(*target_id) else {
                    self.block_error(
                        function.id,
                        block.id,
                        format!("call target {target_id} is not declared"),
                    );
                    return;
                };
                if arguments.len() != target.parameters.len() {
                    self.block_error(
                        function.id,
                        block.id,
                        format!(
                            "call to {target_id} has {} arguments but requires {}",
                            arguments.len(),
                            target.parameters.len()
                        ),
                    );
                }
                for (index, argument) in arguments.iter().enumerate() {
                    let argument_ty = self.verify_value_use(function, block, *argument, defined);
                    let parameter_ty = target
                        .parameters
                        .get(index)
                        .and_then(|id| target.storage(*id))
                        .map(|storage| storage.ty);
                    if argument_ty.is_some()
                        && parameter_ty.is_some()
                        && argument_ty != parameter_ty
                    {
                        self.block_error(
                            function.id,
                            block.id,
                            format!("call argument {index} type mismatch"),
                        );
                    }
                }
                if rvalue.ty != target.return_type {
                    self.block_error(function.id, block.id, "call result type mismatch");
                }
            }
        }
    }

    fn verify_i64_operand(
        &mut self,
        function: &MirFunction,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != MirType::I64 {
                self.block_error(function.id, block.id, "arithmetic operand is not `i64`");
            }
        }
    }

    fn verify_value_use(
        &mut self,
        function: &MirFunction,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) -> Option<MirType> {
        let Some(metadata) = function.value(value) else {
            self.block_error(
                function.id,
                block.id,
                format!("value {value} is not declared in this function"),
            );
            return None;
        };
        if !defined.contains(&value) {
            self.block_error(
                function.id,
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
