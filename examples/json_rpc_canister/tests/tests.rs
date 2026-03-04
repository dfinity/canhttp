use candid::{CandidType, Deserialize};
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
        .update_call::<_, SlotInfo>("make_batch_json_rpc_request", ())
        .await;

    assert!(result.slot > 0);
    // Solana public keys in base58 encoding are 32-44 characters depending on the key's
    // binary representation
    assert!((32..=44).contains(&result.leader.len()));
}

#[derive(CandidType, Deserialize)]
struct SlotInfo {
    slot: u64,
    leader: String,
}
