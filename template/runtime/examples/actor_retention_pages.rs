// Generate the four-watch fixed hash-selected retirement history for native diagnostics.
//
// This replaces the former Python diagnostic so the repository keeps one automation boundary:
// no Python entrypoint remains. Rust is the honest executor here rather than JavaScript because
// the selection needs BLAKE2b with a 16-byte digest, Node exposes only the 512-bit variant, and
// a hand-rolled JavaScript implementation needs minutes for the 1.2M selections. The SDK's
// `blake2_128` is the same primitive the runtime uses for storage keys, so the artifact stays
// exactly what the Python diagnostic produced, including compact JSON and the trailing newline
// that the pinned integration digest covers.
//
// Usage: cargo run --release -p deos-runtime --example actor_retention_pages -- OUTPUT

use std::path::PathBuf;

const PAGES: usize = 9_968;
const WATCHES: usize = 4;
const PREFIX_NIBBLES: u8 = 5;
const LOCAL_BASE: u64 = 5_000_000;
const TAIL_BASE: u64 = 10_000_000;

fn prefix(page: u64, slot: u8) -> u32 {
  let mut payload = [0u8; 9];
  payload[..8].copy_from_slice(&page.to_le_bytes());
  payload[8] = slot;
  let digest = polkadot_sdk::sp_crypto_hashing::blake2_128(&payload);
  u32::from_be_bytes([0, digest[0], digest[1], digest[2]]) >> 4
}

fn source_pages(source: u64) -> Vec<u64> {
  let local = LOCAL_BASE + source;
  let tail = TAIL_BASE + source;
  let mut wanted = std::collections::BTreeSet::new();
  for page in [local, tail] {
    for slot in 1..32u8 {
      wanted.insert(prefix(page, slot));
    }
  }

  let mut selected: Vec<u64> = vec![0];
  let intervals = [
    (1u64, local, PAGES / 2 - 1),
    (local + 1, tail, PAGES - PAGES / 2 - 2),
  ];
  for (start, end, count) in intervals {
    let mut taken = 0usize;
    let mut page = start;
    while page < end {
      let matched = (1..32u8).any(|slot| wanted.contains(&prefix(page, slot)));
      if matched {
        selected.push(page);
        taken += 1;
        if taken == count {
          break;
        }
      }
      page += 1;
    }
    assert_eq!(taken, count, "source {source}: fixed interval exhausted");
    println!(
      "source={source} interval={start}..{end} retained={count} last={}",
      selected[selected.len() - 1]
    );
    if start == 1 {
      selected.push(local);
    }
  }
  selected.push(tail);

  assert_eq!(selected.len(), PAGES, "source {source}: page count");
  let unique: std::collections::BTreeSet<u64> = selected.iter().copied().collect();
  assert_eq!(unique.len(), PAGES, "source {source}: duplicate pages");
  let mut sorted = selected.clone();
  sorted.sort_unstable();
  assert_eq!(sorted, selected, "source {source}: pages are not sorted");
  assert_eq!(
    selected[PAGES / 2],
    local,
    "source {source}: midpoint is not the local page"
  );
  selected
}

fn main() {
  let mut args = std::env::args().skip(1);
  let output = match (args.next(), args.next()) {
    (Some(path), None) if path != "--help" && path != "-h" => PathBuf::from(path),
    _ => {
      println!(
        "Usage: cargo run --release -p deos-runtime --example actor_retention_pages -- OUTPUT"
      );
      println!();
      println!(
        "Generate the four-watch fixed hash-selected retirement history for native diagnostics."
      );
      println!(
        "OUTPUT is a new JSON path; an existing file is refused so the pinned artifact is never"
      );
      println!("overwritten by accident.");
      return;
    }
  };
  if output.exists() {
    eprintln!(
      "[ERROR] {} already exists; validate/reuse it or select a new path",
      output.display()
    );
    std::process::exit(1);
  }

  let source_pages: Vec<Vec<u64>> = (0..WATCHES as u64).map(source_pages).collect();
  // The artifact is rendered directly so key order and separators match the former Python
  // diagnostic exactly; a serializer that sorts keys would break the pinned digest.
  let mut rendered = String::from(
    "{\"format\":\"deos.actor.targeted-retention-pages\",\"version\":1,\"prefix_nibbles\":",
  );
  rendered.push_str(&PREFIX_NIBBLES.to_string());
  rendered.push_str(",\"source_pages\":[");
  for (index, pages) in source_pages.iter().enumerate() {
    if index > 0 {
      rendered.push(',');
    }
    rendered.push('[');
    for (position, page) in pages.iter().enumerate() {
      if position > 0 {
        rendered.push(',');
      }
      rendered.push_str(&page.to_string());
    }
    rendered.push(']');
  }
  rendered.push_str("]}\n");
  std::fs::write(&output, rendered).expect("artifact writes");
  println!(
    "output={} pages={PAGES} watches={WATCHES}",
    output.display()
  );
}
