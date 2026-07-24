/*
Domain: AAA matching-Wasm provenance validation
Owns: Runtime code/state pin, provider echo, RuntimeWasm attestation, and Continuation-output rejection fixtures.
Excludes: Runtime execution, RPC transport, signing, submission, and chain mutation.
Zone: Web-client validation entrypoint; imports automation domain contracts only.
*/
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { runDeosAaaFinalizedSimulation } from '../src/lib/adapters/blockchain/aaa-simulation.ts';
import { runAaaMatchingWasmSimulation } from '../src/lib/automation/matching-wasm.ts';
import {
  createAaaPlanArtifact,
  encodeAaaProgramValue,
} from '../src/lib/automation/plan-artifact.ts';
import {
  decodeAaaRuntimeSimulationResult,
  encodeAaaRuntimeSimulationResult,
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
  conditions: [],
  task: {
    type: 'Stake',
    value: {
      asset: { type: 'Native', value: undefined },
      amount: { type: 'Fixed', value: 0n },
    },
  },
  on_error: { type: 'AbortCycle', value: undefined },
};
const programScale = encodeAaaProgramValue(metadataBytes, {
  type: 'Active',
  value: {
    schedule: {
      trigger: { type: 'Manual', value: undefined },
      cooldown_blocks: 0,
    },
    schedule_window: undefined,
    execution_plan: [step, step],
    funding_source_policy: { type: 'RuntimePolicy', value: undefined },
  },
});
const artifact = createAaaPlanArtifact({
  metadataBytes,
  runtime,
  aaaType: 'System',
  mutability: 'Mutable',
  programScale,
});
const base = {
  artifact,
  aaaId: 14n,
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
  runtimeApi: 'AaaSimulationApi_simulate_current_program',
  runtimeApiVersion: 1,
};

const suspendedRuntimeValue = {
  success: true,
  value: {
    status: { type: 'Suspended', value: undefined },
    cycle_nonce: 7n,
    attempt: 1,
    start_cursor: 0,
    continuation_cursor: 1,
    finalized_through: 0,
    cumulative_outcomes: {
      executed_steps: 3,
      skipped_conditions: 0,
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
        outcome: {
          type: 'Suspended',
          value: { type: 'FundingUnavailable', value: undefined },
        },
      },
    ],
  },
};
const suspendedOutcome = {
  status: 'Suspended',
  cycleNonce: 7n,
  attempt: 1,
  startCursor: 0,
  continuationCursor: 1,
  finalizedThrough: 0,
  cumulativeOutcomes: {
    executedSteps: 3,
    skippedConditions: 0,
    skippedResolution: 0,
    skippedFundingUnavailable: 0,
    failedSteps: 0,
  },
  steps: [
    { stepIndex: 0, outcome: { type: 'Executed' } },
    {
      stepIndex: 1,
      outcome: { type: 'Suspended', reason: 'FundingUnavailable' },
    },
  ],
  resultScale: encodeAaaRuntimeSimulationResult(
    metadataBytes,
    suspendedRuntimeValue,
  ),
};

test('runtime API result codec discovers metadata and preserves bounded evidence', () => {
  const { resultScale, ...expectedOutcome } = suspendedOutcome;
  assert.deepEqual(
    decodeAaaRuntimeSimulationResult(metadataBytes, resultScale),
    {
      success: true,
      outcome: expectedOutcome,
      resultScale,
    },
  );
  const rejectedScale = encodeAaaRuntimeSimulationResult(metadataBytes, {
    success: false,
    value: { type: 'ProgramMismatch', value: undefined },
  });
  assert.deepEqual(
    decodeAaaRuntimeSimulationResult(metadataBytes, rejectedScale),
    { success: false, error: 'ProgramMismatch', resultScale: rejectedScale },
  );
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
            AaaSimulationApi: {
              async simulate_current_program(...args) {
                observedArguments = args;
                return suspendedRuntimeValue;
              },
            },
          },
        },
      };
    },
  };

  const result = await runDeosAaaFinalizedSimulation(connection, {
    artifact,
    aaaId: 14n,
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

test('matching-Wasm gate binds runtime code, metadata, state, API, and plan identity', async () => {
  let observedRequest;
  const result = await runAaaMatchingWasmSimulation({
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

  assert.equal(result.pin.planId, artifact.planId);
  assert.equal(
    result.pin.runtimeCodeHash,
    '0x11c0e79b71c3976ccd0c02d1310e2516c08edc9d8b6f57ccd680d63a4d8e72da',
  );
  assert.equal(result.pin.metadataHash, artifact.metadataHash);
  assert.equal(result.pin.stateRoot, base.snapshot.stateRoot);
  assert.equal(observedRequest.aaaId, 14n);
  assert.equal(observedRequest.mode, 'CurrentContinuation');
  assert.equal(observedRequest.programScale, artifact.programScale);
  assert.equal(result.outcome.continuationCursor, 1);
});

test('provider cannot change any requested runtime or state dependency', async () => {
  await assert.rejects(
    runAaaMatchingWasmSimulation({
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
    runAaaMatchingWasmSimulation({
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
    runAaaMatchingWasmSimulation({
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
    runAaaMatchingWasmSimulation({
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
