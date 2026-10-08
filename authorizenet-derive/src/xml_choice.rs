//! `#[derive(AnetXml)]` on enums: an `xs:choice` group where each variant is one element.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DataEnum, DeriveInput, Fields, Result};

use crate::attrs::{ContainerAttrs, VariantAttrs};
use crate::naming::variant_xml_name;

pub fn expand(input: &DeriveInput, data: &DataEnum) -> Result<TokenStream> {
    let container = ContainerAttrs::parse(&input.attrs)?;
    if let Some(root) = container.root {
        return Err(syn::Error::new_spanned(
            root,
            "`root` is only supported on structs",
        ));
    }
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "a choice enum needs at least one variant",
        ));
    }

    let ident = &input.ident;
    let type_name = ident.to_string();
    let x = quote!(::authorizenet::xml);

    let mut names = Vec::new();
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut debugs = Vec::new();
    for variant in &data.variants {
        let attrs = VariantAttrs::parse(&variant.attrs)?;
        if attrs.value.is_some() || attrs.other {
            return Err(syn::Error::new_spanned(
                variant,
                "`value` and `other` belong on `#[derive(AnetEnum)]` enums; choice variants use `rename`",
            ));
        }
        let ty = match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0].ty,
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "choice variants must have exactly one unnamed field, e.g. `CreditCard(CreditCard)`",
                ));
            }
        };
        let var = &variant.ident;
        let var_name = var.to_string();
        let name = attrs.rename.unwrap_or_else(|| variant_xml_name(var));
        reads.push(quote! {
            #name => ::core::result::Result::Ok(
                Self::#var(<#ty as #x::XmlRead>::read_element(r, start)?),
            ),
        });
        writes.push(quote! {
            Self::#var(value) => #x::XmlWrite::write_element(value, w, #name),
        });
        let shown = if attrs.sensitive {
            quote!(&::authorizenet::xml::Redacted)
        } else {
            quote!(value)
        };
        debugs.push(quote! {
            Self::#var(value) => f.debug_tuple(#var_name).field(#shown).finish(),
        });
        names.push(name);
    }

    Ok(quote! {
        impl #x::XmlChoice for #ident {
            fn matches(name: &str) -> bool {
                matches!(name, #(#names)|*)
            }

            fn read_choice<'a>(
                r: &mut #x::XmlReader<'a>,
                start: &#x::BytesStart<'a>,
                name: &str,
            ) -> ::core::result::Result<Self, #x::XmlError> {
                match name {
                    #(#reads)*
                    other => ::core::result::Result::Err(#x::XmlError::UnknownElement {
                        ty: #type_name,
                        name: other.to_owned(),
                    }),
                }
            }

            fn write_choice<W: ::std::io::Write>(
                &self,
                w: &mut #x::XmlWriter<W>,
            ) -> ::core::result::Result<(), #x::XmlError> {
                match self {
                    #(#writes)*
                }
            }
        }

        impl ::core::fmt::Debug for #ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    #(#debugs)*
                }
            }
        }
    })
}
