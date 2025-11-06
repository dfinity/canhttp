#[cfg(test)]
mod tests;

use crate::{IcError, Runtime};
use async_trait::async_trait;
use candid::{utils::ArgumentEncoder, CandidType, Decode, Encode, Principal};
use serde::de::DeserializeOwned;
use std::{collections::VecDeque, sync::Mutex};

/// An implementation of [`Runtime`] that returns pre-defined results from a queue.
/// This runtime is primarily intended for testing purposes.
#[derive(Debug, Default)]
pub struct StubRuntime {
    // Use a mutex so that this struct is Send and Sync
    call_results: Mutex<VecDeque<Vec<u8>>>,
}

impl StubRuntime {
    /// Create a new empty [`StubRuntime`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Mutate the [`StubRuntime`] instance to add the given stub response.
    ///
    /// Panics if the stub response cannot be encoded using Candid.
    pub fn add_stub_response<Out: CandidType>(self, stub_response: Out) -> Self {
        let result = Encode!(&stub_response).expect("Failed to encode Candid stub response");
        self.call_results.try_lock().unwrap().push_back(result);
        self
    }

    fn call<Out>(&self) -> Result<Out, IcError>
    where
        Out: CandidType + DeserializeOwned,
    {
        let bytes = self
            .call_results
            .try_lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("No available call response"));
        Ok(Decode!(&bytes, Out).expect("Failed to decode Candid stub response"))
    }
}

#[async_trait]
impl Runtime for StubRuntime {
    async fn update_call<In, Out>(
        &self,
        _id: Principal,
        _method: &str,
        _args: In,
        _cycles: u128,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.call()
    }

    async fn query_call<In, Out>(
        &self,
        _id: Principal,
        _method: &str,
        _args: In,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.call()
    }
}
