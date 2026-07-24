use super::{
  AddressEventIngress, PriceForParentDelivery, RuntimeAddressEventIngress, RuntimeFeeCollector,
};
use crate::{
  AccountId, AllPalletsWithSystem, AssetRegistry, Assets, Balance, Balances, ParachainInfo,
  ParachainSystem, PolkadotXcm, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, WeightToFee,
  XcmpQueue,
};
#[cfg(feature = "runtime-benchmarks")]
use alloc::boxed::Box;
use alloc::vec::Vec;

use polkadot_sdk::{
  staging_xcm as xcm, staging_xcm_builder as xcm_builder, staging_xcm_executor as xcm_executor, *,
};

#[cfg(feature = "runtime-benchmarks")]
use frame_support::traits::tokens::imbalance::{
  ImbalanceAccounting, UnsafeConstructorDestructor, UnsafeManualAccounting,
};
use frame_support::{
  parameter_types,
  traits::{ConstU32, Contains, ContainsPair, Everything, Nothing},
  weights::Weight,
};
use frame_system::EnsureRoot;
use pallet_xcm::XcmPassthrough;
use polkadot_sdk::polkadot_parachain_primitives::primitives::Sibling;
use polkadot_sdk::polkadot_sdk_frame::traits::Disabled;
use primitives::AssetKind;
use xcm::latest::Error as XcmError;
use xcm::latest::prelude::*;
use xcm_builder::{
  AccountId32Aliases, AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom,
  ConvertedConcreteId, DenyRecursively, DenyReserveTransferToRelayChain, DenyThenTry,
  EnsureXcmOrigin, FixedWeightBounds, FrameTransactionalProcessor, FungibleAdapter,
  FungiblesAdapter, IsConcrete, NativeAsset, NoChecking, ParentIsPreset, RelayChainAsNative,
  SiblingParachainAsNative, SiblingParachainConvertsVia, SignedAccountId32AsNative,
  SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId,
  UsingComponents, WithComputedOrigin, WithUniqueTopic,
};
use xcm_executor::{
  AssetsInHolding, XcmExecutor,
  traits::{ConvertLocation, JustTry, TransactAsset},
};

parameter_types! {
  // FixedWeightBounds cannot price DepositAsset by the number of held assets. Keep the holding
  // register single-asset until an instruction-specific weigher replaces this launch contract.
  pub const MaxAssetsIntoHolding: u32 = 1;
  pub const MaxInstructions: u32 = 100;
  pub const RelayLocation: Location = Location::parent();
  pub const RelayNetwork: Option<NetworkId> = None;
  pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
  // For the real deployment, it is recommended to set `RelayNetwork` according to the relay chain
  // and prepend `UniversalLocation` with `GlobalConsensus(RelayNetwork::get())`.
  pub UniversalLocation: InteriorLocation = Parachain(ParachainInfo::parachain_id().into()).into();
  pub UnitWeightCost: Weight =
    <crate::weights::pallet_aaa::SubstrateWeight<Runtime> as pallet_aaa::WeightInfo>::xcm_asset_deposit();
}

/// Type for specifying how a `Location` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
  // The parent (Relay-chain) origin converts to the parent `AccountId`.
  ParentIsPreset<AccountId>,
  // Sibling parachain origins convert to AccountId via the `ParaId::into`.
  SiblingParachainConvertsVia<Sibling, AccountId>,
  // Straight up local `AccountId32` origins just alias directly to `AccountId`.
  AccountId32Aliases<RelayNetwork, AccountId>,
);

/// Means for transacting assets on this chain.
pub type LocalAssetTransactor = FungibleAdapter<
  // Use this currency:
  Balances,
  // Use this currency when it is a fungible asset matching the given location or name:
  IsConcrete<RelayLocation>,
  // Do a simple punn to convert an AccountId32 Location into a native chain account ID:
  LocationToAccountId,
  // Our chain's account ID type (we can't get away without mentioning it explicitly):
  AccountId,
  // We don't track any teleports.
  (),
>;

/// This type generates a deterministic AssetId from a Location.
/// It is used by `pallet-asset-registry` to propose IDs for new assets.
pub struct LocationToAssetId;
impl polkadot_sdk::sp_runtime::traits::Convert<Location, u32> for LocationToAssetId {
  fn convert(location: Location) -> u32 {
    use codec::Encode;
    use polkadot_sdk::sp_io::hashing::blake2_256;
    use primitives::assets::{MASK_INDEX, TYPE_FOREIGN};

    let encoded = location.encode();
    let hash = blake2_256(&encoded);

    // Take first 4 bytes to form a u32
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&hash[0..4]);
    let derived_id = u32::from_le_bytes(bytes);

    // Map to Foreign namespace (0xF...)
    // This ensures no collision with Native, Local, Stable, etc.
    let asset_id = TYPE_FOREIGN | (derived_id & MASK_INDEX);

    asset_id
  }
}

pub type ForeignAssetsTransactor = FungiblesAdapter<
  Assets,
  ConvertedConcreteId<u32, Balance, AssetRegistry, JustTry>,
  LocationToAccountId,
  AccountId,
  NoChecking,
  CheckingAccount,
>;

pub struct CheckingAccount;
impl frame_support::traits::Get<AccountId> for CheckingAccount {
  fn get() -> AccountId {
    AccountId::from([0u8; 32])
  }
}

pub struct ForeignAssetsFromSibling;
impl ContainsPair<Asset, Location> for ForeignAssetsFromSibling {
  fn contains(asset: &Asset, origin: &Location) -> bool {
    let AssetId(location) = &asset.id;
    location.starts_with(origin)
  }
}

type BaseAssetTransactor = (LocalAssetTransactor, ForeignAssetsTransactor);

pub struct AaaAwareAssetTransactor;

impl AaaAwareAssetTransactor {
  fn to_asset_kind_and_amount(what: &Asset) -> Option<(AssetKind, Balance)> {
    let amount = match what.fun {
      Fungible(value) => value,
      _ => return None,
    };
    let AssetId(location) = &what.id;
    if location == &RelayLocation::get() {
      return Some((AssetKind::Native, amount));
    }
    let foreign_id = AssetRegistry::location_to_asset(location.clone())?;
    Some((AssetKind::Foreign(foreign_id), amount))
  }

  fn preflight_ingress(
    what: &Asset,
    who: &Location,
    context: Option<&XcmContext>,
  ) -> Result<(), XcmError> {
    let Some(recipient) =
      <LocationToAccountId as ConvertLocation<AccountId>>::convert_location(who)
    else {
      return Ok(());
    };
    let Some((asset, amount)) = Self::to_asset_kind_and_amount(what) else {
      return Ok(());
    };
    let Some(source) = context
      .and_then(|ctx| ctx.origin.as_ref())
      .and_then(|origin| {
        <LocationToAccountId as ConvertLocation<AccountId>>::convert_location(origin)
      })
    else {
      return Ok(());
    };
    <RuntimeAddressEventIngress as AddressEventIngress>::preflight_xcm_inbound(
      &recipient, asset, amount, &source,
    )
    .map_err(|_| XcmError::FailedToTransactAsset("AAA funding batch preflight failed"))
  }

  fn notify_ingress(
    what: &Asset,
    who: &Location,
    context: Option<&XcmContext>,
  ) -> Result<(), XcmError> {
    let Some(recipient) =
      <LocationToAccountId as ConvertLocation<AccountId>>::convert_location(who)
    else {
      return Ok(());
    };
    let Some((asset, amount)) = Self::to_asset_kind_and_amount(what) else {
      return Ok(());
    };
    let source = context
      .and_then(|ctx| ctx.origin.as_ref())
      .and_then(|origin| {
        <LocationToAccountId as ConvertLocation<AccountId>>::convert_location(origin)
      });
    if let Some(source) = source {
      return <RuntimeAddressEventIngress as AddressEventIngress>::on_xcm_inbound(
        &recipient, asset, amount, &source,
      )
      .map_err(|_| XcmError::FailedToTransactAsset("AAA funding notification failed"));
    }
    <RuntimeAddressEventIngress as AddressEventIngress>::on_inbound_without_source(
      &recipient, asset, amount,
    )
    .map_err(|_| XcmError::FailedToTransactAsset("AAA ingress notification failed"))
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[derive(Clone)]
struct BenchmarkCredit(Balance);

#[cfg(feature = "runtime-benchmarks")]
impl UnsafeConstructorDestructor<Balance> for BenchmarkCredit {
  fn unsafe_clone(&self) -> Box<dyn ImbalanceAccounting<Balance>> {
    Box::new(Self(self.0))
  }

  fn forget_imbalance(&mut self) -> Balance {
    core::mem::take(&mut self.0)
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl UnsafeManualAccounting<Balance> for BenchmarkCredit {
  fn saturating_subsume(&mut self, mut other: Box<dyn ImbalanceAccounting<Balance>>) {
    self.0 = self.0.saturating_add(other.amount());
    let _ = other.forget_imbalance();
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl ImbalanceAccounting<Balance> for BenchmarkCredit {
  fn amount(&self) -> Balance {
    self.0
  }

  fn saturating_take(&mut self, amount: Balance) -> Box<dyn ImbalanceAccounting<Balance>> {
    let taken = self.0.min(amount);
    self.0 = self.0.saturating_sub(taken);
    Box::new(Self(taken))
  }
}

#[cfg(feature = "runtime-benchmarks")]
fn benchmark_foreign_location() -> Location {
  Location::new(1, [Junction::Parachain(4_242)])
}

#[cfg(feature = "runtime-benchmarks")]
pub fn setup_benchmark_foreign_asset() -> polkadot_sdk::sp_runtime::DispatchResult {
  AssetRegistry::register_foreign_asset(
    RuntimeOrigin::root(),
    benchmark_foreign_location(),
    primitives::assets::CurrencyMetadata {
      name: b"Benchmark Foreign".to_vec(),
      symbol: b"BFRN".to_vec(),
      decimals: 12,
    },
    1,
    true,
  )
}

#[cfg(feature = "runtime-benchmarks")]
pub fn benchmark_foreign_asset_deposit(
  recipient: &AccountId,
  source: &AccountId,
  amount: Balance,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  let account_location = |account: &AccountId| {
    let mut id = [0u8; 32];
    id.copy_from_slice(account.as_ref());
    Location::new(0, [Junction::AccountId32 { network: None, id }])
  };
  let mut holding = AssetsInHolding::new();
  holding.fungible.insert(
    AssetId(benchmark_foreign_location()),
    Box::new(BenchmarkCredit(amount)),
  );
  let context = XcmContext {
    origin: Some(account_location(source)),
    message_id: [0xAA; 32],
    topic: None,
  };
  <AaaAwareAssetTransactor as TransactAsset>::deposit_asset(
    holding,
    &account_location(recipient),
    Some(&context),
  )
  .map_err(|_| polkadot_sdk::sp_runtime::DispatchError::Other("XcmBenchmarkDepositFailed"))
}

impl TransactAsset for AaaAwareAssetTransactor {
  fn can_check_in(origin: &Location, what: &Asset, context: &XcmContext) -> XcmResult {
    BaseAssetTransactor::can_check_in(origin, what, context)
  }

  fn check_in(origin: &Location, what: &Asset, context: &XcmContext) {
    BaseAssetTransactor::check_in(origin, what, context)
  }

  fn can_check_out(dest: &Location, what: &Asset, context: &XcmContext) -> XcmResult {
    BaseAssetTransactor::can_check_out(dest, what, context)
  }

  fn check_out(dest: &Location, what: &Asset, context: &XcmContext) {
    BaseAssetTransactor::check_out(dest, what, context)
  }

  fn deposit_asset(
    what: AssetsInHolding,
    who: &Location,
    context: Option<&XcmContext>,
  ) -> Result<(), (AssetsInHolding, XcmError)> {
    let notified_assets = what.assets_iter().collect::<Vec<_>>();
    for asset in &notified_assets {
      if let Err(error) = Self::preflight_ingress(asset, who, context) {
        return Err((what, error));
      }
    }
    // One non-recursive layer preserves the non-cloneable holding on notification failure.
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      for asset in &notified_assets {
        if let Err(error) = Self::notify_ingress(asset, who, context) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err((
            what, error,
          )));
        }
      }
      match BaseAssetTransactor::deposit_asset(what, who, context) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn deposit_asset_with_surplus(
    what: AssetsInHolding,
    who: &Location,
    context: Option<&XcmContext>,
  ) -> Result<Weight, (AssetsInHolding, XcmError)> {
    let notified_assets = what.assets_iter().collect::<Vec<_>>();
    for asset in &notified_assets {
      if let Err(error) = Self::preflight_ingress(asset, who, context) {
        return Err((what, error));
      }
    }
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      for asset in &notified_assets {
        if let Err(error) = Self::notify_ingress(asset, who, context) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err((
            what, error,
          )));
        }
      }
      match BaseAssetTransactor::deposit_asset_with_surplus(what, who, context) {
        Ok(surplus) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(surplus))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn withdraw_asset(
    what: &Asset,
    who: &Location,
    context: Option<&XcmContext>,
  ) -> Result<AssetsInHolding, XcmError> {
    BaseAssetTransactor::withdraw_asset(what, who, context)
  }

  fn withdraw_asset_with_surplus(
    what: &Asset,
    who: &Location,
    context: Option<&XcmContext>,
  ) -> Result<(AssetsInHolding, Weight), XcmError> {
    BaseAssetTransactor::withdraw_asset_with_surplus(what, who, context)
  }

  fn internal_transfer_asset(
    asset: &Asset,
    from: &Location,
    to: &Location,
    context: &XcmContext,
  ) -> Result<Asset, XcmError> {
    BaseAssetTransactor::internal_transfer_asset(asset, from, to, context)
  }

  fn internal_transfer_asset_with_surplus(
    asset: &Asset,
    from: &Location,
    to: &Location,
    context: &XcmContext,
  ) -> Result<(Asset, Weight), XcmError> {
    BaseAssetTransactor::internal_transfer_asset_with_surplus(asset, from, to, context)
  }
}

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
  // Sovereign account converter; this attempts to derive an `AccountId` from the origin location
  // using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
  // foreign chains who want to have a local sovereign account on this chain which they control.
  SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
  // Native converter for Relay-chain (Parent) location; will convert to a `Relay` origin when
  // recognized.
  RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
  // Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
  // recognized.
  SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
  // Native signed account converter; this just converts an `AccountId32` origin into a normal
  // `RuntimeOrigin::Signed` origin of the same 32-byte value.
  SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
  // Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
  XcmPassthrough<RuntimeOrigin>,
);

pub struct ParentOrParentsExecutivePlurality;
impl Contains<Location> for ParentOrParentsExecutivePlurality {
  fn contains(location: &Location) -> bool {
    matches!(
      location.unpack(),
      (1, [])
        | (
          1,
          [Plurality {
            id: BodyId::Executive,
            ..
          }]
        )
    )
  }
}

/// Trust filter for reserve asset transfers.
/// Allows receiving reserve assets from:
/// - Parent (Relay Chain)
/// - Sibling Parachains
pub struct ReserveAssetsFrom;
impl Contains<(Location, Vec<Asset>)> for ReserveAssetsFrom {
  fn contains((location, _assets): &(Location, Vec<Asset>)) -> bool {
    matches!(
      location.unpack(),
      // Parent (Relay Chain)
      (1, []) |
      // Sibling Parachains
      (1, [Parachain(_)])
    )
  }
}

pub type Barrier = TrailingSetTopicAsId<
  DenyThenTry<
    DenyRecursively<DenyReserveTransferToRelayChain>,
    (
      TakeWeightCredit,
      WithComputedOrigin<
        (
          AllowTopLevelPaidExecutionFrom<Everything>,
          AllowExplicitUnpaidExecutionFrom<ParentOrParentsExecutivePlurality>,
          // ^^^ Parent and its exec plurality get free execution
        ),
        UniversalLocation,
        ConstU32<8>,
      >,
    ),
  >,
>;

/// Converts a local signed origin into an XCM location. Forms the basis for local origins
/// sending/executing XCMs.
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = WithUniqueTopic<(
  // Two routers - use UMP to communicate with the relay chain:
  cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, PriceForParentDelivery>,
  // ..and XCMP to communicate with the sibling chains.
  XcmpQueue,
)>;

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
  type Aliasers = Nothing;
  type AssetExchanger = ();
  type AssetLocker = ();
  type AssetTrap = PolkadotXcm;
  type AssetTransactor = AaaAwareAssetTransactor;
  type Barrier = Barrier;
  type CallDispatcher = RuntimeCall;
  type FeeManager = ();
  type HrmpChannelAcceptedHandler = ();
  type HrmpChannelClosingHandler = ();
  type HrmpNewChannelOpenRequestHandler = ();
  type IsReserve = (NativeAsset, ForeignAssetsFromSibling);
  type IsTeleporter = (); // Teleporting is disabled.
  type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
  type MessageExporter = ();
  type OriginConverter = XcmOriginToTransactDispatchOrigin;
  type PalletInstancesInfo = AllPalletsWithSystem;
  type ResponseHandler = PolkadotXcm;
  type RuntimeCall = RuntimeCall;
  type SafeCallFilter = Nothing;
  type SubscriptionService = PolkadotXcm;
  type Trader =
    UsingComponents<WeightToFee, RelayLocation, AccountId, Balances, RuntimeFeeCollector>;
  type TransactionalProcessor = FrameTransactionalProcessor;
  type UniversalAliases = Nothing;
  type UniversalLocation = UniversalLocation;
  type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
  type XcmEventEmitter = PolkadotXcm;
  type XcmRecorder = PolkadotXcm;
  type XcmSender = XcmRouter;
}

impl pallet_xcm::Config for Runtime {
  type AdminOrigin = EnsureRoot<AccountId>;
  // ^ Override for AdvertisedXcmVersion default
  type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
  // Aliasing is disabled: xcm_executor::Config::Aliasers is set to `Nothing`.
  type AuthorizedAliasConsideration = Disabled;
  type Currency = Balances;
  type CurrencyMatcher = ();
  type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
  type MaxLockers = ConstU32<8>;
  type MaxRemoteLockConsumers = ConstU32<0>;
  type RemoteLockConsumerIdentifier = ();
  type RuntimeCall = RuntimeCall;
  type RuntimeEvent = RuntimeEvent;
  type RuntimeOrigin = RuntimeOrigin;
  type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
  type SovereignAccountOf = LocationToAccountId;
  type TrustedLockers = ();
  type UniversalLocation = UniversalLocation;
  type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
  type WeightInfo = crate::weights::pallet_xcm::SubstrateWeight<Runtime>;
  // ^ Disable dispatchable execute on the XCM pallet.
  // Needs to be `Everything` for local testing.
  type XcmExecuteFilter = Nothing;
  type XcmExecutor = XcmExecutor<XcmConfig>;
  type XcmReserveTransferFilter = ReserveAssetsFrom;
  type XcmRouter = XcmRouter;
  type XcmTeleportFilter = Everything;

  const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

impl cumulus_pallet_xcm::Config for Runtime {
  type RuntimeEvent = RuntimeEvent;
  type XcmExecutor = XcmExecutor<XcmConfig>;
}

#[cfg(test)]
mod tests {
  use super::*;
  use codec::Encode;
  use polkadot_sdk::{
    frame_support::{
      assert_noop,
      traits::{Contains, OriginTrait},
    },
    sp_runtime::DispatchError,
    staging_xcm_executor::traits::{ConvertOrigin, Properties, ShouldExecute},
  };
  use primitives::assets::{AssetInspector, TYPE_FOREIGN};

  #[test]
  fn xcm_fixed_instruction_weight_binds_single_asset_aaa_deposit_benchmark() {
    assert_eq!(MaxAssetsIntoHolding::get(), 1);
    assert_eq!(
      UnitWeightCost::get(),
      <crate::weights::pallet_aaa::SubstrateWeight<Runtime> as pallet_aaa::WeightInfo>::xcm_asset_deposit()
    );
    assert!(UnitWeightCost::get().proof_size() > 0);
  }

  #[test]
  fn test_location_to_asset_id_relay_token() {
    use polkadot_sdk::sp_runtime::traits::Convert;
    // Relay chain native token (parent)
    let relay_location = Location::parent();

    let asset_id = LocationToAssetId::convert(relay_location);
    // Should return ID directly
    let asset_kind = primitives::AssetKind::Local(asset_id);
    assert!(
      asset_kind.is_foreign(),
      "Relay token should be in Foreign namespace (0xF...)"
    );
  }

  #[test]
  fn test_location_to_asset_id_sibling_parachain() {
    use polkadot_sdk::sp_runtime::traits::Convert;
    // Sibling parachain asset (e.g., from parachain 1000)
    let sibling_location = Location::new(1, [Parachain(1000)]);

    let asset_id = LocationToAssetId::convert(sibling_location);

    let asset_kind = primitives::AssetKind::Local(asset_id);
    assert!(
      asset_kind.is_foreign(),
      "Sibling token should be in Foreign namespace (0xF...)"
    );
  }

  #[test]
  fn test_location_to_asset_id_deterministic() {
    use polkadot_sdk::sp_runtime::traits::Convert;
    // Same location should always produce same asset ID
    let location = Location::new(1, [Parachain(2000)]);

    let id1 = LocationToAssetId::convert(location.clone());
    let id2 = LocationToAssetId::convert(location);

    assert_eq!(id1, id2, "Same location must produce same asset ID");
  }

  #[test]
  fn test_location_to_asset_id_different_parachains() {
    use polkadot_sdk::sp_runtime::traits::Convert;
    // Different parachains should produce different asset IDs
    let para_1000 = Location::new(1, [Parachain(1000)]);
    let para_2000 = Location::new(1, [Parachain(2000)]);

    let id1 = LocationToAssetId::convert(para_1000);
    let id2 = LocationToAssetId::convert(para_2000);

    assert_ne!(
      id1, id2,
      "Different parachains must produce different asset IDs"
    );

    // Both should be in Foreign namespace
    assert_eq!(id1 & primitives::assets::MASK_TYPE, TYPE_FOREIGN);
    assert_eq!(id2 & primitives::assets::MASK_TYPE, TYPE_FOREIGN);
  }

  #[test]
  fn test_location_to_asset_id_complex_location() {
    use polkadot_sdk::sp_runtime::traits::Convert;
    // Asset from sibling parachain with additional context
    let complex_location =
      Location::new(1, [Parachain(1000), PalletInstance(50), GeneralIndex(42)]);

    let asset_id = LocationToAssetId::convert(complex_location);

    assert_eq!(asset_id & primitives::assets::MASK_TYPE, TYPE_FOREIGN);
  }

  #[test]
  fn test_reserve_assets_from_relay() {
    // Relay chain should be trusted for reserve transfers
    let relay_location = Location::parent();
    let assets = vec![];
    assert!(
      ReserveAssetsFrom::contains(&(relay_location, assets)),
      "Relay chain should be trusted"
    );
  }

  #[test]
  fn test_reserve_assets_from_sibling() {
    // Sibling parachains should be trusted
    let sibling_location = Location::new(1, [Parachain(1000)]);
    let assets = vec![];
    assert!(
      ReserveAssetsFrom::contains(&(sibling_location, assets)),
      "Sibling should be trusted"
    );
  }

  #[test]
  fn test_reserve_assets_from_untrusted() {
    // Random locations should not be trusted
    let untrusted_location =
      Location::new(2, [GlobalConsensus(NetworkId::Ethereum { chain_id: 1 })]);
    let assets = vec![];
    assert!(
      !ReserveAssetsFrom::contains(&(untrusted_location, assets)),
      "External network should not be trusted"
    );
  }

  #[test]
  fn test_foreign_assets_from_sibling_filter() {
    // Asset from sibling parachain
    let sibling_origin = Location::new(1, [Parachain(1000)]);
    let asset_location = Location::new(1, [Parachain(1000), PalletInstance(50)]);
    let asset = Asset {
      id: AssetId(asset_location),
      fun: Fungibility::Fungible(1000),
    };

    assert!(
      ForeignAssetsFromSibling::contains(&asset, &sibling_origin),
      "Asset from sibling should be accepted"
    );
  }

  #[test]
  fn test_foreign_assets_from_sibling_rejects_mismatch() {
    // Asset claims to be from para 1000 but origin is para 2000
    let origin = Location::new(1, [Parachain(2000)]);
    let asset_location = Location::new(1, [Parachain(1000)]);
    let asset = Asset {
      id: AssetId(asset_location),
      fun: Fungibility::Fungible(1000),
    };

    assert!(
      !ForeignAssetsFromSibling::contains(&asset, &origin),
      "Asset from different parachain should be rejected"
    );
  }

  #[test]
  fn test_xcm_origin_converter_maps_parent_to_relay_origin() {
    let converted =
      <XcmOriginToTransactDispatchOrigin as ConvertOrigin<RuntimeOrigin>>::convert_origin(
        Location::parent(),
        xcm::latest::OriginKind::Native,
      )
      .expect("parent origin should convert");
    assert_eq!(
      converted.into_caller().encode(),
      RuntimeOrigin::from(cumulus_pallet_xcm::Origin::Relay)
        .into_caller()
        .encode()
    );
  }

  #[test]
  fn test_xcm_origin_converter_maps_sibling_to_sibling_origin() {
    let converted =
      <XcmOriginToTransactDispatchOrigin as ConvertOrigin<RuntimeOrigin>>::convert_origin(
        Location::new(1, [Parachain(2000)]),
        xcm::latest::OriginKind::Native,
      )
      .expect("sibling origin should convert");
    assert_eq!(
      converted.into_caller().encode(),
      RuntimeOrigin::from(cumulus_pallet_xcm::Origin::SiblingParachain(2000u32.into()))
        .into_caller()
        .encode()
    );
  }

  #[test]
  fn test_xcm_origin_converter_maps_account_id32_to_signed_origin() {
    let who = AccountId::new([7u8; 32]);
    let converted =
      <XcmOriginToTransactDispatchOrigin as ConvertOrigin<RuntimeOrigin>>::convert_origin(
        Location::new(
          0,
          [AccountId32 {
            network: None,
            id: who.clone().into(),
          }],
        ),
        xcm::latest::OriginKind::Native,
      )
      .expect("account id32 origin should convert");
    assert_eq!(
      converted.into_caller().encode(),
      RuntimeOrigin::signed(who).into_caller().encode()
    );
  }

  #[test]
  fn test_xcm_origin_converter_maps_xcm_passthrough_origin() {
    let location = Location::new(1, [Parachain(3000), PalletInstance(42)]);
    let converted =
      <XcmOriginToTransactDispatchOrigin as ConvertOrigin<RuntimeOrigin>>::convert_origin(
        location.clone(),
        xcm::latest::OriginKind::Xcm,
      )
      .expect("xcm passthrough origin should convert");
    assert_eq!(
      converted.into_caller().encode(),
      RuntimeOrigin::from(pallet_xcm::Origin::Xcm(location))
        .into_caller()
        .encode()
    );
  }

  #[test]
  fn test_safe_call_filter_now_denies_representative_runtime_calls() {
    let user_call = RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() });
    let admin_call =
      RuntimeCall::Staking(pallet_staking::Call::register_staking_asset { asset_id: 1 });
    let queue_control_call =
      RuntimeCall::XcmpQueue(cumulus_pallet_xcmp_queue::Call::suspend_xcm_execution {});
    assert!(
      !<<XcmConfig as xcm_executor::Config>::SafeCallFilter as Contains<RuntimeCall>>::contains(
        &user_call,
      )
    );
    assert!(
      !<<XcmConfig as xcm_executor::Config>::SafeCallFilter as Contains<RuntimeCall>>::contains(
        &admin_call,
      )
    );
    assert!(
      !<<XcmConfig as xcm_executor::Config>::SafeCallFilter as Contains<RuntimeCall>>::contains(
        &queue_control_call,
      )
    );
  }

  #[test]
  fn test_barrier_allows_paid_execution_from_sibling() {
    let sibling_origin = Location::new(1, [Parachain(2000)]);
    let max_weight = Weight::from_parts(10, 10);
    let mut message = Xcm::<RuntimeCall>(vec![
      WithdrawAsset((Parent, 100).into()),
      BuyExecution {
        fees: (Parent, 100).into(),
        weight_limit: Limited(max_weight),
      },
    ]);
    let mut properties = Properties {
      weight_credit: Weight::zero(),
      message_id: None,
    };
    assert!(
      Barrier::should_execute(
        &sibling_origin,
        message.inner_mut(),
        max_weight,
        &mut properties,
      )
      .is_ok()
    );
  }

  #[test]
  fn test_barrier_allows_explicit_unpaid_execution_from_parent() {
    let max_weight = Weight::from_parts(10, 10);
    let mut message = Xcm::<RuntimeCall>(vec![
      UnpaidExecution {
        weight_limit: Limited(max_weight),
        check_origin: Some(Location::parent()),
      },
      ClearTopic,
    ]);
    let mut properties = Properties {
      weight_credit: Weight::zero(),
      message_id: None,
    };
    assert!(
      Barrier::should_execute(
        &Location::parent(),
        message.inner_mut(),
        max_weight,
        &mut properties,
      )
      .is_ok()
    );
  }

  #[test]
  fn test_barrier_rejects_explicit_unpaid_execution_from_sibling() {
    let sibling_origin = Location::new(1, [Parachain(2000)]);
    let max_weight = Weight::from_parts(10, 10);
    let mut message = Xcm::<RuntimeCall>(vec![
      UnpaidExecution {
        weight_limit: Limited(max_weight),
        check_origin: Some(sibling_origin.clone()),
      },
      ClearTopic,
    ]);
    let mut properties = Properties {
      weight_credit: Weight::zero(),
      message_id: None,
    };
    assert!(
      Barrier::should_execute(
        &sibling_origin,
        message.inner_mut(),
        max_weight,
        &mut properties,
      )
      .is_err()
    );
  }

  #[test]
  fn test_barrier_allows_explicit_unpaid_execution_from_parent_executive_plurality() {
    let executive_origin = Location::new(
      1,
      [Plurality {
        id: BodyId::Executive,
        part: BodyPart::Voice,
      }],
    );
    let max_weight = Weight::from_parts(10, 10);
    let mut message = Xcm::<RuntimeCall>(vec![
      UnpaidExecution {
        weight_limit: Limited(max_weight),
        check_origin: Some(executive_origin.clone()),
      },
      ClearTopic,
    ]);
    let mut properties = Properties {
      weight_credit: Weight::zero(),
      message_id: None,
    };
    assert!(
      Barrier::should_execute(
        &executive_origin,
        message.inner_mut(),
        max_weight,
        &mut properties,
      )
      .is_ok()
    );
  }

  #[test]
  fn test_xcmp_queue_controller_rejects_relay_origin_without_root() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      assert_noop!(
        cumulus_pallet_xcmp_queue::Pallet::<Runtime>::suspend_xcm_execution(RuntimeOrigin::from(
          cumulus_pallet_xcm::Origin::Relay
        ),),
        DispatchError::BadOrigin
      );
    });
  }

  #[test]
  fn test_xcmp_queue_controller_rejects_sibling_origin_without_root() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      assert_noop!(
        cumulus_pallet_xcmp_queue::Pallet::<Runtime>::suspend_xcm_execution(RuntimeOrigin::from(
          cumulus_pallet_xcm::Origin::SiblingParachain(2000u32.into())
        ),),
        DispatchError::BadOrigin
      );
    });
  }

  #[test]
  fn test_xcmp_queue_controller_rejects_signed_origin_without_root() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      assert_noop!(
        cumulus_pallet_xcmp_queue::Pallet::<Runtime>::suspend_xcm_execution(RuntimeOrigin::signed(
          AccountId::new([9u8; 32])
        ),),
        DispatchError::BadOrigin
      );
    });
  }

  #[test]
  fn test_xcmp_queue_controller_allows_root_origin() {
    crate::tests::common::seeded_test_ext().execute_with(|| {
      assert!(
        cumulus_pallet_xcmp_queue::Pallet::<Runtime>::suspend_xcm_execution(RuntimeOrigin::root(),)
          .is_ok()
      );
      assert!(
        cumulus_pallet_xcmp_queue::Pallet::<Runtime>::resume_xcm_execution(RuntimeOrigin::root(),)
          .is_ok()
      );
    });
  }
}
