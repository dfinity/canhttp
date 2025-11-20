use candid::Principal;
use test_fixtures::Setup;

#[test]
fn should_make_http_post_request() {
    let setup = Setup::new("http_canister");
    let http_canister = setup.canister();

    let http_request_result = http_canister.update_call::<_, String>(
        Principal::anonymous(),
        "make_http_post_request",
        (),
    );

    assert!(http_request_result.contains("Hello, World!"));
    assert!(http_request_result.contains("\"X-Id\": \"42\""));
}
