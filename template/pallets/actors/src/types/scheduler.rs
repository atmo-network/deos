use super::{contract::ActorContractCommitment, lifecycle::ActorId};
use frame::prelude::*;

pub type QueueTicket = u64;
pub type QueuePageId = u64;
pub type WakeupPageId = u64;
pub type WakeupSlot = u32;
pub type WakeupCursorIndex = u32;
pub type SchedulerTick = u64;

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum WakeupClock {
  #[default]
  Block,
  Tick,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum WakeupKey<BlockNumber> {
  Block(BlockNumber),
  Tick(SchedulerTick),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct WakeupWorkerFault<BlockNumber> {
  pub key: WakeupKey<BlockNumber>,
  pub page: WakeupPageId,
  pub class: super::observation::CrossingWorkerFaultClass,
}

impl<BlockNumber> WakeupKey<BlockNumber> {
  pub fn clock(&self) -> WakeupClock {
    match self {
      Self::Block(_) => WakeupClock::Block,
      Self::Tick(_) => WakeupClock::Tick,
    }
  }
}

/// Returns the last complete scheduler tick visible at `timestamp_millis`.
pub fn scheduler_tick_floor(timestamp_millis: u64, tick_millis: u64) -> Option<SchedulerTick> {
  (tick_millis > 0).then(|| timestamp_millis / tick_millis)
}

/// Returns the first scheduler tick whose boundary is not earlier than `timestamp_millis`.
pub fn scheduler_tick_ceil(timestamp_millis: u64, tick_millis: u64) -> Option<SchedulerTick> {
  if tick_millis == 0 {
    return None;
  }
  let quotient = timestamp_millis / tick_millis;
  if timestamp_millis.is_multiple_of(tick_millis) {
    Some(quotient)
  } else {
    quotient.checked_add(1)
  }
}

/// Anchors a newly admitted cadence so its first deadline is never earlier than one full period.
pub fn first_cadence_due_tick(
  timestamp_millis: u64,
  tick_millis: u64,
  every_ticks: SchedulerTick,
) -> Option<SchedulerTick> {
  if every_ticks == 0 {
    return None;
  }
  scheduler_tick_ceil(timestamp_millis, tick_millis)?.checked_add(every_ticks)
}

/// Returns the first cadence point strictly after `now_tick`, coalescing every missed period.
pub fn next_cadence_due_tick(
  anchor_tick: SchedulerTick,
  every_ticks: SchedulerTick,
  now_tick: SchedulerTick,
) -> Option<SchedulerTick> {
  if every_ticks == 0 {
    return None;
  }
  let first_due = anchor_tick.checked_add(every_ticks)?;
  let lower = now_tick.checked_add(1)?;
  if lower <= first_due {
    return Some(first_due);
  }
  let delta = lower.checked_sub(anchor_tick)?;
  let periods = delta.div_ceil(every_ticks);
  anchor_tick.checked_add(periods.checked_mul(every_ticks)?)
}

#[cfg(test)]
mod cadence_tick_tests {
  use super::{
    first_cadence_due_tick, next_cadence_due_tick, scheduler_tick_ceil, scheduler_tick_floor,
  };

  const TICK_MILLIS: u64 = 500;
  const FEE_SINK_PERIOD_TICKS: u64 = 120;

  #[test]
  fn tick_quantization_floors_readiness_and_ceils_activation() {
    for (timestamp, floor, ceil) in [(0, 0, 0), (1, 0, 1), (499, 0, 1), (500, 1, 1), (501, 1, 2)] {
      assert_eq!(scheduler_tick_floor(timestamp, TICK_MILLIS), Some(floor));
      assert_eq!(scheduler_tick_ceil(timestamp, TICK_MILLIS), Some(ceil));
    }
    assert_eq!(scheduler_tick_floor(1, 0), None);
    assert_eq!(scheduler_tick_ceil(1, 0), None);
  }

  #[test]
  fn fee_sink_first_deadline_never_shortens_sixty_seconds() {
    for timestamp in [0, 1, 499, 500, 501] {
      let due_tick = first_cadence_due_tick(timestamp, TICK_MILLIS, FEE_SINK_PERIOD_TICKS)
        .expect("valid cadence arithmetic");
      let due_millis = due_tick * TICK_MILLIS;
      assert!(due_millis - timestamp >= 60_000);
      assert!(
        scheduler_tick_floor(due_millis - 1, TICK_MILLIS).expect("nonzero tick duration")
          < due_tick
      );
      assert_eq!(
        scheduler_tick_floor(due_millis, TICK_MILLIS),
        Some(due_tick)
      );
    }
  }

  #[test]
  fn delayed_cadence_coalesces_missed_periods_without_catch_up() {
    assert_eq!(next_cadence_due_tick(1, 120, 1), Some(121));
    assert_eq!(next_cadence_due_tick(1, 120, 120), Some(121));
    assert_eq!(next_cadence_due_tick(1, 120, 121), Some(241));
    assert_eq!(next_cadence_due_tick(1, 120, 500), Some(601));
    assert_eq!(next_cadence_due_tick(1, 0, 1), None);
    assert_eq!(next_cadence_due_tick(u64::MAX, 1, u64::MAX), None);
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum IdleStarvationPhase {
  #[default]
  Healthy,
  Starving {
    consecutive_blocks: u32,
  },
  Alerted {
    consecutive_blocks: u32,
  },
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupPointer<BlockNumber> {
  pub block: WakeupKey<BlockNumber>,
  pub page_id: WakeupPageId,
  pub slot: WakeupSlot,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct TriggerWakeupPointer {
  pub tick: SchedulerTick,
  pub page_id: WakeupPageId,
  pub slot: WakeupSlot,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupEntry {
  pub actor_id: ActorId,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupPage<Entries> {
  pub entries: Entries,
  pub live_entries: u32,
  pub scan_slot: WakeupSlot,
  pub previous_page: Option<WakeupPageId>,
  pub next_page: Option<WakeupPageId>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupBucketState {
  pub head_page: WakeupPageId,
  pub tail_page: WakeupPageId,
  pub next_page_id: WakeupPageId,
  pub live_entries: u32,
  pub cursor_index: Option<WakeupCursorIndex>,
}

pub type QueueEntry<BlockNumber> = ActorStepTicket<BlockNumber, ActorContractCommitment<[u8; 32]>>;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorStepTicket<BlockNumber, ContractCommitment> {
  pub actor_id: ActorId,
  pub cycle_nonce: u64,
  pub cursor: u32,
  pub ticket: QueueTicket,
  pub eligible_at: BlockNumber,
  pub contract_commitment: ContractCommitment,
}

impl<BlockNumber: PartialEq, ContractCommitment: PartialEq>
  ActorStepTicket<BlockNumber, ContractCommitment>
{
  pub fn matches(
    &self,
    actor_id: ActorId,
    cycle_nonce: u64,
    cursor: u32,
    ticket: QueueTicket,
    eligible_at: &BlockNumber,
    contract_commitment: &ContractCommitment,
  ) -> bool {
    self.actor_id == actor_id
      && self.cycle_nonce == cycle_nonce
      && self.cursor == cursor
      && self.ticket == ticket
      && &self.eligible_at == eligible_at
      && &self.contract_commitment == contract_commitment
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueDrainStats {
  pub entries_scanned: u32,
  pub tombstones_skipped: u32,
  pub pages_touched: u32,
  pub pages_deleted: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WakeupDrainStats {
  pub entries_scanned: u32,
  pub ready_entries: u32,
  pub stale_entries: u32,
  pub pages_touched: u32,
  pub pages_deleted: u32,
}
