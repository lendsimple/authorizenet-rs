//! What ties a request type to its response type.

use std::fmt;

use crate::schema::{AnyResponse, Messages};
use crate::validate::Validate;
use crate::xml::{XmlRead, XmlRoot};

/// An API request: a document the client sends, paired with the response it gets.
///
/// Implemented for every request type by the crate's operation table.
pub trait ApiRequest: XmlRoot + Validate {
    /// The response document this request is answered with.
    type Response: ApiResponse;

    /// The operation's name in the client, such as `transactions.create`, for logs.
    const OPERATION: &'static str;
}

/// An API response document. Every response carries `messages`, whose result code
/// says whether the request succeeded.
pub trait ApiResponse:
    XmlRoot
    + XmlRead
    + Clone
    + fmt::Debug
    + Into<AnyResponse>
    + TryFrom<AnyResponse, Error = AnyResponse>
{
    /// The overall result, with one or more result codes and messages.
    fn messages(&self) -> &Messages;

    /// The response inside `any`, if it is of this type.
    #[doc(hidden)]
    fn from_any(any: &AnyResponse) -> Option<&Self>;
}
