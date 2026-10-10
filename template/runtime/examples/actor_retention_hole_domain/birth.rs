//! Additional read and codec inputs for a Manual/Parked birth replacing activation.
//! These are conditional inputs; the signed complete-block witness is a separate owner.
use codec::{Decode, Encode, MaxEncodedLen};
use deos_runtime::{
  AccountId, Address, Nonce, Runtime, RuntimeCall, Signature, TxExtension, UncheckedExtrinsic,
};
use pallet_deos_actors::{
  ActorContractOf, ActorContractTailChunks, ActorIdentityCount, DependencySourceAllocatorState,
  NextActorId, OwnerSlotBitmaps, SovereignIndex, UserCreationEnabled,
};
use polkadot_sdk::{
  frame_support::{
    dispatch::GetDispatchInfo,
    traits::{StorageInfo, StorageInfoTrait},
  },
  frame_system, pallet_transaction_payment,
  sp_core::sr25519,
  sp_runtime::generic,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[path = "../actor_retention_fixture/wide_contract.rs"]
mod wide_contract;

fn transaction_dispatch_info(transaction: &UncheckedExtrinsic) -> Value {
  let info = transaction.get_dispatch_info();
  let pair =
    |weight: polkadot_sdk::frame_support::weights::Weight| [weight.ref_time(), weight.proof_size()];
  json!({"callWeight": pair(info.call_weight), "extensionWeight": pair(info.extension_weight),
    "totalWeight": pair(info.total_weight()), "class": format!("{:?}", info.class)})
}

fn row(
  key: Vec<u8>,
  infos: Vec<StorageInfo>,
  stems: &BTreeSet<Vec<u8>>,
  queries: usize,
  absent: bool,
) -> Value {
  assert_eq!(infos.len(), 1);
  let info = &infos[0];
  assert_eq!(info.prefix, key[..32]);
  assert!((32..=80).contains(&key.len()) && (1..=3).contains(&queries));
  let typed_value = info.max_size.unwrap() as usize - (key.len() - 32);
  // A coherent unused monotone Actor ID has no backend tail. Later tail/hold reads
  // see the newly written overlay. Keep one compact-zero allowance conservatively.
  let value = if absent { 0 } else { typed_value };
  let blob = value + codec::Compact(value as u32).encoded_size();
  let depths = stems
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded native namespace stems
    .filter_map(|stem| {
      let matching = stem
        .iter() // deos-bypass: bounded-iter -- at most 32 bytes in a namespace stem
        .flat_map(|byte| [byte >> 4, byte & 15])
        .zip(key[..32].iter().flat_map(|byte| [byte >> 4, byte & 15])) // deos-bypass: bounded-iter -- fixed 32-byte namespace prefix
        .take_while(|(left, right)| left == right)
        .count();
      (matching < 64).then_some(matching)
    })
    .collect::<BTreeSet<_>>();
  let global = depths.len() + 1;
  let read = 650 * (global + 2 * queries * (key.len() - 32)) + queries * (1284 + blob) + 5;
  json!({"owner": format!("Actors::{}", String::from_utf8(info.storage_name.clone()).unwrap()),
    "namespacePrefix": info.prefix.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed 32-byte storage prefix
    "maxKeyBytes": key.len(), "queries": queries, "globalAncestorNodes": global,
    "typedMaxEncodedValueBytes": typed_value, "valueAllowanceBytes": value,
    "valueBlobAllowanceBytes": blob, "backendAbsent": absent, "readInputBytes": read})
}

pub fn input(stems: &BTreeSet<Vec<u8>>) -> Value {
  let owner = AccountId::new([1; 32]);
  let tails = (<Runtime as pallet_deos_actors::Config>::MaxContractSteps::get() - 1)
    .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK) as usize;
  assert_eq!(tails, 3);
  let additions = vec![
    row(
      UserCreationEnabled::<Runtime>::hashed_key().to_vec(),
      UserCreationEnabled::<Runtime>::storage_info(),
      stems,
      1,
      false,
    ),
    row(
      ActorIdentityCount::<Runtime>::hashed_key().to_vec(),
      ActorIdentityCount::<Runtime>::storage_info(),
      stems,
      1,
      false,
    ),
    row(
      NextActorId::<Runtime>::hashed_key().to_vec(),
      NextActorId::<Runtime>::storage_info(),
      stems,
      1,
      false,
    ),
    row(
      OwnerSlotBitmaps::<Runtime>::hashed_key_for(&owner),
      OwnerSlotBitmaps::<Runtime>::storage_info(),
      stems,
      1,
      false,
    ),
    row(
      ActorContractTailChunks::<Runtime>::hashed_key_for(0u64, 0u32),
      ActorContractTailChunks::<Runtime>::storage_info(),
      stems,
      tails,
      true,
    ),
    row(
      SovereignIndex::<Runtime>::hashed_key_for(&owner),
      SovereignIndex::<Runtime>::storage_info(),
      stems,
      1,
      true,
    ),
  ];
  let call = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
    mutability: pallet_deos_actors::Mutability::Mutable,
    contract: None,
  });
  let declared = call.get_dispatch_info().call_weight;
  let encoded_extension = (
    frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    frame_system::CheckNonce::<Runtime>::from(Nonce::MAX - 1),
    pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  )
    .encode();
  let mut input = &encoded_extension[..];
  let extension = TxExtension::decode(&mut input).unwrap();
  assert!(input.is_empty());
  // Codec-only maximum-width Close-target shape. Admission/read-family reuse is
  // justified separately from this pure constructor and typed length check.
  let assets = [1u32, 2, 3, 4];
  let owners = (1..=40)
    .map(|value| AccountId::new([value; 32]))
    .collect::<Vec<_>>();
  let mut wide = pallet_deos_actors::ActorContract {
    trigger: pallet_deos_actors::Trigger::manual(),
    cooldown_blocks: 0,
    window: None,
    steps: Vec::new().try_into().unwrap(),
    funding: pallet_deos_actors::FundingSourcePolicy::OwnerOnly,
    on_unfunded: pallet_deos_actors::OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: Some(
      pallet_deos_actors::ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        assets
          .into_iter()
          .map(|asset| pallet_deos_actors::ParkedBalanceRule {
            asset: primitives::AssetKind::Local(asset),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    ),
    auto_close_at_cycle_nonce: None,
  };
  wide_contract::wide_close_contract(&assets, &owners, &mut wide);
  let wide_body_bytes = wide.encoded_size();
  assert!(wide_body_bytes <= ActorContractOf::<Runtime>::max_encoded_len());
  let wide_trigger_bytes = wide.trigger.encoded_size();
  let wide_funding_bytes = wide.funding.encoded_size();
  let wide_step_bytes = wide.steps[0].encoded_size();
  let wide_call = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor {
    mutability: pallet_deos_actors::Mutability::Mutable,
    contract: Some(wide.clone()),
  });
  let wide_call_bytes = wide_call.encoded_size();
  assert_eq!(wide_call.get_dispatch_info().call_weight, declared);
  let wide_transaction = UncheckedExtrinsic::new_signed(
    wide_call,
    Address::Id(owner.clone()),
    Signature::Sr25519(sr25519::Signature::from_raw([0; 64])),
    extension.clone(),
  );
  let wide_wire_bytes = wide_transaction.encoded_size();
  // First parked publication can allocate the fixed sources. The allocator is
  // one backend key; later writes/reads of it share the transaction overlay.
  let initial_allocator = row(
    DependencySourceAllocatorState::<Runtime>::hashed_key().to_vec(),
    DependencySourceAllocatorState::<Runtime>::storage_info(),
    stems,
    1,
    false,
  );
  let initial_call = RuntimeCall::Actors(pallet_deos_actors::Call::create_user_actor_at_slot {
    owner_slot: 0,
    mutability: pallet_deos_actors::Mutability::Mutable,
    contract: None,
  });
  let initial_declared = initial_call.get_dispatch_info().call_weight;
  let initial_call_empty_bytes = initial_call.encoded_size();
  let initial_transaction = UncheckedExtrinsic::new_signed(
    initial_call,
    Address::Id(owner.clone()),
    Signature::Sr25519(sr25519::Signature::from_raw([0; 64])),
    extension.clone(),
  );
  let initial_wire = initial_transaction.encode();
  let mut initial_payload_input = &initial_wire[..];
  let codec::Compact(initial_payload_empty) =
    codec::Compact::<u32>::decode(&mut initial_payload_input).unwrap();
  assert_eq!(initial_payload_input.len(), initial_payload_empty as usize);
  let initial_payload_max =
    initial_payload_empty as usize + ActorContractOf::<Runtime>::max_encoded_len();
  let initial_wire_max = initial_payload_max
    + codec::Compact(u32::try_from(initial_payload_max).unwrap()).encoded_size();
  let temporary_dispatch = [
    RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor { actor_id: 0 }),
    RuntimeCall::Actors(pallet_deos_actors::Call::activate_actor {
      actor_id: 0,
      contract: wide,
    }),
  ]
  .map(|call| {
    transaction_dispatch_info(&UncheckedExtrinsic::new_signed(
      call,
      Address::Id(owner.clone()),
      Signature::Sr25519(sr25519::Signature::from_raw([0; 64])),
      extension.clone(),
    ))
  });
  let call_empty_bytes = call.encoded_size();
  let transaction = UncheckedExtrinsic::new_signed(
    call,
    Address::Id(owner),
    Signature::Sr25519(sr25519::Signature::from_raw([0; 64])),
    extension,
  );
  let wire_empty_bytes = transaction.encoded_size();
  // Some/None have equal discriminant widths. MaxEncodedLen bounds every admitted
  // body, including the selected smaller no-policy StopCycle family. Subtract the
  // actual outer Compact prefix and recompute it at the conservative payload upper.
  let body_max = ActorContractOf::<Runtime>::max_encoded_len();
  let wire = transaction.encode();
  let mut payload_input = &wire[..];
  let codec::Compact(payload_empty) = codec::Compact::<u32>::decode(&mut payload_input).unwrap();
  assert_eq!(payload_input.len(), payload_empty as usize);
  let payload_max = payload_empty as usize + body_max;
  let wire_max = payload_max + codec::Compact(u32::try_from(payload_max).unwrap()).encoded_size();
  let ordinary_dispatch = transaction_dispatch_info(&transaction);
  let wide_dispatch = transaction_dispatch_info(&wide_transaction);
  assert_eq!(ordinary_dispatch, wide_dispatch);
  json!({"scope": "conditional_manual_parked_birth_replacing_activation",
    "additionalReadRoles": additions, "declaredWeight": [declared.ref_time(), declared.proof_size()],
    "transactionDispatchInfo": {"deactivate": temporary_dispatch[0], "activate": temporary_dispatch[1],
      "birth": ordinary_dispatch, "firstSourceBirth": transaction_dispatch_info(&initial_transaction)},
    "contractMaxEncodedLen": body_max, "emptyCallBytes": call_empty_bytes,
    "maximumCallBytes": call_empty_bytes + body_max,
    "codecEmptyWireBytes": wire_empty_bytes, "codecEmptyPayloadBytes": payload_empty,
    "maximumWireBytes": wire_max,
    "initialSourceBirthInput": {"allocatorReadRole": initial_allocator,
      "sourceCount": 4, "emptyCallBytes": initial_call_empty_bytes,
      "maximumCallBytes": initial_call_empty_bytes + body_max,
      "codecEmptyWireBytes": initial_wire.len(), "codecEmptyPayloadBytes": initial_payload_empty,
      "maximumWireBytes": initial_wire_max,
      "declaredWeight": [initial_declared.ref_time(), initial_declared.proof_size()]},
    "wideCloseTargetCodec": {"contractBytes": wide_body_bytes, "callBytes": wide_call_bytes,
      "wireBytes": wide_wire_bytes, "triggerBytes": wide_trigger_bytes, "fundingBytes": wide_funding_bytes,
      "stepBytes": wide_step_bytes, "steps": 12, "predicatesPerStep": 4, "splitLegs": 4,
      "sourceAllowlist": 16, "assetAllowlist": 16, "fundingAllowlist": 16,
      "windowStart": 1, "windowEnd": u32::MAX - 1,
      "cooldownBlocks": <Runtime as pallet_deos_actors::Config>::MaxExecutionDelayBlocks::get(),
      "declaredWeight": [declared.ref_time(), declared.proof_size()]},
    "premises": ["Manual Trigger, no ScheduleWindow/close_when, on_unfunded Close; four fixed LP watches",
      "Coherent unused monotone Actor ID and unused automatic owner slot; bounded identity/Active caps",
      "No economic ingress; newly written Contract head/tails are overlay-visible",
      "Same funded owner/currency role bounds as the successful signed temporary pair; at most two signed calls",
      "Immortal era, bounded u32 nonce, zero tip and disabled metadata hash, as in the closed recipe"],
    "exclusions": ["AddressEvent/window target birth and initial public prefix", "physical Weight/CPU/root costs",
      "whole repetition/horizon and complete Close/tariffs/production binding"]})
}
