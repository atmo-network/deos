use crate::AssetKind;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub enum LocalPoolObservationMethod {
  PreExecutionSpot,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub enum OracleAggregationId {
  LastValue,
  Ema { half_life_blocks: u32 },
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub struct OracleFeedId {
  pub asset_in: AssetKind,
  pub asset_out: AssetKind,
  pub method: LocalPoolObservationMethod,
  pub aggregation: OracleAggregationId,
  pub scale: u8,
}

impl OracleFeedId {
  pub const fn directional_local_pool_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
    method: LocalPoolObservationMethod,
    aggregation: OracleAggregationId,
    scale: u8,
  ) -> Self {
    Self {
      asset_in,
      asset_out,
      method,
      aggregation,
      scale,
    }
  }

  pub const fn reverse(self) -> Self {
    Self {
      asset_in: self.asset_out,
      asset_out: self.asset_in,
      method: self.method,
      aggregation: self.aggregation,
      scale: self.scale,
    }
  }

  pub const fn meaning(self) -> OracleMeaning {
    OracleMeaning::DirectionalLocalPoolPrice {
      asset_in: self.asset_in,
      asset_out: self.asset_out,
      method: self.method,
    }
  }
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum OracleMeaning {
  DirectionalLocalPoolPrice {
    asset_in: AssetKind,
    asset_out: AssetKind,
    method: LocalPoolObservationMethod,
  },
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum OracleProvenance {
  DeosRouterPreExecutionReserves,
}

#[cfg(feature = "runtime-benchmarks")]
impl From<u32> for OracleFeedId {
  fn from(value: u32) -> Self {
    Self::directional_local_pool_price(
      AssetKind::Native,
      AssetKind::Local(value),
      LocalPoolObservationMethod::PreExecutionSpot,
      OracleAggregationId::LastValue,
      0,
    )
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl Default for OracleMeaning {
  fn default() -> Self {
    Self::DirectionalLocalPoolPrice {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(0),
      method: LocalPoolObservationMethod::PreExecutionSpot,
    }
  }
}

#[cfg(feature = "runtime-benchmarks")]
impl Default for OracleProvenance {
  fn default() -> Self {
    Self::DeosRouterPreExecutionReserves
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn feed() -> OracleFeedId {
    OracleFeedId::directional_local_pool_price(
      AssetKind::Native,
      AssetKind::Local(7),
      LocalPoolObservationMethod::PreExecutionSpot,
      OracleAggregationId::Ema {
        half_life_blocks: 100,
      },
      12,
    )
  }

  #[test]
  fn directional_identity_keeps_reverse_distinct() {
    let forward = feed();
    let reverse = forward.reverse();
    assert_ne!(forward, reverse);
    assert_eq!(reverse.reverse(), forward);
    assert_eq!(
      forward.meaning().encode(),
      OracleMeaning::DirectionalLocalPoolPrice {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(7),
        method: LocalPoolObservationMethod::PreExecutionSpot,
      }
      .encode()
    );
  }

  #[test]
  fn directional_identity_includes_aggregation_and_scale() {
    let canonical = feed();
    assert_ne!(
      canonical,
      OracleFeedId {
        aggregation: OracleAggregationId::LastValue,
        ..canonical
      }
    );
    assert_ne!(
      canonical,
      OracleFeedId {
        scale: 11,
        ..canonical
      }
    );
  }
}
