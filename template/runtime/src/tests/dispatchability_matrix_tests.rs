use crate::{Runtime, configs::RuntimeBlockWeights};
use polkadot_sdk::frame_support::{
  dispatch::{DispatchClass, Pays},
  weights::Weight,
};

type ActorsWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
type AssetRegistryWeights = crate::weights::pallet_asset_registry::SubstrateWeight<Runtime>;
type GovernanceWeights = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
type OracleWeights = crate::weights::pallet_oracle::SubstrateWeight<Runtime>;
type RouterWeights = crate::weights::pallet_deos_router::SubstrateWeight<Runtime>;
type StakingWeights = crate::weights::pallet_staking::SubstrateWeight<Runtime>;
type TmcWeights = crate::weights::pallet_tmc::SubstrateWeight<Runtime>;

const EXPECTED_CUSTOM_CALL_FAMILIES: usize = 65;

#[derive(Clone, Copy)]
struct DispatchabilityRow {
  family: &'static str,
  class: DispatchClass,
  maximum_weight: Weight,
  success_payment: Pays,
}

fn normal(family: &'static str, maximum_weight: Weight) -> DispatchabilityRow {
  DispatchabilityRow {
    family,
    class: DispatchClass::Normal,
    maximum_weight,
    success_payment: Pays::Yes,
  }
}

fn actor_funded_success(family: &'static str, maximum_weight: Weight) -> DispatchabilityRow {
  DispatchabilityRow {
    family,
    class: DispatchClass::Normal,
    maximum_weight,
    success_payment: Pays::No,
  }
}

#[test]
fn every_custom_runtime_call_family_fits_its_dispatch_envelope_at_maximum_input() {
  use pallet_asset_registry::WeightInfo as _;
  use pallet_deos_actors::WeightInfo as _;
  use pallet_deos_router::WeightInfo as _;
  use pallet_governance::WeightInfo as _;
  use pallet_oracle::WeightInfo as _;
  use pallet_staking::WeightInfo as _;
  use pallet_tmc::WeightInfo as _;

  let close = crate::Actors::close_dispatch_weight_upper();
  let max_sweep = <Runtime as pallet_deos_actors::Config>::MaxSweepBatch::get();
  let max_winners = <Runtime as pallet_governance::Config>::MaxWinningVoteAccountsPerCall::get();
  let max_reward_claims =
    <Runtime as pallet_staking::Config>::MaxSecurityRewardClaimsPerCall::get();
  let observation_publish = OracleWeights::publish_ema_changed()
    .max(OracleWeights::publish_ema_changed_primary_first())
    .max(OracleWeights::publish_ema_changed_primary_existing())
    .max(OracleWeights::publish_ema_changed_secondary_first())
    .max(OracleWeights::publish_ema_changed_secondary_existing())
    .max(OracleWeights::publish_ema_changed_combined())
    .max(OracleWeights::publish_ema_changed_secondary_capacity())
    .max(OracleWeights::publish_ema_refresh())
    .max(OracleWeights::publish_last_value());

  let rows = [
    normal(
      "Actors.create_user_actor",
      ActorsWeights::create_user_actor(),
    ),
    normal(
      "Actors.create_user_actor_at_slot",
      ActorsWeights::create_user_actor_at_slot(),
    ),
    normal(
      "Actors.create_system_actor",
      ActorsWeights::create_system_actor().max(ActorsWeights::create_dormant_system_actor()),
    ),
    normal(
      "Actors.create_system_actor_at_sovereign_id",
      ActorsWeights::create_system_actor_at_sovereign_id(),
    ),
    normal(
      "Actors.pause_actor",
      ActorsWeights::pause_actor().saturating_add(close),
    ),
    normal(
      "Actors.resume_actor",
      ActorsWeights::resume_actor().saturating_add(close),
    ),
    actor_funded_success(
      "Actors.manual_trigger",
      ActorsWeights::manual_trigger().saturating_add(close),
    ),
    normal("Actors.close_actor", close),
    normal(
      "Actors.update_contract",
      ActorsWeights::update_contract().saturating_add(close),
    ),
    normal(
      "Actors.set_global_circuit_breaker",
      ActorsWeights::set_global_circuit_breaker(),
    ),
    normal(
      "Actors.clear_crossing_worker_fault",
      ActorsWeights::clear_crossing_worker_fault(),
    ),
    normal(
      "Actors.clear_observation_fanout_worker_fault",
      ActorsWeights::clear_observation_fanout_worker_fault(),
    ),
    normal(
      "Actors.clear_wakeup_worker_fault",
      ActorsWeights::clear_wakeup_worker_fault(),
    ),
    normal(
      "Actors.permissionless_sweep",
      ActorsWeights::permissionless_sweep().saturating_add(close),
    ),
    normal(
      "Actors.set_active_actor_limit",
      ActorsWeights::set_active_actor_limit(),
    ),
    normal(
      "Actors.permissionless_sweep_many",
      ActorsWeights::permissionless_sweep_many(max_sweep)
        .saturating_add(close.saturating_mul(u64::from(max_sweep))),
    ),
    normal("Actors.activate_actor", ActorsWeights::activate_actor()),
    normal("Actors.deactivate_actor", ActorsWeights::deactivate_actor()),
    normal("Actors.cancel_run", ActorsWeights::run_cancel()),
    normal(
      "AssetRegistry.register_foreign_asset",
      AssetRegistryWeights::register_foreign_asset(),
    ),
    normal(
      "AssetRegistry.register_foreign_asset_with_id",
      AssetRegistryWeights::register_foreign_asset_with_id(),
    ),
    normal(
      "AssetRegistry.link_existing_asset",
      AssetRegistryWeights::link_existing_asset(),
    ),
    normal(
      "AssetRegistry.migrate_location_key",
      AssetRegistryWeights::migrate_location_key(),
    ),
    normal(
      "Governance.record_winning_vote",
      GovernanceWeights::record_winning_vote(),
    ),
    normal(
      "Governance.record_winning_vote_batch",
      GovernanceWeights::record_winning_vote_batch(max_winners),
    ),
    normal(
      "Governance.submit_proposal",
      GovernanceWeights::submit_proposal(),
    ),
    normal(
      "Governance.submit_signed_proposal",
      GovernanceWeights::submit_signed_proposal(),
    ),
    normal(
      "Governance.prepare_payload_admission_witness",
      GovernanceWeights::prepare_payload_admission_witness(),
    ),
    normal(
      "Governance.resolve_proposal",
      GovernanceWeights::resolve_proposal(max_winners),
    ),
    normal(
      "Governance.reject_proposal",
      GovernanceWeights::reject_proposal(),
    ),
    normal("Governance.cast_vote", GovernanceWeights::cast_vote()),
    normal(
      "Governance.resolve_proposal_from_votes",
      GovernanceWeights::resolve_proposal_from_votes(max_winners),
    ),
    normal(
      "Governance.requeue_proposal_for_auto_finalization",
      GovernanceWeights::requeue_proposal_for_auto_finalization(),
    ),
    normal(
      "Governance.force_resolve_proposal_from_votes",
      GovernanceWeights::force_resolve_proposal_from_votes(max_winners),
    ),
    normal(
      "Governance.unlock_vote_power",
      GovernanceWeights::unlock_vote_power(),
    ),
    normal(
      "Oracle.register_feed",
      OracleWeights::register_feed_existing_producer()
        .max(OracleWeights::register_feed_new_producer()),
    ),
    normal("Oracle.pause_feed", OracleWeights::pause_feed()),
    normal("Oracle.resume_feed", OracleWeights::resume_feed()),
    normal("Oracle.deactivate_feed", OracleWeights::deactivate_feed()),
    normal("Oracle.publish", observation_publish),
    normal("DEOS Router.swap", RouterWeights::swap()),
    normal(
      "DEOS Router.update_router_fee",
      RouterWeights::update_router_fee(),
    ),
    normal("DEOS Router.swap_exact_output", RouterWeights::swap()),
    normal("DEOS Router.create_pool", RouterWeights::create_pool()),
    normal(
      "DEOS Staking.register_staking_asset",
      StakingWeights::register_staking_asset(),
    ),
    normal("DEOS Staking.sync_pool", StakingWeights::sync_pool()),
    normal("DEOS Staking.stake", StakingWeights::stake()),
    normal("DEOS Staking.unstake", StakingWeights::unstake()),
    normal(
      "DEOS Staking.recover_unowned_pool",
      StakingWeights::recover_unowned_pool(),
    ),
    normal(
      "DEOS Staking.fund_native_security_reward",
      StakingWeights::fund_native_security_reward(),
    ),
    normal(
      "DEOS Staking.claim_native_security_reward",
      StakingWeights::claim_native_security_reward(),
    ),
    normal(
      "DEOS Staking.claim_native_security_reward_batch",
      StakingWeights::claim_native_security_reward_batch(max_reward_claims),
    ),
    normal(
      "DEOS Staking.claim_and_compound_native_security_reward",
      StakingWeights::claim_and_compound_native_security_reward(),
    ),
    normal(
      "DEOS Staking.expire_native_security_reward",
      StakingWeights::expire_native_security_reward(),
    ),
    normal(
      "DEOS Staking.lock_native_lp_for_collator",
      StakingWeights::lock_native_lp_for_collator(),
    ),
    normal(
      "DEOS Staking.request_unlock_native_lp",
      StakingWeights::request_unlock_native_lp(),
    ),
    normal(
      "DEOS Staking.withdraw_unlocked_native_lp",
      StakingWeights::withdraw_unlocked_native_lp(),
    ),
    normal(
      "DEOS Staking.redelegate_native_lp",
      StakingWeights::redelegate_native_lp(),
    ),
    normal(
      "DEOS Staking.lock_native_lp_for_governance",
      StakingWeights::lock_native_lp_for_governance(),
    ),
    normal(
      "DEOS Staking.request_unlock_native_lp_for_governance",
      StakingWeights::request_unlock_native_lp_for_governance(),
    ),
    normal(
      "DEOS Staking.withdraw_unlocked_native_lp_for_governance",
      StakingWeights::withdraw_unlocked_native_lp_for_governance(),
    ),
    normal(
      "DEOS Staking.lock_native_asset_for_governance",
      StakingWeights::lock_native_asset_for_governance(),
    ),
    normal(
      "DEOS Staking.request_unlock_native_asset_for_governance",
      StakingWeights::request_unlock_native_asset_for_governance(),
    ),
    normal(
      "DEOS Staking.withdraw_unlocked_native_asset_for_governance",
      StakingWeights::withdraw_unlocked_native_asset_for_governance(),
    ),
    normal("TMC.create_curve", TmcWeights::create_curve()),
  ];

  assert_eq!(
    rows.len(),
    EXPECTED_CUSTOM_CALL_FAMILIES,
    "custom runtime call-family inventory changed"
  );
  let block_weights = RuntimeBlockWeights::get();
  for row in rows {
    if row.family == "Actors.manual_trigger" {
      assert_eq!(row.success_payment, Pays::No);
    } else {
      assert_eq!(row.success_payment, Pays::Yes);
    }
    let class_limits = block_weights.get(row.class);
    let max_extrinsic = class_limits.max_extrinsic.unwrap_or(
      class_limits
        .max_total
        .expect("dispatch class must have a total limit"),
    );
    let max_total = class_limits
      .max_total
      .expect("dispatch class must have a total limit");
    assert!(
      row.maximum_weight.all_lte(max_extrinsic),
      "{} maximum {:?} exceeds {:?} max_extrinsic {:?}",
      row.family,
      row.maximum_weight,
      row.class,
      max_extrinsic,
    );
    assert!(
      row.maximum_weight.all_lte(max_total),
      "{} maximum {:?} exceeds {:?} max_total {:?}",
      row.family,
      row.maximum_weight,
      row.class,
      max_total,
    );
  }
}
