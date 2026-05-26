//! `KotlinExt` impl for [`JniExt`].
//!
//! [`JniExt::write_kotlin`] is the single entry point for every Kotlin
//! file the JNI back-end emits. Given one `kotlin_root` it writes:
//!   * `NativeHandle.kt` (package `io.zenoh.jni`).
//!   * One typed-handle class per `ptr_class` entry without
//!     `.suppress_kotlin_code()`.
//!   * One package-level wrapper file for `package()` (top-level
//!     safe wrappers for `package_methods` fns).
//!   * `JNINative.kt` — centralized `external fun` holder.
//!   * One Kotlin fun-interface file per `impl Fn(args) + Send + Sync
//!     + 'static` type, named via [`JniExt::kotlin_callback_name_mangle`]
//!     (default = identity over the `"On"`-prefixed auto-derived name;
//!     in zenoh-jni: `JNIOn<Args>`). Callback types overridden via
//!     [`JniExt::callback_input`] are skipped — the override points at
//!     a hand-written interface.
//!
//! Every `#[prebindgen]` function must be assigned a Kotlin home via
//! `.method(...)` on either a typed-handle / data-class / enum config
//! or on `package(...)`. Undeclared functions are skipped (see
//! `Registry::scan_declared` warnings). There is no "orphan" bucket.
//!
//! All emitters route through [`KotlinFile::write`], which translates
//! `package` into a sub-path under `kotlin_root`.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use quote::ToTokens;

use crate::core::prebindgen_ext::{IntoSource, IntoSourceMode, PrebindgenExt};
use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::jni_ext::{converter_returns_owned_object, JniExt, KotlinMeta, MethodEntry};
use crate::jni::templates;
use crate::kotlin::kotlin_ext::{KotlinFile, WriteKotlinError};
use crate::kotlin::type_map::KotlinTypeMap;

/// Declaration of one auto-generated typed `NativeHandle` subclass.
///
/// Consumed by [`JniExt::write_typed_handles`] (and forwarded to
/// [`JniExt::write_jni_wrappers`] so the same promotion list can carve
/// the matching skip-list). Each entry says "this Kotlin class is the
/// home for the named `#[prebindgen]` functions"; everything else stays
/// in the catch-all `JNIWrappers` object.
#[derive(Clone, Copy)]
pub(crate) struct TypedHandle<'a> {
    /// Short Rust name shown in the class doc comment (e.g. `"Publisher"`).
    /// Pure documentation, doesn't have to match anything in the Registry.
    pub rust_doc: &'a str,
    /// Package-qualified Kotlin class name (e.g.
    /// `"io.zenoh.jni.JNIPublisher"`). The Rust type-key registered for
    /// this FQN via [`JniExt::kotlin_type_fqn`] identifies which
    /// parameter of each promoted function becomes `this`.
    pub kotlin_fqn: &'a str,
    /// `#[prebindgen]` fns declared as **instance methods** via
    /// [`JniExt::method`]. The matched first parameter is dropped from
    /// the Kotlin signature and substituted by inherited `withPtr` /
    /// `consume` scope. Mismatch (no param matches the class type) is a
    /// build-time error.
    pub instance_methods: &'a [MethodEntry],
    /// `#[prebindgen]` fns declared as **companion-object methods** via
    /// [`JniExt::companion_method`]. Rendered inside `companion object`
    /// using the same shape as a package-level wrapper.
    pub companion_methods: &'a [MethodEntry],
}

/// Reverse-lookup the Rust type-key registered for a given Kotlin FQN
/// in [`JniExt::kotlin_type_fqns`]. Used by [`JniExt::write_typed_handles`]
/// to determine which parameter of each promoted function should be
/// dropped (becomes `this`).
fn rust_key_for_fqn<'a>(ext: &'a JniExt, fqn: &str) -> Option<&'a str> {
    ext.kotlin_type_fqns
        .iter()
        .find_map(|(rust, k)| (k == fqn).then_some(rust.as_str()))
}

impl JniExt {
    /// Unified Kotlin emission — single public entry point that fans out
    /// to per-callback fun-interface files, `NativeHandle.kt`, typed-handle
    /// classes (one per `ptr_class` registration), and
    /// `JNIWrappers.kt`. Reads all configuration (typed-handle methods,
    /// callback FQN overrides, Kotlin type names) from internal state set
    /// during the builder phase. Returns every path written.
    pub fn write_kotlin(
        &self,
        registry: &Registry<KotlinMeta>,
        kotlin_root: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        let mut written = Vec::new();
        written.extend(self.emit_callback_files(registry, kotlin_root)?);
        written.extend(self.write_exception_classes(kotlin_root)?);
        written.extend(self.write_enum_classes(registry, kotlin_root)?);
        written.extend(self.write_data_classes(registry, kotlin_root)?);
        written.push(self.write_native_handle(kotlin_root)?);

        // Build the borrowed `TypedHandle<'_>` view from internal config.
        let owned = self.collect_typed_handles();
        let typed_handles: Vec<TypedHandle<'_>> = owned
            .iter()
            .map(|h| TypedHandle {
                rust_doc: &h.rust_doc,
                kotlin_fqn: &h.kotlin_fqn,
                instance_methods: h.instance_methods.as_slice(),
                companion_methods: h.companion_methods.as_slice(),
            })
            .collect();
        let kotlin_types = self.build_kotlin_type_map();
        written.extend(self.write_typed_handles(
            &typed_handles,
            registry,
            &kotlin_types,
            kotlin_root,
        )?);
        for (subpackage, pkg_cfg) in &self.packages {
            if pkg_cfg.functions.is_empty() {
                continue;
            }
            written.push(self.write_jni_package(
                registry,
                &kotlin_types,
                kotlin_root,
                subpackage,
                pkg_cfg,
            )?);
        }
        written.push(self.write_jni_native(
            registry,
            &kotlin_types,
            kotlin_root,
        )?);
        Ok(written)
    }

    /// Per-callback fun-interface emission (one `<mangle_callback>.kt`
    /// file per `impl Fn(...)` type encountered in the resolved
    /// registry). Skips writes for `impl Fn(...)` keys whose Kotlin
    /// FQN was overridden via [`Self::callback_input`] — the override
    /// already points at a hand-maintained callback interface, so the
    /// auto-stub would be dead code. Each emitted file is placed
    /// under `kotlin_root/<kotlin_callback_package as path>/`.
    pub(crate) fn emit_callback_files(
        &self,
        registry: &Registry<KotlinMeta>,
        kotlin_root: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        let mut seen: HashSet<TypeKey> = HashSet::new();
        let mut written = Vec::new();
        for buckets in [&registry.input_types, &registry.output_types] {
            for bucket in buckets.iter() {
                for (key, slot) in bucket {
                    if slot.is_none() {
                        continue;
                    }
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    let ty = key.to_type();
                    if let Some(args) = extract_fn_trait_args(&ty) {
                        // A `callback_input` registration points the
                        // Kotlin signature at a hand-written interface
                        // — skip the auto-stub.
                        if self
                            .types
                            .get(key)
                            .and_then(|c| c.callback_kotlin_fqn.as_ref())
                            .is_some()
                        {
                            continue;
                        }
                        let file = build_callback_kotlin_file(self, &args, registry);
                        written.push(file.write(kotlin_root)?);
                    }
                }
            }
        }
        Ok(written)
    }

    /// Build the `TypedHandle` slice from internal `types` config.
    /// Iterates entries where `opaque.is_some()` and emits one
    /// `TypedHandle` per opaque-handle registration. Stable order by
    /// canonical Rust type-key — keeps generated output deterministic.
    fn collect_typed_handles(&self) -> Vec<OwnedTypedHandle> {
        let mut handles: Vec<OwnedTypedHandle> = Vec::new();
        let mut keys: Vec<&TypeKey> = self.types.keys().collect();
        keys.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for key in keys {
            let cfg = &self.types[key];
            let Some(opaque) = &cfg.opaque else { continue };
            if opaque.suppress_kotlin_code {
                continue;
            }
            let Some(kotlin_fqn) = &cfg.kotlin_name else { continue };
            // rust_doc — short last-segment of the Rust type key (best
            // effort; only used in the generated doc comment).
            let rust_doc = key
                .as_str()
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .find(|s| !s.is_empty())
                .unwrap_or(key.as_str())
                .to_string();
            handles.push(OwnedTypedHandle {
                rust_doc,
                kotlin_fqn: kotlin_fqn.clone(),
                instance_methods: cfg.instance_methods.clone(),
                companion_methods: cfg.companion_methods.clone(),
            });
        }
        handles
    }

    /// Build the `KotlinTypeMap` view consumed by the typed-handle and
    /// JNIWrappers emitters. Combines callback FQNs from
    /// [`Self::collect_kotlin_callback_fqns`] (auto-derived or
    /// override) with `kotlin_name` entries from the structured config.
    /// Structured-config entries win on conflict.
    fn build_kotlin_type_map(&self) -> KotlinTypeMap {
        let mut map = KotlinTypeMap::new().with_primitive_builtins();
        for (key, cfg) in &self.types {
            if let Some(name) = &cfg.kotlin_name {
                map = map.add(key.as_str(), name.clone());
            }
        }
        map
    }
}

/// Owned counterpart of [`TypedHandle`] — used internally so the
/// `collect_typed_handles` helper doesn't have to hand out borrows of
/// `self.types`.
pub(crate) struct OwnedTypedHandle {
    pub rust_doc: String,
    pub kotlin_fqn: String,
    pub instance_methods: Vec<MethodEntry>,
    pub companion_methods: Vec<MethodEntry>,
}

impl JniExt {
    /// Emit `NativeHandle.kt` under `output_dir` (package
    /// `io.zenoh.jni`). The class is the Java-side half of the
    /// borrow/consume contract — `withPtr` for `&T` opaque-handle
    /// borrows, `consume` for by-value `T` opaque-handle drops. By
    /// generating it here, the prebindgen-ext pipeline owns the lock
    /// primitive the rest of the auto-generated wrappers depend on.
    /// The Kotlin exception thrown on closed-handle access is the
    /// framework `JniBindingError` — `NativeHandle` is itself a
    /// framework artefact (the JNI ABI between the generated Rust
    /// converters and the Kotlin handle), and closed-handle access is
    /// a misuse of that infrastructure rather than a domain failure.
    /// Keeping it on the framework exception matches the contract
    /// drawn in [`feedback_internal_contracts`]: everything below the
    /// public zenoh-java API surface is framework-internal.
    pub(crate) fn write_native_handle(&self, output_dir: &Path) -> Result<PathBuf, WriteKotlinError> {
        let exc = self.framework_exception();
        let class_name = self.mangle_harness("NativeHandle");
        let file = templates::native_handle::emit_native_handle(
            &self.package,
            &class_name,
            &exc.kotlin_fqn,
        );
        Ok(file.write(output_dir)?)
    }

    /// Emit one Kotlin file per registered
    /// throwable class (via [`crate::jni::JniExt::throwable`]) — each becomes a
    /// `public class <Name>(message: String? = null) : Exception()`
    /// landing under `<package>/<Name>.kt`. Iterates `self.exceptions`
    /// in declaration order; returns every path written.
    pub(crate) fn write_exception_classes(
        &self,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        let mut written = Vec::new();
        for exc in &self.exceptions {
            // Skip exceptions whose Rust type already has a data-class (or
            // ptr/enum) Kotlin emission — those classes carry the `: Exception`
            // extension themselves (via `cfg.throwable` in
            // `render_data_class_source`). The stub-template path only runs
            // for un-registered exception types — in practice that's the
            // framework's `JniBindingError`, declared inside `JniExt::new`
            // without going through `.throwable()`.
            let key = TypeKey::from_type(&exc.rust_type);
            if self
                .types
                .get(&key)
                .map(|cfg| cfg.kotlin_name.is_some())
                .unwrap_or(false)
            {
                continue;
            }
            let (package, class_name) = match exc.kotlin_fqn.rsplit_once('.') {
                Some((p, c)) => (p.to_string(), c.to_string()),
                None => (String::new(), exc.kotlin_fqn.clone()),
            };
            let file = templates::exception::emit_exception(&package, &class_name, &exc.rust_short);
            written.push(file.write(output_dir)?);
        }
        Ok(written)
    }

    /// Emit one Kotlin `enum class` file per `enum_class`-declared type
    /// (skipping any flagged with `.suppress_kotlin_code()`). Variants
    /// render in declaration order using SCREAMING_SNAKE_CASE names; the
    /// constructor stores the Rust discriminant value (or the ordinal as
    /// a fallback when the discriminant isn't a bare integer literal).
    /// A `fromInt(value: Int)` companion mirrors the `Priority.fromInt`
    /// shape that hand-written enums use today, so adapter code stays
    /// uniform.
    pub(crate) fn write_enum_classes(
        &self,
        registry: &Registry<KotlinMeta>,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        let mut written = Vec::new();
        let callback_fqns = self.collect_kotlin_callback_fqns(registry);
        let mut kotlin_types = KotlinTypeMap::new();
        for (k, v) in callback_fqns.iter() {
            kotlin_types = kotlin_types.add(k, v.clone());
        }
        let configured_types = self.build_kotlin_type_map();
        for (k, v) in configured_types.iter() {
            kotlin_types = kotlin_types.add(k, v.clone());
        }
        // Deterministic order by canonical Rust type-key.
        let mut keys: Vec<&TypeKey> = self.types.keys().collect();
        keys.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for key in keys {
            let cfg = &self.types[key];
            let Some(enum_cfg) = &cfg.enum_cfg else {
                continue;
            };
            if enum_cfg.suppress_kotlin_code {
                continue;
            }
            let Some(kotlin_fqn) = &cfg.kotlin_name else {
                continue;
            };
            // Look up the syn::ItemEnum by the type-key's bare ident.
            let ty = key.to_type();
            let Some(ident) = (if let syn::Type::Path(tp) = &ty {
                tp.path.segments.last().map(|s| s.ident.clone())
            } else {
                None
            }) else {
                continue;
            };
            let Some((item_enum, _)) = registry.enums.get(&ident) else {
                continue;
            };
            let (package, class_name) = match kotlin_fqn.rsplit_once('.') {
                Some((p, c)) => (p.to_string(), c.to_string()),
                None => (String::new(), kotlin_fqn.clone()),
            };
            let file = KotlinFile {
                contents: render_enum_source(
                    self,
                    &package,
                    &class_name,
                    item_enum,
                    &cfg.instance_methods,
                    &cfg.companion_methods,
                    registry,
                    &kotlin_types,
                ),
                package,
                class_name,
            };
            written.push(file.write(output_dir)?);
        }
        Ok(written)
    }

    /// Emit one Kotlin `data class` file per `data_class`-declared
    /// struct. Uses resolved converter metadata to derive Kotlin field
    /// types, so wrappers and data-class declarations stay in sync.
    pub(crate) fn write_data_classes(
        &self,
        registry: &Registry<KotlinMeta>,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        let mut written = Vec::new();
        let callback_fqns = self.collect_kotlin_callback_fqns(registry);
        let mut kotlin_types = KotlinTypeMap::new();
        for (k, v) in callback_fqns.iter() {
            kotlin_types = kotlin_types.add(k, v.clone());
        }
        let configured_types = self.build_kotlin_type_map();
        for (k, v) in configured_types.iter() {
            kotlin_types = kotlin_types.add(k, v.clone());
        }
        let mut rust_names: Vec<String> = Vec::new();
        let mut aliases: Vec<(String, String)> = Vec::new();
        let mut keys: Vec<&TypeKey> = self.types.keys().collect();
        keys.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        for key in keys {
            let cfg = &self.types[key];
            if cfg.opaque.is_some() || cfg.enum_cfg.is_some() || cfg.callback_kotlin_fqn.is_some() {
                continue;
            }
            let Some(kotlin_fqn) = &cfg.kotlin_name else {
                continue;
            };

            let ty = key.to_type();
            let Some(ident) = (if let syn::Type::Path(tp) = &ty {
                tp.path.segments.last().map(|s| s.ident.clone())
            } else {
                None
            }) else {
                continue;
            };
            let Some((item_struct, _)) = registry.structs.get(&ident) else {
                continue;
            };
            rust_names.push(item_struct.ident.to_string());

            let (package, class_name) = match kotlin_fqn.rsplit_once('.') {
                Some((p, c)) => (p.to_string(), c.to_string()),
                None => (String::new(), kotlin_fqn.clone()),
            };
            if item_struct.ident.to_string() != class_name {
                aliases.push((item_struct.ident.to_string(), class_name.clone()));
            }
            let file = KotlinFile {
                contents: render_data_class_source(
                    self,
                    &package,
                    &class_name,
                    item_struct,
                    registry,
                    &kotlin_types,
                    &cfg.instance_methods,
                    &cfg.companion_methods,
                    cfg.throwable,
                    cfg.value_class,
                    key.as_str(),
                ),
                package: package.clone(),
                class_name,
            };
            written.push(file.write(output_dir)?);

            // If data-class naming changed, remove stale legacy file that
            // may have been generated under the old class name.
            let legacy_path = output_dir
                .join(package.replace('.', "/"))
                .join(format!("{}.kt", item_struct.ident));
            if item_struct.ident.to_string() != file.class_name && legacy_path.exists() {
                let _ = std::fs::remove_file(&legacy_path);
            }
        }

        if !rust_names.is_empty() {
            strip_legacy_jni_native_data_classes(output_dir, &self.package, &rust_names)?;
        }

        if !aliases.is_empty() {
            let alias_file = KotlinFile {
                contents: render_data_class_aliases_source(&self.package, &aliases),
                package: self.package.clone(),
                class_name: "JNIDataClassAliases".to_string(),
            };
            written.push(alias_file.write(output_dir)?);
        }

        Ok(written)
    }

    /// Emit the package-level wrapper file under `output_dir`. One
    /// Emit one package-level wrapper file for the given subpackage.
    /// One top-level safe wrapper per `MethodEntry` in `pkg_cfg.functions`.
    /// Wrappers delegate to the centralized Native object (see
    /// [`Self::write_jni_native`]). Opaque-handle parameters become
    /// `NativeHandle`; the wrapper body nests `withPtr` / `consume` per
    /// the type-conversion rule. Non-opaque parameters pass through with
    /// the Kotlin type from `kotlin_types`. Opaque-handle return values
    /// are wrapped in `NativeHandle(...)` before return.
    pub(crate) fn write_jni_package(
        &self,
        registry: &Registry<KotlinMeta>,
        kotlin_types: &KotlinTypeMap,
        output_dir: &Path,
        subpackage: &str,
        pkg_cfg: &crate::jni::jni_ext::PackageConfig,
    ) -> Result<PathBuf, WriteKotlinError> {
        let class_name = self.jni_package_class_name(subpackage);
        let package = if self.package.is_empty() {
            subpackage.to_string()
        } else if subpackage.is_empty() {
            self.package.clone()
        } else {
            format!("{}.{}", self.package, subpackage)
        };
        let contents = render_jni_package_source(
            self,
            registry,
            kotlin_types,
            &pkg_cfg.functions,
            &package,
        );
        let file = KotlinFile {
            package,
            class_name,
            contents,
        };
        Ok(file.write(output_dir)?)
    }

    /// Emit the centralized Native-object Kotlin file under `output_dir`
    /// (class name from [`JniExt::jni_native_class_name`]). Holds one
    /// `external fun` per `#[prebindgen]` function — names mangled via
    /// `kotlin_fun_name_mangle`, parameter and return types rendered at
    /// the JNI **wire** level so the declarations match the Rust extern
    /// symbols generated under
    /// `Java_<package>_<jni_native_class>_<name>`. Loading the native
    /// library is the wrapper layer's responsibility — the auto-generated
    /// holder stays free of any reference to higher-layer types so that
    /// `io.zenoh.jni.*` doesn't depend on `io.zenoh.*`. Trigger
    /// `System.load` / `System.loadLibrary` from wrapper entry points
    /// (e.g. via a `companion object { init { ZenohLoad } }` block) so
    /// the lib is in place before any extern call.
    pub(crate) fn write_jni_native(
        &self,
        registry: &Registry<KotlinMeta>,
        kotlin_types: &KotlinTypeMap,
        output_dir: &Path,
    ) -> Result<PathBuf, WriteKotlinError> {
        let class_name = self.jni_native_class_name();
        let declared = self.declared_functions();
        let contents = render_jni_native_source(self, registry, kotlin_types, &declared, &class_name);
        let file = KotlinFile {
            package: self.package.clone(),
            class_name,
            contents,
        };
        Ok(file.write(output_dir)?)
    }

    /// Emit one Kotlin file per entry in `handles` — each becomes a
    /// `public class <ClassName>(initialPtr: Long) : NativeHandle(initialPtr)`
    /// with the standard `free()` + `private external fun <mangle_fun("freePtr")>(ptr: Long)`
    /// destructor pair, plus one instance method per `#[prebindgen]` fn
    /// listed in [`TypedHandle::functions`]. The promoted method's first
    /// opaque parameter matching the handle's Rust type is dropped — the
    /// method uses inherited `withPtr` / `consume` from [`NativeHandle`]
    /// (i.e. `this` scope) for that param, while every remaining
    /// parameter is emitted exactly as it would appear in the
    /// `JNIWrappers` top-level wrapper (including `impl Into<T>`
    /// dispatch arms and opaque-return wrapping).
    ///
    /// Functions listed under any [`TypedHandle::functions`] are skipped
    /// in [`Self::write_jni_wrappers`] — "Not mentioned functions remain
    /// in `JNIWrapper`" is the assignment rule, exposed by passing the
    /// same `handles` slice to both methods.
    ///
    /// Each handle's `kotlin_fqn` must be registered via
    /// [`Self::kotlin_type_fqn`] so the generator can map it back to its
    /// Rust type-key (which identifies the first param to drop in each
    /// promoted method's signature).
    pub(crate) fn write_typed_handles(
        &self,
        handles: &[TypedHandle<'_>],
        registry: &Registry<KotlinMeta>,
        kotlin_types: &KotlinTypeMap,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        // Merged Kotlin type map (callback FQNs + caller-supplied).
        // Same merge order as `render_jni_wrappers_source` — kotlin_types
        // entries WIN over the auto-derived callback FQNs.
        let callback_fqns = self.collect_kotlin_callback_fqns(registry);
        let mut merged_types = KotlinTypeMap::new();
        for (k, v) in callback_fqns.iter() {
            merged_types = merged_types.add(k, v.clone());
        }
        for (k, v) in kotlin_types.iter() {
            merged_types = merged_types.add(k, v.clone());
        }

        let mut written = Vec::new();
        for handle in handles {
            let (package, class_name) = match handle.kotlin_fqn.rsplit_once('.') {
                Some((p, c)) => (p.to_string(), c.to_string()),
                None => (String::new(), handle.kotlin_fqn.to_string()),
            };
            // The typed-handle's Rust type-key is always required — it
            // identifies which param of each `.method(...)` entry becomes
            // `this`. Even with no methods declared we resolve it (cheap)
            // so the wrapper API stays uniform.
            let rust_key = rust_key_for_fqn(self, handle.kotlin_fqn)
                .unwrap_or_else(|| {
                    panic!(
                        "write_typed_handles: kotlin_fqn `{}` is not registered via \
                         JniExt::kotlin_type_fqn — required to identify the typed \
                         handle's Rust type-key for promoted-method param matching.",
                        handle.kotlin_fqn
                    )
                })
                .to_string();
            let file = KotlinFile {
                contents: render_typed_handle_source(
                    self,
                    &package,
                    &class_name,
                    handle.rust_doc,
                    handle.instance_methods,
                    handle.companion_methods,
                    &rust_key,
                    registry,
                    &merged_types,
                ),
                package,
                class_name,
            };
            written.push(file.write(output_dir)?);
        }
        Ok(written)
    }

    /// Return the `<rust-type-key> → <kotlin FQN>` map for every
    /// `impl Fn(args)` type the Registry has resolved. Use this to merge
    /// into a `KotlinTypeMap` consumed by the aggregated-interface
    /// generator (so it can refer to callbacks by their Kotlin FQN).
    pub(crate) fn collect_kotlin_callback_fqns(&self, registry: &Registry<KotlinMeta>) -> KotlinTypeMap {
        let mut map = KotlinTypeMap::new();
        let mut seen: HashSet<TypeKey> = HashSet::new();
        for buckets in [&registry.input_types, &registry.output_types] {
            for bucket in buckets.iter() {
                for (key, slot) in bucket {
                    if slot.is_none() {
                        continue;
                    }
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    let ty = key.to_type();
                    if let Some(args) = extract_fn_trait_args(&ty) {
                        // Re-use the single source of truth for callback
                        // FQN derivation — same closure-mangled name the
                        // converter dispatcher stamps into metadata.
                        let fqn = self.auto_callback_fqn(&args);
                        map = map.add(key.as_str(), fqn);
                    }
                }
            }
        }
        // Merge in plugin-supplied extra mappings (e.g. data-class FQNs
        // that aren't reachable from impl-Fn types).
        for (rust_canon, fqn) in &self.kotlin_type_fqns {
            map = map.add(rust_canon.as_str(), fqn.clone());
        }
        map
    }
}

fn build_callback_kotlin_file(
    ext: &JniExt,
    args: &[syn::Type],
    registry: &Registry<KotlinMeta>,
) -> KotlinFile {
    let name = derive_callback_name(args);
    let class_name = ext.mangle_callback(&name);
    let package = ext.kotlin_callback_package.clone();

    // Resolve each arg's Kotlin type by reading the output-direction
    // entry's metadata — callback args flow inverse to the callback
    // (Rust produces them, Java consumes them). Fall back to the bare
    // last-segment ident when the metadata is missing (matches today's
    // behavior; preserves the dead-stub compile path).
    let mut params: Vec<String> = Vec::new();
    let mut used_fqns: BTreeSet<String> = BTreeSet::new();
    for (i, arg) in args.iter().enumerate() {
        let kotlin_ty = registry
            .output_entry(arg)
            .and_then(|e| e.metadata.kotlin_name.clone())
            .or_else(|| {
                if let syn::Type::Path(tp) = arg {
                    if let Some(last) = tp.path.segments.last() {
                        return Some(last.ident.to_string());
                    }
                }
                None
            })
            .unwrap_or_else(|| "Any".to_string());
        let short = register_fqn(&kotlin_ty, &mut used_fqns);
        let optional_suffix = if is_option_type(arg) { "?" } else { "" };
        params.push(format!("        p{i}: {short}{optional_suffix},"));
    }

    let contents = templates::callback::render_kotlin_interface(
        &package,
        &class_name,
        &params,
        &used_fqns,
    );
    KotlinFile {
        package,
        class_name,
        contents,
    }
}

/// Derive the auto-callback short Kotlin name for an `impl Fn(args)`
/// signature. Always starts with the hardcoded `"On"` and appends each
/// concatenated parameter type Rust short idents + `"Callback"` suffix
/// (`Fn(Query)` → `"QueryCallback"`, `Fn(Reply)` → `"ReplyCallback"`,
/// `Fn(K, V)` → `"KVCallback"`, `Fn()` → `"Callback"`). The result
/// feeds [`JniExt::mangle_callback`] before the FQN is qualified
/// against [`JniExt::kotlin_callback_package`].
pub(crate) fn derive_callback_name(args: &[syn::Type]) -> String {
    let mut s = String::new();
    for a in args {
        s.push_str(&type_short_ident(a));
    }
    s.push_str("Callback");
    s
}

fn type_short_ident(ty: &syn::Type) -> String {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident.to_string();
        }
    }
    "Unknown".into()
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "Option";
        }
    }
    false
}

/// `true` if `ty` is `Option<&T>` or `Option<&mut T>` (any inner T).
/// Mirrors `option_inner_ref_mutability` in `jni_ext.rs` — kept here too
/// to avoid a cross-module helper just for one call site.
fn is_option_ref(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.last() else { return false };
    if seg.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(ab) = &seg.arguments else { return false };
    let Some(syn::GenericArgument::Type(inner)) = ab.args.first() else { return false };
    matches!(inner, syn::Type::Reference(_))
}

/// Render the Kotlin type for a closeable handle reached through the
/// folded [`CloseStrategy`] layers, given the leaf typed-handle short
/// name (e.g. `"ZKeyExpr"`): `Direct → "ZKeyExpr"`,
/// `Nullable(inner) → "<inner>?"`, `Iterable(inner) → "List<<inner>>"`.
fn render_handle_type(strategy: &crate::jni::jni_ext::CloseStrategy, leaf: &str) -> String {
    use crate::jni::jni_ext::CloseStrategy::*;
    match strategy {
        Direct => leaf.to_string(),
        Nullable(inner) => format!("{}?", render_handle_type(inner, leaf)),
        Iterable(inner) => format!("List<{}>", render_handle_type(inner, leaf)),
    }
}

/// Render the Kotlin `close()` expression for a handle `receiver` through
/// the folded [`CloseStrategy`] layers. Fresh lambda variable per nesting
/// level avoids `it` shadowing; the common single-layer cases are
/// special-cased for readable output (`x?.close()`, `x.forEach { it.close() }`).
fn render_handle_close(strategy: &crate::jni::jni_ext::CloseStrategy, receiver: &str) -> String {
    use crate::jni::jni_ext::CloseStrategy::*;
    fn go(strategy: &crate::jni::jni_ext::CloseStrategy, receiver: &str, depth: usize) -> String {
        match strategy {
            Direct => format!("{receiver}.close()"),
            Nullable(inner) => match &**inner {
                Direct => format!("{receiver}?.close()"),
                _ => {
                    let v = format!("e{depth}");
                    format!("{receiver}?.let {{ {v} -> {} }}", go(inner, &v, depth + 1))
                }
            },
            Iterable(inner) => {
                let v = format!("e{depth}");
                format!("{receiver}.forEach {{ {v} -> {} }}", go(inner, &v, depth + 1))
            }
        }
    }
    go(strategy, receiver, 0)
}

fn register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}

// ── Safe-wrapper emitters ──────────────────────────────────────────────

/// One generated Kotlin `enum class` source — variants in
/// SCREAMING_SNAKE_CASE, each carrying the Rust discriminant as a
/// `val value: Int`, plus a `fromInt(value: Int)` companion. Mirrors
/// the hand-written `io.zenoh.qos.Priority` shape so adapter code that
/// already speaks the `.value` / `.fromInt(...)` idiom keeps working.
fn render_enum_source(
    ext: &JniExt,
    package: &str,
    class_name: &str,
    item_enum: &syn::ItemEnum,
    instance_methods: &[MethodEntry],
    companion_methods_in: &[MethodEntry],
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
) -> String {
    assert!(
        instance_methods.is_empty(),
        "render_enum_source: `{class_name}` has `.method(...)` entries but instance \
         methods on `enum_class`-declared types are not supported yet — declare them \
         as `.companion_method(...)` for now",
    );
    // Same discriminant source of truth the Rust `jint → variant` decode
    // uses, so Kotlin `value(N)` and the generated decode agree.
    let variants: Vec<(String, i64)> = crate::util::enum_discriminant_values(item_enum)
        .into_iter()
        .map(|(ident, value)| {
            (crate::util::camel_to_screaming_snake(&ident.to_string()), value)
        })
        .collect();

    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut companion_methods = String::new();
    for entry in companion_methods_in {
        let (item_fn, _loc) = registry.functions.get(&entry.rust_ident).unwrap_or_else(|| {
            panic!(
                "render_enum_source: `{class_name}` promotes function `{}` \
                 which is not present in `registry.functions` — check the spelling against \
                 the matching `#[prebindgen]` Rust fn name.",
                entry.rust_ident,
            )
        });
        let (block, _kind) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            kotlin_types,
            &mut imports,
            None,
            entry.kotlin_name_override.as_deref(),
        )
        .unwrap_or_else(|| {
            panic!(
                "render_enum_source: `{class_name}` promotes function `{}` \
                 but its parameter types couldn't be Kotlin-resolved — verify that all \
                 non-opaque parameter types are registered in `kotlin_types`.",
                entry.rust_ident,
            )
        });
        if !companion_methods.is_empty() {
            companion_methods.push('\n');
        }
        companion_methods.push_str(&block);
        companion_methods.push('\n');
    }

    let mut import_list: Vec<String> = imports
        .iter()
        .filter(|fqn| {
            let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            !pkg.is_empty() && pkg != package
        })
        .cloned()
        .collect();
    import_list.sort();
    import_list.dedup();

    let mut s = String::new();
    s.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        s.push_str(&format!("package {}\n\n", package));
    }
    for imp in &import_list {
        s.push_str(&format!("import {}\n", imp));
    }
    if !import_list.is_empty() {
        s.push('\n');
    }
    s.push_str(&format!(
        "/** JVM-side surface for the native Rust `{}` enum. */\n",
        item_enum.ident
    ));
    s.push_str(&format!(
        "public enum class {}(public val value: Int) {{\n",
        class_name
    ));
    for (i, (name, value)) in variants.iter().enumerate() {
        let sep = if i + 1 == variants.len() { ";" } else { "," };
        s.push_str(&format!("    {}({}){}\n", name, value, sep));
    }
    s.push('\n');
    s.push_str("    public companion object {\n");
    // `@JvmStatic` exposes `fromInt` as a real static method on the enum
    // class itself (rather than only on the `Companion` nested class). The
    // generated struct-encoder calls it via `env.call_static_method`, which
    // wouldn't find a companion-only method.
    s.push_str(&format!(
        "        @JvmStatic\n        public fun fromInt(value: Int): {} = entries.first {{ it.value == value }}\n",
        class_name
    ));
    if !companion_methods.is_empty() {
        s.push('\n');
        for line in companion_methods.lines() {
            if line.is_empty() {
                s.push('\n');
            } else {
                s.push_str("        ");
                s.push_str(line);
                s.push('\n');
            }
        }
    }
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

/// One generated Kotlin `data class` (or `@JvmInline value class` when
/// `value_class` is set) source for a `data_class` /
/// `value_class`-declared Rust struct.
fn render_data_class_source(
    ext: &JniExt,
    package: &str,
    class_name: &str,
    item_struct: &syn::ItemStruct,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    instance_methods: &[MethodEntry],
    companion_methods_in: &[MethodEntry],
    throwable: bool,
    value_class: bool,
    rust_key: &str,
) -> String {
    assert!(
        !(value_class && !instance_methods.is_empty()),
        "render_data_class_source: `{class_name}` is a `value_class` and has \
         `.method(...)` entries; instance methods on value classes aren't supported yet \
         — declare them as `.companion_method(...)` for now",
    );
    let fields_named = match &item_struct.fields {
        syn::Fields::Named(n) => &n.named,
        _ => {
            panic!(
                "render_data_class_source: struct `{}` must use named fields to map onto Kotlin data class properties",
                item_struct.ident
            )
        }
    };
    if value_class {
        assert!(
            !throwable,
            "render_data_class_source: `{}` is registered as both \
             `value_class` and `throwable` — @JvmInline value \
             classes cannot extend `Exception`. Drop `.throwable()` or \
             switch to `data_class`.",
            item_struct.ident
        );
        assert!(
            fields_named.len() == 1,
            "render_data_class_source: `value_class` requires \
             struct `{}` to have exactly one field; found {}. Use \
             `data_class` for multi-field structs.",
            item_struct.ident,
            fields_named.len()
        );
    }

    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut field_lines: Vec<String> = Vec::new();
    // Track per-field destructible (name, folded close strategy) so the
    // bottom emitter can produce a matching `close()` body for each.
    let mut destructible_fields: Vec<(String, crate::jni::jni_ext::CloseStrategy)> = Vec::new();
    for field in fields_named {
        let field_ident = field.ident.as_ref().unwrap_or_else(|| {
            panic!(
                "render_data_class_source: struct `{}` has an unnamed field in named-fields context",
                item_struct.ident
            )
        });
        let kotlin_field_name = snake_to_camel(&field_ident.to_string());
        // When the class extends Exception (throwable), the `message`
        // field shadows `Exception.message` — Kotlin requires `override`.
        let override_prefix = if throwable && kotlin_field_name == "message" {
            "override "
        } else {
            ""
        };

        // Closeable native-handle field: both the typed Kotlin type
        // (`ZKeyExpr?`, `List<ZKeyExpr>`, …) and the `close()` expression
        // are derived from the folded `HandleInfo` the type-unfolding
        // mechanism propagated onto this field's converter metadata —
        // instead of a syntactic `Option<T>` peel. The struct
        // encoder/decoder in jni_ext.rs bridges the JVM handle object to
        // the per-field jlong-wired converter.
        let field_handle = registry
            .output_entry(&field.ty)
            .and_then(|e| e.metadata.handle.clone())
            .or_else(|| {
                registry
                    .input_entry(&field.ty)
                    .and_then(|e| e.metadata.handle.clone())
            });
        if let Some(h) = field_handle.filter(|h| h.owned) {
            let fqn = ext
                .kotlin_type_fqns
                .iter()
                .find(|(k, _)| k == &h.leaf_key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| {
                    panic!(
                        "render_data_class_source: handle field `{}.{}` leaf `{}` has no \
                         Kotlin FQN registered (ptr_class)",
                        item_struct.ident, field_ident, h.leaf_key
                    )
                });
            let short = register_fqn(&fqn, &mut imports);
            field_lines.push(format!(
                "    {override_prefix}val {kotlin_field_name}: {},",
                render_handle_type(&h.strategy, &short)
            ));
            destructible_fields.push((kotlin_field_name, h.strategy));
            continue;
        }

        let kotlin_ty = registry
            .output_entry(&field.ty)
            .and_then(|e| e.metadata.kotlin_name.clone())
            .or_else(|| registry.input_entry(&field.ty).and_then(|e| e.metadata.kotlin_name.clone()))
            .unwrap_or_else(|| {
                panic!(
                    "render_data_class_source: field `{}.{}` has no Kotlin type mapping; register converters before declaring data_class",
                    item_struct.ident,
                    field_ident
                )
            });
        let short = register_fqn(&kotlin_ty, &mut imports);
        // `Option<T>` whose wire is a JNI primitive (jlong/jint/jboolean/…)
        // and that *isn't* an opaque handle (handled above) is encoded by
        // the struct emitter as the bare primitive with a sentinel for
        // `None` (0 / 0.0 / false). The Kotlin field must match that JVM
        // slot: declare it non-nullable so the constructor signature
        // stays primitive (`J` vs `Ljava/lang/Long;`). Nullable boxing
        // for non-handle primitives would require generator-side changes
        // in `struct_output_body` to `Long.valueOf(...)`.
        let wire = registry
            .output_entry(&field.ty)
            .map(|e| e.destination.clone());
        let primitive_wire = wire
            .as_ref()
            .map(|w| crate::jni::jni_ext::is_jni_primitive(w))
            .unwrap_or(false);
        let optional_suffix = if is_option_type(&field.ty) && !primitive_wire { "?" } else { "" };
        field_lines.push(format!("    {override_prefix}val {kotlin_field_name}: {short}{optional_suffix},"));
    }

    let mut instance_body = String::new();
    for entry in instance_methods {
        let (item_fn, _loc) = registry.functions.get(&entry.rust_ident).unwrap_or_else(|| {
            panic!(
                "render_data_class_source: `{class_name}` promotes function `{}` \
                 which is not present in `registry.functions` — check the spelling against \
                 the matching `#[prebindgen]` Rust fn name.",
                entry.rust_ident,
            )
        });
        let (block, kind) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            kotlin_types,
            &mut imports,
            Some(rust_key),
            entry.kotlin_name_override.as_deref(),
        )
        .unwrap_or_else(|| {
            panic!(
                "render_data_class_source: `{class_name}` promotes function `{}` \
                 but its parameter types couldn't be Kotlin-resolved — verify that all \
                 non-opaque parameter types are registered in `kotlin_types`.",
                entry.rust_ident,
            )
        });
        if kind != MethodKind::Instance {
            panic!(
                ".method({}) on `{class_name}`: the function's first parameter doesn't match \
                 the class's Rust type ({rust_key}) — declare it as `.companion_method(...)` \
                 if it isn't an instance method.",
                entry.rust_ident,
            );
        }
        if !instance_body.is_empty() {
            instance_body.push('\n');
        }
        instance_body.push_str(&block);
        instance_body.push('\n');
    }

    let mut companion_methods = String::new();
    for entry in companion_methods_in {
        let (item_fn, _loc) = registry.functions.get(&entry.rust_ident).unwrap_or_else(|| {
            panic!(
                "render_data_class_source: `{class_name}` promotes function `{}` \
                 which is not present in `registry.functions` — check the spelling against \
                 the matching `#[prebindgen]` Rust fn name.",
                entry.rust_ident,
            )
        });
        let (block, _kind) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            kotlin_types,
            &mut imports,
            None,
            entry.kotlin_name_override.as_deref(),
        )
        .unwrap_or_else(|| {
            panic!(
                "render_data_class_source: `{class_name}` promotes function `{}` \
                 but its parameter types couldn't be Kotlin-resolved — verify that all \
                 non-opaque parameter types are registered in `kotlin_types`.",
                entry.rust_ident,
            )
        });
        if !companion_methods.is_empty() {
            companion_methods.push('\n');
        }
        companion_methods.push_str(&block);
        companion_methods.push('\n');
    }

    // Wrapper methods emitted into subpackages still call the centralized
    // Native object anchored at the base package.
    if package != ext.package && (!instance_body.is_empty() || !companion_methods.is_empty()) {
        imports.insert(format!("{}.{}", ext.package, ext.jni_native_class_name()));
    }

    let mut import_list: Vec<String> = imports
        .iter()
        .filter(|fqn| {
            let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            !pkg.is_empty() && pkg != package
        })
        .cloned()
        .collect();
    import_list.sort();
    import_list.dedup();

    let mut s = String::new();
    s.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        s.push_str(&format!("package {}\n\n", package));
    }
    for imp in &import_list {
        s.push_str(&format!("import {}\n", imp));
    }
    if !import_list.is_empty() {
        s.push('\n');
    }
    if value_class {
        assert!(
            destructible_fields.is_empty(),
            "render_data_class_source: `value_class` struct `{}` \
             has a destructible native-handle field — value classes can \
             only express one inline-erased payload, not the \
             `AutoCloseable` + `close()` contract a handle field needs. \
             Use `data_class` for handle-bearing wrappers.",
            item_struct.ident
        );
        // Single line is enforced by the upstream `fields_named.len() == 1`
        // assertion; strip the data-class formatting (leading indent and
        // trailing comma) so the primary constructor reads cleanly.
        let only = field_lines[0]
            .trim_start()
            .trim_end_matches(',')
            .to_string();
        s.push_str("@JvmInline\n");
        s.push_str(&format!("public value class {}({})", class_name, only));
        if companion_methods.is_empty() {
            s.push('\n');
        } else {
            s.push_str(" {\n");
            s.push_str("    public companion object {\n");
            for line in companion_methods.lines() {
                if line.is_empty() {
                    s.push('\n');
                } else {
                    s.push_str("        ");
                    s.push_str(line);
                    s.push('\n');
                }
            }
            s.push_str("    }\n");
            s.push_str("}\n");
        }
    } else {
        s.push_str(&format!("public data class {}(\n", class_name));
        for line in &field_lines {
            s.push_str(line);
            s.push('\n');
        }
        // Supertype clause. `Exception(...)` (a class) and `AutoCloseable`
        // (an interface) stack — Kotlin allows at most one class super + any
        // interfaces. `: Exception(message)` picks the field literally named
        // `message` to forward to Exception's message slot; falls back to
        // `: Exception()` when no such field exists (data-class auto-toString
        // still surfaces the structured fields).
        let exception_clause: Option<String> = if throwable {
            let has_message = fields_named.iter().any(|f| {
                f.ident
                    .as_ref()
                    .map(|i| i.to_string() == "message")
                    .unwrap_or(false)
            });
            Some(if has_message {
                "Exception(message)".to_string()
            } else {
                "Exception()".to_string()
            })
        } else {
            None
        };
        let supertypes: Vec<String> = match (&exception_clause, !destructible_fields.is_empty()) {
            (Some(e), true) => vec![e.clone(), "AutoCloseable".to_string()],
            (Some(e), false) => vec![e.clone()],
            (None, true) => vec!["AutoCloseable".to_string()],
            (None, false) => vec![],
        };
        if supertypes.is_empty() {
            s.push_str(") {\n");
        } else {
            s.push_str(&format!(") : {} {{\n", supertypes.join(", ")));
        }
        if !destructible_fields.is_empty() {
            // `close()` walks every destructible field via its folded close
            // strategy. `JNINativeHandle.close()` is idempotent
            // (Cleaner.Cleanable.clean() invokes exactly once), so calling
            // this multiple times — or alongside the cleaner's own firing on
            // GC — is safe. NOTE: `data class` copy() shares the handle
            // reference between copies; if you intend to close independently,
            // don't copy this class.
            s.push_str("    override fun close() {\n");
            for (fname, strategy) in &destructible_fields {
                s.push_str(&format!("        {}\n", render_handle_close(strategy, fname)));
            }
            s.push_str("    }\n\n");
        }
        if !instance_body.is_empty() {
            for line in instance_body.lines() {
                if line.is_empty() {
                    s.push('\n');
                } else {
                    s.push_str("    ");
                    s.push_str(line);
                    s.push('\n');
                }
            }
            s.push('\n');
        }
        s.push_str("    public companion object {\n");
        if !companion_methods.is_empty() {
            for line in companion_methods.lines() {
                if line.is_empty() {
                    s.push('\n');
                } else {
                    s.push_str("        ");
                    s.push_str(line);
                    s.push('\n');
                }
            }
        }
        s.push_str("    }\n");
        s.push_str("}\n");
    }
    s
}

fn render_data_class_aliases_source(package: &str, aliases: &[(String, String)]) -> String {
    let mut pairs = aliases.to_vec();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

    let mut s = String::new();
    s.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        s.push_str(&format!("package {}\n\n", package));
    }
    s.push_str("// Compatibility aliases for legacy un-mangled data-class references.\n");
    for (legacy, current) in pairs {
        s.push_str(&format!("public typealias {} = {}\n", legacy, current));
    }
    s
}

fn strip_legacy_jni_native_data_classes(
    output_dir: &Path,
    package: &str,
    _rust_names: &[String],
) -> Result<(), WriteKotlinError> {
    let jni_native_path = output_dir
        .join(package.replace('.', "/"))
        .join("JNINative.kt");
    if !jni_native_path.exists() {
        return Ok(());
    }

    let source = std::fs::read_to_string(&jni_native_path)?;
    let lines: Vec<&str> = source.lines().collect();
    let Some(object_start) = lines
        .iter()
        .position(|line| line.trim_start().starts_with("internal object JNINative {"))
    else {
        return Ok(());
    };

    let mut filtered: Vec<String> = Vec::new();
    for line in &lines[..object_start] {
        let trimmed = line.trim_start();
        if trimmed.starts_with("package ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("//")
            || trimmed.is_empty()
        {
            filtered.push((*line).to_string());
        }
    }
    for line in &lines[object_start..] {
        filtered.push((*line).to_string());
    }

    let mut out = filtered.join("\n");
    out.push('\n');
    if out != source {
        std::fs::write(jni_native_path, out)?;
    }
    Ok(())
}

/// Render one typed-handle Kotlin source file. Pure-shell form (with
/// the closure `|n| format!("{n}ViaJNI")` installed via
/// [`JniExt::kotlin_fun_name_mangle`]):
///
/// ```kotlin
/// public class JNIFoo(initialPtr: Long) : NativeHandle(initialPtr) {
///     public fun free() = free { freePtrViaJNI(it) }
///     private external fun freePtrViaJNI(ptr: Long)
/// }
/// ```
///
/// When `promoted_functions` is non-empty, one extra instance method is
/// appended per `#[prebindgen]` fn — the matching opaque first param
/// (Rust type-key = `promoted_rust_key`) is dropped from the Kotlin
/// signature, and its `withPtr` / `consume` wrapper uses the
/// inherited [`NativeHandle`] scope.
///
/// The free-pointer extern name is built as
/// `<mangle_fun("freePtr")>`. Kotlin/JVM's JNI name mangler binds it
/// to the matching `Java_<pkg>_<class>_<mangle_fun("freePtr")>`
/// extern on the Rust side (the auto-generated destructor).
fn render_typed_handle_source(
    ext: &JniExt,
    package: &str,
    class_name: &str,
    rust_doc_name: &str,
    instance_methods: &[MethodEntry],
    companion_methods: &[MethodEntry],
    promoted_rust_key: &str,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
) -> String {
    // Build method bodies first so we can collect imports up front.
    // Two buckets — instance methods land in the class body; companion
    // methods are wrapped in a `companion object { ... }` block. All
    // promoted wrappers dispatch into the centralized Native object;
    // no per-class `external fun` declarations are emitted here.
    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut instance_body = String::new();
    let mut companion_body = String::new();
    for entry in instance_methods {
        let (item_fn, _loc) = registry.functions.get(&entry.rust_ident).unwrap_or_else(|| {
            panic!(
                "render_typed_handle_source: `{class_name}` promotes function `{}` \
                 which is not present in `registry.functions` — check the spelling against \
                 the matching `#[prebindgen]` Rust fn name.",
                entry.rust_ident,
            )
        });
        let (block, kind) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            kotlin_types,
            &mut imports,
            Some(promoted_rust_key),
            entry.kotlin_name_override.as_deref(),
        )
        .unwrap_or_else(|| {
            panic!(
                "render_typed_handle_source: `{class_name}` promotes function `{}` \
                 but its parameter types couldn't be Kotlin-resolved — verify that all \
                 non-opaque parameter types are registered in `kotlin_types`.",
                entry.rust_ident,
            )
        });
        if kind != MethodKind::Instance {
            panic!(
                ".method({}) on `{class_name}`: the function's first parameter doesn't match \
                 the class's Rust type ({promoted_rust_key}) — declare it as `.companion_method(...)` \
                 if it isn't an instance method.",
                entry.rust_ident,
            );
        }
        if !instance_body.is_empty() {
            instance_body.push('\n');
        }
        for line in block.lines() {
            if line.is_empty() {
                instance_body.push('\n');
            } else {
                instance_body.push_str(line);
                instance_body.push('\n');
            }
        }
    }
    for entry in companion_methods {
        let (item_fn, _loc) = registry.functions.get(&entry.rust_ident).unwrap_or_else(|| {
            panic!(
                "render_typed_handle_source: `{class_name}` promotes function `{}` \
                 which is not present in `registry.functions` — check the spelling against \
                 the matching `#[prebindgen]` Rust fn name.",
                entry.rust_ident,
            )
        });
        let (block, _kind) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            kotlin_types,
            &mut imports,
            None,
            entry.kotlin_name_override.as_deref(),
        )
        .unwrap_or_else(|| {
            panic!(
                "render_typed_handle_source: `{class_name}` promotes function `{}` \
                 but its parameter types couldn't be Kotlin-resolved — verify that all \
                 non-opaque parameter types are registered in `kotlin_types`.",
                entry.rust_ident,
            )
        });
        if !companion_body.is_empty() {
            companion_body.push('\n');
        }
        for line in block.lines() {
            if line.is_empty() {
                companion_body.push('\n');
            } else {
                companion_body.push_str(line);
                companion_body.push('\n');
            }
        }
    }

    let native_handle_class = ext.mangle_harness("NativeHandle");
    let native_handle_fqn = if ext.package.is_empty() {
        native_handle_class.clone()
    } else {
        format!("{}.{}", ext.package, native_handle_class)
    };
    // Typed-handle classes emitted into subpackages still extend and call
    // helpers on the base-package NativeHandle and JNINative objects.
    if package != ext.package {
        imports.insert(native_handle_fqn);
        if !instance_methods.is_empty() || !companion_methods.is_empty() {
            imports.insert(format!("{}.{}", ext.package, ext.jni_native_class_name()));
        }
    }

    // Imports filtered the same way as render_kotlin_interface — drop
    // entries whose package matches our own (no need to import locals).
    let mut import_list: Vec<String> = imports
        .iter()
        .filter(|fqn| {
            let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            !pkg.is_empty() && pkg != package
        })
        .cloned()
        .collect();
    import_list.sort();
    import_list.dedup();

    let mut s = String::new();
    s.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        s.push_str(&format!("package {}\n\n", package));
    }
    if !import_list.is_empty() {
        // Exception and cross-package helper imports are included in
        // `import_list`; emit them even when this class has no promoted
        // methods (e.g. a pure typed handle shell in a subpackage).
        for imp in &import_list {
            s.push_str(&format!("import {}\n", imp));
        }
        s.push('\n');
    }
    let free_extern = ext.mangle_fun("freePtr");
    s.push_str(&format!(
        "/** Typed [{native_handle_class}] for a native Zenoh `{}`. */\n",
        rust_doc_name
    ));
    // The concrete subclass owns its own lifecycle: it is `AutoCloseable`,
    // registers a `Cleaner` action, and that action calls its own
    // `@JvmStatic external freePtr` directly. The base class stays minimal
    // (pointer + lock only) and knows nothing about freeing. The cleanup
    // `Cleanup` references only the detached `state` holder + the static
    // `freePtr`, never `this`, so it can't pin the handle (which would
    // stop the cleaner from ever firing).
    s.push_str(&format!(
        "public class {class_name}(initialPtr: Long) : \
         {native_handle_class}(initialPtr), AutoCloseable {{\n",
    ));
    s.push_str(&format!(
        "    private val cleanable: java.lang.ref.Cleaner.Cleanable =\n        \
            {native_handle_class}.CLEANER.register(this, Cleanup(state))\n\n"
    ));
    // `Cleaner.Cleanable.clean()` runs the action exactly once — whether
    // invoked here or by the cleaner thread on GC — then deregisters, so
    // explicit close() and GC cleanup can't double-free.
    s.push_str("    override fun close() = cleanable.clean()\n\n");
    s.push_str(&format!(
        "    private class Cleanup(private val state: {native_handle_class}.State) : Runnable {{\n        \
         override fun run() = state.freeOnce {{ {class_name}.{free_extern}(it) }}\n    }}\n"
    ));
    if !instance_body.is_empty() {
        s.push('\n');
        for line in instance_body.lines() {
            if line.is_empty() {
                s.push('\n');
            } else {
                s.push_str("    ");
                s.push_str(line);
                s.push('\n');
            }
        }
    }
    // Companion object always exists — at minimum it carries the
    // `@JvmStatic external fun freePtr(ptr: Long)` called by `Cleanup`
    // above. Promoted-method bodies (e.g. typed factory functions) follow.
    s.push('\n');
    s.push_str("    public companion object {\n");
    s.push_str(&format!(
        "        @JvmStatic\n        external fun {free_extern}(ptr: Long)\n",
    ));
    if !companion_body.is_empty() {
        s.push('\n');
        for line in companion_body.lines() {
            if line.is_empty() {
                s.push('\n');
            } else {
                s.push_str("        ");
                s.push_str(line);
                s.push('\n');
            }
        }
    }
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

/// Emit the package-level wrapper file: one safe top-level wrapper per
/// `#[prebindgen]` fn whose name is in `promoted` (i.e. listed in
/// `package_methods.methods`). Each wrapper delegates to the centralized
/// Native object's matching `external fun`. Opaque-handle parameters
/// (detected via the input converter returning `OwnedObject<T>`) become
/// `NativeHandle`; the wrapper body nests `withPtr` / `consume` per the
/// syntactic shape. Non-opaque parameters pass through with the Kotlin
/// type from `kotlin_types`.
fn render_jni_package_source(
    ext: &JniExt,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    functions: &[MethodEntry],
    package: &str,
) -> String {
    // Start with the auto-derived callback FQNs and let user-provided
    // entries WIN — the user (build.rs) may need to override e.g.
    // `impl Fn(Query)` to point at a hand-written
    // `JNIQueryCallback` instead of the auto-derived default.
    let callback_fqns = ext.collect_kotlin_callback_fqns(registry);
    let mut merged_types = KotlinTypeMap::new();
    for (k, v) in callback_fqns.iter() {
        merged_types = merged_types.add(k, v.clone());
    }
    for (k, v) in kotlin_types.iter() {
        merged_types = merged_types.add(k, v.clone());
    }

    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut body = String::new();

    for entry in functions {
        let (item_fn, _loc) = registry.functions.get(&entry.rust_ident).unwrap_or_else(|| {
            panic!(
                "render_jni_package_source: function `{}` registered via .function(...) is \
                 not in the prebindgen registry — check the spelling against the matching \
                 `#[prebindgen]` Rust fn name.",
                entry.rust_ident,
            )
        });
        // Top-level wrappers never carry a `promoted_handle`, so the
        // returned [`MethodKind`] is always `Instance` and can be
        // discarded — there is no companion-object emission here.
        if let Some((block, _kind)) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            &merged_types,
            &mut imports,
            None,
            entry.kotlin_name_override.as_deref(),
        ) {
            body.push_str(&block);
            body.push('\n');
        }
    }

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        out.push_str(&format!("package {}\n\n", package));
    }
    // Exception imports (if any) are added to `imports` by the per-wrapper
    // `@Throws` emission, so no error class is hardcoded here.
    for imp in &imports {
        out.push_str(&format!("import {}\n", imp));
    }
    if !ext.package.is_empty() {
        out.push_str(&format!("import {}.{}\n", ext.package, ext.jni_native_class_name()));
    }
    out.push('\n');
    out.push_str(&body);
    out
}

/// Render the centralized `internal object <jni_native_class>` holder:
/// one `external fun` per `#[prebindgen]` function, at the JNI **wire**
/// level. Parameter and return types match what the Rust extern
/// receives:
///   * opaque-handle (Borrow/Consume) → jlong → `Long`
///   * `enum_class`                  → jint  → `Int` (call passes `.value`)
///   * `Any` (impl-Into Dispatch)     → JObject → `Any`
///   * everything else                → entry's high-level Kotlin name
/// Opaque returns become `Long`; every other return uses
/// [`classify_return`]'s `kt_return` (Unit is empty string). No `init`
/// block is emitted — the holder stays free of any wrapper-layer
/// reference; the wrapper-layer call sites are responsible for
/// triggering `System.load` before invoking any extern.
fn render_jni_native_source(
    ext: &JniExt,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    declared: &HashSet<syn::Ident>,
    class_name: &str,
) -> String {
    let callback_fqns = ext.collect_kotlin_callback_fqns(registry);
    let mut merged_types = KotlinTypeMap::new();
    for (k, v) in callback_fqns.iter() {
        merged_types = merged_types.add(k, v.clone());
    }
    for (k, v) in kotlin_types.iter() {
        merged_types = merged_types.add(k, v.clone());
    }

    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut body = String::new();

    let mut idents: Vec<&syn::Ident> = registry.functions.keys().collect();
    idents.sort();
    for ident in idents {
        if !declared.contains(ident) {
            continue;
        }
        let (item_fn, _loc) = &registry.functions[ident];
        if let Some(line) = render_extern_decl(ext, item_fn, registry, &mut imports) {
            body.push_str(&line);
            body.push('\n');
        }
    }

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !ext.package.is_empty() {
        out.push_str(&format!("package {}\n\n", ext.package));
    }
    for imp in &imports {
        out.push_str(&format!("import {}\n", imp));
    }
    out.push('\n');
    out.push_str(&format!("internal object {} {{\n", class_name));
    for line in body.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("}\n");
    out
}

/// Render one `external fun <mangle_fun(name)>(…): <wire-return>` line
/// at the JNI **wire** level (matches what the Rust extern receives):
///   * opaque-handle (Borrow/Consume) → jlong → `Long`
///   * `enum_class`                  → jint  → `Int` (call passes `.value`)
///   * `Any` (impl-Into Dispatch)     → JObject → `Any`
///   * everything else                → entry's high-level Kotlin name
/// Opaque returns become `Long`; every other return uses
/// [`classify_return`]'s `kt_return` (Unit is empty string).
/// Returns `None` if any parameter's input converter isn't resolved.
pub(crate) fn render_extern_decl(
    ext: &JniExt,
    f: &syn::ItemFn,
    registry: &Registry<KotlinMeta>,
    imports: &mut BTreeSet<String>,
) -> Option<String> {
    use std::fmt::Write;

    let rust_name = f.sig.ident.to_string();
    let kt_name = snake_to_camel(&rust_name);
    let jni_call = ext.mangle_fun(&kt_name);

    let mut params: Vec<(String, String)> = Vec::new();
    for input in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = input else { continue };
        let syn::Pat::Ident(pid) = &*pt.pat else { continue };
        let name = snake_to_camel(&pid.ident.to_string());
        let arg_ty = &*pt.ty;

        let entry = registry.input_entry(arg_ty)?;

        let is_opaque = converter_returns_owned_object(&entry.function.sig.output);
        let arg_no_ref: syn::Type = match arg_ty {
            syn::Type::Reference(r) => (*r.elem).clone(),
            _ => arg_ty.clone(),
        };
        // `Option<&Opaque>` crosses the JNI wire as a primitive `jlong`
        // with `0` encoding `None`; nullability lives in the safe wrapper
        // (`withPtrOrZero`) not the JNI extern. Strip the `?` here so the
        // extern signature matches what the JVM will look up. Detection
        // uses `metadata.handle.is_some()` because the `Option<OwnedObject<T>>`
        // converter doesn't return `OwnedObject` directly so the local
        // `is_opaque` flag (which checks the return shape) misses it.
        let is_opt_ref_opaque =
            entry.metadata.handle.is_some() && is_option_ref(arg_ty);
        let optional = is_option_type(arg_ty) && !is_opt_ref_opaque;

        let kt_type_raw = if is_opaque || is_opt_ref_opaque {
            "Long".to_string()
        } else if ext.is_kotlin_enum(&arg_no_ref) {
            "Int".to_string()
        } else {
            entry.metadata.kotlin_name.clone()?
        };
        let short = register_fqn(&kt_type_raw, imports);
        let suffix = if optional { "?" } else { "" };
        params.push((name, format!("{short}{suffix}")));
    }

    let (kt_return, opaque_ctor) =
        classify_return(ext, &f.sig.output, registry, imports)?;
    let wire_return = if opaque_ctor.is_some() {
        "Long".to_string()
    } else {
        kt_return
    };

    let formals = params
        .iter()
        .map(|(n, t)| format!("{n}: {t}"))
        .collect::<Vec<_>>()
        .join(", ");

    let mut line = String::new();
    if wire_return.is_empty() {
        write!(&mut line, "external fun {jni_call}({formals})").ok()?;
    } else {
        write!(&mut line, "external fun {jni_call}({formals}): {wire_return}").ok()?;
    }
    Some(line)
}

/// Whether a typed-handle-promoted wrapper is emitted as an instance
/// method on the handle class (the first parameter matched the promoted
/// Rust type-key as a literal `&T` / `T`), or inside the class's
/// `companion object` (no param matched, or the candidate was an
/// `impl Into<T>` Dispatch param — those are not eligible for instance
/// promotion even when the inner `T` matches).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MethodKind {
    Instance,
    Companion,
}

/// Emit a single wrapper function. Returns `None` if the function has
/// a parameter whose Kotlin type isn't registered (in that case we
/// skip the function rather than panicking — the legacy `JNINative.kt`
/// retains the unwrapped external fun so callers still have an
/// escape hatch).
///
/// When `promoted_handle` is `Some(rust_key)`, the wrapper is emitted
/// as either an instance method or a companion-object method, depending
/// on whether any parameter matches `rust_key`:
///
/// * **Instance** — the first parameter whose Rust type matches
///   `rust_key` (modulo `&T` borrow) is dropped from the signature, and
///   its `withPtr` / `consume` wrapper uses the inherited
///   [`NativeHandle`] scope (no `<name>.` prefix) so the captured
///   `<name>_ptr` is bound in `this`. Every other parameter is emitted
///   exactly as the `JNIWrappers` top-level form.
/// * **Companion** — no parameter matched (e.g. the fn takes no opaque
///   handle of this type, or it takes an `impl Into<T>` Dispatch param
///   whose inner `T` matches the key — those are intentionally **not**
///   promoted to instance methods). The body is emitted exactly as the
///   `JNIWrappers` top-level form (all params, full Dispatch arm tree,
///   no `this` rewrite); the caller is expected to wrap it inside a
///   `companion object { ... }` block on the typed-handle class.
///
/// When `promoted_handle` is `None` (top-level `JNIWrappers` emission),
/// the returned kind is always [`MethodKind::Instance`] (no
/// promotion-shape decision is made) and the caller can ignore it.
fn render_wrapper_fn(
    ext: &JniExt,
    f: &syn::ItemFn,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    imports: &mut BTreeSet<String>,
    promoted_handle: Option<&str>,
    kotlin_name_override: Option<&str>,
) -> Option<(String, MethodKind)> {
    use std::fmt::Write;

    let rust_name = f.sig.ident.to_string();
    // The Kotlin extern in `JNINative` is keyed on the Rust ident
    // (`snake_to_camel(rust_name)` → `ext.mangle_fun`). The per-entry
    // `.name("...")` override only changes the *user-facing* Kotlin
    // wrapper name; the JNI call still has to hit the one extern that
    // the Rust extern actually emits.
    let default_kt_name = snake_to_camel(&rust_name);
    let kt_name = match kotlin_name_override {
        Some(n) => n.to_string(),
        None => default_kt_name.clone(),
    };
    let jni_call = ext.mangle_fun(&default_kt_name);

    // Pre-parse the promoted Rust type-key (if any) so per-param matching
    // is whitespace-normalised against the canonical form.
    let promoted_key: Option<TypeKey> =
        promoted_handle.map(|s| TypeKey::parse(s));

    // Classify each parameter.
    struct Param {
        kt_name: String,
        kt_type: String,
        mode: ParamMode,
        /// `true` when the param's Rust type is a `enum_class`-declared
        /// enum: the high-level Kotlin signature uses the typed enum
        /// (`Priority`), but the underlying JNI `external fun` declares
        /// the param as `Int` (jint wire). The wrapper bridges the two
        /// by passing `<name>.value` at the call site.
        as_enum_value: bool,
    }
    enum ParamMode {
        Borrow,      // &T opaque-handle → withPtr
        Consume,     // T  opaque-handle → consume
        /// `Option<&T>` / `Option<&mut T>` opaque-handle → `withPtrOrZero`.
        /// Nullable typed-handle param; the wrapper runs the body under
        /// the read lock when the handle is non-null and with `0L` when
        /// null. The Rust converter materializes `Option<OwnedObject<T>>`
        /// and the call site uses `.as_deref()` / `.as_deref_mut()`.
        BorrowNullable,
        /// `impl Into<T>` (Kotlin `Any`). At runtime the parameter
        /// fans out into one arm per declared
        /// [`IntoSource`] in `arms`. See
        /// [`DispatchArm`] for the arm shape.
        Dispatch { arms: Vec<DispatchArm> },
        PassThrough,
        /// Promoted opaque param: identical lock semantics to
        /// `Borrow` / `Consume` (the inner bool flag chooses), but the
        /// wrapper uses inherited [`NativeHandle`] scope (no
        /// `<name>.` prefix) and the param is omitted from the
        /// Kotlin signature. Set when `promoted_handle` matches.
        PromotedBorrow,
        PromotedConsume,
        /// Promoted non-opaque param (e.g. `&Hello` on a `data_class`
        /// instance method). The Kotlin call site substitutes `this` for
        /// the param name — no lock wrapping needed, and the param is
        /// dropped from the wrapper signature. Set when `promoted_handle`
        /// matches a non-opaque type.
        PromotedPassThrough,
    }

    // Tracks whether we've already consumed the promoted-handle slot —
    // only the first matching param is promoted; any later param of the
    // same Rust type stays as a normal Borrow/Consume.
    let mut promoted_taken = false;

    let mut params: Vec<Param> = Vec::new();
    for input in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = input else { continue };
        let syn::Pat::Ident(pid) = &*pt.pat else { continue };
        let name = snake_to_camel(&pid.ident.to_string());
        let arg_ty = &*pt.ty;

        // Strip leading reference for the type-map lookup; the registry's
        // input entry is keyed by the param as-written.
        let entry = registry.input_entry(arg_ty)?;
        // Opaque-handle params surface as the base `JNINativeHandle` (the
        // withPtr/consume lock contract lives there). Detection flows from
        // the folded `HandleInfo` — present for both `&T` and by-value `T`
        // (the `owned` flag is orthogonal to presence) — so it's the same
        // source of truth the typed-surface emitters use.
        let is_opaque = entry.metadata.handle.is_some();

        // `Option<&T>` / `Option<&mut T>` for opaque T uses the typed
        // handle subclass (not the bare `NativeHandle` base) with a `?`
        // suffix, so the call site can call `withPtrOrZero` on the
        // nullable receiver. The typed FQN comes from
        // `ext.kotlin_type_fqns` (set by `ptr_class`), keyed by
        // the handle's `leaf_key`. The `KotlinMeta.kotlin_name` is
        // intentionally the value-context name (`"Long"`) for opaque, so
        // it can't be used here.
        let is_opt_ref_opaque = is_opaque && is_option_ref(arg_ty);
        let (kt_type_raw, optional) = if is_opt_ref_opaque {
            let h = entry.metadata.handle.as_ref()?;
            let fqn = ext
                .kotlin_type_fqns
                .iter()
                .find(|(k, _)| k == &h.leaf_key)
                .map(|(_, v)| v.clone())?;
            (fqn, true)
        } else if is_opaque {
            (ext.mangle_harness("NativeHandle"), false)
        } else {
            // Read the Kotlin name straight off the resolved entry's
            // metadata — the rank-N handler that built this converter
            // is also the one that derived the Kotlin name (primitives
            // from `kotlin_for_wire`, wrappers inherit from inner,
            // user-declared decoders from `with_kotlin_name`).
            let kt = entry.metadata.kotlin_name.clone()?;
            let opt = is_option_type(arg_ty);
            (kt, opt)
        };

        // Does this param match the promoted handle's Rust type?
        // Strip a leading `&` before comparing; the registered type-key
        // is the bare-name form (e.g. `Publisher < 'static >`).
        let matches_promoted = if !promoted_taken {
            if let Some(pk) = &promoted_key {
                let arg_no_ref: syn::Type = match arg_ty {
                    syn::Type::Reference(r) => (*r.elem).clone(),
                    _ => arg_ty.clone(),
                };
                TypeKey::from_type(&arg_no_ref) == *pk
            } else {
                false
            }
        } else {
            false
        };

        // Mode: opaque → Borrow/Consume by Rust syntactic shape.
        // Non-opaque + `Any` triggers Dispatch — one arm per declared
        // `IntoSource`. Everything else (primitives, callbacks, data
        // classes) passes through. Promoted variants kick in when this
        // param is the matched-and-not-yet-consumed handle slot.
        let mode = if is_opaque {
            let borrow = matches!(arg_ty, syn::Type::Reference(_));
            if is_opt_ref_opaque {
                if !ext.package.is_empty() {
                    imports.insert(format!("{}.withPtrOrZero", ext.package));
                }
                // Nullable borrow — promoted form not supported here (no
                // typed-handle subclass to promote against when the param
                // type is `T?`).
                ParamMode::BorrowNullable
            } else if matches_promoted {
                promoted_taken = true;
                if borrow { ParamMode::PromotedBorrow } else { ParamMode::PromotedConsume }
            } else if borrow {
                ParamMode::Borrow
            } else {
                ParamMode::Consume
            }
        } else if matches_promoted {
            // Non-opaque (data/value/enum class) instance-method param:
            // drop from the Kotlin signature, substitute `this` at the
            // JNI call site. No lock semantics — the JNI side decodes the
            // Kotlin instance directly (struct decoder via jobject field
            // reflection, value-class projection, enum `.value` etc.).
            promoted_taken = true;
            ParamMode::PromotedPassThrough
        } else if kt_type_raw == "Any" {
            let sources = entry.into_sources.as_deref().unwrap_or(&[]);
            ParamMode::Dispatch {
                arms: build_dispatch_arms(sources, registry, kotlin_types, imports),
            }
        } else {
            ParamMode::PassThrough
        };

        let short = register_fqn(&kt_type_raw, imports);
        let suffix = if optional { "?" } else { "" };
        // Strip a leading `&` before the enum check — the `&Priority`
        // input converter shares Priority's converter (see the rank-1
        // `& _` arm), and the same `.value` projection applies either
        // way at the call site.
        let arg_no_ref: syn::Type = match arg_ty {
            syn::Type::Reference(r) => (*r.elem).clone(),
            _ => arg_ty.clone(),
        };
        let as_enum_value = ext.is_kotlin_enum(&arg_no_ref);
        params.push(Param {
            kt_name: name,
            kt_type: format!("{short}{suffix}"),
            mode,
            as_enum_value,
        });
    }

    // A promoted-handle was requested but never matched any param —
    // emit as a companion-object method instead of panicking. `.method(...)`
    // is a namespace declaration ("this fn lives on the typed-handle
    // class"), and the generator chooses between an instance method and
    // a companion-object method based on whether any param matched.
    let kind = if promoted_handle.is_some() && !promoted_taken {
        MethodKind::Companion
    } else {
        MethodKind::Instance
    };

    // Return type: peel ZResult<...>; detect opaque-handle return.
    // `opaque_ctor` is the constructor name to wrap the JNI return
    // in (typed FQN short name when registered, else `NativeHandle`).
    let (kt_return, opaque_ctor) = classify_return(ext, &f.sig.output, registry, imports)?;

    // Indices of Dispatch-mode params.
    let dispatch_indices: Vec<usize> = params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| matches!(p.mode, ParamMode::Dispatch { .. }).then_some(i))
        .collect();

    // Build the JNINative call for a given per-Dispatch arm selection.
    // `arm_choice[k]` is the index into the arms list for
    // `dispatch_indices[k]`; values are interpreted by the arm itself
    // — `Unwrap` arms pass `<name>_ptr`, every other arm passes the
    // raw `<name>` (typed handle or non-handle value, untouched).
    let build_call = |arm_choice: &[usize]| -> String {
        let mut args: Vec<String> = Vec::with_capacity(params.len());
        for (i, p) in params.iter().enumerate() {
            let arg = match &p.mode {
                ParamMode::Borrow
                | ParamMode::Consume
                | ParamMode::BorrowNullable
                | ParamMode::PromotedBorrow
                | ParamMode::PromotedConsume => format!("{}_ptr", p.kt_name),
                ParamMode::PromotedPassThrough => {
                    if p.as_enum_value {
                        "this.value".to_string()
                    } else {
                        "this".to_string()
                    }
                }
                ParamMode::Dispatch { arms } => {
                    let pos = dispatch_indices.iter().position(|&di| di == i).unwrap();
                    let arm = &arms[arm_choice[pos]];
                    if arm.unwrap_to_ptr {
                        format!("{}_ptr", p.kt_name)
                    } else {
                        p.kt_name.clone()
                    }
                }
                ParamMode::PassThrough => {
                    if p.as_enum_value {
                        format!("{}.value", p.kt_name)
                    } else {
                        p.kt_name.clone()
                    }
                }
            };
            args.push(arg);
        }
        let mut call = format!("{}.{jni_call}({})", ext.jni_native_class_name(), args.join(", "));
        if let Some(ctor) = &opaque_ctor {
            call = format!("{ctor}({call})");
        }
        call
    };

    // Recurse over Dispatch params; at each level enumerate arms.
    fn build_tree(
        level: usize,
        choice: &mut Vec<usize>,
        dispatch_indices: &[usize],
        params: &[Param],
        build_call: &dyn Fn(&[usize]) -> String,
    ) -> String {
        if level == dispatch_indices.len() {
            return build_call(choice);
        }
        let pi = dispatch_indices[level];
        let arms = match &params[pi].mode {
            ParamMode::Dispatch { arms } => arms,
            _ => unreachable!("dispatch_indices points only at Dispatch params"),
        };
        let name = &params[pi].kt_name;

        // Emit the if/else-if chain over arms, with the final
        // `else` carrying the unconditional-pass-through branch.
        let mut out = String::new();
        for (k, arm) in arms.iter().enumerate() {
            choice.push(k);
            let inner = build_tree(level + 1, choice, dispatch_indices, params, build_call);
            choice.pop();
            // The lock-scope wrapper (`.withPtr` / `.consume`) lives
            // around each NativeHandle-typed arm; non-handle arms
            // (`String`, no-runtime-check else) just inline `inner`.
            // Lambda capture: `<name>_ptr` for unwrap arms (the inner
            // call references it), `_` for typed-handle arms (the
            // inner call passes the typed handle directly to JNI).
            let capture = if arm.unwrap_to_ptr {
                format!("{name}_ptr")
            } else {
                "_".to_string()
            };
            let arm_body = match (&arm.runtime_check, &arm.lock_qual) {
                (Some(check), Some(qual)) => format!(
                    "{prefix} ({name} is {check}) {name}.{qual} {{ {capture} ->\n    {inner}\n}}",
                    prefix = if k == 0 { "if" } else { " else if" },
                ),
                (Some(check), None) => format!(
                    "{prefix} ({name} is {check}) {{\n    {inner}\n}}",
                    prefix = if k == 0 { "if" } else { " else if" },
                ),
                (None, _) => {
                    // Catch-all else branch (no runtime check).
                    if k == 0 {
                        // Single unconditional arm (no opaque sources
                        // declared) — skip the if/else scaffolding.
                        inner.clone()
                    } else {
                        format!(" else {{\n    {inner}\n}}")
                    }
                }
            };
            out.push_str(&arm_body);
        }
        out
    }

    let mut choice: Vec<usize> = Vec::with_capacity(dispatch_indices.len());
    let mut body_expr = build_tree(0, &mut choice, &dispatch_indices, &params, &build_call);

    // Wrap with nested withPtr/consume from innermost to outermost
    // for the syntactic-opaque params. Promoted variants drop the
    // `<name>.` prefix to use the inherited `NativeHandle` scope.
    for p in params.iter().rev() {
        match p.mode {
            ParamMode::Borrow => {
                body_expr = format!(
                    "{name}.withPtr {{ {name}_ptr ->\n    {expr}\n}}",
                    name = p.kt_name,
                    expr = body_expr,
                );
            }
            ParamMode::Consume => {
                body_expr = format!(
                    "{name}.consume {{ {name}_ptr ->\n    {expr}\n}}",
                    name = p.kt_name,
                    expr = body_expr,
                );
            }
            ParamMode::BorrowNullable => {
                // Nullable typed-handle receiver — `withPtrOrZero` runs
                // the block under the read lock when non-null and with
                // `0L` when null.
                body_expr = format!(
                    "{name}.withPtrOrZero {{ {name}_ptr ->\n    {expr}\n}}",
                    name = p.kt_name,
                    expr = body_expr,
                );
            }
            ParamMode::PromotedBorrow => {
                body_expr = format!(
                    "withPtr {{ {name}_ptr ->\n    {expr}\n}}",
                    name = p.kt_name,
                    expr = body_expr,
                );
            }
            ParamMode::PromotedConsume => {
                body_expr = format!(
                    "consume {{ {name}_ptr ->\n    {expr}\n}}",
                    name = p.kt_name,
                    expr = body_expr,
                );
            }
            ParamMode::Dispatch { .. } | ParamMode::PassThrough | ParamMode::PromotedPassThrough => {}
        }
    }

    let _ = ext; // ext no longer needed here — throws comes from registry metadata
    let mut out = String::new();
    // `@Throws` is the UNION of every stage every converter the wrapper
    // drives can raise:
    //   * each input parameter's wire-facing converter (its `?` failure
    //     raises the metadata `throws` exception — framework
    //     `JniBindingError` by default, or a custom one bound via
    //     `Some(parse_quote!(<full path>))` in the input wrapper's
    //     closure);
    //   * each pre_stage on that input's chain (value-inspecting throw
    //     stages — an `input_wrapper` / `output_wrapper` whose closure
    //     returns a rust type with `Some(parse_quote!(<full path>))`
    //     and gets composed onto that type's converter);
    //   * the return type's output converter and its pre_stages
    //     (likewise).
    // Collected into a `BTreeSet` so the emitted annotation is sorted and
    // deterministic; stages/converters with no `throws` metadata
    // contribute nothing.
    let mut throws_fqns: BTreeSet<String> = BTreeSet::new();
    for input in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = input else { continue };
        let arg_ty = &*pt.ty;
        if let Some(entry) = registry.input_entry(arg_ty) {
            if let Some(fqn) = entry.metadata.throws.clone() {
                throws_fqns.insert(fqn);
            }
            for stage in &entry.pre_stages {
                throws_fqns.insert(stage.throws_fqn.clone());
            }
        }
    }
    let return_ty: syn::Type = match &f.sig.output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, ty) => (**ty).clone(),
    };
    if let Some(entry) = registry.output_entry(&return_ty) {
        if let Some(fqn) = entry.metadata.throws.clone() {
            throws_fqns.insert(fqn);
        }
        for stage in &entry.pre_stages {
            throws_fqns.insert(stage.throws_fqn.clone());
        }
    }
    if !throws_fqns.is_empty() {
        let parts: Vec<String> = throws_fqns
            .iter()
            .map(|fqn| format!("{}::class", register_fqn(fqn, imports)))
            .collect();
        let _ = writeln!(out, "@Throws({})", parts.join(", "));
    }
    let param_list: Vec<String> = params
        .iter()
        .filter(|p| !matches!(p.mode, ParamMode::PromotedBorrow | ParamMode::PromotedConsume | ParamMode::PromotedPassThrough))
        .map(|p| format!("{}: {}", p.kt_name, p.kt_type))
        .collect();
    let _ = write!(out, "public fun {kt_name}({})", param_list.join(", "));
    if !kt_return.is_empty() {
        let _ = write!(out, ": {kt_return}");
    }
    let _ = writeln!(out, " =");
    let _ = writeln!(out, "    {body_expr}");
    Some((out, kind))
}

/// One arm of an `impl Into<T>` parameter's Java-side dispatch tree.
/// Produced by [`build_dispatch_arms`] from the
/// [`IntoSource`] list the resolver stored on
/// `TypeEntry::into_sources`.
struct DispatchArm {
    /// `is <KotlinShortName>` check, or `None` for the unconditional
    /// catch-all arm placed last. Examples:
    /// * `Some("JNISession")` — typed-FQN arm; the JNI dispatcher's
    ///   matching arm does `instanceof io/zenoh/jni/JNISession` and
    ///   reads the pointer via `.peek()`. Kotlin holds the lock via
    ///   `.<lock_qual>` and passes the typed handle to JNI unchanged.
    /// * `Some("NativeHandle")` — generic opaque catch-all for
    ///   sources whose Kotlin class isn't registered as a typed FQN.
    ///   The JNI dispatcher's matching arm does `instanceof
    ///   java.lang.Long` and reads the autoboxed long via
    ///   `longValue()`; Kotlin unwraps to `Long` via
    ///   `.<lock_qual> { ptr -> ... }` and passes `ptr` (autoboxed).
    /// * `None` — final else; emits the JNI call unconditionally on
    ///   the raw `Any` parameter. Covers non-opaque source kinds
    ///   (e.g. `String`, `Int`) whose JNI side does its own
    ///   per-class `instanceof` checks downstream of the wire.
    runtime_check: Option<String>,
    /// `withPtr` / `consume` — scope qualifier on the typed handle
    /// (`is NativeHandle` arms only). `None` for the non-handle
    /// catch-all (no lock to acquire).
    lock_qual: Option<&'static str>,
    /// `true` → JNI receives `<name>_ptr` (the `Long` extracted by
    /// `.withPtr`/`.consume`, autoboxed to `java.lang.Long`).
    /// `false` → JNI receives the parameter as-is (typed handle for
    /// typed-FQN arms, raw value for the catch-all). The two cases
    /// pair with the JNI-side `instanceof` shape — `java.lang.Long`
    /// vs typed FQN vs whatever non-opaque source class.
    unwrap_to_ptr: bool,
}

/// Translate the resolver-recorded `IntoSource` list into the Kotlin
/// emit's per-arm dispatch shape. Arm ordering matters: typed-FQN
/// opaque arms come first (so they aren't swallowed by the catch-all
/// else), then the final unconditional `else` branch handling every
/// non-opaque source class (`String`, primitives, etc.) — the JNI
/// dispatcher does its own `instanceof` chain on the wire side for
/// those.
///
/// Opaque sources without a typed FQN are a build-time error in
/// `jobject_to_wire_adapter` (it panics with a registration hint),
/// so this helper never has to emit a generic `is NativeHandle`
/// fallback arm — every opaque source is either typed or rejected.
fn build_dispatch_arms(
    sources: &[IntoSource],
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    imports: &mut BTreeSet<String>,
) -> Vec<DispatchArm> {
    let mut typed: Vec<DispatchArm> = Vec::new();
    for src in sources {
        let canon = TypeKey::from_type(&src.source_type).as_str().to_string();
        // Only opaque sources need an `is <KotlinClass>` arm; the rest
        // (String, primitives) fall through to the catch-all else
        // where the JNI dispatcher's own per-class `instanceof` chain
        // takes over.
        let is_opaque = registry
            .input_entry(&src.source_type)
            .map(|e| converter_returns_owned_object(&e.function.sig.output))
            .unwrap_or(false);
        if !is_opaque {
            continue;
        }
        let qual: &'static str = match src.mode {
            IntoSourceMode::Borrow => "withPtr",
            IntoSourceMode::Consume => "consume",
        };
        let fqn = kotlin_types.lookup(&canon).unwrap_or_else(|| {
            panic!(
                "build_dispatch_arms: opaque source `{}` has no Kotlin FQN registered \
                 — register one via `JniExt::kotlin_type_fqn(...)` and ensure the \
                 corresponding Kotlin class exists.",
                canon
            )
        });
        let short = register_fqn(fqn, imports);
        typed.push(DispatchArm {
            runtime_check: Some(short),
            lock_qual: Some(qual),
            unwrap_to_ptr: false,
        });
    }

    let mut arms = typed;
    // Final unconditional else — JNI dispatcher's own `instanceof`
    // chain handles non-opaque source classes (String, etc.).
    arms.push(DispatchArm {
        runtime_check: None,
        lock_qual: None,
        unwrap_to_ptr: false,
    });
    arms
}


/// Fall-back Kotlin type derived directly from the JNI wire type.
/// Returns the **non-nullable** Kotlin base name — the use site adds
/// a `?` suffix when the entry's Rust type is `Option<…>` (via
/// [`is_option_type`]), so this helper must not double up.
pub(crate) fn kotlin_for_wire(wire: &syn::Type) -> Option<String> {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            let name = last.ident.to_string();
            let kt = match name.as_str() {
                "jboolean" => "Boolean",
                "jbyte" => "Byte",
                "jchar" => "Char",
                "jshort" => "Short",
                "jint" => "Int",
                "jlong" => "Long",
                "jfloat" => "Float",
                "jdouble" => "Double",
                "JString" | "jstring" => "String",
                "JByteArray" | "jbyteArray" => "ByteArray",
                "JObject" | "jobject" => "Any",
                "JClass" => "Any",
                _ => return None,
            };
            return Some(kt.to_string());
        }
    }
    None
}

/// Returns `(kt_return, opaque_ctor)` where:
/// * `kt_return` is the declared Kotlin return type written in the
///   wrapper's signature (empty for `Unit`).
/// * `opaque_ctor` is `Some(<ctor>)` when the return is an
///   opaque-handle type (jlong wire) — `<ctor>` is the registered
///   typed FQN's short name (e.g. `JNIKeyExpr`) when one is mapped,
///   else `NativeHandle`. The wrapper body uses this to construct
///   the returned object so its runtime class survives downstream
///   `instanceof` checks at the JNI boundary. `None` for non-opaque
///   returns. The declared `kt_return` deliberately stays at the
///   base `NativeHandle` for opaque returns even when a typed FQN
///   exists — minimises caller-side churn (typed instance, upcast
///   declared type).
fn classify_return(
    ext: &JniExt,
    output: &syn::ReturnType,
    registry: &Registry<KotlinMeta>,
    imports: &mut BTreeSet<String>,
) -> Option<(String, Option<String>)> {
    let ty = match output {
        syn::ReturnType::Default => return Some((String::new(), None)),
        syn::ReturnType::Type(_, t) => &**t,
    };
    let outer_meta = registry.output_entry(ty).map(|e| e.metadata.clone());
    // Unit returns (incl. `ZResult<()>`, whose inner identity rides
    // `value_rust_key`) declare no Kotlin return type.
    let inner_canon = outer_meta
        .as_ref()
        .and_then(|m| m.value_rust_key.clone())
        .unwrap_or_else(|| ty.to_token_stream().to_string());
    let inner: syn::Type = syn::parse_str(&inner_canon).unwrap_or_else(|_| ty.clone());
    if crate::util::is_unit(&inner) {
        return Some((String::new(), None));
    }
    // Opaque-handle return: read the folded `HandleInfo` the type-unfolding
    // mechanism propagated onto this return type's converter metadata —
    // one source of truth, no shape-specific peeling. The declared return
    // type is the concrete typed handle (`AutoCloseable`, so the caller can
    // `close()` / `use {}`); `opaque_ctor` is the typed short name the
    // wrapper body uses to wrap the jlong so the runtime class survives
    // downstream `instanceof` checks.
    if let Some(h) = outer_meta.as_ref().and_then(|m| m.handle.clone()) {
        let fqn = ext
            .kotlin_type_fqns
            .iter()
            .find(|(k, _)| k == &h.leaf_key)
            .map(|(_, v)| v.clone());
        return Some(match fqn {
            Some(fqn) => {
                let short = register_fqn(&fqn, imports);
                (render_handle_type(&h.strategy, &short), Some(short))
            }
            // No typed FQN registered — fall back to the base harness class.
            None => {
                let base = ext.mangle_harness("NativeHandle");
                (base.clone(), Some(base))
            }
        });
    }
    // Non-opaque: read the Kotlin name straight off the resolved
    // output entry's metadata — the rank-N handler propagates
    // `ZResult<T>` / `Option<T>` / `Vec<T>` derivations alongside the
    // wire, so no peel-and-fallback chain is needed at the use site.
    if let Some(out_entry) = registry.output_entry(ty) {
        if let Some(kt) = out_entry.metadata.kotlin_name.clone() {
            return Some((register_fqn(&kt, imports), None));
        }
    }
    None
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in s.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}
