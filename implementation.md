# authorizenet-rs — Implementation Plan

## Context

We want a new, idiomatic Rust client for Authorize.Net's XML API (`POST /xml/v1/request.api`). It should support both async and blocking use.

The Python client at `~/Web/python-authorizenet` is a **reference, not a source to port**:
- its `schema.py` (xsdata-pydantic, 244 complex types, 34 enums) shows what the API looks like in practice;
- its `tests/data/*.xml` (≈70 request/response fixture pairs) are our compatibility corpus.

**Decisions already made** (answered during planning):

| Topic | Decision |
|---|---|
| Schema source | `cargo xtask codegen` reads a **vendored copy of the live `AnetApiSchema.xsd`** plus `overrides.toml`. It writes readable Rust, and that output is committed. |
| XML engine | **Our own derive proc-macro** on top of `quick-xml`'s streaming `Reader`/`Writer`. Not serde. |
| Sync/async | A **sans-IO core**, plus `Client` (async, `reqwest`) and `blocking::Client` (`ureq`). Each client is behind an additive cargo feature. |
| API version | The **current live XSD**, which has moved ahead of `schema.py` (see Schema drift). |

## What we learned from the Python client (things to keep)

**Request flow** (`client.py`):
1. Inject `merchantAuthentication` into every `ANetApiRequest`. `isAliveRequest` is not one, so it gets none.
2. Serialize with the default namespace `AnetApi/xml/v1/schema/AnetApiSchema.xsd`.
3. POST with `Content-Type: application/xml`, a 60 s timeout and a User-Agent.
4. Treat a non-2xx HTTP status as an error.

**Response handling:**
- Parse into the expected response type. If that fails, parse again as `ErrorResponse`, which holds just `messages`.
- `messages.resultCode == Error` raises an error that carries the code and text of the first message, plus the full typed response.
- The error displays as `"[E00035] The subscription cannot be found."`.

**Operations** (`operation.py`): 50 methods in 12 resource groups. Two pairings break the naming convention:
- `createCustomerProfileFromTransactionRequest` → `CreateCustomerProfileResponse`
- `getTransactionListForCustomerRequest` → `GetTransactionListResponse`

**Schema subtleties to get right:**

*Element order and inheritance*
- **Element order matters.** `xs:sequence` order is enforced server-side.
- XSD extension means parent fields come first, then child fields. There are 20 extension chains, e.g. `CustomerAddressExType → CustomerAddressType → NameAndAddressType`.
- Parent types are also used on their own. For example, `NameAndAddressType` is the type of `shipTo`.

*Element names*
- Root names have irregular casing: `ARBCreateSubscriptionRequest`, `getAUJobDetailsRequest`, `isAliveRequest`, `ErrorResponse`.
- Element names do too: `refTransID` vs `refTransId`, `customerProfileID`, `customerIP`, `FDSFilters`, `AVSResponse`, `DUKPT`, and PascalCase children under `secureAcceptance`.

*Choices and anonymous types*
- **`xs:choice` groups** become Rust enums:
  - `paymentType`, `paymentMaskedType`, `paymentSimpleType`
  - `merchantAuthenticationType` credentials
  - `profileTransactionType`, `creditCardTrackType`
  - the createTransaction `transactionRequest | transactionRequestEnc` choice
  - `ListOfAUDetailsType`, an *unbounded* choice. Python split it into two lists and lost the interleaved order; we keep it as `Vec<AuDetail>`.
- **Anonymous nested types**: 20 of them. `TransactionResponse` alone has 7, and two different inner types are both called `Message`.

*Scalar types*
- Amounts are `xs:decimal` with `fractionDigits=4` and `minInclusive` 0.00 or 0.01.
- Dates come in two kinds: `xs:date` (2 fields) and `xs:dateTime`. Authorize.Net sends both UTC and offset-less "local" times.
- `xs:anyType` appears in `paymentEmvType.emvData` and similar fields.
- There is one attribute: `emailSettingsType/@version`.
- Some list items are `nillable`.

*Python hand-edits that reflect real API behavior* (go into `overrides.toml`)
- `TransactionResponse` gets a bare `<message>` child that is not in the XSD. The fixture `create_transaction_charge_credit_card_response.xml` contains one.
- Several `xs:string` fields are retyped as enums: `settingName`, `transactionType`.

*Real-world response quirks*
- Responses may start with a **UTF-8 BOM**.
- They declare `xsi`/`xsd` namespaces.
- They contain empty elements such as `<cavvResultCode />`.
- They leave out XSD-"required" fields when the result is an error.
- Declined transactions come back as `resultCode=Error`, but the body is still a full `transactionResponse` with `errors`. The error value must therefore keep the typed response.

*Logging*
- Python logs request bodies, **including credentials and card numbers**, at DEBUG. We redact them instead.

**Schema drift.** The live XSD has 264 complexTypes and 57 request elements, while `schema.py` has 46 requests. Items the live XSD adds include `transactionRequestEnc`, `getTransactionSummaryRequest/Response`, and `activationCode` in the `merchantAuthentication` choice. Stage 2 produces a full drift report (`docs/schema-drift.md`).

## Architecture

### Workspace layout
```
authorizenet-rs/
├── Cargo.toml                      # [workspace], edition 2024, resolver 3
├── schema/
│   ├── AnetApiSchema.xsd           # vendored, pinned (date + sha256 in README)
│   └── overrides.toml              # naming, retyping, quirks, sensitivity
├── authorizenet/                   # published crate
│   ├── src/
│   │   ├── lib.rs                  # re-exports, feature docs
│   │   ├── xml/                    # runtime: XmlWrite/XmlRead traits, scalar impls, reader/writer helpers
│   │   ├── types/                  # XmlDate, XmlDateTime, RawXml, Secret wrappers
│   │   ├── schema/
│   │   │   ├── mod.rs
│   │   │   └── enums.rs, types.rs, messages.rs   # @generated by xtask
│   │   ├── operations.rs           # operations! { … } DSL invocation (the operation table)
│   │   ├── core.rs                 # sans-IO: Credentials, Environment, encode(), decode()
│   │   ├── error.rs
│   │   ├── client.rs               # async Client           (feature "async")
│   │   └── blocking.rs             # blocking::Client       (feature "blocking")
│   └── tests/
│       ├── data/                   # fixtures copied from python-authorizenet/tests/data
│       └── *.rs
├── authorizenet-derive/            # proc-macro: #[derive(AnetXml)], #[derive(AnetEnum)]
└── xtask/                          # unpublished: `cargo xtask codegen [--check]`, `cargo xtask drift`
```

### Dependencies
At Stage 1, pin the latest versions with `cargo add`.
- **Runtime:** `quick-xml`, `rust_decimal`, `time` (`macros`, `parsing`, `formatting`), `thiserror`, `tracing`, `secrecy`, `bon` (builders).
- **Feature `async`** (default): `reqwest`, with default features off and `rustls-tls`.
- **Feature `blocking`:** `ureq`, which needs no tokio.
- **Derive crate:** `syn`, `quote`, `proc-macro2`.
- **xtask:** `roxmltree`, `quote`, `prettyplease`, `toml`, `serde`, `heck`.
- **Dev:** `tokio` (macros, rt), `mockito` (one mock server for both sync and async), `pretty_assertions`, `insta`.

### Layer 1: XML runtime + derive DSL (`authorizenet::xml`, `authorizenet-derive`)
Traits (public but `#[doc(hidden)]`-ish; users rarely touch them):
```rust
pub trait XmlWrite { fn write_xml<W: Write>(&self, w: &mut XmlWriter<W>, tag: &str) -> Result<(), XmlError>; }
pub trait XmlRead: Sized { fn read_xml(r: &mut XmlReader<'_>, start: &BytesStart<'_>) -> Result<Self, XmlError>; }
pub trait XmlScalar: Sized { fn to_xml_text(&self) -> Cow<'_, str>; fn from_xml_text(s: &str) -> Result<Self, XmlError>; }
pub trait XmlRoot: XmlWrite + XmlRead { const ROOT: &'static str; }
```
`XmlScalar` is implemented for `String`, `bool`, `i32`, `i64`, `u32`, `Decimal`, `XmlDate`, `XmlDateTime`, and every generated enum.

**Derive attribute DSL** (this is what the generated code looks like):
```rust
#[derive(Debug, Clone, PartialEq, AnetXml, bon::Builder)]
#[anet(root = "createTransactionRequest", request)]   // request => base fields + merchantAuthentication injection
#[non_exhaustive]
pub struct CreateTransactionRequest {
    #[anet(rename = "refId", max_length = 20)]
    pub ref_id: Option<String>,
    #[anet(choice)]                                     // variants are written as sibling elements
    pub transaction: CreateTransactionPayload,          // enum { TransactionRequest(TransactionRequest), TransactionRequestEnc(String) }
}

#[derive(AnetXml, …)]
pub struct CustomerAddress {
    #[anet(flatten)] pub name_and_address: NameAndAddress,   // XSD extension → composition, parent fields first
    #[anet(rename = "phoneNumber", max_length = 25)] pub phone_number: Option<String>,
    …
}

pub struct TransactionRequest {
    #[anet(rename = "lineItems", wrapper, item = "lineItem")]  // ArrayOf* inlined to Vec<T>
    pub line_items: Vec<LineItem>,
    #[anet(rename = "cardCode", sensitive)] …                  // redacted in Debug + logs
}
```

**Supported field attributes:**

| Attribute | Purpose |
|---|---|
| `rename` | XML element name |
| `flatten` | Writes the parent type's fields inline (XSD extension) |
| `choice` | Field is an enum; each variant is a sibling element |
| `wrapper, item = ".."` | A `Vec` wrapped in a container element |
| `list` | Repeated element with no wrapper |
| `attribute` | Written as an XML attribute |
| `sensitive` | Redacted in `Debug` and in logs |
| `max_length`, `min_length`, `pattern`, `min`, `max`, `fraction_digits`, `total_digits`, `max_occurs` | XSD facets, enforced by `validate()` |

**Enums:**
- `#[derive(AnetEnum)]` with `#[anet(value = "authOnlyTransaction")]` on each variant.
- Each enum is `#[non_exhaustive]` and gets an `Other(String)` catch-all, so a new value from the API does not break parsing.

**Writing:**
- Elements are written in declaration order, which is XSD sequence order.
- `None` and empty wrapped `Vec`s are omitted.
- `xmlns="AnetApi/xml/v1/schema/AnetApiSchema.xsd"` goes on the root only.
- Text is escaped by quick-xml.

**Reading:**
- Strip the BOM and skip the XML declaration.
- Match on local names and ignore namespace prefixes.
- Accept children **in any order** (a lenient reader).
- **Skip unknown elements** for forward compatibility. `xml::from_slice_strict` turns unknown elements, duplicates and stray text into errors; tests use it to catch codegen gaps.
- Empty elements:
  - an empty element read into `Option<String>` gives `Some("")`;
  - an empty or `xsi:nil` element read into an optional non-string scalar gives `None`;
  - a missing required field gives `XmlError::MissingField { type, field }`.

**Validation:**
- The derive generates `fn validate(&self) -> Result<(), ValidationError>` from the facets, collecting every violation along with its path.
- `Client` calls it before sending. This is configurable (`validate_requests: bool`, default `true`).

### Layer 2: codegen (`xtask`)
1. Parse the XSD with `roxmltree` into a small IR: `ComplexType { name, base: Option, particles: Vec<Particle>, attrs }`, `SimpleType { base, facets, enumeration }`, and anonymous types named `Parent + Child` (e.g. `TransactionResponseMessages`).
2. Apply `overrides.toml`:
   - **Naming:** type renames such as `ARBCreateSubscriptionRequest → ArbCreateSubscriptionRequest` and `getAUJobDetailsRequest → GetAuJobDetailsRequest`. Snake-case uses acronym-aware `heck` plus an explicit table (`refTransID → ref_trans_id`, `customerIP → customer_ip`, `FDSFilters → fds_filters`). Rust keywords (`type`) are renamed to `r#type` or `kind`.
   - **Field typing:** retype string fields as enums, following Python's hand-edits and the "str holding enum" list (accountType, transactionStatus, settlementState, cardType, permissionName, …) — but only where we are confident. Unknown values still parse because every enum has `Other`.
   - **Quirks:** extra quirk fields (`TransactionResponse.message`), `sensitive` flags, request→response pairings for the two odd operations, and choice-group variant names.
   - **Lists:** make lists `Option<Vec<_>>` where "empty wrapper" and "absent" mean different things.
3. Emit Rust with `quote` + `prettyplease` into `src/schema/generated/{enums,types,messages}.rs`, with an `// @generated — do not edit; run cargo xtask codegen` header. `*Type` suffixes are dropped where that causes no clash (`creditCardType → CreditCard`).
4. `cargo xtask codegen --check` fails in CI if the committed output is stale.
5. `cargo xtask drift --python ../python-authorizenet` writes `docs/schema-drift.md`, the type and field differences from `schema.py`. This is a one-off, but repeatable.

**Request messages:**
- Generated request structs do **not** contain `merchantAuthentication`. `#[anet(request)]` writes `merchantAuthentication` (from the client), then `clientId`, then `refId`, then the struct's own fields.
- `isAliveRequest` is generated without `request`, so it is unauthenticated.
- Response structs embed the `ANetApiResponse` base through `flatten` (`refId`, `messages`, `sessionToken`). They implement `ApiResponse { fn messages(&self) -> &Messages }`.

### Layer 3: operations DSL + sans-IO core
The operation table is the second macro DSL. A `macro_rules!` lives in `operations.rs`:
```rust
operations! {
    transactions => Transactions {
        /// Create a transaction (auth, capture, refund, void, …).
        create(CreateTransactionRequest) -> CreateTransactionResponse;
        get(GetTransactionDetailsRequest) -> GetTransactionDetailsResponse;
        list_for_customer(GetTransactionListForCustomerRequest) -> GetTransactionListResponse;
        …
    }
    customer_profiles => CustomerProfiles { create_from_transaction(CreateCustomerProfileFromTransactionRequest) -> CreateCustomerProfileResponse; … }
    …  // account_updater_jobs, batches, customer_payment_profiles, customer_shipping_addresses,
       // hosted_pages, merchants, misc, mobile_devices, secure_payment_containers, subscriptions
}
```

The macro expands to:
- `impl ApiRequest for Req { type Response = Resp; }`
- the resource handle structs, with doc comments, for **both** clients. Methods are generated twice, once `async` and once blocking, behind their cfgs, from the same table. This avoids the non-additive feature problem that `maybe_async` has.
- `pub enum AnyResponse { … }` with `TryFrom` impls. It is used in errors.

New live-XSD operations (`get_transaction_summary`, …) are added to the table.

**Core (`core.rs`)** has no I/O:
- `Credentials` is an enum mirroring the auth choice. `Credentials::transaction_key(login, key)` covers the common case; the other variants are session token, access token (OAuth), client key, and so on. Secrets are held in `secrecy::SecretString`.
- `Environment` is `Sandbox | Production | Custom(Url)`. Sandbox is `https://apitest.authorize.net/xml/v1/request.api` and Production is `https://api.authorize.net/xml/v1/request.api`.
- `fn encode<R: ApiRequest>(req: &R, creds: &Credentials) -> Result<Vec<u8>, Error>` validates, then serializes.
- `fn decode<R: ApiRequest>(status: u16, body: &[u8]) -> Result<R::Response, Error>`:
  1. Fail on a non-2xx status.
  2. Strip the BOM.
  3. Peek the root name. If it is `ErrorResponse`, return `Error::Api`.
  4. Otherwise parse `R::Response`. If parsing fails, fall back to parsing only the base `messages`. If those say Error, return `Error::Api` with no typed response; otherwise return `Error::Xml`.
  5. If the typed parse worked but `resultCode == Error`, return `Error::Api` with the typed response.

**Errors** (`thiserror`):
```rust
pub enum Error {
    #[error("[{}] {}", .0.code(), .0.text())] Api(Box<ApiError>),   // messages + Option<AnyResponse>
    #[error(transparent)] Validation(ValidationError),
    #[error(transparent)] Xml(XmlError),
    #[error("HTTP {status}")] Status { status: u16, body: String },
    #[error(transparent)] Transport(TransportError),               // wraps reqwest/ureq, feature-gated
}
```
- `ApiError::code()` and `ApiError::text()` return the first message, matching Python's string format.
- `ApiError::messages()` returns all messages.
- `ApiError::response::<T>() -> Option<&T>` gives typed access to a declined transaction's `transactionResponse.errors`.

### Layer 4: clients
```rust
let client = authorizenet::Client::builder()
    .credentials(Credentials::transaction_key(login_id, key))
    .environment(Environment::Sandbox)       // default: Sandbox (matches Python)
    .timeout(Duration::from_secs(60))
    .http_client(reqwest_client)              // optional injection (like Python's httpx client arg)
    .build()?;

let resp = client.transactions().create(req).await?;   // async
let resp = blocking_client.transactions().create(req)?; // blocking
let resp = client.execute(req).await?;                   // generic escape hatch
```
- `Client` is `Clone` because it holds an `Arc` inside, so it can be shared across tasks.
- Python's context managers have no equivalent here: connection pools are owned and dropped automatically.
- Requests carry `User-Agent: authorizenet-rs/{CARGO_PKG_VERSION}` and `Content-Type: application/xml`.
- **Tracing:**
  - the `authorizenet.request` span records the operation and root name;
  - request and response bodies are logged at `TRACE` only, after `redact()` masks `sensitive` elements;
  - the sensitive elements are cardNumber, cardCode, transactionKey, password, accountNumber, routingNumber, track1/2, dataValue, sessionToken and accessToken.
- Two Python items are dropped: `generator.py`, which is a repr-to-code helper with no Rust use, and `parser.py`/`serializer.py`, which become `authorizenet::xml::{to_string, to_string_pretty, from_slice}`.

### Ergonomics
- Every generated struct has `bon::Builder` and is `#[non_exhaustive]`, so new XSD fields are not semver-breaking. Required fields are compile-time-required builder args; `Option` and `Vec` fields are optional setters.
- Public fields can be read, and also written after the struct is built.
- Hand-written convenience constructors live in `schema/ext.rs`, not in generated code. Examples:
  - `Payment::credit_card(number, exp, cvv)`
  - `TransactionRequest::auth_capture(amount, payment)`
  - `MessageType::is_ok()`
- Amounts use `rust_decimal::Decimal` (a `dec!` re-export). The writer keeps the scale, so `45.00` stays `45.00`.
- `XmlDateTime` and `XmlDate` keep the exact lexical value, including an optional offset, so they round-trip losslessly. They convert with `to_offset_date_time()` and `to_primitive()` (`time` crate).

## Stages

## Stage 1: Workspace, XML runtime, derive macros
**Goal**: A cargo workspace with `authorizenet`, `authorizenet-derive` and `xtask` skeletons. It has the `XmlWrite`/`XmlRead`/`XmlScalar` runtime and the `AnetXml`/`AnetEnum` derives, with every attribute except the validation facets. A handful of **hand-written** types prove the derive works: `Messages`, `MessageType`, `MerchantAuthentication` (choice), `NameAndAddress`/`CustomerAddress` (flatten), `AuthenticateTestRequest`/`Response`, and `ErrorResponse`.

**Success Criteria**:
- `cargo build`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` are clean.
- Derive errors give span-accurate messages (checked with `trybuild` UI tests).

**Tests**:
- Round-trip `error_response.xml` and `authenticate_test_response.xml` (copied fixtures).
- BOM-prefixed input parses.
- Unknown elements are skipped; with `strict`, they error.
- Children in any order parse.
- `<x/>` gives `Some("")` for a string and `None` for a decimal.
- Flattened parent fields are written before child fields.
- A choice variant is written as a sibling element.
- `xmlns` is on the root only.
- `Other(String)` enum fallback works.
- `trybuild` compile-fail cases.

**Status**: Complete

**Notes (as built)**:
- Strictness is a runtime choice (`xml::from_slice` vs `xml::from_slice_strict`), not a cfg flag.
- `xs:anyType` needs no `any` attribute: a field typed `types::RawXml` keeps the inner markup as-is.
- `#[derive(AnetXml)]` also implements `Debug`, so that `sensitive` fields are redacted. Types using it must not derive `Debug`.
- `bon::Builder` was brought in now rather than later. Schema structs are `#[non_exhaustive]`, so builders are the only way to construct them outside the crate.
- Validation facets (`max_length`, `pattern`, …) are not parsed yet; they arrive with `validate()`.
- `xml::root_name` (peek at a document's root) was added early for the `ErrorResponse` check in Stage 3.

## Stage 2: XSD codegen
**Goal**:
- Vendor the live XSD (record its date and sha256).
- Implement `xtask codegen` (IR → overrides → emit) and generate every type, enum and request/response message.
- Write `overrides.toml` to cover all the quirks listed above.
- Produce `docs/schema-drift.md`.
- Delete the Stage 1 hand-written types; the generated ones replace them.

**Success Criteria**:
- `cargo xtask codegen --check` passes.
- The generated code compiles warning-free and has rustdoc taken from `xs:documentation`.
- Every response fixture in `tests/data` parses under `strict`. Any field the XSD lacks is either in the overrides or documented.

**Tests**:
- `insta` snapshot of the IR for a few tricky types (`TransactionResponse`, `ListOfAUDetailsType`, `merchantAuthenticationType`, `paymentType`).
- **Golden request tests**: build each Python `*_request.xml` fixture as a Rust value, serialize it, and compare to the fixture with a whitespace-insensitive, element-tree compare. The Python suite never checked request serialization, so this is new coverage.
- Parse → serialize → parse round-trip equality for every response fixture.
- **XSD conformance**: when `xmllint` is present, every serialized request is validated against the vendored XSD with `xmllint --schema` (skipped with a notice otherwise).

**Status**: Complete

**Notes (as built)**:
- Generated code lives in `src/schema/{enums,types,messages}.rs` (excluded from rustfmt), not a `generated/` folder. There are 35 enums, 9 choice enums and 228 structs, 105 of them API messages.
- Golden request tests parse each `*_request.xml` strictly, serialize it, and compare element trees, instead of hand-building 51 Rust values. This gives the same coverage of names, order and values, in both directions. Responses get the same check plus a value round trip.
- New override kinds: `dedupe` (`mobileDeviceMfaLoginResponse` declares `sessionToken` twice), `optional` (fields the live XSD made required that older responses lack: `tapToPhone`, `debtRepaymentIndicator`), and names for anonymous types (`MessagesMessage` → `Message`).
- New derive flag `keep_empty`: a *required* list wrapper is written even when empty (e.g. `customerShippingAddressIdList`). An optional one is omitted.
- Choice-only types (`creditCardTrackType`, `profileTransactionType`, ...) become enums usable directly as element types.
- The inline bases `ANetApiRequest`/`ANetApiResponse` are not generated. `ErrorResponse` holds exactly the common response fields, and `xml::from_slice_as::<ErrorResponse>` reads them from any response (for Stage 3's fallback).
- `authorizenet::__for_each_request!`/`__for_each_response!` list every message type, generated alongside them. `AnyResponse` (Stage 3) should be built from them.
- Facets are kept in the codegen's XSD model but not emitted yet; Stage 3 emits them with `validate()`.
- Fixture exceptions: `mobile_device_login_response.xml` is skipped, because the live XSD removed `mobileDeviceLogin*`. `get_hosted_profile_page_request.xml` is not schema-validated, because its sample profile id `YourProfileID` breaks the `[0-9]+` pattern.
- XSD pin and update steps: `schema/README.md`.



## Stage 3: Operations DSL, sans-IO core, errors
**Goal**:
- `operations!` table covering all 50 Python operations plus the new live-XSD ones.
- `ApiRequest`/`ApiResponse` traits and `AnyResponse`.
- `Credentials` and `Environment`.
- `encode`/`decode` with the ErrorResponse fallback and typed-response errors.
- The `Error`/`ApiError` model.
- Validation facets wired into `validate()`.

**Success Criteria**:
- Every operation is reachable.
- `decode` reproduces all of Python's error semantics.
- No HTTP dependency is needed to compile the core (`--no-default-features` builds).

**Tests**:
- `decode` on `error_response.xml` gives `Api` with code `E00035`, text `"The subscription cannot be found."` and Display `"[E00035] The subscription cannot be found."`.
- A declined-transaction body (a new fixture: resultCode=Error plus `transactionResponse.errors`) gives `Api` with `response::<CreateTransactionResponse>()` returning `Some`.
- Responses missing XSD-required fields under Error fall back to messages-only.
- A non-2xx status gives `Status`.
- `isAliveRequest` is encoded without `merchantAuthentication`.
- Validation catches `refId` longer than 50 (the live XSD's limit; the Python schema had 20), a negative amount, and an amount with more than 4 fraction digits, and reports the path.

**Status**: Complete

**Notes (as built)**:
- `operation_table!` (`src/operations.rs`) is a callback macro: it passes the one table of operations to whichever macro needs it. Stage 3 uses it to implement `ApiRequest` (with `OPERATION`, e.g. `transactions.create`, for logs); Stage 4 uses it again for the clients' resource handles. It has 53 operations: Python's 47, minus `mobileDeviceLogin` (removed from the XSD), plus `transactions.get_summary` and six `mobile_devices` calls (PIN/verify/MFA login, and getting and saving the acceptance device). A test checks that every generated request type is in the table.
- `AnyResponse`, and the `ApiResponse` impl of every response, are generated by codegen rather than written by hand.
- The sans-IO module is `protocol`, not `core`, which would clash with `::core`. `encode` returns the body as a `String`. `decode` is generic over the response type.
- Errors: `Error::{Api, Validation, Xml, Status}`. `ApiError` has `code()`, `text()`, `messages()`, `any_response()`, `response::<T>()`. The `Transport` variant arrives with the HTTP clients in Stage 4.
- Validation: the derive compiles `[class]+` patterns (the only form the XSD uses) into character checks, so there is no `regex` dependency. Facets accumulate along restriction chains. Item counts of an *optional* list wrapper apply only when it is present. All request fixtures pass validation except the known `YourProfileID` sample.
- New fixture `create_transaction_declined_response.xml`, shaped like the documented decline: resultCode `Error`, `E00027`, `transactionResponse.errors`.



## Stage 4: Async + blocking clients
**Goal**:
- `Client` (reqwest) and `blocking::Client` (ureq) with builders, timeout, environment, injected HTTP client, User-Agent and tracing with redaction.
- Resource handles generated for both clients.
- Port the Python test suite: 117 sync+async tests across 13 files.

**Success Criteria**:
- `cargo test --all-features` passes.
- `--no-default-features --features blocking` builds and tests without tokio in the dependency tree (`cargo tree` check).
- The feature matrix builds: none, async, blocking, and both.
- The async methods' futures are `Send`, so they work with `tokio::spawn` (checked at compile time).

**Tests**:
- One test module per Python file (`transactions.rs`, `customer_profiles.rs`, …). Each test has a sync and an async variant against a `mockito` server serving the matching fixture.
- Each test asserts the response type and `MessageType::Ok`, which is what Python asserts.
- Each test also asserts that the **request body** the mock received matches the golden request.
- Error tests are ported from `test_errors.py`.
- Redaction test: a captured trace never contains a card number or transaction key.

**Status**: Complete

**Notes (as built)**:
- API: `Client::new(credentials)` or `Client::builder(credentials)`, with credentials a required argument rather than a `.credentials()` setter, so a client cannot be built without them. Builder settings: `environment`, `timeout`, `user_agent`, `validate_requests`, plus `http_client(reqwest::Client)` / `agent(ureq::Agent)`; timeout and user agent apply per request, so they hold for injected clients too. Resource methods take the request by reference: `client.transactions().create(&request)`. `execute(&request)` sends any request.
- The resource handles of both clients come from `operation_table!` through `resource_handles!`.
- Features: `async` (default, reqwest), `blocking` (ureq), `rustls` (default) or `native-tls`. All six combinations pass clippy with `-D warnings`. `cargo tree` shows no tokio, hyper or reqwest with only `blocking`.
- New `Error::Transport(TransportError)`, with `is_timeout()`.
- Logging: an `authorizenet` span per request with its `operation`; DEBUG events for request, status and failure; TRACE events with bodies, redacted using `SENSITIVE_ELEMENTS`, which codegen emits from `overrides.toml`. A truncated body is redacted to its end.
- Ported tests: 55 sync/async pairs, one per Python test pair. Each asserts result code `Ok` and that the request body the mock received matches the request fixture, where there is one. Requests the Python tests build in code are built the same way with the builders. Not ported: `mobile_devices.login` (removed from the XSD). `subscriptions.get_status` is ported as the error test it is in Python (`E00035`). `get_hosted_profile_page` builds its request as Python does, instead of using the fixture with the invalid `YourProfileID`.
- Beyond the Python suite: HTTP error status, connection failure, timeout, validation on and off, `Send` futures, sharing a client across tokio tasks, and log redaction. The redaction tests sit in their own binary (`tests/logging.rs`) with a global subscriber. A thread-local subscriber was unreliable while other tests ran concurrently, because `tracing` caches callsite interest globally.
- The blocking client reads responses up to 100 MB (ureq's default limit is 10 MB).



## Stage 5: Hardening, docs, release prep
**Goal**:
- Convenience constructors (`schema/ext.rs`).
- `examples/` for the async and blocking flows: charge card, create profile, ARB subscription, and hosted payment page.
- Crate-level docs and feature docs; README with the XSD pin and the regeneration procedure.
- CI (GitHub Actions): fmt, clippy, test feature matrix, MSRV check, `xtask codegen --check`, xmllint conformance, `cargo deny`.
- Opt-in live sandbox tests, `#[ignore]` and gated on `ANET_LOGIN_ID`/`ANET_TRANSACTION_KEY`.

**Success Criteria**:
- `cargo doc` has no warnings and the docs have examples.
- CI is green.
- Live sandbox smoke tests pass (authenticateTest, charge + void, customer profile CRUD).
- `cargo publish --dry-run` succeeds for `authorizenet-derive` and `authorizenet`. The `authorizenet` name is currently free on crates.io.

**Tests**: doc tests on the examples, and the live sandbox suite.

**Status**: In Progress — the work is done; two checks need credentials or a GitHub remote (see Remaining).

**Notes (as built)**:
- Convenience constructors (`src/schema/ext.rs`): `CreditCard::new(..).with_code(..)`, `OpaqueData::new`, `Payment::new` and `From<CreditCard | BankAccount | OpaqueData> for Payment`, `TransactionRequest::{auth_capture, auth_only, prior_auth_capture, refund, void}`, `CreateTransactionRequest::new` / `From<TransactionRequest>`, `Messages::is_ok`, `TransactionResponse::outcome()` → `TransactionOutcome`, `PaymentSchedule::new` with `PaymentScheduleInterval::{months, days}`, and `ApiError::transaction_response()`.
- Examples (`authorizenet/examples`): `charge_credit_card` (charge, then void), `charge_credit_card_blocking`, `customer_profile` (create, get, delete), `subscription` (create, cancel), `hosted_payment_page`. They read `ANET_LOGIN_ID`/`ANET_TRANSACTION_KEY` and target the sandbox.
- Sandbox tests (`tests/sandbox.rs`, `#[ignore]`): isAlive, authenticateTest (both clients), charge + void, customer profile lifecycle. They panic with a clear message if the credentials are not set, rather than passing silently.
- README with usage, features, errors, logging, development and schema-update notes. Its examples are compiled as doctests (`#[cfg(doctest)]` include in `lib.rs`).
- MSRV 1.88: the highest `rust-version` among dependencies, and what let-chains need. Verified with `cargo +1.88 check --all-features --all-targets`.
- CI (`.github/workflows/ci.yml`): fmt; clippy over 5 feature sets; tests on Linux, macOS and Windows over 3 feature sets, with xmllint on Linux for the XSD conformance test; MSRV; `codegen --check`; docs with `-D warnings`; cargo-deny. `.gitattributes` keeps fixtures and generated code byte-exact on Windows.
- `deny.toml`: permissive licenses only (MIT, Apache-2.0, BSD-3-Clause, ISC, Unicode-3.0, CDLA-Permissive-2.0). `cargo deny check` passes locally.
- `cargo publish --dry-run --workspace` packages and verifies both crates.

**Remaining**:
- Run the sandbox tests with sandbox credentials.
- Push to GitHub and confirm the workflow passes. The workflow is valid YAML, and each of its commands passes locally on macOS; it has not run on Actions, and Linux/Windows have not been tried.
- Add a `repository` URL to the manifests before publishing (it was left out rather than guessed).



## Verification (end-to-end)
1. `cargo xtask codegen --check && cargo fmt --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
2. `cargo test --workspace --all-features`, then `cargo test -p authorizenet --no-default-features --features blocking`, then `cargo build -p authorizenet --no-default-features`
3. `cargo test -p authorizenet --test conformance`, which uses xmllint against the vendored XSD.
4. `ANET_LOGIN_ID=… ANET_TRANSACTION_KEY=… cargo test -p authorizenet --all-features -- --ignored`, which runs against the sandbox.
5. `cargo run --example charge_credit_card --features async`, then check the transaction in the sandbox merchant interface.

## Open items to resolve during implementation (not blocking)
- ~~Set the MSRV~~: 1.88 (Stage 5).
- Settle the final list of string→enum retypings. Be conservative: only retype where the XSD or the API docs list the values.
- Decide whether any wrapped lists must be `Option<Vec<_>>`, for update semantics where an empty wrapper and an absent one mean different things. Check against the API reference for update operations.
- License: MIT, matching the Python client.
