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
    len: usize,
  ) -> Result<Self::Pre, TransactionValidityError> {
    let mut state = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
      .ok_or(InvalidTransaction::Custom(MISSING_RESOURCE_STATE))?;
    state
      .ensure_block(System::block_number())
      .map_err(|_| InvalidTransaction::Custom(STALE_RESOURCE_STATE))?;
    let reservation = state
      .reserve(
        state
          .budget()
          .map_err(|_| InvalidTransaction::Custom(RESOURCE_RESERVATION_REJECTED))?
          .limits(),
        BlockResourceDomain::UserDispatch,
        frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
          &RuntimeBlockWeights::get(),
          info,
          len,
        ),
      )
      .map_err(|_| InvalidTransaction::Custom(RESOURCE_RESERVATION_REJECTED))?;
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
    Ok(reservation)
  }

  fn post_dispatch_details(
    mut pre: Self::Pre,
    info: &DispatchInfoOf<RuntimeCall>,
    post_info: &PostDispatchInfoOf<RuntimeCall>,
    len: usize,
    _result: &DispatchResult,
  ) -> Result<Weight, TransactionValidityError> {
    // FRAME reclaims dispatch work, not the base extrinsic or encoded bytes.
    let actual_info = frame_support::dispatch::DispatchInfo {
      call_weight: post_info.calc_actual_weight(info),
      extension_weight: Weight::zero(),
      ..*info
    };
    let actual = frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
      &RuntimeBlockWeights::get(),
      &actual_info,
      len,
    );
    let mut state = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
      .ok_or(InvalidTransaction::Custom(MISSING_RESOURCE_STATE))?;
    state
      .ensure_block(System::block_number())
      .and_then(|()| state.settle(&mut pre, actual))
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
    state
      .begin_prepass(BlockResourceBudgetValue::get())
      .expect("fresh state opens"); // deos-bypass: panic-owner — test constructs a fresh state with no reservations.
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
  fn prepare_uses_frozen_state_not_the_configured_budget() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      let mut state = pallet_deos_actors::BlockResourceState::new(1);
      assert_eq!(
        state.begin_prepass(pallet_deos_actors::BlockResourceBudget::fail_closed(
          Weight::MAX
        )),
        Ok(())
      );
      assert_eq!(state.open_external_phase(), Ok(()));
      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
      let (call, info) = remark();
      assert!(
        info
          .total_weight()
          .all_lte(BlockResourceBudgetValue::get().limits().user_base_turn())
      );
      assert_eq!(
        BlockResourceMeterExtension.prepare((), &RuntimeOrigin::none(), &call, &info, 0),
        Err(InvalidTransaction::Custom(RESOURCE_RESERVATION_REJECTED).into())
      );
      assert_eq!(
        pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get(),
        Some(state)
      );
    });
  }

  #[test]
  fn admission_and_reclaim_preserve_frame_base_and_encoded_length() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      System::set_block_number(1);
      let (call, info) = remark();
      let len = 33usize;
      let charged = frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
        &RuntimeBlockWeights::get(),
        &info,
        len,
      );
      let overhead = RuntimeBlockWeights::get()
        .get(info.class)
        .base_extrinsic
        .saturating_add_proof_size(len as u64);
      for limit in [
        charged,
        charged.saturating_sub(Weight::from_parts(1, 0)),
        charged.saturating_sub(Weight::from_parts(0, 1)),
      ] {
        assert!(
          info.total_weight().all_lte(limit),
          "dispatch alone fits every witness"
        );
        let actor_base = limit / 2;
        let budget = pallet_deos_actors::BlockResourceLimits::new(
          Weight::zero(),
          limit,
          actor_base,
          limit.saturating_sub(actor_base),
        )
        .expect("floor/remainder split") // deos-bypass: panic-owner — test derives exact half and remainder from one finite limit.
        .into_budget()
        .expect("bounded synthetic budget"); // deos-bypass: panic-owner — test pairs finite Shared Economic capacity with zero Actor Control.
        let mut state = pallet_deos_actors::BlockResourceState::new(1);
        assert_eq!(state.begin_prepass(budget), Ok(()));
        assert_eq!(state.open_external_phase(), Ok(()));
        pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
        let reservation =
          BlockResourceMeterExtension.prepare((), &RuntimeOrigin::none(), &call, &info, len);
        if limit != charged {
          assert_eq!(
            reservation,
            Err(InvalidTransaction::Custom(RESOURCE_RESERVATION_REJECTED).into())
          );
          assert_eq!(
            pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get(),
            Some(state)
          );
          continue;
        }
        let reserved = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
          .expect("successful prepare retains state"); // deos-bypass: panic-owner — exact-fit test requires prepare to retain the supplied current-block state.
        assert_eq!(reserved.usage().user_dispatch_used(), charged);
        let post_info = PostDispatchInfoOf::<RuntimeCall> {
          actual_weight: Some(Weight::zero()),
          pays_fee: polkadot_sdk::frame_support::dispatch::Pays::Yes,
        };
        assert_eq!(
          BlockResourceMeterExtension::post_dispatch_details(
            reservation.expect("exact envelope fits"), // deos-bypass: panic-owner — test sets the envelope to the exact SDK charge and asserts its retained reservation.
            &info,
            &post_info,
            len,
            &Err(polkadot_sdk::sp_runtime::DispatchError::Other(
              "failed dispatch"
            )),
          ),
          Ok(Weight::zero())
        );
        let settled = pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get()
          .expect("settlement retains state"); // deos-bypass: panic-owner — preceding successful post-dispatch writes the current-block state.
        assert_eq!(settled.usage().user_dispatch_used(), overhead);
        assert_eq!(settled.outstanding_reservations(), 0);
      }
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
      assert_eq!(
        reserved.usage().user_dispatch_used(),
        frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
          &RuntimeBlockWeights::get(),
          &info,
          0,
        ),
      );

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
      assert_eq!(
        settled.usage().user_dispatch_used(),
        RuntimeBlockWeights::get().get(info.class).base_extrinsic,
      );

      let lost = BlockResourceMeterExtension
        .prepare((), &origin, &call, &info, 0)
        .expect("second reservation fits after reclaim"); // deos-bypass: panic-owner — preceding settlement retains only base overhead, leaving room for another remark.
      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(external_state(1));
      assert_eq!(
        BlockResourceMeterExtension::post_dispatch_details(lost, &info, &actual, 0, &Ok(())),
        Err(InvalidTransaction::Custom(RESOURCE_SETTLEMENT_REJECTED).into())
      );
    });
  }
}
