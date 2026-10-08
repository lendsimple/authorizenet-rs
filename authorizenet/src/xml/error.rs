use std::borrow::Cow;

/// An error reading or writing Authorize.Net XML.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum XmlError {
    /// The document is not well-formed XML.
    #[error("malformed XML: {0}")]
    Syntax(#[from] quick_xml::Error),

    /// The document is not valid UTF-8.
    #[error("XML is not valid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// The document's root element is not the one expected.
    #[error("expected root element <{expected}>, found <{found}>")]
    UnexpectedRoot {
        expected: &'static str,
        found: String,
    },

    /// The document has no root element.
    #[error("XML document has no root element")]
    NoRoot,

    /// A required element (or `@attribute`) is missing.
    #[error("{ty} is missing required <{field}>")]
    MissingField {
        ty: &'static str,
        field: &'static str,
    },

    /// An element that the schema does not define (strict mode only).
    #[error("{ty} has unexpected element <{name}>")]
    UnknownElement { ty: &'static str, name: String },

    /// A single-valued element occurs more than once (strict mode only).
    #[error("{ty} has duplicate element <{name}>")]
    DuplicateElement { ty: &'static str, name: String },

    /// Non-whitespace text inside an element that only has child elements (strict mode only).
    #[error("{ty} has unexpected text content")]
    UnexpectedText { ty: &'static str },

    /// An element with simple content contains a child element.
    #[error("<{element}> must contain only text, found child <{child}>")]
    UnexpectedChild { element: String, child: String },

    /// A text value cannot be converted to the target type.
    #[error("invalid {ty} value {value:?}: {reason}")]
    InvalidValue {
        ty: &'static str,
        value: String,
        reason: Cow<'static, str>,
    },

    /// An entity reference other than the XML predefined ones.
    #[error("unknown entity reference &{0};")]
    UnknownEntity(String),

    /// The document ended in the middle of an element.
    #[error("unexpected end of XML document")]
    UnexpectedEof,

    /// Writing to the output failed.
    #[error("failed to write XML: {0}")]
    Io(#[from] std::io::Error),
}

impl XmlError {
    pub(crate) fn invalid(
        ty: &'static str,
        value: &str,
        reason: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::InvalidValue {
            ty,
            value: value.to_owned(),
            reason: reason.into(),
        }
    }
}

impl From<quick_xml::events::attributes::AttrError> for XmlError {
    fn from(err: quick_xml::events::attributes::AttrError) -> Self {
        Self::Syntax(err.into())
    }
}
