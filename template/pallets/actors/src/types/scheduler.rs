use super::lifecycle::ActorId;
use frame::prelude::*;

pub type QueueTicket = u64;
pub type QueuePageId = u64;
pub type WakeupPageId = u64;
pub type WakeupSlot = u32;
pub type WakeupCursorIndex = u32;

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
  pub block: BlockNumber,
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

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct QueueEntry {
  pub ticket: QueueTicket,
  pub actor_id: ActorId,
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
