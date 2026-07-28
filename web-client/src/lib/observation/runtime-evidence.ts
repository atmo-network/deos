/*
Domain: Typed observation inspection
Owns: Finalized runtime evidence comparison and verified fanout-budget projection.
Excludes: Chain transport, generated evidence production, observation state, and UI layout.
Zone: Observation domain capability.
*/
import {
  getDynamicBuilder,
  getLookupFn,
} from '@polkadot-api/metadata-builders';
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import { blake2AsHex } from '@polkadot/util-crypto';

import { DEOS_OBSERVATION_RUNTIME_EVIDENCE } from './runtime-evidence.generated.ts';
import type {
  ObservationFanoutBudget,
  ObservationFanoutEvidence,
} from './types.ts';

export type ObservationFinalizedRuntimeEvidence = {
  runtime: {
    specName: string;
    implName: string;
    authoringVersion: number;
    specVersion: number;
    implVersion: number;
    systemVersion: number;
    transactionVersion: number;
  };
  runtimeCodeBytes: Uint8Array;
  metadataBytes: Uint8Array;
};

const EXPECTED = DEOS_OBSERVATION_RUNTIME_EVIDENCE;

export type ObservationRuntimeEvidenceIdentity = Omit<
  ObservationFinalizedRuntimeEvidence,
  'runtimeCodeBytes' | 'metadataBytes'
> & {
  runtimeCodeHash: string;
  metadataHash: string;
  fanout: {
    configuredServiceUnitsPerBlock: number;
    fanoutWeightLimit: {
      refTime: bigint;
      proofSize: bigint;
    };
    maxActiveActors: number;
    maxTriggerSources: number;
    queuePageSize: number;
  };
};

function runtimeIdentity(value: ObservationRuntimeEvidenceIdentity) {
  return `${value.runtime.specName}@spec-${value.runtime.specVersion} · code:${value.runtimeCodeHash} · metadata:${value.metadataHash}`;
}

export function expectedObservationFanoutBudget(): ObservationFanoutBudget {
  return {
    runtimeIdentity: `${EXPECTED.runtime.specName}@spec-${EXPECTED.runtime.specVersion} · code:${EXPECTED.runtimeCodeHash} · metadata:${EXPECTED.metadataHash} · descriptors:${EXPECTED.descriptorIdentity}`,
    weightIdentity: EXPECTED.weightIdentity,
    maxServiceUnitsPerBlock: EXPECTED.fanout.maxServiceUnitsPerBlock,
    maxActiveDirtyFeeds: EXPECTED.fanout.maxActiveDirtyFeeds,
    maxSubscriberPagesPerFeed: EXPECTED.fanout.maxSubscriberPagesPerFeed,
  };
}

export function compareObservationRuntimeEvidenceIdentity(
  observed: ObservationRuntimeEvidenceIdentity,
): ObservationFanoutEvidence {
  const reasons: string[] = [];
  for (const [field, expected, actual] of [
    ['spec name', EXPECTED.runtime.specName, observed.runtime.specName],
    [
      'implementation name',
      EXPECTED.runtime.implName,
      observed.runtime.implName,
    ],
    [
      'authoring version',
      EXPECTED.runtime.authoringVersion,
      observed.runtime.authoringVersion,
    ],
    [
      'spec version',
      EXPECTED.runtime.specVersion,
      observed.runtime.specVersion,
    ],
    [
      'implementation version',
      EXPECTED.runtime.implVersion,
      observed.runtime.implVersion,
    ],
    [
      'system version',
      EXPECTED.runtime.systemVersion,
      observed.runtime.systemVersion,
    ],
    [
      'transaction version',
      EXPECTED.runtime.transactionVersion,
      observed.runtime.transactionVersion,
    ],
  ] as const) {
    if (actual !== expected) reasons.push(`${field} mismatch`);
  }
  if (observed.runtimeCodeHash !== EXPECTED.runtimeCodeHash) {
    reasons.push('runtime code mismatch');
  }
  if (observed.metadataHash !== EXPECTED.metadataHash) {
    reasons.push('V16 metadata mismatch');
  }
  if (
    observed.fanout.configuredServiceUnitsPerBlock !==
    EXPECTED.fanout.configuredServiceUnitsPerBlock
  ) {
    reasons.push('configured fanout service-unit bound mismatch');
  }
  if (
    observed.fanout.fanoutWeightLimit.refTime !==
      BigInt(EXPECTED.fanout.fanoutWeightLimit.refTime) ||
    observed.fanout.fanoutWeightLimit.proofSize !==
      BigInt(EXPECTED.fanout.fanoutWeightLimit.proofSize)
  ) {
    reasons.push('fanout Weight limit mismatch');
  }
  const maxActiveDirtyFeeds =
    observed.fanout.maxActiveActors * observed.fanout.maxTriggerSources;
  if (maxActiveDirtyFeeds !== EXPECTED.fanout.maxActiveDirtyFeeds) {
    reasons.push('active dirty-feed bound mismatch');
  }
  const maxSubscriberPagesPerFeed = Math.ceil(
    observed.fanout.maxActiveActors / observed.fanout.queuePageSize,
  );
  if (maxSubscriberPagesPerFeed !== EXPECTED.fanout.maxSubscriberPagesPerFeed) {
    reasons.push('subscriber-page bound mismatch');
  }
  const observedIdentity = runtimeIdentity(observed);
  return reasons.length === 0
    ? { status: 'Verified', observedIdentity }
    : { status: 'EvidenceMismatch', observedIdentity, reasons };
}

function metadataBytes(value: string) {
  if (!/^0x(?:[0-9a-f]{2})+$/i.test(value)) {
    throw new Error('Runtime constant must contain SCALE hex bytes');
  }
  return Uint8Array.from(
    value
      .slice(2)
      .match(/../g)!
      .map((byte) => Number(`0x${byte}`)),
  );
}

function decodeFanoutConstants(bytes: Uint8Array) {
  const metadata = unifyMetadata(decAnyMetadata(bytes));
  const aaaPallets = metadata.pallets.filter((pallet) => pallet.name === 'AAA');
  if (aaaPallets.length !== 1) {
    throw new Error('V16 metadata must expose exactly one AAA pallet');
  }
  const builder = getDynamicBuilder(getLookupFn(metadata));
  const constant = (name: string) => {
    const matches = aaaPallets[0].constants.filter(
      (candidate) => candidate.name === name,
    );
    if (matches.length !== 1) {
      throw new Error(`V16 metadata must expose exactly one AAA.${name}`);
    }
    return builder
      .buildDefinition(matches[0].type)
      .dec(metadataBytes(matches[0].value));
  };
  const numberConstant = (name: string) => {
    const value = constant(name);
    if (!Number.isSafeInteger(value) || (value as number) < 0) {
      throw new Error(`AAA.${name} must decode as a bounded integer`);
    }
    return value as number;
  };
  const weight = constant('ObservationFanoutWeightLimit');
  if (
    weight === null ||
    typeof weight !== 'object' ||
    typeof (weight as { ref_time?: unknown }).ref_time !== 'bigint' ||
    typeof (weight as { proof_size?: unknown }).proof_size !== 'bigint'
  ) {
    throw new Error('AAA.ObservationFanoutWeightLimit must decode as Weight');
  }
  return {
    configuredServiceUnitsPerBlock: numberConstant(
      'MaxObservationFanoutPagesPerBlock',
    ),
    fanoutWeightLimit: {
      refTime: (weight as { ref_time: bigint }).ref_time,
      proofSize: (weight as { proof_size: bigint }).proof_size,
    },
    maxActiveActors: numberConstant('MaxActiveActors'),
    maxTriggerSources: numberConstant('MaxTriggerSources'),
    queuePageSize: numberConstant('QueuePageSize'),
  };
}

export function compareObservationRuntimeEvidence(
  observed: ObservationFinalizedRuntimeEvidence,
): ObservationFanoutEvidence {
  return compareObservationRuntimeEvidenceIdentity({
    runtime: observed.runtime,
    runtimeCodeHash: blake2AsHex(observed.runtimeCodeBytes, 256),
    metadataHash: blake2AsHex(observed.metadataBytes, 256),
    fanout: decodeFanoutConstants(observed.metadataBytes),
  });
}
