//! Syntactic detection of `Option<T>`, `Vec<T>` and `Option<Vec<T>>` field types.

use syn::{GenericArgument, PathArguments, Type};

/// How many times an element may occur, decided from the field's type.
pub enum Cardinality<'a> {
    /// `T`: exactly once.
    Required(&'a Type),
    /// `Option<T>`: at most once.
    Optional(&'a Type),
    /// `Vec<T>`: any number of times.
    List(&'a Type),
    /// `Option<Vec<T>>`: only meaningful with a wrapper element.
    OptionalList(&'a Type),
}

impl<'a> Cardinality<'a> {
    pub fn of(ty: &'a Type) -> Self {
        if let Some(inner) = generic_arg(ty, "Option") {
            if let Some(item) = generic_arg(inner, "Vec") {
                return Self::OptionalList(item);
            }
            return Self::Optional(inner);
        }
        if let Some(item) = generic_arg(ty, "Vec") {
            return Self::List(item);
        }
        Self::Required(ty)
    }
}

/// Returns `T` when `ty` is `<wrapper><T>` (matching the last path segment only).
fn generic_arg<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else { return None };
    if path.qself.is_some() {
        return None;
    }
    let segment = path.path.segments.last()?;
    if segment.ident != wrapper {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}
