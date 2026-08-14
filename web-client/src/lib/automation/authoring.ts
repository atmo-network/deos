/*
Domain: Actors linear plan authoring
Owns: Typed step drafts, immutable ordered-step operations, structural validation, exact ContractInput lowering, and canonical artifact production.
Excludes: Runtime submission, governance authority, adapter execution, simulation, weight modeling, recipes, and widget state.
Zone: Automation domain capability; composes the canonical contract-artifact codec without defining another runtime language.
*/
import { decodeAddress } from '@polkadot/util-crypto';

import {
  ACTORS_MAX_EXECUTION_PLAN_STEPS,
  ACTORS_MAX_PRECONDITION_CLAUSES,
  ACTORS_MAX_PREDICATES_PER_CLAUSE,
  ACTORS_MAX_PREDICATES_PER_STEP,
  ACTORS_MAX_RETRY_ATTEMPTS,
} from './actors-protocol-bounds.ts';
import {
  type ActorContractArtifact,
  type ActorContractRuntimeIdentity,
  type ActorContractType,
  createActorContractArtifact,
  encodeActorContractValue,
} from './contract-artifact.ts';
import type {
  AutomationMutability,
  AutomationStepErrorPolicy,
} from './types.ts';

export type ActorAuthoringAsset =
  | { type: 'Native' }
  | { type: 'Local' | 'Foreign'; id: number };

export type ActorAuthoringAmount =
  | { type: 'Fixed'; value: string }
  | {
      type:
        | 'PercentageOfCurrent'
        | 'PercentageAtOpening'
        | 'PercentageOfLastFunding';
      parts: number;
    }
  | { type: 'AllAvailable' };

export type ActorAuthoringObservationFeed = {
  assetIn: ActorAuthoringAsset;
  assetOut: ActorAuthoringAsset;
  method: 'PreExecutionSpot';
  aggregation: { type: 'LastValue' } | { type: 'Ema'; halfLifeBlocks: number };
  scale: number;
};

export type ActorAuthoringPredicate =
  | {
      type:
        | 'BalanceAbove'
        | 'BalanceBelow'
        | 'BalanceEquals'
        | 'BalanceNotEquals';
      asset: ActorAuthoringAsset;
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
      feed: ActorAuthoringObservationFeed;
      threshold: string;
      maxAgeBlocks: number;
    };

export type ActorAuthoringInputLimit =
  | { type: 'LiveQuote' }
  | { type: 'Absolute'; amount: string };

export type ActorAuthoringTask =
  | {
      type: 'Transfer';
      to: string;
      asset: ActorAuthoringAsset;
      amount: ActorAuthoringAmount;
    }
  | {
      type: 'SplitTransfer';
      asset: ActorAuthoringAsset;
      amount: ActorAuthoringAmount;
      legs: Array<{ to: string; shareParts: number }>;
    }
  | {
      type: 'SwapIn';
      assetIn: ActorAuthoringAsset;
      amountIn: ActorAuthoringAmount;
      assetOut: ActorAuthoringAsset;
      slippageParts: number;
    }
  | {
      type: 'SwapOut';
      assetOut: ActorAuthoringAsset;
      amountOut: ActorAuthoringAmount;
      assetIn: ActorAuthoringAsset;
      inputLimit: ActorAuthoringInputLimit;
      slippageParts: number;
    }
  | {
      type: 'AddLiquidity';
      assetA: ActorAuthoringAsset;
      assetB: ActorAuthoringAsset;
      amountA: ActorAuthoringAmount;
      amountB: ActorAuthoringAmount;
      minLpOut: string;
    }
  | {
      type: 'RemoveLiquidity';
      lpAsset: ActorAuthoringAsset;
      assetA: ActorAuthoringAsset;
      assetB: ActorAuthoringAsset;
      lpAmount: ActorAuthoringAmount;
      minAmountA: string;
      minAmountB: string;
    }
  | {
      type: 'Burn' | 'Mint' | 'Stake';
      asset: ActorAuthoringAsset;
      amount: ActorAuthoringAmount;
    }
  | {
      type: 'DonateLiquidity';
      assetA: ActorAuthoringAsset;
      assetB: ActorAuthoringAsset;
      maxAmountA: ActorAuthoringAmount;
      maxRatioErrorParts: number;
    }
  | {
      type: 'Unstake';
      asset: ActorAuthoringAsset;
      shares: ActorAuthoringAmount;
    }
  | { type: 'StopCycle' };

export const ACTORS_AUTHORING_TASK_TYPES = [
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
] as const satisfies readonly ActorAuthoringTask['type'][];

export const ACTORS_AUTHORING_CONDITION_TYPES = [
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
] as const satisfies readonly ActorAuthoringPredicate['type'][];

export type ActorAuthoringTimedPredicate = {
  timing: 'Opening' | 'Current';
  predicate: ActorAuthoringPredicate;
};

export type ActorAuthoringPreconditions =
  | { type: 'Unconditional' }
  | { type: 'AnyOf'; clauses: ActorAuthoringTimedPredicate[][] };

export type ActorAuthoringStep = {
  key: string;
  preconditions: ActorAuthoringPreconditions;
  task: ActorAuthoringTask;
  errorPolicy: AutomationStepErrorPolicy;
};

export type ActorAuthoringFundingPolicy =
  | { type: 'OwnerOnly' }
  | { type: 'RuntimePolicy' }
  | { type: 'AnyVerifiedIngress' }
  | { type: 'SignedAllowlist'; accounts: string[] };

export type ActorAuthoringTriggerSource =
  | { type: 'Manual' }
  | {
      type: 'OnAddressEvent';
      sourceFilter:
        | { type: 'Any' }
        | { type: 'OwnerOnly' }
        | { type: 'Whitelist'; accounts: string[] };
      assetFilter:
        | { type: 'Any' }
        | { type: 'Whitelist'; assets: ActorAuthoringAsset[] };
    }
  | { type: 'OnObservationChange'; feed: ActorAuthoringObservationFeed };

export type ActorAuthoringTrigger =
  | { type: 'Immediate'; sources: ActorAuthoringTriggerSource[] }
  | {
      type: 'Cadenced';
      everyBlocks: number;
      mode:
        | { type: 'Always' }
        | { type: 'WhenSignalled'; sources: ActorAuthoringTriggerSource[] };
    };

export type ActorAuthoringCompletionPolicy =
  | 'Persistent'
  | 'CloseAfterProductiveCycle';

export type ActorAuthoringContract = {
  actorType: ActorContractType;
  mutability: AutomationMutability;
  completionPolicy: ActorAuthoringCompletionPolicy;
  autoCloseAtCycleNonce: bigint | null;
  trigger: ActorAuthoringTrigger;
  cooldownBlocks: number;
  scheduleWindow: { start: number; end: number } | null;
  fundingPolicy: ActorAuthoringFundingPolicy;
  steps: ActorAuthoringStep[];
};

export type ActorAuthoringLimits = {
  maxExecutionPlanSteps: number;
  maxRetryAttempts: number;
  maxPreconditionClauses: number;
  maxPredicatesPerClause: number;
  maxConditionsPerStep: number;
  maxSplitTransferLegs: number;
  maxWhitelistSize: number;
  maxTriggerSources: number;
};

export const DEOS_ACTORS_AUTHORING_LIMITS: ActorAuthoringLimits = {
  maxExecutionPlanSteps: ACTORS_MAX_EXECUTION_PLAN_STEPS,
  maxRetryAttempts: ACTORS_MAX_RETRY_ATTEMPTS,
  maxPreconditionClauses: ACTORS_MAX_PRECONDITION_CLAUSES,
  maxPredicatesPerClause: ACTORS_MAX_PREDICATES_PER_CLAUSE,
  maxConditionsPerStep: ACTORS_MAX_PREDICATES_PER_STEP,
  maxSplitTransferLegs: 8,
  maxWhitelistSize: 16,
  maxTriggerSources: 4,
};

export type ActorAuthoringIssue = {
  path: string;
  message: string;
};

export type ActorAuthoringValidation =
  | { valid: true; issues: [] }
  | { valid: false; issues: ActorAuthoringIssue[] };

export function createActorAuthoringTask(
  type: ActorAuthoringTask['type'],
): ActorAuthoringTask {
  const native = (): ActorAuthoringAsset => ({ type: 'Native' });
  const local = (): ActorAuthoringAsset => ({ type: 'Local', id: 0 });
  const fixed = (): ActorAuthoringAmount => ({ type: 'Fixed', value: '0' });
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

export function createActorAuthoringPredicate(
  type: ActorAuthoringPredicate['type'],
): ActorAuthoringPredicate {
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

export function createActorAuthoringStep(
  key: string,
  task: ActorAuthoringTask = createActorAuthoringTask('Transfer'),
): ActorAuthoringStep {
  return {
    key,
    preconditions: { type: 'Unconditional' },
    task,
    errorPolicy: { type: 'AbortCycle' },
  };
}

export function createActorAuthoringContract(): ActorAuthoringContract {
  return {
    actorType: 'User',
    mutability: 'Mutable',
    completionPolicy: 'Persistent',
    autoCloseAtCycleNonce: null,
    trigger: { type: 'Immediate', sources: [{ type: 'Manual' }] },
    cooldownBlocks: 0,
    scheduleWindow: null,
    fundingPolicy: { type: 'OwnerOnly' },
    steps: [createActorAuthoringStep('step-1')],
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
  issues: ActorAuthoringIssue[],
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
  issues: ActorAuthoringIssue[],
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
  asset: ActorAuthoringAsset,
  path: string,
  issues: ActorAuthoringIssue[],
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
  amount: ActorAuthoringAmount,
  path: string,
  issues: ActorAuthoringIssue[],
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
  issues: ActorAuthoringIssue[],
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
  feed: ActorAuthoringObservationFeed,
  path: string,
  issues: ActorAuthoringIssue[],
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

function validatePredicate(
  condition: ActorAuthoringPredicate,
  path: string,
  issues: ActorAuthoringIssue[],
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
  assetA: ActorAuthoringAsset,
  assetB: ActorAuthoringAsset,
  path: string,
  issues: ActorAuthoringIssue[],
) {
  if (
    bytesKey(assetCanonicalBytes(assetA)) ===
    bytesKey(assetCanonicalBytes(assetB))
  ) {
    issues.push({ path, message: 'Market task assets must be distinct' });
  }
}

function validateTask(
  task: ActorAuthoringTask,
  path: string,
  issues: ActorAuthoringIssue[],
  limits: ActorAuthoringLimits,
  actorType: ActorContractType,
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
      if (actorType !== 'System') {
        issues.push({
          path,
          message: 'Mint is available only to System Actors',
        });
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
  issues: ActorAuthoringIssue[],
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
  sources: ActorAuthoringTriggerSource[],
  path: string,
  issues: ActorAuthoringIssue[],
  limits: ActorAuthoringLimits,
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
  trigger: ActorAuthoringTrigger,
  issues: ActorAuthoringIssue[],
  limits: ActorAuthoringLimits,
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

export function validateActorAuthoringContract(
  contract: ActorAuthoringContract,
  limits = DEOS_ACTORS_AUTHORING_LIMITS,
): ActorAuthoringValidation {
  const issues: ActorAuthoringIssue[] = [];
  const maxSteps = limits.maxExecutionPlanSteps;
  if (
    contract.completionPolicy !== 'Persistent' &&
    contract.completionPolicy !== 'CloseAfterProductiveCycle'
  ) {
    issues.push({
      path: 'completionPolicy',
      message:
        'Completion policy must be Persistent or CloseAfterProductiveCycle',
    });
  }
  if (
    contract.autoCloseAtCycleNonce != null &&
    (contract.autoCloseAtCycleNonce <= 0n ||
      contract.autoCloseAtCycleNonce > U64_MAX)
  ) {
    issues.push({
      path: 'autoCloseAtCycleNonce',
      message: 'Auto-close target must be a nonzero u64 logical-cycle nonce',
    });
  }
  if (contract.steps.length === 0 || contract.steps.length > maxSteps) {
    issues.push({
      path: 'steps',
      message: `Active ${contract.actorType} Actor Contract requires 1..${maxSteps} steps`,
    });
  }
  if (!isU32(contract.cooldownBlocks)) {
    issues.push({
      path: 'cooldownBlocks',
      message: 'Cooldown must be a u32',
    });
  }
  if (
    contract.scheduleWindow != null &&
    (!isU32(contract.scheduleWindow.start) ||
      !isU32(contract.scheduleWindow.end) ||
      contract.scheduleWindow.end <= contract.scheduleWindow.start)
  ) {
    issues.push({
      path: 'scheduleWindow',
      message:
        'Schedule window requires u32 bounds with end greater than start',
    });
  }
  validateTrigger(contract.trigger, issues, limits);
  if (contract.fundingPolicy.type === 'SignedAllowlist') {
    validateUniqueAddresses(
      contract.fundingPolicy.accounts,
      'fundingPolicy.accounts',
      issues,
      limits.maxWhitelistSize,
    );
  }
  const keys = new Set<string>();
  contract.steps.forEach((step, index) => {
    const path = `steps[${index}]`;
    if (step.key.length === 0 || keys.has(step.key)) {
      issues.push({
        path: `${path}.key`,
        message: 'Authoring step keys must be non-empty and unique',
      });
    }
    keys.add(step.key);
    const clauses =
      step.preconditions.type === 'Unconditional'
        ? []
        : step.preconditions.clauses;
    if (step.preconditions.type === 'AnyOf' && clauses.length === 0) {
      issues.push({
        path: `${path}.preconditions.clauses`,
        message: 'AnyOf requires at least one clause',
      });
    }
    if (clauses.length > limits.maxPreconditionClauses) {
      issues.push({
        path: `${path}.preconditions.clauses`,
        message: `A step supports at most ${limits.maxPreconditionClauses} precondition clauses`,
      });
    }
    const predicateCount = clauses.reduce(
      (total, clause) => total + clause.length,
      0,
    );
    if (predicateCount > limits.maxConditionsPerStep) {
      issues.push({
        path: `${path}.preconditions.clauses`,
        message: `A step supports at most ${limits.maxConditionsPerStep} predicates`,
      });
    }
    clauses.forEach((clause, clauseIndex) => {
      if (clause.length === 0) {
        issues.push({
          path: `${path}.preconditions.clauses[${clauseIndex}]`,
          message: 'A precondition clause must not be empty',
        });
      }
      if (clause.length > limits.maxPredicatesPerClause) {
        issues.push({
          path: `${path}.preconditions.clauses[${clauseIndex}]`,
          message: `A precondition clause supports at most ${limits.maxPredicatesPerClause} predicates`,
        });
      }
      clause.forEach((timed, predicateIndex) => {
        if (timed.timing !== 'Opening' && timed.timing !== 'Current') {
          issues.push({
            path: `${path}.preconditions.clauses[${clauseIndex}][${predicateIndex}].timing`,
            message: 'Predicate timing must be Opening or Current',
          });
        }
        validatePredicate(
          timed.predicate,
          `${path}.preconditions.clauses[${clauseIndex}][${predicateIndex}].predicate`,
          issues,
        );
      });
    });
    if (
      contract.mutability === 'Immutable' &&
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
    validateTask(step.task, `${path}.task`, issues, limits, contract.actorType);
  });
  return issues.length === 0
    ? { valid: true, issues: [] }
    : { valid: false, issues };
}

export function appendActorStep(
  contract: ActorAuthoringContract,
  step: ActorAuthoringStep,
): ActorAuthoringContract {
  return { ...contract, steps: [...contract.steps, structuredClone(step)] };
}

export function replaceActorStep(
  contract: ActorAuthoringContract,
  key: string,
  step: ActorAuthoringStep,
): ActorAuthoringContract {
  const index = contract.steps.findIndex((candidate) => candidate.key === key);
  if (index < 0) throw new Error(`Unknown authoring step key: ${key}`);
  const steps = contract.steps.map((candidate, candidateIndex) =>
    candidateIndex === index ? structuredClone(step) : candidate,
  );
  return { ...contract, steps };
}

export function removeActorStep(
  contract: ActorAuthoringContract,
  key: string,
): ActorAuthoringContract {
  const index = contract.steps.findIndex((candidate) => candidate.key === key);
  if (index < 0) throw new Error(`Unknown authoring step key: ${key}`);
  return {
    ...contract,
    steps: contract.steps.filter(
      (_, candidateIndex) => candidateIndex !== index,
    ),
  };
}

export function moveActorStep(
  contract: ActorAuthoringContract,
  fromIndex: number,
  toIndex: number,
): ActorAuthoringContract {
  if (
    !Number.isSafeInteger(fromIndex) ||
    !Number.isSafeInteger(toIndex) ||
    fromIndex < 0 ||
    toIndex < 0 ||
    fromIndex >= contract.steps.length ||
    toIndex >= contract.steps.length
  ) {
    throw new Error(
      'Step move indexes must address the current ordered contract',
    );
  }
  if (fromIndex === toIndex) return contract;
  const steps = [...contract.steps];
  const [moved] = steps.splice(fromIndex, 1);
  steps.splice(toIndex, 0, moved);
  return { ...contract, steps };
}

function runtimeVariant(type: string, value: unknown = undefined) {
  return { type, value };
}

function lowerAsset(asset: ActorAuthoringAsset) {
  switch (asset.type) {
    case 'Native':
      return runtimeVariant('Native');
    case 'Local':
    case 'Foreign':
      return runtimeVariant(asset.type, asset.id);
  }
}

function lowerAmount(amount: ActorAuthoringAmount) {
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

function lowerObservationFeed(feed: ActorAuthoringObservationFeed) {
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

function lowerPredicate(condition: ActorAuthoringPredicate) {
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

function lowerTask(task: ActorAuthoringTask) {
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

function assetCanonicalBytes(asset: ActorAuthoringAsset) {
  switch (asset.type) {
    case 'Native':
      return [0];
    case 'Local':
      return [1, ...u32Le(asset.id)];
    case 'Foreign':
      return [2, ...u32Le(asset.id)];
  }
}

function observationFeedCanonicalBytes(feed: ActorAuthoringObservationFeed) {
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
    ActorAuthoringTriggerSource,
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
    ActorAuthoringTriggerSource,
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

function triggerSourceCanonicalBytes(source: ActorAuthoringTriggerSource) {
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

function lowerTriggerSource(source: ActorAuthoringTriggerSource) {
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

function lowerTriggerSources(sources: ActorAuthoringTriggerSource[]) {
  return [...sources]
    .sort((left, right) =>
      compareBytes(
        triggerSourceCanonicalBytes(left),
        triggerSourceCanonicalBytes(right),
      ),
    )
    .map(lowerTriggerSource);
}

function lowerTrigger(trigger: ActorAuthoringTrigger) {
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

function lowerFundingPolicy(policy: ActorAuthoringFundingPolicy) {
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

export function lowerActorAuthoringContract(
  contract: ActorAuthoringContract,
  limits = DEOS_ACTORS_AUTHORING_LIMITS,
) {
  const validation = validateActorAuthoringContract(contract, limits);
  if (!validation.valid) {
    throw new Error(
      `Invalid Actors authoring contract: ${validation.issues
        .map((issue) => `${issue.path}: ${issue.message}`)
        .join('; ')}`,
    );
  }
  return runtimeVariant('Active', {
    schedule: {
      trigger: lowerTrigger(contract.trigger),
      cooldown_blocks: contract.cooldownBlocks,
    },
    schedule_window:
      contract.scheduleWindow == null
        ? undefined
        : {
            start: contract.scheduleWindow.start,
            end: contract.scheduleWindow.end,
          },
    steps: contract.steps.map((step) => ({
      preconditions:
        step.preconditions.type === 'Unconditional'
          ? runtimeVariant('Unconditional')
          : runtimeVariant(
              'AnyOf',
              step.preconditions.clauses.map((clause) =>
                clause.map((timed) => ({
                  timing: runtimeVariant(timed.timing),
                  predicate: lowerPredicate(timed.predicate),
                })),
              ),
            ),
      task: lowerTask(step.task),
      on_error:
        step.errorPolicy.type === 'RetryLater'
          ? runtimeVariant('RetryLater', {
              max_attempts: step.errorPolicy.maxAttempts,
            })
          : runtimeVariant(step.errorPolicy.type),
    })),
    completion: runtimeVariant(contract.completionPolicy),
    funding: lowerFundingPolicy(contract.fundingPolicy),
    auto_close_at_cycle_nonce: contract.autoCloseAtCycleNonce ?? undefined,
  });
}

export function createActorArtifactFromAuthoring(input: {
  contract: ActorAuthoringContract;
  metadataBytes: Uint8Array;
  runtime: ActorContractRuntimeIdentity;
  limits?: ActorAuthoringLimits;
}): ActorContractArtifact {
  const runtimeValue = lowerActorAuthoringContract(
    input.contract,
    input.limits ?? DEOS_ACTORS_AUTHORING_LIMITS,
  );
  const contractScale = encodeActorContractValue(
    input.metadataBytes,
    runtimeValue,
  );
  return createActorContractArtifact({
    metadataBytes: input.metadataBytes,
    runtime: input.runtime,
    actorType: input.contract.actorType,
    mutability: input.contract.mutability,
    contractScale,
  });
}
