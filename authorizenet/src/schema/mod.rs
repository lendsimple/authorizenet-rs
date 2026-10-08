//! Authorize.Net schema types.
//!
//! These are hand-written stand-ins that exercise the XML derives. They are replaced
//! by types generated from `AnetApiSchema.xsd` (Stage 2 of `implementation.md`).

use bon::Builder;

use crate::xml::{AnetEnum, AnetXml};

/// `messageTypeEnum`: whether a request succeeded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, AnetEnum)]
#[non_exhaustive]
pub enum MessageType {
    #[anet(value = "Ok")]
    Ok,
    #[anet(value = "Error")]
    Error,
    #[anet(other)]
    Other(String),
}

/// One result message: a code such as `I00001` or `E00035` and its text.
#[derive(Clone, PartialEq, AnetXml, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Message {
    pub code: String,
    pub text: String,
}

/// `messagesType`: the overall result of a request.
#[derive(Clone, PartialEq, AnetXml, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Messages {
    pub result_code: MessageType,
    #[builder(default)]
    pub message: Vec<Message>,
}

/// `merchantAuthenticationType`.
#[derive(Clone, PartialEq, AnetXml, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct MerchantAuthentication {
    pub name: Option<String>,
    #[anet(choice)]
    pub credential: Option<MerchantCredential>,
    pub mobile_device_id: Option<String>,
}

/// The credential choice inside `merchantAuthenticationType`.
#[derive(Clone, PartialEq, AnetXml)]
#[non_exhaustive]
pub enum MerchantCredential {
    #[anet(sensitive)]
    TransactionKey(String),
    #[anet(sensitive)]
    SessionToken(String),
    #[anet(sensitive)]
    Password(String),
    ClientKey(String),
    #[anet(sensitive)]
    AccessToken(String),
}

/// `nameAndAddressType`.
#[derive(Clone, PartialEq, Default, AnetXml, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct NameAndAddress {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip: Option<String>,
    pub country: Option<String>,
}

/// `customerAddressType`, an extension of `nameAndAddressType`.
#[derive(Clone, PartialEq, Default, AnetXml, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct CustomerAddress {
    #[anet(flatten)]
    pub name_and_address: NameAndAddress,
    pub phone_number: Option<String>,
    pub fax_number: Option<String>,
    pub email: Option<String>,
}

/// `ANetApiResponse`: the fields every response starts with.
#[derive(Clone, PartialEq, AnetXml, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct AnetApiResponse {
    pub ref_id: Option<String>,
    pub messages: Messages,
    pub session_token: Option<String>,
}

/// `authenticateTestRequest`: checks that the credentials are valid.
#[derive(Clone, PartialEq, Default, AnetXml, Builder)]
#[builder(on(String, into))]
#[anet(root = "authenticateTestRequest", request)]
#[non_exhaustive]
pub struct AuthenticateTestRequest {
    pub client_id: Option<String>,
    pub ref_id: Option<String>,
}

/// `authenticateTestResponse`.
#[derive(Clone, PartialEq, AnetXml, Builder)]
#[anet(root = "authenticateTestResponse")]
#[non_exhaustive]
pub struct AuthenticateTestResponse {
    #[anet(flatten)]
    pub base: AnetApiResponse,
}

/// `ErrorResponse`: sent instead of the expected response when a request is rejected
/// before it is processed.
#[derive(Clone, PartialEq, AnetXml, Builder)]
#[anet(root = "ErrorResponse")]
#[non_exhaustive]
pub struct ErrorResponse {
    #[anet(flatten)]
    pub base: AnetApiResponse,
}
