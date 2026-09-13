//! Macro-composed traversal of every callable-local identity in executable MIR.
//!
//! The facade owns leaf behavior and assembles private structural fragments
//! inside both generated traversals. A new identity-bearing field belongs in
//! exactly one fragment and must therefore receive the same exhaustive visit
//! position in mutable mapping and read-only observation.

use self::{
    body::define_body_traversal,
    definition::define_definition_traversal,
    instruction::{
        define_array_instruction_traversal, define_core_operation_traversal,
        define_instruction_dispatch, define_io_instruction_traversal,
        define_optional_instruction_traversal,
    },
    leaf::define_identity_leaf_traversal,
    place::define_place_traversal,
    terminator::define_terminator_traversal,
};
use super::super::*;
use super::MirLocalIdentityObserver;

#[cfg(test)]
use super::{
    identity::{LocalIdentityOwnerValidator, PreserveLocalIdentities},
    MirLocalIdentityOwnershipError,
};

mod body;
mod definition;
mod instruction;
mod leaf;
mod place;
mod terminator;

macro_rules! map_identity {
    (storage, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_storage($site, *$identity)?;
        Ok(())
    }};
    (storage_use, $mapper:expr, $site:expr, $role:expr, $identity:expr) => {{
        let _ = $role;
        *$identity = $mapper.map_storage($site, *$identity)?;
        Ok(())
    }};
    (value, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_value($site, *$identity)?;
        Ok(())
    }};
    (value_use, $mapper:expr, $site:expr, $role:expr, $identity:expr) => {{
        let _ = $role;
        *$identity = $mapper.map_value($site, *$identity)?;
        Ok(())
    }};
    (value_definition, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_value_definition($site, *$identity)?;
        Ok(())
    }};
    (block, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_block($site, *$identity)?;
        Ok(())
    }};
    (path_condition, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_path_condition($site, *$identity)?;
        Ok(())
    }};
    (optional_guard, $mapper:expr, $site:expr, $identity:expr) => {{
        *$identity = $mapper.map_optional_guard($site, *$identity)?;
        Ok(())
    }};
}

macro_rules! observe_identity {
    (storage, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_storage($site, *$identity)
    };
    (storage_use, $observer:expr, $site:expr, $role:expr, $identity:expr) => {
        $observer.observe_storage_use($site, $role, *$identity)
    };
    (value, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_value($site, *$identity)
    };
    (value_use, $observer:expr, $site:expr, $role:expr, $identity:expr) => {
        $observer.observe_value_use($site, $role, *$identity)
    };
    (value_definition, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_value_definition($site, *$identity)
    };
    (block, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_block($site, *$identity)
    };
    (path_condition, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_path_condition($site, *$identity)
    };
    (optional_guard, $observer:expr, $site:expr, $identity:expr) => {
        $observer.observe_optional_guard($site, *$identity)
    };
}

/// Defines one traversal over either mutable or shared MIR.
///
/// Private macro fragments divide the structural inventory by MIR
/// responsibility. Every fragment expands here for both mapping and
/// observation, which keeps identity coverage and order single-sourced.
macro_rules! define_identity_traversal {
    ($module:ident, $behavior:ident, ($($mir_mutability:tt)*), $leaf:ident) => {
mod $module {
use super::super::super::*;
use super::super::{MirCallValueUse, MirLocalIdentitySite, MirScalarValueUse, MirStoragePlaceUse, MirStorageUseRole, MirStorageWriteAuthorization, MirValueUseRole, $behavior as MirLocalIdentityMapper};
use super::{
    define_array_instruction_traversal, define_body_traversal, define_core_operation_traversal,
    define_definition_traversal, define_identity_leaf_traversal, define_instruction_dispatch,
    define_io_instruction_traversal, define_optional_instruction_traversal, define_place_traversal,
    define_terminator_traversal,
};

define_definition_traversal!(($($mir_mutability)*));
define_body_traversal!(($($mir_mutability)*));

define_instruction_dispatch!(($($mir_mutability)*));
define_core_operation_traversal!(($($mir_mutability)*));
define_optional_instruction_traversal!(($($mir_mutability)*));
define_io_instruction_traversal!(($($mir_mutability)*));
define_array_instruction_traversal!(($($mir_mutability)*));
define_terminator_traversal!(($($mir_mutability)*));
define_place_traversal!(($($mir_mutability)*));
define_identity_leaf_traversal!(($($mir_mutability)*), $leaf);

}
    };
}

define_identity_traversal!(mapping, MirLocalIdentityMapper, (mut), map_identity);
define_identity_traversal!(observation, MirLocalIdentityObserver, (), observe_identity);

pub(super) use mapping::{
    map_body_local_identities, map_common_local_identities, map_function_attachments,
    map_instruction, map_logical_expression, map_member_attachments, map_path_condition_metadata,
    map_static_publication_attachment, map_terminator,
};
#[cfg(test)]
pub(crate) use mapping::{
    map_function_local_identities, map_member_local_identities,
    map_static_initializer_local_identities,
};
pub(super) use observation::{
    map_body_local_identities as observe_body_local_identities,
    map_instruction as observe_instruction, map_logical_expression as observe_logical_expression,
    map_path_condition_metadata as observe_path_condition_metadata,
    map_terminator as observe_terminator,
};
pub(crate) use observation::{
    map_function_local_identities as observe_function_local_identities,
    map_member_local_identities as observe_member_local_identities,
    map_static_initializer_local_identities as observe_static_initializer_local_identities,
};

/// Observes one borrowed executable definition without exposing its concrete
/// function, member, or static-initializer shape to analyses.
pub(crate) fn observe_definition_local_identities<O: MirLocalIdentityObserver>(
    definition: MirDefinitionRef<'_>,
    observer: &mut O,
) -> Result<(), O::Error> {
    match definition {
        MirDefinitionRef::Function(definition) => {
            observe_function_local_identities(definition, observer)
        }
        MirDefinitionRef::Member(definition) => {
            observe_member_local_identities(definition, observer)
        }
        MirDefinitionRef::StaticInitializer(definition) => {
            observe_static_initializer_local_identities(definition, observer)
        }
    }
}

#[cfg(test)]
pub(crate) fn validate_function_local_identity_owners(
    definition: &MirFunctionDefinition,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable());
    observe_function_local_identities(definition, &mut validator)
}

#[cfg(test)]
pub(crate) fn validate_member_local_identity_owners(
    definition: &MirMemberDefinition,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable);
    observe_member_local_identities(definition, &mut validator)
}

#[cfg(test)]
pub(crate) fn validate_static_initializer_local_identity_owners(
    definition: &MirStaticInitializerBody,
) -> Result<(), MirLocalIdentityOwnershipError> {
    let mut validator = LocalIdentityOwnerValidator::new(definition.callable());
    observe_static_initializer_local_identities(definition, &mut validator)
}

#[cfg(test)]
pub(super) fn preserve_function_local_identities(
    definition: &mut MirFunctionDefinition,
) -> Result<(), std::convert::Infallible> {
    map_function_local_identities(definition, &mut PreserveLocalIdentities)
}
