//! Creates a monthly recurring billing (ARB) subscription, then cancels it.
//!
//! ```sh
//! ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo run --example subscription
//! ```

use authorizenet::schema::{
    ArbCancelSubscriptionRequest, ArbCreateSubscriptionRequest, ArbSubscription, CreditCard,
    NameAndAddress, PaymentSchedule, PaymentScheduleInterval,
};
use authorizenet::{Client, Credentials, Decimal, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let client = Client::new(Credentials::transaction_key(
        var("ANET_LOGIN_ID"),
        var("ANET_TRANSACTION_KEY"),
    ));

    let start = time::OffsetDateTime::now_utc().date() + time::Duration::days(1);
    let subscription = ArbSubscription::builder()
        .name("Monthly plan")
        .payment_schedule(PaymentSchedule::new(
            PaymentScheduleInterval::months(1),
            start,
            12,
        ))
        .amount(Decimal::new(999, 2))
        .payment(CreditCard::new("4111111111111111", "2035-12").into())
        .bill_to(
            NameAndAddress::builder()
                .first_name("Ellen")
                .last_name("Johnson")
                .build(),
        )
        .build();
    let created = client
        .subscriptions()
        .create(
            &ArbCreateSubscriptionRequest::builder()
                .subscription(subscription)
                .build(),
        )
        .await?;
    let id = created
        .subscription_id
        .expect("a new subscription has an id");
    println!("created subscription {id}, starting {start}");

    client
        .subscriptions()
        .cancel(
            &ArbCancelSubscriptionRequest::builder()
                .subscription_id(&id)
                .build(),
        )
        .await?;
    println!("cancelled subscription {id}");
    Ok(())
}
