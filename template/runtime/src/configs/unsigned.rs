//! Bare calls are protocol inherents; external calls use the full transaction pipeline.

use crate::{Actors, ParachainSystem, RuntimeCall, Timestamp};
#[allow(deprecated)] // The Executive generic requires this SDK trait.
use polkadot_sdk::sp_runtime::traits::ValidateUnsigned;
use polkadot_sdk::{
  frame_support::inherent::ProvideInherent,
  sp_runtime::transaction_validity::{
    TransactionSource, TransactionValidity, TransactionValidityError, UnknownTransaction,
  },
};

pub struct RuntimeUnsignedValidator;

#[allow(deprecated)] // The SDK bare application boundary still consumes ValidateUnsigned.
impl ValidateUnsigned for RuntimeUnsignedValidator {
  type Call = RuntimeCall;

  fn pre_dispatch(call: &Self::Call) -> Result<(), TransactionValidityError> {
    let inherent = match call {
      RuntimeCall::Timestamp(call) => Timestamp::is_inherent(call),
      RuntimeCall::ParachainSystem(call) => ParachainSystem::is_inherent(call),
      RuntimeCall::Actors(call) => Actors::is_inherent(call),
      _ => false,
    };
    if inherent {
      Ok(())
    } else {
      Err(UnknownTransaction::NoUnsignedValidator.into())
    }
  }

  fn validate_unsigned(_source: TransactionSource, _call: &Self::Call) -> TransactionValidity {
    Err(UnknownTransaction::NoUnsignedValidator.into())
  }
}
