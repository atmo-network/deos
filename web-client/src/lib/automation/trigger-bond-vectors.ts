/*
Domain: Actors Trigger-state bond vectors
Owns: Runtime-generated reference-policy bond quotes for typed Trigger authoring.
Excludes: Live chain transport, reserve mutation, token formatting policy, and physical detector occupancy.
Zone: Automation domain contract; the runtime API remains canonical at a finalized block.
*/
import vectorsJson from './actors-trigger-bond-vectors.json' with { type: 'json' };

export type ActorTriggerFamily =
  | 'Manual'
  | 'AddressEvent'
  | 'ObservationChange'
  | 'ObservationCrossing'
  | 'Cadenced';

type TriggerBondVectors = {
  formatVersion: number;
  metadataSha256: string;
  actorsWeightSha256: string;
  vectors: { triggerFamily: ActorTriggerFamily; amount: string }[];
};

export const ACTORS_TRIGGER_BOND_VECTORS = vectorsJson as TriggerBondVectors;

export function actorTriggerStateBond(
  triggerFamily: ActorTriggerFamily,
): bigint {
  const vector = ACTORS_TRIGGER_BOND_VECTORS.vectors.find(
    (candidate) => candidate.triggerFamily === triggerFamily,
  );
  if (!vector || !/^(0|[1-9]\d*)$/.test(vector.amount)) {
    throw new Error(
      `Missing canonical Trigger-state bond vector for ${triggerFamily}`,
    );
  }
  return BigInt(vector.amount);
}
