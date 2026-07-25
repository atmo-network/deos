/*
Domain: AAA linear plan authoring
Owns: Typed step drafts, immutable ordered-step operations, structural validation, exact ProgramInput lowering, and canonical artifact production.
Excludes: Runtime submission, governance authority, adapter execution, simulation, weight modeling, recipes, and widget state.
Zone: Automation domain capability; composes the canonical plan-artifact codec without defining another runtime language.
*/
import { decodeAddress } from '@polkadot/util-crypto';

import {
  type AaaPlanArtifact,
  type AaaPlanRuntimeIdentity,
  type AaaPlanType,
  createAaaPlanArtifact,
  encodeAaaProgramValue,
} from './plan-artifact.ts';
import type {
  AutomationMutability,
  AutomationStepErrorPolicy,
} from './types.ts';

export type AaaAuthoringAsset =
  | { type: 'Native' }
  | { type: 'Local' | 'Foreign'; id: number };

export type AaaAuthoringAmount =
  | { type: 'Fixed'; value: string }
  | {
      type:
        | 'PercentageOfCurrent'
        | 'PercentageOfTrigger'
        | 'PercentageOfLastFunding';
      parts: number;
    }
  | { type: 'AllBalance' };

export type AaaAuthoringCondition =
  | {
      type:
        | 'BalanceAbove'
        | 'BalanceBelow'
        | 'BalanceEquals'
        | 'BalanceNotEquals';
      asset: AaaAuthoringAsset;
      threshold: string;
    }
  | {
      type: 'BlockNumberAbove' | 'BlockNumberBelow';
      threshold: number;
    };

export type AaaAuthoringTask =
  | {
      type: 'Transfer';
      to: string;
      asset: AaaAuthoringAsset;
      amount: AaaAuthoringAmount;
    }
  | {
      type: 'SplitTransfer';
      asset: AaaAuthoringAsset;
      amount: AaaAuthoringAmount;
      legs: Array<{ to: string; shareParts: number }>;
    }
  | {
      type: 'SwapExactIn';
      assetIn: AaaAuthoringAsset;
      assetOut: AaaAuthoringAsset;
      amountIn: AaaAuthoringAmount;
      slippageParts: number;
    }
  | {
      type: 'SwapExactOut';
      assetIn: AaaAuthoringAsset;
      assetOut: AaaAuthoringAsset;
      amountOut: AaaAuthoringAmount;
      slippageParts: number;
    }
  | {
      type: 'AddLiquidity';
      assetA: AaaAuthoringAsset;
      assetB: AaaAuthoringAsset;
      amountA: AaaAuthoringAmount;
      amountB: AaaAuthoringAmount;
    }
  | {
      type: 'RemoveLiquidity';
      lpAsset: AaaAuthoringAsset;
      amount: AaaAuthoringAmount;
    }
  | {
      type: 'Burn' | 'Mint' | 'Stake';
      asset: AaaAuthoringAsset;
      amount: AaaAuthoringAmount;
    }
  | {
      type: 'DonateLiquidity';
      assetA: AaaAuthoringAsset;
      assetB: AaaAuthoringAsset;
      amount: AaaAuthoringAmount;
      maxRatioErrorParts: number;
    }
  | {
      type: 'Unstake';
      asset: AaaAuthoringAsset;
      shares: AaaAuthoringAmount;
    }
  | { type: 'StopCycle' };

export const AAA_AUTHORING_TASK_TYPES = [
  'Transfer',
  'SplitTransfer',
  'SwapExactIn',
  'SwapExactOut',
  'AddLiquidity',
  'RemoveLiquidity',
  'Burn',
  'Mint',
  'Stake',
  'DonateLiquidity',
  'Unstake',
  'StopCycle',
] as const satisfies readonly AaaAuthoringTask['type'][];

export const AAA_AUTHORING_CONDITION_TYPES = [
  'BalanceAbove',
  'BalanceBelow',
  'BalanceEquals',
  'BalanceNotEquals',
  'BlockNumberAbove',
  'BlockNumberBelow',
] as const satisfies readonly AaaAuthoringCondition['type'][];

export type AaaAuthoringConditionSet =
  | { type: 'Always' }
  | { type: 'All' | 'Any'; conditions: AaaAuthoringCondition[] };

export type AaaAuthoringStep = {
  key: string;
  conditionSet: AaaAuthoringConditionSet;
  task: AaaAuthoringTask;
  errorPolicy: AutomationStepErrorPolicy;
};

export type AaaAuthoringFundingPolicy =
  | { type: 'OwnerOnly' }
  | { type: 'RuntimePolicy' }
  | { type: 'AnySource' }
  | { type: 'SignedAllowlist'; accounts: string[] };

export type AaaAuthoringTrigger =
  | { type: 'Manual' }
  | { type: 'Timer'; everyBlocks: number }
  | {
      type: 'OnAddressEvent';
      sourceFilter:
        | { type: 'Any' }
        | { type: 'OwnerOnly' }
        | { type: 'Whitelist'; accounts: string[] };
      assetFilter:
        | { type: 'Any' }
        | { type: 'Whitelist'; assets: AaaAuthoringAsset[] };
    };

export type AaaAuthoringProgram = {
  aaaType: AaaPlanType;
  mutability: AutomationMutability;
  trigger: AaaAuthoringTrigger;
  cooldownBlocks: number;
  scheduleWindow: { start: number; end: number } | null;
  fundingPolicy: AaaAuthoringFundingPolicy;
  steps: AaaAuthoringStep[];
};

export type AaaAuthoringLimits = {
  maxUserSteps: number;
  maxSystemSteps: number;
  maxConditionsPerStep: number;
  maxSplitTransferLegs: number;
  maxWhitelistSize: number;
};

export const DEOS_AAA_AUTHORING_LIMITS: AaaAuthoringLimits = {
  maxUserSteps: 3,
  maxSystemSteps: 10,
  maxConditionsPerStep: 4,
  maxSplitTransferLegs: 8,
  maxWhitelistSize: 16,
};

export type AaaAuthoringIssue = {
  path: string;
  message: string;
};

export type AaaAuthoringValidation =
  | { valid: true; issues: [] }
  | { valid: false; issues: AaaAuthoringIssue[] };

export function createAaaAuthoringTask(
  type: AaaAuthoringTask['type'],
): AaaAuthoringTask {
  const native = (): AaaAuthoringAsset => ({ type: 'Native' });
  const local = (): AaaAuthoringAsset => ({ type: 'Local', id: 0 });
  const fixed = (): AaaAuthoringAmount => ({ type: 'Fixed', value: '0' });
  switch (type) {
    case 'Transfer':
      return { type, to: '', asset: native(), amount: fixed() };
    case 'SplitTransfer':
      return {
        type,
        asset: native(),
        amount: fixed(),
        legs: [
          { to: '', shareParts: 500_000_000 },
          { to: '', shareParts: 500_000_000 },
        ],
      };
    case 'SwapExactIn':
      return {
        type,
        assetIn: native(),
        assetOut: local(),
        amountIn: fixed(),
        slippageParts: 10_000_000,
      };
    case 'SwapExactOut':
      return {
        type,
        assetIn: native(),
        assetOut: local(),
        amountOut: fixed(),
        slippageParts: 10_000_000,
      };
    case 'AddLiquidity':
      return {
        type,
        assetA: native(),
        assetB: local(),
        amountA: fixed(),
        amountB: fixed(),
      };
    case 'RemoveLiquidity':
      return { type, lpAsset: local(), amount: fixed() };
    case 'Burn':
    case 'Mint':
    case 'Stake':
      return { type, asset: native(), amount: fixed() };
    case 'DonateLiquidity':
      return {
        type,
        assetA: native(),
        assetB: local(),
        amount: fixed(),
        maxRatioErrorParts: 10_000_000,
      };
    case 'Unstake':
      return { type, asset: native(), shares: fixed() };
    case 'StopCycle':
      return { type };
  }
}

export function createAaaAuthoringCondition(
  type: AaaAuthoringCondition['type'],
): AaaAuthoringCondition {
  switch (type) {
    case 'BalanceAbove':
    case 'BalanceBelow':
    case 'BalanceEquals':
    case 'BalanceNotEquals':
      return { type, asset: { type: 'Native' }, threshold: '0' };
    case 'BlockNumberAbove':
    case 'BlockNumberBelow':
      return { type, threshold: 0 };
  }
}

export function createAaaAuthoringStep(
  key: string,
  task: AaaAuthoringTask = createAaaAuthoringTask('Transfer'),
): AaaAuthoringStep {
  return {
    key,
    conditionSet: { type: 'Always' },
    task,
    errorPolicy: 'AbortCycle',
  };
}

export function createAaaAuthoringProgram(): AaaAuthoringProgram {
  return {
    aaaType: 'User',
    mutability: 'Mutable',
    trigger: { type: 'Manual' },
    cooldownBlocks: 0,
    scheduleWindow: null,
    fundingPolicy: { type: 'OwnerOnly' },
    steps: [createAaaAuthoringStep('step-1')],
  };
}

const U32_MAX = 0xffff_ffff;
const U128_MAX = (1n << 128n) - 1n;
const PERBILL_MAX = 1_000_000_000;
const UNSIGNED_INTEGER = /^(?:0|[1-9][0-9]*)$/;

function isU32(value: number) {
  return Number.isSafeInteger(value) && value >= 0 && value <= U32_MAX;
}

function validatePerbill(
  value: number,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  if (!Number.isSafeInteger(value) || value < 0 || value > PERBILL_MAX) {
    issues.push({
      path,
      message: 'Perbill parts must be an integer from 0 to 1,000,000,000',
    });
  }
}

function validateAddress(
  value: string,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  if (value.trim().length === 0) {
    issues.push({ path, message: 'Account is required' });
    return;
  }
  try {
    decodeAddress(value.trim());
  } catch {
    issues.push({
      path,
      message: 'Account must have a valid address checksum',
    });
  }
}

function validateAsset(
  asset: AaaAuthoringAsset,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  switch (asset.type) {
    case 'Native':
      return;
    case 'Local':
    case 'Foreign':
      if (!isU32(asset.id)) {
        issues.push({ path: `${path}.id`, message: 'Asset id must be a u32' });
      }
  }
}

function validateAmount(
  amount: AaaAuthoringAmount,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  switch (amount.type) {
    case 'Fixed':
      if (!UNSIGNED_INTEGER.test(amount.value)) {
        issues.push({
          path: `${path}.value`,
          message:
            'Fixed amount must be a canonical unsigned base-unit integer',
        });
      } else if (BigInt(amount.value) > U128_MAX) {
        issues.push({
          path: `${path}.value`,
          message: 'Fixed amount must fit the runtime u128 balance type',
        });
      }
      return;
    case 'PercentageOfCurrent':
    case 'PercentageOfTrigger':
    case 'PercentageOfLastFunding':
      validatePerbill(amount.parts, `${path}.parts`, issues);
      return;
    case 'AllBalance':
      return;
  }
}

function validateCondition(
  condition: AaaAuthoringCondition,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  switch (condition.type) {
    case 'BalanceAbove':
    case 'BalanceBelow':
    case 'BalanceEquals':
    case 'BalanceNotEquals':
      validateAsset(condition.asset, `${path}.asset`, issues);
      if (!UNSIGNED_INTEGER.test(condition.threshold)) {
        issues.push({
          path: `${path}.threshold`,
          message: 'Balance threshold must be a canonical unsigned integer',
        });
      }
      return;
    case 'BlockNumberAbove':
    case 'BlockNumberBelow':
      if (!isU32(condition.threshold)) {
        issues.push({
          path: `${path}.threshold`,
          message: 'Block threshold must be a u32',
        });
      }
  }
}

function validateTask(
  task: AaaAuthoringTask,
  path: string,
  issues: AaaAuthoringIssue[],
  limits: AaaAuthoringLimits,
  aaaType: AaaPlanType,
) {
  switch (task.type) {
    case 'Transfer':
      validateAddress(task.to, `${path}.to`, issues);
      validateAsset(task.asset, `${path}.asset`, issues);
      validateAmount(task.amount, `${path}.amount`, issues);
      return;
    case 'SplitTransfer': {
      validateAsset(task.asset, `${path}.asset`, issues);
      validateAmount(task.amount, `${path}.amount`, issues);
      if (
        task.legs.length < 2 ||
        task.legs.length > limits.maxSplitTransferLegs
      ) {
        issues.push({
          path: `${path}.legs`,
          message: `SplitTransfer requires 2..${limits.maxSplitTransferLegs} legs`,
        });
      }
      let total = 0;
      const recipients = new Set<string>();
      task.legs.forEach((leg, index) => {
        validateAddress(leg.to, `${path}.legs[${index}].to`, issues);
        validatePerbill(
          leg.shareParts,
          `${path}.legs[${index}].shareParts`,
          issues,
        );
        if (leg.shareParts === 0) {
          issues.push({
            path: `${path}.legs[${index}].shareParts`,
            message: 'Split share must be nonzero',
          });
        }
        total += leg.shareParts;
        const recipient = leg.to.trim();
        if (recipients.has(recipient)) {
          issues.push({
            path: `${path}.legs[${index}].to`,
            message: 'Split recipients must be unique',
          });
        }
        recipients.add(recipient);
      });
      if (total > PERBILL_MAX) {
        issues.push({
          path: `${path}.legs`,
          message: 'Split shares cannot exceed 1,000,000,000 parts',
        });
      }
      return;
    }
    case 'SwapExactIn':
      validateAsset(task.assetIn, `${path}.assetIn`, issues);
      validateAsset(task.assetOut, `${path}.assetOut`, issues);
      validateAmount(task.amountIn, `${path}.amountIn`, issues);
      validatePerbill(task.slippageParts, `${path}.slippageParts`, issues);
      return;
    case 'SwapExactOut':
      validateAsset(task.assetIn, `${path}.assetIn`, issues);
      validateAsset(task.assetOut, `${path}.assetOut`, issues);
      validateAmount(task.amountOut, `${path}.amountOut`, issues);
      validatePerbill(task.slippageParts, `${path}.slippageParts`, issues);
      return;
    case 'AddLiquidity':
      validateAsset(task.assetA, `${path}.assetA`, issues);
      validateAsset(task.assetB, `${path}.assetB`, issues);
      validateAmount(task.amountA, `${path}.amountA`, issues);
      validateAmount(task.amountB, `${path}.amountB`, issues);
      return;
    case 'RemoveLiquidity':
      validateAsset(task.lpAsset, `${path}.lpAsset`, issues);
      validateAmount(task.amount, `${path}.amount`, issues);
      return;
    case 'Burn':
    case 'Stake':
      validateAsset(task.asset, `${path}.asset`, issues);
      validateAmount(task.amount, `${path}.amount`, issues);
      return;
    case 'Mint':
      if (aaaType !== 'System') {
        issues.push({ path, message: 'Mint is available only to System AAA' });
      }
      validateAsset(task.asset, `${path}.asset`, issues);
      validateAmount(task.amount, `${path}.amount`, issues);
      return;
    case 'DonateLiquidity':
      validateAsset(task.assetA, `${path}.assetA`, issues);
      validateAsset(task.assetB, `${path}.assetB`, issues);
      validateAmount(task.amount, `${path}.amount`, issues);
      validatePerbill(
        task.maxRatioErrorParts,
        `${path}.maxRatioErrorParts`,
        issues,
      );
      return;
    case 'Unstake':
      validateAsset(task.asset, `${path}.asset`, issues);
      validateAmount(task.shares, `${path}.shares`, issues);
      return;
    case 'StopCycle':
      return;
  }
}

function validateUniqueAddresses(
  accounts: string[],
  path: string,
  issues: AaaAuthoringIssue[],
  max: number,
) {
  if (accounts.length === 0 || accounts.length > max) {
    issues.push({ path, message: `Allowlist requires 1..${max} accounts` });
  }
  const seen = new Set<string>();
  accounts.forEach((account, index) => {
    validateAddress(account, `${path}[${index}]`, issues);
    const normalized = account.trim();
    if (seen.has(normalized)) {
      issues.push({
        path: `${path}[${index}]`,
        message: 'Allowlist accounts must be unique',
      });
    }
    seen.add(normalized);
  });
}

function validateTrigger(
  trigger: AaaAuthoringTrigger,
  issues: AaaAuthoringIssue[],
  limits: AaaAuthoringLimits,
) {
  switch (trigger.type) {
    case 'Manual':
      return;
    case 'Timer':
      if (!isU32(trigger.everyBlocks) || trigger.everyBlocks === 0) {
        issues.push({
          path: 'trigger.everyBlocks',
          message: 'Timer cadence must be a positive u32',
        });
      }
      return;
    case 'OnAddressEvent':
      if (trigger.sourceFilter.type === 'Whitelist') {
        validateUniqueAddresses(
          trigger.sourceFilter.accounts,
          'trigger.sourceFilter.accounts',
          issues,
          limits.maxWhitelistSize,
        );
      }
      if (trigger.assetFilter.type === 'Whitelist') {
        if (
          trigger.assetFilter.assets.length === 0 ||
          trigger.assetFilter.assets.length > limits.maxWhitelistSize
        ) {
          issues.push({
            path: 'trigger.assetFilter.assets',
            message: `Asset whitelist requires 1..${limits.maxWhitelistSize} entries`,
          });
        }
        trigger.assetFilter.assets.forEach((asset, index) =>
          validateAsset(asset, `trigger.assetFilter.assets[${index}]`, issues),
        );
      }
  }
}

export function validateAaaAuthoringProgram(
  program: AaaAuthoringProgram,
  limits = DEOS_AAA_AUTHORING_LIMITS,
): AaaAuthoringValidation {
  const issues: AaaAuthoringIssue[] = [];
  const maxSteps =
    program.aaaType === 'User' ? limits.maxUserSteps : limits.maxSystemSteps;
  if (program.steps.length === 0 || program.steps.length > maxSteps) {
    issues.push({
      path: 'steps',
      message: `Active ${program.aaaType} program requires 1..${maxSteps} steps`,
    });
  }
  if (!isU32(program.cooldownBlocks)) {
    issues.push({
      path: 'cooldownBlocks',
      message: 'Cooldown must be a u32',
    });
  }
  if (
    program.scheduleWindow != null &&
    (!isU32(program.scheduleWindow.start) ||
      !isU32(program.scheduleWindow.end) ||
      program.scheduleWindow.end <= program.scheduleWindow.start)
  ) {
    issues.push({
      path: 'scheduleWindow',
      message:
        'Schedule window requires u32 bounds with end greater than start',
    });
  }
  validateTrigger(program.trigger, issues, limits);
  if (program.fundingPolicy.type === 'SignedAllowlist') {
    validateUniqueAddresses(
      program.fundingPolicy.accounts,
      'fundingPolicy.accounts',
      issues,
      limits.maxWhitelistSize,
    );
  }
  const keys = new Set<string>();
  program.steps.forEach((step, index) => {
    const path = `steps[${index}]`;
    if (step.key.length === 0 || keys.has(step.key)) {
      issues.push({
        path: `${path}.key`,
        message: 'Authoring step keys must be non-empty and unique',
      });
    }
    keys.add(step.key);
    const conditions =
      step.conditionSet.type === 'Always' ? [] : step.conditionSet.conditions;
    if (step.conditionSet.type !== 'Always' && conditions.length === 0) {
      issues.push({
        path: `${path}.conditionSet.conditions`,
        message: `${step.conditionSet.type} requires at least one condition`,
      });
    }
    if (conditions.length > limits.maxConditionsPerStep) {
      issues.push({
        path: `${path}.conditionSet.conditions`,
        message: `A step supports at most ${limits.maxConditionsPerStep} conditions`,
      });
    }
    conditions.forEach((condition, conditionIndex) =>
      validateCondition(
        condition,
        `${path}.conditionSet.conditions[${conditionIndex}]`,
        issues,
      ),
    );
    if (
      program.mutability === 'Immutable' &&
      step.errorPolicy === 'RetryLater'
    ) {
      issues.push({
        path: `${path}.errorPolicy`,
        message: 'RetryLater requires a Mutable actor',
      });
    }
    validateTask(step.task, `${path}.task`, issues, limits, program.aaaType);
  });
  return issues.length === 0
    ? { valid: true, issues: [] }
    : { valid: false, issues };
}

export function appendAaaStep(
  program: AaaAuthoringProgram,
  step: AaaAuthoringStep,
): AaaAuthoringProgram {
  return { ...program, steps: [...program.steps, structuredClone(step)] };
}

export function replaceAaaStep(
  program: AaaAuthoringProgram,
  key: string,
  step: AaaAuthoringStep,
): AaaAuthoringProgram {
  const index = program.steps.findIndex((candidate) => candidate.key === key);
  if (index < 0) throw new Error(`Unknown authoring step key: ${key}`);
  const steps = program.steps.map((candidate, candidateIndex) =>
    candidateIndex === index ? structuredClone(step) : candidate,
  );
  return { ...program, steps };
}

export function removeAaaStep(
  program: AaaAuthoringProgram,
  key: string,
): AaaAuthoringProgram {
  const index = program.steps.findIndex((candidate) => candidate.key === key);
  if (index < 0) throw new Error(`Unknown authoring step key: ${key}`);
  return {
    ...program,
    steps: program.steps.filter(
      (_, candidateIndex) => candidateIndex !== index,
    ),
  };
}

export function moveAaaStep(
  program: AaaAuthoringProgram,
  fromIndex: number,
  toIndex: number,
): AaaAuthoringProgram {
  if (
    !Number.isSafeInteger(fromIndex) ||
    !Number.isSafeInteger(toIndex) ||
    fromIndex < 0 ||
    toIndex < 0 ||
    fromIndex >= program.steps.length ||
    toIndex >= program.steps.length
  ) {
    throw new Error(
      'Step move indexes must address the current ordered program',
    );
  }
  if (fromIndex === toIndex) return program;
  const steps = [...program.steps];
  const [moved] = steps.splice(fromIndex, 1);
  steps.splice(toIndex, 0, moved);
  return { ...program, steps };
}

function runtimeVariant(type: string, value: unknown = undefined) {
  return { type, value };
}

function lowerAsset(asset: AaaAuthoringAsset) {
  switch (asset.type) {
    case 'Native':
      return runtimeVariant('Native');
    case 'Local':
    case 'Foreign':
      return runtimeVariant(asset.type, asset.id);
  }
}

function lowerAmount(amount: AaaAuthoringAmount) {
  switch (amount.type) {
    case 'Fixed':
      return runtimeVariant('Fixed', BigInt(amount.value));
    case 'PercentageOfCurrent':
    case 'PercentageOfTrigger':
    case 'PercentageOfLastFunding':
      return runtimeVariant(amount.type, amount.parts);
    case 'AllBalance':
      return runtimeVariant('AllBalance');
  }
}

function lowerCondition(condition: AaaAuthoringCondition) {
  switch (condition.type) {
    case 'BalanceAbove':
    case 'BalanceBelow':
    case 'BalanceEquals':
    case 'BalanceNotEquals':
      return runtimeVariant(condition.type, {
        asset: lowerAsset(condition.asset),
        threshold: BigInt(condition.threshold),
      });
    case 'BlockNumberAbove':
    case 'BlockNumberBelow':
      return runtimeVariant(condition.type, { threshold: condition.threshold });
  }
}

function lowerTask(task: AaaAuthoringTask) {
  switch (task.type) {
    case 'Transfer':
      return runtimeVariant('Transfer', {
        to: task.to,
        asset: lowerAsset(task.asset),
        amount: lowerAmount(task.amount),
      });
    case 'SplitTransfer':
      return runtimeVariant('SplitTransfer', {
        asset: lowerAsset(task.asset),
        amount: lowerAmount(task.amount),
        legs: task.legs.map((leg) => ({
          to: leg.to,
          share: leg.shareParts,
        })),
      });
    case 'SwapExactIn':
      return runtimeVariant('SwapExactIn', {
        asset_in: lowerAsset(task.assetIn),
        asset_out: lowerAsset(task.assetOut),
        amount_in: lowerAmount(task.amountIn),
        slippage_tolerance: task.slippageParts,
      });
    case 'SwapExactOut':
      return runtimeVariant('SwapExactOut', {
        asset_in: lowerAsset(task.assetIn),
        asset_out: lowerAsset(task.assetOut),
        amount_out: lowerAmount(task.amountOut),
        slippage_tolerance: task.slippageParts,
      });
    case 'AddLiquidity':
      return runtimeVariant('AddLiquidity', {
        asset_a: lowerAsset(task.assetA),
        asset_b: lowerAsset(task.assetB),
        amount_a: lowerAmount(task.amountA),
        amount_b: lowerAmount(task.amountB),
      });
    case 'RemoveLiquidity':
      return runtimeVariant('RemoveLiquidity', {
        lp_asset: lowerAsset(task.lpAsset),
        amount: lowerAmount(task.amount),
      });
    case 'Burn':
    case 'Mint':
    case 'Stake':
      return runtimeVariant(task.type, {
        asset: lowerAsset(task.asset),
        amount: lowerAmount(task.amount),
      });
    case 'DonateLiquidity':
      return runtimeVariant('DonateLiquidity', {
        asset_a: lowerAsset(task.assetA),
        asset_b: lowerAsset(task.assetB),
        amount: lowerAmount(task.amount),
        max_ratio_error: task.maxRatioErrorParts,
      });
    case 'Unstake':
      return runtimeVariant('Unstake', {
        asset: lowerAsset(task.asset),
        shares: lowerAmount(task.shares),
      });
    case 'StopCycle':
      return runtimeVariant('StopCycle');
  }
}

function lowerTrigger(trigger: AaaAuthoringTrigger) {
  switch (trigger.type) {
    case 'Manual':
      return runtimeVariant('Manual');
    case 'Timer':
      return runtimeVariant('Timer', { every_blocks: trigger.everyBlocks });
    case 'OnAddressEvent': {
      const sourceFilter = (() => {
        switch (trigger.sourceFilter.type) {
          case 'Any':
          case 'OwnerOnly':
            return runtimeVariant(trigger.sourceFilter.type);
          case 'Whitelist':
            return runtimeVariant('Whitelist', trigger.sourceFilter.accounts);
        }
      })();
      const assetFilter =
        trigger.assetFilter.type === 'Any'
          ? runtimeVariant('Any')
          : runtimeVariant(
              'Whitelist',
              trigger.assetFilter.assets.map(lowerAsset),
            );
      return runtimeVariant('OnAddressEvent', {
        source_filter: sourceFilter,
        asset_filter: assetFilter,
      });
    }
  }
}

function compareAccountBytes(left: string, right: string) {
  const leftBytes = decodeAddress(left.trim());
  const rightBytes = decodeAddress(right.trim());
  for (let index = 0; index < leftBytes.length; index += 1) {
    const difference = leftBytes[index] - rightBytes[index];
    if (difference !== 0) return difference;
  }
  return 0;
}

function lowerFundingPolicy(policy: AaaAuthoringFundingPolicy) {
  switch (policy.type) {
    case 'OwnerOnly':
    case 'RuntimePolicy':
    case 'AnySource':
      return runtimeVariant(policy.type);
    case 'SignedAllowlist':
      return runtimeVariant(
        'SignedAllowlist',
        [...policy.accounts].sort(compareAccountBytes),
      );
  }
}

export function lowerAaaAuthoringProgram(
  program: AaaAuthoringProgram,
  limits = DEOS_AAA_AUTHORING_LIMITS,
) {
  const validation = validateAaaAuthoringProgram(program, limits);
  if (!validation.valid) {
    throw new Error(
      `Invalid AAA authoring program: ${validation.issues
        .map((issue) => `${issue.path}: ${issue.message}`)
        .join('; ')}`,
    );
  }
  return runtimeVariant('Active', {
    schedule: {
      trigger: lowerTrigger(program.trigger),
      cooldown_blocks: program.cooldownBlocks,
    },
    schedule_window:
      program.scheduleWindow == null
        ? undefined
        : {
            start: program.scheduleWindow.start,
            end: program.scheduleWindow.end,
          },
    execution_plan: program.steps.map((step) => ({
      conditions:
        step.conditionSet.type === 'Always'
          ? runtimeVariant('Always')
          : runtimeVariant(
              step.conditionSet.type,
              step.conditionSet.conditions.map(lowerCondition),
            ),
      task: lowerTask(step.task),
      on_error: runtimeVariant(step.errorPolicy),
    })),
    funding_source_policy: lowerFundingPolicy(program.fundingPolicy),
  });
}

export function createAaaArtifactFromAuthoring(input: {
  program: AaaAuthoringProgram;
  metadataBytes: Uint8Array;
  runtime: AaaPlanRuntimeIdentity;
  limits?: AaaAuthoringLimits;
}): AaaPlanArtifact {
  const runtimeValue = lowerAaaAuthoringProgram(
    input.program,
    input.limits ?? DEOS_AAA_AUTHORING_LIMITS,
  );
  const programScale = encodeAaaProgramValue(input.metadataBytes, runtimeValue);
  return createAaaPlanArtifact({
    metadataBytes: input.metadataBytes,
    runtime: input.runtime,
    aaaType: input.program.aaaType,
    mutability: input.program.mutability,
    programScale,
  });
}
