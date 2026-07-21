//! Fully typed HIR consumed by MIR lowering.

use std::borrow::Cow;

use crate::{
    function_table::{DenseFunctionTable, SparseFunctionTable},
    identity::{
        BindingId, CallableId, ClassId, FieldId, FunctionId, InitializerId, LocalId, MethodId,
        ParameterId,
    },
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Class(ClassId),
}

impl Type {
    pub fn name(self) -> Cow<'static, str> {
        match self {
            Self::I64 => Cow::Borrowed("i64"),
            Self::U64 => Cow::Borrowed("u64"),
            Self::U8 => Cow::Borrowed("u8"),
            Self::F64 => Cow::Borrowed("f64"),
            Self::Bool => Cow::Borrowed("bool"),
            Self::Unit => Cow::Borrowed("unit"),
            Self::Class(class) => Cow::Owned(format!("class {class}")),
        }
    }

    /// Returns the English indefinite article used before this type's name in
    /// diagnostics.
    pub const fn indefinite_article(self) -> &'static str {
        match self {
            Self::I64 => "an",
            Self::U64 | Self::U8 | Self::F64 | Self::Bool | Self::Unit | Self::Class(_) => "a",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProgram {
    pub classes: HirClassDeclarationTable,
    pub class_definitions: HirClassDefinitionTable,
    pub declarations: HirFunctionDeclarationTable,
    pub definitions: HirFunctionDefinitionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

impl HirProgram {
    pub fn class(&self, id: ClassId) -> Option<&HirClassDeclaration> {
        self.classes.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&HirFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&HirInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&HirMethodDeclaration> {
        self.class(id.class())?.method(id)
    }

    pub fn member_definition(&self, callable: CallableId) -> Option<&HirMemberDefinition> {
        self.class_definitions
            .get(callable.class()?)?
            .member(callable)
    }

    pub fn callable_signature(&self, callable: CallableId) -> Option<HirCallableSignature<'_>> {
        match callable {
            CallableId::Function(function) => {
                self.declarations
                    .get(function)
                    .map(|declaration| HirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: declaration.return_type,
                    })
            }
            CallableId::Initializer(initializer) => {
                self.initializer(initializer)
                    .map(|declaration| HirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: Type::Unit,
                    })
            }
            CallableId::Method(method) => {
                self.method(method).map(|declaration| HirCallableSignature {
                    parameters: &declaration.parameters,
                    return_type: declaration.return_type,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HirCallableSignature<'hir> {
    pub parameters: &'hir [HirParameter],
    pub return_type: Type,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirClassDeclarationTable {
    entries: Vec<HirClassDeclaration>,
}

impl HirClassDeclarationTable {
    pub(crate) fn new(entries: Vec<HirClassDeclaration>) -> Self {
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, class)| class.id.index() == index),
            "class declarations must be ordered by dense class ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: ClassId) -> Option<&HirClassDeclaration> {
        self.entries.get(id.index()).filter(|class| class.id == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirClassDeclaration> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassDeclaration {
    pub id: ClassId,
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<HirFieldDeclaration>,
    pub initializer: HirInitializerDeclaration,
    pub methods: Vec<HirMethodDeclaration>,
    pub span: Span,
}

impl HirClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&HirFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.fields.get(id.index()).filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&HirInitializerDeclaration> {
        (id.class() == self.id && self.initializer.id == id).then_some(&self.initializer)
    }

    pub fn method(&self, id: MethodId) -> Option<&HirMethodDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.methods
            .get(id.index())
            .filter(|method| method.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldDeclaration {
    pub id: FieldId,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInitializerDeclaration {
    pub id: InitializerId,
    pub parameters: Vec<HirParameter>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirAccess {
    ReadOnly,
    Mutable,
}

impl HirAccess {
    pub const fn permits(self, required: Self) -> bool {
        matches!(
            (self, required),
            (Self::Mutable, _) | (Self::ReadOnly, Self::ReadOnly)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMethodDeclaration {
    pub id: MethodId,
    pub name: String,
    pub name_span: Span,
    pub receiver_access: HirAccess,
    pub parameters: Vec<HirParameter>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirClassDefinitionTable {
    entries: Vec<HirClassDefinition>,
}

impl HirClassDefinitionTable {
    pub(crate) fn new(entries: Vec<HirClassDefinition>) -> Self {
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, class)| class.class.index() == index),
            "class definitions must be ordered by dense class ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: ClassId) -> Option<&HirClassDefinition> {
        self.entries
            .get(id.index())
            .filter(|definition| definition.class == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirClassDefinition> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassDefinition {
    pub class: ClassId,
    pub initializer: HirMemberDefinition,
    pub methods: Vec<HirMemberDefinition>,
    pub span: Span,
}

impl HirClassDefinition {
    pub fn member(&self, callable: CallableId) -> Option<&HirMemberDefinition> {
        match callable {
            CallableId::Function(_) => None,
            CallableId::Initializer(id) if id.class() == self.class => {
                (self.initializer.callable == callable).then_some(&self.initializer)
            }
            CallableId::Method(id) if id.class() == self.class => self
                .methods
                .get(id.index())
                .filter(|definition| definition.callable == callable),
            CallableId::Initializer(_) | CallableId::Method(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMemberDefinition {
    pub callable: CallableId,
    pub locals: Vec<HirLocal>,
    pub body: HirBlock,
    pub span: Span,
}

impl HirMemberDefinition {
    pub fn local(&self, id: LocalId) -> Option<&HirLocal> {
        if id.callable() != self.callable {
            return None;
        }
        self.locals.get(id.index()).filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionDeclarationTable {
    entries: DenseFunctionTable<HirFunctionDeclaration>,
}

impl HirFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<HirFunctionDeclaration>) -> Self {
        Self {
            entries: DenseFunctionTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&HirFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirFunctionDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionDeclaration {
    pub id: FunctionId,
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<HirParameter>,
    pub return_type: Type,
    pub linkage: HirFunctionLinkage,
    pub span: Span,
}

impl HirFunctionDeclaration {
    pub fn parameter(&self, id: ParameterId) -> Option<&HirParameter> {
        (id.callable() == self.id.into())
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFunctionLinkage {
    Internal,
    External { symbol: String },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionDefinitionTable {
    entries: SparseFunctionTable<HirFunctionDefinition>,
}

impl HirFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<HirFunctionDefinition>>) -> Self {
        Self {
            entries: SparseFunctionTable::new(entries, |definition| definition.function),
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&HirFunctionDefinition> {
        self.entries.get(function)
    }

    pub fn iter(&self) -> impl Iterator<Item = &HirFunctionDefinition> {
        self.entries.iter()
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionDefinition {
    pub function: FunctionId,
    pub locals: Vec<HirLocal>,
    pub body: HirBlock,
    pub span: Span,
}

impl HirFunctionDefinition {
    pub fn local(&self, id: LocalId) -> Option<&HirLocal> {
        (id.callable() == self.function.into())
            .then(|| self.locals.get(id.index()))
            .flatten()
            .filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirParameter {
    pub id: ParameterId,
    pub mode: HirParameterMode,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

impl HirParameterMode {
    pub const fn required_access(self) -> Option<HirAccess> {
        match self {
            Self::Value => None,
            Self::ReadOnlyAlias => Some(HirAccess::ReadOnly),
            Self::MutableAlias => Some(HirAccess::Mutable),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocal {
    pub id: LocalId,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

/// Whether execution can reach the end of a checked block or conditional.
///
/// `Terminates` currently means every path executes a `return`. The type
/// checker is the authority for this summary; later phases consume it rather
/// than reconstructing source-level control flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockFlow {
    FallsThrough,
    Terminates,
}

impl BlockFlow {
    pub(crate) const fn then(self, next: Self) -> Self {
        match self {
            Self::FallsThrough => next,
            Self::Terminates => Self::Terminates,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBlock {
    pub statements: Vec<HirStatement>,
    pub flow: BlockFlow,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStatement {
    Local(HirLocalDecl),
    Return(HirReturn),
    Call(HirCallStatement),
    Conditional(HirConditional),
    Block(HirBlock),
    FieldAssignment(HirFieldAssignment),
}

impl HirStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Call(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
            Self::Block(block) => block.span,
            Self::FieldAssignment(statement) => statement.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocalDecl {
    pub local: LocalId,
    pub initializer: HirLocalInitializer,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirLocalInitializer {
    Value(HirExpression),
    Construct(HirConstruction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConstruction {
    pub class: ClassId,
    pub initializer: InitializerId,
    pub arguments: Vec<HirCallArgument>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirReturn {
    pub value: Option<HirExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallStatement {
    pub call: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldAssignment {
    pub place: HirFieldPlace,
    pub value: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditional {
    pub arms: Vec<HirConditionalArm>,
    pub else_block: Option<HirBlock>,
    pub flow: BlockFlow,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditionalArm {
    pub condition: HirExpression,
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpression {
    pub kind: HirExpressionKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExpressionKind {
    Binding(BindingId),
    I64(i64),
    U64(u64),
    U8(u8),
    /// IEEE-754 binary64 payload, kept as raw bits for deterministic HIR.
    F64Bits(u64),
    Boolean(bool),
    Unary {
        operation: HirUnaryOperation,
        operand: Box<HirExpression>,
    },
    Binary {
        operation: HirBinaryOperation,
        left: Box<HirExpression>,
        right: Box<HirExpression>,
    },
    DirectCall {
        function: FunctionId,
        arguments: Vec<HirCallArgument>,
    },
    FieldRead(HirFieldPlace),
    MethodCall {
        receiver: HirObjectPlace,
        method: MethodId,
        arguments: Vec<HirCallArgument>,
    },
    Grouped(Box<HirExpression>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirCallArgument {
    Value(HirExpression),
    Place(HirObjectPlace),
}

impl HirCallArgument {
    pub const fn span(&self) -> Span {
        match self {
            Self::Value(expression) => expression.span,
            Self::Place(place) => place.span,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirObjectPlace {
    pub binding: BindingId,
    pub class: ClassId,
    pub access: HirAccess,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirFieldPlace {
    pub receiver: HirObjectPlace,
    pub field: FieldId,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirUnaryOperation {
    NegateI64,
    NegateF64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirBinaryOperation {
    AddI64,
    SubtractI64,
    MultiplyI64,
    AddU64,
    SubtractU64,
    MultiplyU64,
    AddU8,
    SubtractU8,
    MultiplyU8,
    AddF64,
    SubtractF64,
    MultiplyF64,
}
