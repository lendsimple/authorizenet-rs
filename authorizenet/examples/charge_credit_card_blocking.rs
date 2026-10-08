//! Charges a test card in the sandbox with the blocking client.
//!
//! ```sh
//! ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... \
//!     cargo run --example charge_credit_card_blocking --features blocking
//! ```

use authorizenet::blocking::Client;
use authorizenet::schema::{CreateTransactionRequest, CreditCard, TransactionRequest};
use authorizenet::{Credentials, Decimal, Error};

fn main() -> Result<(), Error> {
    let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let client = Client::new(Credentials::transaction_key(
        var("ANET_LOGIN_ID"),
        var("ANET_TRANSACTION_KEY"),
    ))?;

    let card = CreditCard::new("4111111111111111", "2035-12").with_code("123");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let amount = Decimal::new(100 + i64::from(nanos % 9900), 2);
    let request = CreateTransactionRequest::from(TransactionRequest::auth_capture(amount, card));

    let response = client.transactions().create(&request)?;
    let transaction = response.transaction_response;
    println!(
        "{:?}: transaction {}",
        transaction.outcome(),
        transaction.trans_id.as_deref().unwrap_or("?")
    );
    Ok(())
}
