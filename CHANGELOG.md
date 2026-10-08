# Changelog

All notable changes to `authorizenet` and `authorizenet-derive` are recorded here.
The two crates are released together, with the same version.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the crates follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-08

The first release.

### Added

- Request and response types for the whole Authorize.Net API (35 enums, 9 choice
  enums and 228 structs), generated from the vendored `AnetApiSchema.xsd` by
  `cargo xtask codegen`. Every adjustment to the schema is listed with its reason in
  `schema/overrides.toml`.
- 53 operations, grouped by resource: `client.transactions().create(&request)`,
  `client.customer_profiles()`, `client.subscriptions()`, `client.hosted_pages()`, ...
- An async `Client` and a `blocking::Client`, both generic over a transport:
  `ReqwestTransport` (feature `reqwest`, default) and `UreqTransport` (feature `ureq`),
  or any HTTP client through `transport::Transport` / `blocking::Transport`.
- `protocol::encode` and `protocol::decode`, to use the API without a client.
- Validation of requests against the schema's facets (lengths, patterns, amounts, item
  counts) before sending, reporting the path of each offending field.
- Errors: `Error::Api` keeps the response that reported the error, so a declined
  transaction's reasons are available through `ApiError::transaction_response()`;
  `Error::Validation`, `Error::Status`, `Error::Transport` (with `is_timeout()`) and
  `Error::Xml`.
- `Credentials` for transaction keys, OAuth access tokens, mobile session tokens,
  passwords, client keys, or any `merchantAuthentication`; secrets are held in
  `SecretString`.
- Shortcuts for common requests: `CreditCard::new`, `TransactionRequest::auth_capture`
  and its siblings, `PaymentSchedule::new`, `TransactionResponse::outcome()`.
- `tracing` spans and events for every request, with card numbers, keys and other
  secrets redacted from logged bodies and from `Debug` output.
- TLS through rustls (default) or the platform's library (`native-tls`).

[Unreleased]: https://github.com/lendsimple/authorizenet-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lendsimple/authorizenet-rs/releases/tag/v0.1.0
