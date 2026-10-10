/*
Domain: DEOS Actors canonical control projection
Owns: Finalized semantic/process resolution, generation checks and current residence.
Excludes: Scheduler mutation, Contract reconstruction, eligibility arithmetic and history.
Zone: Blockchain adapter capability; missing canonical metadata is explicitly unavailable.
*/
import type { HexString } from 'polkadot-api';

import type { AutomationActorResidence } from '../../automation/types.ts';

type Identity = {
  sovereign_account: string;
  actor_class:
    | { type: 'User' }
    | { type: 'System'; value: { sovereign_id: bigint } };
  cycle_nonce: bigint;
};
type Hot = {
  lifecycle: { type: 'Active' | 'Paused' };
  cycle_state: { type: 'Idle' | 'Running' | 'Suspended' };
  pending_signal: boolean;
  last_cycle_block: number | null;
};

function record(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} is malformed`);
  }
  return value as Record<string, unknown>;
}

function identity(value: unknown): Identity {
  const item = record(value, 'Actor identity');
  const actorClass = record(item.actor_class, 'Actor class');
  if (
    typeof item.sovereign_account !== 'string' ||
    typeof item.cycle_nonce !== 'bigint' ||
    item.cycle_nonce < 0n
  ) {
    throw new Error('Actor identity is malformed');
  }
  let kind: Identity['actor_class'];
  if (actorClass.type === 'User') {
    kind = { ...actorClass, type: 'User' };
  } else if (actorClass.type === 'System') {
    const system = record(actorClass.value, 'System Actor class');
    if (typeof system.sovereign_id !== 'bigint' || system.sovereign_id < 0n) {
      throw new Error('System Actor identity is malformed');
    }
    kind = {
      ...actorClass,
      type: 'System',
      value: { ...system, sovereign_id: system.sovereign_id },
    };
  } else {
    throw new Error('Actor class is unsupported');
  }
  return {
    ...item,
    sovereign_account: item.sovereign_account,
    actor_class: kind,
    cycle_nonce: item.cycle_nonce,
  };
}

function hot(value: unknown): Hot {
  const item = record(value, 'Actor hot state');
  const lifecycle = record(item.lifecycle, 'Actor lifecycle');
  const cycle = record(item.cycle_state, 'Actor Cycle state');
  if (
    (lifecycle.type !== 'Active' && lifecycle.type !== 'Paused') ||
    (cycle.type !== 'Idle' &&
      cycle.type !== 'Running' &&
      cycle.type !== 'Suspended') ||
    typeof item.pending_signal !== 'boolean' ||
    (item.last_cycle_block !== undefined &&
      item.last_cycle_block !== null &&
      (!Number.isSafeInteger(item.last_cycle_block) ||
        Number(item.last_cycle_block) < 0))
  ) {
    throw new Error('Actor hot state is malformed');
  }
  return {
    ...item,
    lifecycle: { type: lifecycle.type },
    cycle_state: { type: cycle.type },
    pending_signal: item.pending_signal,
    last_cycle_block:
      item.last_cycle_block == null ? null : Number(item.last_cycle_block),
  };
}

function residence(value: unknown): AutomationActorResidence {
  const item = record(value, 'Actor process residence');
  switch (item.type) {
    case 'Service': {
      const kind = record(item.value, 'Actor Service residence');
      if (kind.type !== 'Live' && kind.type !== 'Pending') {
        throw new Error('Actor Service residence is unsupported');
      }
      return { type: 'Service', kind: kind.type };
    }
    case 'Deadline': {
      const deadline = record(item.value, 'Actor Deadline residence');
      const key = record(deadline.key, 'Actor Deadline key');
      if (
        key.type === 'Block' &&
        typeof key.value === 'number' &&
        Number.isSafeInteger(key.value) &&
        key.value >= 0
      ) {
        return { type: 'Deadline', key: { type: 'Block', value: key.value } };
      }
      if (
        key.type === 'Tick' &&
        typeof key.value === 'bigint' &&
        key.value >= 0n
      ) {
        return { type: 'Deadline', key: { type: 'Tick', value: key.value } };
      }
      throw new Error('Actor Deadline key is malformed');
    }
    case 'Parked': {
      const parked = record(item.value, 'Actor Park residence');
      const reason = record(parked.reason, 'Actor Park reason');
      if (!['ParkedBalanceBelowThreshold'].includes(String(reason.type))) {
        throw new Error('Actor Park reason is unsupported');
      }
      return { type: 'Parked', reason: String(reason.type) };
    }
    default:
      throw new Error('Actor process residence is unsupported');
  }
}

export async function readActorControlProjection(
  api: unknown,
  at: HexString,
  actorId: bigint,
) {
  // These names belong to current metadata. Old descriptors must not reinterpret empty legacy
  // storage as absence; the separately gated generated refresh supplies these query capabilities.
  const actors = record(
    record(record(api, 'Actors API').query, 'Actors queries').Actors,
    'Actors storage',
  );
  const semanticQuery = actors.ActorSemanticStates;
  const processQuery = actors.ActorProcesses;
  if (semanticQuery == null || processQuery == null) {
    return {
      status: 'Unavailable',
      reason: 'Canonical Actors metadata is unavailable',
    } as const;
  }
  const semanticReader = record(semanticQuery, 'Actor semantic query');
  const processReader = record(processQuery, 'Actor process query');
  if (
    typeof semanticReader.getValue !== 'function' ||
    typeof processReader.getValue !== 'function'
  ) {
    return {
      status: 'Unavailable',
      reason: 'Canonical Actors storage queries are unavailable',
    } as const;
  }
  const [rawSemantic, rawProcess]: unknown[] = await Promise.all([
    semanticReader.getValue(actorId, { at }),
    processReader.getValue(actorId, { at }),
  ]);
  if (rawSemantic == null) {
    if (rawProcess != null)
      throw new Error('Unregistered Actor retains a process');
    return { status: 'NotRegistered' } as const;
  }
  const semantic = record(rawSemantic, 'Actor semantic state');
  const value = record(semantic.value, 'Actor semantic record');
  if (typeof value.generation !== 'bigint' || value.generation < 0n) {
    throw new Error('Actor semantic generation is malformed');
  }
  const actorIdentity = identity(value.identity);
  if (semantic.type === 'Dormant') {
    if (rawProcess != null)
      throw new Error('Dormant Actor retains an active process');
    return {
      status: 'Dormant',
      identity: actorIdentity,
      generation: value.generation,
    } as const;
  }
  if (semantic.type !== 'Active' || value.generation === 0n) {
    throw new Error('Actor semantic state is unsupported');
  }
  const process = record(rawProcess, 'Active Actor process');
  if (process.generation !== value.generation)
    throw new Error('Actor process generation mismatch');
  const status = record(process.status, 'Actor process status');
  let placement: AutomationActorResidence;
  if (status.type === 'Disabled') {
    if (process.residence != null)
      throw new Error('Disabled Actor retains executable residence');
    placement = { type: 'Disabled' };
  } else if (status.type === 'Serving' || status.type === 'AwaitingFunding') {
    placement = residence(process.residence);
    if (
      status.type === 'AwaitingFunding' &&
      actorIdentity.actor_class.type !== 'User'
    ) {
      throw new Error('System Actor cannot await fee funding');
    }
  } else {
    throw new Error('Active Actor process status is unsupported');
  }
  if (
    status.type === 'AwaitingFunding' &&
    (placement.type === 'Parked' ||
      (placement.type === 'Deadline' && placement.key.type !== 'Tick'))
  ) {
    throw new Error('Financial waiting has unsupported process residence');
  }
  return {
    status: 'Active',
    record: {
      ...value,
      identity: actorIdentity,
      hot: hot(value.hot),
      generation: value.generation,
    },
    residence: placement,
    awaitingFunding: status.type === 'AwaitingFunding',
  } as const;
}
