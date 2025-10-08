# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2025-10-08

### Added

- Add `CyclesChargingPolicy::cycles_to_charge` method ([#26](https://github.com/dfinity/canhttp/pull/26))

### Changed

- Update `ic-cdk` to `v0.18.7` ([#21](https://github.com/dfinity/canhttp/pull/21))
- README ([#13](https://github.com/dfinity/canhttp/pull/13))
- Layer for cycles accounting ([#7](https://github.com/dfinity/canhttp/pull/7))

[0.3.0]: https://github.com/dfinity/canhttp/compare/0.2.1..0.3.0

## [0.2.1] - 2025-07-11

### Added

- An `iter` method to `canhttp::multi::MultiResults` returning a borrowing iterator.

### Changed
- The `canhttp` crate has been moved from the [`evm-rpc-canister`](https://github.com/dfinity/evm-rpc-canister) repository to the new [`canhttp`](https://github.com/dfinity/canhttp) repository.

## [0.2.0] - 2025-07-08

### Added
- Data structures `TimedSizedVec<T>` and `TimedSizedMap<K, V>` to store a limited number of expiring elements ([#434](https://github.com/dfinity/evm-rpc-canister/pull/434))
- Method to list `Ok` results in a `MultiResults` ([#435](https://github.com/dfinity/evm-rpc-canister/pull/435))

### Changed

- **Breaking:** change the `code` field in the `IcError` type to use `ic_error_types::RejectCode` instead of `ic_cdk::api::call::RejectionCode` ([#428](https://github.com/dfinity/evm-rpc-canister/pull/428))

## [0.1.0] - 2025-06-04

### Added

- JSON-RPC request ID with constant binary size ([#397](https://github.com/dfinity/evm-rpc-canister/pull/397))
- Use `canhttp` to make parallel calls ([#391](https://github.com/dfinity/evm-rpc-canister/pull/391))
- Improve validation of JSON-RPC requests and responses to adhere to the JSON-RPC specification ([#386](https://github.com/dfinity/evm-rpc-canister/pull/386) and [#387](https://github.com/dfinity/evm-rpc-canister/pull/387))
- Retry layer ([#378](https://github.com/dfinity/evm-rpc-canister/pull/378))
- JSON RPC conversion layer ([#375](https://github.com/dfinity/evm-rpc-canister/pull/375))
- HTTP conversion layer ([#374](https://github.com/dfinity/evm-rpc-canister/pull/374))
- Observability layer ([#370](https://github.com/dfinity/evm-rpc-canister/pull/370))
- Library `canhttp` ([#364](https://github.com/dfinity/evm-rpc-canister/pull/364))