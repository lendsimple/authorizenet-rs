//! XSD facet attributes (`max_length = 20`, `pattern = "[0-9]+"`, ...) and the
//! validation code generated from them.

use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::{LitInt, LitStr, Result};

#[derive(Default)]
pub struct Facets {
    pub length: Option<usize>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<LitStr>,
    pub min: Option<LitStr>,
    pub max: Option<LitStr>,
    pub total_digits: Option<u32>,
    pub fraction_digits: Option<u32>,
    pub min_occurs: Option<usize>,
    pub max_occurs: Option<usize>,
    /// Where the first facet was written, for error messages.
    pub span: Option<Span>,
}

impl Facets {
    /// Parses one facet if `meta` names one; returns `false` otherwise.
    pub fn parse(&mut self, meta: &ParseNestedMeta) -> Result<bool> {
        let Some(ident) = meta.path.get_ident() else {
            return Ok(false);
        };
        let int = || -> Result<usize> { meta.value()?.parse::<LitInt>()?.base10_parse() };
        let digits = || -> Result<u32> { meta.value()?.parse::<LitInt>()?.base10_parse() };
        let text = || -> Result<LitStr> { meta.value()?.parse() };
        match ident.to_string().as_str() {
            "length" => self.length = Some(int()?),
            "min_length" => self.min_length = Some(int()?),
            "max_length" => self.max_length = Some(int()?),
            "pattern" => {
                let lit = text()?;
                char_class(&lit)?;
                self.pattern = Some(lit);
            }
            "min" => self.min = Some(text()?),
            "max" => self.max = Some(text()?),
            "total_digits" => self.total_digits = Some(digits()?),
            "fraction_digits" => self.fraction_digits = Some(digits()?),
            "min_occurs" => self.min_occurs = Some(int()?),
            "max_occurs" => self.max_occurs = Some(int()?),
            _ => return Ok(false),
        }
        self.span.get_or_insert(ident.span());
        Ok(true)
    }

    pub fn has_occurs(&self) -> bool {
        self.min_occurs.is_some() || self.max_occurs.is_some()
    }

    /// Checks of a single value bound to `value` (a reference), with `path` and `out`
    /// in scope.
    pub fn value_checks(&self) -> TokenStream {
        let v = quote!(::authorizenet::validate);
        let mut checks = Vec::new();
        if self.length.is_some() || self.min_length.is_some() || self.max_length.is_some() {
            let (min, max, exact) = (opt(self.min_length), opt(self.max_length), opt(self.length));
            checks.push(quote! {
                #v::check_length(::core::convert::AsRef::<str>::as_ref(value), #min, #max, #exact, path, out);
            });
        }
        if let Some(pattern) = &self.pattern {
            let class = char_class(pattern).expect("checked when parsed");
            checks.push(quote! {
                #v::check_pattern(
                    ::core::convert::AsRef::<str>::as_ref(value),
                    #pattern,
                    |c| matches!(c, #class),
                    path,
                    out,
                );
            });
        }
        if self.min.is_some() || self.max.is_some() {
            let min = opt_str(&self.min);
            let max = opt_str(&self.max);
            checks.push(quote!(#v::check_range(value, #min, #max, path, out);));
        }
        if self.total_digits.is_some() || self.fraction_digits.is_some() {
            let total = opt_u32(self.total_digits);
            let fraction = opt_u32(self.fraction_digits);
            checks.push(quote!(#v::check_digits(value, #total, #fraction, path, out);));
        }
        quote!(#(#checks)*)
    }

    /// The occurrence check of a list bound to `values`.
    pub fn occurs_check(&self) -> TokenStream {
        if !self.has_occurs() {
            return quote!();
        }
        let (min, max) = (opt(self.min_occurs), opt(self.max_occurs));
        quote!(::authorizenet::validate::check_occurs(values.len(), #min, #max, path, out);)
    }
}

fn opt(value: Option<usize>) -> TokenStream {
    match value {
        Some(n) => {
            let n = Literal::usize_unsuffixed(n);
            quote!(::core::option::Option::Some(#n))
        }
        None => quote!(::core::option::Option::None),
    }
}

fn opt_u32(value: Option<u32>) -> TokenStream {
    match value {
        Some(n) => {
            let n = Literal::u32_unsuffixed(n);
            quote!(::core::option::Option::Some(#n))
        }
        None => quote!(::core::option::Option::None),
    }
}

fn opt_str(value: &Option<LitStr>) -> TokenStream {
    match value {
        Some(s) => quote!(::core::option::Option::Some(#s)),
        None => quote!(::core::option::Option::None),
    }
}

/// Compiles a `[class]+` pattern to the `|`-separated arms of a `matches!` on a char.
///
/// The class may hold characters, ranges (`a-z`) and the escapes `\s` (XML whitespace),
/// `\d` and `\\`. Other patterns are rejected: support them here if the schema adds any.
fn char_class(lit: &LitStr) -> Result<TokenStream> {
    let pattern = lit.value();
    let unsupported = || {
        syn::Error::new(
            lit.span(),
            "only patterns of the form `[class]+` are supported, e.g. \"[0-9a-zA-Z]+\"",
        )
    };
    let body = pattern
        .strip_prefix('[')
        .and_then(|p| p.strip_suffix("]+"))
        .filter(|b| !b.is_empty() && !b.starts_with('^'))
        .ok_or_else(unsupported)?;

    let mut arms = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let escaped = *chars.get(i + 1).ok_or_else(unsupported)?;
            match escaped {
                's' => arms.extend([' ', '\t', '\n', '\r'].map(|c| quote!(#c))),
                'd' => arms.push(quote!('0'..='9')),
                '\\' | '-' | ']' | '[' => arms.push(quote!(#escaped)),
                _ => return Err(unsupported()),
            }
            i += 2;
        } else if chars.get(i + 1) == Some(&'-') && chars.get(i + 2).is_some_and(|&e| e != '\\') {
            let end = chars[i + 2];
            arms.push(quote!(#c..=#end));
            i += 3;
        } else {
            arms.push(quote!(#c));
            i += 1;
        }
    }
    Ok(quote!(#(#arms)|*))
}
