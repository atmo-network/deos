//! Bare calls are protocol inherents; external calls use the full transaction pipeline.

use alloc::collections::BTreeSet;

use super::{
  MaxInboundDownwardMessagesPerContext, MaxInboundHorizontalChannelsPerContext,
  MaxInboundHorizontalMessagesPerContext,
};
use crate::{Actors, ParachainSystem, RuntimeCall, Timestamp};
#[allow(deprecated)] // The Executive generic requires this SDK trait.
use polkadot_sdk::sp_runtime::traits::ValidateUnsigned;
use polkadot_sdk::{
  frame_support::inherent::ProvideInherent,
  sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError,
    UnknownTransaction,
  },
};

/// Count-only admission; payload/proof bytes and caller overhead retain separate owners.
pub(crate) fn validate_inbound_messages_geometry(
  inbound_messages_data: &cumulus_pallet_parachain_system::parachain_inherent::InboundMessagesData,
) -> Result<(), pallet_deos_actors::BlockResourceError> {
  use pallet_deos_actors::{ContextMessageGeometry, ContextMessageLimits};

  let (downward_full, downward_hashed) = inbound_messages_data.downward_messages.messages();
  let (horizontal_full, horizontal_hashed) = inbound_messages_data.horizontal_messages.messages();
  let downward_full = u32::try_from(downward_full.len())
    .map_err(|_| pallet_deos_actors::BlockResourceError::ArithmeticOverflow)?;
  let downward_hashed = u32::try_from(downward_hashed.len())
    .map_err(|_| pallet_deos_actors::BlockResourceError::ArithmeticOverflow)?;
  let horizontal_full_count = u32::try_from(horizontal_full.len())
    .map_err(|_| pallet_deos_actors::BlockResourceError::ArithmeticOverflow)?;
  let horizontal_hashed_count = u32::try_from(horizontal_hashed.len())
    .map_err(|_| pallet_deos_actors::BlockResourceError::ArithmeticOverflow)?;
  let limits = ContextMessageLimits::new(
    MaxInboundDownwardMessagesPerContext::get(),
    MaxInboundHorizontalMessagesPerContext::get(),
    MaxInboundHorizontalChannelsPerContext::get(),
  );
  limits.validate(ContextMessageGeometry::new(
    downward_full,
    downward_hashed,
    horizontal_full_count,
    horizontal_hashed_count,
    0,
  ))?;
  let mut horizontal_channels = BTreeSet::new();
  for sender in horizontal_full
    .iter() // deos-bypass: bounded-iter — aggregate HRMP geometry is validated above and the channel set rejects at runtime maximum plus one.
    .map(|(sender, _)| *sender)
    .chain(
      horizontal_hashed
        .iter() // deos-bypass: bounded-iter — the same admitted full-plus-hashed HRMP bound owns this traversal.
        .map(|(sender, _)| *sender),
    )
  {
    horizontal_channels.insert(sender);
    if horizontal_channels.len() > MaxInboundHorizontalChannelsPerContext::get() as usize {
      return Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded);
    }
  }
  limits.validate(ContextMessageGeometry::new(
    downward_full,
    downward_hashed,
    horizontal_full_count,
    horizontal_hashed_count,
    u32::try_from(horizontal_channels.len())
      .map_err(|_| pallet_deos_actors::BlockResourceError::ArithmeticOverflow)?,
  ))
}

pub struct RuntimeUnsignedValidator;

#[allow(deprecated)] // The SDK bare application boundary still consumes ValidateUnsigned.
impl ValidateUnsigned for RuntimeUnsignedValidator {
  type Call = RuntimeCall;

  fn pre_dispatch(call: &Self::Call) -> Result<(), TransactionValidityError> {
    let inherent = match call {
      RuntimeCall::Timestamp(call) => Timestamp::is_inherent(call),
      RuntimeCall::ParachainSystem(call) => {
        if let polkadot_sdk::cumulus_pallet_parachain_system::Call::set_validation_data {
          inbound_messages_data,
          ..
        } = call
        {
          validate_inbound_messages_geometry(inbound_messages_data)
            .map_err(|_| InvalidTransaction::ExhaustsResources)?;
        }
        ParachainSystem::is_inherent(call)
      }
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
