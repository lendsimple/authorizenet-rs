//! Helpers shared by integration tests.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code, unused_macros)]

use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path = data_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Fixture file names, sorted, whose root element name ends with `suffix`.
pub fn fixtures_with_root_suffix(suffix: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir())
        .expect("tests/data exists")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".xml"))
        .filter(|name| root_name(&fixture(name)).ends_with(suffix))
        .collect();
    names.sort();
    names
}

pub fn root_name(xml: &[u8]) -> String {
    authorizenet::xml::root_name(xml).expect("fixture has a root element")
}

/// Attributes the API sends that the schema does not define, so they are dropped.
const UNDECLARED_ATTRIBUTES: &[&str] = &[
    // ARBGetSubscriptionStatusResponse/@note: "Status with a capital 'S' is obsolete."
    "note",
];

/// Compares two documents as element trees: names, schema attributes, child order,
/// and the trimmed text of leaf elements. Whitespace between elements and namespace
/// declarations are ignored. Returns the first difference.
pub fn compare_trees(expected: &str, actual: &str) -> Result<(), String> {
    let expected = roxmltree::Document::parse(strip_declarations(expected))
        .map_err(|e| format!("expected: {e}"))?;
    let actual = roxmltree::Document::parse(actual).map_err(|e| format!("actual: {e}"))?;
    compare_nodes(expected.root_element(), actual.root_element(), "")
}

/// Drops leading whitespace and XML declarations; one fixture repeats its declaration.
fn strip_declarations(mut xml: &str) -> &str {
    loop {
        xml = xml.trim_start();
        match xml
            .strip_prefix("<?xml")
            .and_then(|rest| rest.split_once("?>"))
        {
            Some((_, rest)) => xml = rest,
            None => return xml,
        }
    }
}

fn compare_nodes(e: roxmltree::Node, a: roxmltree::Node, parent: &str) -> Result<(), String> {
    let path = format!("{parent}/{}", e.tag_name().name());
    if e.tag_name().name() != a.tag_name().name() {
        return Err(format!(
            "{path}: element <{}> where <{}> was expected",
            a.tag_name().name(),
            e.tag_name().name()
        ));
    }
    let attrs = |n: roxmltree::Node| -> Vec<(String, String)> {
        let mut v: Vec<_> = n
            .attributes()
            .filter(|at| at.namespace().is_none() && !UNDECLARED_ATTRIBUTES.contains(&at.name()))
            .map(|at| (at.name().to_owned(), at.value().to_owned()))
            .collect();
        v.sort();
        v
    };
    if attrs(e) != attrs(a) {
        return Err(format!(
            "{path}: attributes {:?} != expected {:?}",
            attrs(a),
            attrs(e)
        ));
    }
    let e_children: Vec<_> = e.children().filter(|n| n.is_element()).collect();
    let a_children: Vec<_> = a.children().filter(|n| n.is_element()).collect();
    if e_children.is_empty() && a_children.is_empty() {
        let text = |n: roxmltree::Node| n.text().unwrap_or_default().trim().to_owned();
        if text(e) != text(a) {
            return Err(format!(
                "{path}: text {:?} != expected {:?}",
                text(a),
                text(e)
            ));
        }
        return Ok(());
    }
    for (i, ec) in e_children.iter().enumerate() {
        match a_children.get(i) {
            Some(ac) => compare_nodes(*ec, *ac, &path)?,
            None => {
                return Err(format!(
                    "{path}: missing <{}> (child {i})",
                    ec.tag_name().name()
                ));
            }
        }
    }
    if let Some(extra) = a_children.get(e_children.len()) {
        return Err(format!("{path}: unexpected <{}>", extra.tag_name().name()));
    }
    Ok(())
}

/// The `<merchantAuthentication>` element of a request fixture, as markup.
pub fn auth_fragment(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let node = doc
        .descendants()
        .find(|n| n.tag_name().name() == "merchantAuthentication")?;
    Some(xml[node.range()].to_owned())
}

// Client test support.

use std::sync::{Arc, Mutex};

use authorizenet::Credentials;
use authorizenet::schema::MerchantAuthentication;
use authorizenet::xml::{self, XmlRoot};

/// The credentials the Python client's tests use.
pub fn credentials() -> Credentials {
    Credentials::transaction_key("5KP3u95bQpv", "346HZ32z3fP4hTG2")
}

/// A request read from a fixture.
pub fn request_fixture<T: XmlRoot>(name: &str) -> T {
    xml::from_slice_strict(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The credentials in a request fixture, so the request sent matches it exactly.
pub fn fixture_credentials(name: &str) -> Credentials {
    let text = String::from_utf8(fixture(name)).expect("UTF-8 fixture");
    match auth_fragment(&text) {
        Some(auth) => Credentials::from_merchant_authentication(
            xml::from_slice_as::<MerchantAuthentication>(auth.as_bytes()).unwrap(),
        ),
        None => credentials(),
    }
}

/// `(request, credentials, golden request fixture)` for a client test case:
/// `fixture("x_request.xml")` reads the request from a fixture, and `built(expr)` uses
/// a request built in code (where the Python test has no fixture).
macro_rules! case {
    (fixture($file:literal)) => {
        (
            common::request_fixture($file),
            common::fixture_credentials($file),
            Some($file),
        )
    };
    (built($request:expr)) => {
        ($request, common::credentials(), None::<&str>)
    };
}

/// A mock Authorize.Net endpoint answering every POST with one canned response, and
/// recording the request body. It only answers requests with the client's content
/// type and default user agent; anything else gets mockito's 501.
pub struct MockApi {
    server: mockito::ServerGuard,
    _mock: mockito::Mock,
    body: Arc<Mutex<Option<Vec<u8>>>>,
}

impl MockApi {
    fn mock(
        server: &mut mockito::ServerGuard,
        status: usize,
        response: Vec<u8>,
        delay: std::time::Duration,
    ) -> (mockito::Mock, Arc<Mutex<Option<Vec<u8>>>>) {
        let body = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&body);
        let mock = server
            .mock("POST", "/")
            .match_header("content-type", "application/xml")
            .match_header(
                "user-agent",
                mockito::Matcher::Regex("^authorizenet-rs/".into()),
            )
            .with_status(status)
            .with_body_from_request(move |request| {
                *captured.lock().unwrap() = Some(request.body().unwrap().clone());
                std::thread::sleep(delay);
                response.clone()
            });
        (mock, body)
    }

    pub fn new(status: usize, response: Vec<u8>) -> Self {
        Self::delayed(status, response, std::time::Duration::ZERO)
    }

    /// Answers only after `delay`, to test timeouts.
    pub fn delayed(status: usize, response: Vec<u8>, delay: std::time::Duration) -> Self {
        let mut server = mockito::Server::new();
        let (mock, body) = Self::mock(&mut server, status, response, delay);
        Self {
            _mock: mock.create(),
            server,
            body,
        }
    }

    pub async fn new_async(status: usize, response: Vec<u8>) -> Self {
        Self::delayed_async(status, response, std::time::Duration::ZERO).await
    }

    pub async fn delayed_async(
        status: usize,
        response: Vec<u8>,
        delay: std::time::Duration,
    ) -> Self {
        let mut server = mockito::Server::new_async().await;
        let (mock, body) = Self::mock(&mut server, status, response, delay);
        Self {
            _mock: mock.create_async().await,
            server,
            body,
        }
    }

    pub fn environment(&self) -> authorizenet::Environment {
        authorizenet::Environment::Custom(self.server.url())
    }

    /// The body of the request received, if any.
    pub fn received(&self) -> Option<String> {
        let body = self.body.lock().unwrap().clone()?;
        Some(String::from_utf8(body).expect("UTF-8 request"))
    }

    /// Checks that a request arrived and, given a golden fixture, that it matches it.
    pub fn check_request(&self, golden: Option<&str>) {
        let body = self.received().expect("the client sent a request");
        if let Some(golden) = golden {
            let expected = String::from_utf8(fixture(golden)).unwrap();
            compare_trees(&expected, &body)
                .unwrap_or_else(|e| panic!("request differs from {golden}: {e}\n{body}"));
        }
    }
}

/// Captures `tracing` output, for checking what the clients log.
#[derive(Clone, Default)]
pub struct LogCapture(Arc<Mutex<Vec<u8>>>);

impl LogCapture {
    /// A subscriber that writes every event, at all levels, here.
    pub fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync + 'static {
        let writer = self.clone();
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish()
    }

    pub fn contents(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl std::io::Write for LogCapture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Invokes `$callback!` with one case per pair of sync/async tests in the Python
/// client's suite: `name: group.method, request => response fixture;`.
///
/// Not ported here: `mobile_devices.login`, because the live XSD no longer has
/// `mobileDeviceLoginRequest`; and `subscriptions.get_status`, whose fixture is an
/// error, which the client test files check separately.
macro_rules! client_cases {
    ($callback:ident) => {
        $callback! {
            // test_account_updater_jobs.py
            account_updater_jobs_get_details: account_updater_jobs.get_details,
                fixture("get_account_updater_job_detail_request.xml") => "get_account_updater_job_detail_response.xml";
            account_updater_jobs_get_summary: account_updater_jobs.get_summary,
                fixture("get_account_updater_job_summary_request.xml") => "get_account_updater_job_summary_response.xml";
            // test_batches.py
            batches_get_statistics: batches.get_statistics,
                fixture("get_batch_statistics_request.xml") => "get_batch_statistics_response.xml";
            batches_list_settled: batches.list_settled,
                fixture("get_settled_batch_list_request.xml") => "get_settled_batch_list_response.xml";
            // test_customer_payment_profiles.py
            customer_payment_profiles_create: customer_payment_profiles.create,
                fixture("create_customer_payment_profile_request.xml") => "create_customer_payment_profile_response.xml";
            customer_payment_profiles_delete: customer_payment_profiles.delete,
                fixture("delete_customer_payment_profile_request.xml") => "delete_customer_payment_profile_response.xml";
            customer_payment_profiles_get: customer_payment_profiles.get,
                fixture("get_customer_payment_profile_request.xml") => "get_customer_payment_profile_response.xml";
            customer_payment_profiles_list: customer_payment_profiles.list,
                fixture("list_customer_payment_profile_request.xml") => "list_customer_payment_profile_response.xml";
            customer_payment_profiles_update: customer_payment_profiles.update,
                fixture("update_customer_payment_profile_request.xml") => "update_customer_payment_profile_response.xml";
            customer_payment_profiles_validate: customer_payment_profiles.validate,
                fixture("validate_customer_payment_profile_request.xml") => "validate_customer_payment_profile_response.xml";
            // test_customer_profiles.py
            customer_profiles_create: customer_profiles.create,
                fixture("create_customer_profile_request.xml") => "create_customer_profile_response.xml";
            customer_profiles_create_from_transaction: customer_profiles.create_from_transaction,
                fixture("create_customer_profile_from_transaction_request.xml") => "create_customer_profile_from_transaction_response.xml";
            customer_profiles_create_transaction: customer_profiles.create_transaction,
                built(common::cases::create_customer_profile_transaction()) => "create_customer_profile_transaction_response.xml";
            customer_profiles_delete: customer_profiles.delete,
                fixture("delete_customer_profile_request.xml") => "delete_customer_profile_response.xml";
            customer_profiles_get: customer_profiles.get,
                fixture("get_customer_profile_request.xml") => "get_customer_profile_response.xml";
            customer_profiles_get_ids: customer_profiles.get_ids,
                fixture("get_customer_profile_ids_request.xml") => "get_customer_profile_ids_response.xml";
            customer_profiles_update: customer_profiles.update,
                fixture("update_customer_profile_request.xml") => "update_customer_profile_response.xml";
            // test_customer_shipping_addresses.py
            customer_shipping_addresses_create: customer_shipping_addresses.create,
                fixture("create_customer_shipping_address_request.xml") => "create_customer_shipping_address_response.xml";
            customer_shipping_addresses_delete: customer_shipping_addresses.delete,
                fixture("delete_customer_shipping_address_request.xml") => "delete_customer_shipping_address_response.xml";
            customer_shipping_addresses_get: customer_shipping_addresses.get,
                fixture("get_customer_shipping_address_request.xml") => "get_customer_shipping_address_response.xml";
            customer_shipping_addresses_update: customer_shipping_addresses.update,
                fixture("update_customer_shipping_address_request.xml") => "update_customer_shipping_address_response.xml";
            // test_hosted_pages.py
            hosted_pages_get_payment_page: hosted_pages.get_payment_page,
                fixture("get_hosted_payment_page_request.xml") => "get_hosted_payment_page_response.xml";
            hosted_pages_get_profile_page: hosted_pages.get_profile_page,
                built(common::cases::hosted_profile_page()) => "get_hosted_profile_page_response.xml";
            // test_merchants.py
            merchants_get: merchants.get,
                fixture("get_merchant_details_request.xml") => "get_merchant_details_response.xml";
            merchants_update: merchants.update,
                built(common::cases::update_merchant_details()) => "update_merchant_details_response.xml";
            // test_misc.py
            misc_decrypt_payment_data: misc.decrypt_payment_data,
                built(common::cases::decrypt_payment_data()) => "decrypt_payment_data_response.xml";
            misc_is_alive: misc.is_alive,
                built(authorizenet::schema::IsAliveRequest::default()) => "is_alive_response.xml";
            misc_logout: misc.logout,
                built(authorizenet::schema::LogoutRequest::default()) => "logout_response.xml";
            misc_test_authenticate: misc.test_authenticate,
                built(authorizenet::schema::AuthenticateTestRequest::default()) => "authenticate_test_response.xml";
            // test_mobile_devices.py
            mobile_devices_register: mobile_devices.register,
                built(common::cases::mobile_device_registration()) => "mobile_device_registration_response.xml";
            // test_secure_payment_containers.py
            secure_payment_containers_create: secure_payment_containers.create,
                built(common::cases::secure_payment_container()) => "secure_payment_container_response.xml";
            // test_subscriptions.py
            subscriptions_cancel: subscriptions.cancel,
                fixture("cancel_subscription_request.xml") => "cancel_subscription_response.xml";
            subscriptions_create: subscriptions.create,
                fixture("create_subscription_request.xml") => "create_subscription_response.xml";
            subscriptions_create_from_profile: subscriptions.create,
                fixture("create_subscription_from_profile_request.xml") => "create_subscription_from_profile_response.xml";
            subscriptions_get: subscriptions.get,
                fixture("get_subscription_request.xml") => "get_subscription_response.xml";
            subscriptions_list: subscriptions.list,
                fixture("list_subscription_request.xml") => "list_subscription_response.xml";
            subscriptions_update: subscriptions.update,
                fixture("update_subscription_request.xml") => "update_subscription_response.xml";
            // test_transactions.py
            transactions_accept_nonce: transactions.create,
                fixture("create_transaction_accept_nonce_request.xml") => "create_transaction_accept_nonce_response.xml";
            transactions_authorize_credit_card: transactions.create,
                fixture("authorize_credit_card_request.xml") => "authorize_credit_card_response.xml";
            transactions_capture_credit_card: transactions.create,
                fixture("capture_credit_card_request.xml") => "capture_credit_card_response.xml";
            transactions_charge_credit_card: transactions.create,
                fixture("charge_credit_card_request.xml") => "charge_credit_card_response.xml";
            transactions_charge_customer_profile: transactions.create,
                fixture("charge_customer_profile_request.xml") => "charge_customer_profile_response.xml";
            transactions_charge_tokenized_credit_card: transactions.create,
                fixture("charge_tokenized_credit_card_request.xml") => "charge_tokenized_credit_card_response.xml";
            transactions_credit_bank_account: transactions.create,
                fixture("credit_bank_account_request.xml") => "credit_bank_account_response.xml";
            transactions_debit_bank_account: transactions.create,
                fixture("debit_bank_account_request.xml") => "debit_bank_account_response.xml";
            transactions_refund: transactions.create,
                fixture("refund_transaction_request.xml") => "refund_transaction_response.xml";
            transactions_void: transactions.create,
                fixture("void_transaction_request.xml") => "void_transaction_response.xml";
            transactions_get: transactions.get,
                fixture("get_transaction_details_request.xml") => "get_transaction_details_response.xml";
            transactions_list: transactions.list,
                fixture("get_transaction_list_request.xml") => "get_transaction_list_response.xml";
            transactions_list_for_customer: transactions.list_for_customer,
                fixture("get_transaction_for_customer_request.xml") => "get_transaction_for_customer_response.xml";
            transactions_list_unsettled: transactions.list_unsettled,
                built(common::cases::unsettled_transactions()) => "get_transaction_unsettled_response.xml";
            transactions_send_receipt: transactions.send_receipt,
                built(common::cases::send_receipt()) => "send_customer_transaction_receipt_response.xml";
            transactions_update_held: transactions.update_held,
                fixture("update_held_transaction_request.xml") => "update_held_transaction_response.xml";
            transactions_update_split_tender_group: transactions.update_split_tender_group,
                fixture("update_split_tender_group_request.xml") => "update_split_tender_group_response.xml";
        }
    };
}

/// Requests the Python tests build in code, built the same way.
pub mod cases {
    use authorizenet::Decimal;
    use authorizenet::schema::*;

    fn dec(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    pub fn create_customer_profile_transaction() -> CreateCustomerProfileTransactionRequest {
        let amount = ProfileTransAmount::builder().amount(dec("10.95")).build();
        let order = ProfileTransOrder::builder()
            .profile_trans_amount(amount)
            .customer_profile_id("1929820324")
            .customer_payment_profile_id("1841987457")
            .order(
                OrderEx::builder()
                    .order(
                        Order::builder()
                            .invoice_number("INV000001")
                            .description("Product Description")
                            .build(),
                    )
                    .build(),
            )
            .build();
        let transaction = ProfileTransaction::ProfileTransAuthCapture(
            ProfileTransAuthCapture::builder()
                .profile_trans_order(order)
                .build(),
        );
        CreateCustomerProfileTransactionRequest::builder()
            .transaction(transaction)
            .build()
    }

    pub fn update_merchant_details() -> UpdateMerchantDetailsRequest {
        UpdateMerchantDetailsRequest::builder()
            .is_test_mode(true)
            .build()
    }

    pub fn decrypt_payment_data() -> DecryptPaymentDataRequest {
        DecryptPaymentDataRequest::builder()
            .opaque_data(
                OpaqueData::builder()
                    .data_descriptor("COMMON.VCO.ONLINE.PAYMENT")
                    .data_value("ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890")
                    .build(),
            )
            .build()
    }

    pub fn mobile_device_registration() -> MobileDeviceRegistrationRequest {
        MobileDeviceRegistrationRequest::builder()
            .mobile_device(
                MobileDevice::builder()
                    .mobile_device_id("a]]b2DC7C-E499-4472-88A8-4B826AFE7B2")
                    .description("iPhone 14")
                    .phone_number("000-000-0000")
                    .build(),
            )
            .build()
    }

    pub fn secure_payment_container() -> SecurePaymentContainerRequest {
        let token = WebCheckOutDataTypeToken::builder()
            .card_number("4111111111111111")
            .expiration_date("2025-12")
            .card_code("999")
            .build();
        SecurePaymentContainerRequest::builder()
            .data(
                WebCheckOutData::builder()
                    .r#type(WebCheckOutType::Token)
                    .id("unique-checkout-id-12345")
                    .token(token)
                    .build(),
            )
            .build()
    }

    pub fn hosted_profile_page() -> GetHostedProfilePageRequest {
        let setting = |name, value: &str| {
            Setting::builder()
                .setting_name(name)
                .setting_value(value)
                .build()
        };
        GetHostedProfilePageRequest::builder()
            .customer_profile_id("1929820324")
            .hosted_profile_settings(
                ArrayOfSetting::builder()
                    .setting(vec![
                        setting(
                            SettingName::HostedProfileReturnUrl,
                            "https://returnurl.com/return/",
                        ),
                        setting(
                            SettingName::HostedProfileReturnUrlText,
                            "Continue to confirmation page.",
                        ),
                        setting(SettingName::HostedProfilePageBorderVisible, "true"),
                    ])
                    .build(),
            )
            .build()
    }

    pub fn send_receipt() -> SendCustomerTransactionReceiptRequest {
        SendCustomerTransactionReceiptRequest::builder()
            .trans_id("60163892435")
            .customer_email("customer@example.com")
            .build()
    }

    pub fn unsettled_transactions() -> GetUnsettledTransactionListRequest {
        GetUnsettledTransactionListRequest::builder()
            .sorting(
                TransactionListSorting::builder()
                    .order_by(TransactionListOrderField::SubmitTimeUtc)
                    .order_descending(true)
                    .build(),
            )
            .paging(Paging::builder().limit(100).offset(1).build())
            .build()
    }
}
