//! Runtime-only, raw diagnostics of Authorship and System initialization.
//! This wrapper is not a consensus pallet and supplies no production Weight coefficients.

use crate::{
  AccountId, Authorship, Balance, Balances, CollatorSelection, Runtime, RuntimeOrigin, System,
};
use codec::{Compact, Encode};
use polkadot_sdk::{
  frame_benchmarking::{BenchmarkError, v2::*},
  frame_executive::OnInitializeWithWeightRegistration,
  frame_support::{
    dispatch::DispatchClass,
    storage::storage_prefix,
    traits::{Currency, Get, Hooks},
  },
  frame_system,
  pallet_collator_selection::{self, WeightInfo as _},
  sp_runtime::{
    generic::{Digest, DigestItem},
    traits::Header as _,
  },
};

// FRAME's non-consensus System benchmark wrapper uses the same local Pallet/Config pattern.
pub struct Pallet<T: Config>(core::marker::PhantomData<T>);

pub trait Config: frame_system::Config {
  type Fixture;
  fn prepare(c: u32, d: u32, record_initialize: bool) -> Result<Self::Fixture, BenchmarkError>;
  fn execute(fixture: &Self::Fixture);
  fn verify(fixture: Self::Fixture);
}

pub struct Fixture {
  number: crate::BlockNumber,
  author: AccountId,
  pot: AccountId,
  pot_balance: Balance,
  reward: Balance,
  issuance: Balance,
  weight: frame_system::ConsumedWeight,
  digest_bytes: usize,
  initialization_header_bytes: usize,
  initialization: Option<Digest>,
}

fn largest_fitting(bound: u32, budget: u64, encoded_size: impl Fn(u32) -> u64) -> u32 {
  let (mut low, mut high) = (0, bound);
  while low < high {
    let middle = low + (high - low).div_ceil(2);
    if encoded_size(middle) <= budget {
      low = middle;
    } else {
      high = middle - 1;
    }
  }
  low
}

fn prepare_digest(d: u32, slot: u64, budget: u32) -> Result<Digest, BenchmarkError> {
  let aura = DigestItem::PreRuntime(
    polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
    slot.encode(),
  );
  let foreign = DigestItem::PreRuntime(*b"test", alloc::vec::Vec::new());
  let budget = u64::from(budget);
  let aura_bytes = aura.encoded_size() as u64;
  let foreign_bytes = foreign.encoded_size() as u64;
  let mut logs = alloc::vec::Vec::new();
  match d {
    0 => (),
    1 => {
      let fixed_bytes = Compact(2u32).encoded_size() as u64 + aura_bytes + foreign_bytes
        - Compact(0u32).encoded_size() as u64;
      let size =
        |payload: u32| fixed_bytes + Compact(payload).encoded_size() as u64 + u64::from(payload);
      if size(0) > budget {
        return Err(BenchmarkError::Stop(
          "Authorship digest budget is too small",
        ));
      }
      let payload = largest_fitting(budget as u32, budget, size);
      logs.push(DigestItem::PreRuntime(
        *b"test",
        alloc::vec![7; payload as usize],
      ));
    }
    2 => {
      let size = |count: u32| {
        Compact(count + 1).encoded_size() as u64 + u64::from(count) * foreign_bytes + aura_bytes
      };
      if size(0) > budget {
        return Err(BenchmarkError::Stop(
          "Authorship digest budget is too small",
        ));
      }
      let count = largest_fitting((budget / foreign_bytes) as u32, budget, size);
      // Setup only: the existing header byte ceiling bounds this vector's item count.
      logs.resize(count as usize, foreign);
    }
    _ => {
      return Err(BenchmarkError::Stop(
        "Authorship digest selector must be 0, 1 or 2",
      ));
    }
  }
  logs.push(aura);
  let digest = Digest { logs };
  if digest.encoded_size() as u64 > budget {
    return Err(BenchmarkError::Stop(
      "Authorship digest exceeds initialization budget",
    ));
  }
  Ok(digest)
}

pub(crate) fn owner_keys() -> [alloc::vec::Vec<u8>; 8] {
  // Do not inherit SDK exclusions for transient cache, ledger, digest, clock or event work.
  [
    (b"Authorship".as_slice(), b"Author".as_slice()),
    (b"System", b"BlockWeight"),
    (b"System", b"Digest"),
    (b"System", b"Number"),
    (b"System", b"Events"),
    (b"System", b"EventCount"),
    (b"System", b"ExecutionPhase"),
    (b"Balances", b"TotalIssuance"),
  ]
  .map(|(pallet, item)| storage_prefix(pallet, item).to_vec())
}

pub(crate) fn initialization_owner_keys() -> [alloc::vec::Vec<u8>; 3] {
  use polkadot_sdk::sp_core::storage::well_known_keys;
  [
    storage_prefix(b"System", b"BlockSize").to_vec(),
    well_known_keys::EXTRINSIC_INDEX.to_vec(),
    well_known_keys::INTRABLOCK_ENTROPY.to_vec(),
  ]
}

impl Config for Runtime {
  type Fixture = Fixture;

  fn prepare(c: u32, d: u32, record_initialize: bool) -> Result<Fixture, BenchmarkError> {
    if c > 1 || d > 2 {
      return Err(BenchmarkError::Stop(
        "Authorship selectors must be c=0..1 and d=0..2",
      ));
    }
    // Reuse the reference host's bounded trusted-roster fixture, not a new consensus port.
    <crate::configs::RuntimeSessionRotation as pallet_session_rotation::BenchmarkHelper>::prepare_rotation();
    let validators = polkadot_sdk::pallet_session::Validators::<Runtime>::get();
    let bound: u32 = <Runtime as polkadot_sdk::pallet_aura::Config>::MaxAuthorities::get();
    assert_eq!(validators.len(), bound as usize);
    assert_eq!(
      polkadot_sdk::pallet_aura::Authorities::<Runtime>::decode_len(),
      Some(bound as usize)
    );
    let author = validators
      .last()
      .cloned()
      .ok_or(BenchmarkError::Stop("Authorship needs a validator"))?;
    // Keep the SDK runner's minimum-number normalization from changing this input.
    if record_initialize && System::block_number() == 0 {
      System::set_block_number(1);
    }
    let number = System::block_number()
      .checked_add(1)
      .ok_or(BenchmarkError::Stop("Authorship block number overflow"))?;
    let empty_header_bytes = crate::Header::new(
      number,
      Default::default(),
      Default::default(),
      Default::default(),
      Digest::default(),
    )
    .encoded_size();
    let header_limit = crate::configs::RuntimeBlockLength::get()
      .max_header_size()
      .min(polkadot_sdk::cumulus_primitives_core::relay_chain::MAX_HEAD_DATA_SIZE);
    let budget =
      header_limit
        .checked_sub(empty_header_bytes as u32)
        .ok_or(BenchmarkError::Stop(
          "Authorship empty header exceeds the existing ceiling",
        ))?;
    // Saturates an initialization-input superset, not a complete sealed-block allowance.
    let digest = prepare_digest(d, u64::from(bound - 1), budget)?;
    let digest_bytes = digest.encoded_size();
    let initialization_header_bytes = empty_header_bytes + digest_bytes;
    let initialization = if record_initialize {
      Some(digest)
    } else {
      System::initialize(&number, &Default::default(), &digest);
      None
    };
    polkadot_sdk::sp_io::storage::clear(&owner_keys()[0]);
    pallet_collator_selection::LastAuthoredBlock::<Runtime>::remove(&author);
    assert!(!frame_system::Account::<Runtime>::contains_key(&author));
    let pot = CollatorSelection::account_id();
    assert_ne!(pot, author);
    let ed = <Balances as Currency<AccountId>>::minimum_balance();
    assert!(ed > 0);
    let pot_balance = if c == 0 {
      0
    } else {
      ed.checked_mul(4)
        .ok_or(BenchmarkError::Stop("Authorship pot overflow"))?
    };
    Balances::force_set_balance(RuntimeOrigin::root(), pot.clone().into(), pot_balance)?;
    System::reset_events();
    let reward = pot_balance.saturating_sub(ed) / 2;
    for key in owner_keys() {
      polkadot_sdk::frame_benchmarking::benchmarking::remove_from_whitelist(key);
    }
    if record_initialize {
      for key in initialization_owner_keys() {
        polkadot_sdk::frame_benchmarking::benchmarking::remove_from_whitelist(key);
      }
    }
    Ok(Fixture {
      number,
      author,
      pot,
      pot_balance,
      reward,
      issuance: Balances::total_issuance(),
      weight: if record_initialize {
        Default::default()
      } else {
        System::block_weight()
      },
      digest_bytes,
      initialization_header_bytes,
      initialization,
    })
  }

  fn execute(fixture: &Fixture) {
    if let Some(digest) = &fixture.initialization {
      System::initialize(&fixture.number, &Default::default(), digest);
    }
    // The actual Executive wrapper also registers the hook's zero return. The callback
    // registers its nonzero declaration internally; finalization is not a second debit.
    let _ = <(Authorship,) as OnInitializeWithWeightRegistration<Runtime>>::on_initialize_with_weight_registration(fixture.number);
    Authorship::on_finalize(fixture.number);
  }

  fn verify(fixture: Fixture) {
    let whitelist = polkadot_sdk::frame_benchmarking::benchmarking::get_whitelist();
    for key in owner_keys() {
      assert!(!whitelist.iter().any(|entry| entry.key == key));
    }
    if fixture.initialization.is_some() {
      for key in initialization_owner_keys() {
        assert!(!whitelist.iter().any(|entry| entry.key == key));
      }
    }
    assert!(!polkadot_sdk::sp_io::storage::exists(&owner_keys()[0]));
    assert_eq!(System::digest().encoded_size(), fixture.digest_bytes);
    assert_eq!(
      System::block_size() as usize,
      fixture.initialization_header_bytes
    );
    assert_eq!(
      pallet_collator_selection::LastAuthoredBlock::<Runtime>::get(&fixture.author),
      fixture.number
    );
    assert_eq!(Balances::free_balance(&fixture.author), fixture.reward);
    assert_eq!(
      Balances::free_balance(&fixture.pot),
      fixture.pot_balance - fixture.reward
    );
    assert_eq!(Balances::total_issuance(), fixture.issuance);
    let mut expected = fixture.weight;
    expected.accrue(
      <Runtime as pallet_collator_selection::Config>::WeightInfo::note_author(),
      DispatchClass::Mandatory,
    );
    assert_eq!(System::block_weight(), expected);
    let transfers = System::events().into_iter().filter(|record| matches!(
      &record.event,
      crate::RuntimeEvent::Balances(polkadot_sdk::pallet_balances::Event::Transfer { from, to, amount })
        if from == &fixture.pot && to == &fixture.author && amount == &fixture.reward
    )).count();
    assert_eq!(transfers, usize::from(fixture.reward != 0));
  }
}

#[benchmarks]
pub(crate) mod cases {
  use super::*;

  /// Categorical: c selects pot state; d selects single/payload/count digest geometry.
  #[benchmark(extra, pov_mode = Measured)]
  fn authorship_complete(c: Linear<0, 1>, d: Linear<0, 2>) -> Result<(), BenchmarkError> {
    let fixture = T::prepare(c, d, false)?;

    #[block]
    {
      T::execute(&fixture);
    }

    T::verify(fixture);
    Ok(())
  }

  /// Wider categorical diagnostic: generated Digest is written after the SDK setup commit.
  #[benchmark(extra, pov_mode = Measured)]
  fn authorship_with_initialize(c: Linear<0, 1>, d: Linear<0, 2>) -> Result<(), BenchmarkError> {
    let fixture = T::prepare(c, d, true)?;

    #[block]
    {
      T::execute(&fixture);
    }

    T::verify(fixture);
    Ok(())
  }

  /// Wider raw diagnostic: no whitelist prefill and root materialization before Recording stops.
  #[benchmark(extra, pov_mode = Measured)]
  fn authorship_initialize_root(c: Linear<0, 1>, d: Linear<0, 2>) -> Result<(), BenchmarkError> {
    let fixture = T::prepare(c, d, true)?;
    polkadot_sdk::frame_benchmarking::benchmarking::set_whitelist(alloc::vec::Vec::new());

    #[block]
    {
      T::execute(&fixture);
      let _ = polkadot_sdk::sp_io::storage::root(T::Version::get().state_version());
    }

    assert!(polkadot_sdk::frame_benchmarking::benchmarking::get_whitelist().is_empty());
    T::verify(fixture);
    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn native_root_instance(
    c: u32,
    d: u32,
    verify: bool,
    recording: &mut impl polkadot_sdk::frame_benchmarking::Recording,
  ) -> Result<(), BenchmarkError> {
    use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
    <SelectedBenchmark as BenchmarkingSetup<Runtime>>::instance(
      &SelectedBenchmark::authorship_initialize_root,
      recording,
      &[(BenchmarkParameter::c, c), (BenchmarkParameter::d, d)],
      verify,
    )
  }

  #[cfg(test)]
  pub(crate) fn native_initialize_instance(
    c: u32,
    d: u32,
    verify: bool,
    recording: &mut impl polkadot_sdk::frame_benchmarking::Recording,
  ) -> Result<(), BenchmarkError> {
    use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
    <SelectedBenchmark as BenchmarkingSetup<Runtime>>::instance(
      &SelectedBenchmark::authorship_with_initialize,
      recording,
      &[(BenchmarkParameter::c, c), (BenchmarkParameter::d, d)],
      verify,
    )
  }

  #[cfg(test)]
  pub(crate) fn native_instance(
    c: u32,
    d: u32,
    verify: bool,
    recording: &mut impl polkadot_sdk::frame_benchmarking::Recording,
  ) -> Result<(), BenchmarkError> {
    use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, BenchmarkingSetup};
    <SelectedBenchmark as BenchmarkingSetup<Runtime>>::instance(
      &SelectedBenchmark::authorship_complete,
      recording,
      &[(BenchmarkParameter::c, c), (BenchmarkParameter::d, d)],
      verify,
    )
  }
}
