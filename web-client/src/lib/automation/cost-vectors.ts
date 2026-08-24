/*
Domain: Actors generated cost vectors
Owns: Fail-closed projection of production-runtime ActorCostApi fixtures across geometry and Trigger families.
Excludes: Runtime API transport, live fee estimation, Action receipts, and historical indexing.
Zone: Automation domain contract; runtime-generated vectors bind browser cost names to metadata and Weight identities.
*/
import vectorsJson from './actors-cost-vectors.json' with { type: 'json' };
import type {
  ActorCostView,
  ActorCostWeight,
  ActorTriggerFeeView,
} from './cost.ts';

export type ActorCostVector = {
  name: string;
  actorId: bigint;
  contractStepCount: number | null;
  triggerFamily: ActorTriggerFeeView['family'] | null;
  quote: ActorCostView;
};

export type ActorCostVectors = {
  format: 'deos.actor.cost-vectors';
  formatVersion: 1;
  runtimeApiVersion: 1;
  metadataSha256: string;
  weightSha256: string;
  vectors: ActorCostVector[];
};

const TRIGGER_FAMILIES = new Set<ActorTriggerFeeView['family']>([
  'Manual',
  'AddressEvent',
  'ObservationChange',
  'ObservationCrossing',
  'AtTime',
  'Cadenced',
]);

function record(value: unknown, label: string): Record<string, unknown> {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function decimal(value: unknown, label: string): bigint {
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)$/.test(value)) {
    throw new Error(`${label} must be a non-negative decimal string`);
  }
  return BigInt(value);
}

function digest(value: unknown, label: string): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`${label} must be a lowercase SHA-256 identity`);
  }
  return value;
}

function identity(value: unknown, label: string): string {
  return `0x${digest(value, label)}`;
}

function weight(value: unknown, label: string): ActorCostWeight {
  const projected = record(value, label);
  return {
    refTime: decimal(projected.refTime, `${label}.refTime`),
    proofSize: decimal(projected.proofSize, `${label}.proofSize`),
  };
}

function triggerFamily(
  value: unknown,
  label: string,
): ActorTriggerFeeView['family'] {
  if (typeof value === 'string' && TRIGGER_FAMILIES.has(value as never)) {
    return value as ActorTriggerFeeView['family'];
  }
  throw new Error(`${label} must be a supported Trigger family`);
}

function quote(value: unknown, label: string): ActorCostView {
  const source = record(value, label);
  if (source.actorType !== 'User' && source.actorType !== 'System') {
    throw new Error(`${label}.actorType must be User or System`);
  }
  const trigger =
    source.prospectiveTriggerFee == null
      ? null
      : (() => {
          const candidate = record(
            source.prospectiveTriggerFee,
            `${label}.prospectiveTriggerFee`,
          );
          return {
            family: triggerFamily(
              candidate.family,
              `${label}.prospectiveTriggerFee.family`,
            ),
            maximumWeight: weight(
              candidate.maximumWeight,
              `${label}.prospectiveTriggerFee.maximumWeight`,
            ),
            fee: decimal(candidate.fee, `${label}.prospectiveTriggerFee.fee`),
            productionWeightIdentity: identity(
              candidate.productionWeightIdentity,
              `${label}.prospectiveTriggerFee.productionWeightIdentity`,
            ),
          } satisfies ActorCostView['prospectiveTriggerFee'];
        })();
  const pipeline =
    source.prospectivePipelineFee == null
      ? null
      : (() => {
          const candidate = record(
            source.prospectivePipelineFee,
            `${label}.prospectivePipelineFee`,
          );
          if (candidate.strategy !== 'UpfrontBounded') {
            throw new Error(
              `${label}.prospectivePipelineFee strategy is unsupported`,
            );
          }
          const machineFee = decimal(
            candidate.machineFee,
            `${label}.prospectivePipelineFee.machineFee`,
          );
          const cleanupFee = decimal(
            candidate.cleanupFee,
            `${label}.prospectivePipelineFee.cleanupFee`,
          );
          const totalFee = decimal(
            candidate.totalFee,
            `${label}.prospectivePipelineFee.totalFee`,
          );
          if (machineFee + cleanupFee !== totalFee) {
            throw new Error(
              `${label}.prospectivePipelineFee total is inconsistent`,
            );
          }
          return {
            machineFee,
            cleanupFee,
            totalFee,
            strategy: 'UpfrontBounded' as const,
            admissionIdentity: identity(
              candidate.admissionIdentity,
              `${label}.prospectivePipelineFee.admissionIdentity`,
            ),
            productionWeightIdentity: identity(
              candidate.productionWeightIdentity,
              `${label}.prospectivePipelineFee.productionWeightIdentity`,
            ),
          };
        })();
  const actionSource = record(
    source.maximumNextActionFee,
    `${label}.maximumNextActionFee`,
  );
  const holdSource = record(source.stateHold, `${label}.stateHold`);
  if (typeof holdSource.exempt !== 'boolean') {
    throw new Error(`${label}.stateHold.exempt must be boolean`);
  }
  const componentsSource = record(
    holdSource.components,
    `${label}.stateHold.components`,
  );
  const components = {
    identity: decimal(componentsSource.identity, `${label}.stateHold.identity`),
    contractHead: decimal(
      componentsSource.contractHead,
      `${label}.stateHold.contractHead`,
    ),
    contractBody: decimal(
      componentsSource.contractBody,
      `${label}.stateHold.contractBody`,
    ),
    detector: decimal(componentsSource.detector, `${label}.stateHold.detector`),
    funding: decimal(componentsSource.funding, `${label}.stateHold.funding`),
    run: decimal(componentsSource.run, `${label}.stateHold.run`),
  };
  const holdTotal = decimal(holdSource.total, `${label}.stateHold.total`);
  if (
    Object.values(components).reduce(
      (total, component) => total + component,
      0n,
    ) !== holdTotal
  ) {
    throw new Error(`${label}.stateHold total is inconsistent`);
  }
  const projected: ActorCostView = {
    actorType: source.actorType,
    creationFee: decimal(source.creationFee, `${label}.creationFee`),
    prospectiveTriggerFee: trigger,
    prospectivePipelineFee: pipeline,
    maximumNextActionFee: {
      maximumEffectWeight: weight(
        actionSource.maximumEffectWeight,
        `${label}.maximumNextActionFee.maximumEffectWeight`,
      ),
      maximumEffectFee: decimal(
        actionSource.maximumEffectFee,
        `${label}.maximumNextActionFee.maximumEffectFee`,
      ),
      productionWeightIdentity: identity(
        actionSource.productionWeightIdentity,
        `${label}.maximumNextActionFee.productionWeightIdentity`,
      ),
    },
    stateHold: {
      exempt: holdSource.exempt,
      basePerComponent: decimal(
        holdSource.basePerComponent,
        `${label}.stateHold.basePerComponent`,
      ),
      perEncodedByte: decimal(
        holdSource.perEncodedByte,
        `${label}.stateHold.perEncodedByte`,
      ),
      components,
      total: holdTotal,
    },
  };
  if (projected.actorType === 'System') {
    const systemFees = [
      projected.creationFee,
      projected.prospectiveTriggerFee?.fee ?? 0n,
      projected.prospectivePipelineFee?.totalFee ?? 0n,
      projected.maximumNextActionFee.maximumEffectFee,
      projected.stateHold.total,
    ];
    if (!projected.stateHold.exempt || systemFees.some((fee) => fee !== 0n)) {
      throw new Error(`${label} violates System fee and hold exemption`);
    }
  } else if (projected.stateHold.exempt) {
    throw new Error(`${label} cannot exempt a User state hold`);
  }
  return projected;
}

function optionalStepCount(value: unknown, label: string): number | null {
  if (value === null) return null;
  if (
    typeof value !== 'number' ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > 32
  ) {
    throw new Error(`${label} must be null or an admitted Step count`);
  }
  return value;
}

function vector(value: unknown, index: number): ActorCostVector {
  const label = `Actor cost vectors[${index}]`;
  const source = record(value, label);
  if (typeof source.name !== 'string' || source.name.length === 0) {
    throw new Error(`${label}.name must be non-empty`);
  }
  const family =
    source.triggerFamily == null
      ? null
      : triggerFamily(source.triggerFamily, `${label}.triggerFamily`);
  const projected = quote(source.quote, `${label}.quote`);
  if ((projected.prospectiveTriggerFee?.family ?? null) !== family) {
    throw new Error(`${label} Trigger family disagrees with its quote`);
  }
  return {
    name: source.name,
    actorId: decimal(source.actorId, `${label}.actorId`),
    contractStepCount: optionalStepCount(
      source.contractStepCount,
      `${label}.contractStepCount`,
    ),
    triggerFamily: family,
    quote: projected,
  };
}

export function parseActorCostVectors(value: unknown): ActorCostVectors {
  const source = record(value, 'Actor cost vector artifact');
  if (
    source.format !== 'deos.actor.cost-vectors' ||
    source.formatVersion !== 1 ||
    source.runtimeApiVersion !== 1
  ) {
    throw new Error('Actor cost vector identity or version is unsupported');
  }
  if (!Array.isArray(source.vectors)) {
    throw new Error('Actor cost vectors must be an array');
  }
  const vectors = source.vectors.map(vector);
  if (new Set(vectors.map((entry) => entry.name)).size !== vectors.length) {
    throw new Error('Actor cost vector names must be unique');
  }
  const geometry = vectors
    .filter(
      (entry) =>
        entry.quote.actorType === 'User' && entry.triggerFamily === 'Manual',
    )
    .map((entry) => entry.contractStepCount)
    .filter((count): count is number => count !== null)
    .sort((left, right) => left - right);
  if (geometry.join(',') !== '0,1,4,8,32') {
    throw new Error('Actor cost vectors must cover Manual 0/1/4/8/32 geometry');
  }
  const families = new Set(
    vectors
      .filter(
        (entry) =>
          entry.quote.actorType === 'User' && entry.contractStepCount === 1,
      )
      .map((entry) => entry.triggerFamily)
      .filter(
        (family): family is ActorTriggerFeeView['family'] => family !== null,
      ),
  );
  if ([...TRIGGER_FAMILIES].some((family) => !families.has(family))) {
    throw new Error('Actor cost vectors must cover every Trigger family');
  }
  if (
    !vectors.some(
      (entry) =>
        entry.quote.actorType === 'System' && entry.contractStepCount === 1,
    ) ||
    !vectors.some(
      (entry) =>
        entry.quote.actorType === 'User' && entry.contractStepCount === null,
    )
  ) {
    throw new Error(
      'Actor cost vectors must cover System and dormant User semantics',
    );
  }
  return {
    format: source.format,
    formatVersion: source.formatVersion,
    runtimeApiVersion: source.runtimeApiVersion,
    metadataSha256: digest(source.metadataSha256, 'metadata identity'),
    weightSha256: digest(source.weightSha256, 'Weight identity'),
    vectors,
  };
}

export const ACTORS_COST_VECTORS = parseActorCostVectors(vectorsJson);
