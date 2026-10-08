//! Stores a customer with a card, reads the profile back, and deletes it.
//!
//! ```sh
//! ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo run --example customer_profile
//! ```

use authorizenet::schema::{
    CreateCustomerProfileRequest, CreditCard, CustomerPaymentProfile, CustomerProfile,
    CustomerProfileBase, DeleteCustomerProfileRequest, GetCustomerProfileRequest, ValidationMode,
};
use authorizenet::{Client, Credentials, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let var = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let client = Client::new(Credentials::transaction_key(
        var("ANET_LOGIN_ID"),
        var("ANET_TRANSACTION_KEY"),
    ))?;

    // Profiles must be unique by merchant customer id, description and email.
    let customer_id = format!(
        "rs-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let payment_profile = CustomerPaymentProfile::builder()
        .payment(CreditCard::new("4111111111111111", "2035-12").into())
        .build();
    let profile = CustomerProfile::builder()
        .customer_profile_base(
            CustomerProfileBase::builder()
                .merchant_customer_id(customer_id)
                .email("customer@example.com")
                .build(),
        )
        .payment_profiles(vec![payment_profile])
        .build();
    let created = client
        .customer_profiles()
        .create(
            &CreateCustomerProfileRequest::builder()
                .profile(profile)
                .validation_mode(ValidationMode::None)
                .build(),
        )
        .await?;
    let id = created
        .customer_profile_id
        .expect("a new profile has an id");
    println!("created profile {id}");

    let fetched = client
        .customer_profiles()
        .get(
            &GetCustomerProfileRequest::builder()
                .customer_profile_id(&id)
                .build(),
        )
        .await?;
    let cards = fetched.profile.map_or(0, |p| p.payment_profiles.len());
    println!("profile {id} has {cards} payment profile(s)");

    client
        .customer_profiles()
        .delete(
            &DeleteCustomerProfileRequest::builder()
                .customer_profile_id(&id)
                .build(),
        )
        .await?;
    println!("deleted profile {id}");
    Ok(())
}
