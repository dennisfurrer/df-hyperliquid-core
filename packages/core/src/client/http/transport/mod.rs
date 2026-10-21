//! The [`Http`] wire and the [`Transport`] traits: any request out with its
//! method, headers and body, the JSON posts to `/info` and `/exchange` every
//! read and action goes through, and what `/exchange` answers.

pub mod answer;
pub mod calls;
pub mod wire;

pub use answer::{Accepted, ExchangeResponse, Status};
pub use calls::{ApiError, EXCHANGE, INFO, Transport, exchange, info, info_request, user_request};
pub use wire::{Http, Method, OFFLINE, Request, Response, TransportError};
