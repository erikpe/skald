use std::{fmt, str::FromStr};

use crate::lexical_policy::is_source_identifier;

/// A non-empty, exact-case logical module path.
///
/// Components use the same identifier policy as Skald source. The path keeps
/// its original case and has one canonical source-visible rendering using
/// `::`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath {
    components: Box<[String]>,
}

impl ModulePath {
    /// Constructs a logical path from already separated components.
    pub fn from_components<I, S>(components: I) -> Result<Self, ModulePathError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let components = components.into_iter().map(Into::into).collect::<Vec<_>>();
        if components.is_empty() {
            return Err(ModulePathError::empty_path());
        }

        for (index, component) in components.iter().enumerate() {
            if component.is_empty() {
                return Err(ModulePathError::invalid_component(
                    ModulePathErrorKind::EmptyComponent,
                    index,
                    component,
                ));
            }
            if !is_source_identifier(component) {
                return Err(ModulePathError::invalid_component(
                    ModulePathErrorKind::InvalidComponent,
                    index,
                    component,
                ));
            }
        }

        Ok(Self {
            components: components.into_boxed_slice(),
        })
    }

    /// Returns the number of logical path components.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Module paths are non-empty by construction.
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Iterates over the logical components in source order.
    pub fn components(&self) -> impl ExactSizeIterator<Item = &str> + DoubleEndedIterator + Clone {
        self.components.iter().map(String::as_str)
    }

    /// Returns the final component.
    pub fn final_component(&self) -> &str {
        self.components
            .last()
            .expect("module paths are constructed non-empty")
    }
}

impl FromStr for ModulePath {
    type Err = ModulePathError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(ModulePathError::empty_path());
        }
        Self::from_components(text.split("::"))
    }
}

impl TryFrom<&str> for ModulePath {
    type Error = ModulePathError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut components = self.components();
        if let Some(first) = components.next() {
            formatter.write_str(first)?;
        }
        for component in components {
            formatter.write_str("::")?;
            formatter.write_str(component)?;
        }
        Ok(())
    }
}

/// The structural reason a logical module path was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulePathErrorKind {
    EmptyPath,
    EmptyComponent,
    InvalidComponent,
}

/// A structured logical-module-path validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePathError {
    kind: ModulePathErrorKind,
    component_index: Option<usize>,
    component: Option<String>,
}

impl ModulePathError {
    fn empty_path() -> Self {
        Self {
            kind: ModulePathErrorKind::EmptyPath,
            component_index: None,
            component: None,
        }
    }

    fn invalid_component(
        kind: ModulePathErrorKind,
        component_index: usize,
        component: &str,
    ) -> Self {
        Self {
            kind,
            component_index: Some(component_index),
            component: Some(component.to_owned()),
        }
    }

    pub const fn kind(&self) -> ModulePathErrorKind {
        self.kind
    }

    /// Returns the zero-based invalid component index, when one exists.
    pub const fn component_index(&self) -> Option<usize> {
        self.component_index
    }

    pub fn component(&self) -> Option<&str> {
        self.component.as_deref()
    }
}

impl fmt::Display for ModulePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ModulePathErrorKind::EmptyPath => {
                formatter.write_str("module path must contain at least one component")
            }
            ModulePathErrorKind::EmptyComponent => write!(
                formatter,
                "module path component {} is empty",
                self.component_index
                    .expect("component errors retain an index")
                    + 1
            ),
            ModulePathErrorKind::InvalidComponent => write!(
                formatter,
                "module path component {} `{}` is not a Skald identifier",
                self.component_index
                    .expect("component errors retain an index")
                    + 1,
                self.component
                    .as_deref()
                    .expect("component errors retain the source component")
            ),
        }
    }
}

impl std::error::Error for ModulePathError {}
