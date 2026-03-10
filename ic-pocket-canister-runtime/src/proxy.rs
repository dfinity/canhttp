//! Proxy canister types for routing update calls through a proxy to attach cycles.

use candid::{decode_one, encode_args, utils::ArgumentEncoder, CandidType, Deserialize, Principal};
use ic_canister_runtime::IcError;
use ic_error_types::RejectCode;
use serde::Serialize;

/// Arguments for calling the proxy canister.
#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub struct ProxyArgs {
    canister_id: Principal,
    method: String,
    #[serde(with = "serde_bytes")]
    args: Vec<u8>,
    cycles: u128,
}

impl ProxyArgs {
    pub fn new<In: ArgumentEncoder>(
        canister_id: Principal,
        method: impl ToString,
        args: In,
        cycles: u128,
    ) -> Self {
        Self {
            canister_id,
            method: method.to_string(),
            args: encode_args(args).unwrap_or_else(panic_when_encode_fails),
            cycles,
        }
    }
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
struct ProxySucceed {
    #[serde(with = "serde_bytes")]
    result: Vec<u8>,
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
enum ProxyError {
    InsufficientCycles { available: u128, required: u128 },
    CallFailed { reason: String },
    UnauthorizedUser,
}

pub fn decode_response(bytes: Vec<u8>) -> Result<Vec<u8>, IcError> {
    let result: Result<ProxySucceed, ProxyError> =
        decode_one(&bytes).map_err(|e| IcError::CandidDecodeFailed {
            message: format!("failed to decode proxy response: {}", e),
        })?;

    match result {
        Ok(ProxySucceed { result }) => Ok(result),
        Err(error) => match error {
            ProxyError::UnauthorizedUser => Err(IcError::CallRejected {
                code: RejectCode::SysFatal,
                message: "Unauthorized caller!".to_string(),
            }),
            ProxyError::InsufficientCycles {
                available,
                required,
            } => Err(IcError::InsufficientLiquidCycleBalance {
                available,
                required,
            }),
            ProxyError::CallFailed { reason } => Err(IcError::CallRejected {
                code: RejectCode::SysFatal,
                message: reason,
            }),
        },
    }
}

fn panic_when_encode_fails(err: candid::error::Error) -> Vec<u8> {
    panic!("failed to encode args: {err}")
}
