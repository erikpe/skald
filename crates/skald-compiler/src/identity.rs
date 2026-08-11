//! Stable identities shared by name-independent compiler phases.
//!
//! Loading assigns provider and module identities; resolution assigns
//! declaration and binding identities. Later phases preserve and compare them
//! without depending on owner implementation details or returning to source
//! names.

use std::fmt;

macro_rules! global_id {
    ($(#[$new_attribute:meta])* $name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(usize);

        impl $name {
            pub const fn index(self) -> usize {
                self.0
            }

            // Construction stays crate-private so the owning compiler stage
            // remains the production authority that allocates identities.
            $(#[$new_attribute])*
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

macro_rules! interface_member_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            interface: InterfaceId,
            index: usize,
        }

        impl $name {
            pub const fn interface(self) -> InterfaceId {
                self.interface
            }
            pub const fn index(self) -> usize {
                self.index
            }
            pub(crate) const fn new(interface: InterfaceId, index: usize) -> Self {
                Self { interface, index }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    "{}:{}{}",
                    self.interface(),
                    $prefix,
                    self.index()
                )
            }
        }
    };
}

macro_rules! callable_local_id {
    ($(#[$new_attribute:meta])* $name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            callable: CallableId,
            index: usize,
        }

        impl $name {
            pub const fn callable(self) -> CallableId {
                self.callable
            }

            pub const fn index(self) -> usize {
                self.index
            }

            // Construction stays crate-private so the semantic owner remains
            // the production authority that allocates callable-local IDs.
            $(#[$new_attribute])*
            pub(crate) fn new(callable: impl Into<CallableId>, index: usize) -> Self {
                Self {
                    callable: callable.into(),
                    index,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}:{}{}", self.callable(), $prefix, self.index())
            }
        }
    };
}

global_id!(
    #[allow(dead_code)]
    ModuleId,
    "m"
);
global_id!(
    #[allow(dead_code)]
    ProviderId,
    "provider"
);
global_id!(
    #[allow(dead_code)]
    PackageId,
    "package"
);
global_id!(FunctionId, "f");
global_id!(ExternalLinkId, "ext");
global_id!(ClassId, "c");
global_id!(InterfaceId, "i");
global_id!(ArrayTypeId, "a");
global_id!(OptionalTypeId, "o");
global_id!(OptionalBoxTypeId, "box");
global_id!(LiteralDataId, "str");
global_id!(VirtualFamilyId, "vf");
global_id!(VirtualSlotId, "vs");

class_member_id!(FieldId, "field");
class_member_id!(StaticFieldId, "static");
class_member_id!(InitializerId, "init");
class_member_id!(CopyConstructorId, "copy");
class_member_id!(CopyAssignmentId, "assign");
class_member_id!(DestructorId, "destroy");
class_member_id!(MethodId, "method");
interface_member_id!(InterfaceRequirementId, "requirement");

/// Stable executable-body identity derived from one static field declaration.
///
/// The field remains the storage and namespace identity. This distinct type
/// owns callable-local compiler identities for the declaration expression
/// without allocating a second source-order index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StaticInitializerId {
    field: StaticFieldId,
}

impl StaticInitializerId {
    pub const fn field(self) -> StaticFieldId {
        self.field
    }

    pub const fn class(self) -> ClassId {
        self.field.class()
    }
}

impl From<StaticFieldId> for StaticInitializerId {
    fn from(field: StaticFieldId) -> Self {
        Self { field }
    }
}

impl fmt::Display for StaticInitializerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:initializer", self.field)
    }
}

/// Stable identity of a declaration with an executable body.
///
/// The tagged identity is deliberately also the body's code-generation
/// identity. Later phases never need a second global body number or a map from
/// source names to backend entries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableId {
    Function(FunctionId),
    StaticInitializer(StaticInitializerId),
    Initializer(InitializerId),
    CopyConstructor(CopyConstructorId),
    CopyAssignment(CopyAssignmentId),
    Destructor(DestructorId),
    Method(MethodId),
}

impl CallableId {
    pub const fn as_function(self) -> Option<FunctionId> {
        match self {
            Self::Function(function) => Some(function),
            Self::StaticInitializer(_)
            | Self::Initializer(_)
            | Self::CopyConstructor(_)
            | Self::CopyAssignment(_)
            | Self::Destructor(_)
            | Self::Method(_) => None,
        }
    }

    pub const fn class(self) -> Option<ClassId> {
        match self {
            Self::Function(_) => None,
            Self::StaticInitializer(initializer) => Some(initializer.class()),
            Self::Initializer(initializer) => Some(initializer.class()),
            Self::CopyConstructor(copy) => Some(copy.class()),
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

impl From<StaticInitializerId> for CallableId {
    fn from(initializer: StaticInitializerId) -> Self {
        Self::StaticInitializer(initializer)
    }
}

impl From<InitializerId> for CallableId {
    fn from(initializer: InitializerId) -> Self {
        Self::Initializer(initializer)
    }
}

impl From<CopyConstructorId> for CallableId {
    fn from(copy: CopyConstructorId) -> Self {
        Self::CopyConstructor(copy)
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
            Self::StaticInitializer(initializer) => initializer.fmt(formatter),
            Self::Initializer(initializer) => initializer.fmt(formatter),
            Self::CopyConstructor(copy) => copy.fmt(formatter),
            Self::CopyAssignment(assignment) => assignment.fmt(formatter),
            Self::Destructor(destructor) => destructor.fmt(formatter),
            Self::Method(method) => method.fmt(formatter),
        }
    }
}

callable_local_id!(ParameterId, "p");
callable_local_id!(LocalId, "l");
// Resolution begins allocating these when source loops are activated.
callable_local_id!(
    #[allow(dead_code)]
    LoopId,
    "loop"
);

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
    use std::any::TypeId;

    use super::*;

    #[test]
    fn request_local_module_identities_are_type_distinct() {
        assert_ne!(TypeId::of::<ModuleId>(), TypeId::of::<ProviderId>());
        assert_ne!(TypeId::of::<ModuleId>(), TypeId::of::<PackageId>());
        assert_ne!(TypeId::of::<ProviderId>(), TypeId::of::<PackageId>());

        let first = ModuleId::new(0);
        let second = ModuleId::new(1);
        assert_eq!(first.index(), 0);
        assert!(first < second);
    }

    #[test]
    fn top_level_identities_preserve_owner_index_ordering_and_display() {
        let module = ModuleId::new(1);
        let provider = ProviderId::new(2);
        let package = PackageId::new(3);
        let first = FunctionId::new(2);
        let second = FunctionId::new(3);
        let family = VirtualFamilyId::new(6);
        let slot = VirtualSlotId::new(7);
        let parameter = ParameterId::new(first, 4);
        let local = LocalId::new(first, 5);
        let loop_id = LoopId::new(first, 6);
        let method_loop = LoopId::new(MethodId::new(ClassId::new(0), 1), 0);

        assert_eq!(module.index(), 1);
        assert_eq!(provider.index(), 2);
        assert_eq!(package.index(), 3);
        assert_eq!(module.to_string(), "m1");
        assert_eq!(provider.to_string(), "provider2");
        assert_eq!(package.to_string(), "package3");
        assert_eq!(first.index(), 2);
        assert!(first < second);
        assert_eq!(parameter.callable(), CallableId::Function(first));
        assert_eq!(parameter.index(), 4);
        assert_eq!(local.callable(), CallableId::Function(first));
        assert_eq!(local.index(), 5);
        assert_eq!(loop_id.callable(), CallableId::Function(first));
        assert_eq!(loop_id.index(), 6);
        assert_eq!(first.to_string(), "f2");
        assert_eq!(family.to_string(), "vf6");
        assert_eq!(slot.to_string(), "vs7");
        assert_eq!(parameter.to_string(), "f2:p4");
        assert_eq!(local.to_string(), "f2:l5");
        assert_eq!(loop_id.to_string(), "f2:loop6");
        assert_eq!(method_loop.to_string(), "c0:method1:loop0");
        assert_ne!(loop_id.callable(), method_loop.callable());
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
        let static_field = StaticFieldId::new(class, 4);
        let static_initializer = StaticInitializerId::from(static_field);
        let initializer = InitializerId::new(class, 0);
        let copy = CopyConstructorId::new(class, 0);
        let assignment = CopyAssignmentId::new(class, 0);
        let destructor = DestructorId::new(class, 0);
        let method = MethodId::new(class, 5);

        assert_eq!(class.index(), 3);
        assert!(class < other_class);
        assert_eq!(field.class(), class);
        assert_eq!(field.index(), 2);
        assert_eq!(static_field.class(), class);
        assert_eq!(static_field.index(), 4);
        assert_eq!(static_initializer.field(), static_field);
        assert_eq!(static_initializer.class(), class);
        assert_eq!(initializer.class(), class);
        assert_eq!(initializer.index(), 0);
        assert_eq!(copy.class(), class);
        assert_eq!(copy.index(), 0);
        assert_eq!(assignment.class(), class);
        assert_eq!(assignment.index(), 0);
        assert_eq!(destructor.class(), class);
        assert_eq!(destructor.index(), 0);
        assert_eq!(method.class(), class);
        assert_eq!(method.index(), 5);
        assert_eq!(class.to_string(), "c3");
        assert_eq!(field.to_string(), "c3:field2");
        assert_eq!(static_field.to_string(), "c3:static4");
        assert_eq!(static_initializer.to_string(), "c3:static4:initializer");
        assert_eq!(initializer.to_string(), "c3:init0");
        assert_eq!(copy.to_string(), "c3:copy0");
        assert_eq!(assignment.to_string(), "c3:assign0");
        assert_eq!(destructor.to_string(), "c3:destroy0");
        assert_eq!(method.to_string(), "c3:method5");
    }

    #[test]
    fn callable_identity_is_the_body_owner_for_every_declaration_kind() {
        let function = CallableId::from(FunctionId::new(1));
        let static_initializer = CallableId::from(StaticInitializerId::from(StaticFieldId::new(
            ClassId::new(2),
            4,
        )));
        let initializer = CallableId::from(InitializerId::new(ClassId::new(2), 0));
        let copy = CallableId::from(CopyConstructorId::new(ClassId::new(2), 0));
        let assignment = CallableId::from(CopyAssignmentId::new(ClassId::new(2), 0));
        let destructor = CallableId::from(DestructorId::new(ClassId::new(2), 0));
        let method = CallableId::from(MethodId::new(ClassId::new(2), 3));

        assert_eq!(function.as_function(), Some(FunctionId::new(1)));
        assert_eq!(function.class(), None);
        assert_eq!(static_initializer.as_function(), None);
        assert_eq!(static_initializer.class(), Some(ClassId::new(2)));
        assert_eq!(initializer.as_function(), None);
        assert_eq!(initializer.class(), Some(ClassId::new(2)));
        assert_eq!(copy.as_function(), None);
        assert_eq!(copy.class(), Some(ClassId::new(2)));
        assert_eq!(assignment.as_function(), None);
        assert_eq!(assignment.class(), Some(ClassId::new(2)));
        assert_eq!(destructor.as_function(), None);
        assert_eq!(destructor.class(), Some(ClassId::new(2)));
        assert_eq!(method.class(), Some(ClassId::new(2)));
        assert_eq!(function.to_string(), "f1");
        assert_eq!(static_initializer.to_string(), "c2:static4:initializer");
        assert_eq!(initializer.to_string(), "c2:init0");
        assert_eq!(copy.to_string(), "c2:copy0");
        assert_eq!(assignment.to_string(), "c2:assign0");
        assert_eq!(destructor.to_string(), "c2:destroy0");
        assert_eq!(method.to_string(), "c2:method3");

        let parameter = ParameterId::new(method, 4);
        let local = LocalId::new(copy, 5);
        let destructor_local = LocalId::new(destructor, 6);
        let static_local = LocalId::new(static_initializer, 7);
        assert_eq!(parameter.callable(), method);
        assert_eq!(local.callable(), copy);
        assert_eq!(destructor_local.callable(), destructor);
        assert_eq!(static_local.callable(), static_initializer);
        assert_eq!(parameter.to_string(), "c2:method3:p4");
        assert_eq!(local.to_string(), "c2:copy0:l5");
        assert_eq!(destructor_local.to_string(), "c2:destroy0:l6");
        assert_eq!(static_local.to_string(), "c2:static4:initializer:l7");
    }
}
