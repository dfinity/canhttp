//! Library to mock HTTP outcalls on the Internet Computer leveraging the [`ic_canister_runtime`]
//! crate's [`Runtime`] trait as well as [`PocketIc`].

#![forbid(unsafe_code)]
#![forbid(missing_docs)]

mod mock;

use async_trait::async_trait;
use candid::{decode_one, encode_args, utils::ArgumentEncoder, CandidType, Principal};
use ic_canister_runtime::{IcError, Runtime};
use ic_cdk::call::{CallFailed, CallRejected};
use ic_error_types::RejectCode;
pub use mock::{
    json::{JsonRpcRequestMatcher, JsonRpcResponse},
    AnyCanisterHttpRequestMatcher, CanisterHttpReject, CanisterHttpReply, CanisterHttpRequestMatcher,
    MockHttpOutcall, MockHttpOutcallBuilder, MockHttpOutcalls, MockHttpOutcallsBuilder,
};
use pocket_ic::{
    common::rest::{CanisterHttpRequest, CanisterHttpResponse, MockCanisterHttpResponse},
    nonblocking::PocketIc,
    RejectResponse,
};
use serde::de::DeserializeOwned;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const DEFAULT_MAX_RESPONSE_BYTES: u64 = 2_000_000;
const MAX_TICKS: usize = 10;

/// [`Runtime`] using [`PocketIc`] to mock HTTP outcalls.
///
/// This runtime allows making calls to canisters through Pocket IC while verifying the HTTP
/// outcalls made and mocking their responses.
///
/// # Examples
/// Call the `make_http_post_request` endpoint on the example [`http_canister`] deployed with
/// Pocket IC and mock the resulting HTTP outcall.
/// ```rust, no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use ic_mock_http_canister_runtime::{
///     AnyCanisterHttpRequestMatcher, CanisterHttpReply, MockHttpOutcallsBuilder,
///     MockHttpRuntime
/// };
/// # use candid::Principal;
/// # use ic_canister_runtime::{Runtime, StubRuntime};
/// # use pocket_ic::nonblocking::PocketIc;
/// # use std::{sync::Arc, mem::MaybeUninit};
///
/// # let pocket_ic: Arc<PocketIc> = unsafe { Arc::new(unsafe { MaybeUninit::zeroed().assume_init() }) };
/// let mocks = MockHttpOutcallsBuilder::new()
///     .given(AnyCanisterHttpRequestMatcher)
///     .respond_with(
///         CanisterHttpReply::with_status(200)
///             .with_body(r#"{"data": "Hello, World!", "headers": {"X-Id": "42"}}"#)
///     );
///
/// let runtime = MockHttpRuntime::new(pocket_ic, Principal::anonymous(), mocks);
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
pub struct MockHttpRuntime {
    env: Arc<PocketIc>,
    caller: Principal,
    mocks: Mutex<MockHttpOutcalls>,
}

impl MockHttpRuntime {
    /// Create a new [`MockHttpRuntime`] with the given [`PocketIc`] and [`MockHttpOutcalls`].
    /// All calls to canisters are made using the given caller identity.
    pub fn new(env: Arc<PocketIc>, caller: Principal, mocks: impl Into<MockHttpOutcalls>) -> Self {
        Self {
            env,
            caller,
            mocks: Mutex::new(mocks.into()),
        }
    }
}

#[async_trait]
impl Runtime for MockHttpRuntime {
    async fn update_call<In, Out>(
        &self,
        id: Principal,
        method: &str,
        args: In,
        _cycles: u128,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        let message_id = self
            .env
            .submit_call(
                id,
                self.caller,
                method,
                encode_args(args).unwrap_or_else(panic_when_encode_fails),
            )
            .await
            .unwrap();
        self.execute_mocks().await;
        self.env
            .await_call(message_id)
            .await
            .map(decode_call_response)
            .map_err(parse_reject_response)?
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
        self.env
            .query_call(
                id,
                self.caller,
                method,
                encode_args(args).unwrap_or_else(panic_when_encode_fails),
            )
            .await
            .map(decode_call_response)
            .map_err(parse_reject_response)?
    }
}

impl MockHttpRuntime {
    async fn execute_mocks(&self) {
        loop {
            let pending_requests = tick_until_http_requests(self.env.as_ref()).await;
            if let Some(request) = pending_requests.first() {
                let maybe_mock = {
                    let mut mocks = self.mocks.lock().unwrap();
                    mocks.pop_matching(request)
                };
                match maybe_mock {
                    Some(mock) => {
                        let mock_response = MockCanisterHttpResponse {
                            subnet_id: request.subnet_id,
                            request_id: request.request_id,
                            response: check_response_size(request, mock.response),
                            additional_responses: vec![],
                        };
                        self.env.mock_canister_http_response(mock_response).await;
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

fn panic_when_encode_fails(err: candid::error::Error) -> Vec<u8> {
    panic!("failed to encode args: {err}")
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
