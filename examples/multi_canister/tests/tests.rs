use candid::Principal;
use test_fixtures::Setup;
use uuid::Uuid;

#[test]
fn should_make_parallel_http_requests() {
    let setup = Setup::new("multi_canister");
    let http_canister = setup.canister();

    let http_request_results = http_canister.update_call::<_, Vec<String>>(
        Principal::anonymous(),
        "make_parallel_http_requests",
        (),
    );

    for uuid in http_request_results {
        assert!(Uuid::parse_str(uuid.as_str()).is_ok());
    }
}
