use test_fixtures::Setup;

#[tokio::test]
async fn should_make_json_rpc_request() {
    let setup = Setup::new("json_rpc_canister");

    let json_rpc_request_result = setup
        .canister()
        .update_call::<_, u64>("make_json_rpc_request", ())
        .await;

    assert!(json_rpc_request_result > 0);
}
