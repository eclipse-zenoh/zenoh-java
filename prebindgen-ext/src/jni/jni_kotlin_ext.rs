//! `KotlinExt` impl for [`JniExt`].
//!
//! [`JniExt::write_kotlin`] is the single entry point for every Kotlin
//! file the JNI back-end emits. Given one `kotlin_root` it writes:
//!   * `NativeHandle.kt` (package `io.zenoh.jni`).
//!   * One typed-handle class per `kotlin_class` entry without
//!     `.suppress_kotlin_code()`.
//!   * `JNIWrappers.kt` — top-level safe wrappers for non-promoted fns.
//!   * One Kotlin fun-interface file per `impl Fn(args) + Send + Sync
//!     + 'static` type, named via [`JniExt::kotlin_callback_name_mangle`]
//!     (default = identity over the `"On"`-prefixed auto-derived name;
//!     in zenoh-jni: `JNIOn<Args>`). Callback types overridden via
//!     [`JniExt::callback_input`] are skipped — the override points at
//!     a hand-written interface.
//!
//! All emitters route through [`KotlinFile::write`], which translates
//! `package` into a sub-path under `kotlin_root`.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use quote::ToTokens;

use crate::core::prebindgen_ext::{IntoSource, IntoSourceMode};
use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::jni_ext::{converter_returns_owned_object, JniExt, KotlinMeta};
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
    /// `#[prebindgen]` fn idents promoted to instance methods on this
    /// class. The matching opaque first parameter is dropped from the
    /// signature; the method uses inherited `withPtr` / `consume`. An
    /// empty slice emits a pure shell (just `free()` + the matching
    /// `<mangle_fun("freePtr")>` extern).
    pub functions: &'a [&'a str],
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
    /// classes (one per `kotlin_class` registration), and
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
        written.push(self.write_native_handle(kotlin_root)?);

        // Build the borrowed `TypedHandle<'_>` view from internal config.
        // The two layers (owned + slice-of-borrowed) keep the borrow
        // checker happy with the existing internal API.
        let owned = self.collect_typed_handles();
        let method_refs: Vec<Vec<&str>> = owned
            .iter()
            .map(|h| h.methods.iter().map(String::as_str).collect())
            .collect();
        let typed_handles: Vec<TypedHandle<'_>> = owned
            .iter()
            .zip(method_refs.iter())
            .map(|(h, m)| TypedHandle {
                rust_doc: &h.rust_doc,
                kotlin_fqn: &h.kotlin_fqn,
                functions: m.as_slice(),
            })
            .collect();
        let kotlin_types = self.build_kotlin_type_map();
        written.extend(self.write_typed_handles(
            &typed_handles,
            registry,
            &kotlin_types,
            kotlin_root,
        )?);
        written.push(self.write_jni_wrappers(
            registry,
            &kotlin_types,
            &typed_handles,
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
                methods: opaque.methods.clone(),
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
    pub methods: Vec<String>,
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
        let file = KotlinFile {
            package: self.package.clone(),
            class_name: "NativeHandle".into(),
            contents: render_native_handle_source(&self.package, &exc.kotlin_fqn),
        };
        Ok(file.write(output_dir)?)
    }

    /// Emit one Kotlin file per registered
    /// [`Self::kotlin_exception_class`] — each becomes a
    /// `public class <Name>(message: String? = null) : Exception()`
    /// landing under `<package>/<Name>.kt`. Iterates `self.exceptions`
    /// in declaration order; returns every path written.
    pub(crate) fn write_exception_classes(
        &self,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        let mut written = Vec::new();
        for exc in &self.exceptions {
            let (package, class_name) = match exc.kotlin_fqn.rsplit_once('.') {
                Some((p, c)) => (p.to_string(), c.to_string()),
                None => (String::new(), exc.kotlin_fqn.clone()),
            };
            let file = KotlinFile {
                contents: render_exception_source(&package, &class_name, &exc.rust_short),
                package,
                class_name,
            };
            written.push(file.write(output_dir)?);
        }
        Ok(written)
    }

    /// Emit one Kotlin `enum class` file per `kotlin_enum`-declared type
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
                contents: render_enum_source(&package, &class_name, item_enum),
                package,
                class_name,
            };
            written.push(file.write(output_dir)?);
        }
        Ok(written)
    }

    /// Emit `JNIWrappers.kt` under `output_dir` (package
    /// `io.zenoh.jni`). One top-level Kotlin function per
    /// `#[prebindgen]` function — **except** those promoted to a typed
    /// handle via `typed_handles` (their wrappers live on the handle
    /// class instead). Opaque-handle parameters become `NativeHandle`;
    /// the wrapper body nests `withPtr` / `consume` per the
    /// type-conversion rule (`&T` → `withPtr`, `T` → `consume`), then
    /// delegates to the matching `JNINative.<mangle_fun(name)>(...)`.
    /// Non-opaque parameters pass through with the Kotlin type from
    /// `kotlin_types`. Opaque-handle return values are wrapped in
    /// `NativeHandle(...)` before being returned.
    pub(crate) fn write_jni_wrappers(
        &self,
        registry: &Registry<KotlinMeta>,
        kotlin_types: &KotlinTypeMap,
        typed_handles: &[TypedHandle<'_>],
        output_dir: &Path,
    ) -> Result<PathBuf, WriteKotlinError> {
        let promoted: HashSet<String> = typed_handles
            .iter()
            .flat_map(|h| h.functions.iter().map(|s| (*s).to_string()))
            .collect();
        let contents = render_jni_wrappers_source(self, registry, kotlin_types, &promoted);
        let file = KotlinFile {
            package: self.package.clone(),
            class_name: "JNIWrappers".into(),
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
            // Find the Rust type-key whose registered FQN matches this
            // handle. Only required when `functions` is non-empty —
            // otherwise we're emitting a pure shell with no promoted
            // methods and the key is unused.
            let rust_key = if handle.functions.is_empty() {
                None
            } else {
                let key = rust_key_for_fqn(self, handle.kotlin_fqn).unwrap_or_else(|| {
                    panic!(
                        "write_typed_handles: kotlin_fqn `{}` is not registered via \
                         JniExt::kotlin_type_fqn — required to identify the typed \
                         handle's Rust type-key for promoted-method param matching.",
                        handle.kotlin_fqn
                    )
                });
                Some(key.to_string())
            };
            let file = KotlinFile {
                contents: render_typed_handle_source(
                    self,
                    &package,
                    &class_name,
                    handle.rust_doc,
                    handle.functions,
                    rust_key.as_deref(),
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

    let contents = render_kotlin_interface(&package, &class_name, &params, &used_fqns);
    KotlinFile {
        package,
        class_name,
        contents,
    }
}

fn render_kotlin_interface(
    package: &str,
    class_name: &str,
    params: &[String],
    used_fqns: &BTreeSet<String>,
) -> String {
    let mut imports: Vec<String> = used_fqns
        .iter()
        .filter(|fqn| {
            let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            !pkg.is_empty() && pkg != package
        })
        .cloned()
        .collect();
    imports.sort();
    imports.dedup();

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        out.push_str(&format!("package {}\n\n", package));
    }
    for imp in &imports {
        out.push_str(&format!("import {}\n", imp));
    }
    if !imports.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("public fun interface {} {{\n", class_name));
    if params.is_empty() {
        out.push_str("    fun run()\n");
    } else {
        out.push_str("    fun run(\n");
        for p in params {
            out.push_str(p);
            out.push('\n');
        }
        out.push_str("    )\n");
    }
    out.push_str("}\n");
    out
}

/// Derive the auto-callback short Kotlin name for an `impl Fn(args)`
/// signature. Always starts with the hardcoded `"On"` and appends each
/// parameter type's Rust short ident (`Fn(Query)` → `"OnQuery"`,
/// `Fn(Reply)` → `"OnReply"`, `Fn(K, V)` → `"OnKV"`, `Fn()` → just
/// `"On"`). The result feeds [`JniExt::mangle_callback`] before the
/// FQN is qualified against [`JniExt::kotlin_callback_package`].
pub(crate) fn derive_callback_name(args: &[syn::Type]) -> String {
    let mut s = String::from("On");
    for a in args {
        s.push_str(&type_short_ident(a));
    }
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

fn register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}

// ── NativeHandle prerequisite + safe-wrappers emitters ────────────────

/// One generated Kotlin exception class — `public class <Name>(message:
/// String? = null) : Exception()` under `<package>`. Mirrors the
/// hand-written `io.zenoh.exceptions.ZError` it replaces; the
/// constructor surface (optional message) stays bit-identical so the
/// move is invisible to Kotlin callers once they update their imports.
fn render_exception_source(package: &str, class_name: &str, rust_doc_name: &str) -> String {
    let mut s = String::new();
    s.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        s.push_str(&format!("package {}\n\n", package));
    }
    s.push_str(&format!(
        "/** JVM-side surface for the native Rust `{}` error. */\n",
        rust_doc_name
    ));
    s.push_str(&format!(
        "public class {}(override val message: String? = null) : Exception()\n",
        class_name
    ));
    s
}

/// One generated Kotlin `enum class` source — variants in
/// SCREAMING_SNAKE_CASE, each carrying the Rust discriminant as a
/// `val value: Int`, plus a `fromInt(value: Int)` companion. Mirrors
/// the hand-written `io.zenoh.qos.Priority` shape so adapter code that
/// already speaks the `.value` / `.fromInt(...)` idiom keeps working.
fn render_enum_source(package: &str, class_name: &str, item_enum: &syn::ItemEnum) -> String {
    // Same discriminant source of truth the Rust `jint → variant` decode
    // uses, so Kotlin `value(N)` and the generated decode agree.
    let variants: Vec<(String, i64)> = crate::util::enum_discriminant_values(item_enum)
        .into_iter()
        .map(|(ident, value)| {
            (crate::util::camel_to_screaming_snake(&ident.to_string()), value)
        })
        .collect();

    let mut s = String::new();
    s.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        s.push_str(&format!("package {}\n\n", package));
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
    s.push_str(&format!(
        "        public fun fromInt(value: Int): {} = entries.first {{ it.value == value }}\n",
        class_name
    ));
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

/// `NativeHandle.kt` source — emitted verbatim into the destination
/// Kotlin source tree. Owns the read/write lock that gates every
/// auto-generated wrapper's access to the underlying `Long` pointer.
fn render_native_handle_source(package: &str, exception_fqn: &str) -> String {
    let exc_short = exception_fqn.rsplit('.').next().unwrap_or(exception_fqn);
    // Import only when the exception lives in a different package — a
    // same-package import is technically legal but flagged as redundant
    // by Kotlin's linter, and the default FQN
    // (`<package>.<rust_short>` of the primary exception) usually
    // matches NativeHandle's own package.
    let exception_pkg = exception_fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
    let import_line = if exception_fqn.contains('.') && exception_pkg != package {
        format!("import {}\n", exception_fqn)
    } else {
        String::new()
    };
    let body = r#"
__EXCEPTION_IMPORT__
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

/**
 * Race-free wrapper around a raw `Box<T>` pointer obtained from native
 * code via `Box::into_raw(Box::new(v))`. Pairs the pointer with a
 * `ReentrantReadWriteLock` so that borrow-style JNI calls run in
 * parallel under the read lock and consume/free serialise against
 * them under the write lock.
 *
 * This is the Java-side half of the type-conversion rule for opaque
 * handles: `&T` parameters route through [withPtr] (borrow); by-value
 * `T` parameters route through [consume] (write lock, slot stays
 * valid during the action, then null-ed in `finally`); destructor
 * entry points without a matching `#[prebindgen]` fn use [free]. The
 * auto-generated wrappers in `JNIWrappers.kt` are the only callers
 * that need to know which mode applies.
 *
 * Marked `open` so the hand-maintained `JNI*.kt` typed-handle classes
 * can subclass for type safety while inheriting the lock contract.
 */
public open class NativeHandle(initial: Long) {
    private val lock = ReentrantReadWriteLock()

    /** Volatile so [peek] is atomic on 32-bit JVMs and observes the
     *  write done by [free] / [consume] without holding the lock. */
    @Volatile private var ptr: Long = initial

    /**
     * Run [block] with the live pointer under the read lock. Throws
    * [ZError] if [free] has already released the handle. Multiple
    * concurrent invocations run in parallel; only [free]/[consume]
     * are serialised against them.
     */
    @Throws(ZError::class)
    public fun <T> withPtr(block: (Long) -> T): T = lock.read {
        val p = ptr
        if (p == 0L) throw ZError("Operation on a closed native handle.")
        block(p)
    }

    /**
     * Take the pointer under the write lock and pass it to [freeFn]
     * exactly once. Subsequent [free] calls are no-ops. Blocks until
     * all in-flight [withPtr] calls finish.
     */
    public fun free(freeFn: (Long) -> Unit) {
        lock.write {
            val p = ptr
            if (p == 0L) return@write
            ptr = 0L
            freeFn(p)
        }
    }

    /**
     * Consume the pointer: take it under the write lock, run [action]
     * with the captured pointer, then null the slot — even if [action]
     * throws. Used by the generator-emitted wrappers whose Rust side
     * runs `*Box::from_raw(...)` — i.e. by-value `T` opaque-handle
     * parameters and `impl Into<T>` arms with `IntoSourceMode::Consume`.
     *
     * The slot stays valid during [action] so the wrapper can pass the
     * typed handle to JNI and have JNI extract the pointer via [peek]
     * — symmetric with [withPtr] for the Borrow path. Unique-ownership
     * is still guaranteed: the write lock excludes every other
    * [withPtr] / [consume] / [free], and the `finally` clause
     * unconditionally nulls the slot before the lock is released, so
    * the next [withPtr] / [consume] / [free] sees `ptr == 0` and
     * cannot reach the freed allocation.
     *
     * Throws if the handle has already been closed/consumed.
     */
    @Throws(ZError::class)
    public fun <R> consume(action: (Long) -> R): R = lock.write {
        val p = ptr
        if (p == 0L) throw ZError("Operation on a closed native handle.")
        try {
            action(p)
        } finally {
            ptr = 0L
        }
    }

    /** True iff [free] has run. */
    public fun isClosed(): Boolean = lock.read { ptr == 0L }

    /**
     * Read the current pointer value without holding the lock.
     * Returns `0L` if the handle has been closed/consumed.
     */
    public fun peek(): Long = ptr
}
"#;
    // Substitute the configured exception class into the placeholder slots:
    // the `__EXCEPTION_IMPORT__` line becomes the configured FQN's `import`
    // (or vanishes if the class is package-local), and every `ZError` token
    // in `@Throws`/`throw`/KDoc becomes the configured short name. Trim the
    // trailing newline of `import_line` when no import is needed so we don't
    // leave a blank line where the placeholder was.
    let import_replacement = if import_line.is_empty() {
        // No import — also drop the placeholder line entirely.
        String::new()
    } else {
        import_line.trim_end_matches('\n').to_string()
    };
    let body = body
        .replace("__EXCEPTION_IMPORT__\n", &if import_replacement.is_empty() {
            String::new()
        } else {
            format!("{}\n", import_replacement)
        })
        .replace("ZError", exc_short);

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !package.is_empty() {
        out.push_str(&format!("package {}\n", package));
    }
    out.push_str(&body);
    out
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
    promoted_functions: &[&str],
    promoted_rust_key: Option<&str>,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
) -> String {
    // Build method bodies first so we can collect imports up front.
    // Two buckets — instance methods land in the class body; companion
    // methods are wrapped in a `companion object { ... }` block.
    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut instance_body = String::new();
    let mut companion_body = String::new();
    for fn_name in promoted_functions {
        let ident = syn::Ident::new(fn_name, proc_macro2::Span::call_site());
        let (item_fn, _loc) = registry.functions.get(&ident).unwrap_or_else(|| {
            panic!(
                "render_typed_handle_source: `{class_name}` promotes function `{fn_name}` \
                 which is not present in `registry.functions` — check the spelling against \
                 the matching `#[prebindgen]` Rust fn name."
            )
        });
        let (block, kind) = render_wrapper_fn(
            ext,
            item_fn,
            registry,
            kotlin_types,
            &mut imports,
            promoted_rust_key,
        )
        .unwrap_or_else(|| {
            panic!(
                "render_typed_handle_source: `{class_name}` promotes function `{fn_name}` \
                 but its parameter types couldn't be Kotlin-resolved — verify that all \
                 non-opaque parameter types are registered in `kotlin_types`."
            )
        });
        let bucket = match kind {
            MethodKind::Instance => &mut instance_body,
            MethodKind::Companion => &mut companion_body,
        };
        if !bucket.is_empty() {
            bucket.push('\n');
        }
        for line in block.lines() {
            if line.is_empty() {
                bucket.push('\n');
            } else {
                bucket.push_str(line);
                bucket.push('\n');
            }
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
    if !promoted_functions.is_empty() {
        // Exception import (if any) is included in `import_list` via the
        // per-method `@Throws` emission — nothing hardcoded here.
        for imp in &import_list {
            s.push_str(&format!("import {}\n", imp));
        }
        s.push('\n');
    }
    s.push_str(&format!(
        "/** Typed [NativeHandle] for a native Zenoh `{}`. */\n",
        rust_doc_name
    ));
    s.push_str(&format!(
        "public class {}(initialPtr: Long) : NativeHandle(initialPtr) {{\n",
        class_name
    ));
    let free_extern = ext.mangle_fun("freePtr");
    s.push_str(&format!(
        "    public fun free() = free {{ {free_extern}(it) }}\n"
    ));
    s.push_str(&format!(
        "    private external fun {free_extern}(ptr: Long)\n"
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
    if !companion_body.is_empty() {
        s.push('\n');
        s.push_str("    public companion object {\n");
        for line in companion_body.lines() {
            if line.is_empty() {
                s.push('\n');
            } else {
                s.push_str("        ");
                s.push_str(line);
                s.push('\n');
            }
        }
        s.push_str("    }\n");
    }
    s.push_str("}\n");
    s
}

/// Emit one safe top-level wrapper function per `#[prebindgen]` fn in
/// `registry.functions`. Opaque-handle parameters (detected via the
/// input converter returning `OwnedObject<T>`) become `NativeHandle`;
/// the wrapper body nests `withPtr` / `consume` per the syntactic
/// shape. Non-opaque parameters pass through with the Kotlin type from
/// `kotlin_types`. The wrappers delegate to
/// `JNINative.<mangle_fun(name)>(...)`.
fn render_jni_wrappers_source(
    ext: &JniExt,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    promoted: &HashSet<String>,
) -> String {
    // Start with the auto-derived callback FQNs and let user-provided
    // entries WIN — the user (build.rs) may need to override e.g.
    // `impl Fn (Query)` to point at a hand-written
    // `JNIQueryableCallback` instead of the auto-derived
    // `JNIQueryCallback`.
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

    // Deterministic order so the emitted file is stable across builds.
    let mut idents: Vec<&syn::Ident> = registry.functions.keys().collect();
    idents.sort();

    for ident in idents {
        // Skip functions promoted to a typed-handle class — their
        // top-level wrapper lives on the handle instead. The Rust-side
        // `JNINative.<mangle_fun(name)>` extern still exists; only the
        // Kotlin-side safe wrapper moves.
        if promoted.contains(&ident.to_string()) {
            continue;
        }
        let (item_fn, _loc) = &registry.functions[ident];
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
        ) {
            body.push_str(&block);
            body.push('\n');
        }
    }

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    if !ext.package.is_empty() {
        out.push_str(&format!("package {}\n\n", ext.package));
    }
    // Exception imports (if any) are added to `imports` by the per-wrapper
    // `@Throws` emission, so no error class is hardcoded here.
    for imp in &imports {
        out.push_str(&format!("import {}\n", imp));
    }
    out.push('\n');
    // Wrap the wrappers in an `object` so the names don't collide with
    // same-named methods on the hand-maintained JNI*.kt classes
    // (`put`, `delete`, `close`, etc.). Callers use `JNIWrappers.put(...)`.
    out.push_str("public object JNIWrappers {\n");
    // Indent each emitted wrapper body by 4 spaces.
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
) -> Option<(String, MethodKind)> {
    use std::fmt::Write;

    let rust_name = f.sig.ident.to_string();
    let kt_name = snake_to_camel(&rust_name);
    let jni_call = ext.mangle_fun(&kt_name);

    // Pre-parse the promoted Rust type-key (if any) so per-param matching
    // is whitespace-normalised against the canonical form.
    let promoted_key: Option<TypeKey> =
        promoted_handle.map(|s| TypeKey::parse(s));

    // Classify each parameter.
    struct Param {
        kt_name: String,
        kt_type: String,
        mode: ParamMode,
        /// `true` when the param's Rust type is a `kotlin_enum`-declared
        /// enum: the high-level Kotlin signature uses the typed enum
        /// (`Priority`), but the underlying JNI `external fun` declares
        /// the param as `Int` (jint wire). The wrapper bridges the two
        /// by passing `<name>.value` at the call site.
        as_enum_value: bool,
    }
    enum ParamMode {
        Borrow,      // &T opaque-handle → withPtr
        Consume,     // T  opaque-handle → consume
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
        let is_opaque = converter_returns_owned_object(&entry.function.sig.output);

        let (kt_type_raw, optional) = if is_opaque {
            ("NativeHandle".to_string(), false)
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
            if matches_promoted {
                promoted_taken = true;
                if borrow { ParamMode::PromotedBorrow } else { ParamMode::PromotedConsume }
            } else if borrow {
                ParamMode::Borrow
            } else {
                ParamMode::Consume
            }
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
    let (kt_return, opaque_ctor) = classify_return(&f.sig.output, registry, kotlin_types, imports)?;

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
                | ParamMode::PromotedBorrow
                | ParamMode::PromotedConsume => format!("{}_ptr", p.kt_name),
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
        let mut call = format!("JNINative.{jni_call}({})", args.join(", "));
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
            ParamMode::Dispatch { .. } | ParamMode::PassThrough => {}
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
        .filter(|p| !matches!(p.mode, ParamMode::PromotedBorrow | ParamMode::PromotedConsume))
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

/// True iff the wire type is `jni::sys::jlong` (or the bare `jlong`
/// alias). Used to detect opaque-handle outputs that should be wrapped
/// in `NativeHandle(...)`.
fn wire_is_jlong(wire: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = wire {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "jlong";
        }
    }
    false
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
    output: &syn::ReturnType,
    registry: &Registry<KotlinMeta>,
    kotlin_types: &KotlinTypeMap,
    imports: &mut BTreeSet<String>,
) -> Option<(String, Option<String>)> {
    let ty = match output {
        syn::ReturnType::Default => return Some((String::new(), None)),
        syn::ReturnType::Type(_, t) => &**t,
    };
    // Detect opaque return: T (or a wrapper W<T>) whose input converter
    // returns `OwnedObject<T>` (i.e. opaque-handle type). The wrapped-
    // value identity is carried generically via
    // `KotlinMeta::value_rust_key` — populated by the composed branch of
    // `lookup_output` for arity-1 wrappers — so the framework doesn't
    // need to peel any specific Result/Option shape here.
    let outer_meta = registry.output_entry(ty).map(|e| e.metadata.clone());
    let inner_canon = outer_meta
        .as_ref()
        .and_then(|m| m.value_rust_key.clone())
        .unwrap_or_else(|| ty.to_token_stream().to_string());
    let inner: syn::Type = syn::parse_str(&inner_canon).unwrap_or_else(|_| ty.clone());
    if crate::util::is_unit(&inner) {
        return Some((String::new(), None));
    }
    // An output is "opaque-handle" iff its registered output converter
    // produces `jlong` (the `Box::into_raw(...) as i64` shape from
    // `opaque_handle_output`). Pull the wire type from the inner type's
    // output entry; the input-side `OwnedObject<T>` check below
    // catches anything we register only on input (rare).
    let output_is_opaque_jlong = registry
        .output_entry(&inner)
        .map(|e| wire_is_jlong(&e.destination))
        .unwrap_or(false);
    let input_is_opaque = registry
        .input_types
        .iter()
        .flat_map(|b| b.iter())
        .any(|(k, slot)| {
            slot.as_ref()
                .map(|e| {
                    k.as_str() == inner_canon
                        && converter_returns_owned_object(&e.function.sig.output)
                })
                .unwrap_or(false)
        });
    if output_is_opaque_jlong || input_is_opaque {
        // Typed-FQN constructor when registered; else NativeHandle.
        // Both compile against the same declared `NativeHandle`
        // return type (subtype relation).
        let ctor = match kotlin_types.lookup(&inner_canon) {
            Some(fqn) if fqn.contains('.') => register_fqn(fqn, imports),
            _ => "NativeHandle".to_string(),
        };
        return Some(("NativeHandle".to_string(), Some(ctor)));
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
