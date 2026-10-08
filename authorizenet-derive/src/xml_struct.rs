//! `#[derive(AnetXml)]` on structs: complex types with an element sequence.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DataStruct, DeriveInput, Fields, Ident, Result, Type};

use crate::attrs::{ContainerAttrs, FieldAttrs};
use crate::cardinality::Cardinality;
use crate::naming::{field_display_name, field_xml_name};

/// One struct field, classified by how it maps to XML.
struct FieldSpec<'a> {
    ident: &'a Ident,
    display: String,
    sensitive: bool,
    kind: Kind<'a>,
}

enum Kind<'a> {
    /// A child element (`name`) with the given cardinality.
    Element { name: String, card: Cardinality<'a> },
    /// A `Vec` wrapped in a container element `name`, items named `item`.
    Wrapped {
        name: String,
        item: String,
        item_ty: &'a Type,
        optional: bool,
    },
    /// An XML attribute on this element.
    Attribute {
        name: String,
        ty: &'a Type,
        optional: bool,
    },
    /// Fields of an XSD base type, written inline before the extension's own fields.
    Flatten { ty: &'a Type },
    /// An `xs:choice` group: each variant is a sibling element.
    Choice { card: Cardinality<'a> },
}

pub fn expand(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "AnetXml structs must have named fields",
        ));
    };
    let container = ContainerAttrs::parse(&input.attrs)?;
    let fields = named
        .named
        .iter()
        .map(classify)
        .collect::<Result<Vec<_>>>()?;

    let ident = &input.ident;
    let vis = &input.vis;
    let type_name = ident.to_string();
    let partial = format_ident!("__AnetPartial{}", ident);
    let x = quote!(::authorizenet::xml);

    let partial_fields = fields.iter().map(|f| partial_field(f, &x));
    let attr_arms = fields.iter().filter_map(|f| accept_attribute(f, &x));
    let child_arms = fields
        .iter()
        .filter_map(|f| accept_child(f, &x, &type_name));
    let finish = fields.iter().map(|f| finish_field(f, &type_name));
    let write_attrs = fields.iter().filter_map(|f| write_attribute(f, &x));
    let write_children = fields.iter().filter_map(|f| write_child(f, &x));
    let debug_fields = fields.iter().map(debug_field);

    let skip_auth = container.request.map(|_| {
        quote! {
            if name == "merchantAuthentication" {
                r.skip(start)?;
                return ::core::result::Result::Ok(true);
            }
        }
    });

    let root_impl = container.root.as_ref().map(|root| {
        let authenticated = container.request.is_some();
        quote! {
            impl #x::XmlRoot for #ident {
                const ROOT: &'static str = #root;
                const AUTHENTICATED: bool = #authenticated;
            }
        }
    });

    Ok(quote! {
        #[doc(hidden)]
        #[derive(Default)]
        #vis struct #partial {
            #(#partial_fields,)*
        }

        impl #x::XmlPartial for #partial {
            type Output = #ident;

            #[allow(unused_variables)]
            fn accept_attribute(
                &mut self,
                name: &str,
                value: &str,
            ) -> ::core::result::Result<bool, #x::XmlError> {
                #(#attr_arms)*
                ::core::result::Result::Ok(false)
            }

            #[allow(unused_variables)]
            fn accept_child<'a>(
                &mut self,
                r: &mut #x::XmlReader<'a>,
                start: &#x::BytesStart<'a>,
                name: &str,
            ) -> ::core::result::Result<bool, #x::XmlError> {
                #skip_auth
                #(#child_arms)*
                ::core::result::Result::Ok(false)
            }

            fn finish(self) -> ::core::result::Result<#ident, #x::XmlError> {
                ::core::result::Result::Ok(#ident {
                    #(#finish,)*
                })
            }
        }

        impl #x::XmlComplex for #ident {
            type Partial = #partial;
            const TYPE_NAME: &'static str = #type_name;

            #[allow(unused_variables)]
            fn write_attributes<'s>(&'s self, attrs: &mut #x::AttrList<'s>) {
                #(#write_attrs)*
            }

            #[allow(unused_variables)]
            fn write_children<W: ::std::io::Write>(
                &self,
                w: &mut #x::XmlWriter<W>,
            ) -> ::core::result::Result<(), #x::XmlError> {
                #(#write_children)*
                ::core::result::Result::Ok(())
            }
        }

        impl #x::XmlWrite for #ident {
            fn write_element<W: ::std::io::Write>(
                &self,
                w: &mut #x::XmlWriter<W>,
                tag: &str,
            ) -> ::core::result::Result<(), #x::XmlError> {
                #x::write_complex(self, w, tag)
            }
        }

        impl #x::XmlRead for #ident {
            fn read_element<'a>(
                r: &mut #x::XmlReader<'a>,
                start: &#x::BytesStart<'a>,
            ) -> ::core::result::Result<Self, #x::XmlError> {
                #x::read_complex(r, start)
            }
        }

        #root_impl

        impl ::core::fmt::Debug for #ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(#type_name)
                    #(#debug_fields)*
                    .finish()
            }
        }
    })
}

fn classify(field: &syn::Field) -> Result<FieldSpec<'_>> {
    let ident = field.ident.as_ref().expect("named field");
    let attrs = FieldAttrs::parse(&field.attrs)?;
    let name = attrs
        .rename
        .clone()
        .unwrap_or_else(|| field_xml_name(ident));
    let card = Cardinality::of(&field.ty);

    let kind = if let Some(span) = attrs.flatten {
        match card {
            Cardinality::Required(ty) => Kind::Flatten { ty },
            _ => return Err(err(span, "`flatten` fields must not be `Option` or `Vec`")),
        }
    } else if attrs.choice.is_some() {
        match card {
            Cardinality::OptionalList(_) => {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "`choice` fields must be `T`, `Option<T>` or `Vec<T>`",
                ));
            }
            card => Kind::Choice { card },
        }
    } else if let Some(span) = attrs.attribute {
        match card {
            Cardinality::Required(ty) => Kind::Attribute {
                name,
                ty,
                optional: false,
            },
            Cardinality::Optional(ty) => Kind::Attribute {
                name,
                ty,
                optional: true,
            },
            _ => return Err(err(span, "`attribute` fields must be `T` or `Option<T>`")),
        }
    } else if let Some(span) = attrs.wrapper {
        let item = attrs.item.clone().expect("checked in FieldAttrs");
        match card {
            Cardinality::List(item_ty) => Kind::Wrapped {
                name,
                item,
                item_ty,
                optional: false,
            },
            Cardinality::OptionalList(item_ty) => Kind::Wrapped {
                name,
                item,
                item_ty,
                optional: true,
            },
            _ => {
                return Err(err(
                    span,
                    "`wrapper` fields must be `Vec<T>` or `Option<Vec<T>>`",
                ));
            }
        }
    } else {
        match card {
            Cardinality::OptionalList(_) => {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "`Option<Vec<T>>` is only supported with `wrapper`",
                ));
            }
            card => Kind::Element { name, card },
        }
    };

    Ok(FieldSpec {
        ident,
        display: field_display_name(ident),
        sensitive: attrs.sensitive,
        kind,
    })
}

fn err(span: Span, msg: &str) -> syn::Error {
    syn::Error::new(span, msg)
}

fn partial_field(f: &FieldSpec, x: &TokenStream) -> TokenStream {
    let ident = f.ident;
    match &f.kind {
        Kind::Element { card, .. } | Kind::Choice { card } => match card {
            Cardinality::Required(ty) | Cardinality::Optional(ty) => {
                quote!(#ident: ::core::option::Option<#ty>)
            }
            Cardinality::List(ty) => quote!(#ident: ::std::vec::Vec<#ty>),
            Cardinality::OptionalList(_) => unreachable!("rejected in classify"),
        },
        Kind::Wrapped { item_ty, .. } => {
            quote!(#ident: ::core::option::Option<::std::vec::Vec<#item_ty>>)
        }
        Kind::Attribute { ty, .. } => quote!(#ident: ::core::option::Option<#ty>),
        Kind::Flatten { ty } => quote!(#ident: <#ty as #x::XmlComplex>::Partial),
    }
}

fn accept_attribute(f: &FieldSpec, x: &TokenStream) -> Option<TokenStream> {
    let ident = f.ident;
    match &f.kind {
        Kind::Attribute { name, ty, .. } => Some(quote! {
            if name == #name {
                self.#ident = ::core::option::Option::Some(
                    <#ty as #x::XmlScalar>::from_xml_text(value)?,
                );
                return ::core::result::Result::Ok(true);
            }
        }),
        Kind::Flatten { .. } => Some(quote! {
            if #x::XmlPartial::accept_attribute(&mut self.#ident, name, value)? {
                return ::core::result::Result::Ok(true);
            }
        }),
        _ => None,
    }
}

fn accept_child(f: &FieldSpec, x: &TokenStream, type_name: &str) -> Option<TokenStream> {
    let ident = f.ident;
    let ok = quote!(return ::core::result::Result::Ok(true););
    match &f.kind {
        Kind::Element { name, card } => Some(match card {
            Cardinality::Required(ty) => quote! {
                if name == #name {
                    let value = <#ty as #x::XmlRead>::read_element(r, start)?;
                    #x::set_once(&mut self.#ident, value, r, #type_name, #name)?;
                    #ok
                }
            },
            Cardinality::Optional(ty) => quote! {
                if name == #name {
                    if let ::core::option::Option::Some(value) =
                        <#ty as #x::XmlRead>::read_optional(r, start)?
                    {
                        #x::set_once(&mut self.#ident, value, r, #type_name, #name)?;
                    }
                    #ok
                }
            },
            Cardinality::List(ty) => quote! {
                if name == #name {
                    if let ::core::option::Option::Some(value) =
                        <#ty as #x::XmlRead>::read_optional(r, start)?
                    {
                        self.#ident.push(value);
                    }
                    #ok
                }
            },
            Cardinality::OptionalList(_) => unreachable!("rejected in classify"),
        }),
        Kind::Wrapped {
            name,
            item,
            item_ty,
            ..
        } => Some(quote! {
            if name == #name {
                let items = #x::read_wrapped::<#item_ty>(r, #name, #item)?;
                #x::set_once(&mut self.#ident, items, r, #type_name, #name)?;
                #ok
            }
        }),
        Kind::Attribute { .. } => None,
        Kind::Flatten { .. } => Some(quote! {
            if #x::XmlPartial::accept_child(&mut self.#ident, r, start, name)? {
                #ok
            }
        }),
        Kind::Choice { card } => Some(match card {
            Cardinality::Required(ty) | Cardinality::Optional(ty) => quote! {
                if <#ty as #x::XmlChoice>::matches(name) {
                    let value = <#ty as #x::XmlChoice>::read_choice(r, start, name)?;
                    #x::set_once(&mut self.#ident, value, r, #type_name, name)?;
                    #ok
                }
            },
            Cardinality::List(ty) => quote! {
                if <#ty as #x::XmlChoice>::matches(name) {
                    self.#ident.push(<#ty as #x::XmlChoice>::read_choice(r, start, name)?);
                    #ok
                }
            },
            Cardinality::OptionalList(_) => unreachable!("rejected in classify"),
        }),
    }
}

fn finish_field(f: &FieldSpec, type_name: &str) -> TokenStream {
    let ident = f.ident;
    let missing = |field: &str| {
        quote! {
            ::authorizenet::xml::XmlError::MissingField {
                ty: #type_name,
                field: #field,
            }
        }
    };
    match &f.kind {
        Kind::Element { name, card } => match card {
            Cardinality::Required(_) => {
                let missing = missing(name);
                quote!(#ident: self.#ident.ok_or(#missing)?)
            }
            _ => quote!(#ident: self.#ident),
        },
        Kind::Choice { card } => match card {
            Cardinality::Required(_) => {
                let missing = missing(&f.display);
                quote!(#ident: self.#ident.ok_or(#missing)?)
            }
            _ => quote!(#ident: self.#ident),
        },
        Kind::Wrapped { optional, .. } => {
            if *optional {
                quote!(#ident: self.#ident)
            } else {
                quote!(#ident: self.#ident.unwrap_or_default())
            }
        }
        Kind::Attribute { name, optional, .. } => {
            if *optional {
                quote!(#ident: self.#ident)
            } else {
                let missing = missing(&format!("@{name}"));
                quote!(#ident: self.#ident.ok_or(#missing)?)
            }
        }
        Kind::Flatten { .. } => {
            quote!(#ident: ::authorizenet::xml::XmlPartial::finish(self.#ident)?)
        }
    }
}

fn write_attribute(f: &FieldSpec, x: &TokenStream) -> Option<TokenStream> {
    let ident = f.ident;
    match &f.kind {
        Kind::Attribute {
            name,
            optional: false,
            ..
        } => Some(quote! {
            attrs.push((#name, #x::XmlScalar::to_xml_text(&self.#ident)));
        }),
        Kind::Attribute {
            name,
            optional: true,
            ..
        } => Some(quote! {
            if let ::core::option::Option::Some(value) = &self.#ident {
                attrs.push((#name, #x::XmlScalar::to_xml_text(value)));
            }
        }),
        Kind::Flatten { .. } => Some(quote! {
            #x::XmlComplex::write_attributes(&self.#ident, attrs);
        }),
        _ => None,
    }
}

fn write_child(f: &FieldSpec, x: &TokenStream) -> Option<TokenStream> {
    let ident = f.ident;
    match &f.kind {
        Kind::Element { name, card } => Some(match card {
            Cardinality::Required(_) => quote! {
                #x::XmlWrite::write_element(&self.#ident, w, #name)?;
            },
            Cardinality::Optional(_) => quote! {
                if let ::core::option::Option::Some(value) = &self.#ident {
                    #x::XmlWrite::write_element(value, w, #name)?;
                }
            },
            Cardinality::List(_) => quote! {
                for value in &self.#ident {
                    #x::XmlWrite::write_element(value, w, #name)?;
                }
            },
            Cardinality::OptionalList(_) => unreachable!("rejected in classify"),
        }),
        Kind::Wrapped {
            name,
            item,
            optional,
            ..
        } => Some(if *optional {
            quote! {
                if let ::core::option::Option::Some(items) = &self.#ident {
                    #x::write_wrapped(items, w, #name, #item)?;
                }
            }
        } else {
            quote! {
                if !self.#ident.is_empty() {
                    #x::write_wrapped(&self.#ident, w, #name, #item)?;
                }
            }
        }),
        Kind::Attribute { .. } => None,
        Kind::Flatten { .. } => Some(quote! {
            #x::XmlComplex::write_children(&self.#ident, w)?;
        }),
        Kind::Choice { card } => Some(match card {
            Cardinality::Required(_) => quote! {
                #x::XmlChoice::write_choice(&self.#ident, w)?;
            },
            Cardinality::Optional(_) => quote! {
                if let ::core::option::Option::Some(value) = &self.#ident {
                    #x::XmlChoice::write_choice(value, w)?;
                }
            },
            Cardinality::List(_) => quote! {
                for value in &self.#ident {
                    #x::XmlChoice::write_choice(value, w)?;
                }
            },
            Cardinality::OptionalList(_) => unreachable!("rejected in classify"),
        }),
    }
}

fn debug_field(f: &FieldSpec) -> TokenStream {
    let ident = f.ident;
    let display = &f.display;
    if !f.sensitive {
        return quote!(.field(#display, &self.#ident));
    }
    let optional = matches!(
        f.kind,
        Kind::Element {
            card: Cardinality::Optional(_),
            ..
        } | Kind::Attribute { optional: true, .. }
            | Kind::Choice {
                card: Cardinality::Optional(_)
            }
    );
    if optional {
        quote!(.field(#display, &self.#ident.as_ref().map(|_| ::authorizenet::xml::Redacted)))
    } else {
        quote!(.field(#display, &::authorizenet::xml::Redacted))
    }
}
