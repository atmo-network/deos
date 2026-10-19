//! Real market construction and authored counterpart to the shared-recipient Transfer profile.
use super::*;
use pallet_deos_actors::{Precondition, Predicate};
use primitives::AssetKind;

pub(super) const FOREIGN: u32 = 0xF000_0001;
pub(super) const WITNESSES: [u32; 2] = [500_001, 500_002];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Workload {
  Transfer,
  RouterFourPredicates,
}

impl Workload {
  pub(super) fn predicates(self) -> u32 {
    if self == Self::Transfer { 0 } else { 4 }
  }

  pub(super) fn steps(self) -> ContractSteps<Runtime> {
    if self == Self::Transfer {
      return actors_integration_tests::transfer_contract_steps(
        super::super::super::common::BOB,
        AssetKind::Native,
        crate::EXISTENTIAL_DEPOSIT,
      );
    }
    let predicates = [
      AssetKind::Native,
      AssetKind::Foreign(FOREIGN),
      AssetKind::Local(WITNESSES[0]),
      AssetKind::Local(WITNESSES[1]),
    ]
    .into_iter()
    .map(|asset| Predicate::BalanceAbove {
      asset,
      threshold: 1,
    })
    .collect::<Vec<_>>();
    vec![StepOf::<Runtime> {
      precondition: Some(Precondition {
        clauses: vec![predicates.try_into().unwrap()].try_into().unwrap(),
      }),
      task: Task::SwapOut {
        asset_in: AssetKind::Foreign(FOREIGN),
        asset_out: AssetKind::Native,
        amount_out: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
        input_limit: InputLimit::Absolute(10 * crate::EXISTENTIAL_DEPOSIT),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap()
  }
}

fn mint(asset: u32, account: &crate::AccountId, amount: u128) {
  let issuer = polkadot_sdk::pallet_assets::Asset::<Runtime>::get(asset)
    .unwrap()
    .issuer;
  assert_ok!(Assets::mint(
    RuntimeOrigin::signed(issuer),
    asset,
    account.clone().into(),
    amount
  ));
}

pub(super) fn prepare_market() {
  let depth = 1_000_000_000_000_000_000_000_000u128;
  add_balance(&ALICE, depth.checked_mul(4).unwrap());
  mint(FOREIGN, &ALICE, depth.checked_mul(2).unwrap());
  for asset in WITNESSES {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      asset,
      ALICE.into(),
      true,
      1
    ));
  }
  let foreign = AssetKind::Foreign(FOREIGN);
  super::super::super::common::ensure_asset_conversion_pool(AssetKind::Native, foreign);
  assert_ok!(super::super::super::common::add_liquidity(
    RuntimeOrigin::signed(ALICE),
    AssetKind::Native,
    foreign,
    depth,
    depth,
    1,
    1,
    &ALICE
  ));
}

pub(super) fn input_cap() -> u128 {
  crate::EXISTENTIAL_DEPOSIT.checked_mul(10).unwrap()
}

pub(super) fn fund(account: &crate::AccountId, blocks: u32) {
  fund_actions(account, blocks.checked_add(1).unwrap());
}

pub(super) fn fund_actions(account: &crate::AccountId, actions: u32) {
  let foreign = input_cap()
    .checked_mul(u128::from(actions))
    .unwrap()
    .checked_add(10)
    .unwrap();
  mint(FOREIGN, account, foreign);
  for asset in WITNESSES {
    mint(asset, account, 10);
  }
}
