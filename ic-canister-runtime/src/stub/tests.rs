use crate::{IcError, Runtime, StubRuntime};
use candid::{CandidType, Principal};
use serde::Deserialize;

const DEFAULT_PRINCIPAL: Principal = Principal::from_slice(&[0x9d, 0xf7, 0x01]);
const DEFAULT_METHOD: &str = "method";
const DEFAULT_ARGS: (&str,) = ("args",);

#[tokio::test]
#[should_panic(expected = "No available call response")]
async fn should_panic_if_no_more_stubs() {
    let runtime = StubRuntime::new();

    let _result: Result<MultiResult, IcError> = runtime
        .update_call(DEFAULT_PRINCIPAL, DEFAULT_METHOD, DEFAULT_ARGS, 0)
        .await;
}

#[tokio::test]
#[should_panic(expected = "Failed to decode Candid stub response")]
async fn should_panic_if_result_cannot_be_decoded() {
    let runtime = StubRuntime::new().add_stub_response("Hello, world!");

    let _result: Result<MultiResult, IcError> = runtime
        .update_call(DEFAULT_PRINCIPAL, DEFAULT_METHOD, DEFAULT_ARGS, 0)
        .await;
}

#[tokio::test]
async fn should_return_single_stub_response() {
    let expected = MultiResult::Consistent("Hello, world!".to_string());
    let runtime = StubRuntime::new().add_stub_response(expected.clone());

    let result: Result<MultiResult, IcError> = runtime
        .update_call(DEFAULT_PRINCIPAL, DEFAULT_METHOD, DEFAULT_ARGS, 0)
        .await;

    assert_eq!(result, Ok(expected));
}

#[tokio::test]
async fn should_return_multiple_stub_responses() {
    let expected1 = MultiResult::Consistent("Hello, world!".to_string());
    let expected2 = MultiResult::Inconsistent(vec![
        "Hello, world!".to_string(),
        "Goodbye, world!".to_string(),
    ]);
    let expected3 = 0_u128;
    let runtime = StubRuntime::new()
        .add_stub_response(expected1.clone())
        .add_stub_response(expected2.clone())
        .add_stub_response(expected3);

    let result1: Result<MultiResult, IcError> = runtime
        .update_call(DEFAULT_PRINCIPAL, DEFAULT_METHOD, DEFAULT_ARGS, 0)
        .await;
    assert_eq!(result1, Ok(expected1));

    let result2: Result<MultiResult, IcError> = runtime
        .update_call(DEFAULT_PRINCIPAL, DEFAULT_METHOD, DEFAULT_ARGS, 0)
        .await;
    assert_eq!(result2, Ok(expected2));

    let result3: Result<u128, IcError> = runtime
        .update_call(DEFAULT_PRINCIPAL, DEFAULT_METHOD, DEFAULT_ARGS, 0)
        .await;
    assert_eq!(result3, Ok(expected3));
}

#[derive(Clone, Debug, PartialEq, CandidType, Deserialize)]
enum MultiResult {
    Consistent(String),
    Inconsistent(Vec<String>),
}
