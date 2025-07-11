use candid::{Decode, Encode, Principal};
use ic_management_canister_types::{CanisterId, CanisterSettings};
use pocket_ic::{PocketIc, PocketIcBuilder};
use std::env::var;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn should_hello_world() {
    let setup = Setup::default();

    let hello_world_result = Decode!(
        &setup
            .env
            .update_call(
                setup.canister_id,
                Principal::anonymous(),
                "hello_world",
                Encode!().unwrap(),
            )
            .unwrap(),
        String
    )
    .unwrap();

    assert_eq!(hello_world_result, "Hello, World");
}

pub struct Setup {
    env: Arc<PocketIc>,
    canister_id: CanisterId,
}
impl Setup {
    pub const DEFAULT_CONTROLLER: Principal = Principal::from_slice(&[0x9d, 0xf7, 0x02]);

    pub fn new() -> Self {
        let env = PocketIcBuilder::new().with_fiduciary_subnet().build();

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
            canister_wasm(),
            Encode!().unwrap(),
            Some(Self::DEFAULT_CONTROLLER),
        );

        Self {
            env: Arc::new(env),
            canister_id,
        }
    }
}

impl Default for Setup {
    fn default() -> Self {
        Self::new()
    }
}

fn canister_wasm() -> Vec<u8> {
    ic_test_utilities_load_wasm::load_wasm(
        PathBuf::from(var("CARGO_MANIFEST_DIR").unwrap()).join("."),
        "http_canister",
        &[],
    )
}
