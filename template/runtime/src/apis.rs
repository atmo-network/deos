// External crates imports
use alloc::{collections::BTreeSet, vec::Vec};

use polkadot_sdk::*;

use frame_support::{
  genesis_builder_helper::{build_state, get_preset},
  weights::Weight,
};
use pallet_aura::Authorities;
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{OpaqueMetadata, crypto::KeyTypeId};
use sp_runtime::{
  ApplyExtrinsicResult,
  traits::Block as BlockT,
  transaction_validity::{TransactionSource, TransactionValidity},
};
use sp_version::RuntimeVersion;

// Local module imports
use super::{
  AccountId, Actors, AssetConversion, Balance, Block, BlockNumber, ConsensusHook, Executive,
  InherentDataExt, Nonce, ParachainInfo, ParachainSystem, Runtime, RuntimeCall,
  RuntimeGenesisConfig, SLOT_DURATION, SessionKeys, System, TransactionPayment, VERSION,
};
use crate::configs::{
  AssetKind, MaxInboundDownwardMessagesPerContext, MaxInboundHorizontalChannelsPerContext,
  MaxInboundHorizontalMessagesPerContext,
};

pub(crate) const CONTEXT_GEOMETRY_INHERENT_IDENTIFIER: sp_inherents::InherentIdentifier =
  *b"deosctx0";

pub(crate) fn context_geometry_fatal_result() -> sp_inherents::CheckInherentsResult {
  let mut result = sp_inherents::CheckInherentsResult::new();
  result
    .put_error(
      CONTEXT_GEOMETRY_INHERENT_IDENTIFIER,
      &sp_inherents::MakeFatalError::from(()),
    )
    .expect("a fresh fixed-size context-geometry error must encode"); // deos-bypass: panic-owner — CheckInherentsResult is fresh and unit MakeFatalError has fixed infallible SCALE encoding.
  result
}

pub(crate) fn validate_context_inherent_geometry(
  data: &sp_inherents::InherentData,
) -> Result<(), pallet_deos_actors::BlockResourceError> {
  use frame_support::inherent::ProvideInherent;

  let Some(cumulus_pallet_parachain_system::Call::set_validation_data {
    inbound_messages_data,
    ..
  }) = <ParachainSystem as ProvideInherent>::create_inherent(data)
  else {
    return Ok(());
  };
  validate_inbound_messages_geometry(&inbound_messages_data)
}

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

impl Runtime {
  #[docify::export]
  fn impl_slot_duration() -> sp_consensus_aura::SlotDuration {
    sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
  }

  #[docify::export]
  fn impl_can_build_upon(
    included_hash: <Block as BlockT>::Hash,
    slot: cumulus_primitives_aura::Slot,
  ) -> bool {
    ConsensusHook::can_build_upon(included_hash, slot)
  }
}

impl_runtime_apis! {
    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            Runtime::impl_slot_duration()
        }

        fn authorities() -> Vec<AuraId> {
            Authorities::<Runtime>::get().into_inner()
        }
    }

    impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
        fn can_build_upon(
            included_hash: <Block as BlockT>::Hash,
            slot: cumulus_primitives_aura::Slot,
        ) -> bool {
            Runtime::impl_can_build_upon(included_hash, slot)
        }
    }

    impl cumulus_primitives_core::RelayParentOffsetApi<Block> for Runtime {
        fn relay_parent_offset() -> u32 {
            0
        }

        fn max_claim_queue_offset() -> u8 {
            1
        }
    }

    impl cumulus_primitives_core::GetParachainInfo<Block> for Runtime {
        fn parachain_id() -> cumulus_primitives_core::ParaId {
            ParachainInfo::parachain_id()
        }
    }

    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: <Block as BlockT>::LazyBlock) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl frame_support::view_functions::runtime_api::RuntimeViewFunction<Block> for Runtime {
        fn execute_view_function(id: frame_support::view_functions::ViewFunctionId, input: Vec<u8>) -> Result<Vec<u8>, frame_support::view_functions::ViewFunctionDispatchError> {
            Runtime::execute_view_function(id, input)
        }
    }

    impl pallet_deos_actors::ActorSimulationApi<
        Block,
        pallet_deos_actors::ActorContractOf<Runtime>,
        pallet_deos_actors::SimulationResult,
    > for Runtime {
        fn simulate_current_contract(
            actor_id: pallet_deos_actors::ActorId,
            expected_type: pallet_deos_actors::ActorType,
            expected_mutability: pallet_deos_actors::Mutability,
            expected_contract: pallet_deos_actors::ActorContractOf<Runtime>,
            mode: pallet_deos_actors::SimulationMode,
            budget: pallet_deos_actors::SimulationBudget,
        ) -> Result<pallet_deos_actors::SimulationResult, pallet_deos_actors::SimulationError> {
            Actors::simulate_current_contract(
                actor_id,
                expected_type,
                expected_mutability,
                expected_contract,
                mode,
                budget,
            )
        }
    }

    impl pallet_deos_actors::ActorCostApi<Block, Balance> for Runtime {
        fn actor_cost_quote(
            actor_id: pallet_deos_actors::ActorId,
        ) -> Result<
            pallet_deos_actors::ActorCostQuote<Balance>,
            pallet_deos_actors::ActorCostQuoteError,
        > {
            Actors::actor_cost_quote(actor_id)
        }
    }

    impl pallet_deos_actors::ActorResourceApi<Block, BlockNumber> for Runtime {
  fn block_resource_budget() -> pallet_deos_actors::BlockResourceBudget {
    crate::configs::BlockResourceBudgetValue::get()
  }

  fn current_block_resource_state() -> Option<pallet_deos_actors::BlockResourceState<BlockNumber>> {
    Actors::block_resource_state()
  }

  fn finalized_block_resource_snapshot(
  ) -> Option<pallet_deos_actors::FinalizedBlockResourceSnapshot<BlockNumber>> {
    Actors::finalized_block_resource_telemetry()
  }
}

impl pallet_deos_actors::ActorEligibilityApi<Block, primitives::OracleFeedId, BlockNumber>
        for Runtime {
        fn actor_eligibility(
            actor_id: pallet_deos_actors::ActorId,
        ) -> Result<
            pallet_deos_actors::ActorEligibility<primitives::OracleFeedId, BlockNumber>,
            pallet_deos_actors::ActorClassificationError,
        > {
            Actors::actor_eligibility(actor_id)
        }

        fn materialization_faults() -> pallet_deos_actors::MaterializationFaults<primitives::OracleFeedId, BlockNumber> {
            pallet_deos_actors::MaterializationFaults {
                crossing: Actors::crossing_worker_fault(),
                fanout: Actors::observation_fanout_worker_fault(),
                wakeup: Actors::wakeup_worker_fault(),
            }
        }

        fn crossing_capacity(feed: primitives::OracleFeedId) -> pallet_deos_actors::CrossingCapacity {
            pallet_deos_actors::CrossingCapacity {
                user_limit: <crate::configs::actor_config::ActorMaxUserCrossingMembersPerFeed as polkadot_sdk::frame_support::traits::Get<u32>>::get(),
                total_limit: <crate::configs::actor_config::ActorMaxCrossingMembersPerFeed as polkadot_sdk::frame_support::traits::Get<u32>>::get(),
                user_memberships: Actors::crossing_user_feed_membership_count(feed),
                total_memberships: Actors::crossing_feed_membership_count(feed),
            }
        }

    }

    impl primitives::TmctolReadModelApi<Block, AccountId, Balance> for Runtime {
        fn tmctol_guarantee_state() -> primitives::TmctolGuaranteeState<AccountId, Balance> {
            crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: <Block as BlockT>::LazyBlock,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            if validate_context_inherent_geometry(&data).is_err() {
                return context_geometry_fatal_result();
            }
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(
            owner: Vec<u8>,
            seed: Option<Vec<u8>>,
        ) -> sp_session::OpaqueGeneratedSessionKeys {
            SessionKeys::generate(owner.as_slice(), seed).into()
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
      for Runtime
    {
      fn query_call_info(
        call: RuntimeCall,
        len: u32,
      ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
        TransactionPayment::query_call_info(call, len)
      }
      fn query_call_fee_details(
        call: RuntimeCall,
        len: u32,
      ) -> pallet_transaction_payment::FeeDetails<Balance> {
        TransactionPayment::query_call_fee_details(call, len)
      }
      fn query_weight_to_fee(weight: Weight) -> Balance {
        TransactionPayment::weight_to_fee(weight)
      }
      fn query_length_to_fee(length: u32) -> Balance {
        TransactionPayment::length_to_fee(length)
      }
    }

    impl pallet_asset_conversion::AssetConversionApi<Block, Balance, AssetKind> for Runtime {
      fn quote_price_exact_tokens_for_tokens(
        asset1: AssetKind,
        asset2: AssetKind,
        amount: Balance,
        include_fee: bool,
      ) -> Option<Balance> {
        AssetConversion::quote_price_exact_tokens_for_tokens(asset1, asset2, amount, include_fee)
      }

      fn quote_price_tokens_for_exact_tokens(
        asset1: AssetKind,
        asset2: AssetKind,
        amount: Balance,
        include_fee: bool,
      ) -> Option<Balance> {
        AssetConversion::quote_price_tokens_for_exact_tokens(asset1, asset2, amount, include_fee)
      }

      fn get_reserves(asset1: AssetKind, asset2: AssetKind) -> Option<(Balance, Balance)> {
        AssetConversion::get_reserves(asset1, asset2).ok()
      }
    }

    impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
        fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
            ParachainSystem::collect_collation_info(header)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            use super::configs::RuntimeBlockWeights;

            let weight = Executive::try_runtime_upgrade(checks).unwrap(); // deos-bypass: panic-owner — TryRuntime traps failed checks.
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: <Block as BlockT>::LazyBlock,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap() // deos-bypass: panic-owner — TryRuntime traps failed checks.
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<polkadot_sdk::frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::BenchmarkList;
            use polkadot_sdk::frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
            use super::*;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        #[allow(non_local_definitions)]
        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
            use frame_benchmarking::{BenchmarkError, BenchmarkBatch};
            use super::*;

            use frame_system_benchmarking::Pallet as SystemBench;
            impl frame_system_benchmarking::Config for Runtime {
                fn setup_set_code_requirements(code: &Vec<u8>) -> Result<(), BenchmarkError> {
                    ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
                    Ok(())
                }

                fn verify_set_code() {
                    System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
                }
            }

            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
            impl cumulus_pallet_session_benchmarking::Config for Runtime {
                fn generate_session_keys_and_proof(owner: Self::AccountId) -> (Self::Keys, Vec<u8>) {
                    let keys = SessionKeys::generate(&codec::Encode::encode(&owner), None);
                    (keys.keys, codec::Encode::encode(&keys.proof))
                }
            }

            impl pallet_xcm::benchmarking::Config for Runtime {
                type DeliveryHelper = ();

                fn get_asset() -> polkadot_sdk::staging_xcm::latest::Asset {
                    use polkadot_sdk::staging_xcm::latest::prelude::*;
                    Asset { id: AssetId(Location::here()), fun: Fungible(EXISTENTIAL_DEPOSIT) }
                }
            }

            use polkadot_sdk::frame_support::traits::WhitelistedStorageKeys;
            let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, crate::genesis_config_presets::get_preset)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            crate::genesis_config_presets::preset_names()
        }
    }
}
