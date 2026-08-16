use crate::lifecycle_types::ActorId;
use frame::prelude::*;

pub type ObservationRevision = u64;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("observation_types", "types"))]
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
#[scale_info(replace_segment("observation_types", "types"))]
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
#[scale_info(replace_segment("observation_types", "types"))]
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
#[scale_info(replace_segment("observation_types", "types"))]
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
