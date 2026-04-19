//! Elegant Chain Specification Hierarchy
//!
//! Provides a unified, type-safe approach to chain configuration with
//! clear separation between development, testnet, and production environments.

#![allow(dead_code)]

use crate::{
  AccountId, AuraId, Balance, BlockNumber, EXISTENTIAL_DEPOSIT, PARACHAIN_ID, ParachainInfoConfig,
  RuntimeGenesisConfig, UNIT,
  configs::{AssetKind, genesis_protocol_asset_metadata, genesis_protocol_assets},
};
use alloc::{vec, vec::Vec};
use cumulus_primitives_core::ParaId;
use frame_support::{build_struct_json_patch, traits::Get};
use polkadot_sdk::{staging_xcm as xcm, *};
use serde_json::Value;
use sp_genesis_builder::PresetId;
use sp_keyring::Sr25519Keyring;

/// Unified chain specification builder
pub struct ChainSpecBuilder {
  /// Chain identifier
  pub chain_id: ParaId,
  /// Initial validators/collators
  pub validators: Vec<(AccountId, AuraId)>,
  /// Endowed accounts with initial balances
  pub endowed_accounts: Vec<AccountId>,
  /// Bootstrap asset owner used by the current launch-line local/runtime presets
  pub bootstrap_asset_owner: AccountId,
  /// Economic parameters
  pub economic_params: EconomicParams,
  /// Network parameters
  pub network_params: NetworkParams,
}

/// Economic parameters for chain configuration
#[derive(Debug, Clone)]
pub struct EconomicParams {
  /// Collator candidacy bond
  pub candidacy_bond: Balance,
  /// Initial endowment for accounts
  pub initial_endowment: Balance,
  /// XCM version for cross-chain compatibility
  pub safe_xcm_version: u32,
}

/// Network parameters for chain configuration
#[derive(Debug, Clone)]
pub struct NetworkParams {
  /// Session length in blocks
  pub session_length: BlockNumber,
}

impl Default for EconomicParams {
  fn default() -> Self {
    Self {
      candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
      initial_endowment: 1u128 << 60, // 1 << 60 units
      safe_xcm_version: xcm::prelude::XCM_VERSION,
    }
  }
}

impl Default for NetworkParams {
  fn default() -> Self {
    Self {
      session_length: 6 * 60 * 24, // 1 day in 6s blocks
    }
  }
}

const LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID: u32 = 0xF000_0001;
const LOCAL_WEB_CLIENT_INITIAL_PRICE: u128 = 1_000_000_000_000;
const LOCAL_WEB_CLIENT_SLOPE: u128 = 1_000_000;
const LOCAL_WEB_CLIENT_FOREIGN_BALANCE: u128 = 1u128 << 60;

fn local_web_client_assets(owner: &AccountId) -> Vec<(u32, AccountId, bool, Balance)> {
  let mut assets = genesis_protocol_assets();
  assets.push((LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID, owner.clone(), true, 1));
  assets
}

fn local_web_client_asset_metadata() -> Vec<(u32, Vec<u8>, Vec<u8>, u8)> {
  let mut metadata = genesis_protocol_asset_metadata();
  metadata.push((
    LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID,
    b"Foreign Token".to_vec(),
    b"FRGN".to_vec(),
    12,
  ));
  metadata
}

fn local_web_client_asset_accounts(owner: &AccountId) -> Vec<(u32, AccountId, Balance)> {
  vec![(
    LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID,
    owner.clone(),
    LOCAL_WEB_CLIENT_FOREIGN_BALANCE,
  )]
}

fn local_web_client_tracked_assets() -> Vec<AssetKind> {
  vec![
    AssetKind::Native,
    AssetKind::Foreign(LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID),
  ]
}

fn local_web_client_curves() -> Vec<(AssetKind, AssetKind, Balance, Balance)> {
  vec![(
    AssetKind::Native,
    AssetKind::Foreign(LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID),
    LOCAL_WEB_CLIENT_INITIAL_PRICE,
    LOCAL_WEB_CLIENT_SLOPE,
  )]
}

impl ChainSpecBuilder {
  /// Create a new chain specification builder
  pub fn new(chain_id: ParaId) -> Self {
    Self {
      chain_id,
      validators: Vec::new(),
      endowed_accounts: Vec::new(),
      bootstrap_asset_owner: Sr25519Keyring::Alice.to_account_id(),
      economic_params: EconomicParams::default(),
      network_params: NetworkParams::default(),
    }
  }

  /// Set the bootstrap asset owner used by local seeded foreign assets
  pub fn with_bootstrap_asset_owner(mut self, account: AccountId) -> Self {
    self.bootstrap_asset_owner = account;
    self
  }

  /// Configure economic parameters
  pub fn with_economic_params(mut self, params: EconomicParams) -> Self {
    self.economic_params = params;
    self
  }

  /// Configure network parameters
  pub fn with_network_params(mut self, params: NetworkParams) -> Self {
    self.network_params = params;
    self
  }

  /// Build the genesis configuration as JSON patch
  pub fn build_genesis_patch(&self) -> Value {
    let mut endowed_accounts = self.endowed_accounts.clone();
    let aaa_fee_sink = <crate::Runtime as pallet_aaa::Config>::FeeSink::get();
    if !endowed_accounts.contains(&aaa_fee_sink) {
      endowed_accounts.push(aaa_fee_sink);
    }

    let mut patch = build_struct_json_patch!(RuntimeGenesisConfig {
      balances: pallet_balances::GenesisConfig {
        balances: endowed_accounts
          .iter()
          .cloned()
          .map(|account| (account, self.economic_params.initial_endowment))
          .collect::<Vec<_>>(),
      },
      assets: pallet_assets::GenesisConfig {
        assets: local_web_client_assets(&self.bootstrap_asset_owner),
        metadata: local_web_client_asset_metadata(),
        accounts: local_web_client_asset_accounts(&self.bootstrap_asset_owner),
        next_asset_id: None,
        reserves: Vec::new(),
      },
      parachain_info: ParachainInfoConfig {
        parachain_id: self.chain_id
      },
      collator_selection: pallet_collator_selection::GenesisConfig {
        invulnerables: self
          .validators
          .iter()
          .cloned()
          .map(|(account, _)| account)
          .collect::<Vec<_>>(),
        candidacy_bond: self.economic_params.candidacy_bond,
        desired_candidates: 0,
      },
      session: pallet_session::GenesisConfig {
        keys: self
          .validators
          .iter()
          .cloned()
          .map(|(account, aura_key)| {
            (
              account.clone(),
              account,
              crate::template_session_keys(aura_key),
            )
          })
          .collect::<Vec<_>>(),
      },
      polkadot_xcm: pallet_xcm::GenesisConfig {
        safe_xcm_version: Some(self.economic_params.safe_xcm_version)
      },
      axial_router: pallet_axial_router::GenesisConfig {
        tracked_assets: local_web_client_tracked_assets(),
        _marker: Default::default(),
      },
      token_minting_curve: pallet_tmc::GenesisConfig {
        curves: local_web_client_curves(),
        _marker: Default::default(),
      },
    });
    patch["collatorSelection"]["desiredCandidates"] = Value::from(0);
    patch
  }

  /// Build and serialize the genesis configuration
  pub fn build(&self) -> Vec<u8> {
    let patch = self.build_genesis_patch();
    serde_json::to_string(&patch)
      .expect("JSON serialization should never fail")
      .into_bytes()
  }
}

/// Development configuration with well-known accounts
pub fn development_config() -> ChainSpecBuilder {
  let validators = vec![
    (
      Sr25519Keyring::Alice.to_account_id(),
      Sr25519Keyring::Alice.public().into(),
    ),
    (
      Sr25519Keyring::Bob.to_account_id(),
      Sr25519Keyring::Bob.public().into(),
    ),
  ];

  let endowed_accounts = Sr25519Keyring::well_known()
    .map(|k| k.to_account_id())
    .collect();

  ChainSpecBuilder::new(PARACHAIN_ID.into())
    .with_bootstrap_asset_owner(Sr25519Keyring::Alice.to_account_id())
    .with_validators(validators)
    .with_endowed_accounts(endowed_accounts)
}

/// Testnet configuration with enhanced security
pub fn testnet_config() -> ChainSpecBuilder {
  let validators = vec![
    (
      Sr25519Keyring::Alice.to_account_id(),
      Sr25519Keyring::Alice.public().into(),
    ),
    (
      Sr25519Keyring::Bob.to_account_id(),
      Sr25519Keyring::Bob.public().into(),
    ),
    (
      Sr25519Keyring::Charlie.to_account_id(),
      Sr25519Keyring::Charlie.public().into(),
    ),
  ];

  let endowed_accounts = vec![
    Sr25519Keyring::Alice.to_account_id(),
    Sr25519Keyring::Bob.to_account_id(),
    Sr25519Keyring::Charlie.to_account_id(),
    Sr25519Keyring::Dave.to_account_id(),
    Sr25519Keyring::Eve.to_account_id(),
    Sr25519Keyring::Ferdie.to_account_id(),
  ];

  let economic_params = EconomicParams {
    candidacy_bond: EXISTENTIAL_DEPOSIT * 32, // Higher bond for testnet
    initial_endowment: 1u128 << 50,           // Smaller endowments
    ..EconomicParams::default()
  };

  ChainSpecBuilder::new(PARACHAIN_ID.into())
    .with_bootstrap_asset_owner(Sr25519Keyring::Alice.to_account_id())
    .with_validators(validators)
    .with_endowed_accounts(endowed_accounts)
    .with_economic_params(economic_params)
}

/// Production configuration with minimal privileges
pub fn production_config(
  validators: Vec<(AccountId, AuraId)>,
  bootstrap_asset_owner: AccountId,
) -> ChainSpecBuilder {
  let economic_params = EconomicParams {
    candidacy_bond: 10_000 * UNIT,  // Significant bond for production
    initial_endowment: 1000 * UNIT, // Modest initial endowments
    ..EconomicParams::default()
  };

  let network_params = NetworkParams {
    session_length: 6 * 60 * 24 * 7, // 1 week sessions
  };

  ChainSpecBuilder::new(PARACHAIN_ID.into())
    .with_bootstrap_asset_owner(bootstrap_asset_owner)
    .with_validators(validators)
    .with_economic_params(economic_params)
    .with_network_params(network_params)
}

// Extension methods for builder pattern elegance
pub trait ChainSpecBuilderExt {
  /// Add multiple validators at once
  fn with_validators(self, validators: Vec<(AccountId, AuraId)>) -> Self;

  /// Add multiple endowed accounts at once
  fn with_endowed_accounts(self, accounts: Vec<AccountId>) -> Self;
}

impl ChainSpecBuilderExt for ChainSpecBuilder {
  fn with_validators(mut self, validators: Vec<(AccountId, AuraId)>) -> Self {
    self.validators = validators;
    self
  }

  fn with_endowed_accounts(mut self, accounts: Vec<AccountId>) -> Self {
    self.endowed_accounts = accounts;
    self
  }
}

/// Provides the JSON representation of predefined genesis config for given preset ID
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
  let builder = match id.as_ref() {
    sp_genesis_builder::DEV_RUNTIME_PRESET => development_config(),
    sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => testnet_config(),
    _ => return None,
  };

  Some(builder.build())
}

/// List of supported preset names
pub fn preset_names() -> Vec<PresetId> {
  vec![
    PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
    PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
  ]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_development_config_builds() {
    let config = development_config();
    let genesis_bytes = config.build();

    assert!(!genesis_bytes.is_empty());
    let genesis_json: serde_json::Value =
      serde_json::from_slice(&genesis_bytes).expect("genesis json must parse");
    assert!(genesis_json.get("balances").is_some());
    assert!(genesis_json.get("sudo").is_none());
    assert!(genesis_json.get("assets").is_some());
    assert!(genesis_json.get("axialRouter").is_some());
    assert!(genesis_json.get("tokenMintingCurve").is_some());
    assert_eq!(
      genesis_json["collatorSelection"]["desiredCandidates"],
      serde_json::Value::from(0),
    );
  }

  #[test]
  fn development_config_includes_well_known_veto_asset_metadata() {
    let config = development_config();
    let genesis_bytes = config.build();
    let genesis_json: serde_json::Value =
      serde_json::from_slice(&genesis_bytes).expect("genesis json must parse");
    let veto_asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_eq!(
      genesis_json["assets"]["assets"][0][0],
      serde_json::Value::from(veto_asset_id)
    );
    assert_eq!(
      genesis_json["assets"]["metadata"][0][0],
      serde_json::Value::from(veto_asset_id)
    );
    assert_eq!(
      genesis_json["assets"]["metadata"][0][1],
      serde_json::json!(b"Veto Governance Token".to_vec())
    );
    assert_eq!(
      genesis_json["assets"]["metadata"][0][2],
      serde_json::json!(b"VETO".to_vec())
    );
  }

  #[test]
  fn test_production_config_customization() {
    let validators = vec![(
      Sr25519Keyring::Alice.to_account_id(),
      Sr25519Keyring::Alice.public().into(),
    )];

    let config = production_config(validators, Sr25519Keyring::Alice.to_account_id());
    assert_eq!(config.economic_params.candidacy_bond, 10_000 * UNIT);
  }
}
