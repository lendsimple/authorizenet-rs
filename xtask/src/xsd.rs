//! A raw model of the XSD constructs that `AnetApiSchema.xsd` uses.
//!
//! Anything outside that subset is an error rather than silently ignored, so a schema
//! update that needs new codegen support fails loudly.

use std::fmt;

use roxmltree::{Document, Node};

const XS: &str = "http://www.w3.org/2001/XMLSchema";

#[derive(Debug, Default)]
pub struct Schema {
    pub simple_types: Vec<SimpleType>,
    pub complex_types: Vec<ComplexType>,
    pub elements: Vec<TopElement>,
}

#[derive(Debug, Clone, Default)]
pub struct SimpleType {
    pub name: Option<String>,
    /// Base type as written, e.g. `xs:string` or `anet:numericString`.
    pub base: String,
    pub facets: Facets,
    pub enumeration: Vec<String>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facets {
    pub length: Option<u32>,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    pub pattern: Option<String>,
    pub min_inclusive: Option<String>,
    pub max_inclusive: Option<String>,
    pub total_digits: Option<u32>,
    pub fraction_digits: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ComplexType {
    pub name: Option<String>,
    /// `xs:complexContent/xs:extension/@base`, as written.
    pub base: Option<String>,
    pub content: Vec<Particle>,
    pub attributes: Vec<Attribute>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Particle {
    Element(Element),
    Choice(Choice),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    pub ty: ElementType,
    pub min: u32,
    pub max: Max,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ElementType {
    /// A reference such as `xs:string` or `anet:paymentType`.
    Named(String),
    Simple(Box<SimpleType>),
    Complex(Box<ComplexType>),
    /// No type given: `xs:anyType`.
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Max {
    Bounded(u32),
    Unbounded,
}

impl Max {
    pub fn is_many(self) -> bool {
        self != Max::Bounded(1)
    }
}

#[derive(Debug, Clone)]
pub struct Choice {
    pub min: u32,
    pub max: Max,
    pub members: Vec<Element>,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub ty: ElementType,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct TopElement {
    pub name: String,
    pub ty: ElementType,
    /// Text of the XML comment just before the element, which describes API methods.
    pub doc: Option<String>,
}

#[derive(Debug)]
pub struct XsdError(String);

impl fmt::Display for XsdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for XsdError {}

type Result<T> = std::result::Result<T, XsdError>;

fn error(node: Node, msg: impl fmt::Display) -> XsdError {
    let pos = node.document().text_pos_at(node.range().start);
    XsdError(format!("XSD line {}: {msg}", pos.row))
}

pub fn parse(text: &str) -> std::result::Result<Schema, Box<dyn std::error::Error>> {
    let doc = Document::parse(text)?;
    let root = doc.root_element();
    let mut schema = Schema::default();
    for node in xs_children(root) {
        match node.tag_name().name() {
            "simpleType" => schema.simple_types.push(simple_type(node)?),
            "complexType" => schema.complex_types.push(complex_type(node)?),
            "element" => schema.elements.push(TopElement {
                name: required_attr(node, "name")?.to_owned(),
                ty: element_type(node)?,
                doc: preceding_comment(node),
            }),
            other => return Err(error(node, format!("unsupported top-level <xs:{other}>")).into()),
        }
    }
    Ok(schema)
}

/// Child elements in the XSD namespace, skipping comments, text and annotations.
fn xs_children<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(|n| {
        n.is_element()
            && n.tag_name().namespace() == Some(XS)
            && n.tag_name().name() != "annotation"
    })
}

fn required_attr<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str> {
    node.attribute(name)
        .ok_or_else(|| error(node, format!("missing @{name}")))
}

fn documentation(node: Node) -> Option<String> {
    let annotation = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "annotation")?;
    let text: Vec<String> = annotation
        .children()
        .filter(|n| n.tag_name().name() == "documentation")
        .filter_map(|n| n.text())
        .map(clean_doc)
        .filter(|s| !s.is_empty())
        .collect();
    (!text.is_empty()).then(|| text.join("\n\n"))
}

/// The comment immediately preceding `node` (ignoring whitespace), minus separator lines
/// and a first line that only repeats the element name.
fn preceding_comment(node: Node) -> Option<String> {
    let mut prev = node.prev_sibling();
    while let Some(p) = prev {
        if p.is_text() && p.text().is_some_and(|t| t.trim().is_empty()) {
            prev = p.prev_sibling();
            continue;
        }
        break;
    }
    let comment = prev.filter(|p| p.is_comment())?.text()?;
    let name = node.attribute("name").unwrap_or_default();
    let lines: Vec<&str> = comment
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.chars().all(|c| matches!(c, '=' | 'x' | '-')))
        .filter(|l| *l != name)
        .collect();
    (!lines.is_empty()).then(|| clean_doc(&lines.join("\n")))
}

fn clean_doc(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn simple_type(node: Node) -> Result<SimpleType> {
    let mut out = SimpleType {
        name: node.attribute("name").map(str::to_owned),
        doc: documentation(node),
        ..SimpleType::default()
    };
    let restriction = xs_children(node)
        .next()
        .filter(|n| n.tag_name().name() == "restriction")
        .ok_or_else(|| error(node, "only <xs:restriction> simple types are supported"))?;
    out.base = required_attr(restriction, "base")?.to_owned();
    for facet in xs_children(restriction) {
        let value = required_attr(facet, "value")?;
        let number = || {
            value
                .parse::<u32>()
                .map_err(|_| error(facet, format!("expected a number, found {value:?}")))
        };
        let f = &mut out.facets;
        match facet.tag_name().name() {
            "enumeration" => out.enumeration.push(value.to_owned()),
            "length" => f.length = Some(number()?),
            "minLength" => f.min_length = Some(number()?),
            "maxLength" => f.max_length = Some(number()?),
            "pattern" => f.pattern = Some(value.to_owned()),
            "minInclusive" => f.min_inclusive = Some(value.to_owned()),
            "maxInclusive" => f.max_inclusive = Some(value.to_owned()),
            "totalDigits" => f.total_digits = Some(number()?),
            "fractionDigits" => f.fraction_digits = Some(number()?),
            other => return Err(error(facet, format!("unsupported facet <xs:{other}>"))),
        }
    }
    Ok(out)
}

fn complex_type(node: Node) -> Result<ComplexType> {
    let mut out = ComplexType {
        name: node.attribute("name").map(str::to_owned),
        doc: documentation(node),
        ..ComplexType::default()
    };
    for child in xs_children(node) {
        match child.tag_name().name() {
            "sequence" => out.content.extend(sequence(child)?),
            "choice" => out.content.push(Particle::Choice(choice(child)?)),
            "attribute" => out.attributes.push(attribute(child)?),
            "complexContent" => {
                let ext = xs_children(child)
                    .next()
                    .filter(|n| n.tag_name().name() == "extension")
                    .ok_or_else(|| {
                        error(child, "only <xs:extension> complex content is supported")
                    })?;
                out.base = Some(required_attr(ext, "base")?.to_owned());
                for part in xs_children(ext) {
                    match part.tag_name().name() {
                        "sequence" => out.content.extend(sequence(part)?),
                        "choice" => out.content.push(Particle::Choice(choice(part)?)),
                        "attribute" => out.attributes.push(attribute(part)?),
                        other => {
                            return Err(error(
                                part,
                                format!("unsupported <xs:{other}> in extension"),
                            ));
                        }
                    }
                }
            }
            other => {
                return Err(error(
                    child,
                    format!("unsupported <xs:{other}> in complexType"),
                ));
            }
        }
    }
    Ok(out)
}

fn occurs(node: Node) -> Result<(u32, Max)> {
    let min = match node.attribute("minOccurs") {
        Some(v) => v.parse().map_err(|_| error(node, "bad minOccurs"))?,
        None => 1,
    };
    let max = match node.attribute("maxOccurs") {
        None => Max::Bounded(1),
        Some("unbounded") => Max::Unbounded,
        Some(v) => Max::Bounded(v.parse().map_err(|_| error(node, "bad maxOccurs"))?),
    };
    Ok((min, max))
}

/// Elements and choices of a sequence. Nested sequences that occur exactly once are spliced in.
fn sequence(node: Node) -> Result<Vec<Particle>> {
    let mut out = Vec::new();
    for child in xs_children(node) {
        match child.tag_name().name() {
            "element" => out.push(Particle::Element(element(child)?)),
            "choice" => out.push(Particle::Choice(choice(child)?)),
            "sequence" if occurs(child)? == (1, Max::Bounded(1)) => out.extend(sequence(child)?),
            other => {
                return Err(error(
                    child,
                    format!("unsupported <xs:{other}> in sequence"),
                ));
            }
        }
    }
    Ok(out)
}

fn choice(node: Node) -> Result<Choice> {
    let (min, max) = occurs(node)?;
    let members = xs_children(node)
        .map(|child| match child.tag_name().name() {
            "element" => element(child),
            other => Err(error(child, format!("unsupported <xs:{other}> in choice"))),
        })
        .collect::<Result<_>>()?;
    Ok(Choice { min, max, members })
}

fn element(node: Node) -> Result<Element> {
    if node.has_attribute("ref") {
        return Err(error(node, "element references are not supported"));
    }
    let (min, max) = occurs(node)?;
    Ok(Element {
        name: required_attr(node, "name")?.to_owned(),
        ty: element_type(node)?,
        min,
        max,
        doc: documentation(node),
    })
}

fn element_type(node: Node) -> Result<ElementType> {
    if let Some(ty) = node.attribute("type") {
        return Ok(ElementType::Named(ty.to_owned()));
    }
    match xs_children(node).next() {
        None => Ok(ElementType::Any),
        Some(child) => match child.tag_name().name() {
            "simpleType" => Ok(ElementType::Simple(Box::new(simple_type(child)?))),
            "complexType" => Ok(ElementType::Complex(Box::new(complex_type(child)?))),
            other => Err(error(child, format!("unsupported <xs:{other}> in element"))),
        },
    }
}

fn attribute(node: Node) -> Result<Attribute> {
    Ok(Attribute {
        name: required_attr(node, "name")?.to_owned(),
        ty: element_type(node)?,
        required: node.attribute("use") == Some("required"),
    })
}
