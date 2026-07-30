//! Callable-owned MIR identities.

use std::fmt;

use crate::identity::CallableId;

macro_rules! owned_id {
    ($name:ident, $prefix:literal) => {
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

owned_id!(StorageId, "s");
owned_id!(ValueId, "v");
owned_id!(BlockId, "b");
owned_id!(OptionalGuardId, "g");
owned_id!(PathConditionId, "p");
