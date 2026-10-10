//! Fixed corpus identity shared by the arithmetic and native admission diagnostics.

use polkadot_sdk::sp_io::hashing::sha2_256;
use serde_json::Value;
use std::{fs::File, io::Read};

pub const PAGES: usize = 9_968;
pub const WATCHES: usize = 4;
pub const INPUT_SHA256: &str = "0d8ae6552e01ef1127451d0f97fa9320ed5913ac4384bc5ffd4cf13f48302db6";

pub fn digest(bytes: &[u8]) -> String {
  sha2_256(bytes)
    .iter() // deos-bypass: bounded-iter -- fixed 32-byte SHA-256 digest
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

pub fn fixed_pages(path: &str) -> Vec<Vec<u64>> {
  assert_eq!(
    WATCHES as u32,
    <<deos_runtime::Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches
      as polkadot_sdk::frame_support::traits::Get<u32>>::get(),
    "fixed corpus watch domain does not match the current runtime"
  );
  let mut input = Vec::new();
  File::open(path)
    .unwrap()
    .take(4_000_001)
    .read_to_end(&mut input)
    .unwrap();
  assert!(input.len() <= 4_000_000);
  assert_eq!(
    digest(&input),
    INPUT_SHA256,
    "only the already fixed corpus is admitted"
  );
  let value: Value = serde_json::from_slice(&input).unwrap();
  assert_eq!(value["format"], "deos.actor.targeted-retention-pages");
  assert_eq!(value["version"], 1);
  assert_eq!(value["prefix_nibbles"], 5);
  let sources = value["source_pages"].as_array().unwrap();
  assert_eq!(sources.len(), WATCHES);
  sources
    .iter() // deos-bypass: bounded-iter -- four sources in the exact hash-bound corpus
    .enumerate()
    .map(|(source, pages)| {
      let pages = pages.as_array().unwrap();
      assert_eq!(pages.len(), PAGES);
      let pages = pages
        .iter() // deos-bypass: bounded-iter -- 9968 pages per hash-bound source
        .map(|page| page.as_u64().unwrap())
        .collect::<Vec<_>>();
      assert!(pages.windows(2).all(|pair| pair[0] < pair[1]));
      assert_eq!(pages[0], 0);
      assert_eq!(pages[PAGES / 2], 5_000_000 + source as u64);
      assert_eq!(pages[PAGES - 1], 10_000_000 + source as u64);
      pages
    })
    .collect()
}
