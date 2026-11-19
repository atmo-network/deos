fn ensure_native_fixture_profile() {
  println!("cargo:rerun-if-env-changed=SKIP_WASM_BUILD");
  if cfg!(all(
    feature = "benchmark-authority-scaffolding",
    not(feature = "runtime-benchmarks")
  )) {
    assert!(
      cfg!(feature = "std"),
      "native admission fixture requires std"
    );
    assert!(
      !std::env::var("CARGO_CFG_TARGET_FAMILY")
        .is_ok_and(|families| families.split(',').any(|family| family == "wasm")),
      "native admission fixture cannot build Wasm"
    );
    assert!(
      !cfg!(feature = "on-chain-release-build"),
      "native admission fixture cannot build a release artifact"
    );
    assert!(
      std::env::var("SKIP_WASM_BUILD").as_deref() == Ok("1"),
      "native admission fixture requires explicit SKIP_WASM_BUILD=1"
    );
  }
}

#[cfg(all(feature = "std", feature = "metadata-hash"))]
#[docify::export(template_enable_metadata_hash)]
fn main() {
  ensure_native_fixture_profile();
  substrate_wasm_builder::WasmBuilder::init_with_defaults()
    .enable_metadata_hash("UNIT", 12)
    .build();
}

#[cfg(all(feature = "std", not(feature = "metadata-hash")))]
fn main() {
  ensure_native_fixture_profile();
  substrate_wasm_builder::WasmBuilder::build_using_defaults();
}

/// The wasm builder is deactivated when compiling
/// this crate for wasm to speed up the compilation.
#[cfg(not(feature = "std"))]
fn main() {
  ensure_native_fixture_profile();
}
