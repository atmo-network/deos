use super::*;

use polkadot_sdk::frame_support::{
  parameter_types,
  traits::{LinearStoragePrice, tokens::fungible::HoldConsideration},
};
use polkadot_sdk::frame_system::EnsureRoot;

parameter_types! {
  pub const PreimageBaseDeposit: Balance = EXISTENTIAL_DEPOSIT;
  pub const PreimageByteDeposit: Balance = 10 * MICRO_UNIT;
  pub const PreimageHoldReason: RuntimeHoldReason =
    RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
  type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
  type RuntimeEvent = RuntimeEvent;
  type Currency = Balances;
  type ManagerOrigin = EnsureRoot<AccountId>;
  type Consideration = HoldConsideration<
    AccountId,
    Balances,
    PreimageHoldReason,
    LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
  >;
}
