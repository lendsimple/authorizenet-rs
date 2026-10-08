//! `XmlScalar`, `XmlRead` and `XmlWrite` for the built-in simple types.

use std::borrow::Cow;
use std::io::Write;
use std::str::FromStr;

use rust_decimal::Decimal;

use super::{
    BytesStart, XmlError, XmlRead, XmlReader, XmlScalar, XmlWrite, XmlWriter, read_scalar,
    read_scalar_optional, write_scalar,
};

/// Implements `XmlWrite` and `XmlRead` in terms of `XmlScalar`.
macro_rules! scalar_element {
    ($($ty:ty),* $(,)?) => {$(
        impl XmlWrite for $ty {
            fn write_element<W: Write>(&self, w: &mut XmlWriter<W>, tag: &str) -> Result<(), XmlError> {
                write_scalar(self, w, tag)
            }
        }

        impl XmlRead for $ty {
            fn read_element<'a>(r: &mut XmlReader<'a>, start: &BytesStart<'a>) -> Result<Self, XmlError> {
                read_scalar(r, start)
            }

            fn read_optional<'a>(
                r: &mut XmlReader<'a>,
                start: &BytesStart<'a>,
            ) -> Result<Option<Self>, XmlError> {
                read_scalar_optional(r, start)
            }
        }
    )*};
}

/// Implements `XmlScalar` for numeric types through `FromStr` and `Display`.
macro_rules! numeric_scalar {
    ($($ty:ty => $name:literal),* $(,)?) => {$(
        impl XmlScalar for $ty {
            const TYPE_NAME: &'static str = $name;

            fn to_xml_text(&self) -> Cow<'_, str> {
                Cow::Owned(self.to_string())
            }

            fn from_xml_text(text: &str) -> Result<Self, XmlError> {
                let trimmed = text.trim();
                <$ty>::from_str(trimmed).map_err(|e| XmlError::invalid($name, trimmed, e.to_string()))
            }
        }
    )*};
}

numeric_scalar! {
    i32 => "int",
    i64 => "long",
    u32 => "unsignedInt",
    u64 => "unsignedLong",
    Decimal => "decimal",
}

impl XmlScalar for String {
    const TYPE_NAME: &'static str = "string";
    const EMPTY_IS_VALUE: bool = true;

    fn to_xml_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    fn from_xml_text(text: &str) -> Result<Self, XmlError> {
        Ok(text.to_owned())
    }
}

impl XmlScalar for bool {
    const TYPE_NAME: &'static str = "boolean";

    fn to_xml_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(if *self { "true" } else { "false" })
    }

    fn from_xml_text(text: &str) -> Result<Self, XmlError> {
        match text.trim() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => Err(XmlError::invalid(
                "boolean",
                other,
                "expected true, false, 1 or 0",
            )),
        }
    }
}

scalar_element!(String, bool, i32, i64, u32, u64, Decimal);
