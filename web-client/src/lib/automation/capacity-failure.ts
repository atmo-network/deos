/*
Domain: Actors typed reactive-capacity failures
Owns: Exact browser copy for runtime capacity errors relevant to Trigger authoring.
Excludes: Error decoding, retry policy, live capacity reads, and physical index topology.
*/
export type ActorReactiveCapacityFailure =
  | 'CrossingUserCapacityExceeded'
  | 'CrossingIndexCapacityExceeded';

export function actorReactiveCapacityFailureMessage(
  failure: ActorReactiveCapacityFailure,
): string {
  switch (failure) {
    case 'CrossingUserCapacityExceeded':
      return 'User capacity for this feed is full; the System reserve cannot be consumed by User Actors.';
    case 'CrossingIndexCapacityExceeded':
      return 'Total Crossing capacity for this feed is full; admission fails atomically.';
  }
}
