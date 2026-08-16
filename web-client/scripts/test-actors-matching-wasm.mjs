/*
Domain: Actors matching-Wasm provenance validation
Owns: Runtime code/state pin, provider echo, RuntimeWasm attestation, and Continuation-output rejection fixtures.
Excludes: Runtime execution, RPC transport, signing, submission, and chain mutation.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  getDeosActorFinalizedAuthoringContext,
  runDeosActorFinalizedSimulation,
} from '../src/lib/adapters/blockchain/actor-simulation.ts';
import {
  createActorContractArtifact,
  encodeActorContractValue,
} from '../src/lib/automation/contract-artifact.ts';
import { runActorMatchingWasmSimulation } from '../src/lib/automation/matching-wasm.ts';
import {
  decodeActorRuntimeSimulationResult,
  encodeActorRuntimeSimulationResult,
} from '../src/lib/automation/runtime-simulation-codec.ts';

const metadataBytes = new Uint8Array(
  await readFile(new URL('../.papi/metadata/deos.scale', import.meta.url)),
);
const runtime = {
  genesisHash: `0x${'11'.repeat(32)}`,
  specVersion: 1,
  transactionVersion: 1,
};
const step = {
  precondition: [
    [
      {
        timing: { type: 'Opening', value: undefined },
        predicate: {
          type: 'ObservationBelow',
          value: {
            feed: {
              asset_in: { type: 'Native', value: undefined },
              asset_out: { type: 'Local', value: 7 },
              method: { type: 'PreExecutionSpot', value: undefined },
              aggregation: {
                type: 'Ema',
                value: { half_life_blocks: 100 },
              },
              scale: 12,
            },
            threshold: 1n,
            max_age_blocks: 12,
          },
        },
      },
    ],
  ],
  task: {
    type: 'Stake',
    value: {
      asset: { type: 'Native', value: undefined },
      amount: { type: 'Fixed', value: 0n },
    },
  },
  on_error: { type: 'AbortCycle', value: undefined },
};
const contractScale = encodeActorContractValue(metadataBytes, {
  type: 'Active',
  value: {
    schedule: {
      trigger: {
        type: 'Immediate',
        value: {
          sources: [{ type: 'Manual', value: undefined }],
        },
      },
      cooldown_blocks: 0,
    },
    schedule_window: undefined,
    steps: [step, step],
    completion: { type: 'Persistent', value: undefined },
    funding: { type: 'RuntimePolicy', value: undefined },
  },
});
const artifact = createActorContractArtifact({
  metadataBytes,
  runtime,
  actorType: 'System',
  mutability: 'Mutable',
  contractScale,
});
const base = {
  artifact,
  actorId: 14n,
  mode: 'CurrentContinuation',
  metadataBytes,
  runtime,
  runtimeCodeBytes: Uint8Array.of(1, 2, 3),
  snapshot: {
    blockHash: `0x${'22'.repeat(32)}`,
    blockNumber: 42,
    stateRoot: `0x${'33'.repeat(32)}`,
    stateSource: 'FinalizedBlock',
  },
  runtimeApi: 'ActorSimulationApi_simulate_current_contract',
  runtimeApiVersion: 1,
};

const suspendedRuntimeValue = {
  success: true,
  value: {
    status: { type: 'Suspended', value: undefined },
    cycle_nonce: 7n,
    start_cursor: 0,
    continuation_cursor: 1,
    unsuccessful_attempts_at_cursor: 2,
    cumulative_outcomes: {
      executed_steps: 3,
      committed_effectful_tasks: 3,
      precondition_skips: 0,
      skipped_resolution: 0,
      skipped_funding_unavailable: 0,
      failed_steps: 0,
    },
    steps: [
      {
        step_index: 0,
        outcome: { type: 'Executed', value: undefined },
      },
      {
        step_index: 1,
        outcome: { type: 'FundingUnavailable', value: undefined },
      },
    ],
  },
};
const suspendedOutcome = {
  status: 'Suspended',
  closeReason: null,
  cycleNonce: 7n,
  startCursor: 0,
  continuationCursor: 1,
  unsuccessfulAttemptsAtCursor: 2,
  cumulativeOutcomes: {
    executedSteps: 3,
    committedEffectfulTasks: 3,
    preconditionSkips: 0,
    skippedResolution: 0,
    skippedFundingUnavailable: 0,
    failedSteps: 0,
  },
  steps: [
    { stepIndex: 0, outcome: { type: 'Executed' } },
    {
      stepIndex: 1,
      outcome: { type: 'FundingUnavailable' },
    },
  ],
  resultScale: encodeActorRuntimeSimulationResult(
    metadataBytes,
    suspendedRuntimeValue,
  ),
};

test('runtime API result codec discovers metadata and preserves bounded evidence', () => {
  const { resultScale, ...expectedOutcome } = suspendedOutcome;
  assert.deepEqual(
    decodeActorRuntimeSimulationResult(metadataBytes, resultScale),
    {
      success: true,
      outcome: expectedOutcome,
      resultScale,
    },
  );
  const stoppedScale = encodeActorRuntimeSimulationResult(metadataBytes, {
    success: true,
    value: {
      status: { type: 'Completed', value: undefined },
      cycle_nonce: 8n,
      start_cursor: 0,
      continuation_cursor: undefined,
      cumulative_outcomes: {
        executed_steps: 1,
        committed_effectful_tasks: 0,
        precondition_skips: 0,
        skipped_resolution: 0,
        skipped_funding_unavailable: 0,
        failed_steps: 0,
      },
      steps: [
        {
          step_index: 1,
          outcome: { type: 'Stopped', value: undefined },
        },
      ],
    },
  });
  const stopped = decodeActorRuntimeSimulationResult(
    metadataBytes,
    stoppedScale,
  );
  assert.equal(stopped.success, true);
  if (stopped.success) {
    assert.deepEqual(stopped.outcome.steps[0].outcome, { type: 'Stopped' });
    assert.equal(stopped.outcome.status, 'Completed');
    assert.equal(stopped.outcome.cumulativeOutcomes.committedEffectfulTasks, 0);
  }
  const closedScale = encodeActorRuntimeSimulationResult(metadataBytes, {
    success: true,
    value: {
      ...suspendedRuntimeValue.value,
      status: {
        type: 'Closed',
        value: { type: 'ProductiveCycleCompleted', value: undefined },
      },
      continuation_cursor: undefined,
      unsuccessful_attempts_at_cursor: undefined,
    },
  });
  const closed = decodeActorRuntimeSimulationResult(metadataBytes, closedScale);
  assert.equal(closed.success, true);
  if (closed.success) {
    assert.equal(closed.outcome.status, 'Closed');
    assert.equal(closed.outcome.closeReason, 'ProductiveCycleCompleted');
    assert.equal(closed.outcome.continuationCursor, null);
    assert.equal(closed.outcome.unsuccessfulAttemptsAtCursor, null);
  }
  const rejectedScale = encodeActorRuntimeSimulationResult(metadataBytes, {
    success: false,
    value: { type: 'ContractMismatch', value: undefined },
  });
  assert.deepEqual(
    decodeActorRuntimeSimulationResult(metadataBytes, rejectedScale),
    { success: false, error: 'ContractMismatch', resultScale: rejectedScale },
  );
});

test('authoring context binds metadata and versions at one finalized block without runtime execution', async () => {
  const at = base.snapshot.blockHash;
  let transportRequests = 0;
  const context = await getDeosActorFinalizedAuthoringContext({
    async ensureConnected() {
      return {
        client: {
          async getFinalizedBlock() {
            return { hash: at, number: 42 };
          },
          async getChainSpecData() {
            return { genesisHash: runtime.genesisHash };
          },
          async _request() {
            transportRequests += 1;
            throw new Error('Authoring context must not fetch runtime code');
          },
        },
        typedApi: {
          apis: {
            Core: {
              async version(options) {
                assert.deepEqual(options, { at });
                return { spec_version: 1, transaction_version: 1 };
              },
            },
            Metadata: {
              async metadata_at_version(version, options) {
                assert.equal(version, 16);
                assert.deepEqual(options, { at });
                return metadataBytes;
              },
            },
          },
        },
      };
    },
  });

  assert.deepEqual(context.runtime, runtime);
  assert.deepEqual(context.finalizedBlock, { hash: at, number: 42 });
  assert.deepEqual(context.metadataBytes, metadataBytes);
  assert.equal(transportRequests, 0);
});

test('finalized transport pins state and invokes the typed runtime API at one block', async () => {
  let observedArguments;
  const at = base.snapshot.blockHash;
  const connection = {
    async ensureConnected() {
      return {
        client: {
          async getFinalizedBlock() {
            return { hash: at, number: 42 };
          },
          async getBlockHeader(hash) {
            assert.equal(hash, at);
            return { stateRoot: base.snapshot.stateRoot };
          },
          async getChainSpecData() {
            return { genesisHash: runtime.genesisHash };
          },
          async _request(method, params) {
            assert.equal(method, 'state_getStorage');
            assert.deepEqual(params, ['0x3a636f6465', at]);
            return '0x010203';
          },
        },
        typedApi: {
          apis: {
            Core: {
              async version(options) {
                assert.deepEqual(options, { at });
                return { spec_version: 1, transaction_version: 1 };
              },
            },
            Metadata: {
              async metadata_at_version(version, options) {
                assert.equal(version, 16);
                assert.deepEqual(options, { at });
                return metadataBytes;
              },
            },
            ActorSimulationApi: {
              async simulate_current_contract(...args) {
                observedArguments = args;
                return suspendedRuntimeValue;
              },
            },
          },
        },
      };
    },
  };

  const result = await runDeosActorFinalizedSimulation(connection, {
    artifact,
    actorId: 14n,
    mode: 'CurrentContinuation',
    finalizedBlock: { hash: at, number: 42 },
  });

  assert.equal(result.outcome.status, 'Suspended');
  assert.equal(result.outcome.continuationCursor, 1);
  assert.equal(observedArguments[0], 14n);
  assert.deepEqual(observedArguments[1], {
    type: 'System',
    value: undefined,
  });
  assert.deepEqual(observedArguments[2], {
    type: 'Mutable',
    value: undefined,
  });
  assert.deepEqual(observedArguments[4], {
    type: 'CurrentContinuation',
    value: undefined,
  });
  assert.deepEqual(observedArguments[5], { at });
});

test('matching-Wasm gate binds runtime code, metadata, state, API, and Actor Contract identity', async () => {
  let observedRequest;
  const result = await runActorMatchingWasmSimulation({
    ...base,
    provider: {
      async simulate(request) {
        observedRequest = request;
        return {
          engine: 'RuntimeWasm',
          pin: request.pin,
          outcome: suspendedOutcome,
        };
      },
    },
  });

  assert.equal(result.pin.contractId, artifact.contractId);
  assert.equal(
    result.pin.runtimeCodeHash,
    '0x11c0e79b71c3976ccd0c02d1310e2516c08edc9d8b6f57ccd680d63a4d8e72da',
  );
  assert.equal(result.pin.metadataHash, artifact.metadataHash);
  assert.equal(result.pin.stateRoot, base.snapshot.stateRoot);
  assert.equal(observedRequest.actorId, 14n);
  assert.equal(observedRequest.mode, 'CurrentContinuation');
  assert.equal(observedRequest.contractScale, artifact.contractScale);
  assert.equal(result.outcome.continuationCursor, 1);
});

test('provider cannot change any requested runtime or state dependency', async () => {
  await assert.rejects(
    runActorMatchingWasmSimulation({
      ...base,
      provider: {
        async simulate(request) {
          return {
            engine: 'RuntimeWasm',
            pin: {
              ...request.pin,
              runtimeCodeHash: `0x${'44'.repeat(32)}`,
            },
            outcome: suspendedOutcome,
          };
        },
      },
    }),
    /does not match the requested runtime\/state pin/,
  );
});

test('provider summary must match canonical runtime SCALE bytes', async () => {
  await assert.rejects(
    runActorMatchingWasmSimulation({
      ...base,
      provider: {
        async simulate(request) {
          return {
            engine: 'RuntimeWasm',
            pin: request.pin,
            outcome: {
              ...suspendedOutcome,
              cumulativeOutcomes: {
                ...suspendedOutcome.cumulativeOutcomes,
                executedSteps: 4,
              },
            },
          };
        },
      },
    }),
    /does not match its canonical SCALE result bytes/,
  );
});

test('local projections and malformed Continuation outcomes fail closed', async () => {
  await assert.rejects(
    runActorMatchingWasmSimulation({
      ...base,
      provider: {
        async simulate(request) {
          return {
            engine: 'AdapterLocalProjection',
            pin: request.pin,
            outcome: suspendedOutcome,
          };
        },
      },
    }),
    /did not attest RuntimeWasm execution/,
  );
  await assert.rejects(
    runActorMatchingWasmSimulation({
      ...base,
      provider: {
        async simulate(request) {
          return {
            engine: 'RuntimeWasm',
            pin: request.pin,
            outcome: {
              ...suspendedOutcome,
              continuationCursor: null,
            },
          };
        },
      },
    }),
    /require a Continuation cursor/,
  );
});
