//! Hand-written conveniences on the generated types, for the most common requests.
//!
//! Everything here is a shortcut for builders that already exist; fields stay public,
//! so a value made here can still be adjusted:
//!
//! ```
//! use authorizenet::Decimal;
//! use authorizenet::schema::{CreditCard, Order, TransactionRequest};
//!
//! let card = CreditCard::new("4111111111111111", "2035-12").with_code("123");
//! let mut charge = TransactionRequest::auth_capture(Decimal::new(1999, 2), card);
//! charge.order = Some(Order::builder().invoice_number("INV-1").build());
//! ```

use rust_decimal::Decimal;
use time::Date;

use super::{
    ArbSubscriptionUnit, BankAccount, CreateTransactionPayload, CreateTransactionRequest,
    CreditCard, CreditCardSimple, MessageType, Messages, OpaqueData, Payment, PaymentInstrument,
    PaymentSchedule, PaymentScheduleInterval, TransactionRequest, TransactionResponse,
    TransactionType,
};
use crate::types::XmlDate;

impl CreditCard {
    /// A card by number and expiration date, written `YYYY-MM` (or `MMYY`).
    pub fn new(number: impl Into<String>, expiration_date: impl Into<String>) -> Self {
        let card = CreditCardSimple::builder()
            .card_number(number)
            .expiration_date(expiration_date)
            .build();
        CreditCard::builder().credit_card_simple(card).build()
    }

    /// Adds the card code (CVV).
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.card_code = Some(code.into());
        self
    }
}

impl OpaqueData {
    /// A payment nonce, such as Accept.js returns: descriptor
    /// `COMMON.ACCEPT.INAPP.PAYMENT` and the nonce as the value.
    pub fn new(descriptor: impl Into<String>, value: impl Into<String>) -> Self {
        OpaqueData::builder()
            .data_descriptor(descriptor)
            .data_value(value)
            .build()
    }
}

impl Payment {
    pub fn new(instrument: PaymentInstrument) -> Self {
        Payment::builder().instrument(instrument).build()
    }
}

impl From<CreditCard> for Payment {
    fn from(card: CreditCard) -> Self {
        Payment::new(PaymentInstrument::CreditCard(card))
    }
}

impl From<BankAccount> for Payment {
    fn from(account: BankAccount) -> Self {
        Payment::new(PaymentInstrument::BankAccount(account))
    }
}

impl From<OpaqueData> for Payment {
    fn from(nonce: OpaqueData) -> Self {
        Payment::new(PaymentInstrument::OpaqueData(nonce))
    }
}

impl TransactionRequest {
    fn with_payment(kind: TransactionType, amount: Decimal, payment: Payment) -> Self {
        TransactionRequest::builder()
            .transaction_type(kind)
            .amount(amount)
            .payment(payment)
            .build()
    }

    fn referring_to(kind: TransactionType, ref_trans_id: String) -> Self {
        TransactionRequest::builder()
            .transaction_type(kind)
            .ref_trans_id(ref_trans_id)
            .build()
    }

    /// Authorizes and captures `amount`: a charge.
    pub fn auth_capture(amount: Decimal, payment: impl Into<Payment>) -> Self {
        Self::with_payment(
            TransactionType::AuthCaptureTransaction,
            amount,
            payment.into(),
        )
    }

    /// Authorizes `amount`, to capture later with [`prior_auth_capture`](Self::prior_auth_capture).
    pub fn auth_only(amount: Decimal, payment: impl Into<Payment>) -> Self {
        Self::with_payment(TransactionType::AuthOnlyTransaction, amount, payment.into())
    }

    /// Captures an earlier authorization. Set `amount` to capture less than was
    /// authorized.
    pub fn prior_auth_capture(ref_trans_id: impl Into<String>) -> Self {
        Self::referring_to(
            TransactionType::PriorAuthCaptureTransaction,
            ref_trans_id.into(),
        )
    }

    /// Refunds `amount` of a settled transaction. For a card, `payment` needs only its
    /// last four digits and an expiration date of `XXXX`.
    pub fn refund(
        amount: Decimal,
        payment: impl Into<Payment>,
        ref_trans_id: impl Into<String>,
    ) -> Self {
        let mut request =
            Self::with_payment(TransactionType::RefundTransaction, amount, payment.into());
        request.ref_trans_id = Some(ref_trans_id.into());
        request
    }

    /// Voids an unsettled transaction.
    pub fn void(ref_trans_id: impl Into<String>) -> Self {
        Self::referring_to(TransactionType::VoidTransaction, ref_trans_id.into())
    }
}

impl CreateTransactionRequest {
    pub fn new(transaction: TransactionRequest) -> Self {
        CreateTransactionRequest::builder()
            .transaction(CreateTransactionPayload::TransactionRequest(transaction))
            .build()
    }
}

impl From<TransactionRequest> for CreateTransactionRequest {
    fn from(transaction: TransactionRequest) -> Self {
        CreateTransactionRequest::new(transaction)
    }
}

impl Messages {
    /// Whether the result code is `Ok`.
    pub fn is_ok(&self) -> bool {
        self.result_code == MessageType::Ok
    }
}

/// What happened to a transaction, from `transactionResponse/responseCode`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionOutcome {
    /// `1`
    Approved,
    /// `2`
    Declined,
    /// `3`
    Error,
    /// `4`: held for review by the fraud detection suite.
    HeldForReview,
    /// A code the API documentation does not list.
    Other(String),
}

impl TransactionResponse {
    /// The outcome, if the response has a response code.
    pub fn outcome(&self) -> Option<TransactionOutcome> {
        Some(match self.response_code.as_deref()?.trim() {
            "1" => TransactionOutcome::Approved,
            "2" => TransactionOutcome::Declined,
            "3" => TransactionOutcome::Error,
            "4" => TransactionOutcome::HeldForReview,
            other => TransactionOutcome::Other(other.to_owned()),
        })
    }
}

impl PaymentScheduleInterval {
    /// Every `months` months (1 to 12).
    pub fn months(months: i16) -> Self {
        PaymentScheduleInterval::builder()
            .length(months)
            .unit(ArbSubscriptionUnit::Months)
            .build()
    }

    /// Every `days` days (7 to 365).
    pub fn days(days: i16) -> Self {
        PaymentScheduleInterval::builder()
            .length(days)
            .unit(ArbSubscriptionUnit::Days)
            .build()
    }
}

impl PaymentSchedule {
    /// `total_occurrences` payments, one every `interval`, from `start`. Use 9999 for a
    /// subscription without an end date.
    pub fn new(interval: PaymentScheduleInterval, start: Date, total_occurrences: i16) -> Self {
        PaymentSchedule::builder()
            .interval(interval)
            .start_date(XmlDate::from(start))
            .total_occurrences(total_occurrences)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::{XmlWrite, XmlWriter};
    use time::macros::date;

    fn xml(value: &impl XmlWrite, tag: &str) -> String {
        let mut w = XmlWriter::new(Vec::new());
        value.write_element(&mut w, tag).unwrap();
        String::from_utf8(w.into_inner()).unwrap()
    }

    #[test]
    fn card_charge() {
        let card = CreditCard::new("4111111111111111", "2035-12").with_code("123");
        let charge = TransactionRequest::auth_capture(Decimal::new(500, 2), card);
        assert_eq!(
            xml(&charge, "transactionRequest"),
            "<transactionRequest><transactionType>authCaptureTransaction</transactionType>\
             <amount>5.00</amount><payment><creditCard><cardNumber>4111111111111111</cardNumber>\
             <expirationDate>2035-12</expirationDate><cardCode>123</cardCode></creditCard>\
             </payment></transactionRequest>"
        );
    }

    #[test]
    fn void_refers_to_the_transaction() {
        assert_eq!(
            xml(&TransactionRequest::void("123"), "transactionRequest"),
            "<transactionRequest><transactionType>voidTransaction</transactionType>\
             <refTransId>123</refTransId></transactionRequest>"
        );
    }

    #[test]
    fn refund_has_payment_and_reference() {
        let refund = TransactionRequest::refund(
            Decimal::new(100, 2),
            CreditCard::new("1111", "XXXX"),
            "456",
        );
        assert_eq!(refund.transaction_type, TransactionType::RefundTransaction);
        assert_eq!(refund.ref_trans_id.as_deref(), Some("456"));
        assert!(refund.payment.is_some());
    }

    #[test]
    fn nonce_payment() {
        let payment = Payment::from(OpaqueData::new("COMMON.ACCEPT.INAPP.PAYMENT", "nonce"));
        assert!(matches!(
            payment.instrument,
            PaymentInstrument::OpaqueData(_)
        ));
    }

    #[test]
    fn outcome_from_response_code() {
        let response = |code: &str| TransactionResponse::builder().response_code(code).build();
        assert_eq!(response("1").outcome(), Some(TransactionOutcome::Approved));
        assert_eq!(
            response("4").outcome(),
            Some(TransactionOutcome::HeldForReview)
        );
        assert_eq!(
            response("9").outcome(),
            Some(TransactionOutcome::Other("9".into()))
        );
        assert_eq!(TransactionResponse::default().outcome(), None);
    }

    #[test]
    fn monthly_schedule() {
        let schedule = PaymentSchedule::new(
            PaymentScheduleInterval::months(1),
            date!(2030 - 01 - 15),
            12,
        );
        assert_eq!(
            xml(&schedule, "paymentSchedule"),
            "<paymentSchedule><interval><length>1</length><unit>months</unit></interval>\
             <startDate>2030-01-15</startDate><totalOccurrences>12</totalOccurrences>\
             </paymentSchedule>"
        );
    }
}
