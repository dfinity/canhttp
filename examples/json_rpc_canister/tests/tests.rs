use candid::Principal;
use test_fixtures::Setup;

#[test]
fn should_make_json_rpc_request() {
    let setup = Setup::new("json_rpc_canister");
    let json_rpc_canister = setup.canister();

    let json_rpc_request_result = json_rpc_canister.update_call::<_, u64>(
        Principal::anonymous(),
        "make_json_rpc_request",
        (),
    );

    assert!(json_rpc_request_result > 0);
}
