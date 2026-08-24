use crate::*;
use polkadot_sdk::frame_benchmarking::v2::*;

#[benchmarks]
mod benches {
  use super::*;

  #[benchmark]
  fn rotate_session() {
    T::BenchmarkHelper::prepare_rotation();

    #[block]
    {
      T::SessionRotation::rotate();
    }

    T::BenchmarkHelper::verify_rotation();
  }

  impl_benchmark_test_suite!(
    Pallet,
    polkadot_sdk::sp_io::TestExternalities::default(),
    crate::tests::Test
  );
}
