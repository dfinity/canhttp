[![Internet Computer portal](https://img.shields.io/badge/InternetComputer-grey?logo=internet%20computer&style=for-the-badge)](https://internetcomputer.org)
[![DFinity Forum](https://img.shields.io/badge/help-post%20on%20forum.dfinity.org-blue?style=for-the-badge)](https://forum.dfinity.org/)
[![GitHub license](https://img.shields.io/badge/license-Apache%202.0-blue.svg?logo=apache&style=for-the-badge)](LICENSE)


# `ic-canister-runtime`

Library to abstract the canister runtime so that code making requests to canisters can be reused, e.g.:
* in production using [`ic_cdk`](https://crates.io/crates/ic-cdk),
* in unit tests by mocking this crate's `Runtime` trait,
* in integration tests by implementing this trait for [PocketIC](https://internetcomputer.org/docs/building-apps/test/pocket-ic) or using the `MockHttpRuntime` implementation from the [`ic-mock-http-canister-runtime`](https://crates.io/crates/ic-mock-http-canister-runtime) crate.

## Usage

Add this to your `Cargo.toml` (see [crates.io](https://crates.io/crates/ic-canister-runtime) for the latest version):

```toml
ic-canister-runtime = "0.1.0"
```

Then, use the library to abstract your code making requests to canisters as follows:
```rust
use ic_canister_runtime::{IcRuntime, Runtime};

let runtime = IcRuntime::new();

// Make a request to the `http_request` example canister's `make_http_post_request` endpoint
// See: https://github.com/dfinity/canhttp/tree/main/examples/http_canister
let http_request_result: String = runtime
    .update_call(canister_id, "make_http_post_request", (), 0)
    .await
    .expect("Call to `http_canister` failed");

assert!(http_request_result.contains("Hello, World!"));
assert!(http_request_result.contains("\"X-Id\": \"42\""));
```

See the [Rust documentation](https://docs.rs/ic-canister-runtime) for more details.

## Cargo Features

### Feature `wallet`

Provides the `CyclesWalletRuntime` implementation which allows routing update calls to a canister through a [cycles wallet](https://github.com/dfinity/cycles-wallet) to attach cycles to them.

## License

This project is licensed under the [Apache License 2.0](https://opensource.org/licenses/Apache-2.0).