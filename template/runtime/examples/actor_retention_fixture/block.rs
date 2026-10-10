//! Shared native genesis, Actor authoring and consensus-block preparation.

#[path = "inherents.rs"]
mod inherents;
pub(crate) use inherents::inherents;

use codec::{Decode, Encode};
use deos_runtime::{
  AccountId, Actors, Address, Balances, Block, Runtime, RuntimeCall, RuntimeGenesisConfig,
  RuntimeOrigin, Signature, System, TxExtension, UncheckedExtrinsic,
};
use pallet_deos_actors::{
  ActorContract, ActorContractOf, ActorId, FundingSourcePolicy, Mutability, OnUnfunded,
  StepErrorPolicy, StepOf, TaskOf, TriggerOf,
};
use polkadot_sdk::{
  frame_support::assert_ok,
  sp_core::{Pair, storage::Storage},
  sp_genesis_builder::{PresetId, runtime_decl_for_genesis_builder::GenesisBuilder},
  sp_runtime::{BuildStorage, MultiAddress, generic},
};

fn merge_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
  match (target, patch) {
    (serde_json::Value::Object(target), serde_json::Value::Object(patch)) => {
      for (key, value) in patch {
        merge_patch(target.entry(key).or_insert(serde_json::Value::Null), value);
      }
    }
    (target, patch) => *target = patch,
  }
}

pub(crate) fn preset_storage(preset: &PresetId) -> Storage {
  let patch = <Runtime as GenesisBuilder<Block>>::get_preset(&Some(preset.clone())).unwrap();
  let mut config = serde_json::to_value(RuntimeGenesisConfig::default()).unwrap();
  merge_patch(&mut config, serde_json::from_slice(&patch).unwrap());
  serde_json::from_value::<RuntimeGenesisConfig>(config)
    .unwrap()
    .build_storage()
    .unwrap()
}

pub(crate) fn contract(
  trigger: TriggerOf<Runtime>,
  tasks: Vec<TaskOf<Runtime>>,
) -> ActorContractOf<Runtime> {
  ActorContract {
    trigger,
    cooldown_blocks: 0,
    window: None,
    steps: tasks
      .into_iter()
      .map(|task| StepOf::<Runtime> {
        precondition: None,
        task,
        on_error: StepErrorPolicy::AbortCycle,
      })
      .collect::<Vec<_>>()
      .try_into()
      .unwrap(),
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: None,
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  }
}

pub(crate) fn create_user(
  owner: &AccountId,
  contract: Option<ActorContractOf<Runtime>>,
) -> ActorId {
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_user_actor(
    RuntimeOrigin::signed(owner.clone()),
    Mutability::Mutable,
    contract,
  ));
  id
}

pub(crate) fn native_transfer(from: &AccountId, to: &AccountId, amount: u128) {
  assert_ok!(Balances::transfer_keep_alive(
    RuntimeOrigin::signed(from.clone()),
    MultiAddress::Id(to.clone()),
    amount,
  ));
}

pub(crate) fn signed_lifecycle(
  pair: &polkadot_sdk::sp_core::sr25519::Pair,
  call: RuntimeCall,
) -> UncheckedExtrinsic {
  let owner = AccountId::from(pair.public());
  // Decode the public complete type: the retained extensions outside this tuple
  // encode unit values, including the runtime-private resource/ingress adapters.
  let bytes = (
    polkadot_sdk::frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    polkadot_sdk::frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&owner)),
    polkadot_sdk::pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  )
    .encode();
  let mut input = &bytes[..];
  let extension = TxExtension::decode(&mut input).unwrap();
  assert!(input.is_empty());
  let payload = generic::SignedPayload::new(call.clone(), extension.clone()).unwrap();
  let signature = payload.using_encoded(|encoded| pair.sign(encoded));
  UncheckedExtrinsic::new_signed(
    call,
    Address::Id(owner),
    Signature::Sr25519(signature),
    extension,
  )
}
