use test_fixtures::Setup;

#[tokio::test]
async fn should_make_json_rpc_request() {
    let setup = Setup::new("json_rpc_canister").await;

    let result = setup
        .canister()
        .update_call::<_, u64>("make_json_rpc_request", ())
        .await;

    assert!(result > 0);
}

#[tokio::test]
async fn should_make_batch_json_rpc_request() {
    let setup = Setup::new("json_rpc_canister").await;

    let result = setup
        .canister()
        .update_call::<_, Vec<u64>>("make_batch_json_rpc_request", ())
        .await;

    for value in result {
        assert!(value > 0);
    }
}
