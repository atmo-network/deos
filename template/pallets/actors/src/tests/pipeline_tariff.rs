use super::*;
use crate::{
  PipelineControlWeightProvider, tariff::PipelineTariffCandidateError as CandidateError,
};

struct ForbiddenPricing;
impl PipelineControlWeightProvider<crate::ActorContractOf<Test>> for ForbiddenPricing {
  fn production_weight_identity() -> [u8; 32] {
    panic!("System catalog must not be queried")
  }
  fn maximum_pipeline_control_weight(
    _: &crate::ActorContractOf<Test>,
  ) -> Option<polkadot_sdk::sp_weights::Weight> {
    panic!("System tariff must not be queried")
  }
}
impl polkadot_sdk::sp_weights::WeightToFee for ForbiddenPricing {
  type Balance = Balance;
  fn weight_to_fee(_: &polkadot_sdk::sp_weights::Weight) -> Balance {
    panic!("Missing/System tariffs must not be converted")
  }
}

#[test]
fn candidate_identity_binds_every_physical_authority_field_without_rebinding_live_authority() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let contract = user_active_contract(manual_schedule(), None, Default::default()).unwrap();
    let quote = || {
      Actors::benchmark_pipeline_tariff_candidate::<ForbiddenPricing, ForbiddenPricing>(
        ActorType::System,
        Mutability::Mutable,
        &contract,
      )
      .unwrap()
    };
    let authority =
      <MockAdmissionCertificateAuthority as crate::AdmissionCertificateAuthorityProvider>::current(
      )
      .unwrap();
    let baseline = quote();
    for coordinate in 0..6 {
      let mut changed = authority;
      match coordinate {
        0 => changed.runtime_actor_semantics_version += 1,
        1 => changed.body_geometry_version += 1,
        2 => changed.production_weight_identity[0] ^= 1,
        3 => changed.configured_bounds_commitment[0] ^= 1,
        4 => {
          changed.maximum_lifecycle_weight = changed
            .maximum_lifecycle_weight
            .saturating_add(polkadot_sdk::sp_weights::Weight::from_parts(1, 0))
        }
        _ => {
          changed.maximum_lifecycle_weight = changed
            .maximum_lifecycle_weight
            .saturating_add(polkadot_sdk::sp_weights::Weight::from_parts(0, 1))
        }
      }
      set_admission_authority_override(Some(changed));
      assert_ne!(quote().candidate_identity, baseline.candidate_identity);
    }
    set_admission_authority_override(None);
    assert_eq!(quote(), baseline);
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<(), ForbiddenPricing>(
        ActorType::User,
        Mutability::Mutable,
        &contract
      ),
      Err(CandidateError::TariffUnavailable)
    );
  });
}

#[test]
fn candidate_reuses_kernel_shape_validation_before_querying_a_model() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let mut contract = user_active_contract(manual_schedule(), None, Default::default()).unwrap();
    contract.steps = contract_steps_with_step(RuntimeStep {
      precondition: None,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::RetryLater { max_attempts: 1 },
    });
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<ForbiddenPricing, ForbiddenPricing>(
        ActorType::System,
        Mutability::Mutable,
        &contract
      ),
      Err(CandidateError::InvalidContract(
        Error::<Test>::InvalidRetryAttemptLimit.into()
      ))
    );
    contract.steps[0].on_error = StepErrorPolicy::ContinueNextStep;
    let predicate = Predicate::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 1,
    };
    contract.steps[0].precondition = Some(Precondition {
      clauses: vec![vec![predicate, predicate].try_into().unwrap()]
        .try_into()
        .unwrap(),
    });
    let encoded = codec::Encode::encode(&contract);
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<ForbiddenPricing, ForbiddenPricing>(
        ActorType::System,
        Mutability::Mutable,
        &contract
      ),
      Err(CandidateError::NonCanonicalSteps)
    );
    assert_eq!(codec::Encode::encode(&contract), encoded);
    contract.steps[0].precondition = None;
    contract.trigger = Trigger::at_time(1);
    contract.cooldown_blocks = 1;
    assert_eq!(
      Actors::benchmark_pipeline_tariff_candidate::<ForbiddenPricing, ForbiddenPricing>(
        ActorType::System,
        Mutability::Mutable,
        &contract
      ),
      Err(CandidateError::InvalidContract(
        Error::<Test>::InvalidTriggerConfiguration.into()
      ))
    );
  });
}
