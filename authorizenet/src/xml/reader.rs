use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

use super::XmlError;

/// A pull reader over a complete in-memory XML document.
///
/// Element names are matched by local name, so namespace prefixes are ignored.
/// In lenient mode (the default) unknown elements and stray text are skipped, which
/// keeps parsing working when Authorize.Net adds fields. Strict mode turns them into
/// errors, which is useful in tests to catch schema gaps.
pub struct XmlReader<'a> {
    input: &'a str,
    inner: quick_xml::Reader<&'a [u8]>,
    strict: bool,
}

impl<'a> XmlReader<'a> {
    pub(crate) fn new(input: &'a str, strict: bool) -> Self {
        let mut inner = quick_xml::Reader::from_str(input);
        let config = inner.config_mut();
        config.expand_empty_elements = true;
        config.check_end_names = true;
        Self {
            input,
            inner,
            strict,
        }
    }

    /// Whether unknown content is an error rather than skipped.
    pub fn is_strict(&self) -> bool {
        self.strict
    }

    fn next(&mut self) -> Result<Event<'a>, XmlError> {
        Ok(self.inner.read_event()?)
    }

    /// Skips the prolog and returns the root element's start tag.
    pub(crate) fn read_root(&mut self) -> Result<BytesStart<'a>, XmlError> {
        loop {
            match self.next()? {
                Event::Start(start) => return Ok(start),
                Event::Eof => return Err(XmlError::NoRoot),
                _ => {}
            }
        }
    }

    /// Skips the rest of the element that `start` opened.
    pub fn skip(&mut self, start: &BytesStart<'a>) -> Result<(), XmlError> {
        self.inner.read_to_end(start.name())?;
        Ok(())
    }

    /// Returns the unparsed markup between `start` and its end tag.
    pub fn read_raw(&mut self, start: &BytesStart<'a>) -> Result<&'a str, XmlError> {
        let span = self.inner.read_to_end(start.name())?;
        let (from, to) = (span.start as usize, span.end as usize);
        Ok(self.input.get(from..to).unwrap_or_default())
    }

    /// Reads the text content of a simple-content element, resolving entity references.
    pub fn read_text(&mut self, start: &BytesStart<'a>) -> Result<String, XmlError> {
        let mut text = String::new();
        loop {
            match self.next()? {
                Event::Text(t) => text.push_str(&t.xml10_content()),
                Event::CData(c) => text.push_str(&c.xml10_content()),
                Event::GeneralRef(r) => match r.resolve_char_ref()? {
                    Some(ch) => text.push(ch),
                    None => match quick_xml::escape::resolve_predefined_entity(&r) {
                        Some(s) => text.push_str(s),
                        None => return Err(XmlError::UnknownEntity(r.to_string())),
                    },
                },
                Event::Start(child) => {
                    return Err(XmlError::UnexpectedChild {
                        element: local_name(start).to_owned(),
                        child: local_name(&child).to_owned(),
                    });
                }
                Event::End(_) => return Ok(text),
                Event::Eof => return Err(XmlError::UnexpectedEof),
                _ => {}
            }
        }
    }

    /// Reads the children of the element just opened, calling `accept` for each child start tag.
    ///
    /// `accept` returns `false` for an element it does not know, which is then skipped
    /// (or rejected in strict mode). `ty` names the parent type in error messages.
    pub fn read_children<F>(&mut self, ty: &'static str, mut accept: F) -> Result<(), XmlError>
    where
        F: FnMut(&mut Self, &BytesStart<'a>, &str) -> Result<bool, XmlError>,
    {
        loop {
            match self.next()? {
                Event::Start(child) => {
                    let name = local_name(&child);
                    if !accept(self, &child, name)? {
                        if self.strict {
                            return Err(XmlError::UnknownElement {
                                ty,
                                name: name.to_owned(),
                            });
                        }
                        self.skip(&child)?;
                    }
                }
                Event::End(_) => return Ok(()),
                Event::Text(t) if t.chars().all(char::is_whitespace) => {}
                Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) if self.strict => {
                    return Err(XmlError::UnexpectedText { ty });
                }
                Event::Eof => return Err(XmlError::UnexpectedEof),
                _ => {}
            }
        }
    }

    /// Whether the element carries `xsi:nil="true"`.
    pub fn is_nil(&self, start: &BytesStart<'a>) -> Result<bool, XmlError> {
        for attr in start.attributes() {
            let attr = attr?;
            if attr.key.prefix().is_some() && attr.key.local_name().as_ref() == "nil" {
                let value = attr.normalized_value(XmlVersion::Implicit1_0)?;
                return Ok(matches!(value.trim(), "true" | "1"));
            }
        }
        Ok(false)
    }
}

/// The element's name without any namespace prefix.
pub(crate) fn local_name<'s>(start: &'s BytesStart<'_>) -> &'s str {
    let name = start.name().0;
    name.rsplit_once(':').map_or(name, |(_, local)| local)
}

/// Unprefixed attributes of an element, excluding namespace declarations.
pub(crate) fn schema_attributes<'s>(
    start: &'s BytesStart<'_>,
) -> impl Iterator<Item = Result<(&'s str, String), XmlError>> + 's {
    start.attributes().filter_map(|attr| {
        let attr = match attr {
            Ok(attr) => attr,
            Err(err) => return Some(Err(err.into())),
        };
        let key = attr.key.0;
        if key == "xmlns" || key.contains(':') {
            return None;
        }
        Some(
            attr.normalized_value(XmlVersion::Implicit1_0)
                .map(|value| (key, value.into_owned()))
                .map_err(XmlError::from),
        )
    })
}
