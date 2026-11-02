//! The JSON body of a notification, as typed values.

use rust_decimal::Decimal;
use serde::Deserialize;
use serde::de::{self, DeserializeOwned, Deserializer};
use serde_json::{Map, Value};
use time::OffsetDateTime;

use crate::schema::{ArbSubscriptionStatus, CustomerType, TransactionOutcome};
use crate::xml::{AnetEnum, XmlScalar};

/// Reads the JSON body of a notification.
///
/// This does not check where the notification came from: verify its signature first,
/// or use [`verify_then_parse`](super::verify_then_parse), which does both.
pub fn parse(body: &[u8]) -> Result<Notification, ParseError> {
    serde_json::from_slice(body).map_err(|source| ParseError { source })
}

/// A body that is not a notification: invalid JSON, or a required field missing or of
/// the wrong type.
#[derive(Debug, thiserror::Error)]
#[error("invalid webhook notification: {source}")]
pub struct ParseError {
    source: serde_json::Error,
}

/// A webhook notification.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Notification {
    /// Unique to the notification; a retried notification keeps its ID.
    pub notification_id: String,
    #[serde(deserialize_with = "scalar")]
    pub event_type: EventType,
    #[serde(deserialize_with = "time::serde::rfc3339::deserialize")]
    pub event_date: OffsetDateTime,
    /// The webhook (subscription to events) the notification was sent for.
    pub webhook_id: String,
    pub payload: Payload,
}

/// What happened, from `eventType`.
///
/// An event Authorize.Net adds after this release parses as `Other`. A later release
/// may give it a variant of its own, so match the string of an event this enum does
/// not list with a guard on [`as_str`](Self::as_str), not with `Other("…")`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, AnetEnum)]
#[non_exhaustive]
pub enum EventType {
    #[anet(value = "net.authorize.payment.authorization.created")]
    AuthorizationCreated,
    #[anet(value = "net.authorize.payment.authcapture.created")]
    AuthCaptureCreated,
    #[anet(value = "net.authorize.payment.capture.created")]
    CaptureCreated,
    #[anet(value = "net.authorize.payment.refund.created")]
    RefundCreated,
    #[anet(value = "net.authorize.payment.priorAuthCapture.created")]
    PriorAuthCaptureCreated,
    #[anet(value = "net.authorize.payment.void.created")]
    VoidCreated,
    #[anet(value = "net.authorize.payment.fraud.held")]
    FraudHeld,
    #[anet(value = "net.authorize.payment.fraud.approved")]
    FraudApproved,
    #[anet(value = "net.authorize.payment.fraud.declined")]
    FraudDeclined,
    #[anet(value = "net.authorize.customer.created")]
    CustomerCreated,
    #[anet(value = "net.authorize.customer.updated")]
    CustomerUpdated,
    #[anet(value = "net.authorize.customer.deleted")]
    CustomerDeleted,
    #[anet(value = "net.authorize.customer.paymentProfile.created")]
    PaymentProfileCreated,
    #[anet(value = "net.authorize.customer.paymentProfile.updated")]
    PaymentProfileUpdated,
    #[anet(value = "net.authorize.customer.paymentProfile.deleted")]
    PaymentProfileDeleted,
    #[anet(value = "net.authorize.customer.subscription.created")]
    SubscriptionCreated,
    #[anet(value = "net.authorize.customer.subscription.updated")]
    SubscriptionUpdated,
    #[anet(value = "net.authorize.customer.subscription.suspended")]
    SubscriptionSuspended,
    #[anet(value = "net.authorize.customer.subscription.terminated")]
    SubscriptionTerminated,
    #[anet(value = "net.authorize.customer.subscription.cancelled")]
    SubscriptionCancelled,
    #[anet(value = "net.authorize.customer.subscription.expiring")]
    SubscriptionExpiring,
    #[anet(value = "net.authorize.customer.subscription.expired")]
    SubscriptionExpired,
    #[anet(value = "net.authorize.customer.subscription.failed")]
    SubscriptionFailed,
    #[anet(value = "net.authorize.partner.boarding.gateway.created")]
    BoardingGatewayCreated,
    #[anet(value = "net.authorize.partner.boarding.rejected")]
    BoardingRejected,
    #[anet(value = "net.authorize.partner.boarding.incomplete")]
    BoardingIncomplete,
    #[anet(value = "net.authorize.partner.boarding.merchantAccount.created")]
    BoardingMerchantAccountCreated,
    #[anet(value = "net.authorize.partner.boarding.merchantAccount.rejected")]
    BoardingMerchantAccountRejected,
    #[anet(value = "net.authorize.partner.boarding.merchantAccount.updated")]
    BoardingMerchantAccountUpdated,
    /// An event this release does not list.
    #[anet(other)]
    Other(String),
}

/// What the notification is about, by its `entityName`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Payload {
    /// `transaction`: payment and fraud events.
    Transaction(TransactionPayload),
    /// `customerProfile`: customer events.
    CustomerProfile(CustomerProfilePayload),
    /// `customerPaymentProfile`: payment profile events.
    CustomerPaymentProfile(CustomerPaymentProfilePayload),
    /// `subscription`: subscription events.
    Subscription(SubscriptionPayload),
    /// Any other entity, such as the `boardingApplication` of partner boarding events,
    /// as its JSON object (`entityName` included).
    Other(Map<String, Value>),
}

impl<'de> Deserialize<'de> for Payload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = Map::deserialize(deserializer)?;
        let entity_name = fields
            .get("entityName")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(match entity_name.as_deref() {
            Some("transaction") => Payload::Transaction(typed(fields)?),
            Some("customerProfile") => Payload::CustomerProfile(typed(fields)?),
            Some("customerPaymentProfile") => Payload::CustomerPaymentProfile(typed(fields)?),
            Some("subscription") => Payload::Subscription(typed(fields)?),
            _ => Payload::Other(fields),
        })
    }
}

fn typed<T: DeserializeOwned, E: de::Error>(fields: Map<String, Value>) -> Result<T, E> {
    serde_json::from_value(Value::Object(fields)).map_err(E::custom)
}

/// The transaction of a payment or fraud event.
///
/// Amounts keep their value but not always their trailing zeros: `45.00` in the JSON
/// reads as `45`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TransactionPayload {
    /// The transaction ID (`transId` in the API).
    #[serde(deserialize_with = "id")]
    pub id: String,
    #[serde(default, deserialize_with = "response_code")]
    pub response_code: Option<TransactionOutcome>,
    pub auth_code: Option<String>,
    pub avs_response: Option<String>,
    pub auth_amount: Option<Decimal>,
    /// The `refId` of the request that created the transaction.
    #[serde(default, deserialize_with = "option_id")]
    pub merchant_reference_id: Option<String>,
    pub invoice_number: Option<String>,
    /// The fraud filters that acted on the transaction, in fraud events.
    #[serde(default)]
    pub fraud_list: Vec<FraudFilter>,
}

/// A fraud filter that acted on a transaction.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct FraudFilter {
    /// Such as `AmountFilter`.
    pub fraud_filter: String,
    /// Such as `authAndHold`.
    pub fraud_action: String,
}

/// The customer profile of a customer event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CustomerProfilePayload {
    /// The customer profile ID.
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub merchant_customer_id: Option<String>,
    pub description: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub payment_profiles: Vec<PaymentProfileSummary>,
}

/// A payment profile listed in a customer event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PaymentProfileSummary {
    /// The customer payment profile ID.
    #[serde(deserialize_with = "id")]
    pub id: String,
    #[serde(default, deserialize_with = "option_scalar")]
    pub customer_type: Option<CustomerType>,
}

/// The payment profile of a payment profile event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CustomerPaymentProfilePayload {
    /// The customer payment profile ID.
    #[serde(deserialize_with = "id")]
    pub id: String,
    #[serde(default, deserialize_with = "option_id")]
    pub customer_profile_id: Option<String>,
    #[serde(default, deserialize_with = "option_scalar")]
    pub customer_type: Option<CustomerType>,
}

/// The subscription of a subscription event.
///
/// Amounts keep their value but not always their trailing zeros: `23.50` in the JSON
/// reads as `23.5`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SubscriptionPayload {
    /// The subscription ID.
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<Decimal>,
    #[serde(default, deserialize_with = "option_scalar")]
    pub status: Option<ArbSubscriptionStatus>,
    pub profile: Option<SubscriptionProfile>,
}

/// The customer profile a subscription charges.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SubscriptionProfile {
    #[serde(default, deserialize_with = "option_id")]
    pub customer_profile_id: Option<String>,
    #[serde(default, deserialize_with = "option_id")]
    pub customer_payment_profile_id: Option<String>,
    #[serde(default, deserialize_with = "option_id")]
    pub customer_shipping_address_id: Option<String>,
}

/// A JSON string or number, as text. Authorize.Net sends some IDs and codes as
/// strings and others as numbers.
#[derive(Deserialize)]
#[serde(untagged)]
enum Text {
    String(String),
    Number(serde_json::Number),
}

impl Text {
    fn into_string(self) -> String {
        match self {
            Text::String(text) => text,
            Text::Number(number) => number.to_string(),
        }
    }
}

fn id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Text::deserialize(deserializer).map(Text::into_string)
}

fn option_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<Text>::deserialize(deserializer)?.map(Text::into_string))
}

fn response_code<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<TransactionOutcome>, D::Error> {
    Ok(option_id(deserializer)?.map(|code| TransactionOutcome::from_code(&code)))
}

/// A value of an enum the API also uses in XML, such as [`CustomerType`].
fn scalar<'de, D: Deserializer<'de>, T: XmlScalar>(deserializer: D) -> Result<T, D::Error> {
    T::from_xml_text(&String::deserialize(deserializer)?).map_err(de::Error::custom)
}

fn option_scalar<'de, D: Deserializer<'de>, T: XmlScalar>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|text| T::from_xml_text(&text).map_err(de::Error::custom))
        .transpose()
}
