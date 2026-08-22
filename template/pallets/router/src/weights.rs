//! # Unmeasured reference values
//!
//! The `WeightInfo` implementations in this file are hand-written estimates, not benchmark
//! output. They exist so the pallet compiles and tests run standalone.
//!
//! A host runtime MUST generate its own weights with `frame-benchmarking` and bind those instead.
//! Binding `SubstrateWeight` or `()` from this file in production underprices execution: the DEOS
//! reference runtime measures several of these calls at more than ten times the value below, with
//! ProofSize and database access that these estimates omit entirely.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use polkadot_sdk::frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions for `pallet_deos_router`.
pub trait WeightInfo {
	fn swap() -> Weight {
		let direct = Self::direct_xyk_exact_input();
		let mint = Self::direct_mint_exact_input();
		let anchored = Self::native_anchored_exact_input();
		Weight::from_parts(
			direct.ref_time().max(mint.ref_time()).max(anchored.ref_time()),
			direct.proof_size().max(mint.proof_size()).max(anchored.proof_size()),
		)
	}
	fn direct_xyk_exact_input() -> Weight;
	fn direct_mint_exact_input() -> Weight;
	fn native_anchored_exact_input() -> Weight;
	fn direct_xyk_exact_output() -> Weight;
	fn native_anchored_exact_output() -> Weight;
	fn update_router_fee() -> Weight;
}

/// Weights for `pallet_deos_router` using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn direct_xyk_exact_input() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn direct_mint_exact_input() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn native_anchored_exact_input() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn direct_xyk_exact_output() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn native_anchored_exact_output() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn update_router_fee() -> Weight {
		Weight::from_parts(10_000_000, 1000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}

// Standalone fallback for mocks and non-production embedding tests.
impl WeightInfo for () {
	fn direct_xyk_exact_input() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn direct_mint_exact_input() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn native_anchored_exact_input() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn direct_xyk_exact_output() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn native_anchored_exact_output() -> Weight { Weight::from_parts(200_000_000, 0) }
	fn update_router_fee() -> Weight {
		Weight::from_parts(10_000_000, 1000)
	}
}
