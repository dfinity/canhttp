//! Example of a canister using `canhttp` to issue HTTP requests.

use canhttp::{
    cycles::{ChargeMyself, CyclesAccountingServiceBuilder},
    http::HttpConversionLayer,
    observability::ObservabilityLayer,
    Client, MaxResponseBytesRequestExtension,
};
use ic_cdk::update;
use tower::{BoxError, Service, ServiceBuilder, ServiceExt};

/// Make an HTTP POST request.
#[update]
pub async fn make_http_post_request() -> String {
    let request = http::Request::post(format!("{}/anything", httpbin_base_url()))
        .max_response_bytes(1_000)
        .header("X-Id", "42")
        .body("Hello, World!".as_bytes().to_vec())
        .unwrap();

    let response = http_client()
        .ready()
        .await
        .expect("Client should be ready")
        .call(request)
        .await
        .expect("Request should succeed");

    assert_eq!(response.status(), http::StatusCode::OK);

    String::from_utf8_lossy(response.body()).to_string()
}

fn http_client(
) -> impl Service<http::Request<Vec<u8>>, Response = http::Response<Vec<u8>>, Error = BoxError> {
    ServiceBuilder::new()
        // Print request, response and errors to the console
        .layer(
            ObservabilityLayer::new()
                .on_request(|request: &http::Request<Vec<u8>>| ic_cdk::println!("{request:?}"))
                .on_response(|_, response: &http::Response<Vec<u8>>| {
                    ic_cdk::println!("{response:?}");
                })
                .on_error(|_, error: &BoxError| {
                    ic_cdk::println!("Error {error:?}");
                }),
        )
        // Only deal with types from the http crate.
        .layer(HttpConversionLayer)
        // Use cycles from the canister to pay for HTTPs outcalls
        .cycles_accounting(ChargeMyself::default())
        // The actual client
        .service(Client::new_with_box_error())
}

fn httpbin_base_url() -> String {
    option_env!("HTTPBIN_URL")
        .unwrap_or_else(|| "https://httpbin.org")
        .to_string()
}

fn main() {}
