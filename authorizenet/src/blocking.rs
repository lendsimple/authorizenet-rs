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
use crate::protocol::CONTENT_TYPE;

/// Larger than any response: transaction lists hold at most 1000 transactions.
const MAX_RESPONSE_BYTES: u64 = 100 * 1024 * 1024;

/// A blocking Authorize.Net client, built on ureq.
///
/// Cloning is cheap and clones share one connection pool, so a client can be shared
/// across threads. Operations are grouped by resource:
///
/// ```no_run
/// use authorizenet::Credentials;
/// use authorizenet::blocking::Client;
/// use authorizenet::schema::AuthenticateTestRequest;
///
/// # fn run() -> Result<(), authorizenet::Error> {
/// let client = Client::new(Credentials::transaction_key("login id", "transaction key"))?;
/// let response = client.misc().test_authenticate(&AuthenticateTestRequest::default())?;
/// println!("{:?}", response.messages.result_code);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    agent: ureq::Agent,
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
            agent: None,
        }
    }

    /// Sends any request. The resource methods, such as
    /// [`transactions`](Self::transactions), call this.
    pub fn execute<R: ApiRequest>(&self, request: &R) -> Result<R::Response, Error> {
        let _span = request_span::<R>().entered();
        let config = &self.inner.config;
        let body = config.encode(request)?;
        let mut response = self
            .inner
            .agent
            .post(&config.endpoint)
            .header("Content-Type", CONTENT_TYPE)
            .config()
            .http_status_as_error(false)
            .timeout_global(Some(config.timeout))
            .user_agent(config.user_agent.as_str())
            .build()
            .send(body)
            .map_err(transport)?;
        let status = response.status().as_u16();
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(transport)?;
        config.decode::<R>(status, &bytes)
    }
}

fn transport(err: ureq::Error) -> Error {
    let timeout = matches!(err, ureq::Error::Timeout(_));
    TransportError::new(err, timeout).into()
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

/// Configures a blocking [`Client`].
#[derive(Debug)]
#[must_use]
pub struct ClientBuilder {
    config: Config,
    agent: Option<ureq::Agent>,
}

impl ClientBuilder {
    common_builder_methods!();

    /// Uses this ureq agent, for example to share its connection pool or set a proxy.
    /// The builder's timeout and user agent still apply to each request.
    pub fn agent(mut self, agent: ureq::Agent) -> Self {
        self.agent = Some(agent);
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        Ok(Client {
            inner: Arc::new(Inner {
                agent: self.agent.unwrap_or_else(ureq::Agent::new_with_defaults),
                config: self.config,
            }),
        })
    }
}

operation_table!(resource_handles, Client, blocking);
