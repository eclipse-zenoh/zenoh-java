//! Strategy enum for transforming the original `#[prebindgen]` function
//! identifier into the wrapper's exported identifier.

use std::sync::Arc;

use quote::format_ident;

use crate::util::snake_to_camel;

/// How to derive the wrapper function name from the original ident.
#[derive(Clone)]
pub enum NameMangler {
    /// No transformation — emit `<orig_ident>` verbatim. Used by
    /// pass-through C-style FFI.
    Identity,
    /// `<prefix><snake_to_camel(orig)><suffix>` — used by JNI.
    CamelPrefixSuffix {
        prefix: String,
        suffix: String,
    },
    /// Caller-supplied closure for arbitrary mangling.
    Custom(Arc<dyn Fn(&syn::Ident) -> syn::Ident + Send + Sync>),
}

impl NameMangler {
    /// Apply this mangler to `orig` and return the wrapper ident.
    pub fn apply(&self, orig: &syn::Ident) -> syn::Ident {
        match self {
            NameMangler::Identity => orig.clone(),
            NameMangler::CamelPrefixSuffix { prefix, suffix } => {
                let camel = snake_to_camel(&orig.to_string());
                format_ident!("{}{}{}", prefix, camel, suffix)
            }
            NameMangler::Custom(f) => f(orig),
        }
    }

    pub fn custom<F>(f: F) -> Self
    where
        F: Fn(&syn::Ident) -> syn::Ident + Send + Sync + 'static,
    {
        NameMangler::Custom(Arc::new(f))
    }
}

impl Default for NameMangler {
    fn default() -> Self {
        NameMangler::Identity
    }
}
