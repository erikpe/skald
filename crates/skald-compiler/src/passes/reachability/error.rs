//! Structured failures from target-independent dependency extraction.

use crate::{
    identity::{
        ArrayTypeId, CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId,
        FieldId, FunctionId, FunctionTypeId, InitializerId, InterfaceRequirementId, MethodId,
        OptionalTypeId, StaticFieldId, VirtualFamilyId,
    },
    mir::{MirPlaceBase, StorageId},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirDependencyExtractionError {
    UnknownFunction(FunctionId),
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
    UnknownClass(ClassId),
    UnknownArrayType(ArrayTypeId),
    UnknownField(FieldId),
    UnknownStaticField(StaticFieldId),
    UnknownOptionalType(OptionalTypeId),
    UnknownStorage(StorageId),
    InvalidPlaceBase(MirPlaceBase),
    InvalidLifecycleFieldType(FieldId),
}
