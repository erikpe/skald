//! Stable target-independent identities for executable MIR work.

use crate::identity::{ArrayTypeId, CallableId, ClassId};

/// Compiler-defined class lifecycle work that may select executable bodies.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirClassLifecycleOperation {
    CopyConstructor,
    CopyAssignment,
    CompleteFinalizer,
}

/// Compiler-defined array lifecycle work that may select executable bodies.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirArrayLifecycleOperation {
    Default,
    Copy,
    Assignment,
    Destruction,
}

/// Stable semantic identity of a callable or implicit lifecycle operation.
///
/// An execution node describes work that may cause other executable work. It
/// is not a declaration, a physically retained body, a whole-program root, or
/// a target artifact. Source spans are deliberately evidence rather than part
/// of this identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirExecutionNode {
    Callable(CallableId),
    ClassLifecycle {
        class: ClassId,
        operation: MirClassLifecycleOperation,
    },
    ArrayLifecycle {
        array: ArrayTypeId,
        operation: MirArrayLifecycleOperation,
    },
}

impl MirExecutionNode {
    pub const fn callable(callable: CallableId) -> Self {
        Self::Callable(callable)
    }

    pub const fn class(class: ClassId, operation: MirClassLifecycleOperation) -> Self {
        Self::ClassLifecycle { class, operation }
    }

    pub const fn array(array: ArrayTypeId, operation: MirArrayLifecycleOperation) -> Self {
        Self::ArrayLifecycle { array, operation }
    }
}

/// Canonical semantic ordering independent of private graph storage.
///
/// The exhaustive matches are an intentional maintenance point: adding a
/// callable or lifecycle category must define its dependency-analysis order in
/// the same change.
pub(crate) const fn mir_execution_node_key(node: MirExecutionNode) -> (u8, usize, usize, usize) {
    match node {
        MirExecutionNode::Callable(callable) => callable_key(callable),
        MirExecutionNode::ClassLifecycle { class, operation } => {
            (7, class.index(), class_lifecycle_key(operation), 0)
        }
        MirExecutionNode::ArrayLifecycle { array, operation } => {
            (8, array.index(), array_lifecycle_key(operation), 0)
        }
    }
}

const fn callable_key(callable: CallableId) -> (u8, usize, usize, usize) {
    match callable {
        CallableId::Function(function) => (0, function.index(), 0, 0),
        CallableId::StaticInitializer(initializer) => {
            let field = initializer.field();
            (1, field.class().index(), field.index(), 0)
        }
        CallableId::Initializer(initializer) => {
            (2, initializer.class().index(), initializer.index(), 0)
        }
        CallableId::CopyConstructor(copy) => (3, copy.class().index(), copy.index(), 0),
        CallableId::CopyAssignment(assignment) => {
            (4, assignment.class().index(), assignment.index(), 0)
        }
        CallableId::Destructor(destructor) => {
            (5, destructor.class().index(), destructor.index(), 0)
        }
        CallableId::Method(method) => (6, method.class().index(), method.index(), 0),
    }
}

const fn class_lifecycle_key(operation: MirClassLifecycleOperation) -> usize {
    match operation {
        MirClassLifecycleOperation::CopyConstructor => 0,
        MirClassLifecycleOperation::CopyAssignment => 1,
        MirClassLifecycleOperation::CompleteFinalizer => 2,
    }
}

const fn array_lifecycle_key(operation: MirArrayLifecycleOperation) -> usize {
    match operation {
        MirArrayLifecycleOperation::Default => 0,
        MirArrayLifecycleOperation::Copy => 1,
        MirArrayLifecycleOperation::Assignment => 2,
        MirArrayLifecycleOperation::Destruction => 3,
    }
}
