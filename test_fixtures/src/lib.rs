use candid::{utils::ArgumentEncoder, CandidType, Encode, Principal};
use ic_canister_runtime::{IcError, Runtime};
use ic_management_canister_types::{CanisterId, CanisterSettings};
use ic_pocket_canister_runtime::PocketIcRuntime;
use pocket_ic::{nonblocking::PocketIc, PocketIcBuilder};
use serde::de::DeserializeOwned;
use std::{env::var, fs, path::PathBuf, sync::Arc};

pub struct Setup {
    env: Arc<PocketIc>,
    canister_id: CanisterId,
    proxy_canister_id: Option<CanisterId>,
}

impl Setup {
    pub const DEFAULT_CALLER: Principal = Principal::from_slice(&[0x9d, 0xf7, 0x01]);
    pub const DEFAULT_CONTROLLER: Principal = Principal::from_slice(&[0x9d, 0xf7, 0x02]);

    pub async fn new(canister_binary_name: &str) -> Self {
        let env = PocketIcBuilder::new()
            .with_nns_subnet() //make_live requires NNS subnet.
            .with_fiduciary_subnet()
            .build_async()
            .await;

        let canister_id = env
            .create_canister_with_settings(
                None,
                Some(CanisterSettings {
                    controllers: Some(vec![Self::DEFAULT_CONTROLLER]),
                    ..CanisterSettings::default()
                }),
            )
            .await;
        env.add_cycles(canister_id, u64::MAX as u128).await;

        env.install_canister(
            canister_id,
            canister_wasm(canister_binary_name),
            Encode!().unwrap(),
            Some(Self::DEFAULT_CONTROLLER),
        )
        .await;

        let mut env = env;
        let _endpoint = env.make_live(None).await;

        Self {
            env: Arc::new(env),
            canister_id,
            proxy_canister_id: None,
        }
    }

    pub async fn with_proxy(self) -> Self {
        let Setup {
            env,
            canister_id,
            proxy_canister_id,
        } = self;
        assert!(proxy_canister_id.is_none(), "Proxy canister already setup");

        let proxy_canister_id = env
            .create_canister_with_settings(
                None,
                Some(CanisterSettings {
                    // Only controllers have access to the proxy service, so we also allow
                    // the default caller
                    controllers: Some(vec![Self::DEFAULT_CONTROLLER, Setup::DEFAULT_CALLER]),
                    ..CanisterSettings::default()
                }),
            )
            .await;
        env.add_cycles(proxy_canister_id, u64::MAX as u128).await;

        env.install_canister(
            proxy_canister_id,
            proxy_wasm().await,
            Encode!().unwrap(),
            Some(Self::DEFAULT_CONTROLLER),
        )
        .await;

        Self {
            env,
            canister_id,
            proxy_canister_id: Some(proxy_canister_id),
        }
    }

    pub fn runtime(&self) -> PocketIcRuntime<'_> {
        let runtime = PocketIcRuntime::new(self.env.as_ref(), Self::DEFAULT_CALLER);
        if let Some(proxy_canister_id) = self.proxy_canister_id {
            runtime.with_proxy_canister(proxy_canister_id)
        } else {
            runtime
        }
    }

    pub fn canister(&self) -> Canister<PocketIcRuntime<'_>> {
        Canister {
            runtime: self.runtime(),
            id: self.canister_id,
        }
    }
}

pub struct Canister<R> {
    runtime: R,
    id: CanisterId,
}

impl<R: Runtime> Canister<R> {
    pub async fn update_call<In, Out>(&self, method: &str, args: In) -> Out
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.update_call_with_cycles(method, args, 0).await
    }

    pub async fn update_call_with_cycles<In, Out>(
        &self,
        method: &str,
        args: In,
        cycles: u128,
    ) -> Out
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.try_update_call_with_cycles(method, args, cycles)
            .await
            .expect("Update call failed")
    }

    pub async fn try_update_call_with_cycles<In, Out>(
        &self,
        method: &str,
        args: In,
        cycles: u128,
    ) -> Result<Out, IcError>
    where
        In: ArgumentEncoder + Send,
        Out: CandidType + DeserializeOwned,
    {
        self.runtime
            .update_call(self.id, method, args, cycles)
            .await
    }
}

pub fn canister_wasm(canister_binary_name: &str) -> Vec<u8> {
    ic_test_utilities_load_wasm::load_wasm(
        PathBuf::from(var("CARGO_MANIFEST_DIR").unwrap()).join("."),
        canister_binary_name,
        &[],
    )
}

async fn proxy_wasm() -> Vec<u8> {
    const DEFAULT_PATH: &str = "../../test_fixtures/wasms/proxy.wasm";
    const DOWNLOAD_URL: &str =
        "https://github.com/dfinity/proxy-canister/releases/download/v0.1.0/proxy.wasm";

    let path = option_env!("PROXY_CANISTER_WASM_PATH")
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from(var("CARGO_MANIFEST_DIR").unwrap()).join(DEFAULT_PATH));

    if !std::path::Path::new(&path).exists() {
        std::process::Command::new("curl")
            .args(["-L", "-o", path.to_str().unwrap(), DOWNLOAD_URL])
            .status()
            .unwrap_or_else(|e| panic!("Failed to download canister WASM: {e:?}"));
    }

    fs::read(&path).unwrap_or_else(|e| panic!("Failed to read proxy canister WASM: {e}"))
}
