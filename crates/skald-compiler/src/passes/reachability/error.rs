//! Structured failures from target-independent dependency extraction.

use std::fmt;

use crate::{
    identity::{
        ArrayTypeId, CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId,
        FieldId, FunctionId, FunctionTypeId, InitializerId, InterfaceRequirementId, MethodId,
        OptionalBoxTypeId, OptionalTypeId, StaticFieldId, VirtualFamilyId,
    },
    mir::{MirExecutionNode, MirPlaceBase, StorageId},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirDependencyExtractionError {
    UnknownFunction(FunctionId),
    NonInternalEntry(FunctionId),
    UnknownMethod(MethodId),
    UnknownVirtualFamily(VirtualFamilyId),
    UnknownInterfaceRequirement(InterfaceRequirementId),
    UnknownCallable(CallableId),
    UnknownFunctionType(FunctionTypeId),
    UnknownInitializer(InitializerId),
    UnknownCopyConstructor(CopyConstructorId),
    UnknownCopyAssignment(CopyAssignmentId),
    UnknownDestructor(DestructorId),
    CallableFunctionTypeMismatch {
        callable: CallableId,
        function_type: FunctionTypeId,
    },
    NonInternalCallableAddress(CallableId),
    UnknownClass(ClassId),
    UnknownArrayType(ArrayTypeId),
    UnknownField(FieldId),
    UnknownStaticField(StaticFieldId),
    UnknownOptionalType(OptionalTypeId),
    UnknownOptionalBoxType(OptionalBoxTypeId),
    UnknownStorage(StorageId),
    InvalidPlaceBase(MirPlaceBase),
    InvalidStaticLifecycleDestination {
        source: CallableId,
        field: StaticFieldId,
    },
    InvalidLifecycleFieldType(FieldId),
    InvalidStaticCleanup(StaticFieldId),
    CyclicOptionalLifecycle(OptionalTypeId),
    MissingReachabilityExplanation(MirExecutionNode),
}

impl fmt::Display for MirDependencyExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFunction(function) => write!(formatter, "unknown function {function}"),
            Self::NonInternalEntry(function) => {
                write!(formatter, "entry function {function} is not internal")
            }
            Self::UnknownMethod(method) => write!(formatter, "unknown method {method}"),
            Self::UnknownVirtualFamily(family) => {
                write!(formatter, "unknown virtual family {family}")
            }
            Self::UnknownInterfaceRequirement(requirement) => {
                write!(formatter, "unknown interface requirement {requirement}")
            }
            Self::UnknownCallable(callable) => write!(formatter, "unknown callable {callable}"),
            Self::UnknownFunctionType(function_type) => {
                write!(formatter, "unknown function type {function_type}")
            }
            Self::UnknownInitializer(initializer) => {
                write!(formatter, "unknown initializer {initializer}")
            }
            Self::UnknownCopyConstructor(copy) => {
                write!(formatter, "unknown copy constructor {copy}")
            }
            Self::UnknownCopyAssignment(copy) => {
                write!(formatter, "unknown copy assignment {copy}")
            }
            Self::UnknownDestructor(destructor) => {
                write!(formatter, "unknown destructor {destructor}")
            }
            Self::CallableFunctionTypeMismatch {
                callable,
                function_type,
            } => write!(
                formatter,
                "callable {callable} does not match function type {function_type}"
            ),
            Self::NonInternalCallableAddress(callable) => {
                write!(
                    formatter,
                    "callable address target {callable} is not internal"
                )
            }
            Self::UnknownClass(class) => write!(formatter, "unknown class {class}"),
            Self::UnknownArrayType(array) => write!(formatter, "unknown array type {array}"),
            Self::UnknownField(field) => write!(formatter, "unknown field {field}"),
            Self::UnknownStaticField(field) => write!(formatter, "unknown static field {field}"),
            Self::UnknownOptionalType(optional) => {
                write!(formatter, "unknown optional type {optional}")
            }
            Self::UnknownOptionalBoxType(optional_box) => {
                write!(formatter, "unknown optional-box type {optional_box}")
            }
            Self::UnknownStorage(storage) => write!(formatter, "unknown storage {storage}"),
            Self::InvalidPlaceBase(base) => {
                write!(formatter, "invalid dependency place base {base:?}")
            }
            Self::InvalidStaticLifecycleDestination { source, field } => write!(
                formatter,
                "static lifecycle destination {field} is invalid in source callable {source}"
            ),
            Self::InvalidLifecycleFieldType(field) => {
                write!(formatter, "field {field} has an invalid lifecycle type")
            }
            Self::InvalidStaticCleanup(field) => {
                write!(
                    formatter,
                    "static field {field} has an invalid cleanup plan"
                )
            }
            Self::CyclicOptionalLifecycle(optional) => {
                write!(formatter, "optional lifecycle for {optional} is cyclic")
            }
            Self::MissingReachabilityExplanation(node) => {
                write!(
                    formatter,
                    "reachable dependency source {node:?} has no explanation"
                )
            }
        }
    }
}

impl std::error::Error for MirDependencyExtractionError {}
