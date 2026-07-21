//! Structural and type verifier for MIR.

use std::{collections::HashSet, fmt};

use crate::identity::{BindingId, CallableId};

use super::model::*;

mod cleanup;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVerificationError {
    pub callable: Option<CallableId>,
    pub block: Option<BlockId>,
    pub message: String,
}

impl fmt::Display for MirVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.callable, self.block) {
            (_, Some(block)) => write!(formatter, "MIR {block}: {}", self.message),
            (Some(callable), None) => write!(formatter, "MIR {callable}: {}", self.message),
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

#[derive(Clone, Copy)]
struct VerifiedPlace {
    ty: MirType,
    access: MirAliasAccess,
}

impl Verifier<'_> {
    fn verify(&mut self) {
        self.verify_classes();
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
            if entry_declaration.is_some_and(|declaration| {
                !declaration.parameters.is_empty() || declaration.return_type != MirType::I64
            }) {
                self.program_error("entry function must have signature `fn main() -> i64`");
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
            self.verify_parameters_declaration(
                &format!("function {}", declaration.id),
                &declaration.parameters,
            );
            if matches!(declaration.return_type, MirType::Class(_)) {
                self.function_error(
                    declaration.id,
                    "function results cannot have class type in this MIR profile",
                );
            }
            if let MirFunctionLinkage::External { symbol } = &declaration.linkage {
                if declaration
                    .parameters
                    .iter()
                    .any(|parameter| parameter.mode != MirParameterMode::Value)
                {
                    self.function_error(
                        declaration.id,
                        "external function cannot declare alias parameters",
                    );
                }
                if symbol != &declaration.name || !is_source_identifier(symbol) {
                    self.function_error(
                        declaration.id,
                        "external symbol must be the declaration's exact source identifier",
                    );
                }
            }
        }

        let mut defined_functions = HashSet::new();
        for (index, definition) in self.program.definitions.indexed_slots() {
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
            self.verify_definition(
                &declaration.parameters,
                declaration.return_type,
                definition.into(),
            );
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

        for (table_key, definition) in self.program.member_definitions.indexed_entries() {
            let callable = definition.callable;
            if table_key != callable {
                self.function_error(
                    callable,
                    format!("member definition table key {table_key} contains {callable}"),
                );
            }
            let signature = match callable {
                CallableId::Initializer(initializer) => self
                    .program
                    .initializer(initializer)
                    .map(|declaration| (&declaration.parameters[..], MirType::Unit)),
                CallableId::Destructor(destructor) => self
                    .program
                    .destructor(destructor)
                    .map(|_| (&[][..], MirType::Unit)),
                CallableId::Method(method) => self
                    .program
                    .method(method)
                    .map(|declaration| (&declaration.parameters[..], declaration.return_type)),
                CallableId::Function(_) => None,
            };
            let Some((parameters, return_type)) = signature else {
                self.function_error(callable, "member definition has no matching declaration");
                continue;
            };
            self.verify_definition(parameters, return_type, definition.into());
        }
    }

    fn verify_classes(&mut self) {
        for (class_index, class) in self.program.classes.iter().enumerate() {
            if class.id.index() != class_index {
                self.program_error(format!(
                    "class declaration table index {class_index} contains {}",
                    class.id
                ));
            }
            for (index, field) in class.fields.iter().enumerate() {
                if field.id.class() != class.id || field.id.index() != index {
                    self.program_error(format!(
                        "class {} field table index {index} contains {}",
                        class.id, field.id
                    ));
                }
                match field.ty {
                    MirType::Unit => self.program_error(format!(
                        "field {} cannot have payload-free type `unit`",
                        field.id
                    )),
                    MirType::Class(target) if self.program.class(target).is_none() => {
                        self.program_error(format!(
                            "field {} has undeclared class type {target}",
                            field.id
                        ));
                    }
                    _ => {}
                }
            }
            for (index, initializer) in class.initializers.iter().enumerate() {
                if initializer.id.class() != class.id || initializer.id.index() != index {
                    self.program_error(format!(
                        "class {} initializer table index {index} contains {}",
                        class.id, initializer.id
                    ));
                }
                self.verify_member_parameters(
                    &format!("initializer {}", initializer.id),
                    &initializer.parameters,
                );
            }
            if let Some(destructor) = &class.destruction.destructor {
                if destructor.id.class() != class.id || destructor.id.index() != 0 {
                    self.program_error(format!(
                        "class {} destructor declaration contains {}",
                        class.id, destructor.id
                    ));
                }
                if destructor.receiver_access != MirReceiverAccess::Mutable {
                    self.program_error(format!(
                        "destructor {} must have mutable receiver access",
                        destructor.id
                    ));
                }
                if self
                    .program
                    .member_definition(destructor.id.into())
                    .is_none()
                {
                    self.program_error(format!(
                        "destructor {} has no member definition",
                        destructor.id
                    ));
                }
            }
            let class_fields: Vec<_> = class
                .fields
                .iter()
                .filter_map(|field| matches!(field.ty, MirType::Class(_)).then_some(field.id))
                .collect();
            let expected_plan =
                MirDestructionPlan::new(class.destruction.destructor.clone(), &class_fields);
            if class.destruction.steps != expected_plan.steps {
                self.program_error(format!(
                    "class {} destruction plan must run its user body first and class fields in reverse declaration order",
                    class.id
                ));
            }
            for (index, method) in class.methods.iter().enumerate() {
                if method.id.class() != class.id || method.id.index() != index {
                    self.program_error(format!(
                        "class {} method table index {index} contains {}",
                        class.id, method.id
                    ));
                }
                self.verify_member_parameters(&format!("method {}", method.id), &method.parameters);
                if matches!(method.return_type, MirType::Class(_)) {
                    self.program_error(format!(
                        "method {} result cannot have class type in this MIR profile",
                        method.id
                    ));
                }
            }
        }
    }

    fn verify_parameters_declaration(&mut self, owner: &str, parameters: &[MirParameter]) {
        for (index, parameter) in parameters.iter().enumerate() {
            match parameter.mode {
                MirParameterMode::Value if !parameter.ty.is_scalar_value() => self.program_error(
                    format!("{owner} value parameter {index} must have scalar value type"),
                ),
                MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias
                    if !matches!(parameter.ty, MirType::Class(_)) =>
                {
                    self.program_error(format!(
                        "{owner} alias parameter {index} must have class type"
                    ));
                }
                _ => {}
            }
            if let MirType::Class(class) = parameter.ty {
                if self.program.class(class).is_none() {
                    self.program_error(format!(
                        "{owner} parameter {index} has undeclared class type {class}"
                    ));
                }
            }
        }
    }

    fn verify_member_parameters(&mut self, owner: &str, parameters: &[MirParameter]) {
        self.verify_parameters_declaration(owner, parameters);
    }

    fn verify_definition(
        &mut self,
        parameters: &[MirParameter],
        return_type: MirType,
        function: MirDefinitionRef<'_>,
    ) {
        self.verify_storage(function);
        self.verify_values(function);
        self.verify_receiver(function);
        self.verify_parameters(parameters, function);

        if function.body().entry.callable() != function.callable() {
            self.function_error(
                function.callable(),
                format!(
                    "entry block {} is owned by another callable body",
                    function.body().entry
                ),
            );
        } else if function.block(function.body().entry).is_none() {
            self.function_error(
                function.callable(),
                format!("entry block {} is not declared", function.body().entry),
            );
        }

        let mut defined_values = HashSet::new();
        let mut seen_blocks = HashSet::new();
        for (index, block) in function.body().blocks.iter().enumerate() {
            if block.id.callable() != function.callable() {
                self.block_error(
                    function.callable(),
                    block.id,
                    "block is owned by another callable body",
                );
            }
            if block.id.index() != index {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("block table index {index} contains {}", block.id),
                );
            }
            if !seen_blocks.insert(block.id) {
                self.block_error(function.callable(), block.id, "duplicate block ID");
            }
            self.verify_block(return_type, function, block, &mut defined_values);
        }
        for error in cleanup::analyze(self.program, function) {
            self.block_error(function.callable(), error.block, error.message);
        }

        for value in function.values() {
            if !defined_values.contains(&value.id) {
                self.function_error(
                    function.callable(),
                    format!("value {} has no definition", value.id),
                );
            }
        }
    }

    fn verify_storage(&mut self, function: MirDefinitionRef<'_>) {
        let mut sources = HashSet::new();
        for (index, storage) in function.storage_entries().iter().enumerate() {
            if storage.id.callable() != function.callable() {
                self.function_error(
                    function.callable(),
                    format!("storage {} is owned by another callable body", storage.id),
                );
            }
            if storage.id.index() != index {
                self.function_error(
                    function.callable(),
                    format!("storage table index {index} contains {}", storage.id),
                );
            }
            if storage.source.callable() != function.callable() {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} has a source binding from another callable body",
                        storage.id
                    ),
                );
            }
            if !sources.insert(storage.source) {
                self.function_error(
                    function.callable(),
                    format!(
                        "source binding {} has multiple storage slots",
                        storage.source
                    ),
                );
            }
            let source_matches_kind = matches!(
                (storage.kind, storage.source),
                (MirStorageKind::Receiver, BindingId::Receiver(_))
                    | (MirStorageKind::Parameter, BindingId::Parameter(_))
                    | (MirStorageKind::AliasParameter(_), BindingId::Parameter(_))
                    | (MirStorageKind::Local, BindingId::Local(_))
            );
            if !source_matches_kind {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} kind does not match its source binding",
                        storage.id
                    ),
                );
            }
            if storage.ty == MirType::Unit {
                self.function_error(
                    function.callable(),
                    format!(
                        "storage {} cannot have payload-free type `unit`",
                        storage.id
                    ),
                );
            }
            if let MirType::Class(class) = storage.ty {
                if self.program.class(class).is_none() {
                    self.function_error(
                        function.callable(),
                        format!("storage {} has undeclared class type {class}", storage.id),
                    );
                }
            }
        }
    }

    fn verify_values(&mut self, function: MirDefinitionRef<'_>) {
        for (index, value) in function.values().iter().enumerate() {
            if value.id.callable() != function.callable() {
                self.function_error(
                    function.callable(),
                    format!("value {} is owned by another callable body", value.id),
                );
            }
            if value.id.index() != index {
                self.function_error(
                    function.callable(),
                    format!("value table index {index} contains {}", value.id),
                );
            }
            if !value.ty.is_scalar_value() {
                self.function_error(
                    function.callable(),
                    format!("value {} must have a scalar value type", value.id),
                );
            }
        }
    }

    fn verify_parameters(&mut self, parameters: &[MirParameter], function: MirDefinitionRef<'_>) {
        if function.parameters().len() != parameters.len() {
            self.function_error(
                function.callable(),
                format!(
                    "definition has {} parameters but declaration requires {}",
                    function.parameters().len(),
                    parameters.len()
                ),
            );
        }
        let mut seen = HashSet::new();
        for (index, parameter) in function.parameters().iter().enumerate() {
            let Some(storage) = function.storage(*parameter) else {
                self.function_error(
                    function.callable(),
                    format!("parameter storage {parameter} is not declared"),
                );
                continue;
            };
            if !seen.insert(*parameter) {
                self.function_error(
                    function.callable(),
                    format!("duplicate parameter storage {parameter}"),
                );
            }
            if !matches!(storage.source, BindingId::Parameter(_)) {
                self.function_error(
                    function.callable(),
                    format!("parameter {parameter} does not identify parameter storage"),
                );
            }
            if !matches!(storage.source, BindingId::Parameter(id) if id.index() == index) {
                self.function_error(
                    function.callable(),
                    format!("parameter position {index} has mismatched source binding"),
                );
            }
            let Some(descriptor) = parameters.get(index) else {
                continue;
            };
            let expected_kind = match descriptor.mode {
                MirParameterMode::Value => MirStorageKind::Parameter,
                MirParameterMode::ReadOnlyAlias => {
                    MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly)
                }
                MirParameterMode::MutableAlias => {
                    MirStorageKind::AliasParameter(MirAliasAccess::Mutable)
                }
            };
            if storage.kind != expected_kind {
                self.function_error(
                    function.callable(),
                    format!("parameter position {index} storage mode differs from declaration"),
                );
            }
            if descriptor.ty != storage.ty {
                self.function_error(
                    function.callable(),
                    format!("parameter position {index} type differs from declaration"),
                );
            }
        }
        for storage in function.storage_entries().iter().filter(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Parameter | MirStorageKind::AliasParameter(_)
            )
        }) {
            if !seen.contains(&storage.id) {
                self.function_error(
                    function.callable(),
                    format!(
                        "parameter storage {} is not listed by the definition",
                        storage.id
                    ),
                );
            }
        }
    }

    fn verify_receiver(&mut self, function: MirDefinitionRef<'_>) {
        let receiver_slots: Vec<_> = function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Receiver)
            .collect();
        let Some(receiver) = function.receiver() else {
            if !receiver_slots.is_empty() {
                self.function_error(
                    function.callable(),
                    "top-level function cannot declare receiver storage",
                );
            }
            return;
        };
        let Some(storage) = function.storage(receiver) else {
            self.function_error(
                function.callable(),
                format!("receiver storage {receiver} is not declared"),
            );
            return;
        };
        if receiver_slots.len() != 1
            || storage.kind != MirStorageKind::Receiver
            || storage.source != BindingId::Receiver(function.callable())
        {
            self.function_error(
                function.callable(),
                "member definition must identify exactly one receiver storage slot",
            );
        }
        let expected = function
            .callable()
            .class()
            .map(MirType::Class)
            .expect("member callable has a class owner");
        if storage.ty != expected {
            self.function_error(
                function.callable(),
                "receiver storage has the wrong class type",
            );
        }
    }

    fn verify_block(
        &mut self,
        return_type: MirType,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        defined_values: &mut HashSet<ValueId>,
    ) {
        // MIR transient values are deliberately block-local before SSA. A
        // separate set per block prevents vector order from accidentally
        // permitting values to cross control-flow edges.
        let mut defined_in_block = HashSet::new();
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::Assign(assignment) => {
                    let Some(result) = function.value(assignment.result) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("assignment result {} is not declared", assignment.result),
                        );
                        continue;
                    };
                    if defined_values.contains(&assignment.result) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("value {} is defined more than once", assignment.result),
                        );
                    }
                    if result.ty != assignment.rvalue.ty {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("assignment type does not match value {}", assignment.result),
                        );
                    }
                    self.verify_rvalue(function, block, &assignment.rvalue, &defined_in_block);
                    defined_values.insert(assignment.result);
                    defined_in_block.insert(assignment.result);
                }
                MirInstruction::Call(call) => {
                    self.verify_call(function, block, call, defined_values, &mut defined_in_block);
                }
                MirInstruction::Cleanup(cleanup) => {
                    let destination = self.verify_place(function, block, &cleanup.destination);
                    if matches!(cleanup.destination.base, MirPlaceBase::AliasParameter(_)) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "cleanup destination must be owning storage",
                        );
                    }
                    if self.program.class(cleanup.target).is_none() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("cleanup target {} is not declared", cleanup.target),
                        );
                    }
                    match destination.map(|place| place.ty) {
                        Some(MirType::Class(class)) if class != cleanup.target => {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "cleanup destination has the wrong class type",
                            );
                        }
                        Some(MirType::Class(_)) => {}
                        Some(_) => self.block_error(
                            function.callable(),
                            block.id,
                            "cleanup destination must have class type",
                        ),
                        None => {}
                    }
                    if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "cleanup destination requires mutable access",
                        );
                    }
                }
                MirInstruction::Initialize(initialize) => {
                    let destination = self.verify_place(function, block, &initialize.destination);
                    if matches!(initialize.destination.base, MirPlaceBase::AliasParameter(_)) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "initializer destination must be owning storage",
                        );
                    }
                    let Some(target) = self.program.initializer(initialize.target) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("initializer target {} is not declared", initialize.target),
                        );
                        continue;
                    };
                    if destination.map(|place| place.ty)
                        != Some(MirType::Class(initialize.target.class()))
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "initializer destination has the wrong class type",
                        );
                    }
                    if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "initializer destination requires mutable access",
                        );
                    }
                    self.verify_arguments(
                        function,
                        block,
                        "initializer",
                        &initialize.arguments,
                        &target.parameters,
                        &defined_in_block,
                    );
                }
                MirInstruction::Store(store) => {
                    let destination = self.verify_place(function, block, &store.destination);
                    let storage_ty = destination.map(|place| place.ty);
                    let value_ty =
                        self.verify_value_use(function, block, store.value, &defined_in_block);
                    if storage_ty.is_some_and(|ty| !ty.is_scalar_value()) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "store destination must have scalar value type",
                        );
                    }
                    if storage_ty.is_some() && value_ty.is_some() && storage_ty != value_ty {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "store operand type mismatch",
                        );
                    }
                    if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "store destination requires mutable access",
                        );
                    }
                }
            }
        }

        match &block.terminator {
            Some(MirTerminator::Return { value, .. }) => {
                if let Some(value) = value {
                    if let Some(ty) =
                        self.verify_value_use(function, block, *value, &defined_in_block)
                    {
                        if return_type == MirType::Unit {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "unit function return must not have an operand",
                            );
                        } else if ty != return_type {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "return operand type mismatch",
                            );
                        }
                    }
                } else if return_type != MirType::Unit {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "value-returning function return has no operand",
                    );
                }
            }
            Some(MirTerminator::Goto { target, .. }) => {
                self.verify_block_target(function, block, *target);
            }
            Some(MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            }) => {
                if let Some(ty) =
                    self.verify_value_use(function, block, *condition, &defined_in_block)
                {
                    if ty != MirType::Bool {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "branch condition is not `bool`",
                        );
                    }
                }
                self.verify_block_target(function, block, *true_target);
                self.verify_block_target(function, block, *false_target);
            }
            None => self.block_error(function.callable(), block.id, "block has no terminator"),
        }
    }

    fn verify_block_target(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: BlockId,
    ) {
        if target.callable() != function.callable() {
            self.block_error(
                function.callable(),
                block.id,
                format!("control-flow target {target} is owned by another callable body"),
            );
        } else if function.block(target).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!("control-flow target {target} is not declared"),
            );
        }
    }

    fn verify_call(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        let arguments_defined = defined_in_block.clone();
        let result_ty = match call.result {
            Some(result) => {
                let metadata = function.value(result);
                if metadata.is_none() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("call result {result} is not declared"),
                    );
                }
                if !defined_values.insert(result) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("value {result} is defined more than once"),
                    );
                }
                defined_in_block.insert(result);
                metadata.map(|metadata| metadata.ty)
            }
            None => None,
        };

        let (parameters, return_type) = match call.target {
            MirCallTarget::Direct(target_id) => {
                if call.receiver.is_some() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "ordinary function call must not have a receiver",
                    );
                }
                let Some(target) = self.program.declarations.get(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("call target {target_id} is not declared"),
                    );
                    return;
                };
                (&target.parameters, target.return_type)
            }
            MirCallTarget::Method(target_id) => {
                let Some(target) = self.program.method(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("method target {target_id} is not declared"),
                    );
                    return;
                };
                match &call.receiver {
                    Some(receiver) => {
                        let receiver = self.verify_place(function, block, receiver);
                        if receiver.map(|place| place.ty) != Some(MirType::Class(target_id.class()))
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "method receiver has the wrong class type",
                            );
                        }
                        if target.receiver_access == MirReceiverAccess::Mutable
                            && receiver.is_some_and(|place| place.access != MirAliasAccess::Mutable)
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "mutable method receiver requires mutable access",
                            );
                        }
                    }
                    None => self.block_error(
                        function.callable(),
                        block.id,
                        "method call requires a receiver",
                    ),
                }
                (&target.parameters, target.return_type)
            }
        };
        self.verify_arguments(
            function,
            block,
            "call",
            &call.arguments,
            parameters,
            &arguments_defined,
        );

        match (return_type, result_ty) {
            (MirType::Unit, Some(_)) => self.block_error(
                function.callable(),
                block.id,
                "unit-returning call must not have a result",
            ),
            (MirType::Unit, None) => {}
            (_, Some(result_ty)) if result_ty != return_type => {
                self.block_error(function.callable(), block.id, "call result type mismatch")
            }
            (_, None) => self.block_error(
                function.callable(),
                block.id,
                "value-returning call has no result",
            ),
            _ => {}
        }
    }

    fn verify_rvalue(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        rvalue: &MirRvalue,
        defined: &HashSet<ValueId>,
    ) {
        match &rvalue.kind {
            MirRvalueKind::ConstantI64(_) => {
                if rvalue.ty != MirType::I64 {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "integer constant is not `i64`",
                    );
                }
            }
            MirRvalueKind::ConstantU64(_) => {
                if rvalue.ty != MirType::U64 {
                    self.block_error(function.callable(), block.id, "u64 constant is not `u64`");
                }
            }
            MirRvalueKind::ConstantU8(_) => {
                if rvalue.ty != MirType::U8 {
                    self.block_error(function.callable(), block.id, "u8 constant is not `u8`");
                }
            }
            MirRvalueKind::ConstantF64Bits(_) => {
                if rvalue.ty != MirType::F64 {
                    self.block_error(function.callable(), block.id, "f64 constant is not `f64`");
                }
            }
            MirRvalueKind::ConstantBool(_) => {
                if rvalue.ty != MirType::Bool {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "boolean constant is not `bool`",
                    );
                }
            }
            MirRvalueKind::Load(place) => {
                let place_ty = self
                    .verify_place(function, block, place)
                    .map(|place| place.ty);
                if place_ty.is_some_and(|ty| !ty.is_scalar_value()) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "load source must have scalar value type",
                    );
                }
                if place_ty.is_some() && place_ty != Some(rvalue.ty) {
                    self.block_error(function.callable(), block.id, "load result type mismatch");
                }
            }
            MirRvalueKind::Unary { operation, operand } => {
                let expected = operation.operand_type();
                if rvalue.ty != expected {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "unary operation result type mismatch",
                    );
                }
                self.verify_arithmetic_operand(function, block, *operand, expected, defined);
            }
            MirRvalueKind::Binary {
                operation,
                left,
                right,
            } => {
                let expected = operation.operand_type();
                if rvalue.ty != expected {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "binary operation result type mismatch",
                    );
                }
                self.verify_arithmetic_operand(function, block, *left, expected, defined);
                self.verify_arithmetic_operand(function, block, *right, expected, defined);
            }
        }
    }

    fn verify_arithmetic_operand(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != expected {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("arithmetic operand is not `{expected}`"),
                );
            }
        }
    }

    fn verify_arguments(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        kind: &str,
        arguments: &[MirArgument],
        parameters: &[MirParameter],
        defined: &HashSet<ValueId>,
    ) {
        if arguments.len() != parameters.len() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "{kind} has {} arguments but requires {}",
                    arguments.len(),
                    parameters.len()
                ),
            );
        }
        for (index, argument) in arguments.iter().enumerate() {
            let Some(parameter) = parameters.get(index) else {
                match argument {
                    MirArgument::Value(value) => {
                        self.verify_value_use(function, block, *value, defined);
                    }
                    MirArgument::Place(place) => {
                        self.verify_place(function, block, place);
                    }
                }
                continue;
            };
            match (argument, parameter.mode) {
                (MirArgument::Value(value), MirParameterMode::Value) => {
                    let argument_ty = self.verify_value_use(function, block, *value, defined);
                    if argument_ty.is_some() && argument_ty != Some(parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                }
                (MirArgument::Place(place), MirParameterMode::ReadOnlyAlias)
                | (MirArgument::Place(place), MirParameterMode::MutableAlias) => {
                    let argument = self.verify_place(function, block, place);
                    if argument.is_some_and(|argument| argument.ty != parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                    if parameter.mode == MirParameterMode::MutableAlias
                        && argument
                            .is_some_and(|argument| argument.access != MirAliasAccess::Mutable)
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} requires mutable access"),
                        );
                    }
                }
                (MirArgument::Value(value), _) => {
                    self.verify_value_use(function, block, *value, defined);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} must be a place"),
                    );
                }
                (MirArgument::Place(place), MirParameterMode::Value) => {
                    self.verify_place(function, block, place);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} must be a value"),
                    );
                }
            }
        }
    }

    fn verify_place(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<VerifiedPlace> {
        let storage_id = place.base.storage();
        let Some(storage) = function.storage(storage_id) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("place base {storage_id} is not declared in this function"),
            );
            return None;
        };
        let access = match (place.base, storage.kind) {
            (MirPlaceBase::Storage(_), MirStorageKind::AliasParameter(_)) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("alias parameter storage {storage_id} requires an indirect base"),
                );
                return None;
            }
            (MirPlaceBase::AliasParameter(_), MirStorageKind::AliasParameter(access)) => access,
            (MirPlaceBase::AliasParameter(_), _) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("indirect alias base {storage_id} is not alias parameter storage"),
                );
                return None;
            }
            (MirPlaceBase::Storage(_), _) => self.storage_access(function, storage),
        };
        let mut ty = storage.ty;
        for projection in &place.projections {
            match *projection {
                MirPlaceProjection::Field(field_id) => {
                    let MirType::Class(owner) = ty else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} has a non-class base"),
                        );
                        return None;
                    };
                    if field_id.class() != owner {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} belongs to the wrong class"),
                        );
                        return None;
                    }
                    let Some(field) = self.program.field(field_id) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} is not declared"),
                        );
                        return None;
                    };
                    ty = field.ty;
                }
            }
        }
        Some(VerifiedPlace { ty, access })
    }

    fn storage_access(
        &self,
        function: MirDefinitionRef<'_>,
        storage: &MirStorage,
    ) -> MirAliasAccess {
        if storage.kind != MirStorageKind::Receiver {
            return MirAliasAccess::Mutable;
        }
        match function.callable() {
            CallableId::Method(method) => match self
                .program
                .method(method)
                .map(|method| method.receiver_access)
            {
                Some(MirReceiverAccess::ReadOnly) => MirAliasAccess::ReadOnly,
                Some(MirReceiverAccess::Mutable) => MirAliasAccess::Mutable,
                None => MirAliasAccess::ReadOnly,
            },
            CallableId::Initializer(_) | CallableId::Destructor(_) => MirAliasAccess::Mutable,
            CallableId::Function(_) => MirAliasAccess::ReadOnly,
        }
    }

    fn verify_value_use(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) -> Option<MirType> {
        let Some(metadata) = function.value(value) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is not declared in this function"),
            );
            return None;
        };
        if !defined.contains(&value) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is used before it is defined in this block"),
            );
        }
        Some(metadata.ty)
    }

    fn program_error(&mut self, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            callable: None,
            block: None,
            message: message.into(),
        });
    }

    fn function_error(&mut self, callable: impl Into<CallableId>, message: impl Into<String>) {
        self.errors.push(MirVerificationError {
            callable: Some(callable.into()),
            block: None,
            message: message.into(),
        });
    }

    fn block_error(
        &mut self,
        callable: impl Into<CallableId>,
        block: BlockId,
        message: impl Into<String>,
    ) {
        self.errors.push(MirVerificationError {
            callable: Some(callable.into()),
            block: Some(block),
            message: message.into(),
        });
    }
}

fn is_source_identifier(symbol: &str) -> bool {
    let mut bytes = symbol.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
