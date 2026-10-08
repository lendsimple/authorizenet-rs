use std::borrow::Cow;
use std::io::Write;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};

use super::XmlError;

/// Attributes collected for an element's start tag.
pub type AttrList<'a> = Vec<(&'static str, Cow<'a, str>)>;

/// A writer that emits elements in the order they are written.
pub struct XmlWriter<W: Write> {
    inner: quick_xml::Writer<W>,
}

impl<W: Write> XmlWriter<W> {
    /// A writer with no indentation, as sent to the API.
    pub fn new(output: W) -> Self {
        Self {
            inner: quick_xml::Writer::new(output),
        }
    }

    /// A writer that indents nested elements by four spaces.
    pub fn pretty(output: W) -> Self {
        Self {
            inner: quick_xml::Writer::new_with_indent(output, b' ', 4),
        }
    }

    /// Returns the underlying output.
    pub fn into_inner(self) -> W {
        self.inner.into_inner()
    }

    pub(crate) fn declaration(&mut self) -> Result<(), XmlError> {
        let decl = BytesDecl::new("1.0", Some("utf-8"), None);
        self.inner.write_event(Event::Decl(decl))?;
        Ok(())
    }

    /// Writes a start tag with the given attributes.
    pub fn start(&mut self, tag: &str, attrs: &[(&str, Cow<'_, str>)]) -> Result<(), XmlError> {
        let mut start = BytesStart::new(tag);
        for (key, value) in attrs {
            start.push_attribute((*key, value.as_ref()));
        }
        self.inner.write_event(Event::Start(start))?;
        Ok(())
    }

    /// Writes an end tag.
    pub fn end(&mut self, tag: &str) -> Result<(), XmlError> {
        self.inner.write_event(Event::End(BytesEnd::new(tag)))?;
        Ok(())
    }

    /// Writes `<tag>text</tag>`, escaping the text.
    pub fn text_element(&mut self, tag: &str, text: &str) -> Result<(), XmlError> {
        self.inner
            .create_element(tag)
            .write_text_content(BytesText::new(text))?;
        Ok(())
    }

    /// Writes `<tag>raw</tag>` without escaping `raw`, which must be well-formed markup.
    pub fn raw_element(&mut self, tag: &str, raw: &str) -> Result<(), XmlError> {
        self.start(tag, &[])?;
        self.inner.get_mut().write_all(raw.as_bytes())?;
        self.end(tag)
    }
}
