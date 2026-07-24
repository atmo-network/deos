/*
Domain: DEOS AAA finalized-state simulation transport
Owns: Finalized block selection, state-pinned metadata/runtime code, typed runtime API invocation, and provider evidence assembly.
Excludes: Local Wasm verification, state-proof verification, signing, submission, artifact authoring, and outcome synthesis.
Zone: Blockchain adapter capability; imports automation contracts and the low-level DEOS connection only.
*/
import type { HexString } from 'polkadot-api';

import type { DeosClient, DeosPapiConnection, DeosTypedApi } from './deos.ts';

import {
  type AaaMatchingWasmResponse,
  runAaaMatchingWasmSimulation,
} from '../../automation/matching-wasm.ts';
import {
  type AaaPlanArtifact,
  type AaaPlanHex,
  type AaaPlanRuntimeIdentity,
  inspectAaaPlanArtifact,
} from '../../automation/plan-artifact.ts';
import {
  AAA_SIMULATION_RUNTIME_API,
  AAA_SIMULATION_RUNTIME_API_VERSION,
  decodeAaaRuntimeSimulationResult,
  encodeAaaRuntimeSimulationResult,
} from '../../automation/runtime-simulation-codec.ts';

const RUNTIME_CODE_STORAGE_KEY = '0x3a636f6465' as HexString;
const METADATA_VERSION = 16;
const HEX_PATTERN = /^0x(?:[0-9a-fA-F]{2})+$/;
const PLAN_HEX_PATTERN = /^0x(?:[0-9a-f]{2})+$/;

export type AaaFinalizedSimulationMode =
  | 'FreshCurrentPlan'
  | 'CurrentContinuation';

export type AaaFinalizedSimulationInput = {
  artifact: AaaPlanArtifact;
  aaaId: bigint;
  mode: AaaFinalizedSimulationMode;
  finalizedBlock?: { hash: AaaPlanHex; number: number };
};

type DeosSimulationConnection = Pick<DeosPapiConnection, 'ensureConnected'>;
type RuntimeVersion = Awaited<
  ReturnType<DeosTypedApi['apis']['Core']['version']>
>;
type RuntimeSimulationProgram = Parameters<
  DeosTypedApi['apis']['AaaSimulationApi']['simulate_current_program']
>[3];

function asPlanHex(value: string, field: string): AaaPlanHex {
  if (!PLAN_HEX_PATTERN.test(value)) {
    throw new Error(`${field} must contain canonical lowercase hex bytes`);
  }
  return value as AaaPlanHex;
}

function hexToBytes(value: string, field: string): Uint8Array {
  if (!HEX_PATTERN.test(value)) {
    throw new Error(`${field} must contain canonical hex bytes`);
  }
  const bytes = new Uint8Array((value.length - 2) / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(
      value.slice(2 + index * 2, 4 + index * 2),
      16,
    );
  }
  return bytes;
}

function runtimeIdentity(
  genesisHash: string,
  version: RuntimeVersion,
): AaaPlanRuntimeIdentity {
  if (!Number.isSafeInteger(version.spec_version)) {
    throw new Error('Finalized runtime spec_version is not a safe integer');
  }
  if (!Number.isSafeInteger(version.transaction_version)) {
    throw new Error(
      'Finalized runtime transaction_version is not a safe integer',
    );
  }
  return {
    genesisHash: asPlanHex(genesisHash, 'genesisHash'),
    specVersion: version.spec_version,
    transactionVersion: version.transaction_version,
  };
}

async function finalizedRuntimeContext(
  client: DeosClient,
  typedApi: DeosTypedApi,
  selectedBlock?: AaaFinalizedSimulationInput['finalizedBlock'],
) {
  const finalizedHead = await client.getFinalizedBlock();
  const finalizedBlock = selectedBlock ?? finalizedHead;
  if (
    selectedBlock != null &&
    selectedBlock.number !== 0 &&
    (selectedBlock.number !== finalizedHead.number ||
      selectedBlock.hash !== finalizedHead.hash)
  ) {
    throw new Error(
      'Explicit simulation block must be genesis or the current finalized head',
    );
  }
  const at = finalizedBlock.hash as HexString;
  const [header, chainSpec, version, metadata, runtimeCodeHex] =
    await Promise.all([
      client.getBlockHeader(at),
      client.getChainSpecData(),
      typedApi.apis.Core.version({ at }),
      typedApi.apis.Metadata.metadata_at_version(METADATA_VERSION, { at }),
      client._request<string | null>('state_getStorage', [
        RUNTIME_CODE_STORAGE_KEY,
        at,
      ]),
    ]);
  if (
    selectedBlock?.number === 0 &&
    selectedBlock.hash !== chainSpec.genesisHash
  ) {
    throw new Error('Explicit genesis fixture hash does not match the chain');
  }
  if (!(metadata instanceof Uint8Array)) {
    throw new Error('Finalized runtime does not expose V16 metadata');
  }
  if (runtimeCodeHex == null) {
    throw new Error('Finalized state does not expose runtime :code');
  }
  return {
    at,
    blockNumber: finalizedBlock.number,
    stateRoot: header.stateRoot,
    metadataBytes: metadata,
    runtimeCodeBytes: hexToBytes(runtimeCodeHex, 'runtime :code'),
    runtime: runtimeIdentity(chainSpec.genesisHash, version),
  };
}

async function executeSimulation(
  typedApi: DeosTypedApi,
  at: HexString,
  request: {
    aaaId: bigint;
    aaaType: AaaPlanArtifact['aaaType'];
    mutability: AaaPlanArtifact['mutability'];
    runtimeProgram: RuntimeSimulationProgram;
    mode: AaaFinalizedSimulationMode;
  },
) {
  return typedApi.apis.AaaSimulationApi.simulate_current_program(
    request.aaaId,
    { type: request.aaaType, value: undefined },
    { type: request.mutability, value: undefined },
    request.runtimeProgram,
    { type: request.mode, value: undefined },
    { at },
  );
}

export async function runDeosAaaFinalizedSimulation(
  connection: DeosSimulationConnection,
  input: AaaFinalizedSimulationInput,
): Promise<AaaMatchingWasmResponse> {
  const { client, typedApi } = await connection.ensureConnected();
  const context = await finalizedRuntimeContext(
    client,
    typedApi,
    input.finalizedBlock,
  );

  return runAaaMatchingWasmSimulation({
    artifact: input.artifact,
    aaaId: input.aaaId,
    mode: input.mode,
    metadataBytes: context.metadataBytes,
    runtime: context.runtime,
    runtimeCodeBytes: context.runtimeCodeBytes,
    snapshot: {
      blockHash: asPlanHex(context.at, 'finalized block hash'),
      blockNumber: context.blockNumber,
      stateRoot: asPlanHex(context.stateRoot, 'finalized state root'),
      stateSource: 'FinalizedBlock',
    },
    runtimeApi: AAA_SIMULATION_RUNTIME_API,
    runtimeApiVersion: AAA_SIMULATION_RUNTIME_API_VERSION,
    provider: {
      async simulate(request) {
        const inspection = inspectAaaPlanArtifact(
          input.artifact,
          context.metadataBytes,
          context.runtime,
        );
        if (!inspection.valid) {
          throw new Error(
            `Invalid finalized AAA artifact: ${inspection.errors.join('; ')}`,
          );
        }
        const runtimeResult = await executeSimulation(typedApi, context.at, {
          aaaId: request.aaaId,
          aaaType: request.aaaType,
          mutability: request.mutability,
          runtimeProgram: inspection.runtimeValue as RuntimeSimulationProgram,
          mode: request.mode,
        });
        const resultScale = encodeAaaRuntimeSimulationResult(
          context.metadataBytes,
          runtimeResult,
        );
        const decoded = decodeAaaRuntimeSimulationResult(
          context.metadataBytes,
          resultScale,
        );
        if (!decoded.success) {
          throw new Error(`Runtime simulation rejected: ${decoded.error}`);
        }
        return {
          engine: 'RuntimeWasm',
          pin: request.pin,
          outcome: { ...decoded.outcome, resultScale },
        };
      },
    },
  });
}
