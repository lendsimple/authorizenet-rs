//! Checks of the XSD's facets: lengths, patterns, numeric ranges and digits, and how
//! many times a list element may occur.
//!
//! Every schema type implements [`Validate`]. The client validates requests before
//! sending them, so a value the API would reject with a terse `E00003` is reported
//! with the path of the offending field instead.

use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;

use crate::types::{RawXml, XmlDate, XmlDateTime};

/// A type whose values can be checked against the schema's facets.
pub trait Validate {
    /// Checks `self` and everything inside it, collecting every violation.
    fn validate(&self) -> Result<(), ValidationError> {
        let mut path = Path::default();
        let mut violations = Vec::new();
        self.validate_into(&mut path, &mut violations);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(ValidationError { violations })
        }
    }

    #[doc(hidden)]
    fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>);
}

/// One value that breaks a facet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Where the value is, by XML element names: `transactionRequest.lineItems[0].name`.
    pub path: String,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// A value does not satisfy the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    violations: Vec<Violation>,
}

impl ValidationError {
    /// Every violation found, in document order. Never empty.
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (first, rest) = self.violations.split_first().expect("never empty");
        write!(f, "{first}")?;
        match rest.len() {
            0 => Ok(()),
            1 => write!(f, " (and 1 more violation)"),
            n => write!(f, " (and {n} more violations)"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// The location being validated, maintained by derived code.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct Path {
    segments: Vec<Segment>,
}

#[derive(Debug)]
enum Segment {
    Field(&'static str),
    Index(usize),
}

impl Path {
    pub fn push_field(&mut self, name: &'static str) {
        self.segments.push(Segment::Field(name));
    }

    pub fn push_index(&mut self, index: usize) {
        self.segments.push(Segment::Index(index));
    }

    pub fn pop(&mut self) {
        self.segments.pop();
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Field(name) => {
                    if !out.is_empty() {
                        out.push('.');
                    }
                    out.push_str(name);
                }
                Segment::Index(i) => out.push_str(&format!("[{i}]")),
            }
        }
        out
    }
}

fn violation(path: &Path, out: &mut Vec<Violation>, message: String) {
    out.push(Violation {
        path: path.render(),
        message,
    });
}

/// Types with no facets of their own.
macro_rules! no_facets {
    ($($ty:ty),*) => {$(
        impl Validate for $ty {
            fn validate_into(&self, _path: &mut Path, _out: &mut Vec<Violation>) {}
        }
    )*};
}

no_facets!(
    String,
    bool,
    i16,
    i32,
    i64,
    u32,
    u64,
    Decimal,
    XmlDate,
    XmlDateTime,
    RawXml
);

// Facet checks called by derived code. Lengths count characters, as in XSD.

#[doc(hidden)]
pub fn check_length(
    value: &str,
    min: Option<usize>,
    max: Option<usize>,
    exact: Option<usize>,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    let len = value.chars().count();
    if let Some(exact) = exact
        && len != exact
    {
        violation(
            path,
            out,
            format!("must be exactly {exact} characters, not {len}"),
        );
    }
    if let Some(min) = min
        && len < min
    {
        violation(
            path,
            out,
            format!("must be at least {min} characters, not {len}"),
        );
    }
    if let Some(max) = max
        && len > max
    {
        violation(
            path,
            out,
            format!("must be at most {max} characters, not {len}"),
        );
    }
}

/// `allowed` is the character class of a `[class]+` pattern.
#[doc(hidden)]
pub fn check_pattern(
    value: &str,
    pattern: &'static str,
    allowed: fn(char) -> bool,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    if value.is_empty() || !value.chars().all(allowed) {
        violation(path, out, format!("must match the pattern {pattern}"));
    }
}

#[doc(hidden)]
pub fn check_range<T>(
    value: &T,
    min: Option<&str>,
    max: Option<&str>,
    path: &Path,
    out: &mut Vec<Violation>,
) where
    T: PartialOrd + FromStr + fmt::Display,
{
    let bound = |text: &str| {
        T::from_str(text)
            .ok()
            .unwrap_or_else(|| panic!("schema bound {text:?} must parse as the field's type"))
    };
    if let Some(min) = min
        && *value < bound(min)
    {
        violation(path, out, format!("must be at least {min}, not {value}"));
    }
    if let Some(max) = max
        && *value > bound(max)
    {
        violation(path, out, format!("must be at most {max}, not {value}"));
    }
}

/// Digit counts ignore trailing fractional zeros, as XSD does: `45.00` has 2 digits
/// and no fraction digits.
#[doc(hidden)]
pub fn check_digits(
    value: &Decimal,
    total: Option<u32>,
    fraction: Option<u32>,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    let normalized = value.normalize();
    if let Some(fraction) = fraction
        && normalized.scale() > fraction
    {
        violation(
            path,
            out,
            format!(
                "must have at most {fraction} digits after the decimal point, not {}",
                normalized.scale()
            ),
        );
    }
    if let Some(total) = total {
        let digits = normalized
            .mantissa()
            .unsigned_abs()
            .to_string()
            .trim_start_matches('0')
            .len()
            .max(1) as u32;
        if digits > total {
            violation(
                path,
                out,
                format!("must have at most {total} digits, not {digits}"),
            );
        }
    }
}

#[doc(hidden)]
pub fn check_occurs(
    len: usize,
    min: Option<usize>,
    max: Option<usize>,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    if let Some(min) = min
        && len < min
    {
        violation(
            path,
            out,
            format!("must have at least {min} items, not {len}"),
        );
    }
    if let Some(max) = max
        && len > max
    {
        violation(
            path,
            out,
            format!("must have at most {max} items, not {len}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(check: impl FnOnce(&Path, &mut Vec<Violation>)) -> Vec<String> {
        let mut path = Path::default();
        path.push_field("transactionRequest");
        path.push_field("lineItems");
        path.push_index(2);
        path.push_field("name");
        let mut out = Vec::new();
        check(&path, &mut out);
        out.into_iter().map(|v| v.to_string()).collect()
    }

    fn dec(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    #[test]
    fn path_renders_fields_and_indexes() {
        let out = run(|p, o| check_length("abc", None, Some(2), None, p, o));
        assert_eq!(
            out,
            ["transactionRequest.lineItems[2].name: must be at most 2 characters, not 3"]
        );
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert!(run(|p, o| check_length("ééé", None, Some(3), None, p, o)).is_empty());
    }

    #[test]
    fn exact_length() {
        assert_eq!(
            run(|p, o| check_length("US", None, None, Some(3), p, o)).len(),
            1
        );
    }

    #[test]
    fn pattern_requires_at_least_one_allowed_char() {
        let digits = |c: char| c.is_ascii_digit();
        assert!(run(|p, o| check_pattern("123", "[0-9]+", digits, p, o)).is_empty());
        assert_eq!(
            run(|p, o| check_pattern("12a", "[0-9]+", digits, p, o)).len(),
            1
        );
        assert_eq!(
            run(|p, o| check_pattern("", "[0-9]+", digits, p, o)).len(),
            1
        );
    }

    #[test]
    fn decimal_range() {
        assert_eq!(
            run(|p, o| check_range(&dec("-1.00"), Some("0.00"), None, p, o)),
            ["transactionRequest.lineItems[2].name: must be at least 0.00, not -1.00"]
        );
        assert!(run(|p, o| check_range(&dec("0.01"), Some("0.01"), None, p, o)).is_empty());
    }

    #[test]
    fn integer_range() {
        assert_eq!(
            run(|p, o| check_range(&1001, Some("1"), Some("1000"), p, o)).len(),
            1
        );
    }

    #[test]
    fn fraction_digits_ignore_trailing_zeros() {
        assert!(run(|p, o| check_digits(&dec("45.1200"), None, Some(2), p, o)).is_empty());
        assert_eq!(
            run(|p, o| check_digits(&dec("1.23456"), None, Some(4), p, o)).len(),
            1
        );
    }

    #[test]
    fn total_digits() {
        assert!(run(|p, o| check_digits(&dec("0.12345"), Some(5), None, p, o)).is_empty());
        assert_eq!(
            run(|p, o| check_digits(&dec("123.456"), Some(5), None, p, o)).len(),
            1
        );
    }

    #[test]
    fn occurrences() {
        assert_eq!(run(|p, o| check_occurs(31, None, Some(30), p, o)).len(), 1);
        assert_eq!(run(|p, o| check_occurs(0, Some(1), None, p, o)).len(), 1);
    }

    #[test]
    fn error_display_counts_the_rest() {
        let v = |path: &str| Violation {
            path: path.into(),
            message: "bad".into(),
        };
        let err = ValidationError {
            violations: vec![v("a"), v("b"), v("c")],
        };
        assert_eq!(err.to_string(), "a: bad (and 2 more violations)");
    }
}
