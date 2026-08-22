use polkadot_sdk::sp_core::U256;

/// Computes `numerator * scale / denominator` for quantities with identical
/// numerator/denominator units.
///
/// The multiplication and division are widened to `U256`. A zero denominator,
/// failed widened operation, or quotient that cannot narrow exactly to `u128`
/// returns `None`; authoritative callers must fail closed rather than saturate.
pub fn checked_scaled_ratio(numerator: u128, denominator: u128, scale: u128) -> Option<u128> {
  if denominator == 0 {
    return None;
  }
  U256::from(numerator)
    .checked_mul(U256::from(scale))?
    .checked_div(U256::from(denominator))?
    .try_into()
    .ok()
}

#[cfg(test)]
mod tests {
  use super::checked_scaled_ratio;

  #[test]
  fn equal_near_maximum_quantities_preserve_scale() {
    assert_eq!(
      checked_scaled_ratio(u128::MAX, u128::MAX, 1_000_000_000_000),
      Some(1_000_000_000_000)
    );
  }

  #[test]
  fn asymmetric_extremes_are_checked_without_native_width_saturation() {
    assert_eq!(checked_scaled_ratio(1, u128::MAX, u128::MAX), Some(1));
    assert_eq!(checked_scaled_ratio(u128::MAX, 1, 2), None);
  }

  #[test]
  fn zero_denominator_and_unrepresentable_narrowing_fail_closed() {
    assert_eq!(checked_scaled_ratio(1, 0, 1), None);
    assert_eq!(checked_scaled_ratio(u128::MAX, 1, u128::MAX), None);
  }

  #[test]
  fn representable_maximum_quotient_narrows_exactly() {
    assert_eq!(
      checked_scaled_ratio(u128::MAX, u128::MAX, u128::MAX),
      Some(u128::MAX)
    );
  }
}
