//! `KotlinExt` impl for [`JniExt`].
//!
//! Today's pipeline emits two kinds of Kotlin output:
//! 1. One aggregated `JNINative.kt` (interface + data classes + external
//!    funs). This is currently produced by the existing
//!    [`crate::kotlin::KotlinInterfaceGenerator`] called separately from
//!    `build.rs`.
//! 2. One `JNI<Stem>Callback.kt` per `impl Fn(args) + Send + Sync + 'static`
//!    type encountered. These get emitted here via `JniExt::write_kotlin`.
//!
//! The split is deliberate: the per-callback files are the new artifact
//! introduced by the rewrite; the aggregated interface remains the
//! responsibility of the existing generator and is not touched by JniExt.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use quote::ToTokens;

use crate::core::registry::{extract_fn_trait_args, Registry, TypeKey};
use crate::jni::jni_ext::{converter_returns_owned_object, JniExt};
use crate::kotlin::kotlin_ext::{KotlinExt, KotlinFile, WriteKotlinError};
use crate::kotlin::type_map::KotlinTypeMap;

impl KotlinExt for JniExt {
    fn write_kotlin(
        &self,
        registry: &Registry,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, WriteKotlinError> {
        // Iterate every resolved type entry in either direction and look for
        // impl Fn(...) wires. Deduplicate by canonical type key.
        let mut seen: HashSet<TypeKey> = HashSet::new();
        let mut written = Vec::new();
        let target_dir = if !self.kotlin_callback_dir.as_os_str().is_empty() {
            self.kotlin_callback_dir.clone()
        } else {
            output_dir.to_path_buf()
        };

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
                        let file = build_callback_kotlin_file(self, &args, registry);
                        // Write directly under target_dir (which is already
                        // the package-qualified callbacks directory),
                        // bypassing KotlinFile::write's package-nesting.
                        std::fs::create_dir_all(&target_dir)?;
                        let path = target_dir.join(format!("{}.kt", file.class_name));
                        std::fs::write(&path, &file.contents)?;
                        written.push(path);
                    }
                }
            }
        }
        Ok(written)
    }
}

impl JniExt {
    /// Emit `NativeHandle.kt` under `output_dir` (package
    /// `io.zenoh.jni`). The class is the Java-side half of the
    /// borrow/consume contract — `withPtr` for `&T` opaque-handle
    /// borrows, `consume` for by-value `T` opaque-handle drops. By
    /// generating it here, the prebindgen-ext pipeline owns the lock
    /// primitive the rest of the auto-generated wrappers depend on.
    pub fn write_native_handle(&self, output_dir: &Path) -> Result<PathBuf, WriteKotlinError> {
        let file = KotlinFile {
            package: "io.zenoh.jni".into(),
            class_name: "NativeHandle".into(),
            contents: render_native_handle_source(),
        };
        Ok(file.write(output_dir)?)
    }

    /// Emit `JNIWrappers.kt` under `output_dir` (package
    /// `io.zenoh.jni`). One top-level Kotlin function per
    /// `#[prebindgen]` function. Opaque-handle parameters become
    /// `NativeHandle`; the wrapper body nests `withPtr` / `consume`
    /// per the type-conversion rule
    /// (`&T` → `withPtr`, `T` → `consume`), then delegates to the
    /// matching `JNINative.<name>ViaJNI(...)`. Non-opaque parameters
    /// pass through with the Kotlin type from `kotlin_types`. Opaque-
    /// handle return values are wrapped in `NativeHandle(...)` before
    /// being returned.
    pub fn write_jni_wrappers(
        &self,
        registry: &Registry,
        kotlin_types: &KotlinTypeMap,
        output_dir: &Path,
    ) -> Result<PathBuf, WriteKotlinError> {
        let contents = render_jni_wrappers_source(self, registry, kotlin_types);
        let file = KotlinFile {
            package: "io.zenoh.jni".into(),
            class_name: "JNIWrappers".into(),
            contents,
        };
        Ok(file.write(output_dir)?)
    }

    /// Return the `<rust-type-key> → <kotlin FQN>` map for every
    /// `impl Fn(args)` type the Registry has resolved. Use this to merge
    /// into a `KotlinTypeMap` consumed by the aggregated-interface
    /// generator (so it can refer to callbacks by their Kotlin FQN).
    pub fn collect_kotlin_callback_fqns(&self, registry: &Registry) -> KotlinTypeMap {
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
                        let stem = derive_callback_stem(&args);
                        let fqn = if self.kotlin_callback_package.is_empty() {
                            format!("JNI{}Callback", stem)
                        } else {
                            format!("{}.JNI{}Callback", self.kotlin_callback_package, stem)
                        };
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
    registry: &Registry,
) -> KotlinFile {
    let stem = derive_callback_stem(args);
    let class_name = format!("JNI{}Callback", stem);
    let package = ext.kotlin_callback_package.clone();

    let kotlin_types = ext.collect_kotlin_callback_fqns(registry);

    // Resolve each arg's Kotlin type. Falls back to the bare last-segment
    // ident when not found in the map (matches today's
    // CallbacksConverter::emit_for_signature lookup behavior).
    let mut params: Vec<String> = Vec::new();
    let mut used_fqns: BTreeSet<String> = BTreeSet::new();
    for (i, arg) in args.iter().enumerate() {
        let canon = arg.to_token_stream().to_string();
        let kotlin_ty = kotlin_types
            .lookup(&canon)
            .map(str::to_string)
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

fn derive_callback_stem(args: &[syn::Type]) -> String {
    if args.is_empty() {
        return "Empty".into();
    }
    let mut s = String::new();
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

/// `NativeHandle.kt` source — emitted verbatim into the destination
/// Kotlin source tree. Owns the read/write lock that gates every
/// auto-generated wrapper's access to the underlying `Long` pointer.
fn render_native_handle_source() -> String {
    r#"// Auto-generated by JniExt — do not edit by hand.
package io.zenoh.jni

import io.zenoh.exceptions.ZError
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

/**
 * Race-free wrapper around a raw `Box<T>` pointer obtained from native
 * code via `Box::into_raw(Box::new(v))`. Pairs the pointer with a
 * `ReentrantReadWriteLock` so that borrow-style JNI calls run in
 * parallel under the read lock and consume/close serialise against
 * them under the write lock.
 *
 * This is the Java-side half of the type-conversion rule for opaque
 * handles: `&T` parameters route through [withPtr] (borrow); by-value
 * `T` parameters route through [consume] (write lock + atomic null);
 * destructor entry points without a matching `#[prebindgen]` fn use
 * [close]. The auto-generated wrappers in `JNIWrappers.kt` are the
 * only callers that need to know which mode applies.
 *
 * Marked `open` so the hand-maintained `JNI*.kt` typed-handle classes
 * can subclass for type safety while inheriting the lock contract.
 */
public open class NativeHandle(initial: Long) {
    private val lock = ReentrantReadWriteLock()

    /** Volatile so [peek] is atomic on 32-bit JVMs and observes the
     *  write done by [close] / [consume] without holding the lock. */
    @Volatile private var ptr: Long = initial

    /**
     * Run [block] with the live pointer under the read lock. Throws
     * [ZError] if [close] has already released the handle. Multiple
     * concurrent invocations run in parallel; only [close]/[consume]
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
     * exactly once. Subsequent [close] calls are no-ops. Blocks until
     * all in-flight [withPtr] calls finish.
     */
    public fun close(freeFn: (Long) -> Unit) {
        lock.write {
            val p = ptr
            if (p == 0L) return@write
            ptr = 0L
            freeFn(p)
        }
    }

    /**
     * Consume the pointer: take it under the write lock, atomically
     * null the field, and pass the captured pointer to [action]. Used
     * by the generator-emitted wrappers whose Rust side runs
     * `*Box::from_raw(...)` — i.e. by-value `T` opaque-handle
     * parameters. The write lock + atomic-null give Rust a
     * unique-ownership guarantee: the boxed allocation has no
     * surviving borrows when `Box::from_raw` reconstructs it.
     * Throws if the handle has already been closed/consumed.
     */
    @Throws(ZError::class)
    public fun <R> consume(action: (Long) -> R): R = lock.write {
        val p = ptr
        if (p == 0L) throw ZError("Operation on a closed native handle.")
        ptr = 0L
        action(p)
    }

    /** True iff [close] has run. */
    public fun isClosed(): Boolean = lock.read { ptr == 0L }

    /**
     * Read the current pointer value without holding the lock.
     * Returns `0L` if the handle has been closed/consumed.
     */
    public fun peek(): Long = ptr
}
"#
    .to_string()
}

/// Emit one safe top-level wrapper function per `#[prebindgen]` fn in
/// `registry.functions`. Opaque-handle parameters (detected via the
/// input converter returning `OwnedObject<T>`) become `NativeHandle`;
/// the wrapper body nests `withPtr` / `consume` per the syntactic
/// shape. Non-opaque parameters pass through with the Kotlin type from
/// `kotlin_types`. The wrappers delegate to
/// `JNINative.<name>ViaJNI(...)`.
fn render_jni_wrappers_source(
    ext: &JniExt,
    registry: &Registry,
    kotlin_types: &KotlinTypeMap,
) -> String {
    use std::fmt::Write;

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
        let (item_fn, _loc) = &registry.functions[ident];
        if let Some(block) = render_wrapper_fn(item_fn, registry, &merged_types, &mut imports) {
            body.push_str(&block);
            body.push('\n');
        }
    }

    let mut out = String::new();
    out.push_str("// Auto-generated by JniExt — do not edit by hand.\n");
    out.push_str("package io.zenoh.jni\n\n");
    out.push_str("import io.zenoh.exceptions.ZError\n");
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

/// Emit a single wrapper function. Returns `None` if the function has
/// a parameter whose Kotlin type isn't registered (in that case we
/// skip the function rather than panicking — the legacy `JNINative.kt`
/// retains the unwrapped external fun so callers still have an
/// escape hatch).
fn render_wrapper_fn(
    f: &syn::ItemFn,
    registry: &Registry,
    kotlin_types: &KotlinTypeMap,
    imports: &mut BTreeSet<String>,
) -> Option<String> {
    use std::fmt::Write;

    let rust_name = f.sig.ident.to_string();
    let kt_name = snake_to_camel(&rust_name);
    let jni_call = format!("{kt_name}ViaJNI");

    // Classify each parameter.
    struct Param {
        kt_name: String,
        kt_type: String,
        mode: ParamMode,
    }
    enum ParamMode {
        Borrow,       // &T opaque-handle → withPtr
        Consume,      // T  opaque-handle → consume
        /// `impl Into<T>` (Kotlin `Any`) — at runtime an arm matches
        /// a `NativeHandle` (then `.withPtr` / `.consume` per
        /// `consume_dispatch`) or a non-handle source (pass-through).
        /// `consume_dispatch = true` when the impl-Into dispatcher
        /// reconstructs `Box<T>` for the opaque arm, requiring the
        /// Kotlin caller to surrender the slot via
        /// `NativeHandle.consume`.
        DispatchAny { consume_dispatch: bool },
        PassThrough,
    }

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
            // Look up the Kotlin type via the merged type map; fall
            // back to deriving from the wire type when the param
            // isn't pre-registered (e.g. `impl Into<T>` shapes wired
            // as `JObject`).
            let key = match arg_ty {
                syn::Type::Reference(r) => r.elem.to_token_stream().to_string(),
                _ => arg_ty.to_token_stream().to_string(),
            };
            let kt = kotlin_types
                .lookup(&key)
                .map(str::to_string)
                .or_else(|| kotlin_for_wire(&entry.destination))?;
            let opt = is_option_type(arg_ty);
            (kt, opt)
        };

        // Mode: opaque → Borrow/Consume by Rust syntactic shape.
        // Non-opaque → `Any` triggers DispatchAny (`impl Into<T>` wire
        // = JObject; runtime might be a NativeHandle that we should
        // unwrap before crossing the JNI boundary). Everything else
        // (primitives, callbacks, data classes) passes through.
        let mode = if is_opaque {
            if matches!(arg_ty, syn::Type::Reference(_)) {
                ParamMode::Borrow
            } else {
                ParamMode::Consume
            }
        } else if kt_type_raw == "Any" {
            // The Rust dispatcher's body tells us whether the opaque
            // arm consumes the `Box` (`Box::from_raw`) or borrow-clones
            // it (`OwnedObject::from_raw(...).clone()`). Inspect the
            // already-emitted function for the consume marker so the
            // Kotlin side matches: `consume` writes-locks + nulls the
            // slot, while `withPtr` keeps it alive.
            let consume = dispatcher_consumes_opaque(&entry.function);
            ParamMode::DispatchAny { consume_dispatch: consume }
        } else {
            ParamMode::PassThrough
        };

        let short = register_fqn(&kt_type_raw, imports);
        let suffix = if optional { "?" } else { "" };
        params.push(Param {
            kt_name: name,
            kt_type: format!("{short}{suffix}"),
            mode,
        });
    }

    // Return type: peel ZResult<...>; detect opaque-handle return.
    let (kt_return, return_is_opaque) = classify_return(&f.sig.output, registry, kotlin_types, imports)?;

    // Helper: build the JNINative call for a given DispatchAny "unwrap mask".
    // mask bit k = 1 means dispatch_indices[k] is unwrapped (use `<name>_ptr`).
    let dispatch_indices: Vec<usize> = params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| matches!(p.mode, ParamMode::DispatchAny { .. }).then_some(i))
        .collect();

    let build_call = |mask: u32| -> String {
        let mut args: Vec<String> = Vec::with_capacity(params.len());
        for (i, p) in params.iter().enumerate() {
            let arg = match p.mode {
                ParamMode::Borrow | ParamMode::Consume => format!("{}_ptr", p.kt_name),
                ParamMode::DispatchAny { .. } => {
                    let pos = dispatch_indices.iter().position(|&di| di == i).unwrap();
                    if (mask >> pos) & 1 == 1 {
                        format!("{}_ptr", p.kt_name)
                    } else {
                        p.kt_name.clone()
                    }
                }
                ParamMode::PassThrough => p.kt_name.clone(),
            };
            args.push(arg);
        }
        let mut call = format!("JNINative.{jni_call}({})", args.join(", "));
        if return_is_opaque {
            call = format!("NativeHandle({call})");
        }
        call
    };

    // Per-DispatchAny scope qualifier: `withPtr` (Borrow) or
    // `consume` (Consume). Matches the Rust dispatcher's opaque-arm
    // body (`.clone()` vs `Box::from_raw`).
    let dispatch_qualifiers: Vec<&'static str> = dispatch_indices
        .iter()
        .map(|&i| match params[i].mode {
            ParamMode::DispatchAny { consume_dispatch: true } => "consume",
            _ => "withPtr",
        })
        .collect();

    // Build the DispatchAny decision tree. At each level we pick one
    // dispatch_indices[level]: `if (p is NativeHandle) p.<qual> { ... } else { ... }`,
    // where <qual> = `withPtr` for Borrow-mode dispatchers and
    // `consume` for Consume-mode dispatchers.
    fn build_tree(
        level: usize,
        mask: u32,
        dispatch_indices: &[usize],
        qualifiers: &[&str],
        params: &[(String, ())],
        build_call: &dyn Fn(u32) -> String,
    ) -> String {
        if level == dispatch_indices.len() {
            return build_call(mask);
        }
        let name = &params[dispatch_indices[level]].0;
        let qual = qualifiers[level];
        let with_branch = build_tree(level + 1, mask | (1 << level), dispatch_indices, qualifiers, params, build_call);
        let else_branch = build_tree(level + 1, mask, dispatch_indices, qualifiers, params, build_call);
        format!(
            "if ({name} is NativeHandle) {name}.{qual} {{ {name}_ptr ->\n    {with_branch}\n}} else {{\n    {else_branch}\n}}"
        )
    }

    let param_names_for_tree: Vec<(String, ())> = params.iter().map(|p| (p.kt_name.clone(), ())).collect();
    let mut body_expr = build_tree(0, 0, &dispatch_indices, &dispatch_qualifiers, &param_names_for_tree, &build_call);

    // Wrap with nested withPtr/consume from innermost to outermost.
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
            ParamMode::DispatchAny { .. } | ParamMode::PassThrough => {}
        }
    }

    let mut out = String::new();
    let _ = writeln!(out, "@Throws(ZError::class)");
    let param_list: Vec<String> = params.iter().map(|p| format!("{}: {}", p.kt_name, p.kt_type)).collect();
    let _ = write!(out, "public fun {kt_name}({})", param_list.join(", "));
    if !kt_return.is_empty() {
        let _ = write!(out, ": {kt_return}");
    }
    let _ = writeln!(out, " =");
    let _ = writeln!(out, "    {body_expr}");
    Some(out)
}

/// Detect whether an `impl Into<T>` dispatcher's body reconstructs a
/// `Box<T>` from the inbound jlong (i.e. at least one source arm is
/// declared `IntoSourceMode::Consume`). The Kotlin caller for a
/// Consume-mode parameter must surrender the slot via
/// `NativeHandle.consume`; for a Borrow-only dispatcher it stays on
/// `NativeHandle.withPtr`.
///
/// Inspection is by token-stream search for `Box :: from_raw` inside
/// the generated `ItemFn`'s body. The dispatcher emitter at
/// `jni_ext.rs:emit_into_dispatcher` is the only producer of that
/// pattern in this code path, so the marker is unambiguous.
///
/// Note (Step 1 limit): the check is target-uniform. When the
/// dispatcher mixes Borrow and Consume opaque arms, every runtime
/// `is NativeHandle` match is treated as Consume — fine for the
/// single-opaque-source case (today's only usage), but a Step 2
/// follow-up is needed for per-arm differentiation via typed
/// `JNI<T>` Java classes.
fn dispatcher_consumes_opaque(item_fn: &syn::ItemFn) -> bool {
    let tokens = item_fn.block.to_token_stream().to_string();
    tokens.contains("Box :: from_raw") || tokens.contains("Box::from_raw")
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
/// Used when the type-map doesn't have an entry for a Rust type —
/// covers `impl Into<...>` (JObject-wired) and rarely-used primitives.
fn kotlin_for_wire(wire: &syn::Type) -> Option<String> {
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
                "JString" | "jstring" => "String?",
                "JByteArray" | "jbyteArray" => "ByteArray?",
                "JObject" | "jobject" => "Any",
                "JClass" => "Any",
                _ => return None,
            };
            return Some(kt.to_string());
        }
    }
    None
}

fn classify_return(
    output: &syn::ReturnType,
    registry: &Registry,
    kotlin_types: &KotlinTypeMap,
    imports: &mut BTreeSet<String>,
) -> Option<(String, bool)> {
    let ty = match output {
        syn::ReturnType::Default => return Some((String::new(), false)),
        syn::ReturnType::Type(_, t) => &**t,
    };
    // Detect opaque return: ZResult<T> or T where T's input converter
    // returns `OwnedObject<T>` (i.e. opaque-handle type). Peel ZResult
    // first because that's the common signature shape.
    let inner = peel_zresult(ty).unwrap_or(ty);
    if crate::util::is_unit(inner) {
        return Some((String::new(), false));
    }
    let inner_canon = inner.to_token_stream().to_string();
    // An output is "opaque-handle" iff its registered output converter
    // produces `jlong` (the `Box::into_raw(...) as i64` shape from
    // `opaque_handle_output`). Pull the wire type from the inner type's
    // output entry; the input-side `OwnedObject<T>` check below
    // catches anything we register only on input (rare).
    let output_is_opaque_jlong = registry
        .output_entry(inner)
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
        return Some(("NativeHandle".to_string(), true));
    }
    // Non-opaque: try the full return key first (covers `ZResult<T>`
    // entries in the map), fall back to the peeled inner key, then
    // wire-type fallback via the output entry.
    let full_canon = ty.to_token_stream().to_string();
    if let Some(kt) = kotlin_types.lookup(&full_canon) {
        return Some((register_fqn(kt, imports), false));
    }
    if let Some(kt) = kotlin_types.lookup(&inner_canon) {
        return Some((register_fqn(kt, imports), false));
    }
    if let Some(out_entry) = registry.output_entry(ty) {
        if let Some(kt) = kotlin_for_wire(&out_entry.destination) {
            return Some((register_fqn(&kt, imports), false));
        }
    }
    None
}

fn peel_zresult(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(tp) = ty else { return None };
    let last = tp.path.segments.last()?;
    if last.ident != "ZResult" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else { return None };
    let syn::GenericArgument::Type(inner) = args.args.first()? else { return None };
    Some(inner)
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
