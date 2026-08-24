use super::{CrossingWorkerFault, ObservationFanoutWorkerFault, WakeupWorkerFault};
use frame::prelude::*;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum ActorFaultKind {
  Control,
  Body,
  Detector,
  Wakeup,
  Queue,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum FaultId {
  CrossingWorker,
  ObservationFanoutWorker,
  WakeupWorker,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum FaultContext<FeedId, BlockNumber> {
  Crossing(CrossingWorkerFault<FeedId>),
  ObservationFanout(ObservationFanoutWorkerFault<FeedId>),
  Wakeup(WakeupWorkerFault<BlockNumber>),
}
