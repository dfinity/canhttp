use candid::{decode_args, encode_args, utils::ArgumentEncoder, CandidType, Encode, Principal};
use ic_management_canister_types::{CanisterId, CanisterSettings};
use pocket_ic::{PocketIc, PocketIcBuilder};
use serde::de::DeserializeOwned;
use std::{env::var, path::PathBuf, sync::Arc};

pub struct Setup {
    env: Arc<PocketIc>,
    canister_id: CanisterId,
}

impl Setup {
    pub const DEFAULT_CONTROLLER: Principal = Principal::from_slice(&[0x9d, 0xf7, 0x02]);

    pub fn new(canister_binary_name: &str) -> Self {
        let env = PocketIcBuilder::new()
            .with_nns_subnet() //make_live requires NNS subnet.
            .with_fiduciary_subnet()
            .build();

        let canister_id = env.create_canister_with_settings(
            None,
            Some(CanisterSettings {
                controllers: Some(vec![Self::DEFAULT_CONTROLLER]),
                ..CanisterSettings::default()
            }),
        );
        env.add_cycles(canister_id, u64::MAX as u128);

        env.install_canister(
            canister_id,
            canister_wasm(canister_binary_name),
            Encode!().unwrap(),
            Some(Self::DEFAULT_CONTROLLER),
        );

        let mut env = env;
        let _endpoint = env.make_live(None);

        Self {
            env: Arc::new(env),
            canister_id,
        }
    }

    pub fn canister(&self) -> Canister {
        Canister {
            env: self.env.clone(),
            id: self.canister_id,
        }
    }
}

pub struct Canister {
    env: Arc<PocketIc>,
    id: CanisterId,
}

impl Canister {
    pub fn update_call<In, Out>(&self, sender: Principal, method: &str, args: In) -> Out
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        let message_id = self
            .env
            .submit_call(
                self.id,
                sender,
                method,
                encode_args(args).unwrap_or_else(|e| {
                    panic!("Failed to encode arguments for method {method}: {e}")
                }),
            )
            .unwrap_or_else(|e| panic!("Failed to call method {method}: {e}"));
        let response_bytes = self
            .env
            .await_call_no_ticks(message_id)
            .unwrap_or_else(|e| panic!("Failed to await call for method {method}: {e}"));
        let (res,) = decode_args(&response_bytes).unwrap_or_else(|e| {
            panic!("Failed to decode canister response for method {method}: {e}")
        });
        res
    }
}

fn canister_wasm(canister_binary_name: &str) -> Vec<u8> {
    ic_test_utilities_load_wasm::load_wasm(
        PathBuf::from(var("CARGO_MANIFEST_DIR").unwrap()).join("."),
        canister_binary_name,
        &[],
    )
}
