/*
Domain: AAA matching-runtime simulation provenance
Owns: Runtime-code, metadata, finalized-state, provider-response, and runtime-output identity binding.
Excludes: Runtime API implementation, Wasm execution, chain transport, signing, submission, and local outcome synthesis.
Zone: Automation domain trust gate; providers execute while this module rejects mismatched or incomplete evidence.
*/
import { blake2AsHex } from '@polkadot/util-crypto';

import {
  type AaaPlanArtifact,
  type AaaPlanHex,
  type AaaPlanRuntimeIdentity,
  inspectAaaPlanArtifact,
} from './plan-artifact.ts';
import {
  type AaaDecodedRuntimeSimulationOutcome,
  decodeAaaRuntimeSimulationResult,
} from './runtime-simulation-codec.ts';

export type AaaMatchingWasmPin = {
  planId: AaaPlanHex;
  genesisHash: AaaPlanHex;
  blockHash: AaaPlanHex;
  blockNumber: number;
  stateRoot: AaaPlanHex;
  stateSource: 'FinalizedBlock' | 'VerifiedStateProof';
  runtimeCodeHash: AaaPlanHex;
  metadataHash: AaaPlanHex;
  specVersion: number;
  transactionVersion: number;
  runtimeApi: string;
  runtimeApiVersion: number;
};

export type AaaRuntimeSimulationOutcome = AaaDecodedRuntimeSimulationOutcome & {
  resultScale: AaaPlanHex;
};

export type AaaMatchingWasmResponse = {
  engine: 'RuntimeWasm';
  pin: AaaMatchingWasmPin;
  outcome: AaaRuntimeSimulationOutcome;
};

export type AaaMatchingWasmProvider = {
  simulate(request: {
    pin: AaaMatchingWasmPin;
    aaaId: bigint;
    mode: 'FreshCurrentPlan' | 'CurrentContinuation';
    programScale: AaaPlanHex;
    aaaType: AaaPlanArtifact['aaaType'];
    mutability: AaaPlanArtifact['mutability'];
  }): Promise<AaaMatchingWasmResponse>;
};

const HASH_PATTERN = /^0x[0-9a-f]{64}$/;
const BYTES_PATTERN = /^0x(?:[0-9a-f]{2})*$/;

function validateHash(value: string, field: string) {
  if (!HASH_PATTERN.test(value)) {
    throw new Error(`${field} must be canonical 32-byte lowercase hex`);
  }
}

function validateIndex(value: number, field: string) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
}

function samePin(left: AaaMatchingWasmPin, right: AaaMatchingWasmPin) {
  return (Object.keys(left) as Array<keyof AaaMatchingWasmPin>).every(
    (key) => left[key] === right[key],
  );
}

export async function runAaaMatchingWasmSimulation(input: {
  artifact: AaaPlanArtifact;
  aaaId: bigint;
  mode: 'FreshCurrentPlan' | 'CurrentContinuation';
  metadataBytes: Uint8Array;
  runtime: AaaPlanRuntimeIdentity;
  runtimeCodeBytes: Uint8Array;
  snapshot: {
    blockHash: AaaPlanHex;
    blockNumber: number;
    stateRoot: AaaPlanHex;
    stateSource: AaaMatchingWasmPin['stateSource'];
  };
  runtimeApi: string;
  runtimeApiVersion: number;
  provider: AaaMatchingWasmProvider;
}): Promise<AaaMatchingWasmResponse> {
  const inspection = inspectAaaPlanArtifact(
    input.artifact,
    input.metadataBytes,
    input.runtime,
  );
  if (!inspection.valid) {
    throw new Error(
      `Invalid AAA plan artifact: ${inspection.errors.join('; ')}`,
    );
  }
  const maxSteps = activeExecutionPlanLength(inspection.runtimeValue);
  if (input.aaaId < 0n) {
    throw new Error('aaaId must be non-negative');
  }
  if (!['FreshCurrentPlan', 'CurrentContinuation'].includes(input.mode)) {
    throw new Error('mode must identify one runtime simulation path');
  }
  validateHash(input.snapshot.blockHash, 'blockHash');
  validateHash(input.snapshot.stateRoot, 'stateRoot');
  validateIndex(input.snapshot.blockNumber, 'blockNumber');
  if (
    !['FinalizedBlock', 'VerifiedStateProof'].includes(
      input.snapshot.stateSource,
    )
  ) {
    throw new Error(
      'stateSource must identify finalized or proof-verified state',
    );
  }
  validateIndex(input.runtimeApiVersion, 'runtimeApiVersion');
  if (input.runtimeApi.trim().length === 0) {
    throw new Error('runtimeApi must identify the executed runtime method');
  }
  if (input.runtimeCodeBytes.length === 0) {
    throw new Error(
      'runtimeCodeBytes must contain the state-pinned runtime code',
    );
  }

  const pin: AaaMatchingWasmPin = {
    planId: input.artifact.planId,
    genesisHash: input.artifact.genesisHash,
    blockHash: input.snapshot.blockHash,
    blockNumber: input.snapshot.blockNumber,
    stateRoot: input.snapshot.stateRoot,
    stateSource: input.snapshot.stateSource,
    runtimeCodeHash: blake2AsHex(input.runtimeCodeBytes, 256) as AaaPlanHex,
    metadataHash: input.artifact.metadataHash,
    specVersion: input.artifact.specVersion,
    transactionVersion: input.artifact.transactionVersion,
    runtimeApi: input.runtimeApi.trim(),
    runtimeApiVersion: input.runtimeApiVersion,
  };
  const response = await input.provider.simulate({
    pin,
    aaaId: input.aaaId,
    mode: input.mode,
    programScale: input.artifact.programScale,
    aaaType: input.artifact.aaaType,
    mutability: input.artifact.mutability,
  });
  if (response.engine !== 'RuntimeWasm') {
    throw new Error('Provider did not attest RuntimeWasm execution');
  }
  if (!samePin(response.pin, pin)) {
    throw new Error(
      'Provider response does not match the requested runtime/state pin',
    );
  }
  validateOutcome(response.outcome, maxSteps);
  const decoded = decodeAaaRuntimeSimulationResult(
    input.metadataBytes,
    response.outcome.resultScale,
  );
  if (!decoded.success) {
    throw new Error(`Runtime simulation rejected: ${decoded.error}`);
  }
  if (!sameOutcome(response.outcome, decoded.outcome)) {
    throw new Error(
      'Provider outcome does not match its canonical SCALE result bytes',
    );
  }
  return response;
}

function activeExecutionPlanLength(runtimeValue: unknown) {
  if (runtimeValue == null || typeof runtimeValue !== 'object') {
    throw new Error('Runtime simulation requires an Active ProgramInput');
  }
  const program = runtimeValue as Record<string, unknown>;
  if (program.type !== 'Active' || program.value == null) {
    throw new Error('Runtime simulation requires an Active ProgramInput');
  }
  const value = program.value as Record<string, unknown>;
  if (
    !Array.isArray(value.execution_plan) ||
    value.execution_plan.length === 0
  ) {
    throw new Error('Runtime simulation requires a non-empty execution plan');
  }
  return value.execution_plan.length;
}

function sameOutcome(
  response: AaaRuntimeSimulationOutcome,
  decoded: AaaDecodedRuntimeSimulationOutcome,
) {
  const comparableResponse = { ...response, resultScale: undefined };
  return (
    JSON.stringify(comparableResponse, bigintReplacer) ===
    JSON.stringify(decoded, bigintReplacer)
  );
}

function bigintReplacer(_key: string, value: unknown) {
  return typeof value === 'bigint' ? `${value}n` : value;
}

function validateOutcome(
  outcome: AaaRuntimeSimulationOutcome,
  maxSteps: number,
) {
  if (!['Completed', 'Aborted', 'Suspended'].includes(outcome.status)) {
    throw new Error('Unsupported runtime simulation status');
  }
  validateIndex(outcome.attempt, 'outcome.attempt');
  validateIndex(outcome.startCursor, 'outcome.startCursor');
  if (outcome.cycleNonce < 0n) {
    throw new Error('outcome.cycleNonce must be non-negative');
  }
  if (!BYTES_PATTERN.test(outcome.resultScale)) {
    throw new Error(
      'outcome.resultScale must be canonical lowercase SCALE hex',
    );
  }
  if (outcome.finalizedThrough != null) {
    validateIndex(outcome.finalizedThrough, 'outcome.finalizedThrough');
  }
  if (outcome.steps.length > maxSteps) {
    throw new Error(
      'Runtime step evidence exceeds the admitted execution plan',
    );
  }
  let previousStep = -1;
  for (const step of outcome.steps) {
    validateIndex(step.stepIndex, 'outcome.steps.stepIndex');
    if (
      step.stepIndex < outcome.startCursor ||
      step.stepIndex >= maxSteps ||
      step.stepIndex <= previousStep
    ) {
      throw new Error(
        'Runtime step evidence must be ordered within the attempted suffix',
      );
    }
    previousStep = step.stepIndex;
  }
  if (outcome.status === 'Suspended') {
    if (outcome.continuationCursor == null) {
      throw new Error(
        'Suspended runtime outcomes require a Continuation cursor',
      );
    }
    validateIndex(outcome.continuationCursor, 'outcome.continuationCursor');
    if (outcome.continuationCursor >= maxSteps) {
      throw new Error(
        'Continuation cursor exceeds the admitted execution plan',
      );
    }
    if (outcome.continuationCursor < outcome.startCursor) {
      throw new Error(
        'Continuation cursor cannot precede the attempted suffix',
      );
    }
    if (
      outcome.finalizedThrough != null &&
      outcome.finalizedThrough >= outcome.continuationCursor
    ) {
      throw new Error('Suspended cursor must remain the unresolved step');
    }
  } else if (outcome.continuationCursor != null) {
    throw new Error(
      'Only Suspended runtime outcomes may expose a Continuation cursor',
    );
  }
}
