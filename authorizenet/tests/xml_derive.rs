//! Behavior of the `AnetXml` / `AnetEnum` derives, using small test-only types.

use authorizenet::Decimal;
use authorizenet::types::RawXml;
use authorizenet::xml::{self, AnetEnum, AnetXml, XmlError};
use pretty_assertions::assert_eq;

#[derive(Debug, Clone, PartialEq, AnetEnum)]
enum Color {
    #[anet(value = "red")]
    Red,
    #[anet(value = "deepBlue")]
    DeepBlue,
    #[anet(other)]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, AnetEnum)]
enum Closed {
    #[anet(value = "a")]
    A,
}

#[derive(Clone, PartialEq, Default, AnetXml)]
struct Base {
    first: Option<String>,
    second: Option<String>,
}

#[derive(Clone, PartialEq, AnetXml)]
enum Pick {
    CreditCard(String),
    #[anet(rename = "bankAcct")]
    BankAccount(String),
}

#[derive(Clone, PartialEq, Default, AnetXml)]
#[anet(root = "doc")]
struct Doc {
    #[anet(attribute)]
    version: Option<i32>,
    #[anet(flatten)]
    base: Base,
    own_field: Option<String>,
    amount: Option<Decimal>,
    #[anet(rename = "refTransID")]
    ref_trans_id: Option<String>,
    color: Option<Color>,
    #[anet(choice)]
    pick: Option<Pick>,
    #[anet(wrapper, item = "item")]
    items: Vec<i32>,
    #[anet(wrapper, item = "tag")]
    tags: Option<Vec<String>>,
    repeated: Vec<String>,
    #[anet(sensitive)]
    secret: Option<String>,
    raw: Option<RawXml>,
}

#[derive(Clone, PartialEq, AnetXml)]
#[anet(root = "needs")]
struct Needs {
    required: String,
    #[anet(choice)]
    pick: Pick,
}

const PROLOG: &str = r#"<?xml version="1.0" encoding="utf-8"?>"#;

fn doc(body: &str) -> String {
    format!(r#"{PROLOG}<doc xmlns="AnetApi/xml/v1/schema/AnetApiSchema.xsd">{body}</doc>"#)
}

#[test]
fn empty_document_writes_root_with_namespace_only() {
    assert_eq!(xml::to_string(&Doc::default()).unwrap(), doc(""));
}

#[test]
fn flattened_base_fields_are_written_before_own_fields() {
    let value = Doc {
        base: Base {
            first: Some("1".into()),
            second: Some("2".into()),
        },
        own_field: Some("3".into()),
        ..Doc::default()
    };
    assert_eq!(
        xml::to_string(&value).unwrap(),
        doc("<first>1</first><second>2</second><ownField>3</ownField>")
    );
}

#[test]
fn choice_variant_is_written_as_sibling_element() {
    let value = Doc {
        own_field: Some("x".into()),
        pick: Some(Pick::BankAccount("123".into())),
        ..Doc::default()
    };
    assert_eq!(
        xml::to_string(&value).unwrap(),
        doc("<ownField>x</ownField><bankAcct>123</bankAcct>")
    );
}

#[test]
fn choice_variant_default_name_is_lower_camel() {
    let value: Doc = xml::from_str(&doc("<creditCard>4111</creditCard>")).unwrap();
    assert_eq!(value.pick, Some(Pick::CreditCard("4111".into())));
}

#[test]
fn namespace_is_declared_on_root_only() {
    let value = Doc {
        base: Base {
            first: Some("1".into()),
            ..Base::default()
        },
        ..Doc::default()
    };
    let out = xml::to_string(&value).unwrap();
    assert_eq!(out.matches("xmlns").count(), 1);
}

#[test]
fn attribute_is_written_on_start_tag() {
    let value = Doc {
        version: Some(2),
        ..Doc::default()
    };
    assert_eq!(
        xml::to_string(&value).unwrap(),
        format!(
            r#"{PROLOG}<doc xmlns="AnetApi/xml/v1/schema/AnetApiSchema.xsd" version="2"></doc>"#
        )
    );
}

#[test]
fn attribute_is_read() {
    let input = r#"<doc xmlns="AnetApi/xml/v1/schema/AnetApiSchema.xsd" version="7"/>"#;
    assert_eq!(xml::from_str::<Doc>(input).unwrap().version, Some(7));
}

#[test]
fn rename_sets_element_name() {
    let value = Doc {
        ref_trans_id: Some("9".into()),
        ..Doc::default()
    };
    assert_eq!(
        xml::to_string(&value).unwrap(),
        doc("<refTransID>9</refTransID>")
    );
}

#[test]
fn wrapped_list_round_trips() {
    let value = Doc {
        items: vec![1, 2],
        ..Doc::default()
    };
    let out = xml::to_string(&value).unwrap();
    assert_eq!(out, doc("<items><item>1</item><item>2</item></items>"));
    assert_eq!(xml::from_str::<Doc>(&out).unwrap(), value);
}

#[test]
fn empty_wrapped_vec_is_omitted() {
    let out = xml::to_string(&Doc::default()).unwrap();
    assert!(!out.contains("<items"));
}

#[test]
fn optional_wrapped_list_distinguishes_empty_from_absent() {
    let value = Doc {
        tags: Some(Vec::new()),
        ..Doc::default()
    };
    let out = xml::to_string(&value).unwrap();
    assert_eq!(out, doc("<tags></tags>"));
    assert_eq!(xml::from_str::<Doc>(&out).unwrap().tags, Some(Vec::new()));
    assert_eq!(xml::from_str::<Doc>(&doc("")).unwrap().tags, None);
}

#[test]
fn repeated_elements_without_wrapper() {
    let value: Doc = xml::from_str(&doc("<repeated>a</repeated><repeated>b</repeated>")).unwrap();
    assert_eq!(value.repeated, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn children_are_accepted_in_any_order() {
    let value: Doc = xml::from_str(&doc(
        "<ownField>3</ownField><second>2</second><first>1</first>",
    ))
    .unwrap();
    assert_eq!(value.base.first.as_deref(), Some("1"));
    assert_eq!(value.base.second.as_deref(), Some("2"));
    assert_eq!(value.own_field.as_deref(), Some("3"));
}

#[test]
fn empty_element_is_empty_string_for_string_fields() {
    let value: Doc = xml::from_str(&doc("<ownField/>")).unwrap();
    assert_eq!(value.own_field, Some(String::new()));
}

#[test]
fn empty_element_is_none_for_decimal_fields() {
    let value: Doc = xml::from_str(&doc("<amount/>")).unwrap();
    assert_eq!(value.amount, None);
}

#[test]
fn decimal_keeps_its_scale() {
    let value: Doc = xml::from_str(&doc("<amount>45.00</amount>")).unwrap();
    assert_eq!(
        xml::to_string(&value).unwrap(),
        doc("<amount>45.00</amount>")
    );
}

#[test]
fn nil_element_is_none() {
    let input =
        doc(r#"<ownField xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:nil="true"/>"#);
    assert_eq!(xml::from_str::<Doc>(&input).unwrap().own_field, None);
}

#[test]
fn string_text_is_not_trimmed() {
    let value: Doc = xml::from_str(&doc("<ownField> padded </ownField>")).unwrap();
    assert_eq!(value.own_field.as_deref(), Some(" padded "));
}

#[test]
fn entity_references_are_resolved() {
    let value: Doc = xml::from_str(&doc("<ownField>a &amp; b &#x41;&#66;</ownField>")).unwrap();
    assert_eq!(value.own_field.as_deref(), Some("a & b AB"));
}

#[test]
fn text_is_escaped_when_written() {
    let value = Doc {
        own_field: Some("<a & b>".into()),
        ..Doc::default()
    };
    let out = xml::to_string(&value).unwrap();
    assert_eq!(out, doc("<ownField>&lt;a &amp; b&gt;</ownField>"));
    assert_eq!(xml::from_str::<Doc>(&out).unwrap(), value);
}

#[test]
fn enum_values_round_trip() {
    let value: Doc = xml::from_str(&doc("<color>deepBlue</color>")).unwrap();
    assert_eq!(value.color, Some(Color::DeepBlue));
    assert_eq!(
        xml::to_string(&value).unwrap(),
        doc("<color>deepBlue</color>")
    );
}

#[test]
fn unlisted_enum_value_is_kept_in_other() {
    let value: Doc = xml::from_str(&doc("<color>green</color>")).unwrap();
    assert_eq!(value.color, Some(Color::Other("green".into())));
    assert_eq!(xml::to_string(&value).unwrap(), doc("<color>green</color>"));
}

#[test]
fn enum_without_other_rejects_unlisted_value() {
    use authorizenet::xml::XmlScalar;
    assert!(matches!(
        Closed::from_xml_text("b"),
        Err(XmlError::InvalidValue { ty: "Closed", .. })
    ));
}

#[test]
fn enum_display_is_xml_value() {
    assert_eq!(Color::DeepBlue.to_string(), "deepBlue");
    assert_eq!(Color::Red.as_str(), "red");
}

#[test]
fn raw_xml_keeps_inner_markup() {
    let value: Doc = xml::from_str(&doc("<raw><a x=\"1\">t</a><b/></raw>")).unwrap();
    assert_eq!(value.raw, Some(RawXml("<a x=\"1\">t</a><b/>".into())));
    assert_eq!(
        xml::to_string(&value).unwrap(),
        doc("<raw><a x=\"1\">t</a><b/></raw>")
    );
}

#[test]
fn unknown_elements_are_skipped_in_lenient_mode() {
    let input = doc("<newField><nested>x</nested></newField><ownField>1</ownField>");
    let value: Doc = xml::from_str(&input).unwrap();
    assert_eq!(value.own_field.as_deref(), Some("1"));
}

#[test]
fn unknown_elements_are_errors_in_strict_mode() {
    let input = doc("<newField/>");
    let err = xml::from_slice_strict::<Doc>(input.as_bytes()).unwrap_err();
    assert!(matches!(
        err,
        XmlError::UnknownElement { ty: "Doc", ref name } if name == "newField"
    ));
}

#[test]
fn duplicate_single_element_is_error_in_strict_mode() {
    let input = doc("<ownField>1</ownField><ownField>2</ownField>");
    assert!(matches!(
        xml::from_slice_strict::<Doc>(input.as_bytes()),
        Err(XmlError::DuplicateElement { .. })
    ));
    assert_eq!(
        xml::from_str::<Doc>(&input).unwrap().own_field.as_deref(),
        Some("2")
    );
}

#[test]
fn stray_text_is_error_in_strict_mode() {
    let input = doc("stray");
    assert!(matches!(
        xml::from_slice_strict::<Doc>(input.as_bytes()),
        Err(XmlError::UnexpectedText { ty: "Doc" })
    ));
    assert!(xml::from_str::<Doc>(&input).is_ok());
}

#[test]
fn missing_required_element_is_error() {
    let input = r#"<needs><creditCard>1</creditCard></needs>"#;
    assert!(matches!(
        xml::from_str::<Needs>(input),
        Err(XmlError::MissingField {
            ty: "Needs",
            field: "required"
        })
    ));
}

#[test]
fn missing_required_choice_is_error() {
    let input = r#"<needs><required>x</required></needs>"#;
    assert!(matches!(
        xml::from_str::<Needs>(input),
        Err(XmlError::MissingField {
            ty: "Needs",
            field: "pick"
        })
    ));
}

#[test]
fn invalid_scalar_value_is_error() {
    assert!(matches!(
        xml::from_str::<Doc>(&doc("<amount>abc</amount>")),
        Err(XmlError::InvalidValue { ty: "decimal", .. })
    ));
}

#[test]
fn child_inside_scalar_is_error() {
    assert!(matches!(
        xml::from_str::<Doc>(&doc("<ownField><x/></ownField>")),
        Err(XmlError::UnexpectedChild { .. })
    ));
}

#[test]
fn wrong_root_is_error() {
    assert!(matches!(
        xml::from_str::<Doc>("<other/>"),
        Err(XmlError::UnexpectedRoot {
            expected: "doc",
            ..
        })
    ));
}

#[test]
fn namespace_prefixes_are_ignored() {
    let input = r#"<a:doc xmlns:a="AnetApi/xml/v1/schema/AnetApiSchema.xsd"><a:ownField>1</a:ownField></a:doc>"#;
    assert_eq!(
        xml::from_str::<Doc>(input).unwrap().own_field.as_deref(),
        Some("1")
    );
}

#[test]
fn byte_order_mark_is_ignored() {
    let mut input = b"\xEF\xBB\xBF".to_vec();
    input.extend_from_slice(doc("<ownField>1</ownField>").as_bytes());
    let value: Doc = xml::from_slice(&input).unwrap();
    assert_eq!(value.own_field.as_deref(), Some("1"));
}

#[test]
fn root_name_reads_first_element() {
    let input = format!("\u{FEFF}{PROLOG}<!-- c --><ErrorResponse/>");
    assert_eq!(xml::root_name(input.as_bytes()).unwrap(), "ErrorResponse");
}

#[test]
fn malformed_xml_is_syntax_error() {
    assert!(matches!(
        xml::from_str::<Doc>("<doc><ownField></doc>"),
        Err(XmlError::Syntax(_))
    ));
}

#[test]
fn debug_redacts_sensitive_fields() {
    let value = Doc {
        secret: Some("hunter2".into()),
        own_field: Some("visible".into()),
        ..Doc::default()
    };
    let debug = format!("{value:?}");
    assert!(!debug.contains("hunter2"), "{debug}");
    assert!(debug.contains("secret: Some([REDACTED])"), "{debug}");
    assert!(debug.contains("visible"), "{debug}");
}

#[test]
fn pretty_output_parses_back() {
    let value = Doc {
        base: Base {
            first: Some("1".into()),
            ..Base::default()
        },
        items: vec![1],
        ..Doc::default()
    };
    let pretty = xml::to_string_pretty(&value).unwrap();
    assert!(pretty.contains('\n'));
    assert_eq!(
        xml::from_slice_strict::<Doc>(pretty.as_bytes()).unwrap(),
        value
    );
}
