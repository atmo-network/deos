use deos_runtime::{PARACHAIN_ID, ParachainInfoConfig, Runtime, RuntimeGenesisConfig};
use polkadot_sdk::{
  cumulus_primitives_core::ParaId, sp_core::storage::Storage, sp_io::TestExternalities,
  sp_runtime::BuildStorage,
};
use std::{env, path::PathBuf, process};

fn canonical_metadata_storage() -> Result<Storage, String> {
  // Metadata constants such as ParachainSystem::SelfParaId and
  // PolkadotXcm::UniversalLocation are storage-backed. Export under the same
  // canonical parachain identity used by every reference preset rather than
  // silently encoding the empty-externalities default (ParaId 100).
  RuntimeGenesisConfig {
    parachain_info: ParachainInfoConfig {
      parachain_id: ParaId::from(PARACHAIN_ID),
      ..Default::default()
    },
    ..Default::default()
  }
  .build_storage()
}

fn usage() {
  eprintln!("Usage: export_metadata <output-path> [metadata-version]");
}

fn main() {
  let args = env::args().skip(1).collect::<Vec<_>>();
  let help_requested = matches!(args.first().map(String::as_str), Some("--help" | "-h"));
  if args.is_empty() || args.len() > 2 || help_requested {
    usage();
    process::exit(if help_requested { 0 } else { 1 });
  }
  let output_path = PathBuf::from(&args[0]);
  let metadata_version = args
    .get(1)
    .map(|value| {
      value.parse::<u32>().unwrap_or_else(|error| {
        eprintln!("Invalid metadata version `{value}`: {error}");
        process::exit(1);
      })
    })
    .unwrap_or(16);
  let storage = canonical_metadata_storage().unwrap_or_else(|error| {
    eprintln!("Failed to build canonical metadata externalities: {error}");
    process::exit(1);
  });
  let mut ext = TestExternalities::new(storage);
  let metadata = ext.execute_with(|| {
    Runtime::metadata_at_version(metadata_version).unwrap_or_else(|| {
      eprintln!("Runtime metadata version {metadata_version} is unavailable");
      process::exit(1);
    })
  });
  if let Some(parent) = output_path.parent() {
    std::fs::create_dir_all(parent).unwrap_or_else(|error| {
      eprintln!(
        "Failed to create output directory `{}`: {error}",
        parent.display()
      );
      process::exit(1);
    });
  }
  std::fs::write(&output_path, &*metadata).unwrap_or_else(|error| {
    eprintln!(
      "Failed to write metadata `{}`: {error}",
      output_path.display()
    );
    process::exit(1);
  });
  println!(
    "Wrote metadata v{metadata_version} to {}",
    output_path.display()
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn metadata_externalities_use_the_reference_parachain_identity() {
    let mut ext = TestExternalities::new(
      canonical_metadata_storage().expect("canonical metadata storage builds"),
    );
    ext.execute_with(|| {
      assert_eq!(
        deos_runtime::ParachainInfo::parachain_id(),
        ParaId::from(PARACHAIN_ID)
      );
    });
  }
}
