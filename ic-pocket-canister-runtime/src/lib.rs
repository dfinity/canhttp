//! Library to mock HTTP outcalls on the Internet Computer leveraging the [`ic_canister_runtime`]
//! crate's [`Runtime`] trait as well as [`PocketIc`].

#![forbid(unsafe_code)]
#![forbid(missing_docs)]

mod mock;
mod proxy;

use async_trait::async_trait;
use candid::{decode_one, encode_args, utils::ArgumentEncoder, CandidType, Principal};
use ic_canister_runtime::{IcError, Runtime};
use ic_cdk::call::{CallFailed, CallRejected};
use ic_error_types::RejectCode;
pub use mock::{
    json::{
        BatchJsonRpcRequestMatcher, BatchJsonRpcResponse, JsonRpcHttpRequestMatcher,
        JsonRpcHttpResponse, JsonRpcRequestMatcher, JsonRpcResponse, SingleJsonRpcMatcher,
    },
    AnyCanisterHttpRequestMatcher, CanisterHttpReject, CanisterHttpReply,
    CanisterHttpRequestMatcher, MockHttpOutcall, MockHttpOutcallBuilder, MockHttpOutcalls,
    MockHttpOutcallsBuilder,
};
use pocket_ic::{
    common::rest::{CanisterHttpRequest, CanisterHttpResponse, MockCanisterHttpResponse},
    nonblocking::PocketIc,
    RejectResponse,
};
use serde::de::DeserializeOwned;
use std::time::Duration;
use tokio::sync::Mutex;

const DEFAULT_MAX_RESPONSE_BYTES: u64 = 2_000_000;
const MAX_TICKS: usize = 10;

/// [`Runtime`] using [`PocketIc`] to make calls to canisters.
///
/// # Examples
/// Call the `make_http_post_request` endpoint on the example [`http_canister`] deployed with
/// Pocket IC and mock the resulting HTTP outcall.
/// ```rust, no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ic_canister_runtime::Runtime;
/// use ic_pocket_canister_runtime::{
///     AnyCanisterHttpRequestMatcher, CanisterHttpReply, MockHttpOutcallsBuilder,
///     PocketIcRuntime
/// };
/// use pocket_ic::nonblocking::PocketIc;
/// # use candid::Principal;
///
/// let mocks = MockHttpOutcallsBuilder::new()
///     .given(AnyCanisterHttpRequestMatcher)
///     .respond_with(
///         CanisterHttpReply::with_status(200)
///             .with_body(r#"{"data": "Hello, World!", "headers": {"X-Id": "42"}}"#)
///     );
///
/// let pocket_ic = PocketIc::new().await;
/// let runtime = PocketIcRuntime::new(&pocket_ic, Principal::anonymous())
///     .with_http_mocks(mocks.build());
/// # let canister_id = Principal::anonymous();
///
/// let http_request_result: String = runtime
///     .update_call(canister_id, "make_http_post_request", (), 0)
///     .await
///     .expect("Call to `http_canister` failed");
///
/// assert!(http_request_result.contains("Hello, World!"));
/// assert!(http_request_result.contains("\"X-Id\": \"42\""));
/// # Ok(())
/// # }
/// ```
///
/// [`http_canister`]: https://github.com/dfinity/canhttp/tree/main/examples/http_canister/
pub struct PocketIcRuntime<'a> {
    env: &'a PocketIc,
    caller: Principal,
    // The mocks are stored in a Mutex<Box<?>> so they can be modified in the implementation of
    // the `Runtime::update_call` method using interior mutability.
    // This is necessary since `Runtime::update_call` takes an immutable reference to the runtime.
    mocks: Option<Mutex<Box<dyn ExecuteHttpOutcallMocks>>>,
    proxy_canister_id: Option<Principal>,
}

impl<'a> PocketIcRuntime<'a> {
    /// Create a new [`PocketIcRuntime`] with the given [`PocketIc`].
    /// All calls to canisters are made using the given caller identity.
    pub fn new(env: &'a PocketIc, caller: Principal) -> Self {
        Self {
            env,
            caller,
            mocks: None,
            proxy_canister_id: None,
        }
    }

    /// Mock HTTP outcalls and their responses.
    ///
    /// This allows making calls to canisters through Pocket IC while verifying the HTTP outcalls
    /// made and mocking their responses.
    ///
    /// # Examples
    /// Call the `make_http_post_request` endpoint on the example [`http_canister`] deployed with
    /// Pocket IC and mock the resulting HTTP outcall.
    /// ```rust, no_run
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use ic_canister_runtime::Runtime;
    /// use ic_pocket_canister_runtime::{
    ///     AnyCanisterHttpRequestMatcher, CanisterHttpReply, MockHttpOutcallsBuilder,
    ///     PocketIcRuntime
    /// };
    /// use pocket_ic::nonblocking::PocketIc;
    /// # use candid::Principal;
    ///
    /// let mocks = MockHttpOutcallsBuilder::new()
    ///     // Matches any HTTP outcall request
    ///     .given(AnyCanisterHttpRequestMatcher)
    ///     // Assert that the HTTP outcall response has the given status code and body
    ///     .respond_with(
    ///         CanisterHttpReply::with_status(200)
    ///             .with_body(r#"{"data": "Hello, World!", "headers": {"X-Id": "42"}}"#)
    ///     );
    ///
    /// let pocket_ic = PocketIc::new().await;
    /// let runtime = PocketIcRuntime::new(&pocket_ic, Principal::anonymous())
    ///     .with_http_mocks(mocks.build());
    /// # let canister_id = Principal::anonymous();
    ///
    /// let http_request_result: String = runtime
    ///     .update_call(canister_id, "make_http_post_request", (), 0)
    ///     .await
    ///     .expect("Call to `http_canister` failed");
    ///
    /// assert!(http_request_result.contains("Hello, World!"));
    /// assert!(http_request_result.contains("\"X-Id\": \"42\""));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`http_canister`]: https://github.com/dfinity/canhttp/tree/main/examples/http_canister/
    pub fn with_http_mocks(mut self, mocks: impl ExecuteHttpOutcallMocks + 'static) -> Self {
        self.mocks = Some(Mutex::new(Box::new(mocks)));
        self
    }

    /// Route update calls through a [proxy canister](https://github.com/dfinity/proxy-canister)
    /// to attach cycles to them.
    ///
    /// When a proxy canister is configured, all `update_call` requests are forwarded through
    /// the proxy canister, which attaches the specified cycles before calling the target canister.
    /// Query calls are not affected and go directly to the target canister.
    pub fn with_proxy_canister(mut self, proxy_canister_id: Principal) -> Self {
        self.proxy_canister_id = Some(proxy_canister_id);
        self
    }

    async fn submit_and_await_call<In>(
        &self,
        id: Principal,
        method: &str,
        args: In,
    ) -> Result<Vec<u8>, IcError>
    where
        In: ArgumentEncoder + Send,
    {
        let message_id = self
            .env
            .submit_call(id, self.caller, method, encode_args_or_panic(args))
            .await
            .map_err(parse_reject_response)?;
        if let Some(mock) = &self.mocks {
            mock.try_lock()
                .unwrap()
                .execute_http_outcall_mocks(self.env)
                .await;
        }
        if self.env.auto_progress_enabled().await {
            self.env.await_call_no_ticks(message_id).await
        } else {
            self.env.await_call(message_id).await
        }
        .map_err(parse_reject_response)
    }
}

impl<'a> AsRef<PocketIc> for PocketIcRuntime<'a> {
    fn as_ref(&self) -> &'a PocketIc {
        self.env
    }
}

#[async_trait]
impl Runtime for PocketIcRuntime<'_> {
    async fn update_call<In, Out>(
        &self,
        id: Principal,
        method: &str,
        args: In,
        cycles: u128,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        let bytes = match self.proxy_canister_id {
            Some(proxy_id) => {
                let proxy_args = proxy::ProxyArgs::new(id, method, args, cycles);
                let response = self
                    .submit_and_await_call(proxy_id, "proxy", (proxy_args,))
                    .await?;
                proxy::decode_response(response)?
            }
            None => self.submit_and_await_call(id, method, args).await?,
        };
        decode_call_response(bytes)
    }

    async fn query_call<In, Out>(
        &self,
        id: Principal,
        method: &str,
        args: In,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        let bytes = self
            .env
            .query_call(id, self.caller, method, encode_args_or_panic(args))
            .await
            .map_err(parse_reject_response)?;
        decode_call_response(bytes)
    }
}

/// Execute HTTP outcall mocks.
#[async_trait]
pub trait ExecuteHttpOutcallMocks: Send + Sync {
    /// Execute HTTP outcall mocks.
    async fn execute_http_outcall_mocks(&mut self, runtime: &PocketIc) -> ();
}

#[async_trait]
impl ExecuteHttpOutcallMocks for MockHttpOutcalls {
    async fn execute_http_outcall_mocks(&mut self, env: &PocketIc) -> () {
        loop {
            let pending_requests = tick_until_http_requests(env).await;
            if let Some(request) = pending_requests.first() {
                let maybe_mock = { self.pop_matching(request) };
                match maybe_mock {
                    Some(mock) => {
                        let mock_response = MockCanisterHttpResponse {
                            subnet_id: request.subnet_id,
                            request_id: request.request_id,
                            response: check_response_size(request, mock.response),
                            additional_responses: vec![],
                        };
                        env.mock_canister_http_response(mock_response).await;
                    }
                    None => {
                        panic!("No mocks matching the request: {:?}", request);
                    }
                }
            } else {
                return;
            }
        }
    }
}

fn check_response_size(
    request: &CanisterHttpRequest,
    response: CanisterHttpResponse,
) -> CanisterHttpResponse {
    if let CanisterHttpResponse::CanisterHttpReply(reply) = &response {
        let max_response_bytes = request
            .max_response_bytes
            .unwrap_or(DEFAULT_MAX_RESPONSE_BYTES);
        if reply.body.len() as u64 > max_response_bytes {
            // Approximate replica behavior since headers are not accounted for.
            return CanisterHttpResponse::CanisterHttpReject(
                pocket_ic::common::rest::CanisterHttpReject {
                    reject_code: RejectCode::SysFatal as u64,
                    message: format!("Http body exceeds size limit of {max_response_bytes} bytes.",),
                },
            );
        }
    }
    response
}

fn parse_reject_response(response: RejectResponse) -> IcError {
    CallFailed::CallRejected(CallRejected::with_rejection(
        response.reject_code as u32,
        response.reject_message,
    ))
    .into()
}

fn decode_call_response<Out>(bytes: Vec<u8>) -> Result<Out, IcError>
where
    Out: CandidType + DeserializeOwned,
{
    decode_one(&bytes).map_err(|e| IcError::CandidDecodeFailed {
        message: e.to_string(),
    })
}

fn encode_args_or_panic<Tuple: ArgumentEncoder>(arguments: Tuple) -> Vec<u8> {
    encode_args(arguments).unwrap_or_else(|e| panic!("failed to encode args: {e}"))
}

async fn tick_until_http_requests(env: &PocketIc) -> Vec<CanisterHttpRequest> {
    let mut requests = Vec::new();
    for _ in 0..MAX_TICKS {
        requests = env.get_canister_http().await;
        if !requests.is_empty() {
            break;
        }
        env.tick().await;
        env.advance_time(Duration::from_nanos(1)).await;
    }
    requests
}
