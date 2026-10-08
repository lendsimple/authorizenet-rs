//! The async client.

use std::fmt;
use std::sync::Arc;

use tracing::Instrument;

use crate::api::ApiRequest;
use crate::config::{Config, common_builder_methods, request_span};
use crate::credentials::Credentials;
use crate::error::Error;
use crate::operations::{operation_table, resource_handles};
#[cfg(feature = "reqwest")]
use crate::transport::ReqwestTransport;
use crate::transport::Transport;

/// An async Authorize.Net client.
///
/// It sends requests through a [`Transport`], by default a [`ReqwestTransport`] (with
/// the `reqwest` feature), which runs on tokio. Use
/// [`with_transport`](Client::with_transport) to send them with another HTTP client.
///
/// Cloning is cheap and clones share one transport, so a client can be shared across
/// tasks. Operations are grouped by resource:
///
/// ```no_run
/// # #[cfg(feature = "reqwest")]
/// # async fn run() -> Result<(), authorizenet::Error> {
/// use authorizenet::{Client, Credentials};
/// use authorizenet::schema::AuthenticateTestRequest;
///
/// let client = Client::new(Credentials::transaction_key("login id", "transaction key"));
/// let response = client.misc().test_authenticate(&AuthenticateTestRequest::default()).await?;
/// println!("{:?}", response.messages.result_code);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "reqwest")]
pub struct Client<T = ReqwestTransport> {
    inner: Arc<Inner<T>>,
}

/// An async Authorize.Net client.
///
/// It sends requests through a [`Transport`]; enable the `reqwest` feature for a
/// default one. Cloning is cheap and clones share one transport.
#[cfg(not(feature = "reqwest"))]
pub struct Client<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    transport: T,
    config: Config,
}

#[cfg(feature = "reqwest")]
impl Client {
    /// A client for the sandbox with default settings and a [`ReqwestTransport`].
    ///
    /// # Panics
    ///
    /// Like `reqwest::Client::new`, if the TLS backend cannot be initialized.
    pub fn new(credentials: Credentials) -> Self {
        Self::builder(credentials).build()
    }

    /// A builder to configure the environment, timeout and so on.
    pub fn builder(credentials: Credentials) -> ClientBuilder {
        ClientBuilder::new(credentials, ReqwestTransport::default())
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
    pub async fn execute<R: ApiRequest + Sync>(&self, request: &R) -> Result<R::Response, Error> {
        let config = &self.inner.config;
        async move {
            let body = config.encode(request)?;
            let response = self.inner.transport.send(config.http_request(body)).await?;
            config.decode::<R>(response.status, &response.body)
        }
        .instrument(request_span::<R>())
        .await
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

/// Configures a [`Client`].
#[cfg(feature = "reqwest")]
#[must_use]
pub struct ClientBuilder<T = ReqwestTransport> {
    config: Config,
    transport: T,
}

/// Configures a [`Client`].
#[cfg(not(feature = "reqwest"))]
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

operation_table!(resource_handles, Client, Transport, async);
