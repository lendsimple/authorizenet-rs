//! Gets a token for the Accept Hosted payment form, which collects the card on
//! Authorize.Net's page instead of yours.
//!
//! ```sh
//! ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo run --example hosted_payment_page
//! ```

use authorizenet::schema::{
    ArrayOfSetting, GetHostedPaymentPageRequest, Setting, SettingName, TransactionRequest,
    TransactionType,
};
use authorizenet::{Client, Credentials, Decimal, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let client = Client::new(Credentials::transaction_key(
        var("ANET_LOGIN_ID"),
        var("ANET_TRANSACTION_KEY"),
    ));

    let transaction = TransactionRequest::builder()
        .transaction_type(TransactionType::AuthCaptureTransaction)
        .amount(Decimal::new(2000, 2))
        .build();
    let settings = ArrayOfSetting::builder()
        .setting(vec![
            Setting::builder()
                .setting_name(SettingName::HostedPaymentReturnOptions)
                .setting_value(
                    r#"{"showReceipt": true, "url": "https://example.com/receipt", "urlText": "Continue"}"#,
                )
                .build(),
        ])
        .build();
    let request = GetHostedPaymentPageRequest::builder()
        .transaction_request(transaction)
        .hosted_payment_settings(settings)
        .build();

    let response = client.hosted_pages().get_payment_page(&request).await?;
    println!("POST this token to https://test.authorize.net/payment/payment as `token`:");
    println!("{}", response.token);
    Ok(())
}
