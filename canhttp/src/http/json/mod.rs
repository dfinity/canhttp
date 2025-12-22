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
    BatchJsonRpcRequest, HttpBatchJsonRpcRequest, HttpJsonRpcRequest, JsonRequestConversionError,
    JsonRequestConverter, JsonRpcRequest,
};
pub use response::{
    BatchJsonRpcResponse, ConsistentJsonRpcIdFilter, ConsistentResponseIdFilterError,
    CreateJsonRpcIdFilter, HttpBatchJsonRpcResponse, HttpJsonRpcResponse,
    JsonResponseConversionError, JsonResponseConverter, JsonRpcError, JsonRpcResponse,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeSet;
use std::fmt::Debug;
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

/// Middleware that combines an [`HttpConversionLayer`] and a [`JsonConversionLayer`] to create
/// a JSON-RPC over HTTP [`Service`].
///
/// This middleware can be used either with regular JSON-RPC requests and responses (i.e.
/// [`JsonRpcRequest`] and [`JsonRpcResponse`]) or with batch JSON-RPC requests and responses
/// (i.e. [`BatchJsonRpcRequest`] and [`BatchJsonRpcResponse`]).
///
/// This middleware includes a [`ConsistentJsonRpcIdFilter`], which ensures that each response
/// carries a valid JSON-RPC ID matching the corresponding request ID. This guarantees that the
/// [`Service`] complies with the [JSON-RPC 2.0 specification].
///
/// [`Service`]: tower::Service
/// [JSON-RPC 2.0 specification]: https://www.jsonrpc.org/specification
#[derive(Debug)]
pub struct JsonRpcHttpLayer<Request, Response> {
    _marker: PhantomData<(Request, Response)>,
}

impl<Request, Response> JsonRpcHttpLayer<Request, Response> {
    /// Returns a new [`JsonRpcHttpLayer`].
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<Request, Response> Clone for JsonRpcHttpLayer<Request, Response> {
    fn clone(&self) -> Self {
        Self {
            _marker: self._marker,
        }
    }
}

impl<Request, Response> Default for JsonRpcHttpLayer<Request, Response> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Request, Response, S> Layer<S> for JsonRpcHttpLayer<Request, Response>
where
    (Request, Response): JsonRpcCall<Request, Response>,
    Request: Serialize,
    Response: DeserializeOwned,
{
    type Service = FilterResponse<
        ConvertResponse<
            ConvertRequest<
                ConvertResponse<ConvertRequest<S, HttpRequestConverter>, HttpResponseConverter>,
                JsonRequestConverter<Request>,
            >,
            JsonResponseConverter<Response>,
        >,
        CreateJsonRpcIdFilter<Request, Response>,
    >;

    fn layer(&self, inner: S) -> Self::Service {
        stack(
            HttpConversionLayer,
            JsonConversionLayer::<Request, Response>::new(),
            CreateResponseFilterLayer::new(CreateJsonRpcIdFilter::new()),
        )
        .layer(inner)
    }
}

fn stack<L1, L2, L3>(l1: L1, l2: L2, l3: L3) -> Stack<L1, Stack<L2, L3>> {
    Stack::new(l1, Stack::new(l2, l3))
}

/// Represents a JSON-RPC request/response pair and its ID semantics.
///
/// Defines the request and response types, the ID type, and how to generate
/// and verify that a response matches a request.
pub trait JsonRpcCall<Request, Response> {
    /// The type used to identify requests and responses.
    type Id: Debug;

    /// Returns the expected response ID for a given request.
    ///
    /// # Panics
    ///
    /// Panics if the request ID is [`Id::Null`], which indicates a notification
    /// (a request for which no response is expected).
    fn expected_response_id(request: &http::Request<Request>) -> Self::Id;

    /// Checks that a response has a consistent ID for the given request ID.
    ///
    /// Returns `Ok(())` if the response ID is consistent, or
    /// `ConsistentResponseIdFilterError` if it is not.
    fn has_consistent_response_id(
        request_id: &Self::Id,
        response: &http::Response<Response>,
    ) -> Result<(), ConsistentResponseIdFilterError>;
}

impl<Params, Result> JsonRpcCall<JsonRpcRequest<Params>, JsonRpcResponse<Result>>
    for (JsonRpcRequest<Params>, JsonRpcResponse<Result>)
{
    type Id = Id;

    fn expected_response_id(request: &HttpJsonRpcRequest<Params>) -> Self::Id {
        expected_response_id(request.body())
    }

    fn has_consistent_response_id(
        request_id: &Id,
        response: &HttpJsonRpcResponse<Result>,
    ) -> std::result::Result<(), ConsistentResponseIdFilterError> {
        let response_id = response.body().id();
        if request_id == response_id || should_have_null_id(response.body()) {
            Ok(())
        } else {
            Err(ConsistentResponseIdFilterError::InconsistentId {
                status: response.status().into(),
                request_id: request_id.clone(),
                response_id: response_id.clone(),
            })
        }
    }
}

impl<Params, Result> JsonRpcCall<BatchJsonRpcRequest<Params>, BatchJsonRpcResponse<Result>>
    for (BatchJsonRpcRequest<Params>, BatchJsonRpcResponse<Result>)
{
    type Id = BTreeSet<Id>;

    fn expected_response_id(requests: &HttpBatchJsonRpcRequest<Params>) -> Self::Id {
        requests
            .body()
            .iter()
            .map(expected_response_id)
            .collect::<BTreeSet<_>>()
    }

    fn has_consistent_response_id(
        request_ids: &Self::Id,
        responses: &HttpBatchJsonRpcResponse<Result>,
    ) -> std::result::Result<(), ConsistentResponseIdFilterError> {
        let expected_missing_id_count = responses
            .body()
            .iter()
            .filter(|response| should_have_null_id(response))
            .count();

        let response_ids = responses
            .body()
            .iter()
            .map(|response| response.id())
            .collect::<BTreeSet<_>>();

        let missing_id_count = request_ids
            .iter()
            .filter(|id| !response_ids.contains(id))
            .count();

        let unexpected_id_count = response_ids
            .iter()
            .filter(|id| !request_ids.contains(id))
            .count();

        if (unexpected_id_count == 0) && (missing_id_count <= expected_missing_id_count) {
            Ok(())
        } else {
            Err(ConsistentResponseIdFilterError::InconsistentBatchIds {
                status: responses.status().into(),
                request_ids: request_ids.clone(),
                response_ids: response_ids.into_iter().cloned().collect(),
            })
        }
    }
}

// From the [JSON-RPC specification](https://www.jsonrpc.org/specification):
// If there was an error in detecting the id in the Request object
// (e.g. Parse error/Invalid Request), it MUST be Null.
fn should_have_null_id<T>(response: &JsonRpcResponse<T>) -> bool {
    let (response_id, result) = response.as_parts();
    response_id.is_null() && result.is_err_and(|e| e.is_parse_error() || e.is_invalid_request())
}

fn expected_response_id<T>(request: &JsonRpcRequest<T>) -> Id {
    match request.id() {
        Id::Null => panic!("ERROR: a null request ID is a notification that indicates that the client is not interested in the response."),
        id @ (Id::Number(_) | Id::String(_)) => id.clone()
    }
}
