use super::{CrossingWorkerFault, ObservationFanoutWorkerFault};
use frame::prelude::*;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum ActorFaultKind {
  Control,
  Body,
  Detector,
  Queue,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum FaultId {
  CrossingWorker,
  ObservationFanoutWorker,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum FaultContext<FeedId> {
  Crossing(CrossingWorkerFault<FeedId>),
  ObservationFanout(ObservationFanoutWorkerFault<FeedId>),
}
