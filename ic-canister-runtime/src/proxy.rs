use crate::{IcError, Runtime};
use async_trait::async_trait;
use candid::{decode_one, encode_args, utils::ArgumentEncoder, CandidType, Deserialize, Principal};
use ic_error_types::RejectCode;
use serde::{de::DeserializeOwned, Serialize};

/// Runtime wrapping another [`Runtime`] instance, where update calls are forwarded through a
/// [proxy canister](https://github.com/dfinity/proxy-canister) to attach cycles to them.
pub struct ProxyRuntime<R> {
    runtime: R,
    proxy_canister_id: Principal,
}

impl<R> ProxyRuntime<R> {
    /// Create a new [`ProxyRuntime`] wrapping the given [`Runtime`] by forwarding update calls
    /// through the given proxy canister to attach cycles.
    pub fn new(runtime: R, proxy_canister_id: Principal) -> Self {
        ProxyRuntime {
            runtime,
            proxy_canister_id,
        }
    }

    /// Modify the underlying runtime by applying a transformation function.
    ///
    /// The transformation does not necessarily produce a runtime of the same type.
    pub fn with_runtime<S, F: FnOnce(R) -> S>(self, transformation: F) -> ProxyRuntime<S> {
        ProxyRuntime {
            runtime: transformation(self.runtime),
            proxy_canister_id: self.proxy_canister_id,
        }
    }
}

impl<R> AsRef<R> for ProxyRuntime<R> {
    fn as_ref(&self) -> &R {
        &self.runtime
    }
}

#[async_trait]
impl<R: Runtime + Send + Sync> Runtime for ProxyRuntime<R> {
    async fn update_call<In, Out>(
        &self,
        id: Principal,
        method: &str,
        args: In,
        cycles: u128,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.runtime
            .update_call::<(ProxyArgs,), Result<ProxySucceed, ProxyError>>(
                self.proxy_canister_id,
                "proxy",
                (ProxyArgs::new(id, method, args, cycles),),
                0,
            )
            .await
            .and_then(decode_proxy_canister_response)
    }

    async fn query_call<In, Out>(
        &self,
        id: Principal,
        method: &str,
        args: In,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.runtime.query_call(id, method, args).await
    }
}

fn decode_proxy_canister_response<Out>(
    result: Result<ProxySucceed, ProxyError>,
) -> Result<Out, IcError>
where
    Out: CandidType + DeserializeOwned,
{
    match result {
        Ok(ProxySucceed { result }) => {
            decode_one(&result).map_err(|e| IcError::CandidDecodeFailed {
                message: format!(
                    "failed to decode canister response as {}: {}",
                    std::any::type_name::<Out>(),
                    e
                ),
            })
        }
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

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
struct ProxyArgs {
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

fn panic_when_encode_fails(err: candid::error::Error) -> Vec<u8> {
    panic!("failed to encode args: {err}")
}
