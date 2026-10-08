//! Value types for XSD built-ins that have no exact Rust equivalent.

use std::borrow::Cow;
use std::fmt;
use std::io::Write;

use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::{Date, OffsetDateTime, PrimitiveDateTime, UtcOffset};

use crate::xml::{
    BytesStart, XmlError, XmlRead, XmlReader, XmlScalar, XmlWrite, XmlWriter, read_scalar,
    read_scalar_optional, write_scalar,
};

const DATE: &[BorrowedFormatItem<'_>] = format_description!("[year]-[month]-[day]");
const DATE_TIME: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]");
const DATE_TIME_OUT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
const DATE_TIME_FRACTION_OUT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond]");
const OFFSET: &[BorrowedFormatItem<'_>] =
    format_description!("[offset_hour sign:mandatory]:[offset_minute]");

/// An `xs:dateTime`, with or without a UTC offset.
///
/// Authorize.Net sends UTC times with `Z` and merchant-local times without an offset.
/// The original text is kept so that a value round-trips unchanged.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct XmlDateTime {
    datetime: PrimitiveDateTime,
    offset: Option<UtcOffset>,
    lexical: String,
}

impl XmlDateTime {
    /// Parses `YYYY-MM-DDThh:mm:ss[.fff][Z|±hh:mm]`.
    pub fn parse(text: &str) -> Result<Self, XmlError> {
        let invalid = |reason: &str| XmlError::invalid("dateTime", text, reason.to_owned());
        let (body, offset) = split_offset(text).map_err(|_| invalid("invalid UTC offset"))?;
        let datetime = PrimitiveDateTime::parse(body, DATE_TIME)
            .map_err(|e| invalid(&format!("expected YYYY-MM-DDThh:mm:ss ({e})")))?;
        Ok(Self {
            datetime,
            offset,
            lexical: text.to_owned(),
        })
    }

    /// The date and time as written, ignoring any offset.
    pub fn datetime(&self) -> PrimitiveDateTime {
        self.datetime
    }

    /// The UTC offset, if the value has one.
    pub fn offset(&self) -> Option<UtcOffset> {
        self.offset
    }

    /// The instant this value denotes, if it has an offset.
    pub fn to_offset_date_time(&self) -> Option<OffsetDateTime> {
        self.offset
            .map(|offset| self.datetime.assume_offset(offset))
    }

    /// The text as it appears in XML.
    pub fn as_str(&self) -> &str {
        &self.lexical
    }
}

impl From<OffsetDateTime> for XmlDateTime {
    fn from(value: OffsetDateTime) -> Self {
        let datetime = PrimitiveDateTime::new(value.date(), value.time());
        let mut lexical = format_date_time(datetime);
        lexical.push_str(&format_offset(value.offset()));
        Self {
            datetime,
            offset: Some(value.offset()),
            lexical,
        }
    }
}

impl From<PrimitiveDateTime> for XmlDateTime {
    fn from(datetime: PrimitiveDateTime) -> Self {
        Self {
            datetime,
            offset: None,
            lexical: format_date_time(datetime),
        }
    }
}

/// An `xs:date`, with or without a UTC offset.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct XmlDate {
    date: Date,
    offset: Option<UtcOffset>,
    lexical: String,
}

impl XmlDate {
    /// Parses `YYYY-MM-DD[Z|±hh:mm]`.
    pub fn parse(text: &str) -> Result<Self, XmlError> {
        let invalid = |reason: &str| XmlError::invalid("date", text, reason.to_owned());
        let (body, offset) = split_offset(text).map_err(|_| invalid("invalid UTC offset"))?;
        let date =
            Date::parse(body, DATE).map_err(|e| invalid(&format!("expected YYYY-MM-DD ({e})")))?;
        Ok(Self {
            date,
            offset,
            lexical: text.to_owned(),
        })
    }

    pub fn date(&self) -> Date {
        self.date
    }

    pub fn offset(&self) -> Option<UtcOffset> {
        self.offset
    }

    /// The text as it appears in XML.
    pub fn as_str(&self) -> &str {
        &self.lexical
    }
}

impl From<Date> for XmlDate {
    fn from(date: Date) -> Self {
        let lexical = date.format(DATE).expect("year within YYYY range");
        Self {
            date,
            offset: None,
            lexical,
        }
    }
}

/// Splits a trailing `Z` or `±hh:mm` from an XSD date or time.
fn split_offset(text: &str) -> Result<(&str, Option<UtcOffset>), time::error::Parse> {
    let text = text.trim();
    if let Some(body) = text.strip_suffix('Z') {
        return Ok((body, Some(UtcOffset::UTC)));
    }
    // `±hh:mm` is the last six characters; a date's own `-` is earlier.
    if let Some(split) = text.len().checked_sub(6)
        && text.is_char_boundary(split)
    {
        let (body, tail) = text.split_at(split);
        if tail.starts_with(['+', '-']) && tail.as_bytes()[3] == b':' && body.len() >= 10 {
            return Ok((body, Some(UtcOffset::parse(tail, OFFSET)?)));
        }
    }
    Ok((text, None))
}

fn format_date_time(datetime: PrimitiveDateTime) -> String {
    let format = if datetime.nanosecond() == 0 {
        DATE_TIME_OUT
    } else {
        DATE_TIME_FRACTION_OUT
    };
    datetime.format(format).expect("year within YYYY range")
}

fn format_offset(offset: UtcOffset) -> String {
    if offset.is_utc() {
        "Z".to_owned()
    } else {
        offset.format(OFFSET).expect("offset is formattable")
    }
}

/// Markup kept as-is, for `xs:anyType` elements such as `emvData`.
///
/// Holds the element's inner XML exactly as received; when writing, it must be
/// well-formed.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct RawXml(pub String);

macro_rules! debug_as_str {
    ($($ty:ty),*) => {$(
        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.as_str(), f)
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    )*};
}

debug_as_str!(XmlDateTime, XmlDate);

impl fmt::Debug for RawXml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RawXml").field(&self.0).finish()
    }
}

macro_rules! date_scalar {
    ($($ty:ty => $name:literal),*) => {$(
        impl XmlScalar for $ty {
            const TYPE_NAME: &'static str = $name;

            fn to_xml_text(&self) -> Cow<'_, str> {
                Cow::Borrowed(&self.lexical)
            }

            fn from_xml_text(text: &str) -> Result<Self, XmlError> {
                Self::parse(text)
            }
        }

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

date_scalar!(XmlDateTime => "dateTime", XmlDate => "date");

impl XmlWrite for RawXml {
    fn write_element<W: Write>(&self, w: &mut XmlWriter<W>, tag: &str) -> Result<(), XmlError> {
        w.raw_element(tag, &self.0)
    }
}

impl XmlRead for RawXml {
    fn read_element<'a>(r: &mut XmlReader<'a>, start: &BytesStart<'a>) -> Result<Self, XmlError> {
        Ok(Self(r.read_raw(start)?.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime, offset};

    #[test]
    fn date_time_with_utc_designator() {
        let value = XmlDateTime::parse("2010-05-31T19:09:04Z").unwrap();
        assert_eq!(
            value.to_offset_date_time(),
            Some(datetime!(2010-05-31 19:09:04 UTC))
        );
    }

    #[test]
    fn date_time_with_numeric_offset_and_fraction() {
        let value = XmlDateTime::parse("2010-05-31T12:09:04.25-07:00").unwrap();
        assert_eq!(value.offset(), Some(offset!(-7)));
        assert_eq!(value.datetime(), datetime!(2010-05-31 12:09:04.25));
    }

    #[test]
    fn local_date_time_has_no_offset() {
        let value = XmlDateTime::parse("2010-05-31T12:09:04").unwrap();
        assert_eq!(value.offset(), None);
        assert_eq!(value.to_offset_date_time(), None);
    }

    #[test]
    fn date_time_keeps_lexical_form() {
        let text = "2010-05-31T19:09:04.250+00:00";
        assert_eq!(XmlDateTime::parse(text).unwrap().as_str(), text);
    }

    #[test]
    fn date_time_from_offset_uses_z_for_utc() {
        let value = XmlDateTime::from(datetime!(2024-01-02 03:04:05 UTC));
        assert_eq!(value.as_str(), "2024-01-02T03:04:05Z");
    }

    #[test]
    fn date_time_from_offset_formats_fraction_and_offset() {
        let value = XmlDateTime::from(datetime!(2024-01-02 03:04:05.5 -05:00));
        assert_eq!(value.as_str(), "2024-01-02T03:04:05.5-05:00");
    }

    #[test]
    fn invalid_date_time_is_rejected() {
        assert!(XmlDateTime::parse("2010-05-31").is_err());
    }

    #[test]
    fn date_without_offset() {
        let value = XmlDate::parse("2024-02-29").unwrap();
        assert_eq!(value.date(), date!(2024 - 02 - 29));
        assert_eq!(value.offset(), None);
    }

    #[test]
    fn date_with_offset() {
        let value = XmlDate::parse("2024-02-29-05:00").unwrap();
        assert_eq!(value.date(), date!(2024 - 02 - 29));
        assert_eq!(value.offset(), Some(offset!(-5)));
    }

    #[test]
    fn date_from_date() {
        assert_eq!(XmlDate::from(date!(2024 - 02 - 09)).as_str(), "2024-02-09");
    }
}
