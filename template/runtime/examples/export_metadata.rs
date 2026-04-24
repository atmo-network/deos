use deos_runtime::Runtime;
use polkadot_sdk::{sp_core::storage::Storage, sp_io::TestExternalities};
use std::{env, path::PathBuf, process};

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
  let mut ext = TestExternalities::new(Storage::default());
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
