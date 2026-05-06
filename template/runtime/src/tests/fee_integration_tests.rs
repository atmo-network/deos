use super::common::{ALICE, INITIAL_BALANCE, aaa_fee_sink_account, new_test_ext};
use crate::{AAA, Balances, Staking, configs::RuntimeFeeSplit};
use polkadot_sdk::frame_support::{
  traits::{
    Currency, Hooks, OnUnbalanced,
    fungible::Balanced,
    tokens::{Fortitude, Precision, Preservation},
  },
  weights::Weight,
};

#[test]
fn transaction_fee_split_routes_to_fee_sink_when_author_is_unresolved() {
  new_test_ext().execute_with(|| {
    let fee_sink = aaa_fee_sink_account();
    let amount = 1_000_000_000_000u128;
    let sink_before = Balances::free_balance(&fee_sink);
    let credit = <Balances as Balanced<_>>::withdraw(
      &ALICE,
      amount,
      Precision::Exact,
      Preservation::Preserve,
      Fortitude::Polite,
    )
    .expect("Alice has enough balance for fee withdrawal");
    RuntimeFeeSplit::on_unbalanced(credit);
    assert_eq!(Balances::free_balance(&fee_sink), sink_before + amount);
    assert_eq!(Balances::free_balance(&ALICE), INITIAL_BALANCE - amount);
  });
}

#[test]
fn fee_sink_actor_splits_phase1_native_flow_to_staking_and_lp_ingress() {
  new_test_ext().execute_with(|| {
    let fee_sink = aaa_fee_sink_account();
    let staking_pool = Staking::pool_account_for(0);
    let lp_reward = Staking::lp_reward_account_for(0);
    let amount = 1_000_000_000_000u128;
    let pool_before = Balances::free_balance(&staking_pool);
    let lp_before = Balances::free_balance(&lp_reward);
    let _ = <Balances as Currency<_>>::deposit_creating(&fee_sink, amount);
    crate::System::set_block_number(2);
    let _ = AAA::on_initialize(2);
    let _ = AAA::on_idle(2, Weight::from_parts(u64::MAX, u64::MAX));
    let distributable = amount - crate::EXISTENTIAL_DEPOSIT;
    assert_eq!(
      Balances::free_balance(&staking_pool),
      pool_before + distributable / 2
    );
    assert_eq!(
      Balances::free_balance(&lp_reward),
      lp_before + distributable / 2
    );
    assert_eq!(
      Balances::free_balance(&fee_sink),
      crate::EXISTENTIAL_DEPOSIT
    );
  });
}
