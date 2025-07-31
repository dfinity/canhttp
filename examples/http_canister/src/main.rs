use canhttp::cycles::{ChargeMyself, CyclesAccountingServiceBuilder};
use canhttp::http::HttpConversionLayer;
use canhttp::{Client, MaxResponseBytesRequestExtension};
use ic_cdk::update;
use tower::{BoxError, Service, ServiceBuilder, ServiceExt};

#[update]
pub async fn make_http_post_request() -> String {
    let request = http::Request::post("https://httpbin.org/anything")
        .max_response_bytes(1_000)
        .header("X-Id", "42")
        .body("Hello, World!".as_bytes().to_vec())
        .unwrap();

    let response = http_client()
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), http::StatusCode::OK);

    String::from_utf8_lossy(response.body()).to_string()
}

fn http_client(
) -> impl Service<http::Request<Vec<u8>>, Response = http::Response<Vec<u8>>, Error = BoxError> {
    ServiceBuilder::new()
        .layer(HttpConversionLayer)
        .cycles_accounting(34, ChargeMyself::default())
        .service(Client::new_with_box_error())
}

fn main() {}
