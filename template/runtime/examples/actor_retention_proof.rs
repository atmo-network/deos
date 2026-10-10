mod actor_retention_corpus;

use actor_retention_corpus::{PAGES, WATCHES, fixed_pages};
use codec::Encode;
use deos_runtime::Runtime;
use pallet_deos_actors::{
  Config, DependencyRegistrationFreeIndices, DependencyRegistrationFreePositions,
  DependencyRegistrationPosition,
};
use polkadot_sdk::{
  sp_core::{Blake2Hasher, H256},
  sp_trie::{LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder},
};
use std::collections::BTreeSet;

type Layout = LayoutV1<Blake2Hasher>;
type Positions = DependencyRegistrationFreePositions<Runtime>;
type Indices = DependencyRegistrationFreeIndices<Runtime>;

#[derive(Clone, Copy)]
enum Table {
  Positions,
  Indices,
}

fn position(pages: &[u64], index: u64) -> DependencyRegistrationPosition {
  DependencyRegistrationPosition {
    page: pages[(index / 31) as usize],
    slot: (index % 31 + 1) as u8,
  }
}

fn key(table: Table, source: u64, pages: &[u64], index: u64) -> Vec<u8> {
  let full = match table {
    Table::Positions => Positions::hashed_key_for(source, index),
    Table::Indices => Indices::hashed_key_for(source, position(pages, index)),
  };
  assert_eq!(
    full.len(),
    if matches!(table, Table::Positions) {
      80
    } else {
      81
    }
  );
  full[56..].to_vec()
}

fn prefix(source: u64, page: u64, slot: u8) -> u32 {
  let full = Indices::hashed_key_for(source, DependencyRegistrationPosition { page, slot });
  (u32::from(full[56]) << 16 | u32::from(full[57]) << 8 | u32::from(full[58])) >> 4
}

fn run(path: &str) {
  let source_pages = fixed_pages(path);
  let pages = u64::from(<Runtime as Config>::MaxActiveActors::get()) - 32;
  let watches = u64::from(<Runtime as Config>::MaxParkedBalanceWatches::get());
  assert_eq!((pages, watches), (PAGES as u64, WATCHES as u64));
  let count = 31 * pages;
  let local = 31 * (pages / 2);
  let limit = <Runtime as Config>::BlockResourceBudget::get()
    .limits()
    .actor_control()
    .proof_size();
  println!(
    "scope=hash_selected_retirement_suffix_forest pages={pages} watches={watches} holes_per_source={count} control_proof_limit={limit}"
  );
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let mut recorder = Recorder::<Layout>::new();
  let mut records = Vec::new();
  let mut roots = BTreeSet::<H256>::new();
  let mut queries = Vec::new();
  for source in 0..watches {
    let retained = &source_pages[source as usize];
    assert_eq!(retained.len(), pages as usize);
    let wanted = [retained[pages as usize / 2], *retained.last().unwrap()]
      .into_iter()
      .flat_map(|page| (1..32).map(move |slot| prefix(source, page, slot)))
      .collect::<BTreeSet<_>>();
    for (index, page) in retained
      .iter(/* deos-bypass: bounded-iter -- exactly 9968 validated fixture pages per source. */)
      .enumerate()
    {
      if index != 0 && index != pages as usize / 2 && index != pages as usize - 1 {
        assert!((1..32).any(|slot| wanted.contains(&prefix(source, *page, slot))));
      }
    }
    for table in [Table::Positions, Table::Indices] {
      let mut rows = (0..count)
        .map(|index| {
          let value = match table {
            Table::Positions => position(retained, index).encode(),
            Table::Indices => index.encode(),
          };
          (key(table, source, retained, index), value)
        })
        .collect::<Vec<_>>();
      rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
      assert!(rows.windows(2).all(|pair| pair[0].0 != pair[1].0));
      let root = Layout::trie_build(&mut database, rows);
      roots.insert(root);
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      let mut selected = Vec::new();
      for index in (local..local + 31).chain(count - 31..count) {
        let value = match table {
          Table::Positions => position(retained, index).encode(),
          Table::Indices => index.encode(),
        };
        let key = key(table, source, retained, index);
        assert_eq!(trie.get(&key).unwrap(), Some(value.clone()));
        selected.push((key, Some(value)));
      }
      let absent = match table {
        Table::Positions => key(table, source, retained, count),
        Table::Indices => Indices::hashed_key_for(
          source,
          DependencyRegistrationPosition {
            page: retained[pages as usize / 2],
            slot: 0,
          },
        )[56..]
          .to_vec(),
      };
      assert_eq!(trie.get(&absent).unwrap(), None);
      selected.push((absent, None));
      drop(trie);
      records.extend(recorder.drain());
      queries.push((root, selected));
    }
    println!("source={source} roots_checked=2");
  }
  // Only strict descendants are invariant under complete source-prefix embedding.
  let descendants = StorageProof::new(
    records
      .iter() // deos-bypass: bounded-iter -- fixed eight typed tables and 63 reads per table
      .filter(|record| !roots.contains(&record.hash))
      .map(|record| record.data.clone()),
  );
  let required_node_bytes: usize = descendants.iter_nodes().map(Vec::len).sum();
  let proof = StorageProof::new(records.into_iter().map(|record| record.data));
  println!(
    "forest_read_proof_bytes={} required_descendant_node_bytes={required_node_bytes} exceeds_control={}",
    proof.encoded_size(),
    required_node_bytes as u64 > limit
  );
  let proof_database = proof.into_memory_db::<Blake2Hasher>();
  for (root, selected) in queries {
    let trie = TrieDBBuilder::<Layout>::new(&proof_database, &root).build();
    for (key, expected) in selected {
      assert_eq!(trie.get(&key).unwrap(), expected);
    }
  }
  println!(
    "All eight read roots replayed; public history, complete Close and compact/compressed PoV remain separate obligations."
  );
}

fn main() {
  let mut args = std::env::args().skip(1);
  match (args.next(), args.next()) {
    (Some(arg), None) if matches!(arg.as_str(), "--help" | "-h") => println!(
      "Four-watch fixed hash-selected retirement proof diagnostic.\nGenerate pages with actor_retention_pages.py OUTPUT, then pass that JSON path.\nPopulation and five-nibble selection are fixed. This is a native paired suffix forest, not public Actor execution, compact/compressed PoV or a production Weight."
    ),
    (Some(path), None) => run(&path),
    _ => {
      eprintln!("Pass one generated JSON path or --help.");
      std::process::exit(2);
    }
  }
}
