//! Stable identities shared by name-independent compiler phases.
//!
//! Resolution assigns these identities when source declarations and bindings
//! are selected. Later phases preserve and compare them without depending on
//! resolver implementation details or returning to source names.

use std::fmt;

macro_rules! global_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(usize);

        impl $name {
            pub const fn index(self) -> usize {
                self.0
            }

            // Construction stays crate-private so resolution remains the
            // production authority that allocates semantic identities.
            pub(crate) const fn new(index: usize) -> Self {
                Self(index)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}{}", $prefix, self.index())
            }
        }
    };
}

macro_rules! class_member_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            class: ClassId,
            index: usize,
        }

        impl $name {
            pub const fn class(self) -> ClassId {
                self.class
            }

            pub const fn index(self) -> usize {
                self.index
            }

            // Construction stays crate-private so resolution remains the
            // production authority that allocates semantic identities.
            pub(crate) const fn new(class: ClassId, index: usize) -> Self {
                Self { class, index }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}:{}{}", self.class(), $prefix, self.index())
            }
        }
    };
}

global_id!(FunctionId, "f");
global_id!(ClassId, "c");
global_id!(VirtualFamilyId, "vf");
global_id!(VirtualSlotId, "vs");

class_member_id!(FieldId, "field");
class_member_id!(InitializerId, "init");
class_member_id!(CopyAssignmentId, "assign");
class_member_id!(DestructorId, "destroy");
class_member_id!(MethodId, "method");

/// Stable identity of a declaration with an executable body.
///
/// The tagged identity is deliberately also the body's code-generation
/// identity. Later phases never need a second global body number or a map from
/// source names to backend entries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableId {
    Function(FunctionId),
    Initializer(InitializerId),
    CopyAssignment(CopyAssignmentId),
    Destructor(DestructorId),
    Method(MethodId),
}

impl CallableId {
    pub const fn as_function(self) -> Option<FunctionId> {
        match self {
            Self::Function(function) => Some(function),
            Self::Initializer(_)
            | Self::CopyAssignment(_)
            | Self::Destructor(_)
            | Self::Method(_) => None,
        }
    }

    pub const fn class(self) -> Option<ClassId> {
        match self {
            Self::Function(_) => None,
            Self::Initializer(initializer) => Some(initializer.class()),
            Self::CopyAssignment(assignment) => Some(assignment.class()),
            Self::Destructor(destructor) => Some(destructor.class()),
            Self::Method(method) => Some(method.class()),
        }
    }
}

impl From<FunctionId> for CallableId {
    fn from(function: FunctionId) -> Self {
        Self::Function(function)
    }
}

impl From<InitializerId> for CallableId {
    fn from(initializer: InitializerId) -> Self {
        Self::Initializer(initializer)
    }
}

impl From<CopyAssignmentId> for CallableId {
    fn from(assignment: CopyAssignmentId) -> Self {
        Self::CopyAssignment(assignment)
    }
}

impl From<DestructorId> for CallableId {
    fn from(destructor: DestructorId) -> Self {
        Self::Destructor(destructor)
    }
}

impl From<MethodId> for CallableId {
    fn from(method: MethodId) -> Self {
        Self::Method(method)
    }
}

impl fmt::Display for CallableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function(function) => function.fmt(formatter),
            Self::Initializer(initializer) => initializer.fmt(formatter),
            Self::CopyAssignment(assignment) => assignment.fmt(formatter),
            Self::Destructor(destructor) => destructor.fmt(formatter),
            Self::Method(method) => method.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParameterId {
    callable: CallableId,
    index: usize,
}

impl ParameterId {
    pub const fn callable(self) -> CallableId {
        self.callable
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub(crate) fn new(callable: impl Into<CallableId>, index: usize) -> Self {
        Self {
            callable: callable.into(),
            index,
        }
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:p{}", self.callable(), self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalId {
    callable: CallableId,
    index: usize,
}

impl LocalId {
    pub const fn callable(self) -> CallableId {
        self.callable
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub(crate) fn new(callable: impl Into<CallableId>, index: usize) -> Self {
        Self {
            callable: callable.into(),
            index,
        }
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:l{}", self.callable(), self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BindingId {
    Receiver(CallableId),
    Parameter(ParameterId),
    Local(LocalId),
}

impl BindingId {
    pub const fn callable(self) -> CallableId {
        match self {
            Self::Receiver(callable) => callable,
            Self::Parameter(id) => id.callable(),
            Self::Local(id) => id.callable(),
        }
    }
}

impl fmt::Display for BindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receiver(callable) => write!(formatter, "{callable}:self"),
            Self::Parameter(id) => id.fmt(formatter),
            Self::Local(id) => id.fmt(formatter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_identities_preserve_owner_index_ordering_and_display() {
        let first = FunctionId::new(2);
        let second = FunctionId::new(3);
        let family = VirtualFamilyId::new(6);
        let slot = VirtualSlotId::new(7);
        let parameter = ParameterId::new(first, 4);
        let local = LocalId::new(first, 5);

        assert_eq!(first.index(), 2);
        assert!(first < second);
        assert_eq!(parameter.callable(), CallableId::Function(first));
        assert_eq!(parameter.index(), 4);
        assert_eq!(local.callable(), CallableId::Function(first));
        assert_eq!(local.index(), 5);
        assert_eq!(first.to_string(), "f2");
        assert_eq!(family.to_string(), "vf6");
        assert_eq!(slot.to_string(), "vs7");
        assert_eq!(parameter.to_string(), "f2:p4");
        assert_eq!(local.to_string(), "f2:l5");
        assert_eq!(
            BindingId::Parameter(parameter).callable(),
            CallableId::Function(first)
        );
        assert_eq!(
            BindingId::Local(local).callable(),
            CallableId::Function(first)
        );
        assert_eq!(BindingId::Parameter(parameter).to_string(), "f2:p4");
        assert_eq!(BindingId::Local(local).to_string(), "f2:l5");
    }

    #[test]
    fn class_member_identities_retain_their_owner_and_category() {
        let class = ClassId::new(3);
        let other_class = ClassId::new(4);
        let field = FieldId::new(class, 2);
        let initializer = InitializerId::new(class, 0);
        let assignment = CopyAssignmentId::new(class, 0);
        let destructor = DestructorId::new(class, 0);
        let method = MethodId::new(class, 5);

        assert_eq!(class.index(), 3);
        assert!(class < other_class);
        assert_eq!(field.class(), class);
        assert_eq!(field.index(), 2);
        assert_eq!(initializer.class(), class);
        assert_eq!(initializer.index(), 0);
        assert_eq!(assignment.class(), class);
        assert_eq!(assignment.index(), 0);
        assert_eq!(destructor.class(), class);
        assert_eq!(destructor.index(), 0);
        assert_eq!(method.class(), class);
        assert_eq!(method.index(), 5);
        assert_eq!(class.to_string(), "c3");
        assert_eq!(field.to_string(), "c3:field2");
        assert_eq!(initializer.to_string(), "c3:init0");
        assert_eq!(assignment.to_string(), "c3:assign0");
        assert_eq!(destructor.to_string(), "c3:destroy0");
        assert_eq!(method.to_string(), "c3:method5");
    }

    #[test]
    fn callable_identity_is_the_body_owner_for_every_declaration_kind() {
        let function = CallableId::from(FunctionId::new(1));
        let initializer = CallableId::from(InitializerId::new(ClassId::new(2), 0));
        let assignment = CallableId::from(CopyAssignmentId::new(ClassId::new(2), 0));
        let destructor = CallableId::from(DestructorId::new(ClassId::new(2), 0));
        let method = CallableId::from(MethodId::new(ClassId::new(2), 3));

        assert_eq!(function.as_function(), Some(FunctionId::new(1)));
        assert_eq!(function.class(), None);
        assert_eq!(initializer.as_function(), None);
        assert_eq!(initializer.class(), Some(ClassId::new(2)));
        assert_eq!(assignment.as_function(), None);
        assert_eq!(assignment.class(), Some(ClassId::new(2)));
        assert_eq!(destructor.as_function(), None);
        assert_eq!(destructor.class(), Some(ClassId::new(2)));
        assert_eq!(method.class(), Some(ClassId::new(2)));
        assert_eq!(function.to_string(), "f1");
        assert_eq!(initializer.to_string(), "c2:init0");
        assert_eq!(assignment.to_string(), "c2:assign0");
        assert_eq!(destructor.to_string(), "c2:destroy0");
        assert_eq!(method.to_string(), "c2:method3");

        let parameter = ParameterId::new(method, 4);
        let local = LocalId::new(initializer, 5);
        let destructor_local = LocalId::new(destructor, 6);
        assert_eq!(parameter.callable(), method);
        assert_eq!(local.callable(), initializer);
        assert_eq!(destructor_local.callable(), destructor);
        assert_eq!(parameter.to_string(), "c2:method3:p4");
        assert_eq!(local.to_string(), "c2:init0:l5");
        assert_eq!(destructor_local.to_string(), "c2:destroy0:l6");
    }
}
