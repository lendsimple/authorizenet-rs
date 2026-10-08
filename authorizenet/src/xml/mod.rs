//! XML mapping for Authorize.Net's schema.
//!
//! Schema types implement these traits through `#[derive(AnetXml)]` and
//! `#[derive(AnetEnum)]`. Most users only need [`to_string`] and [`from_slice`],
//! and usually not even those: the client calls them.

mod error;
mod reader;
mod scalar;
mod writer;

use std::borrow::Cow;
use std::fmt;
use std::io::Write;

pub use authorizenet_derive::{AnetEnum, AnetXml};
#[doc(hidden)]
pub use quick_xml::events::BytesStart;

pub use error::XmlError;
pub use reader::XmlReader;
pub use writer::{AttrList, XmlWriter};

/// The namespace of every element in the Authorize.Net API.
pub const NAMESPACE: &str = "AnetApi/xml/v1/schema/AnetApiSchema.xsd";

/// A value that can be written as an element.
pub trait XmlWrite {
    /// Writes `self` as an element named `tag`.
    fn write_element<W: Write>(&self, w: &mut XmlWriter<W>, tag: &str) -> Result<(), XmlError>;
}

/// A value that can be read from an element.
pub trait XmlRead: Sized {
    /// Reads the element that `start` opened, through its end tag.
    fn read_element<'a>(r: &mut XmlReader<'a>, start: &BytesStart<'a>) -> Result<Self, XmlError>;

    /// Reads an element backing an optional field. `xsi:nil` elements are `None`.
    fn read_optional<'a>(
        r: &mut XmlReader<'a>,
        start: &BytesStart<'a>,
    ) -> Result<Option<Self>, XmlError> {
        if r.is_nil(start)? {
            r.skip(start)?;
            return Ok(None);
        }
        Self::read_element(r, start).map(Some)
    }
}

/// A value with simple (text-only) content.
pub trait XmlScalar: Sized {
    /// The type's name in error messages.
    const TYPE_NAME: &'static str;

    /// Whether an empty element is a value (`""` for strings) rather than an absent one.
    const EMPTY_IS_VALUE: bool = false;

    fn to_xml_text(&self) -> Cow<'_, str>;

    fn from_xml_text(text: &str) -> Result<Self, XmlError>;
}

/// A complex type: attributes plus a sequence of child elements.
pub trait XmlComplex: Sized {
    /// Accumulates fields while the element's children are read.
    #[doc(hidden)]
    type Partial: XmlPartial<Output = Self>;

    /// The type's name in error messages.
    const TYPE_NAME: &'static str;

    fn write_attributes<'s>(&'s self, attrs: &mut AttrList<'s>);

    fn write_children<W: Write>(&self, w: &mut XmlWriter<W>) -> Result<(), XmlError>;
}

/// In-progress state of a complex type being read.
#[doc(hidden)]
pub trait XmlPartial: Default {
    type Output;

    /// Returns `false` if the attribute is not part of this type.
    fn accept_attribute(&mut self, name: &str, value: &str) -> Result<bool, XmlError>;

    /// Reads the child `start` opened and returns `true`, or returns `false` (consuming
    /// nothing) if the element is not part of this type.
    fn accept_child<'a>(
        &mut self,
        r: &mut XmlReader<'a>,
        start: &BytesStart<'a>,
        name: &str,
    ) -> Result<bool, XmlError>;

    fn finish(self) -> Result<Self::Output, XmlError>;
}

/// An `xs:choice` group: exactly one of several sibling elements.
///
/// A choice enum can also be an element's whole content; it then implements
/// [`XmlRead`] and [`XmlWrite`] for that wrapping element.
pub trait XmlChoice: Sized {
    /// The type's name in error messages.
    const TYPE_NAME: &'static str;

    /// Whether `name` is one of the group's elements.
    fn matches(name: &str) -> bool;

    fn read_choice<'a>(
        r: &mut XmlReader<'a>,
        start: &BytesStart<'a>,
        name: &str,
    ) -> Result<Self, XmlError>;

    fn write_choice<W: Write>(&self, w: &mut XmlWriter<W>) -> Result<(), XmlError>;
}

/// A complex type that can be a whole document.
pub trait XmlRoot: XmlComplex {
    /// The root element's name.
    const ROOT: &'static str;

    /// Whether the document carries `merchantAuthentication` (an `ANetApiRequest`).
    const AUTHENTICATED: bool;
}

/// Shown in place of a sensitive value in `Debug` output.
pub struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Serializes a document.
pub fn to_string<T: XmlRoot>(value: &T) -> Result<String, XmlError> {
    let w = XmlWriter::new(Vec::new());
    finish(write_document(value, w, |_| Ok(()))?)
}

/// Serializes a document with indentation, for logs and debugging.
pub fn to_string_pretty<T: XmlRoot>(value: &T) -> Result<String, XmlError> {
    let w = XmlWriter::pretty(Vec::new());
    finish(write_document(value, w, |_| Ok(()))?)
}

/// Serializes a request document, writing `auth` as its first child,
/// `<merchantAuthentication>`.
pub fn to_string_with_auth<T: XmlRoot, A: XmlWrite>(
    value: &T,
    auth: &A,
) -> Result<String, XmlError> {
    let w = XmlWriter::new(Vec::new());
    finish(write_document(value, w, |w| {
        auth.write_element(w, "merchantAuthentication")
    })?)
}

fn write_document<T, W, F>(
    value: &T,
    mut w: XmlWriter<W>,
    first_children: F,
) -> Result<XmlWriter<W>, XmlError>
where
    T: XmlRoot,
    W: Write,
    F: FnOnce(&mut XmlWriter<W>) -> Result<(), XmlError>,
{
    w.declaration()?;
    let mut attrs: AttrList = vec![("xmlns", Cow::Borrowed(NAMESPACE))];
    value.write_attributes(&mut attrs);
    w.start(T::ROOT, &attrs)?;
    first_children(&mut w)?;
    value.write_children(&mut w)?;
    w.end(T::ROOT)?;
    Ok(w)
}

fn finish(w: XmlWriter<Vec<u8>>) -> Result<String, XmlError> {
    String::from_utf8(w.into_inner()).map_err(|e| XmlError::Utf8(e.utf8_error()))
}

/// Parses a document, skipping elements the schema does not define.
pub fn from_slice<T: XmlRoot>(input: &[u8]) -> Result<T, XmlError> {
    parse_document(input, false)
}

/// Parses a document, rejecting elements and text the schema does not define.
pub fn from_slice_strict<T: XmlRoot>(input: &[u8]) -> Result<T, XmlError> {
    parse_document(input, true)
}

/// Parses a document from a string, skipping elements the schema does not define.
pub fn from_str<T: XmlRoot>(input: &str) -> Result<T, XmlError> {
    parse_document(input.as_bytes(), false)
}

/// Parses a document's root element as `T`, whatever the element is called.
///
/// Useful for reading the common response fields from any response, or a fragment
/// such as a `<merchantAuthentication>` element.
pub fn from_slice_as<T: XmlRead>(input: &[u8]) -> Result<T, XmlError> {
    let mut r = XmlReader::new(decode(input)?, false);
    let start = r.read_root()?;
    T::read_element(&mut r, &start)
}

/// The local name of a document's root element.
pub fn root_name(input: &[u8]) -> Result<String, XmlError> {
    let mut r = XmlReader::new(decode(input)?, false);
    let start = r.read_root()?;
    Ok(reader::local_name(&start).to_owned())
}

fn parse_document<T: XmlRoot>(input: &[u8], strict: bool) -> Result<T, XmlError> {
    let mut r = XmlReader::new(decode(input)?, strict);
    let start = r.read_root()?;
    let found = reader::local_name(&start);
    if found != T::ROOT {
        return Err(XmlError::UnexpectedRoot {
            expected: T::ROOT,
            found: found.to_owned(),
        });
    }
    read_complex(&mut r, &start)
}

/// Strips a UTF-8 byte order mark, which Authorize.Net responses may start with.
fn decode(input: &[u8]) -> Result<&str, XmlError> {
    let input = input.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(input);
    Ok(std::str::from_utf8(input)?)
}

// Helpers called by derived code.

#[doc(hidden)]
pub fn write_complex<T: XmlComplex, W: Write>(
    value: &T,
    w: &mut XmlWriter<W>,
    tag: &str,
) -> Result<(), XmlError> {
    let mut attrs = AttrList::new();
    value.write_attributes(&mut attrs);
    w.start(tag, &attrs)?;
    value.write_children(w)?;
    w.end(tag)
}

#[doc(hidden)]
pub fn read_complex<'a, T: XmlComplex>(
    r: &mut XmlReader<'a>,
    start: &BytesStart<'a>,
) -> Result<T, XmlError> {
    let mut partial = T::Partial::default();
    for attr in reader::schema_attributes(start) {
        let (name, value) = attr?;
        // Unknown attributes are always ignored: the schema has only one attribute.
        partial.accept_attribute(name, &value)?;
    }
    r.read_children(T::TYPE_NAME, |r, child, name| {
        partial.accept_child(r, child, name)
    })?;
    partial.finish()
}

#[doc(hidden)]
pub fn write_scalar<T: XmlScalar, W: Write>(
    value: &T,
    w: &mut XmlWriter<W>,
    tag: &str,
) -> Result<(), XmlError> {
    w.text_element(tag, &value.to_xml_text())
}

#[doc(hidden)]
pub fn read_scalar<'a, T: XmlScalar>(
    r: &mut XmlReader<'a>,
    start: &BytesStart<'a>,
) -> Result<T, XmlError> {
    T::from_xml_text(&r.read_text(start)?)
}

#[doc(hidden)]
pub fn read_scalar_optional<'a, T: XmlScalar>(
    r: &mut XmlReader<'a>,
    start: &BytesStart<'a>,
) -> Result<Option<T>, XmlError> {
    let nil = r.is_nil(start)?;
    let text = r.read_text(start)?;
    if nil || (!T::EMPTY_IS_VALUE && text.trim().is_empty()) {
        return Ok(None);
    }
    T::from_xml_text(&text).map(Some)
}

#[doc(hidden)]
pub fn write_choice_element<T: XmlChoice, W: Write>(
    value: &T,
    w: &mut XmlWriter<W>,
    tag: &str,
) -> Result<(), XmlError> {
    w.start(tag, &[])?;
    value.write_choice(w)?;
    w.end(tag)
}

#[doc(hidden)]
pub fn read_choice_element<T: XmlChoice>(r: &mut XmlReader<'_>) -> Result<T, XmlError> {
    let mut value = None;
    r.read_children(T::TYPE_NAME, |r, child, name| {
        if !T::matches(name) {
            return Ok(false);
        }
        let choice = T::read_choice(r, child, name)?;
        set_once(&mut value, choice, r, T::TYPE_NAME, name)?;
        Ok(true)
    })?;
    value.ok_or(XmlError::MissingField {
        ty: T::TYPE_NAME,
        field: "choice",
    })
}

#[doc(hidden)]
pub fn write_wrapped<T: XmlWrite, W: Write>(
    items: &[T],
    w: &mut XmlWriter<W>,
    tag: &str,
    item: &str,
) -> Result<(), XmlError> {
    w.start(tag, &[])?;
    for value in items {
        value.write_element(w, item)?;
    }
    w.end(tag)
}

#[doc(hidden)]
pub fn read_wrapped<T: XmlRead>(
    r: &mut XmlReader<'_>,
    tag: &'static str,
    item: &str,
) -> Result<Vec<T>, XmlError> {
    let mut items = Vec::new();
    r.read_children(tag, |r, child, name| {
        if name != item {
            return Ok(false);
        }
        if let Some(value) = T::read_optional(r, child)? {
            items.push(value);
        }
        Ok(true)
    })?;
    Ok(items)
}

/// Stores a single-valued field. A repeat is an error in strict mode; otherwise the
/// last occurrence wins.
#[doc(hidden)]
pub fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    r: &XmlReader<'_>,
    ty: &'static str,
    name: &str,
) -> Result<(), XmlError> {
    if slot.is_some() && r.is_strict() {
        return Err(XmlError::DuplicateElement {
            ty,
            name: name.to_owned(),
        });
    }
    *slot = Some(value);
    Ok(())
}
