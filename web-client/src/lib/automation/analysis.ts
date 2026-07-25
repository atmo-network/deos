/*
Domain: AAA static program analysis
Owns: Identity-bound structural composition, semantic surfaces, forward dependencies, failure controls, and per-cursor suffix envelopes.
Excludes: Runtime state claims, adapter execution, SCALE implementation, independent weight calculation, signing, submission, and graph authoring.
Zone: Automation domain capability; consumes canonical plan inspection and forecast contracts.
*/
import {
  type AaaCostSegment,
  type AaaStepCostInput,
  type AaaWeight,
  forecastAaaCosts,
} from './forecast.ts';
import {
  type AaaPlanArtifact,
  type AaaPlanHex,
  type AaaPlanProjection,
  type AaaPlanRuntimeIdentity,
  inspectAaaPlanArtifact,
} from './plan-artifact.ts';

export const AAA_STATIC_ANALYZER_VERSION = '1' as const;

export type AaaTaskName =
  | 'Transfer'
  | 'SplitTransfer'
  | 'SwapExactIn'
  | 'SwapExactOut'
  | 'AddLiquidity'
  | 'RemoveLiquidity'
  | 'Burn'
  | 'Mint'
  | 'Stake'
  | 'DonateLiquidity'
  | 'Unstake'
  | 'StopCycle';

export type AaaConditionName =
  | 'BalanceAbove'
  | 'BalanceBelow'
  | 'BalanceEquals'
  | 'BalanceNotEquals'
  | 'BlockNumberAbove'
  | 'BlockNumberBelow';

export type AaaAmountName =
  | 'Fixed'
  | 'PercentageOfCurrent'
  | 'PercentageOfTrigger'
  | 'PercentageOfLastFunding'
  | 'AllBalance';

export type AaaRequiredAdapter =
  | 'AssetOps'
  | 'DexOps'
  | 'StakingOps'
  | 'LiquidityDonationOps';

export type AaaStaticObservationWindow =
  | 'artifact-time'
  | 'logical-run-start'
  | 'step-attempt-time'
  | 'retry-time';

export type AaaStaticStepControl = 'advance' | 'terminate' | 'stutter-current';

export type AaaTemporaryFailureReachability = 'yes' | 'no' | 'unknown';

export type AaaStaticWeightModel = {
  identity: string;
  version: string;
  stepBaseFee: bigint;
  conditionReadFee: bigint;
  evaluationWeight: (conditionCount: number) => AaaWeight;
  taskUpper: (input: {
    task: AaaTaskName;
    parameters: AaaPlanProjection;
    splitLegs: number;
  }) => { weight: AaaWeight; executionFeeUpper: bigint };
  lifecycleOverhead: AaaCostSegment;
  fundingPromotionOverhead: AaaCostSegment;
  referenceBudget?: AaaWeight;
};

export type AaaAdapterCapabilityProfile = {
  identity: string;
  adapters?: Partial<
    Record<AaaRequiredAdapter, 'supported' | 'unsupported' | 'unknown'>
  >;
  temporaryFailures?: Partial<
    Record<AaaTaskName, AaaTemporaryFailureReachability>
  >;
};

export type AaaAmountSemantics = {
  path: string;
  resolution: AaaAmountName;
  dataDependencies: Array<
    | 'artifact-value'
    | 'current-balance-or-shares'
    | 'trigger-snapshot'
    | 'last-funding-snapshot'
    | 'task-policy-capacity'
  >;
  minimumBalanceDependency: 'task-policy';
  feeReserveDependency: 'task-policy';
  valueObservation: AaaStaticObservationWindow;
  retryObservation: 'reobserve-live' | 'reuse-frozen-with-live-capacity';
};

export type AaaStaticCost = {
  refTime: string;
  proofSize: string;
  feeUpper: string;
};

export type AaaStaticStepAnalysis = {
  index: number;
  conditionSet: {
    mode: 'Always' | 'All' | 'Any';
    atomicCount: number;
    evaluation: 'all-atoms-no-short-circuit';
    admission: 'always' | 'all-true' | 'at-least-one-true';
    falseControl: 'advance-fixed-successor';
    atomicError: 'fail-whole-group';
  };
  conditions: Array<{
    type: AaaConditionName;
    value: AaaPlanProjection;
    observation: 'balance' | 'block-number';
    readSurface: AaaPlanProjection | 'current-block';
    pure: true;
    observationWindow: 'step-attempt-time';
    boundedReadCount: 1;
  }>;
  task: AaaTaskName;
  parameters: AaaPlanProjection;
  amounts: AaaAmountSemantics[];
  errorPolicy: 'AbortCycle' | 'ContinueNextStep' | 'RetryLater';
  possibleControls: AaaStaticStepControl[];
  requiredAdapters: AaaRequiredAdapter[];
  costs: {
    evaluation: AaaStaticCost;
    executionUpper: AaaStaticCost;
    totalUpper: AaaStaticCost;
  };
  economicSurface: {
    assetsRead: AaaPlanProjection[];
    assetsWritten: AaaPlanProjection[];
    adapterDerivedAssetsRead: boolean;
    adapterDerivedAssetsWritten: boolean;
    recipients: AaaPlanProjection[];
    transferExposure: boolean;
    mintExposure: boolean;
    burnExposure: boolean;
    liquidityMutation: boolean;
    stakingMutation: boolean;
    possibleActorSignals: AaaPlanProjection[];
    committedNonCompensatedEffects: boolean;
  };
  failureSurface: {
    possibleContinue: boolean;
    possibleAbort: boolean;
    possibleRetryCurrent: boolean;
    temporaryFailureReachability: AaaTemporaryFailureReachability;
    continuationEligible: boolean;
  };
};

export type AaaForwardDataDependency = {
  fromStep: number;
  toStep: number;
  asset: AaaPlanProjection;
  readBy: 'condition' | 'task-or-amount';
  observationWindows: AaaStaticObservationWindow[];
};

export type AaaStaticSuffixEnvelope = {
  cursor: number;
  remainingSteps: number;
  maximumRefTime: string;
  maximumProofSize: string;
  evaluationFeeUpper: string;
  executionFeeUpper: string;
  lifecycleOverhead: AaaStaticCost;
  fundingPromotionOverhead: AaaStaticCost;
  requiredAdapters: AaaRequiredAdapter[];
  assetsRead: AaaPlanProjection[];
  assetsWritten: AaaPlanProjection[];
  committedEffectClasses: Array<
    'transfer' | 'mint' | 'burn' | 'liquidity' | 'staking'
  >;
  retryableSteps: number[];
};

export type AaaStaticFinding =
  | {
      kind: 'CommittedEffectBeforeRetryableStep';
      before: number;
      retryable: number;
    }
  | { kind: 'LiveRetryTimeObservation'; step: number; path: string }
  | {
      kind: 'PreExistingBalanceMixedWithCurrentRunOutput';
      writer: number;
      reader: number;
      asset: AaaPlanProjection;
    }
  | {
      kind: 'AdapterCapability';
      step: number;
      adapter: AaaRequiredAdapter;
      status: 'unsupported' | 'unknown';
    }
  | { kind: 'UnknownTemporaryFailureClassification'; step: number }
  | {
      kind: 'StopCycleFailureMayFallThrough';
      step: number;
      suffixHasEconomicEffects: boolean;
    }
  | {
      kind: 'PotentialCrossActorFeedbackEdge';
      step: number;
      recipient: AaaPlanProjection;
    }
  | { kind: 'ProofSizeDominantSuffix'; cursor: number }
  | {
      kind: 'AdministrativeInvalidationSurface';
      conditional: true;
      actions: Array<
        | 'execution-plan-replacement'
        | 'schedule-replacement'
        | 'funding-policy-replacement'
        | 'deactivation'
        | 'cancellation'
        | 'terminal-transition'
        | 'incompatible-runtime-upgrade'
      >;
    };

export type ProgramStaticAnalysis = {
  provenance: 'StaticStructuralProjection';
  identity: {
    planId: AaaPlanHex;
    genesisHash: AaaPlanHex;
    metadataHash: AaaPlanHex;
    specVersion: number;
    transactionVersion: number;
    runtimeModelIdentity: string;
    weightModelIdentity: string;
    adapterCapabilityIdentity: string | null;
    analyzerVersion: typeof AAA_STATIC_ANALYZER_VERSION;
  };
  program: 'Dormant' | 'Active';
  actorType: AaaPlanArtifact['aaaType'];
  mutability: AaaPlanArtifact['mutability'];
  steps: AaaStaticStepAnalysis[];
  economicSurface: AaaStaticStepAnalysis['economicSurface'];
  dataDependencies: AaaForwardDataDependency[];
  suffixEnvelopes: AaaStaticSuffixEnvelope[];
  findings: AaaStaticFinding[];
};

type ParsedVariant = { type: string; value: AaaPlanProjection };

type TaskSemantics = {
  adapter: AaaRequiredAdapter | null;
  assetsRead: AaaPlanProjection[];
  assetsWritten: AaaPlanProjection[];
  adapterDerivedAssetsRead: boolean;
  adapterDerivedAssetsWritten: boolean;
  recipients: AaaPlanProjection[];
  effects: Array<'transfer' | 'mint' | 'burn' | 'liquidity' | 'staking'>;
};

function record(value: AaaPlanProjection, label: string) {
  if (value == null || Array.isArray(value) || typeof value !== 'object') {
    throw new Error(`${label} must be an object projection`);
  }
  return value as Record<string, AaaPlanProjection>;
}

function array(value: AaaPlanProjection, label: string) {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value;
}

function variant(value: AaaPlanProjection, label: string): ParsedVariant {
  const projected = record(value, label);
  if (typeof projected.type !== 'string' || !('value' in projected)) {
    throw new Error(`${label} must be a projected runtime variant`);
  }
  return { type: projected.type, value: projected.value };
}

function member(
  value: AaaPlanProjection,
  key: string,
  label: string,
): AaaPlanProjection {
  const projected = record(value, label);
  if (!(key in projected)) throw new Error(`${label}.${key} is required`);
  return projected[key];
}

function fingerprint(value: AaaPlanProjection) {
  return JSON.stringify(value);
}

function uniqueProjection(values: AaaPlanProjection[]) {
  const seen = new Set<string>();
  return values.filter((value) => {
    const key = fingerprint(value);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function unique<T>(values: T[]) {
  return [...new Set(values)];
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

function staticCost(weight: AaaWeight, feeUpper: bigint): AaaStaticCost {
  return {
    refTime: weight.refTime.toString(),
    proofSize: weight.proofSize.toString(),
    feeUpper: feeUpper.toString(),
  };
}

function validateModel(model: AaaStaticWeightModel) {
  for (const [field, value] of [
    ['stepBaseFee', model.stepBaseFee],
    ['conditionReadFee', model.conditionReadFee],
    ['lifecycle fee', model.lifecycleOverhead.fee],
    ['funding fee', model.fundingPromotionOverhead.fee],
  ] as const) {
    if (value < 0n) throw new Error(`${field} must be non-negative`);
  }
  if (model.identity.length === 0 || model.version.length === 0) {
    throw new Error('Weight model identity and version are required');
  }
}

function asTaskName(value: string): AaaTaskName {
  switch (value) {
    case 'Transfer':
    case 'SplitTransfer':
    case 'SwapExactIn':
    case 'SwapExactOut':
    case 'AddLiquidity':
    case 'RemoveLiquidity':
    case 'Burn':
    case 'Mint':
    case 'Stake':
    case 'DonateLiquidity':
    case 'Unstake':
    case 'StopCycle':
      return value;
    default:
      throw new Error(`Unsupported Task variant: ${value}`);
  }
}

function asConditionName(value: string): AaaConditionName {
  switch (value) {
    case 'BalanceAbove':
    case 'BalanceBelow':
    case 'BalanceEquals':
    case 'BalanceNotEquals':
    case 'BlockNumberAbove':
    case 'BlockNumberBelow':
      return value;
    default:
      throw new Error(`Unsupported Condition variant: ${value}`);
  }
}

function asAmountName(value: string): AaaAmountName | null {
  switch (value) {
    case 'Fixed':
    case 'PercentageOfCurrent':
    case 'PercentageOfTrigger':
    case 'PercentageOfLastFunding':
    case 'AllBalance':
      return value;
    default:
      return null;
  }
}

function errorPolicy(value: AaaPlanProjection) {
  const type = variant(value, 'StepErrorPolicy').type;
  switch (type) {
    case 'AbortCycle':
    case 'ContinueNextStep':
    case 'RetryLater':
      return type;
    default:
      throw new Error(`Unsupported StepErrorPolicy variant: ${type}`);
  }
}

function collectAmounts(
  value: AaaPlanProjection,
  path = '',
): AaaAmountSemantics[] {
  if (Array.isArray(value)) {
    return value.flatMap((child, index) =>
      collectAmounts(child, `${path}/${index}`),
    );
  }
  if (value == null || typeof value !== 'object') return [];
  const projected = value as Record<string, AaaPlanProjection>;
  const type = typeof projected.type === 'string' ? projected.type : null;
  const amount = type == null ? null : asAmountName(type);
  if (amount != null) {
    const [dependency, valueObservation, retryObservation] = (() => {
      switch (amount) {
        case 'Fixed':
          return [
            'artifact-value',
            'artifact-time',
            'reuse-frozen-with-live-capacity',
          ] as const;
        case 'PercentageOfCurrent':
        case 'AllBalance':
          return [
            'current-balance-or-shares',
            'step-attempt-time',
            'reobserve-live',
          ] as const;
        case 'PercentageOfTrigger':
          return [
            'trigger-snapshot',
            'logical-run-start',
            'reuse-frozen-with-live-capacity',
          ] as const;
        case 'PercentageOfLastFunding':
          return [
            'last-funding-snapshot',
            'logical-run-start',
            'reuse-frozen-with-live-capacity',
          ] as const;
      }
    })();
    return [
      {
        path,
        resolution: amount,
        dataDependencies: [dependency, 'task-policy-capacity'],
        minimumBalanceDependency: 'task-policy',
        feeReserveDependency: 'task-policy',
        valueObservation,
        retryObservation,
      },
    ];
  }
  return Object.keys(projected)
    .sort()
    .flatMap((key) => collectAmounts(projected[key], `${path}/${key}`));
}

function taskSemantics(
  task: AaaTaskName,
  parameters: AaaPlanProjection,
): TaskSemantics {
  const asset = (key: string) => member(parameters, key, task);
  const actorRecipient = (value: AaaPlanProjection) => [value];
  switch (task) {
    case 'Transfer': {
      const current = asset('asset');
      return {
        adapter: 'AssetOps',
        assetsRead: [current],
        assetsWritten: [current],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: actorRecipient(member(parameters, 'to', task)),
        effects: ['transfer'],
      };
    }
    case 'SplitTransfer': {
      const current = asset('asset');
      const legs = array(member(parameters, 'legs', task), `${task}.legs`);
      return {
        adapter: 'AssetOps',
        assetsRead: [current],
        assetsWritten: [current],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: legs.map((leg) => member(leg, 'to', 'SplitTransfer.leg')),
        effects: ['transfer'],
      };
    }
    case 'SwapExactIn':
    case 'SwapExactOut': {
      const assetIn = asset('asset_in');
      const assetOut = asset('asset_out');
      return {
        adapter: 'DexOps',
        assetsRead: uniqueProjection([assetIn, assetOut]),
        assetsWritten: uniqueProjection([assetIn, assetOut]),
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: [],
        effects: ['transfer', 'liquidity'],
      };
    }
    case 'AddLiquidity': {
      const assetA = asset('asset_a');
      const assetB = asset('asset_b');
      return {
        adapter: 'DexOps',
        assetsRead: uniqueProjection([assetA, assetB]),
        assetsWritten: uniqueProjection([assetA, assetB]),
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: true,
        recipients: [],
        effects: ['liquidity'],
      };
    }
    case 'RemoveLiquidity': {
      const lpAsset = asset('lp_asset');
      return {
        adapter: 'DexOps',
        assetsRead: [lpAsset],
        assetsWritten: [lpAsset],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: true,
        recipients: [],
        effects: ['liquidity'],
      };
    }
    case 'Burn': {
      const current = asset('asset');
      return {
        adapter: 'AssetOps',
        assetsRead: [current],
        assetsWritten: [current],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: [],
        effects: ['burn'],
      };
    }
    case 'Mint': {
      const current = asset('asset');
      return {
        adapter: 'AssetOps',
        assetsRead: [],
        assetsWritten: [current],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: [],
        effects: ['mint'],
      };
    }
    case 'Stake': {
      const current = asset('asset');
      return {
        adapter: 'StakingOps',
        assetsRead: [current],
        assetsWritten: [current],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: true,
        recipients: [],
        effects: ['staking'],
      };
    }
    case 'DonateLiquidity': {
      const assetA = asset('asset_a');
      const assetB = asset('asset_b');
      return {
        adapter: 'LiquidityDonationOps',
        assetsRead: uniqueProjection([assetA, assetB]),
        assetsWritten: uniqueProjection([assetA, assetB]),
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: [],
        effects: ['liquidity'],
      };
    }
    case 'Unstake': {
      const current = asset('asset');
      return {
        adapter: 'StakingOps',
        assetsRead: [current],
        assetsWritten: [current],
        adapterDerivedAssetsRead: true,
        adapterDerivedAssetsWritten: true,
        recipients: [],
        effects: ['staking'],
      };
    }
    case 'StopCycle':
      return {
        adapter: null,
        assetsRead: [],
        assetsWritten: [],
        adapterDerivedAssetsRead: false,
        adapterDerivedAssetsWritten: false,
        recipients: [],
        effects: [],
      };
  }
}

function conditionAnalysis(condition: AaaPlanProjection) {
  const parsed = variant(condition, 'Condition');
  const type = asConditionName(parsed.type);
  const balance = type.startsWith('Balance');
  return {
    type,
    value: parsed.value,
    observation: balance ? ('balance' as const) : ('block-number' as const),
    readSurface: balance
      ? member(parsed.value, 'asset', `Condition.${type}`)
      : ('current-block' as const),
    pure: true as const,
    observationWindow: 'step-attempt-time' as const,
    boundedReadCount: 1 as const,
  };
}

function controlsFor(
  policy: AaaStaticStepAnalysis['errorPolicy'],
  mutability: AaaPlanArtifact['mutability'],
) {
  switch (policy) {
    case 'ContinueNextStep':
      return ['advance'] as AaaStaticStepControl[];
    case 'AbortCycle':
      return ['advance', 'terminate'] as AaaStaticStepControl[];
    case 'RetryLater':
      return mutability === 'Mutable'
        ? ([
            'advance',
            'terminate',
            'stutter-current',
          ] as AaaStaticStepControl[])
        : (['advance', 'terminate'] as AaaStaticStepControl[]);
  }
}

function capabilityStatus(
  profile: AaaAdapterCapabilityProfile | undefined,
  adapter: AaaRequiredAdapter,
) {
  return profile?.adapters?.[adapter] ?? 'unknown';
}

function temporaryReachability(
  profile: AaaAdapterCapabilityProfile | undefined,
  task: AaaTaskName,
): AaaTemporaryFailureReachability {
  return profile?.temporaryFailures?.[task] ?? 'unknown';
}

function stepCost(
  artifact: AaaPlanArtifact,
  model: AaaStaticWeightModel,
  index: number,
  conditionCount: number,
  task: AaaTaskName,
  parameters: AaaPlanProjection,
) {
  const splitLegs =
    task === 'SplitTransfer'
      ? array(member(parameters, 'legs', task), `${task}.legs`).length
      : 0;
  const evaluationWeight = model.evaluationWeight(conditionCount);
  const upper = model.taskUpper({ task, parameters, splitLegs });
  const forecast = forecastAaaCosts({
    artifact,
    blockHash: artifact.genesisHash,
    blockNumber: 0,
    model: model.identity,
    modelVersion: model.version,
    actorType: artifact.aaaType,
    stepBaseFee: model.stepBaseFee,
    conditionReadFee: model.conditionReadFee,
    steps: [
      {
        stepIndex: 0,
        conditionCount,
        conditionOutcome: 'Unknown',
        executionDisposition: 'Unknown',
        evaluationWeight,
        executionWeightUpper: upper.weight,
        executionFeeUpper: upper.executionFeeUpper,
      },
    ],
    lifecycle: { weight: { refTime: 0n, proofSize: 0n }, fee: 0n },
  });
  return {
    input: {
      stepIndex: index,
      conditionCount,
      conditionOutcome: 'Unknown' as const,
      executionDisposition: 'Unknown' as const,
      evaluationWeight,
      executionWeightUpper: upper.weight,
      executionFeeUpper: upper.executionFeeUpper,
    },
    output: {
      evaluation: staticCost(
        forecast.evaluation.weight,
        forecast.evaluation.fee,
      ),
      executionUpper: staticCost(
        forecast.executionUpper.weight,
        forecast.executionUpper.fee,
      ),
      totalUpper: staticCost(
        forecast.totalUpper.weight,
        forecast.totalUpper.fee,
      ),
    },
  };
}

function parseSteps(
  artifact: AaaPlanArtifact,
  executionPlan: AaaPlanProjection,
  model: AaaStaticWeightModel,
  capabilities?: AaaAdapterCapabilityProfile,
) {
  const forecastInputs: AaaStepCostInput[] = [];
  const steps = array(executionPlan, 'ProgramInput.execution_plan').map(
    (projectedStep, index): AaaStaticStepAnalysis => {
      const parsedConditionSet = variant(
        member(projectedStep, 'conditions', `Step ${index}`),
        `Step ${index}.conditions`,
      );
      if (!['Always', 'All', 'Any'].includes(parsedConditionSet.type)) {
        throw new Error(
          `Unsupported ConditionSet variant: ${parsedConditionSet.type}`,
        );
      }
      const mode = parsedConditionSet.type as 'Always' | 'All' | 'Any';
      const conditions =
        mode === 'Always'
          ? []
          : array(
              parsedConditionSet.value,
              `Step ${index}.conditions.${mode}`,
            ).map(conditionAnalysis);
      if (mode !== 'Always' && conditions.length === 0) {
        throw new Error(`${mode} condition set must remain non-empty`);
      }
      const parsedTask = variant(
        member(projectedStep, 'task', `Step ${index}`),
        `Step ${index}.task`,
      );
      const task = asTaskName(parsedTask.type);
      const parameters = parsedTask.value;
      const policy = errorPolicy(
        member(projectedStep, 'on_error', `Step ${index}`),
      );
      const semantics = taskSemantics(task, parameters);
      const costs = stepCost(
        artifact,
        model,
        index,
        conditions.length,
        task,
        parameters,
      );
      forecastInputs.push(costs.input);
      const temporary = temporaryReachability(capabilities, task);
      const continuationEligible =
        task !== 'StopCycle' &&
        artifact.mutability === 'Mutable' &&
        policy === 'RetryLater';
      return {
        index,
        conditionSet: {
          mode,
          atomicCount: conditions.length,
          evaluation: 'all-atoms-no-short-circuit',
          admission:
            mode === 'Always'
              ? 'always'
              : mode === 'All'
                ? 'all-true'
                : 'at-least-one-true',
          falseControl: 'advance-fixed-successor',
          atomicError: 'fail-whole-group',
        },
        conditions,
        task,
        parameters,
        amounts: collectAmounts(parameters),
        errorPolicy: policy,
        possibleControls:
          task === 'StopCycle'
            ? ['advance', 'terminate']
            : controlsFor(policy, artifact.mutability),
        requiredAdapters: semantics.adapter == null ? [] : [semantics.adapter],
        costs: costs.output,
        economicSurface: {
          assetsRead: semantics.assetsRead,
          assetsWritten: semantics.assetsWritten,
          adapterDerivedAssetsRead: semantics.adapterDerivedAssetsRead,
          adapterDerivedAssetsWritten: semantics.adapterDerivedAssetsWritten,
          recipients: semantics.recipients,
          transferExposure: semantics.effects.includes('transfer'),
          mintExposure: semantics.effects.includes('mint'),
          burnExposure: semantics.effects.includes('burn'),
          liquidityMutation: semantics.effects.includes('liquidity'),
          stakingMutation: semantics.effects.includes('staking'),
          possibleActorSignals: semantics.recipients,
          committedNonCompensatedEffects: semantics.effects.length > 0,
        },
        failureSurface: {
          possibleContinue:
            task !== 'StopCycle' && policy === 'ContinueNextStep',
          possibleAbort: task !== 'StopCycle' && policy !== 'ContinueNextStep',
          possibleRetryCurrent: continuationEligible,
          temporaryFailureReachability: task === 'StopCycle' ? 'no' : temporary,
          continuationEligible,
        },
      };
    },
  );
  return { steps, forecastInputs };
}

function dependencyWindows(step: AaaStaticStepAnalysis) {
  return unique([
    ...step.conditions.map(() => 'step-attempt-time' as const),
    ...step.amounts.flatMap((amount) => {
      const windows = [amount.valueObservation];
      if (amount.retryObservation === 'reobserve-live')
        windows.push('retry-time');
      return windows;
    }),
  ]);
}

function forwardDependencies(steps: AaaStaticStepAnalysis[]) {
  const dependencies: AaaForwardDataDependency[] = [];
  for (let from = 0; from < steps.length; from += 1) {
    for (let to = from + 1; to < steps.length; to += 1) {
      for (const written of steps[from].economicSurface.assetsWritten) {
        const conditionMatch = steps[to].conditions.some(
          (condition) =>
            condition.readSurface !== 'current-block' &&
            fingerprint(condition.readSurface) === fingerprint(written),
        );
        const taskMatch = steps[to].economicSurface.assetsRead.some(
          (read) => fingerprint(read) === fingerprint(written),
        );
        if (!conditionMatch && !taskMatch) continue;
        dependencies.push({
          fromStep: from,
          toStep: to,
          asset: written,
          readBy: conditionMatch ? 'condition' : 'task-or-amount',
          observationWindows: dependencyWindows(steps[to]),
        });
      }
    }
  }
  return dependencies;
}

function suffixEnvelopes(
  artifact: AaaPlanArtifact,
  steps: AaaStaticStepAnalysis[],
  forecastInputs: AaaStepCostInput[],
  model: AaaStaticWeightModel,
) {
  const lifecycle = addSegment(
    model.lifecycleOverhead,
    model.fundingPromotionOverhead,
  );
  const envelopes: AaaStaticSuffixEnvelope[] = [];
  for (let cursor = 0; cursor <= steps.length; cursor += 1) {
    const suffixInputs = forecastInputs.slice(cursor).map((input, index) => ({
      ...input,
      stepIndex: index,
    }));
    const forecast = forecastAaaCosts({
      artifact,
      blockHash: artifact.genesisHash,
      blockNumber: 0,
      model: model.identity,
      modelVersion: model.version,
      actorType: artifact.aaaType,
      stepBaseFee: model.stepBaseFee,
      conditionReadFee: model.conditionReadFee,
      steps: suffixInputs,
      lifecycle,
    });
    const suffix = steps.slice(cursor);
    envelopes.push({
      cursor,
      remainingSteps: suffix.length,
      maximumRefTime: forecast.totalUpper.weight.refTime.toString(),
      maximumProofSize: forecast.totalUpper.weight.proofSize.toString(),
      evaluationFeeUpper: forecast.evaluation.fee.toString(),
      executionFeeUpper: forecast.executionUpper.fee.toString(),
      lifecycleOverhead: staticCost(
        model.lifecycleOverhead.weight,
        model.lifecycleOverhead.fee,
      ),
      fundingPromotionOverhead: staticCost(
        model.fundingPromotionOverhead.weight,
        model.fundingPromotionOverhead.fee,
      ),
      requiredAdapters: unique(suffix.flatMap((step) => step.requiredAdapters)),
      assetsRead: uniqueProjection(
        suffix.flatMap((step) => step.economicSurface.assetsRead),
      ),
      assetsWritten: uniqueProjection(
        suffix.flatMap((step) => step.economicSurface.assetsWritten),
      ),
      committedEffectClasses: unique(
        suffix.flatMap((step) => {
          const effects: AaaStaticSuffixEnvelope['committedEffectClasses'] = [];
          if (step.task === 'Transfer' || step.task === 'SplitTransfer') {
            effects.push('transfer');
          }
          if (step.economicSurface.mintExposure) effects.push('mint');
          if (step.economicSurface.burnExposure) effects.push('burn');
          if (step.economicSurface.liquidityMutation) effects.push('liquidity');
          if (step.economicSurface.stakingMutation) effects.push('staking');
          return effects;
        }),
      ),
      retryableSteps: suffix
        .filter((step) => step.failureSurface.continuationEligible)
        .map((step) => step.index),
    });
  }
  return envelopes;
}

function aggregateEconomicSurface(steps: AaaStaticStepAnalysis[]) {
  return {
    assetsRead: uniqueProjection(
      steps.flatMap((step) => step.economicSurface.assetsRead),
    ),
    assetsWritten: uniqueProjection(
      steps.flatMap((step) => step.economicSurface.assetsWritten),
    ),
    adapterDerivedAssetsRead: steps.some(
      (step) => step.economicSurface.adapterDerivedAssetsRead,
    ),
    adapterDerivedAssetsWritten: steps.some(
      (step) => step.economicSurface.adapterDerivedAssetsWritten,
    ),
    recipients: uniqueProjection(
      steps.flatMap((step) => step.economicSurface.recipients),
    ),
    transferExposure: steps.some(
      (step) => step.economicSurface.transferExposure,
    ),
    mintExposure: steps.some((step) => step.economicSurface.mintExposure),
    burnExposure: steps.some((step) => step.economicSurface.burnExposure),
    liquidityMutation: steps.some(
      (step) => step.economicSurface.liquidityMutation,
    ),
    stakingMutation: steps.some((step) => step.economicSurface.stakingMutation),
    possibleActorSignals: uniqueProjection(
      steps.flatMap((step) => step.economicSurface.possibleActorSignals),
    ),
    committedNonCompensatedEffects: steps.some(
      (step) => step.economicSurface.committedNonCompensatedEffects,
    ),
  };
}

function findings(
  steps: AaaStaticStepAnalysis[],
  dependencies: AaaForwardDataDependency[],
  envelopes: AaaStaticSuffixEnvelope[],
  model: AaaStaticWeightModel,
  capabilities?: AaaAdapterCapabilityProfile,
): AaaStaticFinding[] {
  const results: AaaStaticFinding[] = [];
  for (const step of steps) {
    if (step.task === 'StopCycle' && step.errorPolicy === 'ContinueNextStep') {
      results.push({
        kind: 'StopCycleFailureMayFallThrough',
        step: step.index,
        suffixHasEconomicEffects: steps
          .slice(step.index + 1)
          .some(
            (suffixStep) =>
              suffixStep.economicSurface.committedNonCompensatedEffects,
          ),
      });
    }
    for (const adapter of step.requiredAdapters) {
      const status = capabilityStatus(capabilities, adapter);
      if (status !== 'supported') {
        results.push({
          kind: 'AdapterCapability',
          step: step.index,
          adapter,
          status,
        });
      }
    }
    if (step.failureSurface.temporaryFailureReachability === 'unknown') {
      results.push({
        kind: 'UnknownTemporaryFailureClassification',
        step: step.index,
      });
    }
    for (const amount of step.amounts) {
      if (amount.retryObservation === 'reobserve-live') {
        results.push({
          kind: 'LiveRetryTimeObservation',
          step: step.index,
          path: amount.path,
        });
      }
    }
    for (const recipient of step.economicSurface.possibleActorSignals) {
      results.push({
        kind: 'PotentialCrossActorFeedbackEdge',
        step: step.index,
        recipient,
      });
    }
  }
  for (const retryable of steps.filter(
    (step) => step.failureSurface.continuationEligible,
  )) {
    const before = steps.find(
      (step) =>
        step.index < retryable.index &&
        step.economicSurface.committedNonCompensatedEffects,
    );
    if (before != null) {
      results.push({
        kind: 'CommittedEffectBeforeRetryableStep',
        before: before.index,
        retryable: retryable.index,
      });
    }
  }
  for (const dependency of dependencies) {
    const reader = steps[dependency.toStep];
    if (
      reader.amounts.some(
        (amount) =>
          amount.resolution === 'PercentageOfCurrent' ||
          amount.resolution === 'AllBalance',
      )
    ) {
      results.push({
        kind: 'PreExistingBalanceMixedWithCurrentRunOutput',
        writer: dependency.fromStep,
        reader: dependency.toStep,
        asset: dependency.asset,
      });
    }
  }
  if (model.referenceBudget != null) {
    for (const envelope of envelopes) {
      const refTime = BigInt(envelope.maximumRefTime);
      const proofSize = BigInt(envelope.maximumProofSize);
      if (
        proofSize * model.referenceBudget.refTime >
        refTime * model.referenceBudget.proofSize
      ) {
        results.push({
          kind: 'ProofSizeDominantSuffix',
          cursor: envelope.cursor,
        });
      }
    }
  }
  if (steps.some((step) => step.failureSurface.continuationEligible)) {
    results.push({
      kind: 'AdministrativeInvalidationSurface',
      conditional: true,
      actions: [
        'execution-plan-replacement',
        'schedule-replacement',
        'funding-policy-replacement',
        'deactivation',
        'cancellation',
        'terminal-transition',
        'incompatible-runtime-upgrade',
      ],
    });
  }
  return results;
}

export function analyzeAaaProgram(input: {
  artifact: AaaPlanArtifact;
  metadataBytes: Uint8Array;
  runtime: AaaPlanRuntimeIdentity & { modelIdentity: string };
  weightModel: AaaStaticWeightModel;
  adapterCapabilities?: AaaAdapterCapabilityProfile;
}): ProgramStaticAnalysis {
  validateModel(input.weightModel);
  if (input.runtime.modelIdentity.length === 0) {
    throw new Error('Runtime model identity is required');
  }
  const inspection = inspectAaaPlanArtifact(
    input.artifact,
    input.metadataBytes,
    input.runtime,
  );
  if (!inspection.valid) {
    throw new Error(
      `Invalid canonical plan artifact: ${inspection.errors.join('; ')}`,
    );
  }
  const program = variant(inspection.projection, 'ProgramInput');
  let steps: AaaStaticStepAnalysis[] = [];
  let forecastInputs: AaaStepCostInput[] = [];
  if (program.type === 'Active') {
    ({ steps, forecastInputs } = parseSteps(
      input.artifact,
      member(program.value, 'execution_plan', 'ProgramInput.Active'),
      input.weightModel,
      input.adapterCapabilities,
    ));
  } else if (program.type !== 'Dormant') {
    throw new Error(`Unsupported ProgramInput variant: ${program.type}`);
  }
  const dependencies = forwardDependencies(steps);
  const envelopes = suffixEnvelopes(
    input.artifact,
    steps,
    forecastInputs,
    input.weightModel,
  );
  return {
    provenance: 'StaticStructuralProjection',
    identity: {
      planId: input.artifact.planId,
      genesisHash: input.artifact.genesisHash,
      metadataHash: input.artifact.metadataHash,
      specVersion: input.artifact.specVersion,
      transactionVersion: input.artifact.transactionVersion,
      runtimeModelIdentity: input.runtime.modelIdentity,
      weightModelIdentity: `${input.weightModel.identity}@${input.weightModel.version}`,
      adapterCapabilityIdentity: input.adapterCapabilities?.identity ?? null,
      analyzerVersion: AAA_STATIC_ANALYZER_VERSION,
    },
    program: program.type,
    actorType: input.artifact.aaaType,
    mutability: input.artifact.mutability,
    steps,
    economicSurface: aggregateEconomicSurface(steps),
    dataDependencies: dependencies,
    suffixEnvelopes: envelopes,
    findings: findings(
      steps,
      dependencies,
      envelopes,
      input.weightModel,
      input.adapterCapabilities,
    ),
  };
}
