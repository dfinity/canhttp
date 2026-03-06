//! Example of a canister using `canhttp` to issue HTTP requests.

use canhttp::{
    cycles::{ChargeCaller, ChargeMyself, CyclesAccountingServiceBuilder, CyclesChargingPolicy},
    http::HttpConversionLayer,
    observability::ObservabilityLayer,
    CanisterReadyLayer, Client, MaxResponseBytesRequestExtension,
};
use http_canister::InsufficientCyclesError;
use ic_cdk::update;
use tower::{BoxError, Service, ServiceBuilder, ServiceExt};

/// Make an HTTP POST request.
#[update]
pub async fn make_http_post_request() -> String {
    let response = http_client(ChargeMyself::default())
        .ready()
        .await
        .expect("Client should be ready")
        .call(request())
        .await
        .expect("Request should succeed");

    assert_eq!(response.status(), http::StatusCode::OK);

    String::from_utf8_lossy(response.body()).to_string()
}

/// Make an HTTP POST request and charge the user cycles for it.
#[update]
pub async fn make_http_post_request_and_charge_user_cycles(
) -> Result<String, InsufficientCyclesError> {
    // Use cycles attached by the caller to pay for HTTPs outcalls and charge an additional fee.
    match http_client(ChargeCaller::new(|_request, cost| cost + 1_000_000))
        .ready()
        .await
        .expect("Client should be ready")
        .call(request())
        .await
    {
        Ok(response) => {
            assert_eq!(response.status(), http::StatusCode::OK);
            Ok(String::from_utf8_lossy(response.body()).to_string())
        }
        // Return an error if the call failed due to insufficient cycles, otherwise panic.
        Err(e) => Err(InsufficientCyclesError::try_from(e).expect("Request should succeed")),
    }
}

/// Make multiple HTTP POST requests in a loop,
/// ensuring via [`CanisterReadyLayer`] that the loop will stop if the canister is stopped.
#[update]
pub async fn infinite_loop_make_http_post_request() -> String {
    let mut client = ServiceBuilder::new()
        .layer(CanisterReadyLayer)
        .service(http_client(ChargeMyself::default()));

    loop {
        match client.ready().await {
            Ok(ready) => {
                let response = ready.call(request()).await.expect("Request should succeed");
                assert_eq!(response.status(), http::StatusCode::OK);
            }
            Err(e) => return format!("Not ready: {}", e),
        }
    }
}

fn http_client<C>(
    cycles_charging_policy: C,
) -> impl Service<http::Request<Vec<u8>>, Response = http::Response<Vec<u8>>, Error = BoxError>
where
    C: CyclesChargingPolicy + Clone,
    <C as CyclesChargingPolicy>::Error: std::error::Error + Send + Sync + 'static,
{
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
        // The strategy to use to charge cycles for the request
        .cycles_accounting(cycles_charging_policy)
        // The actual client
        .service(Client::new_with_box_error())
}

fn request() -> http::Request<Vec<u8>> {
    fn httpbin_base_url() -> String {
        option_env!("HTTPBIN_URL")
            .unwrap_or_else(|| "https://httpbin.org")
            .to_string()
    }

    http::Request::post(format!("{}/anything", httpbin_base_url()))
        .max_response_bytes(1_000)
        .header("X-Id", "42")
        .body("Hello, World!".as_bytes().to_vec())
        .unwrap()
}

fn main() {}
