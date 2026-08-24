//! Transaction-extension binding for the shared block-resource meter.

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use pallet_deos_actors::{BlockResourceDomain, BlockResourceReservation};
use polkadot_sdk::sp_runtime::{
  DispatchResult, impl_tx_ext_default,
  traits::{DispatchInfoOf, PostDispatchInfoOf, TransactionExtension},
  transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use scale_info::TypeInfo;

const MISSING_RESOURCE_STATE: u8 = 42;
const STALE_RESOURCE_STATE: u8 = 43;
const RESOURCE_RESERVATION_REJECTED: u8 = 44;
const RESOURCE_SETTLEMENT_REJECTED: u8 = 45;

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct BlockResourceMeterExtension;

impl TransactionExtension<RuntimeCall> for BlockResourceMeterExtension {
  const IDENTIFIER: &'static str = "BlockResourceMeter";
  type Implicit = ();
  type Val = ();
  type Pre = BlockResourceReservation;

  fn weight(&self, _call: &RuntimeCall) -> Weight {
    <crate::weights::pallet_deos_actors::SubstrateWeight<Runtime> as pallet_deos_actors::WeightInfo>::block_resource_meter_extension()
  }

  fn prepare(
    self,
    _val: Self::Val,
    _origin: &<RuntimeCall as polkadot_sdk::sp_runtime::traits::Dispatchable>::RuntimeOrigin,
    _call: &RuntimeCall,
    info: &DispatchInfoOf<RuntimeCall>,
    _len: usize,
  ) -> Result<Self::Pre, TransactionValidityError> {
    let mut state = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
      .ok_or(InvalidTransaction::Custom(MISSING_RESOURCE_STATE))?;
    state
      .ensure_block(System::block_number())
      .map_err(|_| InvalidTransaction::Custom(STALE_RESOURCE_STATE))?;
    let reservation = state
      .reserve(
        BlockResourceBudgetValue::get().limits(),
        BlockResourceDomain::UserDispatch,
        info.total_weight(),
      )
      .map_err(|_| InvalidTransaction::Custom(RESOURCE_RESERVATION_REJECTED))?;
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
    Ok(reservation)
  }

  fn post_dispatch_details(
    mut pre: Self::Pre,
    info: &DispatchInfoOf<RuntimeCall>,
    post_info: &PostDispatchInfoOf<RuntimeCall>,
    _len: usize,
    _result: &DispatchResult,
  ) -> Result<Weight, TransactionValidityError> {
    let mut state = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
      .ok_or(InvalidTransaction::Custom(MISSING_RESOURCE_STATE))?;
    state
      .ensure_block(System::block_number())
      .and_then(|()| state.settle(&mut pre, post_info.calc_actual_weight(info)))
      .map_err(|_| InvalidTransaction::Custom(RESOURCE_SETTLEMENT_REJECTED))?;
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
    Ok(Weight::zero())
  }

  impl_tx_ext_default!(RuntimeCall; validate);
}

#[cfg(test)]
mod tests {
  use super::*;
  use polkadot_sdk::frame_support::dispatch::GetDispatchInfo;

  fn remark() -> (RuntimeCall, DispatchInfoOf<RuntimeCall>) {
    let call = RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() });
    let mut info = call.get_dispatch_info();
    info.extension_weight = BlockResourceMeterExtension.weight(&call);
    (call, info)
  }

  fn external_state(block: BlockNumber) -> pallet_deos_actors::BlockResourceState<BlockNumber> {
    let mut state = pallet_deos_actors::BlockResourceState::new(block);
    state.begin_prepass().expect("fresh state opens"); // deos-bypass: panic-owner — test constructs a fresh state with no reservations.
    state.open_external_phase().expect("empty prepass closes"); // deos-bypass: panic-owner — preceding transition establishes PrepassExecuting without reservations.
    state
  }

  #[test]
  fn prepare_rejects_missing_stale_and_over_capacity_state_distinctly() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::kill();
      let (call, mut info) = remark();
      let origin = RuntimeOrigin::none();
      assert_eq!(
        BlockResourceMeterExtension.prepare((), &origin, &call, &info, 0),
        Err(InvalidTransaction::Custom(MISSING_RESOURCE_STATE).into())
      );

      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(external_state(0));
      assert_eq!(
        BlockResourceMeterExtension.prepare((), &origin, &call, &info, 0),
        Err(InvalidTransaction::Custom(STALE_RESOURCE_STATE).into())
      );

      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(external_state(1));
      info.call_weight = Weight::MAX;
      assert_eq!(
        BlockResourceMeterExtension.prepare((), &origin, &call, &info, 0),
        Err(InvalidTransaction::Custom(RESOURCE_RESERVATION_REJECTED).into())
      );
    });
  }

  #[test]
  fn settlement_reclaims_valid_actual_and_rejects_lost_reservation_authority() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(external_state(1));
      let (call, info) = remark();
      let origin = RuntimeOrigin::none();
      let reservation = BlockResourceMeterExtension
        .prepare((), &origin, &call, &info, 0)
        .expect("current ExternalPhase must admit one remark"); // deos-bypass: panic-owner — production budget fit is covered by the maximum signed-call integration test.
      let reserved = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
        .expect("prepare retains state"); // deos-bypass: panic-owner — successful prepare writes authoritative state.
      assert_eq!(reserved.outstanding_reservations(), 1);
      assert_eq!(reserved.usage().user_dispatch_used(), info.total_weight());

      let actual = PostDispatchInfoOf::<RuntimeCall> {
        actual_weight: Some(Weight::zero()),
        pays_fee: polkadot_sdk::frame_support::dispatch::Pays::Yes,
      };
      assert_eq!(
        BlockResourceMeterExtension::post_dispatch_details(reservation, &info, &actual, 0, &Ok(())),
        Ok(Weight::zero())
      );
      let settled = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
        .expect("settlement retains state"); // deos-bypass: panic-owner — successful settlement writes authoritative state.
      assert_eq!(settled.outstanding_reservations(), 0);
      assert_eq!(settled.usage().user_dispatch_used(), Weight::zero());

      let lost = BlockResourceMeterExtension
        .prepare((), &origin, &call, &info, 0)
        .expect("second reservation fits after reclaim"); // deos-bypass: panic-owner — preceding zero-actual settlement restores complete capacity.
      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(external_state(1));
      assert_eq!(
        BlockResourceMeterExtension::post_dispatch_details(lost, &info, &actual, 0, &Ok(())),
        Err(InvalidTransaction::Custom(RESOURCE_SETTLEMENT_REJECTED).into())
      );
    });
  }
}
