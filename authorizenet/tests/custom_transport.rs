//! Both clients with custom, in-memory transports: no HTTP, no cargo features, and for
//! the async client no tokio (a minimal executor drives it).

mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake};
use std::time::Duration;

use authorizenet::schema::{AuthenticateTestRequest, MessageType};
use authorizenet::transport::{HttpRequest, HttpResponse};
use authorizenet::{
    Client, ClientBuilder, DEFAULT_TIMEOUT, DEFAULT_USER_AGENT, Environment, Error,
    SANDBOX_ENDPOINT, TransportError, blocking,
};
use common::{credentials, fixture};

/// What the client handed to the transport.
#[derive(Debug, Clone)]
struct Sent {
    url: String,
    body: String,
    content_type: String,
    user_agent: String,
    timeout: Duration,
}

/// Answers every request with the same result and records what was sent.
#[derive(Clone)]
struct FakeTransport {
    answer: Arc<dyn Fn() -> Result<HttpResponse, TransportError> + Send + Sync>,
    sent: Arc<Mutex<Vec<Sent>>>,
}

impl FakeTransport {
    fn answering(status: u16, fixture_name: &str) -> Self {
        let body = fixture(fixture_name);
        Self::with(move || Ok(HttpResponse::new(status, body.clone())))
    }

    fn with(
        answer: impl Fn() -> Result<HttpResponse, TransportError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            answer: Arc::new(answer),
            sent: Arc::default(),
        }
    }

    fn answer(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError> {
        self.sent.lock().unwrap().push(Sent {
            url: request.url.to_owned(),
            body: request.body,
            content_type: request.content_type.to_owned(),
            user_agent: request.user_agent.to_owned(),
            timeout: request.timeout,
        });
        (self.answer)()
    }

    fn only_request(&self) -> Sent {
        let sent = self.sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "expected exactly one request");
        sent[0].clone()
    }
}

impl authorizenet::transport::Transport for FakeTransport {
    async fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError> {
        self.answer(request)
    }
}

impl blocking::Transport for FakeTransport {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError> {
        self.answer(request)
    }
}

/// Runs a future to completion on the current thread, without an async runtime.
fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        std::thread::park();
    }
}

fn authenticate() -> AuthenticateTestRequest {
    AuthenticateTestRequest::default()
}

#[test]
fn async_client_sends_through_a_custom_transport() {
    let transport = FakeTransport::answering(200, "authenticate_test_response.xml");
    let client = Client::with_transport(credentials(), transport.clone());
    let response = block_on(client.misc().test_authenticate(&authenticate())).unwrap();
    assert_eq!(response.messages.result_code, MessageType::Ok);

    let sent = transport.only_request();
    assert_eq!(sent.url, SANDBOX_ENDPOINT);
    assert_eq!(sent.content_type, "application/xml");
    assert_eq!(sent.user_agent, DEFAULT_USER_AGENT);
    assert_eq!(sent.timeout, DEFAULT_TIMEOUT);
    assert!(
        sent.body.contains("<authenticateTestRequest"),
        "{}",
        sent.body
    );
    assert!(
        sent.body
            .contains("<transactionKey>346HZ32z3fP4hTG2</transactionKey>")
    );
}

#[test]
fn blocking_client_sends_through_a_custom_transport() {
    let transport = FakeTransport::answering(200, "authenticate_test_response.xml");
    let client = blocking::Client::with_transport(credentials(), transport.clone());
    let response = client.misc().test_authenticate(&authenticate()).unwrap();
    assert_eq!(response.messages.result_code, MessageType::Ok);
    assert!(
        transport
            .only_request()
            .body
            .contains("<authenticateTestRequest")
    );
}

#[test]
fn builder_settings_reach_the_transport() {
    let transport = FakeTransport::answering(200, "authenticate_test_response.xml");
    let client = ClientBuilder::new(credentials(), transport.clone())
        .environment(Environment::Custom("https://example.test/api".into()))
        .timeout(Duration::from_secs(5))
        .user_agent("my-app/1.0")
        .build();
    block_on(client.misc().test_authenticate(&authenticate())).unwrap();

    let sent = transport.only_request();
    assert_eq!(sent.url, "https://example.test/api");
    assert_eq!(sent.timeout, Duration::from_secs(5));
    assert_eq!(sent.user_agent, "my-app/1.0");
}

#[test]
fn blocking_builder_settings_reach_the_transport() {
    let transport = FakeTransport::answering(200, "authenticate_test_response.xml");
    let client = blocking::ClientBuilder::new(credentials(), transport.clone())
        .environment(Environment::Production)
        .build();
    client.misc().test_authenticate(&authenticate()).unwrap();
    assert_eq!(
        transport.only_request().url,
        authorizenet::PRODUCTION_ENDPOINT
    );
}

#[test]
fn transport_errors_reach_the_caller() {
    let transport = FakeTransport::with(|| Err(TransportError::timeout("took too long")));
    let client = Client::with_transport(credentials(), transport);
    let result = block_on(client.misc().test_authenticate(&authenticate()));
    assert!(
        matches!(&result, Err(Error::Transport(e)) if e.is_timeout() && e.to_string() == "took too long"),
        "{result:?}"
    );

    let transport = FakeTransport::with(|| Err(TransportError::new("connection reset")));
    let client = blocking::Client::with_transport(credentials(), transport);
    let result = client.misc().test_authenticate(&authenticate());
    assert!(
        matches!(&result, Err(Error::Transport(e)) if !e.is_timeout()),
        "{result:?}"
    );
}

#[test]
fn responses_are_decoded_as_with_the_built_in_transports() {
    let client = Client::with_transport(
        credentials(),
        FakeTransport::answering(200, "error_response.xml"),
    );
    let result = block_on(client.misc().test_authenticate(&authenticate()));
    assert_eq!(
        result.unwrap_err().as_api().map(|e| e.code()),
        Some("E00035")
    );

    let client = blocking::Client::with_transport(
        credentials(),
        FakeTransport::answering(502, "error_response.xml"),
    );
    let result = client.misc().test_authenticate(&authenticate());
    assert!(
        matches!(result, Err(Error::Status { status: 502, .. })),
        "{result:?}"
    );
}

#[test]
fn futures_are_send_with_a_custom_transport() {
    fn assert_send<T: Send>(_: &T) {}
    let client = Client::with_transport(
        credentials(),
        FakeTransport::answering(200, "authenticate_test_response.xml"),
    );
    let request = authenticate();
    assert_send(&client.misc().test_authenticate(&request));
}

#[test]
fn clients_expose_their_transport() {
    let transport = FakeTransport::answering(200, "authenticate_test_response.xml");
    let client = Client::with_transport(credentials(), transport.clone());
    assert!(Arc::ptr_eq(&client.transport().sent, &transport.sent));
}
