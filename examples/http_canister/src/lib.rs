use candid::{CandidType, Deserialize};
use canhttp::cycles::ChargeCallerError;
use tower::BoxError;

#[derive(Debug, CandidType, Deserialize)]
pub struct InsufficientCyclesError {
    pub expected: u128,
    pub received: u128,
}

impl TryFrom<BoxError> for InsufficientCyclesError {
    type Error = BoxError;

    fn try_from(error: BoxError) -> Result<Self, Self::Error> {
        match error.downcast::<ChargeCallerError>() {
            Ok(error) => Ok(InsufficientCyclesError::from(*error)),
            Err(error) => Err(error),
        }
    }
}

impl From<ChargeCallerError> for InsufficientCyclesError {
    fn from(error: ChargeCallerError) -> Self {
        match error {
            ChargeCallerError::InsufficientCyclesError { expected, received } => {
                Self { expected, received }
            }
        }
    }
}
