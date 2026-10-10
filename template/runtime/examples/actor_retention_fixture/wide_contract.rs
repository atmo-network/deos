//! Pure maximum-width no-policy Close-target Contract shared by native diagnostics.
use super::Runtime;
use codec::{Encode, MaxEncodedLen};
use polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor;
type AccountId = <Runtime as polkadot_sdk::frame_system::Config>::AccountId;
use pallet_deos_actors::{
  ActorContractOf, AmountResolution, AssetFilter, Config, ContractSteps, FundingSourcePolicy,
  FundingSourcePolicyOf, PreconditionOf, Predicate, ScheduleWindow, SourceFilter, Step,
  StepErrorPolicy, Task, TaskOf, Trigger, TriggerOf,
};
use primitives::{AssetKind, LocalPoolObservationMethod, OracleAggregationId, OracleFeedId};
use std::collections::BTreeSet;

pub fn wide_close_steps(assets: &[u32]) -> ContractSteps<Runtime> {
  let predicates = <Runtime as Config>::MaxPredicatesPerStep::get() as usize;
  assert_eq!(predicates, 4);
  assert!(!assets.is_empty());
  // Maximum-width parked bodies may name unfunded identities without adding watches.
  let mut step_assets = assets.to_vec();
  for index in 0..predicates as u32 {
    if step_assets.len() >= predicates {
      break;
    }
    let asset = primitives::assets::TYPE_LP | index;
    if !step_assets.contains(&asset) {
      step_assets.push(asset);
    }
  }
  let assets = step_assets.as_slice();
  let clauses = (0..predicates)
    .map(|index| {
      let predicate = Predicate::ObservationAbove {
        feed: OracleFeedId::directional_local_pool_price(
          AssetKind::Local(assets[index]),
          AssetKind::Local(assets[(index + 1) % assets.len()]),
          LocalPoolObservationMethod::PreExecutionSpot,
          OracleAggregationId::Ema {
            half_life_blocks: 1,
          },
          12,
        ),
        threshold: 1,
        max_age_blocks: 1,
      };
      assert_eq!(predicate.encoded_size(), Predicate::<AssetKind, <Runtime as Config>::Balance, BlockNumberFor<Runtime>, OracleFeedId>::max_encoded_len());
      vec![predicate].try_into().unwrap()
    })
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();
  let mut precondition = PreconditionOf::<Runtime> { clauses };
  precondition.clauses.sort_by_key(Encode::encode);
  assert_eq!(precondition.predicate_count(), predicates as u32);
  let step = Step {
    precondition: Some(precondition),
    task: Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(assets[2]),
      asset_a: AssetKind::Local(assets[0]),
      asset_b: AssetKind::Local(assets[1]),
      lp_amount: AmountResolution::Fixed(u128::MAX),
      min_amount_a: u128::MAX,
      min_amount_b: u128::MAX,
    },
    on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
  };
  assert_eq!(
    step.task.encoded_size(),
    TaskOf::<Runtime>::max_encoded_len()
  );
  vec![step; <Runtime as Config>::MaxContractSteps::get() as usize]
    .try_into()
    .unwrap()
}

pub fn wide_close_contract(
  assets: &[u32],
  owners: &[AccountId],
  contract: &mut ActorContractOf<Runtime>,
) {
  let count = <Runtime as Config>::MaxWhitelistSize::get() as usize;
  assert_eq!(count, 16);
  // Alice's custody preparation must not activate the independent AddressEvent Trigger.
  let mut sources = owners[1..=count].to_vec();
  sources.sort_by_key(Encode::encode);
  let mut filter_assets = assets
    .iter(/* deos-bypass: bounded-iter -- supplied bounded LP identities. */)
    .copied()
    .map(AssetKind::Local)
    .collect::<Vec<_>>();
  // AddressEvent filters admit asset identities without allocating their ledgers.
  for id in 0..count as u32 {
    if filter_assets.len() == count {
      break;
    }
    let asset = AssetKind::Local(id);
    if !filter_assets.contains(&asset) {
      filter_assets.push(asset);
    }
  }
  filter_assets.sort_by_key(Encode::encode);
  filter_assets.dedup();
  assert_eq!(filter_assets.len(), count);
  assert!(
    filter_assets
      .iter(/* deos-bypass: bounded-iter -- the sixteen canonical AddressEvent assets above. */)
      .all(|asset| asset.encoded_size() == AssetKind::max_encoded_len())
  );
  contract.steps = wide_close_steps(assets);
  contract.trigger = Trigger::address_event(
    SourceFilter::Whitelist(sources.try_into().unwrap()),
    AssetFilter::Whitelist(filter_assets.try_into().unwrap()),
  );
  contract.funding = FundingSourcePolicy::SignedAllowlist(
    owners[..count]
      .iter(/* deos-bypass: bounded-iter -- the sixteen fixed signed funding accounts. */)
      .cloned()
      .collect::<BTreeSet<_>>()
      .try_into()
      .unwrap(),
  );
  contract.cooldown_blocks = <Runtime as Config>::MaxExecutionDelayBlocks::get();
  contract.window = Some(ScheduleWindow {
    start: 1,
    end: u32::MAX - 1,
  });
  assert!(contract.auto_close_at_cycle_nonce.is_none());
  assert_eq!(
    contract.trigger.encoded_size(),
    TriggerOf::<Runtime>::max_encoded_len()
  );
  assert_eq!(
    contract.funding.encoded_size(),
    FundingSourcePolicyOf::<Runtime>::max_encoded_len()
  );
  assert_eq!(
    contract
      .parked_balance_activation
      .as_ref()
      .unwrap()
      .watches
      .len(),
    assets
      .len()
      .min(<Runtime as Config>::MaxParkedBalanceWatches::get() as usize)
  );
}
