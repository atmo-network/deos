use super::ObservationValue;
use super::lifecycle::ActorId;
use frame::prelude::*;

pub type ObservationRevision = u64;

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub enum CrossingTraversal {
  Upward,
  Downward,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub enum CrossingMembershipRole {
  Fire,
  Rearm,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub struct CrossingLeafKey<FeedId> {
  pub feed: FeedId,
  pub traversal: CrossingTraversal,
  pub threshold: ObservationValue,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub struct CrossingRadixNodeKey<FeedId> {
  pub feed: FeedId,
  pub traversal: CrossingTraversal,
  pub depth: u8,
  pub prefix: ObservationValue,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingLeafState {
  pub tail_page: u32,
  pub page_count: u32,
  pub member_count: u32,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingMember {
  pub actor_id: ActorId,
  pub generation: u64,
}

#[derive(
  polkadot_sdk::frame_support::CloneNoBound,
  polkadot_sdk::frame_support::DebugNoBound,
  polkadot_sdk::frame_support::PartialEqNoBound,
  polkadot_sdk::frame_support::EqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
  MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxEntries))]
pub struct CrossingMemberPage<MaxEntries: Get<u32>> {
  pub entries: BoundedVec<CrossingMember, MaxEntries>,
}

impl<MaxEntries: Get<u32>> Default for CrossingMemberPage<MaxEntries> {
  fn default() -> Self {
    Self {
      entries: BoundedVec::default(),
    }
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingMembershipLocator<FeedId> {
  pub key: CrossingLeafKey<FeedId>,
  pub page: u32,
  pub offset: u32,
  pub generation: u64,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingTransitionObligation {
  pub revision: ObservationRevision,
  pub previous: ObservationValue,
  pub current: ObservationValue,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingPendingFeedState<FeedId> {
  pub previous: Option<FeedId>,
  pub next: Option<FeedId>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingPendingFeedList<FeedId> {
  pub head: Option<FeedId>,
  pub tail: Option<FeedId>,
  pub cursor: Option<FeedId>,
  pub count: u32,
}

impl<FeedId> Default for CrossingPendingFeedList<FeedId> {
  fn default() -> Self {
    Self {
      head: None,
      tail: None,
      cursor: None,
      count: 0,
    }
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingRangeCursor {
  pub revision: ObservationRevision,
  pub traversal: CrossingTraversal,
  pub search_bound: ObservationValue,
  pub current_threshold: Option<ObservationValue>,
  pub page: u32,
  pub offset: u32,
  pub exhausted: bool,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum CrossingWorkerFaultClass {
  Invariant,
  Capacity,
  SchedulerExhausted,
  Other,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct CrossingWorkerFault<FeedId> {
  pub feed: FeedId,
  pub revision: Option<ObservationRevision>,
  pub threshold: Option<ObservationValue>,
  pub class: CrossingWorkerFaultClass,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ObservationFanoutWorkerFault<FeedId> {
  pub feed: FeedId,
  pub revision: ObservationRevision,
  pub subscriber_page: Option<u32>,
  pub class: CrossingWorkerFaultClass,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum CrossingWorkPlan {
  Empty,
  CompleteTransition,
  SeekMiss,
  OpenLeaf,
  AdvanceLeaf,
  AdvancePage,
  SkipPostInstallationTransition,
  SkipPostInstallationPairPending,
  SkipPostInstallationPair,
  RearmCohort,
  RearmCohortPairPending,
  RearmCohortPair,
  FireCohortPending,
  FireCohortPairPending,
  FireCohortPlacedBatch,
  FireCohortCoalescedPair,
  FireCohortCoalesced,
  FireCohortPlaced,
  FireCohortClosed,
  StructuralFault,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ObservationSubscriberPageList {
  pub head: u32,
  pub tail: u32,
  pub count: u32,
}

#[derive(
  polkadot_sdk::frame_support::CloneNoBound,
  polkadot_sdk::frame_support::DebugNoBound,
  polkadot_sdk::frame_support::PartialEqNoBound,
  polkadot_sdk::frame_support::EqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
  MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxEntries))]
pub struct ObservationSubscriberPage<MaxEntries: Get<u32>> {
  pub previous: Option<u32>,
  pub next: Option<u32>,
  pub entries: BoundedVec<Option<ActorId>, MaxEntries>,
}

impl<MaxEntries: Get<u32>> Default for ObservationSubscriberPage<MaxEntries> {
  fn default() -> Self {
    Self {
      previous: None,
      next: None,
      entries: BoundedVec::default(),
    }
  }
}

impl<MaxEntries: Get<u32>> core::ops::Deref for ObservationSubscriberPage<MaxEntries> {
  type Target = BoundedVec<Option<ActorId>, MaxEntries>;

  fn deref(&self) -> &Self::Target {
    &self.entries
  }
}

impl<MaxEntries: Get<u32>> core::ops::DerefMut for ObservationSubscriberPage<MaxEntries> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.entries
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DirtyObservationState<FeedId, BlockNumber> {
  pub latest_revision: ObservationRevision,
  pub fanout_revision: ObservationRevision,
  pub dirty_since: BlockNumber,
  pub next_subscriber_page: Option<u32>,
  pub previous_dirty_feed: Option<FeedId>,
  pub next_dirty_feed: Option<FeedId>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DirtyObservationList<FeedId> {
  pub head: Option<FeedId>,
  pub tail: Option<FeedId>,
  pub cursor: Option<FeedId>,
  pub count: u32,
}

impl<FeedId> Default for DirtyObservationList<FeedId> {
  fn default() -> Self {
    Self {
      head: None,
      tail: None,
      cursor: None,
      count: 0,
    }
  }
}
