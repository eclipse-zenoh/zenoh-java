//! Generator for the destination Kotlin interface file (data classes +
//! `internal object` with `external fun` prototypes).

use std::collections::BTreeSet;
use std::path::PathBuf;

use quote::ToTokens;

use prebindgen::SourceLocation;

use crate::core::type_registry::TypeRegistry;
use crate::kotlin::type_map::KotlinTypeMap;
use crate::util::snake_to_camel;

/// Builder for [`KotlinInterfaceGenerator`].
pub struct KotlinInterfaceBuilder {
    output_path: PathBuf,
    package: String,
    class_name: String,
    throws_class_fqn: Option<String>,
    init_load_fqn: Option<String>,
    function_suffix: String,
    types: TypeRegistry,
    kotlin_types: KotlinTypeMap,
}

impl Default for KotlinInterfaceBuilder {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            package: String::new(),
            class_name: String::new(),
            throws_class_fqn: None,
            init_load_fqn: None,
            function_suffix: String::new(),
            types: TypeRegistry::new(),
            kotlin_types: KotlinTypeMap::new(),
        }
    }
}

impl KotlinInterfaceBuilder {
    pub fn output_path(mut self, p: impl Into<PathBuf>) -> Self {
        self.output_path = p.into();
        self
    }
    pub fn package(mut self, p: impl Into<String>) -> Self {
        self.package = p.into();
        self
    }
    pub fn class_name(mut self, n: impl Into<String>) -> Self {
        self.class_name = n.into();
        self
    }
    /// FQN of the exception type to annotate every `external fun` with via
    /// `@Throws(<last>::class)`.
    pub fn throws_class(mut self, fqn: impl Into<String>) -> Self {
        self.throws_class_fqn = Some(fqn.into());
        self
    }
    /// FQN of a singleton referenced from the generated `init { ... }` block.
    pub fn init_load(mut self, fqn: impl Into<String>) -> Self {
        self.init_load_fqn = Some(fqn.into());
        self
    }
    /// Suffix appended to the camelCase Kotlin function name. Should
    /// mirror the Rust-side `NameMangler`'s suffix (e.g. `"ViaJNI"`).
    pub fn function_suffix(mut self, s: impl Into<String>) -> Self {
        self.function_suffix = s.into();
        self
    }
    pub fn type_registry(mut self, t: TypeRegistry) -> Self {
        self.types = self.types.merge(t);
        self
    }
    pub fn kotlin_types(mut self, k: KotlinTypeMap) -> Self {
        self.kotlin_types.map.extend(k.map);
        self
    }

    pub fn build(self) -> KotlinInterfaceGenerator {
        KotlinInterfaceGenerator {
            cfg: self,
            data_classes: Vec::new(),
            external_funs: Vec::new(),
            used_fqns: BTreeSet::new(),
        }
    }
}

pub struct KotlinInterfaceGenerator {
    cfg: KotlinInterfaceBuilder,
    data_classes: Vec<String>,
    external_funs: Vec<String>,
    used_fqns: BTreeSet<String>,
}

impl KotlinInterfaceGenerator {
    pub fn builder() -> KotlinInterfaceBuilder {
        KotlinInterfaceBuilder::default()
    }

    /// Process one item: structs become `data class`es, fns become
    /// `external fun`s. Other variants are ignored (a typical pipeline
    /// will pre-filter the iterator anyway).
    pub fn add_item(&mut self, item: &syn::Item, loc: &SourceLocation) {
        match item {
            syn::Item::Struct(s) => self.process_struct(s, loc),
            syn::Item::Fn(f) => self.process_fn(f, loc),
            _ => {}
        }
    }

    fn process_struct(&mut self, s: &syn::ItemStruct, loc: &SourceLocation) {
        let struct_name = s.ident.to_string();
        let syn::Fields::Named(named) = &s.fields else {
            panic!("KotlinInterfaceGenerator: tuple/unit struct unsupported at {loc}");
        };

        let mut field_lines: Vec<String> = Vec::new();
        for field in &named.named {
            let fname = field
                .ident
                .as_ref()
                .unwrap_or_else(|| panic!("unnamed field in struct `{struct_name}` at {loc}"))
                .to_string();
            let camel = snake_to_camel(&fname);
            let kotlin_ty = self.lookup_field_kotlin_type(&field.ty, &struct_name, &fname, loc);
            // FQN-import bug fix: register FQN-shaped kotlin types so the
            // data class uses the short name and the file gets the import.
            let short = register_fqn(&kotlin_ty, &mut self.used_fqns);
            let nullable = if is_option_type(&field.ty) { "?" } else { "" };
            field_lines.push(format!("    val {}: {}{},", camel, short, nullable));
        }

        let block = format!(
            "data class {}(\n{}\n) {{\n    companion object\n}}",
            struct_name,
            field_lines.join("\n")
        );
        self.data_classes.push(block);
    }

    fn process_fn(&mut self, f: &syn::ItemFn, loc: &SourceLocation) {
        let original_name = f.sig.ident.to_string();
        let camel = snake_to_camel(&original_name);
        let kt_fn_name = format!("{}{}", camel, self.cfg.function_suffix);

        let mut local_used: BTreeSet<String> = BTreeSet::new();
        let mut params: Vec<String> = Vec::new();

        for input in &f.sig.inputs {
            let syn::FnArg::Typed(pat_type) = input else {
                panic!("receiver args not supported at {loc}");
            };
            let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
                panic!("non-ident param pattern at {loc}");
            };
            let name = pat_ident.ident.to_string();
            let ty = &*pat_type.ty;

            let key = ty.to_token_stream().to_string();
            let binding = self.cfg.types.types.get(&key).unwrap_or_else(|| {
                panic!(
                    "Kotlin: unsupported parameter type `{}` for `{}` at {loc}",
                    ty.to_token_stream(),
                    name
                )
            });
            let kotlin_ty = self.cfg.kotlin_types.lookup(&key).unwrap_or_else(|| {
                panic!(
                    "Kotlin: no kotlin_type registered for `{}` (param `{}` of `{}`) at {loc}",
                    key, name, original_name
                )
            });
            let short = register_fqn(kotlin_ty, &mut local_used);
            let suffix = if is_option_type(&binding.rust_type) {
                "?"
            } else {
                ""
            };
            params.push(format!(
                "{}: {}{}",
                kotlin_param_name(&name, binding.is_pointer()),
                short,
                suffix
            ));
        }

        let ret_suffix: String = match &f.sig.output {
            syn::ReturnType::Default => String::new(),
            syn::ReturnType::Type(_, ty) => {
                if crate::util::is_unit(ty) {
                    String::new()
                } else {
                    let key = ty.to_token_stream().to_string();
                    // Treat returns whose wire form is `()` as unit too —
                    // matches the universal `FunctionsConverter`'s unit
                    // detection. The wire registry is the source of truth.
                    let wire_is_unit = self
                        .cfg
                        .types
                        .types
                        .get(&key)
                        .map(|b| crate::util::is_unit(b.wire_type_ref()))
                        .unwrap_or(false);
                    if wire_is_unit {
                        String::new()
                    } else {
                        let kotlin_ty = self.cfg.kotlin_types.lookup(&key).unwrap_or_else(|| {
                            panic!(
                                "Kotlin: no kotlin_type registered for return `{}` of `{}` at {loc}",
                                key, original_name
                            )
                        });
                        let short = register_fqn(kotlin_ty, &mut local_used);
                        format!(": {}", short)
                    }
                }
            }
        };

        let params_joined = if params.is_empty() {
            String::new()
        } else {
            format!("\n        {},\n    ", params.join(",\n        "))
        };
        let throws_line = match self.cfg.throws_class_fqn.as_ref() {
            Some(fqn) => {
                let short = fqn.rsplit('.').next().unwrap_or(fqn);
                format!("@Throws({}::class)\n    ", short)
            }
            None => String::new(),
        };
        let block = format!(
            "    @JvmStatic\n    {}external fun {}({}){}",
            throws_line, kt_fn_name, params_joined, ret_suffix
        );
        self.external_funs.push(block);
        self.used_fqns.extend(local_used);
        if let Some(fqn) = self.cfg.throws_class_fqn.as_ref() {
            if fqn.contains('.') {
                self.used_fqns.insert(fqn.clone());
            }
        }
    }

    fn lookup_field_kotlin_type(
        &self,
        ty: &syn::Type,
        struct_name: &str,
        field_name: &str,
        loc: &SourceLocation,
    ) -> String {
        let syn::Type::Path(tp) = ty else {
            panic!(
                "Kotlin: unsupported field type `{}` for `{}.{}` at {loc}",
                ty.to_token_stream(),
                struct_name,
                field_name
            );
        };
        let last = tp.path.segments.last().unwrap_or_else(|| {
            panic!(
                "Kotlin: empty path for `{}.{}` at {loc}",
                struct_name, field_name
            )
        });
        let bare = last.ident.to_string();
        if let Some(s) = self.cfg.kotlin_types.lookup(&bare) {
            return s.to_string();
        }
        // Fall back to canonical key lookup for types like `Vec<u8>`.
        let key = ty.to_token_stream().to_string();
        if let Some(s) = self.cfg.kotlin_types.lookup(&key) {
            return s.to_string();
        }
        panic!(
            "Kotlin: no kotlin_type registered for `{}` (field `{}.{}`) at {loc}",
            bare, struct_name, field_name
        );
    }

    /// Render and write the assembled `.kt` file. No-op if `output_path`
    /// is empty.
    pub fn write(&self) -> std::io::Result<()> {
        if self.cfg.output_path.as_os_str().is_empty() {
            return Ok(());
        }
        let contents = self.render();
        if let Some(parent) = self.cfg.output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.cfg.output_path, contents)
    }

    fn render(&self) -> String {
        let mut used = self.used_fqns.clone();
        if let Some(fqn) = self.cfg.init_load_fqn.as_ref() {
            if fqn.contains('.') {
                used.insert(fqn.clone());
            }
        }

        let mut imports: Vec<String> = used
            .into_iter()
            .filter(|fqn| {
                let pkg = fqn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
                !pkg.is_empty() && pkg != self.cfg.package
            })
            .collect();
        imports.sort();
        imports.dedup();

        let mut out = String::new();
        out.push_str("// Auto-generated by JniConverter — do not edit by hand.\n");
        if !self.cfg.package.is_empty() {
            out.push_str(&format!("package {}\n\n", self.cfg.package));
        }
        for imp in &imports {
            out.push_str(&format!("import {}\n", imp));
        }
        if !imports.is_empty() {
            out.push('\n');
        }
        for block in &self.data_classes {
            out.push_str(block);
            out.push_str("\n\n");
        }
        out.push_str(&format!("internal object {} {{\n", self.cfg.class_name));
        if let Some(fqn) = self.cfg.init_load_fqn.as_ref() {
            let short = fqn.rsplit('.').next().unwrap_or(fqn);
            out.push_str(&format!("    init {{ {} }}\n\n", short));
        }
        for (i, block) in self.external_funs.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(block);
            out.push('\n');
        }
        out.push_str("}\n");
        out
    }
}

/// Map a Rust snake_case arg name to its Kotlin camelCase form, appending
/// `"Ptr"` for raw-pointer slots.
fn kotlin_param_name(rust_name: &str, is_pointer: bool) -> String {
    let base = snake_to_camel(rust_name);
    if is_pointer {
        format!("{}Ptr", base)
    } else {
        base
    }
}

/// Record `fqn` in `used` if it looks fully-qualified (contains `.`) and
/// return the short name used at the emission site.
fn register_fqn(fqn: &str, used: &mut BTreeSet<String>) -> String {
    if fqn.contains('.') {
        used.insert(fqn.to_string());
        fqn.rsplit('.').next().unwrap_or(fqn).to_string()
    } else {
        fqn.to_string()
    }
}

/// Check if a Rust type is `Option<_>`.
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            return last.ident == "Option";
        }
    }
    false
}
