use codec::Encode;
use deos_runtime::Runtime;
use pallet_deos_actors::{
  Config, DependencyRegistrationFreeIndices, DependencyRegistrationFreePositions,
  DependencyRegistrationPosition,
};
use polkadot_sdk::{
  sp_core::{Blake2Hasher, H256},
  sp_trie::{
    LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder, TrieLayout,
    delta_trie_root,
  },
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

fn position(source: u64, index: u64) -> DependencyRegistrationPosition {
  DependencyRegistrationPosition {
    page: (index / 31) * (source + 1),
    slot: (index % 31 + 1) as u8,
  }
}

fn key(table: Table, source: u64, index: u64) -> Vec<u8> {
  let full = match table {
    Table::Positions => Positions::hashed_key_for(source, index),
    Table::Indices => Indices::hashed_key_for(source, position(source, index)),
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

fn rows(table: Table, source: u64, count: u64, local: u64, after: bool) -> Vec<(Vec<u8>, Vec<u8>)> {
  let mut rows = (0..count)
    .filter_map(|index| {
      let (index, value) = match table {
        Table::Positions => {
          if after && index >= count - 31 {
            return None;
          }
          let value_index = if after && (local..local + 31).contains(&index) {
            count - 1 - (index - local)
          } else {
            index
          };
          (index, position(source, value_index).encode())
        }
        Table::Indices => {
          if after && (local..local + 31).contains(&index) {
            return None;
          }
          let value_index = if after && index >= count - 31 {
            local + count - 1 - index
          } else {
            index
          };
          (index, value_index.encode())
        }
      };
      Some((key(table, source, index), value))
    })
    .collect::<Vec<_>>();
  rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
  assert!(rows.windows(2).all(|pair| pair[0].0 != pair[1].0));
  rows
}

fn run() {
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  // Reserve 32 transient identities for a C32 append/close cohort. Each retained page has
  // one live slot-zero member and 31 holes. Different monotone page strides avoid assuming
  // identical source subtries; source-only empty-page churn can change those frontiers.
  let pages = u64::from(<Runtime as Config>::MaxActiveActors::get()) - 32;
  let watches = u64::from(<Runtime as Config>::MaxParkedBalanceWatches::get());
  assert!(pages > 2);
  let count = 31 * pages;
  let local = 31 * (pages / 2);
  let limit = <Runtime as Config>::BlockResourceBudget::get()
    .limits()
    .actor_control()
    .proof_size();
  println!(
    "scope=typed_hole_suffix_forest pages={pages} watches={watches} holes_per_source={count} control_proof_limit={limit}"
  );

  let mut database = MemoryDB::<Blake2Hasher>::default();
  let mut recorder = Recorder::<Layout>::new();
  let mut read_records = Vec::new();
  let mut root_records = Vec::new();
  let mut roots = BTreeSet::<H256>::new();
  let mut transitions = Vec::new();
  for source in 0..watches {
    for table in [Table::Positions, Table::Indices] {
      let before = rows(table, source, count, local, false);
      let root = Layout::trie_build(&mut database, before);
      roots.insert(root);
      {
        let trie = TrieDBBuilder::<Layout>::new(&database, &root)
          .with_recorder(&mut recorder)
          .build();
        for index in (local..local + 31).chain(count - 31..count) {
          let expected = match table {
            Table::Positions => position(source, index).encode(),
            Table::Indices => index.encode(),
          };
          assert_eq!(
            trie.get(&key(table, source, index)).unwrap(),
            Some(expected)
          );
        }
        let absent = match table {
          Table::Positions => key(table, source, count),
          Table::Indices => {
            let target = DependencyRegistrationPosition {
              page: (pages / 2) * (source + 1),
              slot: 0,
            };
            Indices::hashed_key_for(source, target)[56..].to_vec()
          }
        };
        assert_eq!(trie.get(&absent).unwrap(), None);
      }
      read_records.extend(recorder.drain());
      let delta = (0..31u64)
        .flat_map(|offset| {
          let target = local + offset;
          let tail = count - 1 - offset;
          match table {
            Table::Positions => [
              (
                key(table, source, target),
                Some(position(source, tail).encode()),
              ),
              (key(table, source, tail), None),
            ],
            Table::Indices => [
              (key(table, source, target), None),
              (key(table, source, tail), Some(target.encode())),
            ],
          }
        })
        .collect::<Vec<_>>();
      let post = delta_trie_root::<Layout, _, _, _, _, _>(
        &mut database,
        root,
        delta.clone(),
        Some(&mut recorder),
        None,
      )
      .unwrap();
      assert_eq!(
        post,
        Layout::trie_root(rows(table, source, count, local, true))
      );
      root_records.extend(recorder.drain());
      transitions.push((root, post, delta));
    }
    println!("source={source} roots_checked=2 page_stride={}", source + 1);
  }
  // A complete source-prefix node changes its partial key when embedded in the full chain
  // trie. Its strict descendants do not. Exclude every forest root before comparing the
  // unique required descendant payload with the budget; SCALE framing only adds bytes.
  let descendants = StorageProof::new(
    read_records
      .iter() // deos-bypass: bounded-iter -- fixed reference watch count and 63-key reads per typed table
      .filter(|record| !roots.contains(&record.hash))
      .map(|record| record.data.clone()),
  );
  let required_node_bytes: usize = descendants.iter_nodes().map(Vec::len).sum();
  let proof = StorageProof::new(
    read_records
      .into_iter()
      .chain(root_records)
      .map(|record| record.data),
  );
  println!(
    "forest_proof_bytes={} required_descendant_node_bytes={required_node_bytes} exceeds_control={}",
    proof.encoded_size(),
    required_node_bytes as u64 > limit,
  );
  let mut proof_database = proof.into_memory_db::<Blake2Hasher>();
  for (root, post, delta) in transitions {
    assert_eq!(
      delta_trie_root::<Layout, _, _, _, _, _>(&mut proof_database, root, delta, None, None)
        .unwrap(),
      post,
    );
  }
  println!(
    "All paired suffix roots replayed; public-state reachability and full Close remain separate obligations."
  );
}

fn main() {
  let mut args = std::env::args().skip(1);
  match (args.next(), args.next()) {
    (None, None) => run(),
    (Some(arg), None) if matches!(arg.as_str(), "--help" | "-h") => println!(
      "Fixed reference-configured typed-hole proof diagnostic. No population parameters.\nRun with SKIP_WASM_BUILD=1 cargo run --locked -p deos-runtime --example actor_hole_proof.\nThis measures a suffix forest, not admitted public Actor cleanup, RefTime or a production Weight."
    ),
    _ => {
      eprintln!("Only --help is supported; the diagnostic geometry is fixed.");
      std::process::exit(2);
    }
  }
}
