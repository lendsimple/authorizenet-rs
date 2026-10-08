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
    let mut validates = Vec::new();
    for variant in &data.variants {
        let attrs = VariantAttrs::parse(&variant.attrs)?;
        if attrs.facets.has_occurs() {
            return Err(syn::Error::new(
                attrs.facets.span.expect("set with facets"),
                "`min_occurs`/`max_occurs` do not apply to a choice variant",
            ));
        }
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
        let checks = attrs.facets.value_checks();
        validates.push(quote! {
            Self::#var(value) => {
                path.push_field(#name);
                #checks
                ::authorizenet::validate::Validate::validate_into(value, path, out);
                path.pop();
            }
        });
        names.push(name);
    }

    Ok(quote! {
        impl #x::XmlChoice for #ident {
            const TYPE_NAME: &'static str = #type_name;

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

        impl #x::XmlWrite for #ident {
            fn write_element<W: ::std::io::Write>(
                &self,
                w: &mut #x::XmlWriter<W>,
                tag: &str,
            ) -> ::core::result::Result<(), #x::XmlError> {
                #x::write_choice_element(self, w, tag)
            }
        }

        impl #x::XmlRead for #ident {
            fn read_element<'a>(
                r: &mut #x::XmlReader<'a>,
                _start: &#x::BytesStart<'a>,
            ) -> ::core::result::Result<Self, #x::XmlError> {
                #x::read_choice_element(r)
            }
        }

        impl ::authorizenet::validate::Validate for #ident {
            fn validate_into(
                &self,
                path: &mut ::authorizenet::validate::Path,
                out: &mut ::std::vec::Vec<::authorizenet::validate::Violation>,
            ) {
                match self {
                    #(#validates)*
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
