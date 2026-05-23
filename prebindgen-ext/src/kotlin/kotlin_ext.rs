//! Kotlin emission primitives — `KotlinFile` value and `WriteKotlinError`
//! shared by the JNI back-end's emitters in `crate::jni::jni_kotlin_ext`.
//!
//! Public surface (`KotlinFile`, `WriteKotlinError`) is re-exported by the
//! `jni` module for back-ends; the trait-based dispatch is gone — Kotlin
//! emission is JniExt-inherent.

use std::path::{Path, PathBuf};

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
