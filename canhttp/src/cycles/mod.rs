//! Middleware to handle cycles accounting.
//!
//! Issuing HTTPs outcalls requires cycles, and this layer takes care of 2 things:
//! 1. Estimate the number of cycles required for an HTTPs outcall.
//! 2. Decide how the canister should charge for those cycles.
//!
//! # Examples
//!
//! To let the canister pay for HTTPs outcalls with its own cycle:
//! ```rust
//! use canhttp::{cycles::{ChargeMyself, CyclesAccountingServiceBuilder}, Client};
//! use tower::{Service, ServiceBuilder, ServiceExt, BoxError};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut service = ServiceBuilder::new()
//!   .cycles_accounting(34, ChargeMyself::default())
//!   .service(Client::new_with_box_error());
//!
//! let _ = service.ready().await.unwrap();
//!
//! # Ok(())
//! # }
//! ```
//!
//! To charge the caller of the canister for the whole cost of the HTTPs outcall with an additional fix fee of 1M cycles:
//! ```rust
//! use canhttp::{cycles::{ChargeCaller, CyclesAccountingServiceBuilder}, Client};
//! use tower::{Service, ServiceBuilder, ServiceExt, BoxError};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut service = ServiceBuilder::new()
//!   .cycles_accounting(34, ChargeCaller::new(|_request, cost| cost + 1_000_000))
//!   .service(Client::new_with_box_error());
//!
//! let _ = service.ready().await.unwrap();
//!
//! # Ok(())
//! # }
//! ```
#[cfg(test)]
mod tests;

use crate::client::IcHttpRequestWithCycles;
use crate::convert::{Convert, ConvertRequestLayer};
use crate::ConvertServiceBuilder;
use ic_cdk::api::management_canister::http_request::CanisterHttpRequestArgument;
use std::convert::Infallible;
use thiserror::Error;
use tower::ServiceBuilder;
use tower_layer::Stack;

/// Charge cycles to pay for a single HTTPs outcall.
pub trait CyclesChargingPolicy {
    /// Type returned in case of a charging error.
    type Error;

    /// Charge cycles and return the charged amount.
    fn charge_cycles(
        &self,
        request: &CanisterHttpRequestArgument,
        request_cycles_cost: u128,
    ) -> Result<u128, Self::Error>;
}

/// Canister using that library will pay for HTTPs outcalls with its own cycles.
#[derive(Default, Clone)]
pub struct ChargeMyself {}

impl CyclesChargingPolicy for ChargeMyself {
    type Error = Infallible;

    fn charge_cycles(
        &self,
        _request: &CanisterHttpRequestArgument,
        _request_cycles_cost: u128,
    ) -> Result<u128, Self::Error> {
        // no-op,
        Ok(0)
    }
}

/// Cycles will be transferred from the caller of the canister using that library to pay for HTTPs outcalls.
#[derive(Clone)]
pub struct ChargeCaller<F> {
    cycles_to_charge: F,
}

impl<F> ChargeCaller<F>
where
    F: Fn(&CanisterHttpRequestArgument, u128) -> u128,
{
    /// Create a new instance of [`ChargeCaller`].
    pub fn new(cycles_to_charge: F) -> Self {
        ChargeCaller { cycles_to_charge }
    }
}

impl<F> CyclesChargingPolicy for ChargeCaller<F>
where
    F: Fn(&CanisterHttpRequestArgument, u128) -> u128,
{
    type Error = ChargeCallerError;

    fn charge_cycles(
        &self,
        request: &CanisterHttpRequestArgument,
        request_cycles_cost: u128,
    ) -> Result<u128, Self::Error> {
        let cycles_to_charge = (self.cycles_to_charge)(request, request_cycles_cost);
        if cycles_to_charge > 0 {
            let cycles_available = ic_cdk::api::call::msg_cycles_available128();
            if cycles_available < cycles_to_charge {
                return Err(ChargeCallerError::InsufficientCyclesError {
                    expected: cycles_to_charge,
                    received: cycles_available,
                });
            }
            let cycles_received = ic_cdk::api::call::msg_cycles_accept128(cycles_to_charge);
            assert_eq!(
                cycles_received, cycles_to_charge,
                "Expected to receive {cycles_to_charge}, but got {cycles_received}"
            );
        }
        Ok(cycles_to_charge)
    }
}

/// Estimate the exact minimum cycles amount required to send an HTTPs outcall as specified
/// [here](https://internetcomputer.org/docs/current/developer-docs/gas-cost#https-outcalls).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CyclesCostEstimator {
    num_nodes_in_subnet: u32,
}

impl CyclesCostEstimator {
    /// Maximum value for `max_response_bytes` which is 2MB,
    /// see the [IC specification](https://internetcomputer.org/docs/current/references/ic-interface-spec#ic-http_request).
    pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 2_000_000;

    /// Create a new estimator for a subnet having the given number of nodes.
    pub const fn new(num_nodes_in_subnet: u32) -> Self {
        CyclesCostEstimator {
            num_nodes_in_subnet,
        }
    }

    /// Compute the number of cycles required to send the given request via HTTPs outcall.
    ///
    /// An HTTP outcall entails calling the `http_request` method on the management canister interface,
    /// which requires that cycles to pay for the call must be explicitly attached with the call
    /// ([IC specification](https://internetcomputer.org/docs/current/references/ic-interface-spec#ic-http_request)).
    /// The required amount of cycles to attach is specified
    /// [here](https://internetcomputer.org/docs/current/developer-docs/gas-cost#https-outcalls).
    pub fn cost_of_http_request(&self, request: &CanisterHttpRequestArgument) -> u128 {
        let payload_body_bytes = request
            .body
            .as_ref()
            .map(|body| body.len())
            .unwrap_or_default();
        let extra_payload_bytes = request.url.len()
            + request
                .headers
                .iter()
                .map(|header| header.name.len() + header.value.len())
                .sum::<usize>()
            + request.transform.as_ref().map_or(0, |transform| {
                transform.function.0.method.len() + transform.context.len()
            });
        let max_response_bytes = request
            .max_response_bytes
            .unwrap_or(Self::DEFAULT_MAX_RESPONSE_BYTES);
        let request_bytes = (payload_body_bytes + extra_payload_bytes) as u128;
        self.base_fee()
            + self.request_fee(request_bytes)
            + self.response_fee(max_response_bytes as u128)
    }

    fn base_fee(&self) -> u128 {
        3_000_000_u128
            .saturating_add(60_000_u128.saturating_mul(self.num_nodes_as_u128()))
            .saturating_mul(self.num_nodes_as_u128())
    }

    fn request_fee(&self, bytes: u128) -> u128 {
        400_u128
            .saturating_mul(self.num_nodes_as_u128())
            .saturating_mul(bytes)
    }

    fn response_fee(&self, bytes: u128) -> u128 {
        800_u128
            .saturating_mul(self.num_nodes_as_u128())
            .saturating_mul(bytes)
    }

    fn num_nodes_as_u128(&self) -> u128 {
        self.num_nodes_in_subnet as u128
    }
}

/// Error return by the [`CyclesAccounting`] middleware.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum ChargeCallerError {
    /// Error returned when the caller should be charged but did not attach sufficiently many cycles.
    #[error("insufficient cycles (expected {expected:?}, received {received:?})")]
    InsufficientCyclesError {
        /// Expected amount of cycles. Minimum value that should have been sent.
        expected: u128,
        /// Received amount of cycles
        received: u128,
    },
}

/// A middleware to handle cycles accounting, i.e. verify if sufficiently many cycles are available in a request.
/// How cycles are estimated is given by `CyclesEstimator`
#[derive(Clone, Debug)]
pub struct CyclesAccounting<Charging> {
    cycles_cost_estimator: CyclesCostEstimator,
    charging_policy: Charging,
}

impl<Charging> CyclesAccounting<Charging> {
    /// Create a new middleware given the cycles estimator.
    pub fn new(num_nodes_in_subnet: u32, charging_policy: Charging) -> Self {
        Self {
            cycles_cost_estimator: CyclesCostEstimator::new(num_nodes_in_subnet),
            charging_policy,
        }
    }
}

impl<Charging> Convert<CanisterHttpRequestArgument> for CyclesAccounting<Charging>
where
    Charging: CyclesChargingPolicy,
{
    type Output = IcHttpRequestWithCycles;
    type Error = Charging::Error;

    fn try_convert(
        &mut self,
        request: CanisterHttpRequestArgument,
    ) -> Result<Self::Output, Self::Error> {
        let cycles_to_attach = self.cycles_cost_estimator.cost_of_http_request(&request);
        self.charging_policy
            .charge_cycles(&request, cycles_to_attach)?;
        Ok(IcHttpRequestWithCycles {
            request,
            cycles: cycles_to_attach,
        })
    }
}

/// Extension trait that adds methods to [`tower::ServiceBuilder`] for adding middleware
/// related to cycles accounting
pub trait CyclesAccountingServiceBuilder<L> {
    /// Add cycles accounting.
    ///
    /// See the [module docs](crate::cycles) for examples.
    fn cycles_accounting<C>(
        self,
        num_nodes_in_subnet: u32,
        charging: C,
    ) -> ServiceBuilder<Stack<ConvertRequestLayer<CyclesAccounting<C>>, L>>;
}

impl<L> CyclesAccountingServiceBuilder<L> for ServiceBuilder<L> {
    fn cycles_accounting<C>(
        self,
        num_nodes_in_subnet: u32,
        charging: C,
    ) -> ServiceBuilder<Stack<ConvertRequestLayer<CyclesAccounting<C>>, L>> {
        self.convert_request(CyclesAccounting::new(num_nodes_in_subnet, charging))
    }
}
