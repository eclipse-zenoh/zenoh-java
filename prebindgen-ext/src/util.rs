//! Shared internal utilities used by multiple modules.

/// Convert a `snake_case` Rust identifier name to `camelCase`.
pub(crate) fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else if i == 0 {
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Convert a `CamelCase` Rust identifier to `SCREAMING_SNAKE_CASE`. Used to
/// project Rust enum variant idents into Kotlin enum constant names.
pub(crate) fn camel_to_screaming_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_uppercase());
    }
    out
}

/// True iff `ty` is the unit type `()`.
pub(crate) fn is_unit(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Tuple(t) if t.elems.is_empty())
}

/// Pull a signed integer out of a `syn::Expr` literal (`5`, `-3`,
/// `0x07`). Returns `None` for anything else (constants, paths,
/// arithmetic).
pub(crate) fn extract_int_literal(expr: &syn::Expr) -> Option<i64> {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Int(int) => int.base10_parse::<i64>().ok(),
            _ => None,
        },
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => extract_int_literal(expr).map(|v| -v),
        _ => None,
    }
}

/// Resolve each enum variant to its discriminant value following Rust's
/// own assignment rule: an explicit `= N` sets the value, an implicit
/// variant takes the previous value plus one (starting at 0). This is
/// the single source of truth for both the Kotlin `value(N)` constants
/// and the generated Rust `jint → variant` decode — keeping the two
/// from drifting and removing the need for a hand-written
/// `TryFrom<i32>` on the flat enum. Non-literal discriminants are
/// rejected because prebindgen-ext cannot reliably evaluate arbitrary
/// expressions at codegen time.
pub(crate) fn enum_discriminant_values(e: &syn::ItemEnum) -> Vec<(syn::Ident, i64)> {
    let mut out = Vec::with_capacity(e.variants.len());
    let mut next: i64 = 0;
    for variant in &e.variants {
        let value = match variant.discriminant.as_ref() {
            Some((_, expr)) => extract_int_literal(expr).unwrap_or_else(|| {
                panic!(
                    "enum `{}` variant `{}` has a non-literal discriminant; use a literal integer value (e.g. `= 1`) or an implicit discriminant",
                    e.ident,
                    variant.ident
                )
            }),
            None => next,
        };
        out.push((variant.ident.clone(), value));
        next = value + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{camel_to_screaming_snake, enum_discriminant_values};

    #[test]
    fn camel_to_screaming_snake_basics() {
        assert_eq!(camel_to_screaming_snake("RealTime"), "REAL_TIME");
        assert_eq!(camel_to_screaming_snake("InteractiveHigh"), "INTERACTIVE_HIGH");
        assert_eq!(camel_to_screaming_snake("Data"), "DATA");
        assert_eq!(camel_to_screaming_snake("Background"), "BACKGROUND");
    }

    fn discriminants(e: syn::ItemEnum) -> Vec<(String, i64)> {
        enum_discriminant_values(&e)
            .into_iter()
            .map(|(ident, value)| (ident.to_string(), value))
            .collect()
    }

    #[test]
    fn discriminants_no_explicit_values() {
        // Implicit C-like enum: 0, 1, 2 — matches Rust's default repr,
        // which is also what the `as jint` output cast produces.
        let e: syn::ItemEnum = syn::parse_quote! { enum E { A, B, C } };
        assert_eq!(
            discriminants(e),
            vec![("A".into(), 0), ("B".into(), 1), ("C".into(), 2)]
        );
    }

    #[test]
    fn discriminants_all_explicit() {
        let e: syn::ItemEnum = syn::parse_quote! {
            enum E { A = 1, B = 2, C = 7 }
        };
        assert_eq!(
            discriminants(e),
            vec![("A".into(), 1), ("B".into(), 2), ("C".into(), 7)]
        );
    }

    #[test]
    fn discriminants_mixed_follow_rust_rule() {
        // Explicit sets the value; the next implicit variant is prev + 1.
        let e: syn::ItemEnum = syn::parse_quote! {
            enum E { A = 5, B, C = 1, D }
        };
        assert_eq!(
            discriminants(e),
            vec![
                ("A".into(), 5),
                ("B".into(), 6),
                ("C".into(), 1),
                ("D".into(), 2),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "non-literal discriminant")]
    fn discriminants_non_literal_rejected() {
        let e: syn::ItemEnum = syn::parse_quote! {
            enum E { A = OTHER, B }
        };
        let _ = discriminants(e);
    }
}
