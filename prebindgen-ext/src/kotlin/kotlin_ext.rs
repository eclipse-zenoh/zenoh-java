//! `KotlinExt` — destination-language trait for emitting Kotlin output.
//!
//! Lives in the `kotlin` module so a build that targets only Rust (e.g. a
//! future cbindgen back-end) doesn't pull in any Kotlin-specific code.
//!
//! Called only after the `Registry` is fully resolved. The trait is a
//! single-method entry point; implementations are free to emit one file
//! per item, one aggregated file, or any mix. Today's JNI back-end emits
//! one big `JNINative.kt` (interface + data classes + external funs) plus
//! per-callback `JNI<Stem>Callback.kt` files.

use std::path::{Path, PathBuf};

use crate::core::registry::Registry;

/// One Kotlin file's contents.
#[derive(Clone, Debug)]
pub struct KotlinFile {
    /// Java/Kotlin package (`io.zenoh.jni.callbacks`). Empty for default
    /// package.
    pub package: String,
    /// Class/interface name without `.kt` extension. Becomes the file name
    /// (e.g. `JNISampleCallback` → `JNISampleCallback.kt`).
    pub class_name: String,
    /// Full file contents — package line and any imports must already be
    /// included by the ext.
    pub contents: String,
}

impl KotlinFile {
    /// Resolve the on-disk path for this file under `output_dir`. The
    /// `package` is translated to a directory path (`.` → `/`).
    pub fn path_under(&self, output_dir: &Path) -> PathBuf {
        let dir = if self.package.is_empty() {
            output_dir.to_path_buf()
        } else {
            output_dir.join(self.package.replace('.', "/"))
        };
        dir.join(format!("{}.kt", self.class_name))
    }

    /// Write this file to its `path_under(output_dir)`, creating parent
    /// directories as needed.
    pub fn write(&self, output_dir: &Path) -> Result<PathBuf, std::io::Error> {
        let path = self.path_under(output_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &self.contents)?;
        Ok(path)
    }
}

/// Errors surfaced by Kotlin emission.
#[derive(Debug)]
pub enum WriteKotlinError {
    Io(std::io::Error),
    /// Bubbled from the ext-specific implementation.
    Other(String),
}

impl std::fmt::Display for WriteKotlinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteKotlinError::Io(e) => write!(f, "I/O error writing Kotlin file: {}", e),
            WriteKotlinError::Other(s) => write!(f, "Kotlin emission error: {}", s),
        }
    }
}

impl std::error::Error for WriteKotlinError {}

impl From<std::io::Error> for WriteKotlinError {
    fn from(e: std::io::Error) -> Self {
        WriteKotlinError::Io(e)
    }
}

/// Implemented by destination-language back-ends that produce Kotlin output.
///
/// One method, called once per build. The implementation walks the resolved
/// `Registry` (the resolver has already filled every required wire type and
/// converter body) and writes whatever Kotlin files it needs under
/// `output_dir`. Returns the list of paths written, for logging.
pub trait KotlinExt {
    fn write_kotlin(
        &self,
        registry: &Registry,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError>;
}

/// Convenience entry point — same shape as `core::write::write_rust`.
pub fn write_kotlin<P: AsRef<Path>, E: KotlinExt>(
    registry: &Registry,
    ext: &E,
    output_dir: P,
) -> Result<Vec<PathBuf>, WriteKotlinError> {
    ext.write_kotlin(registry, output_dir.as_ref())
}
