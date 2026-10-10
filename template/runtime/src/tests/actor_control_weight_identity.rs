use super::*;

type ControlWeights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;

#[test]
fn control_weight_identity_binds_every_consumed_user_opening_profile() {
  let maximum_tail_chunks = ActorMaxContractSteps::get()
    .saturating_sub(1)
    .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
  let profiles = (0..=maximum_tail_chunks)
    .map(|tails| {
      let expected = if tails == 0 {
        ControlWeights::scheduler_inner_opening_user_complete_header_max()
      } else {
        ControlWeights::scheduler_inner_opening_user_complete_header_max_tail(tails)
      };
      assert_eq!(
        RuntimeStepControlWeight::user_opening_control_weight(tails),
        expected
      );
      expected
    })
    .collect::<alloc::vec::Vec<_>>();
  let identity = RuntimeStepControlWeight::production_weight_identity().unwrap();
  assert_eq!(
    RuntimeStepControlWeight::production_weight_identity_from_user_opening(&profiles),
    Some(identity),
  );
  for index in 0..profiles.len() {
    for delta in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let mut changed = profiles.clone();
      changed[index] = changed[index].checked_add(&delta).unwrap();
      assert_ne!(
        RuntimeStepControlWeight::production_weight_identity_from_user_opening(&changed),
        Some(identity),
        "User Opening profile {index} must bind each Weight dimension ({delta:?})",
      );
    }
  }
}
