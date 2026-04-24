use crate::{
  AccountId, BalancesConfig, CollatorSelectionConfig, EXISTENTIAL_DEPOSIT, ParachainInfoConfig,
  PolkadotXcmConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys,
  configs::{AssetKind, genesis_protocol_asset_metadata, genesis_protocol_assets},
};

use alloc::{vec, vec::Vec};

use polkadot_sdk::{staging_xcm as xcm, *};

use cumulus_primitives_core::ParaId;
use frame_support::build_struct_json_patch;
use parachains_common::AuraId;
use serde_json::Value;
use sp_genesis_builder::PresetId;
use sp_keyring::Sr25519Keyring;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;
/// Parachain id used for genesis config presets of parachain template.
#[docify::export_content]
pub const PARACHAIN_ID: u32 = 2000;

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn template_session_keys(keys: AuraId) -> SessionKeys {
  SessionKeys { aura: keys }
}

const LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID: u32 = 0;
const LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID: u32 = 0xF000_0001;
const LOCAL_WEB_CLIENT_INITIAL_PRICE: u128 = 1_000_000_000_000;
const LOCAL_WEB_CLIENT_SLOPE: u128 = 1_000_000;
const LOCAL_WEB_CLIENT_FOREIGN_BALANCE: u128 = 1u128 << 60;

fn local_web_client_assets(owner: &AccountId) -> Vec<(u32, AccountId, bool, u128)> {
  let mut assets = genesis_protocol_assets();
  assets.push((
    LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID,
    owner.clone(),
    true,
    1,
  ));
  assets.push((LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID, owner.clone(), true, 1));
  assets
}

fn local_web_client_asset_metadata() -> Vec<(u32, Vec<u8>, Vec<u8>, u8)> {
  let mut metadata = genesis_protocol_asset_metadata();
  metadata.push((
    LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID,
    b"Native Staking Token".to_vec(),
    b"NTVE".to_vec(),
    12,
  ));
  metadata.push((
    LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID,
    b"Foreign Token".to_vec(),
    b"FRGN".to_vec(),
    12,
  ));
  metadata
}

fn local_web_client_asset_accounts(owner: &AccountId) -> Vec<(u32, AccountId, u128)> {
  vec![
    (
      LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID,
      owner.clone(),
      LOCAL_WEB_CLIENT_FOREIGN_BALANCE,
    ),
    (
      LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID,
      owner.clone(),
      LOCAL_WEB_CLIENT_FOREIGN_BALANCE,
    ),
  ]
}

fn local_web_client_tracked_assets() -> Vec<AssetKind> {
  vec![
    AssetKind::Native,
    AssetKind::Foreign(LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID),
  ]
}

fn local_web_client_curves() -> Vec<(AssetKind, AssetKind, u128, u128)> {
  vec![(
    AssetKind::Native,
    AssetKind::Foreign(LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID),
    LOCAL_WEB_CLIENT_INITIAL_PRICE,
    LOCAL_WEB_CLIENT_SLOPE,
  )]
}

fn testnet_genesis(
  invulnerables: Vec<(AccountId, AuraId)>,
  endowed_accounts: Vec<AccountId>,
  bootstrap_asset_owner: AccountId,
  id: ParaId,
) -> Value {
  let mut patch = build_struct_json_patch!(RuntimeGenesisConfig {
    balances: BalancesConfig {
      balances: endowed_accounts
        .iter()
        .cloned()
        .map(|k| (k, 1u128 << 60))
        .collect::<Vec<_>>(),
    },
    assets: pallet_assets::GenesisConfig {
      assets: local_web_client_assets(&bootstrap_asset_owner),
      metadata: local_web_client_asset_metadata(),
      accounts: local_web_client_asset_accounts(&bootstrap_asset_owner),
      next_asset_id: None,
      reserves: Vec::new(),
    },
    parachain_info: ParachainInfoConfig { parachain_id: id },
    collator_selection: CollatorSelectionConfig {
      invulnerables: invulnerables
        .iter()
        .cloned()
        .map(|(acc, _)| acc)
        .collect::<Vec<_>>(),
      candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
      desired_candidates: 0,
    },
    session: SessionConfig {
      keys: invulnerables
        .into_iter()
        .map(|(acc, aura)| {
          (
            acc.clone(),                 // account id
            acc,                         // validator id
            template_session_keys(aura), // session keys
          )
        })
        .collect::<Vec<_>>(),
    },
    polkadot_xcm: PolkadotXcmConfig {
      safe_xcm_version: Some(SAFE_XCM_VERSION)
    },
    axial_router: pallet_axial_router::GenesisConfig {
      tracked_assets: local_web_client_tracked_assets(),
      _marker: Default::default(),
    },
    token_minting_curve: pallet_tmc::GenesisConfig {
      curves: local_web_client_curves(),
      _marker: Default::default(),
    },
    staking: pallet_staking::GenesisConfig {
      registered_assets: vec![LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID],
      _marker: Default::default(),
    },
  });
  patch["collatorSelection"]["desiredCandidates"] = Value::from(0);
  patch
}

fn local_testnet_genesis() -> Value {
  testnet_genesis(
    // initial collators.
    vec![
      (
        Sr25519Keyring::Alice.to_account_id(),
        Sr25519Keyring::Alice.public().into(),
      ),
      (
        Sr25519Keyring::Bob.to_account_id(),
        Sr25519Keyring::Bob.public().into(),
      ),
    ],
    Sr25519Keyring::well_known()
      .map(|k| k.to_account_id())
      .collect(),
    Sr25519Keyring::Alice.to_account_id(),
    PARACHAIN_ID.into(),
  )
}

fn development_config_genesis() -> Value {
  testnet_genesis(
    // initial collators.
    vec![
      (
        Sr25519Keyring::Alice.to_account_id(),
        Sr25519Keyring::Alice.public().into(),
      ),
      (
        Sr25519Keyring::Bob.to_account_id(),
        Sr25519Keyring::Bob.public().into(),
      ),
    ],
    Sr25519Keyring::well_known()
      .map(|k| k.to_account_id())
      .collect(),
    Sr25519Keyring::Alice.to_account_id(),
    PARACHAIN_ID.into(),
  )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<vec::Vec<u8>> {
  let patch = match id.as_ref() {
    sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_testnet_genesis(),
    sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
    _ => return None,
  };
  Some(
    serde_json::to_string(&patch)
      .expect("serialization to json is expected to work. qed.")
      .into_bytes(),
  )
}

/// List of supported presets.
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
  fn development_preset_includes_well_known_veto_asset_metadata() {
    let genesis = development_config_genesis();
    let veto_asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_eq!(
      genesis["assets"]["assets"][0][0],
      Value::from(veto_asset_id)
    );
    assert_eq!(
      genesis["assets"]["metadata"][0][0],
      Value::from(veto_asset_id)
    );
    assert_eq!(
      genesis["assets"]["metadata"][0][1],
      serde_json::json!(b"Veto Governance Token".to_vec())
    );
    assert_eq!(
      genesis["assets"]["metadata"][0][2],
      serde_json::json!(b"VETO".to_vec())
    );
    assert_eq!(genesis["assets"]["metadata"][0][3], Value::from(12));
    assert_eq!(
      genesis["assets"]["assets"][1][0],
      Value::from(LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID)
    );
    assert_eq!(
      genesis["assets"]["metadata"][1][0],
      Value::from(LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID)
    );
    assert_eq!(
      genesis["assets"]["metadata"][1][1],
      serde_json::json!(b"Native Staking Token".to_vec())
    );
    assert_eq!(
      genesis["assets"]["metadata"][1][2],
      serde_json::json!(b"NTVE".to_vec())
    );
    assert_eq!(genesis["assets"]["metadata"][1][3], Value::from(12));
    assert_eq!(
      genesis["assets"]["assets"][2][0],
      Value::from(LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID)
    );
    assert_eq!(
      genesis["assets"]["metadata"][2][0],
      Value::from(LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID)
    );
    assert_eq!(
      genesis["assets"]["metadata"][2][1],
      serde_json::json!(b"Foreign Token".to_vec())
    );
    assert_eq!(
      genesis["assets"]["metadata"][2][2],
      serde_json::json!(b"FRGN".to_vec())
    );
    assert_eq!(genesis["assets"]["metadata"][2][3], Value::from(12));
    assert_eq!(
      genesis["staking"]["registeredAssets"],
      serde_json::json!([LOCAL_WEB_CLIENT_NATIVE_STAKING_ASSET_ID])
    );
    assert_eq!(
      genesis["axialRouter"]["trackedAssets"],
      serde_json::json!([
        "Native",
        { "Foreign": LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID }
      ])
    );
    assert_eq!(
      genesis["tokenMintingCurve"]["curves"],
      serde_json::json!([
        [
          "Native",
          { "Foreign": LOCAL_WEB_CLIENT_FOREIGN_ASSET_ID },
          LOCAL_WEB_CLIENT_INITIAL_PRICE,
          LOCAL_WEB_CLIENT_SLOPE,
        ]
      ])
    );
  }
}
