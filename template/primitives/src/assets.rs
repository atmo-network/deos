use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

/// This enum serves as the single source of truth for asset types across all pallets,
/// enabling type-safe interactions between Axial Router, TMC, Burning Manager, and other actors.
///
/// - `Native`: The system's native token (managed by pallet-balances).
/// - `Local(u32)`: Local synthetic assets (managed by pallet-assets).
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  Serialize,
  Deserialize,
)]
pub enum AssetKind {
  /// Native token managed by pallet-balances
  #[default]
  Native,
  /// Local asset managed by pallet-assets
  Local(u32),
  /// Foreign asset managed by pallet-assets via XCM mapping (0xF... namespace)
  Foreign(u32),
}

impl From<u32> for AssetKind {
  fn from(asset_id: u32) -> Self {
    AssetKind::Local(asset_id)
  }
}

// Bitmask Architecture for Asset Classification
//
// 32-bit ID Structure:
// [ 4 bits: Type ] [ 28 bits: Index/ID ]
//
// Types:
// 0x1... -> Protocol Tokens ($VETO, $BLDR)
// 0x5... -> Native/Local Staking Receipts
// 0x6... -> Foreign Staking Receipts
// 0x7... -> LP Tokens
// 0xF... -> Foreign/XCM Assets

pub const MASK_TYPE: u32 = 0xF000_0000;
pub const MASK_INDEX: u32 = 0x0FFF_FFFF;

pub const TYPE_PROTOCOL: u32 = 0x1000_0000;
pub const TYPE_STAKED: u32 = 0x5000_0000;
pub const TYPE_STAKED_FOREIGN: u32 = 0x6000_0000;
pub const TYPE_LP: u32 = 0x7000_0000;
pub const TYPE_FOREIGN: u32 = 0xF000_0000;

/// Helper trait to inspect AssetKind properties
pub trait AssetInspector {
  fn is_native(&self) -> bool;
  fn local_id(&self) -> Option<u32>;
  fn is_protocol(&self) -> bool;
  fn is_lp(&self) -> bool;
  fn is_staked(&self) -> bool;
  fn is_foreign(&self) -> bool;
}

impl AssetKind {
  /// Deterministically derive the local staking-receipt asset ID for a given base asset.
  ///
  /// Native and local assets use the `0x5...` receipt namespace. Foreign assets use the dedicated
  /// `0x6...` receipt namespace so their receipts remain collision-free under the current
  /// 32-bit `[type:4 | index:28]` contract.
  pub fn into_staked(self) -> Option<Self> {
    match self {
      AssetKind::Native => Some(AssetKind::Local(TYPE_STAKED | 0)),
      AssetKind::Local(id) => {
        let asset_type = id & MASK_TYPE;
        // Prevent stacking: st(stXXXX), st(stfXXXX), or st(LP) is not allowed
        if asset_type == TYPE_STAKED || asset_type == TYPE_STAKED_FOREIGN || asset_type == TYPE_LP {
          None
        } else {
          Some(AssetKind::Local(TYPE_STAKED | (id & MASK_INDEX)))
        }
      }
      AssetKind::Foreign(id) => Some(AssetKind::Local(TYPE_STAKED_FOREIGN | (id & MASK_INDEX))),
    }
  }
}

impl AssetInspector for AssetKind {
  fn is_native(&self) -> bool {
    matches!(self, AssetKind::Native)
  }

  fn local_id(&self) -> Option<u32> {
    match self {
      AssetKind::Local(id) | AssetKind::Foreign(id) => Some(*id),
      _ => None,
    }
  }

  fn is_protocol(&self) -> bool {
    match self {
      AssetKind::Local(id) => (id & MASK_TYPE) == TYPE_PROTOCOL,
      _ => false,
    }
  }

  fn is_staked(&self) -> bool {
    match self {
      AssetKind::Local(id) => {
        let asset_type = id & MASK_TYPE;
        asset_type == TYPE_STAKED || asset_type == TYPE_STAKED_FOREIGN
      }
      _ => false,
    }
  }

  fn is_lp(&self) -> bool {
    match self {
      AssetKind::Local(id) => (id & MASK_TYPE) == TYPE_LP,
      _ => false,
    }
  }

  fn is_foreign(&self) -> bool {
    match self {
      AssetKind::Foreign(_) => true,
      AssetKind::Local(id) => (id & MASK_TYPE) == TYPE_FOREIGN,
      _ => false,
    }
  }
}

/// Trait for type conversions with additional context
pub trait TryConvertFrom<T, Context> {
  type Error;
  fn try_convert_from(value: T, context: Context) -> Result<Self, Self::Error>
  where
    Self: Sized;
}

/// Helper to construct compile-time IDs
const fn make_id(type_mask: u32, index: u32) -> u32 {
  type_mask | (index & MASK_INDEX)
}

/// Well-known asset constants serving as system defaults
pub mod well_known {
  use super::*;

  pub const VETO: u32 = make_id(TYPE_PROTOCOL, 1);
  pub const BLDR: u32 = make_id(TYPE_PROTOCOL, 2);
}

/// Metadata container for currencies
#[derive(Encode, Decode, DecodeWithMemTracking, Eq, PartialEq, Clone, Debug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CurrencyMetadata {
  pub name: Vec<u8>,
  pub symbol: Vec<u8>,
  pub decimals: u8,
}

/// Helper to resolve metadata for well-known assets (off-chain / view logic)
pub fn get_well_known_metadata(asset: AssetKind) -> Option<CurrencyMetadata> {
  match asset {
    AssetKind::Native => Some(CurrencyMetadata {
      name: b"Native Token".to_vec(),
      symbol: b"NATIVE".to_vec(),
      decimals: 12,
    }),
    AssetKind::Local(id) => match id {
      well_known::VETO => Some(CurrencyMetadata {
        name: b"Veto Governance Token".to_vec(),
        symbol: b"VETO".to_vec(),
        decimals: 12,
      }),
      well_known::BLDR => Some(CurrencyMetadata {
        name: b"Builder Incentive Token".to_vec(),
        symbol: b"BLDR".to_vec(),
        decimals: 12,
      }),
      _ => None,
    },
    AssetKind::Foreign(_) => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn well_known_ids_in_protocol_namespace() {
    assert_eq!(well_known::VETO & MASK_TYPE, TYPE_PROTOCOL);
    assert_eq!(well_known::BLDR & MASK_TYPE, TYPE_PROTOCOL);
  }

  #[test]
  fn asset_inspection() {
    let veto = AssetKind::Local(well_known::VETO);
    assert!(veto.is_protocol());
    assert!(!veto.is_lp());
    assert!(!veto.is_foreign());

    let native = AssetKind::Native;
    assert!(native.is_native());
    assert!(!native.is_protocol());
  }

  #[test]
  fn metadata() {
    let meta = get_well_known_metadata(AssetKind::Native).unwrap();
    assert_eq!(meta.symbol, b"NATIVE".to_vec());

    let meta = get_well_known_metadata(AssetKind::Local(well_known::BLDR)).unwrap();
    assert_eq!(meta.symbol, b"BLDR".to_vec());

    assert!(get_well_known_metadata(AssetKind::Local(0)).is_none());
  }

  #[test]
  fn bitmask_boundaries() {
    let max_protocol = AssetKind::Local(TYPE_PROTOCOL | MASK_INDEX);
    assert!(max_protocol.is_protocol());
    assert!(!max_protocol.is_lp());

    let min_lp = AssetKind::Local(TYPE_LP);
    assert!(min_lp.is_lp());
    assert!(!min_lp.is_protocol());
  }

  #[test]
  fn lp_namespace_isolation() {
    let lp_token = AssetKind::Local(TYPE_LP | 12345);
    assert!(lp_token.is_lp());
    assert!(!lp_token.is_protocol());
    assert!(!lp_token.is_foreign());

    let not_lp = AssetKind::Local(TYPE_PROTOCOL | 12345);
    assert!(!not_lp.is_lp());
  }

  #[test]
  fn foreign_asset_isolation() {
    let foreign_asset = AssetKind::Foreign(TYPE_FOREIGN | 12345);
    assert!(foreign_asset.is_foreign());
    assert!(!foreign_asset.is_native());
    assert_eq!(foreign_asset.local_id(), Some(TYPE_FOREIGN | 12345));

    let protocol_asset = AssetKind::Local(TYPE_PROTOCOL | 1);
    assert!(!protocol_asset.is_foreign());

    assert!(!AssetKind::Native.is_foreign());
  }

  #[test]
  fn staked_asset_derivation() {
    let native_staked = AssetKind::Native.into_staked().unwrap();
    assert!(native_staked.is_staked());
    assert_eq!(native_staked, AssetKind::Local(TYPE_STAKED | 0));

    let local_base = AssetKind::Local(TYPE_PROTOCOL | 123);
    let local_staked = local_base.into_staked().unwrap();
    assert!(local_staked.is_staked());
    assert_eq!(local_staked, AssetKind::Local(TYPE_STAKED | 123));

    let foreign_base = AssetKind::Foreign(TYPE_FOREIGN | 456);
    let foreign_staked = foreign_base.into_staked().unwrap();
    assert!(foreign_staked.is_staked());
    assert_eq!(foreign_staked, AssetKind::Local(TYPE_STAKED_FOREIGN | 456));

    // Prevent st(stXXXX)
    assert!(local_staked.into_staked().is_none());

    // Prevent st(stfXXXX)
    assert!(foreign_staked.into_staked().is_none());

    // Prevent st(LP)
    let lp = AssetKind::Local(TYPE_LP | 789);
    assert!(lp.into_staked().is_none());
  }

  #[test]
  fn protocol_tokens_match_ecosystem_constants() {
    use crate::ecosystem::protocol_tokens;
    assert_eq!(well_known::VETO, protocol_tokens::VETO_ASSET_ID);
    assert_eq!(well_known::BLDR, protocol_tokens::BLDR_ASSET_ID);
    assert_ne!(well_known::VETO, well_known::BLDR);
  }
}
