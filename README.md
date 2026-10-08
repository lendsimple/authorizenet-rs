# authorizenet

A typed Rust client for the [Authorize.Net](https://developer.authorize.net/api/reference/)
payment API, with async and blocking clients.

- Every request and response type is generated from Authorize.Net's own XML schema,
  so the client covers the whole API: 53 operations, from charging cards to customer
  profiles, recurring billing and hosted payment pages.
- Requests are checked against the schema's limits (lengths, patterns, amounts)
  before they are sent, with errors that name the offending field.
- Card numbers, keys and other secrets never appear in `Debug` output or logs.

```toml
[dependencies]
authorizenet = "0.1"
```

| Feature | Default | |
|---|---|---|
| `async` | yes | `Client`, built on reqwest, for tokio |
| `blocking` | | `blocking::Client`, built on ureq, with no async runtime |
| `rustls` | yes | TLS with rustls |
| `native-tls` | | TLS with the platform's library |

The minimum supported Rust version is 1.88.

## Charging a card

```rust,no_run
use authorizenet::schema::{CreateTransactionRequest, CreditCard, TransactionRequest};
use authorizenet::{Client, Credentials, Decimal, Environment};

#[tokio::main]
async fn main() -> Result<(), authorizenet::Error> {
    let client = Client::builder(Credentials::transaction_key("API login id", "transaction key"))
        .environment(Environment::Sandbox)
        .build()?;

    let card = CreditCard::new("4111111111111111", "2035-12").with_code("123");
    let charge = TransactionRequest::auth_capture(Decimal::new(1999, 2), card);
    let response = client
        .transactions()
        .create(&CreateTransactionRequest::from(charge))
        .await?;

    println!("approved: {:?}", response.transaction_response.trans_id);
    Ok(())
}
```

Operations are grouped by resource, as in the API reference: `client.customer_profiles()`,
`client.subscriptions()`, `client.hosted_pages()`, and so on. Each takes the request type
generated from the schema, which you build with its builder, and returns its response
type. The blocking client has the same methods:

```rust,no_run
use authorizenet::Credentials;
use authorizenet::blocking::Client;
use authorizenet::schema::AuthenticateTestRequest;

let client = Client::new(Credentials::transaction_key("API login id", "transaction key"))?;
client.misc().test_authenticate(&AuthenticateTestRequest::default())?;
# Ok::<(), authorizenet::Error>(())
```

The async client runs on tokio; it is cheap to clone and its futures can be spawned.
Do not call the blocking client from async code, since it blocks the thread.

## Errors

When the API answers with result code `Error`, the call returns `Error::Api`. A
declined transaction is one of these, and keeps the transaction response that says
why:

```rust,no_run
# use authorizenet::{Client, Error};
# use authorizenet::schema::CreateTransactionRequest;
# async fn charge(client: &Client, request: &CreateTransactionRequest) -> Result<(), Error> {
match client.transactions().create(request).await {
    Ok(response) => println!("{:?}", response.transaction_response.outcome()),
    Err(Error::Api(err)) => {
        println!("{err}"); // [E00027] The transaction was unsuccessful.
        for e in err.transaction_response().iter().flat_map(|t| &t.errors) {
            println!("{:?}: {:?}", e.error_code, e.error_text);
        }
    }
    Err(err) => return Err(err),
}
# Ok(())
# }
```

Other errors are `Validation` (the request was not sent), `Status` (an HTTP error),
`Transport` (connection, TLS or timeout) and `Xml`.

## Logging

The clients use [`tracing`](https://docs.rs/tracing). Each request runs in an
`authorizenet` span with its operation name. Request and response bodies are logged
at `TRACE` level, with secrets replaced by `[REDACTED]`.

## Without a client

With `default-features = false`, `authorizenet::protocol` encodes requests and decodes
responses without doing any I/O, so you can send them with any HTTP client.

## Examples

`authorizenet/examples` has runnable examples against the sandbox: charging a card
(async and blocking), customer profiles, subscriptions and the hosted payment page.

```sh
ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo run --example charge_credit_card
```

## Development

The workspace has the `authorizenet` crate, its derive macros (`authorizenet-derive`),
and `xtask`, which generates the schema types:

```sh
cargo test --workspace --all-features   # unit, fixture, mock-server and doc tests
cargo xtask codegen                     # regenerate after changing schema/
cargo xtask codegen --check             # CI: is the generated code up to date?
```

The schema is vendored in `schema/AnetApiSchema.xsd`. `schema/README.md` records its
version and how to update it, and `schema/overrides.toml` lists every adjustment the
generator makes, with its reason.

Tests that call the real sandbox are ignored by default:

```sh
ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo test --all-features --test sandbox -- --ignored
```

## License

MIT
