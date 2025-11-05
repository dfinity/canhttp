use crate::{IcError, Runtime};
use async_trait::async_trait;
use candid::{decode_one, encode_args, utils::ArgumentEncoder, CandidType, Principal};
use ic_agent::{Agent, AgentError};
use ic_error_types::RejectCode;
use serde::de::DeserializeOwned;

/// Runtime for interacting with a canister through an [`ic_agent::Agent`].
#[derive(Clone, Debug)]
pub struct AgentRuntime<'a> {
    agent: &'a Agent,
}

impl<'a> AgentRuntime<'a> {
    /// Create a new [`AgentRuntime`] with the given [`Agent`].
    pub fn new(agent: &'a Agent) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl Runtime for AgentRuntime<'_> {
    async fn update_call<In, Out>(
        &self,
        id: Principal,
        method: &str,
        args: In,
        _cycles: u128,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.agent
            .update(&id, method)
            .with_arg(encode_args(args).unwrap_or_else(panic_when_encode_fails))
            .call_and_wait()
            .await
            .map_err(IcError::from)
            .and_then(decode_agent_response)
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
        self.agent
            .query(&id, method)
            .with_arg(encode_args(args).unwrap_or_else(panic_when_encode_fails))
            .call()
            .await
            .map_err(IcError::from)
            .and_then(decode_agent_response)
    }
}

fn decode_agent_response<Out>(result: Vec<u8>) -> Result<Out, IcError>
where
    Out: CandidType + DeserializeOwned,
{
    decode_one::<Out>(&result).map_err(|e| IcError::CandidDecodeFailed {
        message: e.to_string(),
    })
}

impl From<AgentError> for IcError {
    fn from(e: AgentError) -> Self {
        IcError::CallRejected {
            code: RejectCode::SysFatal,
            message: e.to_string(),
        }
    }
}

fn panic_when_encode_fails(err: candid::error::Error) -> Vec<u8> {
    panic!("failed to encode args: {err}")
}
