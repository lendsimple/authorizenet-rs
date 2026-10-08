//! Turns the raw XSD into Rust-shaped definitions, applying `overrides.toml`.
//!
//! Mapping rules:
//! - A named simple type with enumerations becomes an enum; other simple types become
//!   their built-in base (`String`, `Decimal`, ...).
//! - A complex type whose only content is one repeated element (`ArrayOfLineItem`) is
//!   not generated: fields of that type become a wrapped `Vec` of the item.
//! - A complex type that is only an `xs:choice` becomes an enum of the choice's elements.
//! - An extension keeps its base as a `flatten` field, except for the API base types in
//!   `inline_bases`, whose fields are copied in.
//! - Anonymous types are named after their owner plus the element (`TransactionResponse`
//!   + `prePaidCard` = `TransactionResponsePrePaidCard`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use heck::{ToSnakeCase, ToUpperCamelCase};
use serde::Deserialize;

use crate::xsd::{Choice, ComplexType, Element, ElementType, Max, Particle, Schema, SimpleType};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    #[serde(default)]
    pub skip_elements: Vec<String>,
    #[serde(default)]
    pub inline_bases: Vec<String>,
    pub request_base: String,
    pub request_auth_element: String,
    #[serde(default)]
    pub dedupe: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
    #[serde(default)]
    pub sensitive: BTreeSet<String>,
    #[serde(default)]
    pub types: BTreeMap<String, String>,
    #[serde(default)]
    pub choices: BTreeMap<String, ChoiceNames>,
    #[serde(default)]
    pub retype: BTreeMap<String, String>,
    #[serde(default)]
    pub extra_fields: Vec<ExtraField>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceNames {
    #[serde(rename = "enum")]
    pub enum_name: String,
    pub field: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraField {
    pub owner: String,
    pub after: String,
    pub element: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Default)]
pub struct Model {
    pub enums: Vec<EnumDef>,
    pub choices: Vec<ChoiceDef>,
    pub structs: Vec<StructDef>,
}

#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    /// Where the type comes from, for docs: an XSD type name or `owner/element`.
    pub origin: String,
    pub doc: Option<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug)]
pub struct EnumVariant {
    pub name: String,
    pub value: String,
}

#[derive(Debug)]
pub struct ChoiceDef {
    pub name: String,
    pub origin: String,
    pub doc: Option<String>,
    pub variants: Vec<ChoiceVariant>,
}

#[derive(Debug)]
pub struct ChoiceVariant {
    pub name: String,
    pub xml: String,
    pub ty: RustType,
    pub sensitive: bool,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub origin: String,
    pub doc: Option<String>,
    pub fields: Vec<Field>,
    /// Root element name, for API messages.
    pub root: Option<String>,
    pub request: bool,
    /// Every field can be absent, so the struct implements `Default`.
    pub default: bool,
}

#[derive(Debug)]
pub struct Field {
    /// Rust field name (snake_case; may need `r#`).
    pub name: String,
    /// XML element or attribute name; empty for `flatten` and `choice` fields.
    pub xml: String,
    pub kind: FieldKind,
    pub ty: RustType,
    pub card: Card,
    pub sensitive: bool,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Element,
    /// `keep_empty`: the wrapper element is required, so it is written even when empty.
    Wrapped {
        item: String,
        keep_empty: bool,
    },
    Attribute,
    Flatten,
    Choice,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    String,
    Bool,
    I16,
    I32,
    I64,
    Decimal,
    Date,
    DateTime,
    Raw,
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Card {
    Required,
    Optional,
    List,
}

/// How an element's type maps to Rust.
enum Resolved {
    /// A single value of this type.
    Value(RustType),
    /// A wrapper type: a `Vec` of the item element.
    Wrapped { item: String, item_ty: RustType },
}

pub fn build(schema: &Schema, overrides: &Overrides) -> Result<Model, String> {
    let mut b = Builder::new(schema, overrides)?;
    b.run()?;
    b.finish()
}

struct Builder<'s> {
    schema: &'s Schema,
    ov: &'s Overrides,
    simple: HashMap<&'s str, &'s SimpleType>,
    complex: HashMap<&'s str, &'s ComplexType>,
    /// Named wrapper types: XSD name -> repeated item element.
    wrappers: HashMap<&'s str, &'s Element>,
    model: Model,
    used_types: HashSet<String>,
    used_choices: HashSet<String>,
}

fn local(qname: &str) -> &str {
    qname.split_once(':').map_or(qname, |(_, l)| l)
}

/// `ArrayOfLineItem`-style: no base, no attributes, one repeated element.
fn wrapper_item(ct: &ComplexType) -> Option<&Element> {
    match (
        ct.base.as_ref(),
        ct.attributes.is_empty(),
        ct.content.as_slice(),
    ) {
        (None, true, [Particle::Element(e)]) if e.max.is_many() => Some(e),
        _ => None,
    }
}

/// A type that is nothing but one choice that occurs exactly once.
fn bare_choice(ct: &ComplexType) -> Option<&Choice> {
    match (
        ct.base.as_ref(),
        ct.attributes.is_empty(),
        ct.content.as_slice(),
    ) {
        (None, true, [Particle::Choice(c)]) if c.min == 1 && c.max == Max::Bounded(1) => Some(c),
        _ => None,
    }
}

fn upper(name: &str) -> String {
    name.to_upper_camel_case()
}

impl<'s> Builder<'s> {
    fn new(schema: &'s Schema, ov: &'s Overrides) -> Result<Self, String> {
        let simple = schema
            .simple_types
            .iter()
            .map(|t| (t.name.as_deref().expect("named"), t))
            .collect();
        let complex: HashMap<&str, &ComplexType> = schema
            .complex_types
            .iter()
            .map(|t| (t.name.as_deref().expect("named"), t))
            .collect();
        let bases: HashSet<&str> = schema
            .complex_types
            .iter()
            .filter_map(|t| t.base.as_deref().map(local))
            .collect();
        let wrappers = complex
            .iter()
            .filter(|(name, _)| !bases.contains(*name))
            .filter_map(|(name, ct)| wrapper_item(ct).map(|e| (*name, e)))
            .collect();
        Ok(Self {
            schema,
            ov,
            simple,
            complex,
            wrappers,
            model: Model::default(),
            used_types: HashSet::new(),
            used_choices: HashSet::new(),
        })
    }

    fn run(&mut self) -> Result<(), String> {
        for st in &self.schema.simple_types {
            if !st.enumeration.is_empty() {
                let xsd = st.name.as_deref().expect("named");
                let name = self.type_name(xsd);
                self.push_enum(name, xsd.to_owned(), st)?;
            }
        }
        for ct in &self.schema.complex_types {
            let xsd = ct.name.as_deref().expect("named");
            if self.wrappers.contains_key(xsd) {
                continue;
            }
            if self.ov.inline_bases.iter().any(|b| b == xsd) {
                continue;
            }
            let name = self.type_name(xsd);
            if let Some(choice) = bare_choice(ct) {
                self.push_choice(name, xsd.to_owned(), ct.doc.clone(), choice)?;
            } else {
                self.push_struct(name, xsd.to_owned(), ct, None)?;
            }
        }
        for el in &self.schema.elements {
            if self.ov.skip_elements.contains(&el.name) {
                continue;
            }
            let name = self
                .ov
                .types
                .get(&el.name)
                .cloned()
                .unwrap_or_else(|| upper(&el.name));
            if self.ov.types.contains_key(&el.name) {
                self.used_types.insert(el.name.clone());
            }
            let ct = match &el.ty {
                ElementType::Complex(ct) => (**ct).clone(),
                // `<xs:element name="ErrorResponse" type="anet:ANetApiResponse"/>`
                ElementType::Named(ty) => ComplexType {
                    base: Some(ty.clone()),
                    ..ComplexType::default()
                },
                _ => return Err(format!("root element {} has no complex type", el.name)),
            };
            let mut def = self.build_struct(name, el.name.clone(), &ct, Some(&el.name))?;
            def.doc = el.doc.clone().or(def.doc);
            self.model.structs.push(def);
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Model, String> {
        self.apply_dedupe()?;
        self.apply_optional()?;
        self.apply_retypes()?;
        self.apply_extra_fields()?;
        self.check_overrides_used()?;
        self.check_names()?;
        self.compute_defaults();
        Ok(self.model)
    }

    /// Rust name of a named XSD type.
    fn type_name(&mut self, xsd: &str) -> String {
        if let Some(name) = self.ov.types.get(xsd) {
            self.used_types.insert(xsd.to_owned());
            return name.clone();
        }
        let camel = upper(xsd);
        for suffix in ["Enum", "Type"] {
            if let Some(stripped) = camel.strip_suffix(suffix)
                && !stripped.is_empty()
            {
                return stripped.to_owned();
            }
        }
        camel
    }

    /// Name of an anonymous type: `owner` + element, unless `[types]` renames it.
    fn anonymous_name(&mut self, context: String) -> String {
        match self.ov.types.get(&context) {
            Some(name) => {
                self.used_types.insert(context);
                name.clone()
            }
            None => context,
        }
    }

    fn push_enum(&mut self, name: String, origin: String, st: &SimpleType) -> Result<(), String> {
        if st.base != "xs:string" {
            return Err(format!(
                "{origin}: enumerations on {} are not supported",
                st.base
            ));
        }
        let variants = st
            .enumeration
            .iter()
            .map(|value| EnumVariant {
                name: variant_name(value),
                value: value.clone(),
            })
            .collect();
        self.model.enums.push(EnumDef {
            name,
            origin,
            doc: st.doc.clone(),
            variants,
        });
        Ok(())
    }

    fn push_choice(
        &mut self,
        name: String,
        origin: String,
        doc: Option<String>,
        choice: &Choice,
    ) -> Result<(), String> {
        let mut variants = Vec::new();
        for m in &choice.members {
            let ty = match self.resolve(&name, &m.name, &m.ty)? {
                Resolved::Value(ty) => ty,
                Resolved::Wrapped { .. } => {
                    return Err(format!("{origin}: list member {} in a choice", m.name));
                }
            };
            variants.push(ChoiceVariant {
                name: upper(&m.name),
                xml: m.name.clone(),
                ty,
                sensitive: self.ov.sensitive.contains(&m.name),
            });
        }
        self.model.choices.push(ChoiceDef {
            name,
            origin,
            doc,
            variants,
        });
        Ok(())
    }

    fn push_struct(
        &mut self,
        name: String,
        origin: String,
        ct: &ComplexType,
        root: Option<&str>,
    ) -> Result<(), String> {
        let def = self.build_struct(name, origin, ct, root)?;
        self.model.structs.push(def);
        Ok(())
    }

    fn build_struct(
        &mut self,
        name: String,
        origin: String,
        ct: &ComplexType,
        root: Option<&str>,
    ) -> Result<StructDef, String> {
        let mut fields = Vec::new();
        let mut request = false;
        if let Some(base) = ct.base.as_deref().map(local) {
            if self.ov.inline_bases.iter().any(|b| b == base) {
                let base_ct = *self
                    .complex
                    .get(base)
                    .ok_or_else(|| format!("{origin}: unknown base {base}"))?;
                for particle in &base_ct.content {
                    if base == self.ov.request_base
                        && matches!(particle, Particle::Element(e) if e.name == self.ov.request_auth_element)
                    {
                        request = true;
                        continue;
                    }
                    fields.extend(self.particle_fields(&name, particle)?);
                }
            } else {
                if !self.complex.contains_key(base) {
                    return Err(format!("{origin}: unknown base {base}"));
                }
                let base_name = self.type_name(base);
                fields.push(Field {
                    name: base_name.to_snake_case(),
                    xml: String::new(),
                    kind: FieldKind::Flatten,
                    ty: RustType::Named(base_name),
                    card: Card::Required,
                    sensitive: false,
                    doc: None,
                });
            }
        }
        for particle in &ct.content {
            fields.extend(self.particle_fields(&name, particle)?);
        }
        for attr in &ct.attributes {
            let ty = match self.resolve(&name, &attr.name, &attr.ty)? {
                Resolved::Value(ty) => ty,
                Resolved::Wrapped { .. } => return Err(format!("{origin}: list attribute")),
            };
            fields.push(Field {
                name: attr.name.to_snake_case(),
                xml: attr.name.clone(),
                kind: FieldKind::Attribute,
                ty,
                card: if attr.required {
                    Card::Required
                } else {
                    Card::Optional
                },
                sensitive: false,
                doc: None,
            });
        }
        if request && root.is_none() {
            return Err(format!(
                "{origin}: only root elements can extend the request base"
            ));
        }
        Ok(StructDef {
            name,
            origin,
            doc: ct.doc.clone(),
            fields,
            root: root.map(str::to_owned),
            request,
            default: false,
        })
    }

    fn particle_fields(&mut self, owner: &str, particle: &Particle) -> Result<Vec<Field>, String> {
        match particle {
            Particle::Element(e) => Ok(vec![self.element_field(owner, e)?]),
            Particle::Choice(c) => {
                let (enum_name, field) = match self.ov.choices.get(owner) {
                    Some(names) => {
                        self.used_choices.insert(owner.to_owned());
                        (names.enum_name.clone(), names.field.clone())
                    }
                    None => (format!("{owner}Choice"), "choice".to_owned()),
                };
                let card = if c.max.is_many() {
                    Card::List
                } else if c.min == 0 || c.members.iter().any(|m| m.min == 0) {
                    Card::Optional
                } else {
                    Card::Required
                };
                self.push_choice(enum_name.clone(), format!("{owner} choice"), None, c)?;
                Ok(vec![Field {
                    name: field,
                    xml: String::new(),
                    kind: FieldKind::Choice,
                    ty: RustType::Named(enum_name),
                    card,
                    sensitive: false,
                    doc: None,
                }])
            }
        }
    }

    fn element_field(&mut self, owner: &str, e: &Element) -> Result<Field, String> {
        let (kind, ty, card) = match self.resolve(owner, &e.name, &e.ty)? {
            Resolved::Wrapped { item, item_ty } => {
                if e.max.is_many() {
                    return Err(format!("{owner}.{}: repeated list wrapper", e.name));
                }
                (
                    FieldKind::Wrapped {
                        item,
                        keep_empty: e.min > 0,
                    },
                    item_ty,
                    Card::List,
                )
            }
            Resolved::Value(ty) => {
                let card = if e.max.is_many() {
                    Card::List
                } else if e.min == 0 {
                    Card::Optional
                } else {
                    Card::Required
                };
                (FieldKind::Element, ty, card)
            }
        };
        Ok(Field {
            name: e.name.to_snake_case(),
            xml: e.name.clone(),
            kind,
            ty,
            card,
            sensitive: self.ov.sensitive.contains(&e.name),
            doc: e.doc.clone(),
        })
    }

    /// Maps the type of element `elem` inside `owner`. An anonymous type is named
    /// `owner` + `elem`; the item of an anonymous list wrapper is named `owner` + item.
    fn resolve(&mut self, owner: &str, elem: &str, ty: &ElementType) -> Result<Resolved, String> {
        let context = self.anonymous_name(format!("{owner}{}", upper(elem)));
        match ty {
            ElementType::Any => Ok(Resolved::Value(RustType::Raw)),
            ElementType::Named(q) if q.starts_with("xs:") => Ok(Resolved::Value(builtin(q)?)),
            ElementType::Named(q) => {
                let name = local(q);
                if self.simple.contains_key(name) {
                    return Ok(Resolved::Value(self.named_simple(name)?));
                }
                if let Some(item) = self.wrappers.get(name).copied() {
                    return self.wrapped(owner, item);
                }
                if self.complex.contains_key(name) {
                    return Ok(Resolved::Value(RustType::Named(self.type_name(name))));
                }
                Err(format!("unknown type {q}"))
            }
            ElementType::Simple(st) => {
                if st.enumeration.is_empty() {
                    return Ok(Resolved::Value(self.simple_base(st)?));
                }
                self.push_enum(context.clone(), context.clone(), st)?;
                Ok(Resolved::Value(RustType::Named(context)))
            }
            ElementType::Complex(ct) => {
                if let Some(item) = wrapper_item(ct) {
                    // The wrapper type disappears, so its item is named after the owner.
                    return self.wrapped(owner, item);
                }
                if let Some(choice) = bare_choice(ct) {
                    self.push_choice(context.clone(), context.clone(), ct.doc.clone(), choice)?;
                } else {
                    self.push_struct(context.clone(), context.clone(), ct, None)?;
                }
                Ok(Resolved::Value(RustType::Named(context)))
            }
        }
    }

    fn wrapped(&mut self, owner: &str, item: &Element) -> Result<Resolved, String> {
        match self.resolve(owner, &item.name, &item.ty)? {
            Resolved::Value(item_ty) => Ok(Resolved::Wrapped {
                item: item.name.clone(),
                item_ty,
            }),
            Resolved::Wrapped { .. } => Err(format!("{owner}: nested list wrapper {}", item.name)),
        }
    }

    fn named_simple(&mut self, name: &str) -> Result<RustType, String> {
        let st = *self.simple.get(name).expect("checked");
        if !st.enumeration.is_empty() {
            return Ok(RustType::Named(self.type_name(name)));
        }
        self.simple_base(st)
    }

    fn simple_base(&mut self, st: &SimpleType) -> Result<RustType, String> {
        if st.base.starts_with("xs:") {
            builtin(&st.base)
        } else {
            self.named_simple(local(&st.base))
        }
    }

    fn struct_mut(&mut self, name: &str) -> Option<&mut StructDef> {
        self.model.structs.iter_mut().find(|s| s.name == name)
    }

    fn apply_dedupe(&mut self) -> Result<(), String> {
        for key in &self.ov.dedupe {
            let (owner, element) = key
                .split_once('.')
                .ok_or_else(|| format!("dedupe key {key:?} is not Owner.element"))?;
            let def = self
                .struct_mut(owner)
                .ok_or_else(|| format!("dedupe {key}: no type {owner}"))?;
            let positions: Vec<usize> = def
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.xml == element)
                .map(|(i, _)| i)
                .collect();
            match positions.as_slice() {
                [_, later] => {
                    def.fields.remove(*later);
                }
                _ => {
                    return Err(format!(
                        "dedupe {key}: <{element}> occurs {} times",
                        positions.len()
                    ));
                }
            }
        }
        Ok(())
    }

    fn apply_optional(&mut self) -> Result<(), String> {
        for key in &self.ov.optional {
            let (owner, element) = key
                .split_once('.')
                .ok_or_else(|| format!("optional key {key:?} is not Owner.element"))?;
            let def = self
                .struct_mut(owner)
                .ok_or_else(|| format!("optional {key}: no type {owner}"))?;
            let field = def
                .fields
                .iter_mut()
                .find(|f| f.xml == element)
                .ok_or_else(|| format!("optional {key}: {owner} has no <{element}>"))?;
            if field.card != Card::Required {
                return Err(format!("optional {key}: field is already {:?}", field.card));
            }
            field.card = Card::Optional;
        }
        Ok(())
    }

    fn apply_retypes(&mut self) -> Result<(), String> {
        for (key, enum_xsd) in &self.ov.retype {
            let (owner, element) = key
                .split_once('.')
                .ok_or_else(|| format!("retype key {key:?} is not Owner.element"))?;
            if !self
                .simple
                .get(enum_xsd.as_str())
                .is_some_and(|st| !st.enumeration.is_empty())
            {
                return Err(format!("retype {key}: {enum_xsd} is not an enumeration"));
            }
            let enum_name = self.type_name(enum_xsd);
            let def = self
                .struct_mut(owner)
                .ok_or_else(|| format!("retype {key}: no type {owner}"))?;
            let field = def
                .fields
                .iter_mut()
                .find(|f| f.xml == element)
                .ok_or_else(|| format!("retype {key}: {owner} has no <{element}>"))?;
            if field.ty != RustType::String {
                return Err(format!("retype {key}: field is {:?}, not String", field.ty));
            }
            field.ty = RustType::Named(enum_name);
        }
        Ok(())
    }

    fn apply_extra_fields(&mut self) -> Result<(), String> {
        let known: HashSet<String> = self.all_type_names().into_iter().collect();
        for extra in &self.ov.extra_fields {
            if !known.contains(&extra.ty) {
                return Err(format!(
                    "extra field {}.{}: no type {}",
                    extra.owner, extra.element, extra.ty
                ));
            }
            let def = self
                .model
                .structs
                .iter_mut()
                .find(|s| s.name == extra.owner)
                .ok_or_else(|| format!("extra field: no type {}", extra.owner))?;
            let at = def
                .fields
                .iter()
                .position(|f| f.xml == extra.after)
                .ok_or_else(|| format!("extra field: {} has no <{}>", extra.owner, extra.after))?;
            def.fields.insert(
                at + 1,
                Field {
                    name: extra.element.to_snake_case(),
                    xml: extra.element.clone(),
                    kind: FieldKind::Element,
                    ty: RustType::Named(extra.ty.clone()),
                    card: Card::Optional,
                    sensitive: false,
                    doc: Some("Not in the XSD, but sent by the API.".to_owned()),
                },
            );
        }
        Ok(())
    }

    fn check_overrides_used(&self) -> Result<(), String> {
        let mut stale = Vec::new();
        for key in self.ov.types.keys() {
            if !self.used_types.contains(key) {
                stale.push(format!("types.{key}"));
            }
        }
        for key in self.ov.choices.keys() {
            if !self.used_choices.contains(key) {
                stale.push(format!("choices.{key}"));
            }
        }
        let mut elements = HashSet::new();
        self.collect_xml_names(&mut elements);
        for name in &self.ov.sensitive {
            if !elements.contains(name.as_str()) {
                stale.push(format!("sensitive {name}"));
            }
        }
        for name in &self.ov.skip_elements {
            if !self.schema.elements.iter().any(|e| &e.name == name) {
                stale.push(format!("skip_elements {name}"));
            }
        }
        if stale.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "overrides match nothing in the schema: {}",
                stale.join(", ")
            ))
        }
    }

    fn collect_xml_names<'m>(&'m self, out: &mut HashSet<&'m str>) {
        for s in &self.model.structs {
            out.extend(s.fields.iter().map(|f| f.xml.as_str()));
        }
        for c in &self.model.choices {
            out.extend(c.variants.iter().map(|v| v.xml.as_str()));
        }
    }

    fn all_type_names(&self) -> Vec<String> {
        let m = &self.model;
        m.enums
            .iter()
            .map(|e| e.name.clone())
            .chain(m.choices.iter().map(|c| c.name.clone()))
            .chain(m.structs.iter().map(|s| s.name.clone()))
            .collect()
    }

    fn check_names(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        let mut seen = HashMap::new();
        for name in self.all_type_names() {
            *seen.entry(name).or_insert(0) += 1;
        }
        const RESERVED: &[&str] = &[
            "Decimal",
            "RawXml",
            "XmlDate",
            "XmlDateTime",
            "Option",
            "Vec",
            "String",
            "Result",
            "Box",
        ];
        for (name, count) in &seen {
            if *count > 1 {
                problems.push(format!("type name {name} is generated {count} times"));
            }
            if RESERVED.contains(&name.as_str()) {
                problems.push(format!("type name {name} shadows a built-in"));
            }
        }
        for s in &self.model.structs {
            let mut fields = HashSet::new();
            for f in &s.fields {
                if !fields.insert(&f.name) {
                    problems.push(format!("{} has two fields named {}", s.name, f.name));
                }
                if matches!(f.name.as_str(), "self" | "super" | "crate") {
                    problems.push(format!("{}.{} cannot be a Rust identifier", s.name, f.name));
                }
            }
        }
        for e in &self.model.enums {
            let mut variants = HashSet::from(["Other".to_owned()]);
            for v in &e.variants {
                if !variants.insert(v.name.clone()) {
                    problems.push(format!(
                        "{} has two variants named {} (or one named Other)",
                        e.name, v.name
                    ));
                }
            }
        }
        for c in &self.model.choices {
            let mut variants = HashSet::new();
            for v in &c.variants {
                if !variants.insert(&v.name) {
                    problems.push(format!("{} has two variants named {}", c.name, v.name));
                }
            }
        }
        problems.sort();
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }

    /// A struct is `Default` when every field may be absent. Flattened bases must be
    /// `Default` themselves, so iterate until nothing changes.
    fn compute_defaults(&mut self) {
        loop {
            let defaults: HashSet<String> = self
                .model
                .structs
                .iter()
                .filter(|s| s.default)
                .map(|s| s.name.clone())
                .collect();
            let mut changed = false;
            for s in &mut self.model.structs {
                if s.default {
                    continue;
                }
                let all_optional = s.fields.iter().all(|f| match (&f.kind, f.card) {
                    (FieldKind::Flatten, _) => {
                        matches!(&f.ty, RustType::Named(n) if defaults.contains(n))
                    }
                    (_, Card::Required) => false,
                    _ => true,
                });
                if all_optional {
                    s.default = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
}

fn builtin(qname: &str) -> Result<RustType, String> {
    Ok(match qname {
        "xs:string" => RustType::String,
        "xs:boolean" => RustType::Bool,
        "xs:short" => RustType::I16,
        "xs:int" => RustType::I32,
        "xs:long" | "xs:integer" => RustType::I64,
        "xs:decimal" => RustType::Decimal,
        "xs:date" => RustType::Date,
        "xs:dateTime" => RustType::DateTime,
        other => return Err(format!("unsupported built-in type {other}")),
    })
}

/// A Rust variant name for an enumeration value: `authOnlyTransaction` ->
/// `AuthOnlyTransaction`, `API_Merchant_BasicReporting` -> `ApiMerchantBasicReporting`.
fn variant_name(value: &str) -> String {
    let name = upper(value);
    if name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        name
    } else {
        format!("V{name}")
    }
}
