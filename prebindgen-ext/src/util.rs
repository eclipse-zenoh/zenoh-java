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

#[cfg(test)]
mod tests {
    use super::camel_to_screaming_snake;

    #[test]
    fn camel_to_screaming_snake_basics() {
        assert_eq!(camel_to_screaming_snake("RealTime"), "REAL_TIME");
        assert_eq!(camel_to_screaming_snake("InteractiveHigh"), "INTERACTIVE_HIGH");
        assert_eq!(camel_to_screaming_snake("Data"), "DATA");
        assert_eq!(camel_to_screaming_snake("Background"), "BACKGROUND");
    }
}
