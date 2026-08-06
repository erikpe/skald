use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// Resolves an explicit compiler path or the `skac` sibling of this process.
pub fn locate_compiler(explicit: Option<&Path>) -> Result<PathBuf, CompilerLocationError> {
    let requested = match explicit {
        Some(path) => path.to_path_buf(),
        None => std::env::current_exe()
            .map_err(CompilerLocationError::current_executable)?
            .parent()
            .expect("the current executable must have a parent directory")
            .join("skac"),
    };
    let compiler = fs::canonicalize(&requested)
        .map_err(|source| CompilerLocationError::path(requested.clone(), source))?;
    let metadata = fs::metadata(&compiler)
        .map_err(|source| CompilerLocationError::path(compiler.clone(), source))?;
    if !metadata.is_file() {
        return Err(CompilerLocationError::invalid(
            compiler,
            "compiler path is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(CompilerLocationError::invalid(
                compiler,
                "compiler file is not executable",
            ));
        }
    }
    Ok(compiler)
}

/// A clear compiler-path resolution or usability failure.
#[derive(Debug)]
pub struct CompilerLocationError {
    path: Option<PathBuf>,
    message: String,
    source: Option<io::Error>,
}

impl CompilerLocationError {
    fn current_executable(source: io::Error) -> Self {
        Self {
            path: None,
            message: "could not locate the running golden-test executable".to_owned(),
            source: Some(source),
        }
    }

    fn path(path: PathBuf, source: io::Error) -> Self {
        Self {
            path: Some(path),
            message: "could not resolve the Skald compiler".to_owned(),
            source: Some(source),
        }
    }

    fn invalid(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path: Some(path),
            message: message.into(),
            source: None,
        }
    }

    pub fn path_ref(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CompilerLocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)?;
        if let Some(path) = &self.path {
            write!(formatter, " at {}", path.display())?;
        }
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CompilerLocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}
