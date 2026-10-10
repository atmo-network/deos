use super::*;
use pallet_deos_actors::tariff::PipelineTariffCandidateError as CandidateError;
use pallet_deos_actors::{ActorType, Mutability, PipelineControlWeightProvider};
use polkadot_sdk::sp_weights::WeightToFee;
use std::cell::Cell;

type Contract = pallet_deos_actors::ActorContractOf<Runtime>;

std::thread_local! {
  // Test observers only, not chain state or a reference pricing model.
  static CALLS: Cell<(u32, u32)> = const { Cell::new((0, 0)) };
}

struct ModelConverter<const RATE: u32>;
impl<const RATE: u32> ModelConverter<RATE> {
  const OFFSET: Balance = 1;
  const PROOF_RATE: Balance = 3;
}
impl<const RATE: u32> WeightToFee for ModelConverter<RATE> {
  type Balance = Balance;
  fn weight_to_fee(weight: &Weight) -> Balance {
    CALLS.with(|calls| {
      let (p, c) = calls.get();
      calls.set((p, c + 1));
    });
    Self::OFFSET
      + Balance::from(weight.ref_time()) * Balance::from(RATE)
      + Balance::from(weight.proof_size()) * Self::PROOF_RATE
  }
}

struct ModelDeclaration<const EPOCH: u32, const RATE: u32>;
impl<const EPOCH: u32, const RATE: u32> ModelDeclaration<EPOCH, RATE> {
  const REF_BASE: u64 = 37;
  const REF_PER_STEP: u64 = 5;
  const PROOF_BASE: u64 = 11;
  const PROOF_PER_POLICY: u64 = 1;
  const MAX_STEPS: usize = 3;
}
impl<const EPOCH: u32, const RATE: u32> PipelineControlWeightProvider<Contract>
  for ModelDeclaration<EPOCH, RATE>
{
  fn production_weight_identity() -> [u8; 32] {
    // RATE commits exactly the explicitly paired ModelConverter, including its offset/proof term.
    (
      b"CANDIDATE_NATIVE_MODEL_ONLY_V1",
      EPOCH,
      (
        RATE,
        ModelConverter::<RATE>::OFFSET,
        ModelConverter::<RATE>::PROOF_RATE,
      ),
      (
        Self::REF_BASE,
        Self::REF_PER_STEP,
        Self::PROOF_BASE,
        Self::PROOF_PER_POLICY,
        Self::MAX_STEPS as u32,
      ),
      NATIVE_ASSET,
      b"close-only-zero-to-three-steps-policy-syntax",
    )
      .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256)
  }
  fn maximum_pipeline_control_weight(contract: &Contract) -> Option<Weight> {
    CALLS.with(|calls| {
      let (p, c) = calls.get();
      calls.set((p + 1, c));
    });
    (contract.steps.len() <= Self::MAX_STEPS).then(|| {
      Weight::from_parts(
        Self::REF_BASE + Self::REF_PER_STEP * contract.steps.len() as u64,
        Self::PROOF_BASE + Self::PROOF_PER_POLICY * u64::from(contract.close_when.is_some()),
      )
    })
  }
}

struct ForbiddenPricing;
impl PipelineControlWeightProvider<Contract> for ForbiddenPricing {
  fn production_weight_identity() -> [u8; 32] {
    panic!("System must not need the User catalog")
  }
  fn maximum_pipeline_control_weight(_: &Contract) -> Option<Weight> {
    panic!("System must not query a User price")
  }
}
impl WeightToFee for ForbiddenPricing {
  type Balance = Balance;
  fn weight_to_fee(_: &Weight) -> Balance {
    panic!("System must not invoke User conversion")
  }
}

fn empty() -> Contract {
  active_contract(pallet_deos_actors::Trigger::manual(), 1, alloc::vec![])
}
fn stop() -> Contract {
  active_contract(
    pallet_deos_actors::Trigger::manual(),
    1,
    alloc::vec![step(
      pallet_deos_actors::Task::StopCycle,
      pallet_deos_actors::StepErrorPolicy::ContinueNextStep
    )],
  )
}
fn policy() -> Contract {
  let mut contract = stop();
  contract.close_when = Some(pallet_deos_actors::CloseWhen {
    clauses: alloc::vec![
      alloc::vec![pallet_deos_actors::BehaviorAtom {
        metric: pallet_deos_actors::BehaviorMetric::CycleCompletedTotal,
        threshold: 1
      }]
      .try_into()
      .unwrap()
    ]
    .try_into()
    .unwrap(),
  });
  contract
}
fn root() -> alloc::vec::Vec<u8> {
  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1)
}
fn quote<const EPOCH: u32, const RATE: u32>(
  contract: &Contract,
) -> pallet_deos_actors::tariff::PipelineTariffCandidate<Balance> {
  Actors::benchmark_pipeline_tariff_candidate::<ModelDeclaration<EPOCH, RATE>, ModelConverter<RATE>>(ActorType::User, Mutability::Mutable, contract).unwrap()
}

#[test]
fn complete_owner_converts_once_without_extra_common_task_or_destruction_cost() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let mut transfer = stop();
    transfer.steps = alloc::vec![
      step(
        pallet_deos_actors::Task::Transfer {
          to: BOB,
          asset: NATIVE_ASSET,
          amount: pallet_deos_actors::AmountResolution::Fixed(1)
        },
        pallet_deos_actors::StepErrorPolicy::ContinueNextStep
      ),
      transfer.steps[0].clone()
    ]
    .try_into()
    .unwrap();
    let mut zero_policy = policy();
    zero_policy.steps = Default::default();
    for contract in [empty(), stop(), transfer, policy(), zero_policy] {
      let before = root();
      let encoded = contract.encode();
      CALLS.with(|calls| calls.set((0, 0)));
      let candidate = quote::<1, 1>(&contract);
      let expected = Weight::from_parts(
        37 + 5 * contract.steps.len() as u64,
        11 + u64::from(contract.close_when.is_some()),
      );
      assert_eq!(candidate.control_weight, expected);
      assert_eq!(
        candidate.pipeline_machine_fee,
        1 + Balance::from(expected.ref_time()) + Balance::from(expected.proof_size()) * 3
      );
      assert_eq!(CALLS.with(Cell::get), (1, 1));
      assert_eq!(contract.encode(), encoded);
      assert_eq!(root(), before);
    }
    let contract = policy();
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        Some(contract)
      ),
      pallet_deos_actors::Error::<Runtime>::BehaviorPolicyUnavailable
    );
  });
}

#[test]
fn missing_invalid_and_wait_sources_never_become_insolvency_or_fallback() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let contract = empty();
    let before = root();
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<(), ModelConverter<1>>(
        ActorType::User,
        Mutability::Mutable,
        &contract
      ),
      Err(CandidateError::TariffUnavailable)
    );
    let mut unsupported = stop();
    unsupported.steps = alloc::vec![unsupported.steps[0].clone(); 4]
      .try_into()
      .unwrap();
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<ModelDeclaration<1, 1>, ModelConverter<1>>(
        ActorType::User,
        Mutability::Mutable,
        &unsupported
      ),
      Err(CandidateError::TariffUnavailable)
    );
    let mut waiting = contract.clone();
    waiting.on_unfunded = pallet_deos_actors::OnUnfunded::AwaitFunding;
    CALLS.with(|calls| calls.set((0, 0)));
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<ModelDeclaration<1, 1>, ModelConverter<1>>(
        ActorType::User,
        Mutability::Mutable,
        &waiting
      ),
      Err(CandidateError::UnsupportedUnfundedPolicy)
    );
    let mut invalid = policy();
    invalid.close_when.as_mut().unwrap().clauses[0][0].threshold = 0;
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<ModelDeclaration<1, 1>, ModelConverter<1>>(
        ActorType::User,
        Mutability::Mutable,
        &invalid
      ),
      Err(CandidateError::InvalidContract(
        pallet_deos_actors::Error::<Runtime>::InvalidCloseWhen.into()
      ))
    );
    assert_eq!(CALLS.with(Cell::get), (0, 0));
    assert_eq!(root(), before);
    let candidate =
      Actors::benchmark_pipeline_tariff_candidate::<ForbiddenPricing, ForbiddenPricing>(
        ActorType::System,
        Mutability::Mutable,
        &contract,
      )
      .unwrap();
    assert_eq!(candidate.control_weight, Weight::zero());
    assert_eq!(candidate.pipeline_machine_fee, 0);
    assert_eq!(root(), before);
  });
}

#[test]
fn identity_binds_real_conversion_catalog_full_source_and_class_not_live_balances() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let contract = stop();
    let before = root();
    let first = quote::<1, 1>(&contract);
    let rate = quote::<1, 2>(&contract);
    let epoch = quote::<2, 1>(&contract);
    assert_eq!(first.control_weight, rate.control_weight);
    assert_eq!(first.control_weight, epoch.control_weight);
    assert_ne!(first.pipeline_machine_fee, rate.pipeline_machine_fee);
    assert_eq!(first.pipeline_machine_fee, epoch.pipeline_machine_fee);
    assert_ne!(first.candidate_identity, rate.candidate_identity);
    assert_ne!(first.candidate_identity, epoch.candidate_identity);
    let mut header = contract.clone();
    header.funding = pallet_deos_actors::FundingSourcePolicy::OwnerOnly;
    let changed_header = quote::<1, 1>(&header);
    assert_eq!(first.pipeline_machine_fee, changed_header.pipeline_machine_fee);
    assert_ne!(first.candidate_identity, changed_header.candidate_identity);
    let immutable = Actors::benchmark_pipeline_tariff_candidate::<ModelDeclaration<1, 1>, ModelConverter<1>>(ActorType::User, Mutability::Immutable, &contract).unwrap();
    assert_ne!(first.candidate_identity, immutable.candidate_identity);
    assert_eq!(root(), before);
    assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(ALICE), BOB, 1));
    System::set_block_number(2);
    let changed_state = root();
    assert_eq!(first, quote::<1, 1>(&contract));
    assert_eq!(root(), changed_state);
    let authority = <FixtureAdmissionCertificateAuthority as pallet_deos_actors::AdmissionCertificateAuthorityProvider>::current().unwrap();
    assert_eq!(authority.production_weight_identity, pallet_deos_actors::AdmissionCertificateAuthority::compose_production_weight_identity(
      <FixtureStepControlWeight as pallet_deos_actors::StepControlWeightProvider<pallet_deos_actors::StepOf<Runtime>>>::production_weight_identity().unwrap(),
      <FixtureTaskEffectWeight as pallet_deos_actors::TaskEffectWeightProvider<pallet_deos_actors::TaskOf<Runtime>>>::production_weight_identity().unwrap(),
    ));
  });
}

#[test]
fn infallible_converter_boundary_accepts_exact_maxima_without_invented_overflow_guards() {
  struct MaximumModel;
  impl PipelineControlWeightProvider<Contract> for MaximumModel {
    fn production_weight_identity() -> [u8; 32] {
      polkadot_sdk::sp_io::hashing::blake2_256(
        b"NATIVE_MODEL_ONLY_MAX_WEIGHT_AND_MAX_BALANCE_CONVERTER",
      )
    }
    fn maximum_pipeline_control_weight(_: &Contract) -> Option<Weight> {
      Some(Weight::MAX)
    }
  }
  impl WeightToFee for MaximumModel {
    type Balance = Balance;
    fn weight_to_fee(_: &Weight) -> Balance {
      Balance::MAX
    }
  }
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let candidate = Actors::benchmark_pipeline_tariff_candidate::<MaximumModel, MaximumModel>(
      ActorType::User,
      Mutability::Mutable,
      &empty(),
    )
    .unwrap();
    assert_eq!(candidate.control_weight, Weight::MAX);
    assert_eq!(candidate.pipeline_machine_fee, Balance::MAX);
    // Not monetary capacity, physical admission, or proof that an arbitrary converter cannot saturate.
  });
}
