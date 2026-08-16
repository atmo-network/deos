/*
Domain: Actors runtime-simulation SCALE codec
Owns: Metadata discovery, canonical runtime result bytes, typed rejection, and bounded outcome projection.
Excludes: Chain transport, provider trust, Wasm execution, artifact identity, and local outcome synthesis.
Zone: Automation domain capability; matching-runtime providers and trust gates may import it.
*/
import {
  getDynamicBuilder,
  getLookupFn,
} from '@polkadot-api/metadata-builders';
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';

import type { ActorContractHex } from './contract-artifact.ts';

export const ACTORS_SIMULATION_RUNTIME_API =
  'ActorSimulationApi_simulate_current_contract' as const;
export const ACTORS_SIMULATION_RUNTIME_API_VERSION = 1 as const;

export type ActorRuntimeStepOutcome =
  | { type: 'Executed' }
  | { type: 'Stopped' }
  | { type: 'Skipped'; reason: string }
  | { type: 'FundingUnavailable' }
  | {
      type: 'Failed';
      retryClass: string;
      error: { type: string; value: unknown };
    };

export type ActorDecodedRuntimeSimulationOutcome = {
  status: 'Completed' | 'Failed' | 'Suspended' | 'Closed';
  closeReason: string | null;
  cycleNonce: bigint;
  startCursor: number;
  continuationCursor: number | null;
  unsuccessfulAttemptsAtCursor: number | null;
  cumulativeOutcomes: {
    executedSteps: number;
    committedEffectfulTasks: number;
    preconditionSkips: number;
    skippedResolution: number;
    skippedFundingUnavailable: number;
    failedSteps: number;
  };
  steps: Array<{ stepIndex: number; outcome: ActorRuntimeStepOutcome }>;
};

export type ActorDecodedRuntimeSimulationResult =
  | {
      success: true;
      outcome: ActorDecodedRuntimeSimulationOutcome;
      resultScale: ActorContractHex;
    }
  | { success: false; error: string; resultScale: ActorContractHex };

const BYTES_PATTERN = /^0x(?:[0-9a-f]{2})*$/;
const MAX_RESULT_BYTES = 64 * 1024;
const INPUT_NAMES = [
  'actor_id',
  'expected_type',
  'expected_mutability',
  'expected_contract',
  'mode',
];

function bytesToHex(bytes: Uint8Array): ActorContractHex {
  return `0x${Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}

function hexToBytes(value: ActorContractHex): Uint8Array {
  if (!BYTES_PATTERN.test(value)) {
    throw new Error(
      'Runtime simulation result must be canonical lowercase hex',
    );
  }
  const bytes = new Uint8Array((value.length - 2) / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number(`0x${value.slice(2 + index * 2, 4 + index * 2)}`);
  }
  return bytes;
}

function metadataMethod(metadataBytes: Uint8Array) {
  const metadata = unifyMetadata(decAnyMetadata(metadataBytes));
  const apis = metadata.apis.filter(
    (candidate) => candidate.name === 'ActorSimulationApi',
  );
  if (
    apis.length !== 1 ||
    !('version' in apis[0]) ||
    apis[0].version !== ACTORS_SIMULATION_RUNTIME_API_VERSION
  ) {
    throw new Error(
      'Metadata must expose ActorSimulationApi version 1 exactly once',
    );
  }
  const methods = apis[0].methods.filter(
    (candidate) => candidate.name === 'simulate_current_contract',
  );
  if (
    methods.length !== 1 ||
    methods[0].inputs.length !== INPUT_NAMES.length ||
    !methods[0].inputs.every(
      (input, index) => input.name === INPUT_NAMES[index],
    )
  ) {
    throw new Error(
      'Metadata must expose the canonical simulate_current_contract signature',
    );
  }
  return { metadata, method: methods[0] };
}

function asRecord(value: unknown, field: string): Record<string, unknown> {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${field} must be a runtime object`);
  }
  return value as Record<string, unknown>;
}

function asIndex(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
  return value;
}

function asVariant(value: unknown, field: string) {
  const variant = asRecord(value, field);
  if (typeof variant.type !== 'string' || variant.type.length === 0) {
    throw new Error(`${field} must carry a runtime variant type`);
  }
  return { type: variant.type, value: variant.value };
}

function asOptionalIndex(value: unknown, field: string): number | null {
  return value === undefined ? null : asIndex(value, field);
}

function projectStepOutcome(value: unknown): ActorRuntimeStepOutcome {
  const variant = asVariant(value, 'step.outcome');
  switch (variant.type) {
    case 'Executed':
      return { type: 'Executed' };
    case 'Stopped':
      return { type: 'Stopped' };
    case 'Skipped':
      return {
        type: 'Skipped',
        reason: asVariant(variant.value, 'step.outcome.reason').type,
      };
    case 'FundingUnavailable':
      return { type: 'FundingUnavailable' };
    case 'Failed': {
      const failure = asRecord(variant.value, 'step.outcome.failure');
      return {
        type: 'Failed',
        retryClass: asVariant(failure.retry, 'step.outcome.failure.retry').type,
        error: asVariant(failure.error, 'step.outcome.failure.error'),
      };
    }
    default:
      throw new Error(`Unsupported runtime step outcome ${variant.type}`);
  }
}

function projectOutcome(value: unknown): ActorDecodedRuntimeSimulationOutcome {
  const outcome = asRecord(value, 'simulation outcome');
  const parsedStatus = asVariant(outcome.status, 'simulation status');
  const status = parsedStatus.type;
  if (!['Completed', 'Failed', 'Suspended', 'Closed'].includes(status)) {
    throw new Error(`Unsupported runtime simulation status ${status}`);
  }
  const closeReason =
    status === 'Closed'
      ? asVariant(parsedStatus.value, 'simulation close reason').type
      : null;
  if (typeof outcome.cycle_nonce !== 'bigint' || outcome.cycle_nonce < 0n) {
    throw new Error('cycle_nonce must be a non-negative bigint');
  }
  const totals = asRecord(outcome.cumulative_outcomes, 'cumulative_outcomes');
  if (!Array.isArray(outcome.steps)) {
    throw new Error('steps must be an ordered runtime array');
  }
  return {
    status: status as ActorDecodedRuntimeSimulationOutcome['status'],
    closeReason,
    cycleNonce: outcome.cycle_nonce,
    startCursor: asIndex(outcome.start_cursor, 'start_cursor'),
    continuationCursor: asOptionalIndex(
      outcome.continuation_cursor,
      'continuation_cursor',
    ),
    unsuccessfulAttemptsAtCursor: asOptionalIndex(
      outcome.unsuccessful_attempts_at_cursor,
      'unsuccessful_attempts_at_cursor',
    ),
    cumulativeOutcomes: {
      executedSteps: asIndex(totals.executed_steps, 'executed_steps'),
      committedEffectfulTasks: asIndex(
        totals.committed_effectful_tasks,
        'committed_effectful_tasks',
      ),
      preconditionSkips: asIndex(
        totals.precondition_skips,
        'precondition_skips',
      ),
      skippedResolution: asIndex(
        totals.skipped_resolution,
        'skipped_resolution',
      ),
      skippedFundingUnavailable: asIndex(
        totals.skipped_funding_unavailable,
        'skipped_funding_unavailable',
      ),
      failedSteps: asIndex(totals.failed_steps, 'failed_steps'),
    },
    steps: outcome.steps.map((value, index) => {
      const step = asRecord(value, `steps[${index}]`);
      return {
        stepIndex: asIndex(step.step_index, `steps[${index}].step_index`),
        outcome: projectStepOutcome(step.outcome),
      };
    }),
  };
}

export function encodeActorRuntimeSimulationResult(
  metadataBytes: Uint8Array,
  runtimeValue: unknown,
): ActorContractHex {
  const { metadata, method } = metadataMethod(metadataBytes);
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    method.output,
  );
  return bytesToHex(codec.enc(runtimeValue));
}

export function decodeActorRuntimeSimulationResult(
  metadataBytes: Uint8Array,
  resultScale: ActorContractHex,
): ActorDecodedRuntimeSimulationResult {
  const { metadata, method } = metadataMethod(metadataBytes);
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    method.output,
  );
  const source = hexToBytes(resultScale);
  if (source.length > MAX_RESULT_BYTES) {
    throw new Error(
      'Runtime simulation Result exceeds the client decode bound',
    );
  }
  const decoded = asRecord(codec.dec(source), 'runtime Result');
  if (bytesToHex(codec.enc(decoded)) !== resultScale) {
    throw new Error(
      'Runtime simulation Result does not round-trip canonically',
    );
  }
  if (decoded.success === true) {
    return {
      success: true,
      outcome: projectOutcome(decoded.value),
      resultScale,
    };
  }
  if (decoded.success === false) {
    return {
      success: false,
      error: asVariant(decoded.value, 'simulation error').type,
      resultScale,
    };
  }
  throw new Error('Runtime simulation output must be a SCALE Result');
}
