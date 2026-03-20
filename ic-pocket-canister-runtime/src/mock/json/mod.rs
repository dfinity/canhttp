#[cfg(test)]
mod tests;

use crate::mock::CanisterHttpRequestMatcher;
use canhttp::http::json::{ConstantSizeId, Id, JsonRpcRequest};
use pocket_ic::common::rest::{
    CanisterHttpHeader, CanisterHttpMethod, CanisterHttpReply, CanisterHttpRequest,
    CanisterHttpResponse,
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, str::FromStr};
use url::{Host, Url};

/// Matches the body of a single JSON-RPC request.
#[derive(Clone, Debug)]
pub struct SingleJsonRpcMatcher {
    method: String,
    id: Option<Id>,
    params: Option<Value>,
}

impl SingleJsonRpcMatcher {
    /// Create a [`SingleJsonRpcMatcher`] that matches only JSON-RPC requests with the given method.
    pub fn with_method(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            id: None,
            params: None,
        }
    }

    /// Mutates the [`SingleJsonRpcMatcher`] to match only requests whose JSON-RPC request ID is a
    /// [`ConstantSizeId`] with the given value.
    pub fn with_id(self, id: u64) -> Self {
        self.with_raw_id(Id::from(ConstantSizeId::from(id)))
    }

    /// Mutates the [`SingleJsonRpcMatcher`] to match only requests whose JSON-RPC request ID is an
    /// [`Id`] with the given value.
    pub fn with_raw_id(self, id: Id) -> Self {
        Self {
            id: Some(id),
            ..self
        }
    }

    /// Mutates the [`SingleJsonRpcMatcher`] to match only requests with the given JSON-RPC request
    /// parameters.
    pub fn with_params(self, params: impl Into<Value>) -> Self {
        Self {
            params: Some(params.into()),
            ..self
        }
    }

    fn matches_body(&self, request: &JsonRpcRequest<Value>) -> bool {
        if self.method != request.method() {
            return false;
        }
        if let Some(ref id) = self.id {
            if id != request.id() {
                return false;
            }
        }
        if let Some(ref params) = self.params {
            if Some(params) != request.params() {
                return false;
            }
        }
        true
    }
}

/// Matches [`CanisterHttpRequest`]s whose body can be deserialized and matched by `B`.
///
/// The type parameter `B` determines what kind of body is matched:
/// * [`SingleJsonRpcMatcher`] for single JSON-RPC requests (see [`JsonRpcRequestMatcher`])
/// * `Vec<SingleJsonRpcMatcher>` for batch JSON-RPC requests (see [`BatchJsonRpcRequestMatcher`])
#[derive(Clone, Debug)]
pub struct HttpRequestMatcher<B> {
    body: B,
    url: Option<Url>,
    host: Option<Host>,
    request_headers: Option<Vec<CanisterHttpHeader>>,
    max_response_bytes: Option<u64>,
}

/// Matches [`CanisterHttpRequest`]s whose body is a single JSON-RPC request.
pub type JsonRpcRequestMatcher = HttpRequestMatcher<SingleJsonRpcMatcher>;

impl<B> HttpRequestMatcher<B> {
    /// Mutates the matcher to match only requests with the given [URL].
    ///
    /// [URL]: https://internetcomputer.org/docs/references/ic-interface-spec#ic-http_request
    pub fn with_url(self, url: &str) -> Self {
        Self {
            url: Some(Url::parse(url).expect("BUG: invalid URL")),
            ..self
        }
    }

    /// Mutates the matcher to match only requests whose [URL] has the given host.
    ///
    /// [URL]: https://internetcomputer.org/docs/references/ic-interface-spec#ic-http_request
    pub fn with_host(self, host: &str) -> Self {
        Self {
            host: Some(Host::parse(host).expect("BUG: invalid host for a URL")),
            ..self
        }
    }

    /// Mutates the matcher to match requests with the given HTTP headers.
    pub fn with_request_headers(self, headers: Vec<(impl ToString, impl ToString)>) -> Self {
        Self {
            request_headers: Some(
                headers
                    .into_iter()
                    .map(|(name, value)| CanisterHttpHeader {
                        name: name.to_string(),
                        value: value.to_string(),
                    })
                    .collect(),
            ),
            ..self
        }
    }

    /// Mutates the matcher to match requests with the given [`max_response_bytes`].
    ///
    /// [`max_response_bytes`]: https://internetcomputer.org/docs/references/ic-interface-spec#ic-http_request
    pub fn with_max_response_bytes(self, max_response_bytes: impl Into<u64>) -> Self {
        Self {
            max_response_bytes: Some(max_response_bytes.into()),
            ..self
        }
    }

    fn matches_http(&self, request: &CanisterHttpRequest) -> bool {
        let req_url = Url::from_str(&request.url).expect("BUG: invalid URL");
        if let Some(ref mock_url) = self.url {
            if mock_url != &req_url {
                return false;
            }
        }
        if let Some(ref host) = self.host {
            match req_url.host() {
                Some(ref req_host) if req_host == host => {}
                _ => return false,
            }
        }
        if CanisterHttpMethod::POST != request.http_method {
            return false;
        }
        if let Some(ref headers) = self.request_headers {
            fn lower_case_header_name(
                CanisterHttpHeader { name, value }: &CanisterHttpHeader,
            ) -> CanisterHttpHeader {
                CanisterHttpHeader {
                    name: name.to_lowercase(),
                    value: value.clone(),
                }
            }
            let expected: BTreeSet<_> = headers.iter().map(lower_case_header_name).collect();
            let actual: BTreeSet<_> = request.headers.iter().map(lower_case_header_name).collect();
            if expected != actual {
                return false;
            }
        }
        if let Some(max_response_bytes) = self.max_response_bytes {
            if Some(max_response_bytes) != request.max_response_bytes {
                return false;
            }
        }
        true
    }
}

impl HttpRequestMatcher<SingleJsonRpcMatcher> {
    /// Create a [`JsonRpcRequestMatcher`] that matches only JSON-RPC requests with the given method.
    pub fn with_method(method: impl Into<String>) -> Self {
        Self {
            body: SingleJsonRpcMatcher::with_method(method),
            url: None,
            host: None,
            request_headers: None,
            max_response_bytes: None,
        }
    }

    /// Mutates the [`JsonRpcRequestMatcher`] to match only requests whose JSON-RPC request ID is a
    /// [`ConstantSizeId`] with the given value.
    pub fn with_id(self, id: u64) -> Self {
        Self {
            body: self.body.with_id(id),
            ..self
        }
    }

    /// Mutates the [`JsonRpcRequestMatcher`] to match only requests whose JSON-RPC request ID is an
    /// [`Id`] with the given value.
    pub fn with_raw_id(self, id: Id) -> Self {
        Self {
            body: self.body.with_raw_id(id),
            ..self
        }
    }

    /// Mutates the [`JsonRpcRequestMatcher`] to match only requests with the given JSON-RPC request
    /// parameters.
    pub fn with_params(self, params: impl Into<Value>) -> Self {
        Self {
            body: self.body.with_params(params),
            ..self
        }
    }
}

impl CanisterHttpRequestMatcher for HttpRequestMatcher<SingleJsonRpcMatcher> {
    fn matches(&self, request: &CanisterHttpRequest) -> bool {
        if !self.matches_http(request) {
            return false;
        }
        match serde_json::from_slice::<JsonRpcRequest<Value>>(&request.body) {
            Ok(actual_body) => self.body.matches_body(&actual_body),
            Err(_) => false,
        }
    }
}

/// Matches [`CanisterHttpRequest`]s whose body is a batch JSON-RPC request.
pub type BatchJsonRpcRequestMatcher = HttpRequestMatcher<Vec<SingleJsonRpcMatcher>>;

impl HttpRequestMatcher<Vec<SingleJsonRpcMatcher>> {
    /// Create a [`BatchJsonRpcRequestMatcher`] that matches a batch JSON-RPC request
    /// containing exactly the given individual matchers, matched pairwise in order.
    pub fn batch(matchers: Vec<SingleJsonRpcMatcher>) -> Self {
        Self {
            body: matchers,
            url: None,
            host: None,
            request_headers: None,
            max_response_bytes: None,
        }
    }
}

impl CanisterHttpRequestMatcher for HttpRequestMatcher<Vec<SingleJsonRpcMatcher>> {
    fn matches(&self, request: &CanisterHttpRequest) -> bool {
        if !self.matches_http(request) {
            return false;
        }
        match serde_json::from_slice::<Vec<JsonRpcRequest<Value>>>(&request.body) {
            Ok(actual_batch) => {
                actual_batch.len() == self.body.len()
                    && self
                        .body
                        .iter()
                        .zip(actual_batch.iter())
                        .all(|(matcher, req)| matcher.matches_body(req))
            }
            Err(_) => false,
        }
    }
}

/// A mocked HTTP outcall response.
///
/// The type parameter `B` determines what kind of body is returned:
/// * [`Value`] for single JSON-RPC responses (see [`JsonRpcResponse`])
/// * `Vec<Value>` for batch JSON-RPC responses (see [`BatchJsonRpcResponse`])
#[derive(Clone)]
pub struct HttpResponse<B> {
    status: u16,
    headers: Vec<CanisterHttpHeader>,
    body: B,
}

/// A mocked single JSON-RPC HTTP outcall response.
pub type JsonRpcResponse = HttpResponse<Value>;

/// A mocked batch JSON-RPC HTTP outcall response.
pub type BatchJsonRpcResponse = HttpResponse<Vec<Value>>;

impl<B: Serialize> From<HttpResponse<B>> for CanisterHttpResponse {
    fn from(response: HttpResponse<B>) -> Self {
        CanisterHttpResponse::CanisterHttpReply(CanisterHttpReply {
            status: response.status,
            headers: response.headers,
            body: serde_json::to_vec(&response.body).unwrap(),
        })
    }
}

impl From<Value> for HttpResponse<Value> {
    fn from(body: Value) -> Self {
        Self {
            status: 200,
            headers: vec![],
            body,
        }
    }
}

impl From<&Value> for HttpResponse<Value> {
    fn from(body: &Value) -> Self {
        Self::from(body.clone())
    }
}

impl From<String> for HttpResponse<Value> {
    fn from(body: String) -> Self {
        Self::from(Value::from_str(&body).expect("BUG: invalid JSON-RPC response"))
    }
}

impl From<&str> for HttpResponse<Value> {
    fn from(body: &str) -> Self {
        Self::from(body.to_string())
    }
}

impl HttpResponse<Value> {
    /// Mutates the response to set the given JSON-RPC response ID to a [`ConstantSizeId`] with the
    /// given value.
    pub fn with_id(self, id: u64) -> Self {
        self.with_raw_id(Id::from(ConstantSizeId::from(id)))
    }

    /// Mutates the response to set the given JSON-RPC response ID to the given [`Id`].
    pub fn with_raw_id(mut self, id: Id) -> Self {
        self.body["id"] = serde_json::to_value(id).expect("BUG: cannot serialize ID");
        self
    }
}

impl From<Vec<Value>> for HttpResponse<Vec<Value>> {
    fn from(body: Vec<Value>) -> Self {
        Self {
            status: 200,
            headers: vec![],
            body,
        }
    }
}
