//! Example of a canister using `canhttp` to issue JSON-RPC HTTP requests.

use canhttp::{
    cycles::{ChargeMyself, CyclesAccountingServiceBuilder},
    http::{
        json::{
            CreateJsonRpcIdFilter, HttpJsonRpcRequest, HttpJsonRpcResponse, Id,
            JsonConversionLayer, JsonRpcRequest, JsonRpcResponse,
        },
        HttpConversionLayer,
    },
    observability::ObservabilityLayer,
    Client, ConvertServiceBuilder,
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

    let response = json_rpc_client()
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

fn json_rpc_client<Params, Result>(
) -> impl Service<HttpJsonRpcRequest<Params>, Response = HttpJsonRpcResponse<Result>, Error = BoxError>
where
    Params: Debug + Serialize,
    Result: Debug + DeserializeOwned,
{
    ServiceBuilder::new()
        // Print request, response and errors to the console
        .layer(
            ObservabilityLayer::new()
                .on_request(|request: &HttpJsonRpcRequest<Params>| ic_cdk::println!("{request:?}"))
                .on_response(|_, response: &HttpJsonRpcResponse<Result>| {
                    ic_cdk::println!("{response:?}");
                })
                .on_error(|_, error: &BoxError| {
                    ic_cdk::println!("Error {error:?}");
                }),
        )
        // Ensure the requests and responses have matching JSON-RPC request IDs
        .filter_response(CreateJsonRpcIdFilter::new())
        // Convert HTTP requests and responses to JSON-RPC requests and responses
        .layer(JsonConversionLayer::<
            JsonRpcRequest<Params>,
            JsonRpcResponse<Result>,
        >::new())
        // Deal with requests and responses from the `http` crate instead of the `ic-cdk`
        .layer(HttpConversionLayer)
        // Use cycles from the canister to pay for HTTPs outcalls
        .cycles_accounting(ChargeMyself::default())
        // The actual client
        .service(Client::new_with_box_error())
}

fn solana_test_validator_base_url() -> String {
    option_env!("SOLANA_TEST_VALIDATOR_URL")
        .unwrap_or_else(|| "https://api.devnet.solana.com")
        .to_string()
}

fn main() {}
