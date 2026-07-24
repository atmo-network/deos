/*
Domain: AAA control-plane forecasting
Owns: State-pinned amount resolution, separated Weight/fee aggregation, and staleness provenance.
Excludes: Chain queries, adapter quote execution, state mutation, simulation, signing, and history persistence.
Zone: Automation domain capability; consumers must supply one coherent runtime/state snapshot.
*/
import type { AaaPlanArtifact, AaaPlanHex } from './plan-artifact';

export const PERBILL_DENOMINATOR = 1_000_000_000n;

export type AaaAmountResolution =
  | { type: 'Fixed'; value: bigint }
  | { type: 'AllBalance' }
  | {
      type:
        | 'PercentageOfCurrent'
        | 'PercentageOfTrigger'
        | 'PercentageOfLastFunding';
      parts: number;
    };

export type AaaAmountPolicy =
  | 'PreserveSpend'
  | 'ExpendableSpend'
  | 'Mint'
  | 'UnstakeShares';

export type AaaAmountObservation = {
  resolution: AaaAmountResolution;
  policy: AaaAmountPolicy;
  current: bigint;
  minimumBalance: bigint;
  reservedFee: bigint;
  isFeeNative: boolean;
  trigger?: bigint;
  lastFunding?: bigint;
};

export type AaaAmountForecast = {
  status: 'Resolved' | 'Skipped' | 'FundingUnavailable' | 'SnapshotUnavailable';
  amount: bigint | null;
  basis: bigint | null;
  spendLimit: bigint;
};

export type AaaWeight = {
  refTime: bigint;
  proofSize: bigint;
};

export type AaaForecastPin = {
  planId: AaaPlanHex;
  blockHash: AaaPlanHex;
  blockNumber: number;
  metadataHash: AaaPlanHex;
  model: string;
  modelVersion: string;
};

export type AaaStepCostInput = {
  stepIndex: number;
  conditionCount: number;
  conditionOutcome: 'Pass' | 'Fail' | 'Unknown';
  executionDisposition: 'Execute' | 'Skip' | 'Unknown';
  evaluationWeight: AaaWeight;
  executionWeightUpper: AaaWeight;
  executionFeeUpper: bigint;
};

export type AaaCostSegment = {
  weight: AaaWeight;
  fee: bigint;
};

export type AaaCostForecast = {
  scope: 'StaticAllStepsReached';
  pin: AaaForecastPin;
  evaluation: AaaCostSegment;
  executionMinimum: AaaCostSegment;
  executionUpper: AaaCostSegment;
  lifecycle: AaaCostSegment;
  totalMinimum: AaaCostSegment;
  totalUpper: AaaCostSegment;
  steps: Array<{
    stepIndex: number;
    conditionOutcome: AaaStepCostInput['conditionOutcome'];
    executionDisposition: AaaStepCostInput['executionDisposition'];
    evaluationFee: bigint;
    executionFeeMinimum: bigint;
    executionFeeUpper: bigint;
  }>;
};

function saturatingSubtract(value: bigint, subtract: bigint) {
  return value > subtract ? value - subtract : 0n;
}

function validateBalance(value: bigint, field: string) {
  if (value < 0n) throw new Error(`${field} must be non-negative`);
}

function percentage(parts: number, value: bigint) {
  if (!Number.isSafeInteger(parts) || parts < 0 || parts > 1_000_000_000) {
    throw new Error(
      'Perbill parts must be an integer between 0 and 1,000,000,000',
    );
  }
  return (BigInt(parts) * value) / PERBILL_DENOMINATOR;
}

export function resolveAaaAmount(
  input: AaaAmountObservation,
): AaaAmountForecast {
  validateBalance(input.current, 'current');
  validateBalance(input.minimumBalance, 'minimumBalance');
  validateBalance(input.reservedFee, 'reservedFee');
  if (input.trigger != null) validateBalance(input.trigger, 'trigger');
  if (input.lastFunding != null)
    validateBalance(input.lastFunding, 'lastFunding');

  const isShares = input.policy === 'UnstakeShares';
  const spendableCurrent = isShares
    ? input.current
    : saturatingSubtract(
        input.current,
        input.isFeeNative ? input.reservedFee : 0n,
      );
  const spendLimit =
    input.policy === 'PreserveSpend'
      ? saturatingSubtract(spendableCurrent, input.minimumBalance)
      : spendableCurrent;

  let basis: bigint | null = null;
  let amount: bigint;
  switch (input.resolution.type) {
    case 'Fixed':
      validateBalance(input.resolution.value, 'fixed amount');
      amount = input.resolution.value;
      break;
    case 'AllBalance':
      basis = isShares ? input.current : spendLimit;
      amount = basis;
      break;
    case 'PercentageOfCurrent':
      basis = isShares ? input.current : spendLimit;
      amount = percentage(input.resolution.parts, basis);
      if (input.resolution.parts !== 0 && basis !== 0n && amount === 0n) {
        return { status: 'Skipped', amount: null, basis, spendLimit };
      }
      break;
    case 'PercentageOfTrigger':
      if (input.trigger == null) {
        return {
          status: 'SnapshotUnavailable',
          amount: null,
          basis: null,
          spendLimit,
        };
      }
      basis = input.trigger;
      amount = percentage(input.resolution.parts, basis);
      if (input.resolution.parts !== 0 && basis !== 0n && amount === 0n) {
        return { status: 'Skipped', amount: null, basis, spendLimit };
      }
      break;
    case 'PercentageOfLastFunding':
      if (input.lastFunding == null || input.lastFunding === 0n) {
        return {
          status: 'FundingUnavailable',
          amount: null,
          basis: input.lastFunding ?? null,
          spendLimit,
        };
      }
      basis = input.lastFunding;
      amount = percentage(input.resolution.parts, basis);
      if (input.resolution.parts !== 0 && amount === 0n) {
        return { status: 'Skipped', amount: null, basis, spendLimit };
      }
      break;
  }

  if (amount === 0n)
    return { status: 'Skipped', amount: null, basis, spendLimit };
  if (input.policy !== 'Mint' && amount > spendLimit) {
    return {
      status: 'FundingUnavailable',
      amount: null,
      basis,
      spendLimit,
    };
  }
  return { status: 'Resolved', amount, basis, spendLimit };
}

function zeroWeight(): AaaWeight {
  return { refTime: 0n, proofSize: 0n };
}

function addWeight(left: AaaWeight, right: AaaWeight): AaaWeight {
  return {
    refTime: left.refTime + right.refTime,
    proofSize: left.proofSize + right.proofSize,
  };
}

function addSegment(
  left: AaaCostSegment,
  right: AaaCostSegment,
): AaaCostSegment {
  return {
    weight: addWeight(left.weight, right.weight),
    fee: left.fee + right.fee,
  };
}

function validateWeight(weight: AaaWeight, field: string) {
  validateBalance(weight.refTime, `${field}.refTime`);
  validateBalance(weight.proofSize, `${field}.proofSize`);
}

export function forecastAaaCosts(input: {
  artifact: AaaPlanArtifact;
  blockHash: AaaPlanHex;
  blockNumber: number;
  model: string;
  modelVersion: string;
  actorType: 'User' | 'System';
  stepBaseFee: bigint;
  conditionReadFee: bigint;
  steps: AaaStepCostInput[];
  lifecycle: AaaCostSegment;
}): AaaCostForecast {
  if (!Number.isSafeInteger(input.blockNumber) || input.blockNumber < 0) {
    throw new Error('blockNumber must be a non-negative safe integer');
  }
  validateBalance(input.stepBaseFee, 'stepBaseFee');
  validateBalance(input.conditionReadFee, 'conditionReadFee');
  validateWeight(input.lifecycle.weight, 'lifecycle.weight');
  validateBalance(input.lifecycle.fee, 'lifecycle.fee');

  let evaluation: AaaCostSegment = { weight: zeroWeight(), fee: 0n };
  let executionMinimum: AaaCostSegment = { weight: zeroWeight(), fee: 0n };
  let executionUpper: AaaCostSegment = { weight: zeroWeight(), fee: 0n };
  const steps = input.steps.map((step, index) => {
    if (step.stepIndex !== index)
      throw new Error('steps must use contiguous ordered indices');
    if (!Number.isSafeInteger(step.conditionCount) || step.conditionCount < 0) {
      throw new Error('conditionCount must be a non-negative safe integer');
    }
    validateWeight(step.evaluationWeight, `steps[${index}].evaluationWeight`);
    validateWeight(
      step.executionWeightUpper,
      `steps[${index}].executionWeightUpper`,
    );
    validateBalance(
      step.executionFeeUpper,
      `steps[${index}].executionFeeUpper`,
    );
    const evaluationFee =
      input.actorType === 'User'
        ? input.stepBaseFee +
          input.conditionReadFee * BigInt(step.conditionCount)
        : 0n;
    evaluation = addSegment(evaluation, {
      weight: step.evaluationWeight,
      fee: evaluationFee,
    });
    if (
      step.conditionOutcome === 'Fail' &&
      step.executionDisposition === 'Execute'
    ) {
      throw new Error('A condition-failed step cannot execute');
    }
    const mayExecute = step.executionDisposition !== 'Skip';
    const mustExecute = step.executionDisposition === 'Execute';
    if (mayExecute) {
      executionUpper = addSegment(executionUpper, {
        weight: step.executionWeightUpper,
        fee: input.actorType === 'User' ? step.executionFeeUpper : 0n,
      });
    }
    if (mustExecute) {
      executionMinimum = addSegment(executionMinimum, {
        weight: step.executionWeightUpper,
        fee: input.actorType === 'User' ? step.executionFeeUpper : 0n,
      });
    }
    return {
      stepIndex: step.stepIndex,
      conditionOutcome: step.conditionOutcome,
      executionDisposition: step.executionDisposition,
      evaluationFee,
      executionFeeMinimum:
        input.actorType === 'User' && mustExecute ? step.executionFeeUpper : 0n,
      executionFeeUpper:
        input.actorType === 'User' && mayExecute ? step.executionFeeUpper : 0n,
    };
  });

  const pin: AaaForecastPin = {
    planId: input.artifact.planId,
    blockHash: input.blockHash,
    blockNumber: input.blockNumber,
    metadataHash: input.artifact.metadataHash,
    model: input.model,
    modelVersion: input.modelVersion,
  };
  return {
    scope: 'StaticAllStepsReached',
    pin,
    evaluation,
    executionMinimum,
    executionUpper,
    lifecycle: input.lifecycle,
    totalMinimum: addSegment(
      addSegment(evaluation, executionMinimum),
      input.lifecycle,
    ),
    totalUpper: addSegment(
      addSegment(evaluation, executionUpper),
      input.lifecycle,
    ),
    steps,
  };
}

export function isAaaForecastStale(
  forecast: AaaCostForecast,
  current: {
    blockHash: AaaPlanHex;
    metadataHash: AaaPlanHex;
    model: string;
    modelVersion: string;
  },
) {
  return (
    forecast.pin.blockHash !== current.blockHash ||
    forecast.pin.metadataHash !== current.metadataHash ||
    forecast.pin.model !== current.model ||
    forecast.pin.modelVersion !== current.modelVersion
  );
}
