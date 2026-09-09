/*
Domain: DEOS Actors canonical control projection
Owns: Finalized locator-to-control-cell resolution and absent/dormant/corrupt classification.
Excludes: Scheduler mutation, Contract reconstruction, lifecycle inference, and historical materialization.
Zone: Blockchain adapter capability; consumers read one bounded canonical control owner.
*/
import type { HexString } from 'polkadot-api';

import type { DeosTypedApi } from './deos.ts';

export async function readActorControlProjection(
  typedApi: DeosTypedApi,
  at: HexString,
  actorId: bigint,
) {
  const [location, dormantIdentity] = await Promise.all([
    typedApi.query.Actors.ActorControlLocators.getValue(actorId, { at }),
    typedApi.query.Actors.ActorIdentities.getValue(actorId, { at }),
  ]);
  if (location == null) {
    return dormantIdentity == null
      ? ({ status: 'NotRegistered' } as const)
      : ({ status: 'Dormant', identity: dormantIdentity } as const);
  }
  if (dormantIdentity != null) {
    throw new Error(
      'Actors identity is present in both dormant and active control owners',
    );
  }

  let cell;
  switch (location.type) {
    case 'Unsignaled':
      cell = await typedApi.query.Actors.ActorUnsignaledControlCells.getValue(
        actorId,
        { at },
      );
      break;
    case 'Ready': {
      const ticket = location.value.ticket;
      const chunk = await typedApi.query.Actors.ActorReadyFrameChunks.getValue(
        ticket / 32n,
        { at },
      );
      cell = chunk?.[Number(ticket % 32n)];
      break;
    }
    case 'Waiting': {
      const page = await typedApi.query.Actors.ActorWaitingFrameChunks.getValue(
        [location.value.key, location.value.page],
        { at },
      );
      const entry = page?.entries[location.value.slot];
      cell = entry?.type === 'Primary' ? entry.value : undefined;
      break;
    }
    default:
      throw new Error('Actors control locator has an unsupported variant');
  }
  if (cell == null || cell.actor_id !== actorId) {
    throw new Error('Actors control locator does not resolve to its Actor');
  }
  return { status: 'Active', location, cell } as const;
}
