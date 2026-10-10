//! Current-domain signed wide Close through the real SDK caller and optional PVF export.
//! Public native preparation is outside the encoded block; no resource maximum is claimed.

#[path = "actor_retention_fixture/wide_close.rs"]
mod wide_close_fixture;
use deos_runtime::{Block, Runtime};
use polkadot_sdk::sp_genesis_builder::runtime_decl_for_genesis_builder::GenesisBuilder;
use wide_close_fixture::run;

fn main() -> Result<(), String> {
  let arguments = std::env::args().skip(1).collect::<Vec<_>>();
  if arguments == ["--help"] || arguments == ["-h"] {
    println!(
      "Usage: actor_wide_close [--sdk-wasm PATH | --pvf-fixture PATH NEW_DIR]\n\nRuns both named presets through current one-watch, twelve-Step wide no-policy signed Close.\nNative public preparation is outside the encoded block; SDK builders and recording/replay\nCore imports compare complete TOP/root and push executor/decode/duplicate rollback.\n--sdk-wasm selects actual Wasm with full version checks and no native fallback.\n--pvf-fixture binds parent code and exports Aura-sealed candidates and complete expected\nresults to a new directory. Worker validation is separate. The old four-watch corpus is\nnot used or modified. This is finite schema/caller acceptance, not complete physical\nresource/tariff, population, public history, live node or relay provenance."
    );
    return Ok(());
  }
  let (wasm, output) = match arguments.as_slice() {
    [] => (None, None),
    [flag, path] if flag == "--sdk-wasm" => (
      Some(std::fs::read(path).map_err(|error| format!("cannot read Wasm: {error}"))?),
      None,
    ),
    [flag, path, directory] if flag == "--pvf-fixture" => {
      let code = std::fs::read(path).map_err(|error| format!("cannot read Wasm: {error}"))?;
      let output = std::path::PathBuf::from(directory);
      std::fs::create_dir(&output)
        .map_err(|error| format!("PVF output must be a new directory: {error}"))?;
      (Some(code), Some(output))
    }
    _ => return Err("unexpected arguments; use --help".into()),
  };
  for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
    run(&preset, wasm.as_deref(), output.as_deref());
  }
  Ok(())
}
