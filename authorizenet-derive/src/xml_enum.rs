//! `#[derive(AnetEnum)]`: an `xs:string` restriction with enumerated values.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Result};

use crate::attrs::VariantAttrs;

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "AnetEnum can only be derived for enums",
        ));
    };
    let ident = &input.ident;
    let type_name = ident.to_string();
    let x = quote!(::authorizenet::xml);

    let mut to_str = Vec::new();
    let mut from_str = Vec::new();
    let mut other = None;
    for variant in &data.variants {
        let attrs = VariantAttrs::parse(&variant.attrs)?;
        if attrs.rename.is_some() || attrs.sensitive {
            return Err(syn::Error::new_spanned(
                variant,
                "AnetEnum variants take `value = \"...\"` or `other`",
            ));
        }
        let var = &variant.ident;
        if attrs.other {
            let is_string_tuple =
                matches!(&variant.fields, Fields::Unnamed(f) if f.unnamed.len() == 1);
            if !is_string_tuple {
                return Err(syn::Error::new_spanned(
                    variant,
                    "the `other` variant must hold the raw value, e.g. `Other(String)`",
                ));
            }
            if other.is_some() {
                return Err(syn::Error::new_spanned(
                    variant,
                    "only one variant may be marked `other`",
                ));
            }
            other = Some(var);
            continue;
        }
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "AnetEnum variants must be unit variants (except the `other` variant)",
            ));
        }
        let Some(value) = attrs.value else {
            return Err(syn::Error::new_spanned(
                variant,
                "missing `#[anet(value = \"...\")]` giving the XML value",
            ));
        };
        to_str.push(quote!(Self::#var => #value,));
        from_str.push(quote!(#value => Self::#var,));
    }

    let (other_to_str, fallback) = match other {
        Some(var) => (
            quote!(Self::#var(value) => value.as_str(),),
            quote!(value => Self::#var(value.to_owned()),),
        ),
        None => (
            quote!(),
            quote! {
                value => {
                    return ::core::result::Result::Err(#x::XmlError::InvalidValue {
                        ty: #type_name,
                        value: value.to_owned(),
                        reason: "not one of the enumerated values".into(),
                    });
                }
            },
        ),
    };

    Ok(quote! {
        impl #ident {
            /// The value as it appears in XML.
            pub fn as_str(&self) -> &str {
                match self {
                    #(#to_str)*
                    #other_to_str
                }
            }
        }

        impl ::core::fmt::Display for #ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl #x::XmlScalar for #ident {
            const TYPE_NAME: &'static str = #type_name;

            fn to_xml_text(&self) -> ::std::borrow::Cow<'_, str> {
                ::std::borrow::Cow::Borrowed(self.as_str())
            }

            fn from_xml_text(text: &str) -> ::core::result::Result<Self, #x::XmlError> {
                ::core::result::Result::Ok(match text.trim() {
                    #(#from_str)*
                    #fallback
                })
            }
        }

        impl #x::XmlWrite for #ident {
            fn write_element<W: ::std::io::Write>(
                &self,
                w: &mut #x::XmlWriter<W>,
                tag: &str,
            ) -> ::core::result::Result<(), #x::XmlError> {
                #x::write_scalar(self, w, tag)
            }
        }

        impl #x::XmlRead for #ident {
            fn read_element<'a>(
                r: &mut #x::XmlReader<'a>,
                start: &#x::BytesStart<'a>,
            ) -> ::core::result::Result<Self, #x::XmlError> {
                #x::read_scalar(r, start)
            }

            fn read_optional<'a>(
                r: &mut #x::XmlReader<'a>,
                start: &#x::BytesStart<'a>,
            ) -> ::core::result::Result<::core::option::Option<Self>, #x::XmlError> {
                #x::read_scalar_optional(r, start)
            }
        }
    })
}
