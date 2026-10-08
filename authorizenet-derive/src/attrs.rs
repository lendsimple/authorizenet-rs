//! Parsing of `#[anet(...)]` attributes.

use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::{Attribute, LitStr, Result};

/// Attributes on the struct or enum itself.
#[derive(Default)]
pub struct ContainerAttrs {
    /// Root element name, for types that can be a whole document.
    pub root: Option<LitStr>,
    /// The root is an `ANetApiRequest`: `merchantAuthentication` is injected when writing.
    pub request: Option<Span>,
}

impl ContainerAttrs {
    pub fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut out = Self::default();
        for attr in anet_attrs(attrs) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("root") {
                    out.root = Some(meta.value()?.parse()?);
                } else if meta.path.is_ident("request") {
                    out.request = Some(meta.path.segments[0].ident.span());
                } else {
                    return Err(unknown(&meta));
                }
                Ok(())
            })?;
        }
        if let (Some(span), None) = (out.request, &out.root) {
            return Err(syn::Error::new(span, "`request` requires `root = \"...\"`"));
        }
        Ok(out)
    }
}

/// Attributes on a struct field.
#[derive(Default)]
pub struct FieldAttrs {
    pub rename: Option<String>,
    pub flatten: Option<Span>,
    pub choice: Option<Span>,
    pub attribute: Option<Span>,
    pub wrapper: Option<Span>,
    pub item: Option<String>,
    pub item_span: Option<Span>,
    pub sensitive: bool,
}

impl FieldAttrs {
    pub fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut out = Self::default();
        for attr in anet_attrs(attrs) {
            attr.parse_nested_meta(|meta| {
                let span = meta.path.segments[0].ident.span();
                if meta.path.is_ident("rename") {
                    out.rename = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("flatten") {
                    out.flatten = Some(span);
                } else if meta.path.is_ident("choice") {
                    out.choice = Some(span);
                } else if meta.path.is_ident("attribute") {
                    out.attribute = Some(span);
                } else if meta.path.is_ident("wrapper") {
                    out.wrapper = Some(span);
                } else if meta.path.is_ident("item") {
                    out.item_span = Some(span);
                    out.item = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("sensitive") {
                    out.sensitive = true;
                } else {
                    return Err(unknown(&meta));
                }
                Ok(())
            })?;
        }
        out.check_conflicts()?;
        Ok(out)
    }

    fn check_conflicts(&self) -> Result<()> {
        let kinds = [
            ("flatten", self.flatten),
            ("choice", self.choice),
            ("attribute", self.attribute),
            ("wrapper", self.wrapper),
        ];
        let set: Vec<_> = kinds
            .iter()
            .filter_map(|(n, s)| s.map(|s| (*n, s)))
            .collect();
        if let [(first, _), (second, span), ..] = set.as_slice() {
            return Err(syn::Error::new(
                *span,
                format!("`{first}` and `{second}` cannot be combined"),
            ));
        }
        if let Some(span) = self.flatten.or(self.choice)
            && self.rename.is_some()
        {
            return Err(syn::Error::new(
                span,
                "`rename` has no effect on `flatten` or `choice` fields",
            ));
        }
        match (self.wrapper, self.item_span) {
            (Some(span), None) => Err(syn::Error::new(
                span,
                "`wrapper` requires `item = \"...\"` naming the repeated child element",
            )),
            (None, Some(span)) => Err(syn::Error::new(
                span,
                "`item` is only valid together with `wrapper`",
            )),
            _ => Ok(()),
        }
    }
}

/// Attributes on an enum variant (choice variants and scalar enum values).
#[derive(Default)]
pub struct VariantAttrs {
    pub rename: Option<String>,
    pub value: Option<String>,
    pub other: bool,
    pub sensitive: bool,
}

impl VariantAttrs {
    pub fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut out = Self::default();
        for attr in anet_attrs(attrs) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    out.rename = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("value") {
                    out.value = Some(meta.value()?.parse::<LitStr>()?.value());
                } else if meta.path.is_ident("other") {
                    out.other = true;
                } else if meta.path.is_ident("sensitive") {
                    out.sensitive = true;
                } else {
                    return Err(unknown(&meta));
                }
                Ok(())
            })?;
        }
        Ok(out)
    }
}

fn anet_attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|a| a.path().is_ident("anet"))
}

fn unknown(meta: &ParseNestedMeta) -> syn::Error {
    let name = meta
        .path
        .get_ident()
        .map(ToString::to_string)
        .unwrap_or_else(|| "?".into());
    meta.error(format!("unknown anet attribute `{name}`"))
}
