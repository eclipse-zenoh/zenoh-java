//! `KotlinExt` — destination-language trait for emitting Kotlin output.
//!
//! Lives in the `kotlin` module so a build that targets only Rust (e.g. a
//! future cbindgen back-end) doesn't pull in any Kotlin-specific code.
//!
//! Called only after the `Registry` is fully resolved; trait methods get a
//! `&Registry` they can query for resolved wire types and converter names.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::registry::{Registry, TypeEntry, TypeKey};

/// One Kotlin file's contents — the `KotlinExt` returns these and
/// `write_kotlin` writes them out.
#[derive(Clone, Debug)]
pub struct KotlinFile {
    /// Java/Kotlin package (`io.zenoh.jni.callbacks`). Empty for default
    /// package.
    pub package: String,
    /// Class/interface name without `.kt` extension. Becomes the file name
    /// (e.g. `JNISampleCallback` → `JNISampleCallback.kt`).
    pub class_name: String,
    /// Full file contents — the package line and any imports must already
    /// be included by the ext.
    pub contents: String,
}

/// Emits Kotlin output corresponding to a resolved `Registry`.
pub trait KotlinExt {
    /// Emit a Kotlin file for one `#[prebindgen]` fn. `None` = no Kotlin
    /// file produced for this fn (e.g. the fn is part of a bigger interface
    /// emitted via a different mechanism).
    fn on_function(&self, f: &syn::ItemFn, registry: &Registry) -> Option<KotlinFile>;
    fn on_struct(&self, s: &syn::ItemStruct, registry: &Registry) -> Option<KotlinFile>;
    fn on_enum(&self, e: &syn::ItemEnum, registry: &Registry) -> Option<KotlinFile>;

    /// Emit a Kotlin file for one resolved type entry. Called once per
    /// resolved type per direction; most types return None. Callbacks return
    /// `JNI<Stem>Callback.kt`-shaped files here.
    fn on_type(
        &self,
        ty: &syn::Type,
        entry: &TypeEntry,
        registry: &Registry,
    ) -> Option<KotlinFile>;
}

/// Errors surfaced by Kotlin emission.
#[derive(Debug)]
pub enum WriteKotlinError {
    Io(std::io::Error),
}

impl std::fmt::Display for WriteKotlinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteKotlinError::Io(e) => write!(f, "I/O error writing Kotlin file: {}", e),
        }
    }
}

impl std::error::Error for WriteKotlinError {}

impl From<std::io::Error> for WriteKotlinError {
    fn from(e: std::io::Error) -> Self {
        WriteKotlinError::Io(e)
    }
}

/// Walk every `#[prebindgen]` fn / struct / enum and every resolved type
/// entry in the registry; for each, ask the ext for a `KotlinFile`; write
/// every returned file under `output_dir`.
///
/// The output path for a returned `KotlinFile` is
/// `<output_dir>/<package_path>/<class_name>.kt` where `package_path`
/// translates `.` to `/`. Empty packages drop straight into `output_dir`.
///
/// Files with the same `(package, class_name)` are deduplicated — the first
/// occurrence wins. (This matches the resolver convention that names are
/// pure functions of the input pair.)
pub fn write_kotlin<P: AsRef<Path>, E: KotlinExt>(
    registry: &Registry,
    ext: &E,
    output_dir: P,
) -> Result<Vec<PathBuf>, WriteKotlinError> {
    let mut files: HashMap<(String, String), KotlinFile> = HashMap::new();

    let mut record = |opt: Option<KotlinFile>| {
        if let Some(f) = opt {
            files.entry((f.package.clone(), f.class_name.clone())).or_insert(f);
        }
    };

    for (item, _loc) in registry.functions.values() {
        record(ext.on_function(item, registry));
    }
    for (item, _loc) in registry.structs.values() {
        record(ext.on_struct(item, registry));
    }
    for (item, _loc) in registry.enums.values() {
        record(ext.on_enum(item, registry));
    }
    for_each_resolved_type(registry, |key, entry| {
        let ty = key.to_type();
        record(ext.on_type(&ty, entry, registry));
    });

    // Write all collected files.
    let base = output_dir.as_ref();
    let mut written = Vec::with_capacity(files.len());
    let mut keys: Vec<_> = files.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let f = &files[&key];
        let dir = if f.package.is_empty() {
            base.to_path_buf()
        } else {
            base.join(f.package.replace('.', "/"))
        };
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.kt", f.class_name));
        fs::write(&path, &f.contents)?;
        written.push(path);
    }
    Ok(written)
}

fn for_each_resolved_type<F: FnMut(&TypeKey, &TypeEntry)>(registry: &Registry, mut f: F) {
    for bucket in &registry.input_types {
        for (k, slot) in bucket {
            if let Some(e) = slot {
                f(k, e);
            }
        }
    }
    for bucket in &registry.output_types {
        for (k, slot) in bucket {
            if let Some(e) = slot {
                f(k, e);
            }
        }
    }
}
