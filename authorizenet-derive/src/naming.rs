//! Default XML names derived from Rust identifiers.

use syn::Ident;

/// `ref_trans_id` -> `refTransId`, `r#type` -> `type`.
pub fn field_xml_name(ident: &Ident) -> String {
    let raw = ident.to_string();
    let raw = raw.strip_prefix("r#").unwrap_or(&raw);
    let mut out = String::with_capacity(raw.len());
    for (i, part) in raw.split('_').filter(|p| !p.is_empty()).enumerate() {
        if i == 0 {
            out.push_str(part);
        } else {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    out
}

/// `CreditCard` -> `creditCard`.
pub fn variant_xml_name(ident: &Ident) -> String {
    let raw = ident.to_string();
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => raw,
    }
}

/// Rust-facing name of a field for `Debug` output and error messages.
pub fn field_display_name(ident: &Ident) -> String {
    let raw = ident.to_string();
    raw.strip_prefix("r#").unwrap_or(&raw).to_owned()
}
