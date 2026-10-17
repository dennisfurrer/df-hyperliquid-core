//! HTTP to `/info` and `/exchange`: the [`Http`] wire, the [`Transport`] every read and action
//! goes through, what `/exchange` answers, the signed body it takes, and the rate limit.

mod api;
pub mod body;
pub mod limit;
pub mod transport;

pub(crate) use api::Api;
pub use limit::{Kind, Spent};
pub use transport::{
    Accepted, ApiError, EXCHANGE, ExchangeResponse, Http, INFO, Request, Response, Status, Transport, TransportError,
    exchange, info, info_request, user_request,
};
