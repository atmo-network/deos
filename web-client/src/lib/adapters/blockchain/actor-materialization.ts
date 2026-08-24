/*
Domain: Actors materialization transport
Owns: Finalized ActorEligibilityApi capacity/fault reads and lossless lowering into automation contracts.
Excludes: Fault repair, scheduler policy, historical indexing, and UI presentation.
Zone: Blockchain adapter capability; imports only the automation materialization contract.
*/
import type { DeosTypedApi } from './deos.ts';

import type {
  ActorMaterializationProjection,
  MaterializationFaultClass,
} from '../../automation/materialization.ts';

type ActorEligibilityAt = NonNullable<
  Parameters<
    DeosTypedApi['apis']['ActorEligibilityApi']['materialization_faults']
  >[0]
>['at'];

type RecordValue = Record<string, unknown>;

function record(value: unknown, field: string): RecordValue {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${field} must be a runtime object`);
  }
  return value as RecordValue;
}

function count(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
  return value;
}

function integer(value: unknown, field: string): bigint {
  if (typeof value === 'bigint' && value >= 0n) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  throw new Error(`${field} must be an unsigned runtime integer`);
}

function optional<T>(value: unknown, project: (value: unknown) => T): T | null {
  return value === undefined ? null : project(value);
}

function variant(
  value: unknown,
  field: string,
): { type: string; value?: unknown } {
  const decoded = record(value, field);
  if (typeof decoded.type !== 'string') {
    throw new Error(`${field} must carry a runtime variant type`);
  }
  return decoded as { type: string; value?: unknown };
}

function faultClass(value: unknown): MaterializationFaultClass {
  const type = variant(value, 'materialization fault class').type;
  if (
    type !== 'Invariant' &&
    type !== 'Capacity' &&
    type !== 'SchedulerExhausted' &&
    type !== 'Other'
  ) {
    throw new Error(`Unsupported materialization fault class ${type}`);
  }
  return type;
}

function bytes(value: unknown, field: string): Uint8Array {
  if (value instanceof Uint8Array) return value;
  const asBytes = record(value, field).asBytes;
  if (typeof asBytes === 'function') {
    const decoded = asBytes.call(value);
    if (decoded instanceof Uint8Array) return decoded;
  }
  throw new Error(`${field} must carry runtime bytes`);
}

export function projectActorMaterialization(
  runtimeCapacity: unknown,
  runtimeFaults: unknown,
): ActorMaterializationProjection {
  const capacity = record(runtimeCapacity, 'Crossing capacity');
  const faults = record(runtimeFaults, 'materialization faults');

  return {
    crossingCapacity: {
      userLimit: count(capacity.user_limit, 'Crossing user limit'),
      totalLimit: count(capacity.total_limit, 'Crossing total limit'),
      userMemberships: count(
        capacity.user_memberships,
        'Crossing user memberships',
      ),
      totalMemberships: count(
        capacity.total_memberships,
        'Crossing total memberships',
      ),
    },
    faults: {
      crossing: optional(faults.crossing, (value) => {
        const fault = record(value, 'Crossing fault');
        return {
          feed: fault.feed,
          revision: optional(fault.revision, (item) =>
            integer(item, 'revision'),
          ),
          threshold: optional(fault.threshold, (item) =>
            integer(item, 'threshold'),
          ),
          class: faultClass(fault.class),
        };
      }),
      fanout: optional(faults.fanout, (value) => {
        const fault = record(value, 'fanout fault');
        return {
          feed: fault.feed,
          revision: integer(fault.revision, 'fanout revision'),
          subscriberPage: optional(fault.subscriber_page, (item) =>
            count(item, 'subscriber page'),
          ),
          subscriberPosition: count(
            fault.subscriber_position,
            'subscriber position',
          ),
          actorId: optional(fault.actor_id, (item) =>
            integer(item, 'actor id'),
          ),
          semanticContractId: optional(fault.semantic_contract_id, (item) =>
            bytes(item, 'semantic contract id'),
          ),
          bodyCommitment: optional(fault.body_commitment, (item) =>
            bytes(item, 'body commitment'),
          ),
          admissionIdentity: optional(fault.admission_identity, (item) =>
            bytes(item, 'admission identity'),
          ),
          branch: variant(fault.branch, 'fanout branch').type,
          class: faultClass(fault.class),
        };
      }),
      wakeup: optional(faults.wakeup, (value) => {
        const fault = record(value, 'wakeup fault');
        const key = variant(fault.key, 'wakeup key');
        return {
          key:
            key.type === 'Block'
              ? {
                  type: 'Block' as const,
                  block: count(key.value, 'wakeup block'),
                }
              : key.type === 'Tick'
                ? {
                    type: 'Tick' as const,
                    tick: integer(key.value, 'wakeup tick'),
                  }
                : (() => {
                    throw new Error(`Unsupported wakeup key ${key.type}`);
                  })(),
          page: count(fault.page, 'wakeup page'),
          class: faultClass(fault.class),
        };
      }),
    },
  };
}

export async function readActorMaterializationProjection(
  typedApi: DeosTypedApi,
  at: ActorEligibilityAt,
  feed: Parameters<
    DeosTypedApi['apis']['ActorEligibilityApi']['crossing_capacity']
  >[0],
): Promise<ActorMaterializationProjection> {
  const [capacity, faults] = await Promise.all([
    typedApi.apis.ActorEligibilityApi.crossing_capacity(feed, { at }),
    typedApi.apis.ActorEligibilityApi.materialization_faults({ at }),
  ]);
  return projectActorMaterialization(capacity, faults);
}
