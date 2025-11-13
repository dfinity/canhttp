//! Middleware to add a JSON translation layer (over HTTP).
//!
//! Transforms a low-level service that transmits bytes into one that transmits JSON payloads:
//!
//! ```text
//!                 │                     ▲              
//! http::Request<I>│                     │http::Response<O>
//!               ┌─┴─────────────────────┴───┐          
//!               │   JsonResponseConverter   │          
//!               └─┬─────────────────────▲───┘          
//!                 │                     │              
//!               ┌─▼─────────────────────┴───┐          
//!               │   JsonRequestConverter    │          
//!               └─┬─────────────────────┬───┘          
//!      HttpRequest│                     │HttpResponse
//!                 ▼                     │              
//!               ┌─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─┐
//!               │          SERVICE          │
//!               └─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─┘
//! ```
//! This can be used to transmit any kind of JSON payloads, such as JSON RPC over HTTP.
//!
//! # Examples
//!
//! A simple [`Service`] to make JSON requests and echo the request back:
//! ```rust
//! use canhttp::http::{HttpRequest, HttpResponse, json::JsonConversionLayer};
//! use tower::{Service, ServiceBuilder, ServiceExt, BoxError};
//! use serde_json::json;
//!
//! async fn echo_bytes(request: HttpRequest) -> Result<HttpResponse, BoxError> {
//!     Ok(http::Response::new(request.into_body()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut service = ServiceBuilder::new()
//!   .layer(JsonConversionLayer::<serde_json::Value, serde_json::Value>::new())
//!   .service_fn(echo_bytes);
//!
//! let request = http::Request::post("https://internetcomputer.org")
//!   .header("Content-Type", "application/json")
//!   .body(json!({"key": "value"}))
//!   .unwrap();
//!
//! let response = service.ready().await.unwrap().call(request).await.unwrap();
//!
//! assert_eq!(response.into_body()["key"], "value");
//! # Ok(())
//! # }
//! ```
//!
//! [`Service`]: tower::Service

use crate::{
    convert::{
        ConvertRequest, ConvertRequestLayer, ConvertResponse, ConvertResponseLayer,
        CreateResponseFilterLayer, FilterResponse,
    },
    http::{HttpConversionLayer, HttpRequestConverter, HttpResponseConverter},
};
pub use id::{ConstantSizeId, Id};
pub use request::{
    HttpJsonRpcRequest, JsonRequestConversionError, JsonRequestConverter, JsonRpcRequest,
};
pub use response::{
    ConsistentJsonRpcIdFilter, ConsistentResponseIdFilterError, CreateJsonRpcIdFilter,
    HttpJsonRpcResponse, JsonResponseConversionError, JsonResponseConverter, JsonRpcError,
    JsonRpcResponse, JsonRpcResult,
};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use tower_layer::{Layer, Stack};
pub use version::Version;

#[cfg(test)]
mod tests;

mod id;
mod request;
mod response;
mod version;

/// Middleware that combines [`JsonRequestConverter`] to convert requests
/// and [`JsonResponseConverter`] to convert responses to a [`Service`].
///
/// See the [module docs](crate::http::json) for an example.
///
/// [`Service`]: tower::Service
#[derive(Debug)]
pub struct JsonConversionLayer<I, O> {
    _marker: PhantomData<(I, O)>,
}

impl<I, O> JsonConversionLayer<I, O> {
    /// Returns a new [`JsonConversionLayer`].
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<I, O> Clone for JsonConversionLayer<I, O> {
    fn clone(&self) -> Self {
        Self {
            _marker: self._marker,
        }
    }
}

impl<I, O> Default for JsonConversionLayer<I, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, I, O> Layer<S> for JsonConversionLayer<I, O>
where
    I: Serialize,
    O: DeserializeOwned,
{
    type Service =
        ConvertResponse<ConvertRequest<S, JsonRequestConverter<I>>, JsonResponseConverter<O>>;

    fn layer(&self, inner: S) -> Self::Service {
        let stack = tower_layer::Stack::new(
            ConvertRequestLayer::new(JsonRequestConverter::<I>::new()),
            ConvertResponseLayer::new(JsonResponseConverter::<O>::new()),
        );
        stack.layer(inner)
    }
}

/// Middleware that combines a [`HttpConversionLayer`], a [`JsonConversionLayer`] to create
/// an JSON-RPC over HTTP [`Service`].
/// This middleware also contains a [`ConsistentJsonRpcIdFilter`] to ensure the responses
/// have a valid JSON-RPC ID that matches the request ID.
///
/// [`Service`]: tower::Service
#[derive(Debug, Default)]
pub struct JsonRpcHttpLayer<Params, Result> {
    _marker: PhantomData<(Params, Result)>,
}

impl<Params, Result> JsonRpcHttpLayer<Params, Result> {
    /// Returns a new [`JsonRpcHttpLayer`].
    pub fn new() -> Self {
        Self::new()
    }
}

impl<Params, Result> Clone for JsonRpcHttpLayer<Params, Result> {
    fn clone(&self) -> Self {
        Self {
            _marker: self._marker,
        }
    }
}

impl<Params, Result, S> Layer<S> for JsonRpcHttpLayer<Params, Result>
where
    Params: Serialize,
    Result: DeserializeOwned,
{
    type Service = FilterResponse<
        ConvertResponse<
            ConvertRequest<
                ConvertResponse<ConvertRequest<S, HttpRequestConverter>, HttpResponseConverter>,
                JsonRequestConverter<JsonRpcRequest<Params>>,
            >,
            JsonResponseConverter<JsonRpcResponse<Result>>,
        >,
        CreateJsonRpcIdFilter<Params, Result>,
    >;

    fn layer(&self, inner: S) -> Self::Service {
        stack(
            HttpConversionLayer,
            JsonConversionLayer::<JsonRpcRequest<Params>, JsonRpcResponse<Result>>::new(),
            CreateResponseFilterLayer::new(CreateJsonRpcIdFilter::new()),
        )
        .layer(inner)
    }
}

fn stack<L1, L2, L3>(l1: L1, l2: L2, l3: L3) -> Stack<L1, Stack<L2, L3>> {
    Stack::new(l1, Stack::new(l2, l3))
}
