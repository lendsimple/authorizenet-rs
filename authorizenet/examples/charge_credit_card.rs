//! Charges a test card in the sandbox, then voids the charge.
//!
//! ```sh
//! ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo run --example charge_credit_card
//! ```

use authorizenet::schema::{CreateTransactionRequest, CreditCard, TransactionRequest};
use authorizenet::{Client, Credentials, Decimal, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::new(credentials_from_env());

    let card = CreditCard::new("4111111111111111", "2035-12").with_code("123");
    // The sandbox rejects a repeat of the same charge within two minutes.
    let charge = TransactionRequest::auth_capture(varying_amount(), card);
    let request = CreateTransactionRequest::from(charge);

    match client.transactions().create(&request).await {
        Ok(response) => {
            let transaction = response.transaction_response;
            println!(
                "{:?}: transaction {} ({})",
                transaction.outcome(),
                transaction.trans_id.as_deref().unwrap_or("?"),
                transaction.auth_code.as_deref().unwrap_or("no auth code"),
            );
            if let Some(id) = transaction.trans_id {
                let void = CreateTransactionRequest::from(TransactionRequest::void(id));
                client.transactions().create(&void).await?;
                println!("voided");
            }
        }
        Err(Error::Api(err)) => {
            // A decline: the transaction response says why.
            println!("failed: {err}");
            for e in err.transaction_response().iter().flat_map(|t| &t.errors) {
                println!("  {:?}: {:?}", e.error_code, e.error_text);
            }
        }
        Err(err) => return Err(err),
    }
    Ok(())
}

fn credentials_from_env() -> Credentials {
    let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    Credentials::transaction_key(var("ANET_LOGIN_ID"), var("ANET_TRANSACTION_KEY"))
}

/// An amount between 1.00 and 99.99 that differs from run to run.
fn varying_amount() -> Decimal {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    Decimal::new(100 + i64::from(nanos % 9900), 2)
}
