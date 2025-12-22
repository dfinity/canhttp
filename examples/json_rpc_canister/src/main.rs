//! Example of a canister using `canhttp` to issue JSON-RPC HTTP requests.
use canhttp::{
    cycles::{ChargeMyself, CyclesAccountingServiceBuilder},
    http::json::{
        HttpBatchJsonRpcResponse, HttpJsonRpcResponse, Id, JsonRpcHttpLayer, JsonRpcPayload,
        JsonRpcRequest,
    },
    observability::ObservabilityLayer,
    Client,
};
use ic_cdk::update;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;
use std::fmt::Debug;
use tower::{BoxError, Service, ServiceBuilder, ServiceExt};

/// Make a JSON-RPC request to the Solana JSON-RPC API.
#[update]
pub async fn make_json_rpc_request() -> u64 {
    const ID: Id = Id::Number(999);

    // Send a [`getSlot`](https://solana.com/docs/rpc/http/getslot) JSON-RPC request that fetches
    // the current height of the Solana blockchain
    let request = http::Request::post(solana_test_validator_base_url())
        .header("Content-Type", "application/json")
        .body(JsonRpcRequest::new("getSlot", json!([{"commitment": "finalized"}])).with_id(ID))
        .unwrap();

    let response: HttpJsonRpcResponse<u64> = client()
        .ready()
        .await
        .expect("Client should be ready")
        .call(request)
        .await
        .expect("Request should succeed");
    assert_eq!(response.status(), http::StatusCode::OK);

    let (id, result) = response.into_body().into_parts();
    assert_eq!(id, ID);

    result.expect("JSON-RPC API call should succeed")
}

/// Make a batch JSON-RPC request to the Solana JSON-RPC API.
#[update]
pub async fn make_batch_json_rpc_request() -> Vec<u64> {
    // Send [`getSlot`](https://solana.com/docs/rpc/http/getslot) JSON-RPC requests that fetch
    // the current height of the Solana blockchain with different commitment requirements.
    let requests = http::Request::post(solana_test_validator_base_url())
        .header("Content-Type", "application/json")
        .body(vec![
            JsonRpcRequest::new("getSlot", json!([{"commitment": "finalized"}])).with_id(0_u64),
            JsonRpcRequest::new("getSlot", json!([{"commitment": "confirmed"}])).with_id(1_u64),
            JsonRpcRequest::new("getSlot", json!([{"commitment": "processed"}])).with_id(2_u64),
        ])
        .unwrap();

    let response: HttpBatchJsonRpcResponse<u64> = client()
        .ready()
        .await
        .expect("Client should be ready")
        .call(requests)
        .await
        .expect("Request should succeed");
    assert_eq!(response.status(), http::StatusCode::OK);

    response
        .into_body()
        .into_iter()
        .zip(0_u64..)
        .map(|(response, expected_id)| {
            let (id, result) = response.into_parts();
            assert_eq!(id, expected_id.into());
            result.expect("JSON-RPC API call should succeed")
        })
        .collect()
}

fn client<Request, Response>(
) -> impl Service<http::Request<Request>, Response = http::Response<Response>, Error = BoxError>
where
    (): JsonRpcPayload<Request, Response>,
    Request: Debug + Serialize,
    Response: Debug + DeserializeOwned,
{
    ServiceBuilder::new()
        .layer(
            ObservabilityLayer::new()
                .on_request(|request: &http::Request<Request>| ic_cdk::println!("{request:?}"))
                .on_response(|_, response: &http::Response<Response>| {
                    ic_cdk::println!("{response:?}");
                })
                .on_error(|_, error: &BoxError| {
                    ic_cdk::println!("Error {error:?}");
                }),
        )
        .layer(JsonRpcHttpLayer::new())
        .cycles_accounting(ChargeMyself::default())
        .service(Client::new_with_box_error())
}

fn solana_test_validator_base_url() -> String {
    option_env!("SOLANA_TEST_VALIDATOR_URL")
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com")
        .to_string()
}

fn main() {}
