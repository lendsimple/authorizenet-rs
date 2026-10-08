//! The async client.

use std::fmt;
use std::sync::Arc;

use tracing::Instrument;

use crate::api::ApiRequest;
use crate::config::{Config, common_builder_methods, request_span};
use crate::credentials::Credentials;
use crate::error::{Error, TransportError};
use crate::operations::{operation_table, resource_handles};
use crate::protocol::CONTENT_TYPE;

/// An async Authorize.Net client, built on reqwest (and so on tokio).
///
/// Cloning is cheap and clones share one connection pool, so a client can be shared
/// across tasks. Operations are grouped by resource:
///
/// ```no_run
/// use authorizenet::{Client, Credentials};
/// use authorizenet::schema::AuthenticateTestRequest;
///
/// # async fn run() -> Result<(), authorizenet::Error> {
/// let client = Client::new(Credentials::transaction_key("login id", "transaction key"))?;
/// let response = client.misc().test_authenticate(&AuthenticateTestRequest::default()).await?;
/// println!("{:?}", response.messages.result_code);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    config: Config,
}

impl Client {
    /// A client for the sandbox with default settings.
    pub fn new(credentials: Credentials) -> Result<Self, Error> {
        Self::builder(credentials).build()
    }

    /// A builder to configure the environment, timeout and so on.
    pub fn builder(credentials: Credentials) -> ClientBuilder {
        ClientBuilder {
            config: Config::new(credentials),
            http: None,
        }
    }

    /// Sends any request. The resource methods, such as
    /// [`transactions`](Self::transactions), call this.
    pub async fn execute<R: ApiRequest + Sync>(&self, request: &R) -> Result<R::Response, Error> {
        let config = &self.inner.config;
        async move {
            let body = config.encode(request)?;
            let response = self
                .inner
                .http
                .post(&config.endpoint)
                .header(reqwest::header::CONTENT_TYPE, CONTENT_TYPE)
                .header(reqwest::header::USER_AGENT, &config.user_agent)
                .timeout(config.timeout)
                .body(body)
                .send()
                .await
                .map_err(transport)?;
            let status = response.status().as_u16();
            let bytes = response.bytes().await.map_err(transport)?;
            config.decode::<R>(status, &bytes)
        }
        .instrument(request_span::<R>())
        .await
    }
}

fn transport(err: reqwest::Error) -> Error {
    let timeout = err.is_timeout();
    TransportError::new(err, timeout).into()
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

/// Configures a [`Client`].
#[derive(Debug)]
#[must_use]
pub struct ClientBuilder {
    config: Config,
    http: Option<reqwest::Client>,
}

impl ClientBuilder {
    common_builder_methods!();

    /// Uses this reqwest client, for example to share its connection pool or set a
    /// proxy. The builder's timeout and user agent still apply to each request.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder()
                .build()
                .map_err(|e| TransportError::new(e, false))?,
        };
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                config: self.config,
            }),
        })
    }
}

operation_table!(resource_handles, Client, async);
