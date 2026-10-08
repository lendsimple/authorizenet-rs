//! Merchant credentials and the API endpoints.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};

use crate::schema::{MerchantAuthentication, MerchantCredential};

/// How requests authenticate: the `merchantAuthentication` element of each request.
///
/// Secrets are held in [`SecretString`]s and never appear in `Debug` output.
///
/// ```
/// use authorizenet::Credentials;
///
/// let credentials = Credentials::transaction_key("5KP3u95bQpv", "346HZ32z3fP4hTG2");
/// assert!(!format!("{credentials:?}").contains("346HZ32z3fP4hTG2"));
/// ```
#[derive(Clone)]
pub struct Credentials {
    repr: Repr,
}

#[derive(Clone)]
enum Repr {
    Standard {
        login_id: Option<String>,
        secret: Secret,
        mobile_device_id: Option<String>,
    },
    Custom(Box<MerchantAuthentication>),
}

#[derive(Clone)]
enum Secret {
    TransactionKey(SecretString),
    SessionToken(SecretString),
    Password(SecretString),
    AccessToken(SecretString),
    ClientKey(String),
}

impl Secret {
    fn kind(&self) -> &'static str {
        match self {
            Secret::TransactionKey(_) => "transaction key",
            Secret::SessionToken(_) => "session token",
            Secret::Password(_) => "password",
            Secret::AccessToken(_) => "access token",
            Secret::ClientKey(_) => "client key",
        }
    }
}

impl Credentials {
    fn standard(login_id: Option<String>, secret: Secret) -> Self {
        Self {
            repr: Repr::Standard {
                login_id,
                secret,
                mobile_device_id: None,
            },
        }
    }

    /// An API login id and transaction key: the usual credentials of a server.
    pub fn transaction_key(
        login_id: impl Into<String>,
        transaction_key: impl Into<SecretString>,
    ) -> Self {
        Self::standard(
            Some(login_id.into()),
            Secret::TransactionKey(transaction_key.into()),
        )
    }

    /// An OAuth access token.
    pub fn access_token(token: impl Into<SecretString>) -> Self {
        Self::standard(None, Secret::AccessToken(token.into()))
    }

    /// The session token of a logged-in mobile device, which must also be identified
    /// with [`with_mobile_device_id`](Self::with_mobile_device_id).
    pub fn session_token(token: impl Into<SecretString>) -> Self {
        Self::standard(None, Secret::SessionToken(token.into()))
    }

    /// A login id and password, as used to log in mobile devices.
    pub fn password(login_id: impl Into<String>, password: impl Into<SecretString>) -> Self {
        Self::standard(Some(login_id.into()), Secret::Password(password.into()))
    }

    /// An API login id and public client key, as used by Accept.js and Accept Mobile.
    pub fn client_key(login_id: impl Into<String>, client_key: impl Into<String>) -> Self {
        Self::standard(Some(login_id.into()), Secret::ClientKey(client_key.into()))
    }

    /// Any `merchantAuthentication`, for the rarer forms such as impersonation or a
    /// fingerprint.
    pub fn from_merchant_authentication(auth: MerchantAuthentication) -> Self {
        Self {
            repr: Repr::Custom(Box::new(auth)),
        }
    }

    /// Identifies the mobile device the credentials belong to.
    pub fn with_mobile_device_id(mut self, id: impl Into<String>) -> Self {
        let id = id.into();
        match &mut self.repr {
            Repr::Standard {
                mobile_device_id, ..
            } => *mobile_device_id = Some(id),
            Repr::Custom(auth) => auth.mobile_device_id = Some(id),
        }
        self
    }

    /// The API login id, if the credentials have one.
    pub fn login_id(&self) -> Option<&str> {
        match &self.repr {
            Repr::Standard { login_id, .. } => login_id.as_deref(),
            Repr::Custom(auth) => auth.name.as_deref(),
        }
    }

    /// The `merchantAuthentication` element to send. This exposes the secret.
    pub(crate) fn to_merchant_authentication(&self) -> MerchantAuthentication {
        let (login_id, secret, mobile_device_id) = match &self.repr {
            Repr::Custom(auth) => return (**auth).clone(),
            Repr::Standard {
                login_id,
                secret,
                mobile_device_id,
            } => (login_id, secret, mobile_device_id),
        };
        let expose = |s: &SecretString| s.expose_secret().to_owned();
        let credential = match secret {
            Secret::TransactionKey(s) => MerchantCredential::TransactionKey(expose(s)),
            Secret::SessionToken(s) => MerchantCredential::SessionToken(expose(s)),
            Secret::Password(s) => MerchantCredential::Password(expose(s)),
            Secret::AccessToken(s) => MerchantCredential::AccessToken(expose(s)),
            Secret::ClientKey(key) => MerchantCredential::ClientKey(key.clone()),
        };
        MerchantAuthentication::builder()
            .maybe_name(login_id.clone())
            .credential(credential)
            .maybe_mobile_device_id(mobile_device_id.clone())
            .build()
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Standard {
                login_id,
                secret,
                mobile_device_id,
            } => f
                .debug_struct("Credentials")
                .field("kind", &secret.kind())
                .field("login_id", login_id)
                .field("mobile_device_id", mobile_device_id)
                .finish_non_exhaustive(),
            // MerchantAuthentication's Debug redacts its secrets.
            Repr::Custom(auth) => f.debug_tuple("Credentials").field(auth).finish(),
        }
    }
}

/// The sandbox endpoint, for testing with a sandbox account.
pub const SANDBOX_ENDPOINT: &str = "https://apitest.authorize.net/xml/v1/request.api";

/// The production endpoint.
pub const PRODUCTION_ENDPOINT: &str = "https://api.authorize.net/xml/v1/request.api";

/// Which Authorize.Net environment to send requests to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Environment {
    /// The sandbox, for testing. Sandbox credentials do not work in production.
    #[default]
    Sandbox,
    /// The production environment, which moves real money.
    Production,
    /// Another endpoint URL, such as a mock server in tests.
    Custom(String),
}

impl Environment {
    /// The URL requests are posted to.
    pub fn endpoint(&self) -> &str {
        match self {
            Environment::Sandbox => SANDBOX_ENDPOINT,
            Environment::Production => PRODUCTION_ENDPOINT,
            Environment::Custom(url) => url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_key_builds_merchant_authentication() {
        let auth = Credentials::transaction_key("login", "key").to_merchant_authentication();
        assert_eq!(auth.name.as_deref(), Some("login"));
        assert_eq!(
            auth.credential,
            Some(MerchantCredential::TransactionKey("key".into()))
        );
        assert_eq!(auth.mobile_device_id, None);
    }

    #[test]
    fn session_token_with_mobile_device() {
        let auth = Credentials::session_token("token")
            .with_mobile_device_id("device-1")
            .to_merchant_authentication();
        assert_eq!(auth.name, None);
        assert_eq!(
            auth.credential,
            Some(MerchantCredential::SessionToken("token".into()))
        );
        assert_eq!(auth.mobile_device_id.as_deref(), Some("device-1"));
    }

    #[test]
    fn custom_credentials_are_sent_as_given() {
        let auth = MerchantAuthentication::builder()
            .name("login")
            .credential(MerchantCredential::ClientKey("public".into()))
            .build();
        let credentials = Credentials::from_merchant_authentication(auth.clone());
        assert_eq!(credentials.to_merchant_authentication(), auth);
        assert_eq!(credentials.login_id(), Some("login"));
    }

    #[test]
    fn debug_shows_kind_and_login_but_not_secret() {
        let debug = format!("{:?}", Credentials::transaction_key("login", "s3cret"));
        assert!(!debug.contains("s3cret"), "{debug}");
        assert!(debug.contains("transaction key"), "{debug}");
        assert!(debug.contains("login"), "{debug}");
    }

    #[test]
    fn sandbox_is_the_default_environment() {
        assert_eq!(Environment::default().endpoint(), SANDBOX_ENDPOINT);
        assert_eq!(
            Environment::Custom("http://localhost:1234".into()).endpoint(),
            "http://localhost:1234"
        );
    }
}
