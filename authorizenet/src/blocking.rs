//! The blocking client, which needs no async runtime.
//!
//! Do not call it from async code: it blocks the thread. In a tokio task, use the
//! async [`Client`](crate::Client), or wrap calls in `tokio::task::spawn_blocking`.

use std::fmt;
use std::sync::Arc;

use crate::api::ApiRequest;
use crate::config::{Config, common_builder_methods, request_span};
use crate::credentials::Credentials;
use crate::error::{Error, TransportError};
use crate::operations::{operation_table, resource_handles};
use crate::transport::{HttpRequest, HttpResponse};

/// Sends requests for the blocking [`Client`].
///
/// Return a [`TransportError`] only when no response was received (a connection, TLS
/// or timeout failure); an HTTP error status is a response. See
/// [`transport`](crate::transport) for the async counterpart and an example.
#[cfg(not(target_arch = "wasm32"))]
pub trait Transport: Send + Sync {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError>;
}

/// Sends requests for the blocking [`Client`].
///
/// On WebAssembly the transport need not be `Send` or `Sync`: there are no threads to
/// share it with, and host HTTP APIs (such as WASI's) are often single-threaded.
#[cfg(target_arch = "wasm32")]
pub trait Transport {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError>;
}

/// A blocking Authorize.Net client.
///
/// It sends requests through a [`Transport`], by default a [`UreqTransport`] (with the
/// `ureq` feature). Use [`with_transport`](Client::with_transport) to send them with
/// another HTTP client.
///
/// Cloning is cheap and clones share one transport, so a client can be shared across
/// threads. Operations are grouped by resource:
///
/// ```no_run
/// # #[cfg(feature = "ureq")]
/// # fn run() -> Result<(), authorizenet::Error> {
/// use authorizenet::Credentials;
/// use authorizenet::blocking::Client;
/// use authorizenet::schema::AuthenticateTestRequest;
///
/// let client = Client::new(Credentials::transaction_key("login id", "transaction key"));
/// let response = client.misc().test_authenticate(&AuthenticateTestRequest::default())?;
/// println!("{:?}", response.messages.result_code);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "ureq")]
pub struct Client<T = UreqTransport> {
    inner: Arc<Inner<T>>,
}

/// A blocking Authorize.Net client.
///
/// It sends requests through a [`Transport`]; enable the `ureq` feature for a default
/// one. Cloning is cheap and clones share one transport.
#[cfg(not(feature = "ureq"))]
pub struct Client<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    transport: T,
    config: Config,
}

#[cfg(feature = "ureq")]
impl Client {
    /// A client for the sandbox with default settings and a [`UreqTransport`].
    pub fn new(credentials: Credentials) -> Self {
        Self::builder(credentials).build()
    }

    /// A builder to configure the environment, timeout and so on.
    pub fn builder(credentials: Credentials) -> ClientBuilder {
        ClientBuilder::new(credentials, UreqTransport::default())
    }
}

impl<T: Transport> Client<T> {
    /// A client for the sandbox with default settings, sending requests through
    /// `transport`. Use [`ClientBuilder::new`] to change the settings.
    pub fn with_transport(credentials: Credentials, transport: T) -> Self {
        ClientBuilder::new(credentials, transport).build()
    }

    /// The transport requests are sent through.
    pub fn transport(&self) -> &T {
        &self.inner.transport
    }

    /// Sends any request. The resource methods, such as
    /// [`transactions`](Self::transactions), call this.
    pub fn execute<R: ApiRequest>(&self, request: &R) -> Result<R::Response, Error> {
        let _span = request_span::<R>().entered();
        let config = &self.inner.config;
        let body = config.encode(request)?;
        let response = self.inner.transport.send(config.http_request(body))?;
        config.decode::<R>(response.status, &response.body)
    }
}

impl<T> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

/// Configures a blocking [`Client`].
#[cfg(feature = "ureq")]
#[must_use]
pub struct ClientBuilder<T = UreqTransport> {
    config: Config,
    transport: T,
}

/// Configures a blocking [`Client`].
#[cfg(not(feature = "ureq"))]
#[must_use]
pub struct ClientBuilder<T> {
    config: Config,
    transport: T,
}

impl<T> ClientBuilder<T> {
    /// A builder for a client that sends requests through `transport`.
    pub fn new(credentials: Credentials, transport: T) -> Self {
        Self {
            config: Config::new(credentials),
            transport,
        }
    }

    common_builder_methods!();
}

impl<T: Transport> ClientBuilder<T> {
    pub fn build(self) -> Client<T> {
        Client {
            inner: Arc::new(Inner {
                transport: self.transport,
                config: self.config,
            }),
        }
    }
}

impl<T> fmt::Debug for ClientBuilder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// A [`Transport`] built on [ureq](https://docs.rs/ureq).
#[cfg(feature = "ureq")]
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
}

/// Larger than any response: transaction lists hold at most 1000 transactions.
#[cfg(feature = "ureq")]
const MAX_RESPONSE_BYTES: u64 = 100 * 1024 * 1024;

#[cfg(feature = "ureq")]
impl Default for UreqTransport {
    /// A transport with its own connection pool.
    fn default() -> Self {
        Self {
            agent: ureq::Agent::new_with_defaults(),
        }
    }
}

#[cfg(feature = "ureq")]
impl From<ureq::Agent> for UreqTransport {
    /// Uses an existing ureq agent, for example to share its connection pool or proxy
    /// settings.
    fn from(agent: ureq::Agent) -> Self {
        Self { agent }
    }
}

#[cfg(feature = "ureq")]
impl Transport for UreqTransport {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError> {
        let error = |err: ureq::Error| {
            if matches!(err, ureq::Error::Timeout(_)) {
                TransportError::timeout(err)
            } else {
                TransportError::new(err)
            }
        };
        let mut response = self
            .agent
            .post(request.url)
            .header("Content-Type", request.content_type)
            .config()
            .http_status_as_error(false)
            .timeout_global(Some(request.timeout))
            .user_agent(request.user_agent)
            .build()
            .send(request.body)
            .map_err(error)?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(error)?;
        Ok(HttpResponse::new(status, body))
    }
}

operation_table!(resource_handles, Client, Transport, blocking);
