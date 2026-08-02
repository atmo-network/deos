/*
Domain: AAA linear plan authoring
Owns: Typed step drafts, immutable ordered-step operations, structural validation, exact ProgramInput lowering, and canonical artifact production.
Excludes: Runtime submission, governance authority, adapter execution, simulation, weight modeling, recipes, and widget state.
Zone: Automation domain capability; composes the canonical plan-artifact codec without defining another runtime language.
*/
import { decodeAddress } from '@polkadot/util-crypto';

import {
  AAA_MAX_EXECUTION_PLAN_STEPS,
  AAA_MAX_RETRY_ATTEMPTS,
} from './aaa-protocol-bounds.ts';
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
        | 'PercentageAtOpening'
        | 'PercentageOfLastFunding';
      parts: number;
    }
  | { type: 'AllAvailable' };

export type AaaAuthoringObservationFeed = {
  assetIn: AaaAuthoringAsset;
  assetOut: AaaAuthoringAsset;
  method: 'PreExecutionSpot';
  aggregation: { type: 'LastValue' } | { type: 'Ema'; halfLifeBlocks: number };
  scale: number;
};

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
    }
  | {
      type:
        | 'ObservationAbove'
        | 'ObservationBelow'
        | 'ObservationEquals'
        | 'ObservationNotEquals';
      feed: AaaAuthoringObservationFeed;
      threshold: string;
      maxAgeBlocks: number;
    };

export type AaaAuthoringInputLimit =
  | { type: 'LiveQuote' }
  | { type: 'Absolute'; amount: string };

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
      type: 'SwapIn';
      assetIn: AaaAuthoringAsset;
      amountIn: AaaAuthoringAmount;
      assetOut: AaaAuthoringAsset;
      slippageParts: number;
    }
  | {
      type: 'SwapOut';
      assetOut: AaaAuthoringAsset;
      amountOut: AaaAuthoringAmount;
      assetIn: AaaAuthoringAsset;
      inputLimit: AaaAuthoringInputLimit;
      slippageParts: number;
    }
  | {
      type: 'AddLiquidity';
      assetA: AaaAuthoringAsset;
      assetB: AaaAuthoringAsset;
      amountA: AaaAuthoringAmount;
      amountB: AaaAuthoringAmount;
      minLpOut: string;
    }
  | {
      type: 'RemoveLiquidity';
      lpAsset: AaaAuthoringAsset;
      assetA: AaaAuthoringAsset;
      assetB: AaaAuthoringAsset;
      lpAmount: AaaAuthoringAmount;
      minAmountA: string;
      minAmountB: string;
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
      maxAmountA: AaaAuthoringAmount;
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
  'SwapIn',
  'SwapOut',
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
  'ObservationAbove',
  'ObservationBelow',
  'ObservationEquals',
  'ObservationNotEquals',
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
  | { type: 'AnyVerifiedIngress' }
  | { type: 'SignedAllowlist'; accounts: string[] };

export type AaaAuthoringTriggerSource =
  | { type: 'Manual' }
  | {
      type: 'OnAddressEvent';
      sourceFilter:
        | { type: 'Any' }
        | { type: 'OwnerOnly' }
        | { type: 'Whitelist'; accounts: string[] };
      assetFilter:
        | { type: 'Any' }
        | { type: 'Whitelist'; assets: AaaAuthoringAsset[] };
    }
  | { type: 'OnObservationChange'; feed: AaaAuthoringObservationFeed };

export type AaaAuthoringTrigger =
  | { type: 'Immediate'; sources: AaaAuthoringTriggerSource[] }
  | {
      type: 'Cadenced';
      everyBlocks: number;
      mode:
        | { type: 'Always' }
        | { type: 'WhenSignalled'; sources: AaaAuthoringTriggerSource[] };
    };

export type AaaAuthoringCompletionPolicy =
  | 'Persistent'
  | 'CloseAfterProductiveRun';

export type AaaAuthoringProgram = {
  aaaType: AaaPlanType;
  mutability: AutomationMutability;
  completionPolicy: AaaAuthoringCompletionPolicy;
  autoCloseAtCycleNonce: bigint | null;
  trigger: AaaAuthoringTrigger;
  cooldownBlocks: number;
  scheduleWindow: { start: number; end: number } | null;
  fundingPolicy: AaaAuthoringFundingPolicy;
  steps: AaaAuthoringStep[];
};

export type AaaAuthoringLimits = {
  maxExecutionPlanSteps: number;
  maxRetryAttempts: number;
  maxConditionsPerStep: number;
  maxSplitTransferLegs: number;
  maxWhitelistSize: number;
  maxTriggerSources: number;
};

export const DEOS_AAA_AUTHORING_LIMITS: AaaAuthoringLimits = {
  maxExecutionPlanSteps: AAA_MAX_EXECUTION_PLAN_STEPS,
  maxRetryAttempts: AAA_MAX_RETRY_ATTEMPTS,
  maxConditionsPerStep: 4,
  maxSplitTransferLegs: 8,
  maxWhitelistSize: 16,
  maxTriggerSources: 4,
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
    case 'SwapIn':
      return {
        type,
        assetIn: native(),
        amountIn: fixed(),
        assetOut: local(),
        slippageParts: 10_000_000,
      };
    case 'SwapOut':
      return {
        type,
        assetOut: local(),
        amountOut: fixed(),
        assetIn: native(),
        inputLimit: { type: 'LiveQuote' },
        slippageParts: 10_000_000,
      };
    case 'AddLiquidity':
      return {
        type,
        assetA: native(),
        assetB: local(),
        amountA: fixed(),
        amountB: fixed(),
        minLpOut: '1',
      };
    case 'RemoveLiquidity':
      return {
        type,
        lpAsset: local(),
        assetA: native(),
        assetB: local(),
        lpAmount: fixed(),
        minAmountA: '1',
        minAmountB: '1',
      };
    case 'Burn':
    case 'Mint':
    case 'Stake':
      return { type, asset: native(), amount: fixed() };
    case 'DonateLiquidity':
      return {
        type,
        assetA: native(),
        assetB: local(),
        maxAmountA: fixed(),
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
    case 'ObservationAbove':
    case 'ObservationBelow':
    case 'ObservationEquals':
    case 'ObservationNotEquals':
      return {
        type,
        feed: {
          assetIn: { type: 'Native' },
          assetOut: { type: 'Local', id: 0 },
          method: 'PreExecutionSpot',
          aggregation: { type: 'LastValue' },
          scale: 12,
        },
        threshold: '0',
        maxAgeBlocks: 1,
      };
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
    errorPolicy: { type: 'AbortCycle' },
  };
}

export function createAaaAuthoringProgram(): AaaAuthoringProgram {
  return {
    aaaType: 'User',
    mutability: 'Mutable',
    completionPolicy: 'Persistent',
    autoCloseAtCycleNonce: null,
    trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
    cooldownBlocks: 0,
    scheduleWindow: null,
    fundingPolicy: { type: 'OwnerOnly' },
    steps: [createAaaAuthoringStep('step-1')],
  };
}

const U32_MAX = 0xffff_ffff;
const U64_MAX = (1n << 64n) - 1n;
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
      } else if (
        BigInt(amount.value) === 0n ||
        BigInt(amount.value) > U128_MAX
      ) {
        issues.push({
          path: `${path}.value`,
          message:
            'Fixed amount must be nonzero and fit the runtime u128 balance type',
        });
      }
      return;
    case 'PercentageOfCurrent':
    case 'PercentageAtOpening':
    case 'PercentageOfLastFunding':
      validatePerbill(amount.parts, `${path}.parts`, issues);
      if (amount.parts === 0) {
        issues.push({
          path: `${path}.parts`,
          message: 'Percentage amount must be nonzero',
        });
      }
      return;
    case 'AllAvailable':
      return;
  }
}

function validatePositiveBalanceBound(
  value: string,
  path: string,
  label: string,
  issues: AaaAuthoringIssue[],
) {
  if (!UNSIGNED_INTEGER.test(value)) {
    issues.push({
      path,
      message: `${label} must be a canonical unsigned base-unit integer`,
    });
    return;
  }
  const bound = BigInt(value);
  if (bound === 0n || bound > U128_MAX) {
    issues.push({
      path,
      message: `${label} must be greater than zero and fit u128`,
    });
  }
}

function validateObservationFeed(
  feed: AaaAuthoringObservationFeed,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  validateAsset(feed.assetIn, `${path}.assetIn`, issues);
  validateAsset(feed.assetOut, `${path}.assetOut`, issues);
  if (
    feed.aggregation.type === 'Ema' &&
    (!isU32(feed.aggregation.halfLifeBlocks) ||
      feed.aggregation.halfLifeBlocks === 0)
  ) {
    issues.push({
      path: `${path}.aggregation.halfLifeBlocks`,
      message: 'EMA half-life must be a nonzero u32',
    });
  }
  if (!Number.isInteger(feed.scale) || feed.scale < 0 || feed.scale > 255) {
    issues.push({
      path: `${path}.scale`,
      message: 'Observation scale must be a u8',
    });
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
      return;
    case 'ObservationAbove':
    case 'ObservationBelow':
    case 'ObservationEquals':
    case 'ObservationNotEquals':
      validateObservationFeed(condition.feed, `${path}.feed`, issues);
      if (!UNSIGNED_INTEGER.test(condition.threshold)) {
        issues.push({
          path: `${path}.threshold`,
          message: 'Observation threshold must be a canonical unsigned integer',
        });
      }
      if (!isU32(condition.maxAgeBlocks) || condition.maxAgeBlocks === 0) {
        issues.push({
          path: `${path}.maxAgeBlocks`,
          message: 'Observation maximum age must be a nonzero u32',
        });
      }
  }
}

function validateDistinctAssets(
  assetA: AaaAuthoringAsset,
  assetB: AaaAuthoringAsset,
  path: string,
  issues: AaaAuthoringIssue[],
) {
  if (
    bytesKey(assetCanonicalBytes(assetA)) ===
    bytesKey(assetCanonicalBytes(assetB))
  ) {
    issues.push({ path, message: 'Market task assets must be distinct' });
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
    case 'SwapIn':
      validateAsset(task.assetIn, `${path}.assetIn`, issues);
      validateAsset(task.assetOut, `${path}.assetOut`, issues);
      validateDistinctAssets(task.assetIn, task.assetOut, path, issues);
      validateAmount(task.amountIn, `${path}.amountIn`, issues);
      validatePerbill(task.slippageParts, `${path}.slippageParts`, issues);
      return;
    case 'SwapOut':
      validateAsset(task.assetOut, `${path}.assetOut`, issues);
      validateAmount(task.amountOut, `${path}.amountOut`, issues);
      validateAsset(task.assetIn, `${path}.assetIn`, issues);
      validateDistinctAssets(task.assetIn, task.assetOut, path, issues);
      if (task.inputLimit.type === 'Absolute') {
        validatePositiveBalanceBound(
          task.inputLimit.amount,
          `${path}.inputLimit.amount`,
          'Absolute input ceiling',
          issues,
        );
      }
      validatePerbill(task.slippageParts, `${path}.slippageParts`, issues);
      return;
    case 'AddLiquidity':
      validateAsset(task.assetA, `${path}.assetA`, issues);
      validateAsset(task.assetB, `${path}.assetB`, issues);
      validateDistinctAssets(task.assetA, task.assetB, path, issues);
      validateAmount(task.amountA, `${path}.amountA`, issues);
      validateAmount(task.amountB, `${path}.amountB`, issues);
      validatePositiveBalanceBound(
        task.minLpOut,
        `${path}.minLpOut`,
        'Minimum LP output',
        issues,
      );
      return;
    case 'RemoveLiquidity':
      validateAsset(task.lpAsset, `${path}.lpAsset`, issues);
      validateAsset(task.assetA, `${path}.assetA`, issues);
      validateAsset(task.assetB, `${path}.assetB`, issues);
      validateAmount(task.lpAmount, `${path}.lpAmount`, issues);
      validatePositiveBalanceBound(
        task.minAmountA,
        `${path}.minAmountA`,
        'Minimum asset A output',
        issues,
      );
      validatePositiveBalanceBound(
        task.minAmountB,
        `${path}.minAmountB`,
        'Minimum asset B output',
        issues,
      );
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
      validateDistinctAssets(task.assetA, task.assetB, path, issues);
      validateAmount(task.maxAmountA, `${path}.maxAmountA`, issues);
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
    let normalized = account.trim();
    try {
      normalized = bytesKey(decodeAddress(normalized));
    } catch {
      // validateAddress owns the malformed-address diagnostic.
    }
    if (seen.has(normalized)) {
      issues.push({
        path: `${path}[${index}]`,
        message: 'Allowlist accounts must be unique',
      });
    }
    seen.add(normalized);
  });
}

function validateTriggerSources(
  sources: AaaAuthoringTriggerSource[],
  path: string,
  issues: AaaAuthoringIssue[],
  limits: AaaAuthoringLimits,
) {
  if (sources.length === 0 || sources.length > limits.maxTriggerSources) {
    issues.push({
      path,
      message: `Trigger policy requires 1..${limits.maxTriggerSources} sources`,
    });
  }
  const seen = new Set<string>();
  sources.forEach((source, sourceIndex) => {
    const sourcePath = `${path}[${sourceIndex}]`;
    if (source.type === 'OnAddressEvent') {
      if (source.sourceFilter.type === 'Whitelist') {
        validateUniqueAddresses(
          source.sourceFilter.accounts,
          `${sourcePath}.sourceFilter.accounts`,
          issues,
          limits.maxWhitelistSize,
        );
      }
      if (source.assetFilter.type === 'Whitelist') {
        if (
          source.assetFilter.assets.length === 0 ||
          source.assetFilter.assets.length > limits.maxWhitelistSize
        ) {
          issues.push({
            path: `${sourcePath}.assetFilter.assets`,
            message: `Asset whitelist requires 1..${limits.maxWhitelistSize} entries`,
          });
        }
        const assets = new Set<string>();
        source.assetFilter.assets.forEach((asset, assetIndex) => {
          validateAsset(
            asset,
            `${sourcePath}.assetFilter.assets[${assetIndex}]`,
            issues,
          );
          const key = bytesKey(assetCanonicalBytes(asset));
          if (assets.has(key)) {
            issues.push({
              path: `${sourcePath}.assetFilter.assets[${assetIndex}]`,
              message: 'Asset whitelist entries must be unique',
            });
          }
          assets.add(key);
        });
      }
    } else if (source.type === 'OnObservationChange') {
      validateObservationFeed(source.feed, `${sourcePath}.feed`, issues);
    }
    try {
      const key = bytesKey(triggerSourceCanonicalBytes(source));
      if (seen.has(key)) {
        issues.push({
          path: sourcePath,
          message: 'Trigger sources must be semantically unique',
        });
      }
      seen.add(key);
    } catch {
      // Field-level validation owns malformed account diagnostics.
    }
  });
}

function validateTrigger(
  trigger: AaaAuthoringTrigger,
  issues: AaaAuthoringIssue[],
  limits: AaaAuthoringLimits,
) {
  switch (trigger.type) {
    case 'Immediate':
      validateTriggerSources(
        trigger.sources,
        'trigger.sources',
        issues,
        limits,
      );
      return;
    case 'Cadenced':
      if (!isU32(trigger.everyBlocks) || trigger.everyBlocks === 0) {
        issues.push({
          path: 'trigger.everyBlocks',
          message: 'Cadence must be a positive u32',
        });
      }
      if (trigger.mode.type === 'WhenSignalled') {
        validateTriggerSources(
          trigger.mode.sources,
          'trigger.mode.sources',
          issues,
          limits,
        );
      }
  }
}

export function validateAaaAuthoringProgram(
  program: AaaAuthoringProgram,
  limits = DEOS_AAA_AUTHORING_LIMITS,
): AaaAuthoringValidation {
  const issues: AaaAuthoringIssue[] = [];
  const maxSteps = limits.maxExecutionPlanSteps;
  if (
    program.completionPolicy !== 'Persistent' &&
    program.completionPolicy !== 'CloseAfterProductiveRun'
  ) {
    issues.push({
      path: 'completionPolicy',
      message:
        'Completion policy must be Persistent or CloseAfterProductiveRun',
    });
  }
  if (
    program.autoCloseAtCycleNonce != null &&
    (program.autoCloseAtCycleNonce <= 0n ||
      program.autoCloseAtCycleNonce > U64_MAX)
  ) {
    issues.push({
      path: 'autoCloseAtCycleNonce',
      message: 'Auto-close target must be a nonzero u64 logical-run nonce',
    });
  }
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
      step.errorPolicy.type === 'RetryLater'
    ) {
      issues.push({
        path: `${path}.errorPolicy`,
        message: 'RetryLater requires a Mutable actor',
      });
    }
    if (
      step.errorPolicy.type === 'RetryLater' &&
      (!Number.isSafeInteger(step.errorPolicy.maxAttempts) ||
        step.errorPolicy.maxAttempts < 2 ||
        step.errorPolicy.maxAttempts > limits.maxRetryAttempts)
    ) {
      issues.push({
        path: `${path}.errorPolicy.maxAttempts`,
        message: `RetryLater max attempts must be within 2..${limits.maxRetryAttempts}`,
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
    case 'PercentageAtOpening':
    case 'PercentageOfLastFunding':
      return runtimeVariant(amount.type, amount.parts);
    case 'AllAvailable':
      return runtimeVariant('AllAvailable');
  }
}

function lowerObservationFeed(feed: AaaAuthoringObservationFeed) {
  return {
    asset_in: lowerAsset(feed.assetIn),
    asset_out: lowerAsset(feed.assetOut),
    method: runtimeVariant(feed.method),
    aggregation:
      feed.aggregation.type === 'LastValue'
        ? runtimeVariant('LastValue')
        : runtimeVariant('Ema', {
            half_life_blocks: feed.aggregation.halfLifeBlocks,
          }),
    scale: feed.scale,
  };
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
    case 'ObservationAbove':
    case 'ObservationBelow':
    case 'ObservationEquals':
    case 'ObservationNotEquals':
      return runtimeVariant(condition.type, {
        feed: lowerObservationFeed(condition.feed),
        threshold: BigInt(condition.threshold),
        max_age_blocks: condition.maxAgeBlocks,
      });
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
    case 'SwapIn':
      return runtimeVariant('SwapIn', {
        asset_in: lowerAsset(task.assetIn),
        amount_in: lowerAmount(task.amountIn),
        asset_out: lowerAsset(task.assetOut),
        slippage_tolerance: task.slippageParts,
      });
    case 'SwapOut':
      return runtimeVariant('SwapOut', {
        asset_out: lowerAsset(task.assetOut),
        amount_out: lowerAmount(task.amountOut),
        asset_in: lowerAsset(task.assetIn),
        input_limit:
          task.inputLimit.type === 'LiveQuote'
            ? runtimeVariant('LiveQuote')
            : runtimeVariant('Absolute', BigInt(task.inputLimit.amount)),
        slippage_tolerance: task.slippageParts,
      });
    case 'AddLiquidity':
      return runtimeVariant('AddLiquidity', {
        asset_a: lowerAsset(task.assetA),
        asset_b: lowerAsset(task.assetB),
        amount_a: lowerAmount(task.amountA),
        amount_b: lowerAmount(task.amountB),
        min_lp_out: BigInt(task.minLpOut),
      });
    case 'RemoveLiquidity':
      return runtimeVariant('RemoveLiquidity', {
        lp_asset: lowerAsset(task.lpAsset),
        asset_a: lowerAsset(task.assetA),
        asset_b: lowerAsset(task.assetB),
        lp_amount: lowerAmount(task.lpAmount),
        min_amount_a: BigInt(task.minAmountA),
        min_amount_b: BigInt(task.minAmountB),
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
        max_amount_a: lowerAmount(task.maxAmountA),
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

function compareBytes(
  left: Uint8Array | number[],
  right: Uint8Array | number[],
) {
  const length = Math.min(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const difference = left[index] - right[index];
    if (difference !== 0) return difference;
  }
  return left.length - right.length;
}

function bytesKey(bytes: Uint8Array | number[]) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join(
    '',
  );
}

function u32Le(value: number) {
  return [
    value & 0xff,
    (value >>> 8) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 24) & 0xff,
  ];
}

function boundedVecBytes(values: (Uint8Array | number[])[]) {
  return [values.length << 2, ...values.flatMap((value) => Array.from(value))];
}

function assetCanonicalBytes(asset: AaaAuthoringAsset) {
  switch (asset.type) {
    case 'Native':
      return [0];
    case 'Local':
      return [1, ...u32Le(asset.id)];
    case 'Foreign':
      return [2, ...u32Le(asset.id)];
  }
}

function observationFeedCanonicalBytes(feed: AaaAuthoringObservationFeed) {
  return [
    ...assetCanonicalBytes(feed.assetIn),
    ...assetCanonicalBytes(feed.assetOut),
    0,
    ...(feed.aggregation.type === 'LastValue'
      ? [0]
      : [1, ...u32Le(feed.aggregation.halfLifeBlocks)]),
    feed.scale,
  ];
}

function sourceFilterCanonicalBytes(
  filter: Extract<
    AaaAuthoringTriggerSource,
    { type: 'OnAddressEvent' }
  >['sourceFilter'],
) {
  switch (filter.type) {
    case 'Any':
      return [0];
    case 'OwnerOnly':
      return [1];
    case 'Whitelist':
      return [
        2,
        ...boundedVecBytes(
          [...filter.accounts]
            .sort(compareAccountBytes)
            .map((account) => decodeAddress(account.trim())),
        ),
      ];
  }
}

function assetFilterCanonicalBytes(
  filter: Extract<
    AaaAuthoringTriggerSource,
    { type: 'OnAddressEvent' }
  >['assetFilter'],
) {
  switch (filter.type) {
    case 'Any':
      return [0];
    case 'Whitelist':
      return [
        1,
        ...boundedVecBytes(
          [...filter.assets]
            .sort((left, right) =>
              compareBytes(
                assetCanonicalBytes(left),
                assetCanonicalBytes(right),
              ),
            )
            .map(assetCanonicalBytes),
        ),
      ];
  }
}

function triggerSourceCanonicalBytes(source: AaaAuthoringTriggerSource) {
  switch (source.type) {
    case 'Manual':
      return [0];
    case 'OnAddressEvent':
      return [
        1,
        ...sourceFilterCanonicalBytes(source.sourceFilter),
        ...assetFilterCanonicalBytes(source.assetFilter),
      ];
    case 'OnObservationChange':
      return [2, ...observationFeedCanonicalBytes(source.feed)];
  }
}

function lowerTriggerSource(source: AaaAuthoringTriggerSource) {
  switch (source.type) {
    case 'Manual':
      return runtimeVariant('Manual');
    case 'OnAddressEvent': {
      const sourceFilter = (() => {
        switch (source.sourceFilter.type) {
          case 'Any':
          case 'OwnerOnly':
            return runtimeVariant(source.sourceFilter.type);
          case 'Whitelist':
            return runtimeVariant(
              'Whitelist',
              [...source.sourceFilter.accounts].sort(compareAccountBytes),
            );
        }
      })();
      const assetFilter =
        source.assetFilter.type === 'Any'
          ? runtimeVariant('Any')
          : runtimeVariant(
              'Whitelist',
              [...source.assetFilter.assets]
                .sort((left, right) =>
                  compareBytes(
                    assetCanonicalBytes(left),
                    assetCanonicalBytes(right),
                  ),
                )
                .map(lowerAsset),
            );
      return runtimeVariant('OnAddressEvent', {
        source_filter: sourceFilter,
        asset_filter: assetFilter,
      });
    }
    case 'OnObservationChange':
      return runtimeVariant('OnObservationChange', {
        feed: lowerObservationFeed(source.feed),
      });
  }
}

function lowerTriggerSources(sources: AaaAuthoringTriggerSource[]) {
  return [...sources]
    .sort((left, right) =>
      compareBytes(
        triggerSourceCanonicalBytes(left),
        triggerSourceCanonicalBytes(right),
      ),
    )
    .map(lowerTriggerSource);
}

function lowerTrigger(trigger: AaaAuthoringTrigger) {
  switch (trigger.type) {
    case 'Immediate':
      return runtimeVariant('Immediate', {
        sources: lowerTriggerSources(trigger.sources),
      });
    case 'Cadenced':
      return runtimeVariant('Cadenced', {
        every_blocks: trigger.everyBlocks,
        mode:
          trigger.mode.type === 'Always'
            ? runtimeVariant('Always')
            : runtimeVariant(
                'WhenSignalled',
                lowerTriggerSources(trigger.mode.sources),
              ),
      });
  }
}

function compareAccountBytes(left: string, right: string) {
  return compareBytes(decodeAddress(left.trim()), decodeAddress(right.trim()));
}

function lowerFundingPolicy(policy: AaaAuthoringFundingPolicy) {
  switch (policy.type) {
    case 'OwnerOnly':
    case 'RuntimePolicy':
    case 'AnyVerifiedIngress':
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
      on_error:
        step.errorPolicy.type === 'RetryLater'
          ? runtimeVariant('RetryLater', {
              max_attempts: step.errorPolicy.maxAttempts,
            })
          : runtimeVariant(step.errorPolicy.type),
    })),
    completion_policy: runtimeVariant(program.completionPolicy),
    funding_source_policy: lowerFundingPolicy(program.fundingPolicy),
    auto_close_at_cycle_nonce: program.autoCloseAtCycleNonce ?? undefined,
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
