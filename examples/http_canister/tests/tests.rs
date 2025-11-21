use test_fixtures::Setup;

#[tokio::test]
async fn should_make_http_post_request() {
    let setup = Setup::new("http_canister");

    let http_request_result = setup
        .canister()
        .update_call::<_, String>("make_http_post_request", ())
        .await;

    assert!(http_request_result.contains("Hello, World!"));
    assert!(http_request_result.contains("\"X-Id\": \"42\""));
}
