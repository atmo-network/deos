/*
Domain: Actors generated fee-envelope vectors
Owns: Validation and policy projection of package-generated fee-envelope conformance vectors.
Excludes: Runtime weight calculation, plan simulation, metadata decoding, and fee collection.
Zone: Automation domain contract; generated Rust vectors constrain browser fee forecasting.
*/
import vectorsJson from './actors-fee-envelope-vectors.json' with { type: 'json' };

export type ActorFeeEnvelopeActorType = 'User' | 'System';
export type ActorFeeChargeKind = 'EvaluationOnly' | 'Attempted';

type ActorFeeEnvelopeInput = {
  evaluation: string;
  execution: string;
};

type ActorFeeEnvelopeStep = ActorFeeEnvelopeInput & {
  total: string;
};

export type ActorFeeEnvelopeVector = {
  actorType: ActorFeeEnvelopeActorType;
  cursor: number;
  inputs: ActorFeeEnvelopeInput[];
  steps: ActorFeeEnvelopeStep[];
  total: string;
};

export type ActorFeeSettlementCase = {
  name: string;
  actorType: ActorFeeEnvelopeActorType;
  cursor: number;
  inputs: ActorFeeEnvelopeInput[];
  initialReservation: string;
  chargeKinds: ActorFeeChargeKind[];
  charges: string[];
  reservationRemaining: string[];
};

export type ActorFeeFloorCase = {
  name: string;
  actorType: ActorFeeEnvelopeActorType;
  isFeeNative: boolean;
  assetMinimum: string;
  minUserBalance: string;
  protectedMinimum: string;
};

export type ActorFeeEnvelopeVectors = {
  format: 'deos.actor.fee-envelope-vectors';
  formatVersion: 2;
  metadataSha256: string;
  weightSha256: string;
  vectors: ActorFeeEnvelopeVector[];
  settlementCases: ActorFeeSettlementCase[];
  floorCases: ActorFeeFloorCase[];
};

export type ActorFeeStepSettlement = {
  charge: bigint;
  reservationRemaining: bigint;
};

function record(value: unknown, label: string): Record<string, unknown> {
  if (value == null || Array.isArray(value) || typeof value !== 'object') {
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

function actorType(value: unknown, label: string): ActorFeeEnvelopeActorType {
  if (value === 'User' || value === 'System') return value;
  throw new Error(`${label} must be User or System`);
}

function chargeKind(value: unknown, label: string): ActorFeeChargeKind {
  if (value === 'EvaluationOnly' || value === 'Attempted') return value;
  throw new Error(`${label} must be EvaluationOnly or Attempted`);
}

function input(value: unknown, label: string): ActorFeeEnvelopeInput {
  const projected = record(value, label);
  decimal(projected.evaluation, `${label}.evaluation`);
  decimal(projected.execution, `${label}.execution`);
  return projected as ActorFeeEnvelopeInput;
}

function inputs(value: unknown, label: string): ActorFeeEnvelopeInput[] {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value.map((entry, index) => input(entry, `${label}[${index}]`));
}

function nonNegativeStrings(value: unknown, label: string): string[] {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value.map((entry, index) => {
    decimal(entry, `${label}[${index}]`);
    return entry as string;
  });
}

function cursor(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative safe integer`);
  }
  return value;
}

function vector(value: unknown, index: number): ActorFeeEnvelopeVector {
  const label = `fee-envelope vectors[${index}]`;
  const projected = record(value, label);
  const parsedInputs = inputs(projected.inputs, `${label}.inputs`);
  if (!Array.isArray(projected.steps)) {
    throw new Error(`${label}.steps must be an array`);
  }
  const parsed: ActorFeeEnvelopeVector = {
    actorType: actorType(projected.actorType, `${label}.actorType`),
    cursor: cursor(projected.cursor, `${label}.cursor`),
    inputs: parsedInputs,
    steps: projected.steps.map((entry, stepIndex) => {
      const step = record(entry, `${label}.steps[${stepIndex}]`);
      decimal(step.evaluation, `${label}.steps[${stepIndex}].evaluation`);
      decimal(step.execution, `${label}.steps[${stepIndex}].execution`);
      decimal(step.total, `${label}.steps[${stepIndex}].total`);
      return step as ActorFeeEnvelopeStep;
    }),
    total: typeof projected.total === 'string' ? projected.total : '',
  };
  const total = decimal(parsed.total, `${label}.total`);
  if (parsed.cursor > parsed.inputs.length) {
    throw new Error(`${label}.cursor exceeds inputs`);
  }
  if (parsed.steps.length !== parsed.inputs.length - parsed.cursor) {
    throw new Error(`${label} does not cover exactly one suffix`);
  }
  const summed = parsed.steps.reduce((sum, step, stepIndex) => {
    const source = parsed.inputs[parsed.cursor + stepIndex];
    const evaluation = decimal(
      step.evaluation,
      `${label}.steps[${stepIndex}].evaluation`,
    );
    const execution = decimal(
      step.execution,
      `${label}.steps[${stepIndex}].execution`,
    );
    const stepTotal = decimal(step.total, `${label}.steps[${stepIndex}].total`);
    const expectedEvaluation =
      parsed.actorType === 'User'
        ? decimal(source.evaluation, `${label}.inputs[${stepIndex}].evaluation`)
        : 0n;
    const expectedExecution =
      parsed.actorType === 'User'
        ? decimal(source.execution, `${label}.inputs[${stepIndex}].execution`)
        : 0n;
    if (
      evaluation !== expectedEvaluation ||
      execution !== expectedExecution ||
      stepTotal !== evaluation + execution
    ) {
      throw new Error(
        `${label}.steps[${stepIndex}] disagrees with package envelope semantics`,
      );
    }
    return sum + stepTotal;
  }, 0n);
  if (summed !== total) {
    throw new Error(`${label}.total disagrees with its checked suffix steps`);
  }
  return parsed;
}

export function settleActorFeeStep(
  actorType: ActorFeeEnvelopeActorType,
  reservation: bigint,
  fee: { evaluation: bigint; execution: bigint },
  kind: ActorFeeChargeKind,
): ActorFeeStepSettlement {
  if (reservation < 0n || fee.evaluation < 0n || fee.execution < 0n) {
    throw new Error('Actors fee settlement inputs must be non-negative');
  }
  if (actorType === 'System') {
    return { charge: 0n, reservationRemaining: 0n };
  }
  const total = fee.evaluation + fee.execution;
  if (reservation < total) {
    throw new Error('Actors fee reservation underflows the selected step');
  }
  return {
    charge: kind === 'EvaluationOnly' ? fee.evaluation : total,
    reservationRemaining: reservation - total,
  };
}

export function actorFeeStepCharge(
  actorType: ActorFeeEnvelopeActorType,
  evaluation: bigint,
  execution: bigint,
  kind: ActorFeeChargeKind,
): bigint {
  return settleActorFeeStep(
    actorType,
    evaluation + execution,
    { evaluation, execution },
    kind,
  ).charge;
}

export function actorUserFeeBudgetAdmits(
  feeNativeBalance: bigint,
  minUserBalance: bigint,
  attemptFeeUpper: bigint,
): boolean {
  if (feeNativeBalance < 0n || minUserBalance < 0n || attemptFeeUpper < 0n) {
    throw new Error('Actors fee-budget inputs must be non-negative');
  }
  return (
    feeNativeBalance >= minUserBalance &&
    feeNativeBalance - minUserBalance >= attemptFeeUpper
  );
}

export function actorFeeNativeProtectedMinimum(
  actorType: ActorFeeEnvelopeActorType,
  isFeeNative: boolean,
  assetMinimum: bigint,
  minUserBalance: bigint,
): bigint {
  if (assetMinimum < 0n || minUserBalance < 0n) {
    throw new Error('Actors protected-minimum inputs must be non-negative');
  }
  return actorType === 'User' && isFeeNative
    ? assetMinimum > minUserBalance
      ? assetMinimum
      : minUserBalance
    : assetMinimum;
}

function settlementCase(value: unknown, index: number): ActorFeeSettlementCase {
  const label = `fee-envelope settlement cases[${index}]`;
  const projected = record(value, label);
  const parsedInputs = inputs(projected.inputs, `${label}.inputs`);
  const parsedCursor = cursor(projected.cursor, `${label}.cursor`);
  if (parsedCursor > parsedInputs.length) {
    throw new Error(`${label}.cursor exceeds inputs`);
  }
  if (!Array.isArray(projected.chargeKinds)) {
    throw new Error(`${label}.chargeKinds must be an array`);
  }
  const chargeKinds = projected.chargeKinds.map((entry, entryIndex) =>
    chargeKind(entry, `${label}.chargeKinds[${entryIndex}]`),
  );
  const charges = nonNegativeStrings(projected.charges, `${label}.charges`);
  const reservationRemaining = nonNegativeStrings(
    projected.reservationRemaining,
    `${label}.reservationRemaining`,
  );
  if (
    chargeKinds.length !== parsedInputs.length - parsedCursor ||
    charges.length !== chargeKinds.length ||
    reservationRemaining.length !== chargeKinds.length
  ) {
    throw new Error(`${label} must cover exactly one suffix`);
  }
  if (typeof projected.name !== 'string' || projected.name.length === 0) {
    throw new Error(`${label}.name must be a non-empty string`);
  }
  const parsed: ActorFeeSettlementCase = {
    name: projected.name,
    actorType: actorType(projected.actorType, `${label}.actorType`),
    cursor: parsedCursor,
    inputs: parsedInputs,
    initialReservation:
      typeof projected.initialReservation === 'string'
        ? projected.initialReservation
        : '',
    chargeKinds,
    charges,
    reservationRemaining,
  };
  let reservation = decimal(
    parsed.initialReservation,
    `${label}.initialReservation`,
  );
  const expectedInitial = parsed.inputs
    .slice(parsed.cursor)
    .reduce(
      (total, source) =>
        parsed.actorType === 'User'
          ? total +
            decimal(source.evaluation, `${label}.inputs.evaluation`) +
            decimal(source.execution, `${label}.inputs.execution`)
          : total,
      0n,
    );
  if (reservation !== expectedInitial) {
    throw new Error(
      `${label}.initialReservation disagrees with package envelope`,
    );
  }
  parsed.chargeKinds.forEach((kind, stepIndex) => {
    const source = parsed.inputs[parsed.cursor + stepIndex];
    const settlement = settleActorFeeStep(
      parsed.actorType,
      reservation,
      {
        evaluation: decimal(
          source.evaluation,
          `${label}.inputs[${stepIndex}].evaluation`,
        ),
        execution: decimal(
          source.execution,
          `${label}.inputs[${stepIndex}].execution`,
        ),
      },
      kind,
    );
    if (
      settlement.charge !==
        decimal(parsed.charges[stepIndex], `${label}.charges[${stepIndex}]`) ||
      settlement.reservationRemaining !==
        decimal(
          parsed.reservationRemaining[stepIndex],
          `${label}.reservationRemaining[${stepIndex}]`,
        )
    ) {
      throw new Error(`${label} disagrees with package settlement semantics`);
    }
    reservation = settlement.reservationRemaining;
  });
  if (reservation !== 0n) {
    throw new Error(`${label} does not release its suffix reservation to zero`);
  }
  return parsed;
}

function floorCase(value: unknown, index: number): ActorFeeFloorCase {
  const label = `fee-envelope floor cases[${index}]`;
  const projected = record(value, label);
  if (typeof projected.name !== 'string' || projected.name.length === 0) {
    throw new Error(`${label}.name must be a non-empty string`);
  }
  if (typeof projected.isFeeNative !== 'boolean') {
    throw new Error(`${label}.isFeeNative must be boolean`);
  }
  const parsed: ActorFeeFloorCase = {
    name: projected.name,
    actorType: actorType(projected.actorType, `${label}.actorType`),
    isFeeNative: projected.isFeeNative,
    assetMinimum:
      typeof projected.assetMinimum === 'string' ? projected.assetMinimum : '',
    minUserBalance:
      typeof projected.minUserBalance === 'string'
        ? projected.minUserBalance
        : '',
    protectedMinimum:
      typeof projected.protectedMinimum === 'string'
        ? projected.protectedMinimum
        : '',
  };
  const expected = actorFeeNativeProtectedMinimum(
    parsed.actorType,
    parsed.isFeeNative,
    decimal(parsed.assetMinimum, `${label}.assetMinimum`),
    decimal(parsed.minUserBalance, `${label}.minUserBalance`),
  );
  if (
    expected !== decimal(parsed.protectedMinimum, `${label}.protectedMinimum`)
  ) {
    throw new Error(
      `${label} disagrees with package protected-minimum semantics`,
    );
  }
  return parsed;
}

function requireNames(
  values: Array<{ name: string }>,
  expected: string[],
  label: string,
): void {
  for (const name of expected) {
    if (!values.some((value) => value.name === name)) {
      throw new Error(`${label} omit ${name}`);
    }
  }
}

const SHA256_HEX = /^[0-9a-f]{64}$/;

function identity(value: unknown, label: string): string {
  if (typeof value !== 'string' || !SHA256_HEX.test(value)) {
    throw new Error(`${label} must be a 64-hex-char sha256 identity`);
  }
  return value;
}

export function parseActorFeeEnvelopeVectors(
  value: unknown,
): ActorFeeEnvelopeVectors {
  const projected = record(value, 'Actors fee-envelope vectors');
  if (projected.format !== 'deos.actor.fee-envelope-vectors') {
    throw new Error('Unsupported Actors fee-envelope vector format');
  }
  if (projected.formatVersion !== 2) {
    throw new Error('Unsupported Actors fee-envelope vector version');
  }
  const metadataSha256 = identity(
    projected.metadataSha256,
    'Actors fee-envelope metadata identity',
  );
  const weightSha256 = identity(
    projected.weightSha256,
    'Actors fee-envelope weight identity',
  );
  if (!Array.isArray(projected.vectors)) {
    throw new Error('Actors fee-envelope vectors must be an array');
  }
  if (!Array.isArray(projected.settlementCases)) {
    throw new Error('Actors fee-envelope settlement cases must be an array');
  }
  if (!Array.isArray(projected.floorCases)) {
    throw new Error('Actors fee-envelope floor cases must be an array');
  }
  const vectors = projected.vectors.map(vector);
  for (const expected of ['User', 'System'] as const) {
    if (
      !vectors.some(
        (candidate) =>
          candidate.actorType === expected && candidate.cursor === 0,
      )
    ) {
      throw new Error(
        `Actors fee-envelope vectors omit ${expected} cursor zero`,
      );
    }
  }
  const settlementCases = projected.settlementCases.map(settlementCase);
  requireNames(
    settlementCases,
    ['releaseToZero', 'attemptPricedRollback', 'systemFeeExemption'],
    'Actors fee-envelope settlement cases',
  );
  const floorCases = projected.floorCases.map(floorCase);
  requireNames(
    floorCases,
    ['userFeeNative', 'userNonFeeNative', 'systemFeeNative'],
    'Actors fee-envelope floor cases',
  );
  return {
    format: 'deos.actor.fee-envelope-vectors',
    formatVersion: 2,
    metadataSha256,
    weightSha256,
    vectors,
    settlementCases,
    floorCases,
  };
}

export const ACTORS_FEE_ENVELOPE_VECTORS =
  parseActorFeeEnvelopeVectors(vectorsJson);

const FEE_CHARGES_BY_ACTOR = new Map(
  ACTORS_FEE_ENVELOPE_VECTORS.vectors
    .filter((vector) => vector.cursor === 0)
    .map((vector) => [
      vector.actorType,
      vector.steps.some((step) => step.total !== '0'),
    ]),
);

export function actorFeeEnvelopeCharges(
  actorType: ActorFeeEnvelopeActorType,
): boolean {
  const charges = FEE_CHARGES_BY_ACTOR.get(actorType);
  if (charges == null) {
    throw new Error(`Actors fee-envelope vectors omit ${actorType} fee policy`);
  }
  return charges;
}
