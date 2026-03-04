use candid::{CandidType, Deserialize};
use regex_lite::Regex;
use test_fixtures::Setup;

/// See https://solana.com/developers/guides/advanced/exchange#basic-verification
static SOLANA_PUBKEY_REGEX: &str = r"^[1-9A-HJ-NP-Za-km-z]{32,44}$";

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
    assert!(Regex::new(SOLANA_PUBKEY_REGEX)
        .unwrap()
        .is_match(&result.leader));
}

#[derive(CandidType, Deserialize)]
struct SlotInfo {
    slot: u64,
    leader: String,
}
