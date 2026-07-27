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
import {
  type AaaAmountName,
  type AaaConditionName,
  type AaaSemanticTask,
  type AaaTaskName,
  aaaAmountSemantics,
  aaaConditionSemantics,
  aaaTaskSemantics,
} from './semantic-manifest.ts';

export type {
  AaaAmountName,
  AaaConditionName,
  AaaTaskName,
} from './semantic-manifest.ts';

export const AAA_STATIC_ANALYZER_VERSION = '9' as const;

export type AaaRequiredAdapter =
  | 'AssetOps'
  | 'DexOps'
  | 'StakingOps'
  | 'LiquidityOps';

export type AaaStaticObservationWindow =
  | 'artifact-time'
  | 'logical-run-start'
  | 'step-attempt-time'
  | 'retry-time';

export type AaaStaticStepControl =
  | 'advance'
  | 'complete-cycle'
  | 'terminate'
  | 'stutter-current';

export type AaaStaticSuccessfulControl = Extract<
  AaaStaticStepControl,
  'advance' | 'complete-cycle'
>;

export type AaaStaticFailureControl = Exclude<
  AaaStaticStepControl,
  'complete-cycle'
>;

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

export type AaaMinimumBalanceEvidence = {
  provenance: 'FinalizedStateProjection';
  identity: string;
  blockHash: AaaPlanHex;
  entries: Array<{
    asset: AaaPlanProjection;
    minimumBalance: string;
    recipientBalances: Array<{
      recipient: AaaPlanProjection;
      balance: string;
    }>;
  }>;
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

export type AaaStaticRecipient =
  | { kind: 'ActorSovereign' }
  | { kind: 'Explicit'; value: AaaPlanProjection }
  | { kind: 'AdapterDerived' };

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
    observation: 'balance' | 'block-number' | 'scalar-observation';
    readSurface:
      | AaaPlanProjection
      | 'current-block'
      | {
          feed: AaaPlanProjection;
          maxAgeBlocks: number;
          freshness: 'fresh-only';
          nonFreshResult: 'false';
        };
    pure: true;
    observationWindow: 'step-attempt-time';
    boundedReadCount: 1;
  }>;
  task: AaaTaskName;
  parameters: AaaPlanProjection;
  amounts: AaaAmountSemantics[];
  errorPolicy: 'AbortCycle' | 'ContinueNextStep' | 'RetryLater';
  retryMaxAttempts: number | null;
  successfulControl: AaaStaticSuccessfulControl;
  failureControls: AaaStaticFailureControl[];
  availability: 'UserAndSystem' | 'SystemOnly';
  weightOwner: string;
  boundedInternalAlgorithm:
    | 'None'
    | 'PalletSplitFanout'
    | 'RuntimeAdapterContract';
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
    recipients: AaaStaticRecipient[];
    transferExposure: boolean;
    mintExposure: boolean;
    burnExposure: boolean;
    liquidityMutation: boolean;
    stakingMutation: boolean;
    possibleActorSignals: AaaStaticRecipient[];
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

export type AaaStaticTriggerAnalysis = {
  admission: 'Immediate' | 'CadencedAlways' | 'CadencedWhenSignalled';
  everyBlocks: number | null;
  sourceCount: number;
  sourceKinds: Array<'Manual' | 'AddressEvent' | 'ObservationChange'>;
  observationFeeds: AaaPlanProjection[];
};

export type AaaStaticFinding =
  | {
      kind: 'ExternallySignalledAdmission';
      gate: 'Immediate' | 'Cadenced';
      sourceKinds: Array<'Manual' | 'AddressEvent' | 'ObservationChange'>;
    }
  | { kind: 'PeriodicAdmission'; everyBlocks: number }
  | {
      kind: 'TriggerAmountCompatibilityViolation';
      steps: number[];
      sourceKinds: Array<'Manual' | 'AddressEvent' | 'ObservationChange'>;
      reason: 'AddressEventOnlyRequired';
    }
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
      kind: 'SystemReferenceDeviationGuard';
      step: number;
      reference: 'FreshEmaOrDirectReserve';
      localExecutionGuard: true;
      fairPriceProof: false;
      orderingProtection: false;
    }
  | {
      kind: 'SplitTransferDepositPreflight';
      step: number;
      failureClass: 'Temporary';
      atomic: true;
    }
  | {
      kind: 'SplitTransferLegBelowKnownMinimum';
      step: number;
      leg: number;
      asset: AaaPlanProjection;
      recipient: AaaPlanProjection;
      amount: string;
      minimumBalance: string;
      evidenceIdentity: string;
      evidenceBlockHash: AaaPlanHex;
    }
  | {
      kind: 'StopCycleFailureMayFallThrough';
      step: number;
      suffixHasEconomicEffects: boolean;
    }
  | {
      kind: 'PotentialCrossActorFeedbackEdge';
      step: number;
      recipient: AaaStaticRecipient;
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
    minimumBalanceEvidenceIdentity: string | null;
    minimumBalanceEvidenceBlockHash: AaaPlanHex | null;
    analyzerVersion: typeof AAA_STATIC_ANALYZER_VERSION;
  };
  program: 'Dormant' | 'Active';
  actorType: AaaPlanArtifact['aaaType'];
  mutability: AaaPlanArtifact['mutability'];
  completionPolicy: 'Persistent' | 'CloseAfterProductiveRun' | null;
  trigger: AaaStaticTriggerAnalysis | null;
  steps: AaaStaticStepAnalysis[];
  economicSurface: AaaStaticStepAnalysis['economicSurface'];
  dataDependencies: AaaForwardDataDependency[];
  suffixEnvelopes: AaaStaticSuffixEnvelope[];
  findings: AaaStaticFinding[];
};

type ParsedVariant = { type: string; value: AaaPlanProjection };

type TaskSemantics = {
  task: AaaTaskName;
  adapter: AaaRequiredAdapter | null;
  assetsRead: AaaPlanProjection[];
  assetsWritten: AaaPlanProjection[];
  adapterDerivedAssetsRead: boolean;
  adapterDerivedAssetsWritten: boolean;
  recipients: AaaStaticRecipient[];
  effects: Array<'transfer' | 'mint' | 'burn' | 'liquidity' | 'staking'>;
  availability: AaaStaticStepAnalysis['availability'];
  successfulControl: AaaStaticSuccessfulControl;
  weightOwner: string;
  boundedInternalAlgorithm: AaaStaticStepAnalysis['boundedInternalAlgorithm'];
  committedNonCompensatedEffects: boolean;
  amountSurfaces: AaaSemanticTask['amountSurfaces'];
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

function safeInteger(value: AaaPlanProjection, label: string): number {
  const projected = record(value, label);
  const integer = projected.$integer;
  if (typeof integer !== 'string' || !/^[0-9]+$/.test(integer)) {
    throw new Error(`${label} must be an unsigned integer projection`);
  }
  const parsed = Number(integer);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${label} exceeds the safe integer range`);
  }
  return parsed;
}

function fingerprint(value: AaaPlanProjection) {
  return JSON.stringify(value);
}

function unsignedBigInt(value: AaaPlanProjection, label: string): bigint {
  const projected = record(value, label);
  const integer = projected.$integer;
  if (typeof integer !== 'string' || !/^[0-9]+$/.test(integer)) {
    throw new Error(`${label} must be an unsigned integer projection`);
  }
  return BigInt(integer);
}

function evidenceBalance(value: string, label: string): bigint {
  if (!/^[0-9]+$/.test(value)) {
    throw new Error(`${label} must be an unsigned decimal string`);
  }
  return BigInt(value);
}

function validateMinimumBalanceEvidence(evidence: AaaMinimumBalanceEvidence) {
  if (evidence.provenance !== 'FinalizedStateProjection') {
    throw new Error(
      'Minimum-balance evidence must be a finalized state projection',
    );
  }
  if (evidence.identity.trim().length === 0) {
    throw new Error('Minimum-balance evidence identity is required');
  }
  if (!/^0x[0-9a-fA-F]{64}$/.test(evidence.blockHash)) {
    throw new Error('Minimum-balance evidence blockHash must be 32-byte hex');
  }
  const assets = new Set<string>();
  evidence.entries.forEach((entry, entryIndex) => {
    const assetKey = fingerprint(entry.asset);
    if (assets.has(assetKey)) {
      throw new Error('Minimum-balance evidence assets must be unique');
    }
    assets.add(assetKey);
    evidenceBalance(
      entry.minimumBalance,
      `minimumBalanceEvidence.entries[${entryIndex}].minimumBalance`,
    );
    const recipients = new Set<string>();
    entry.recipientBalances.forEach((recipient, recipientIndex) => {
      const recipientKey = fingerprint(recipient.recipient);
      if (recipients.has(recipientKey)) {
        throw new Error(
          'Minimum-balance evidence recipients must be unique per asset',
        );
      }
      recipients.add(recipientKey);
      evidenceBalance(
        recipient.balance,
        `minimumBalanceEvidence.entries[${entryIndex}].recipientBalances[${recipientIndex}].balance`,
      );
    });
  });
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

function uniqueRecipients(values: AaaStaticRecipient[]) {
  const seen = new Set<string>();
  return values.filter((value) => {
    const key = JSON.stringify(value);
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

function errorPolicy(value: AaaPlanProjection) {
  const parsed = variant(value, 'StepErrorPolicy');
  switch (parsed.type) {
    case 'AbortCycle':
    case 'ContinueNextStep':
      return { type: parsed.type, maxAttempts: null } as const;
    case 'RetryLater': {
      const maxAttempts = safeInteger(
        member(parsed.value, 'max_attempts', 'StepErrorPolicy.RetryLater'),
        'StepErrorPolicy.RetryLater.max_attempts',
      );
      if (maxAttempts === 0) {
        throw new Error(
          'StepErrorPolicy.RetryLater.max_attempts must be nonzero',
        );
      }
      return { type: parsed.type, maxAttempts } as const;
    }
    default:
      throw new Error(`Unsupported StepErrorPolicy variant: ${parsed.type}`);
  }
}

function semanticPathValues(
  value: AaaPlanProjection,
  path: string,
  label: string,
): AaaPlanProjection[] {
  if (!path.startsWith('/')) throw new Error(`${label} path must be absolute`);
  let values = [value];
  for (const segment of path.slice(1).split('/')) {
    values = values.flatMap((current) => {
      if (segment === '*') return array(current, label);
      if (Array.isArray(current)) {
        const index = Number(segment);
        if (
          !Number.isSafeInteger(index) ||
          index < 0 ||
          index >= current.length
        ) {
          throw new Error(`${label} has invalid array segment ${segment}`);
        }
        return [current[index]];
      }
      return [member(current, segment, label)];
    });
  }
  return values;
}

function semanticValue(
  value: AaaPlanProjection,
  path: string,
  label: string,
): AaaPlanProjection {
  const values = semanticPathValues(value, path, label);
  if (values.length !== 1) throw new Error(`${label} must resolve once`);
  return values[0];
}

function taskAmounts(
  semantics: TaskSemantics,
  parameters: AaaPlanProjection,
): AaaAmountSemantics[] {
  return semantics.amountSurfaces.map((surface) => {
    const projected = variant(
      semanticValue(
        parameters,
        surface.path,
        `${semantics.task}.${surface.role}`,
      ),
      `${semantics.task}.${surface.role}`,
    );
    const amount = aaaAmountSemantics(projected.type);
    return {
      path: surface.path,
      resolution: amount.resolution,
      dataDependencies: amount.dataDependencies.map((dependency) => {
        switch (dependency) {
          case 'ArtifactValue':
            return 'artifact-value';
          case 'CurrentBalanceOrShares':
            return 'current-balance-or-shares';
          case 'TriggerSnapshot':
            return 'trigger-snapshot';
          case 'LastFundingSnapshot':
            return 'last-funding-snapshot';
          case 'TaskPolicyCapacity':
            return 'task-policy-capacity';
        }
      }),
      minimumBalanceDependency: 'task-policy' as const,
      feeReserveDependency: 'task-policy' as const,
      valueObservation: (() => {
        switch (amount.valueObservationWindow) {
          case 'ArtifactTime':
            return 'artifact-time' as const;
          case 'LogicalRunStart':
            return 'logical-run-start' as const;
          case 'StepAttemptTime':
            return 'step-attempt-time' as const;
        }
      })(),
      retryObservation:
        amount.retryObservation === 'ReobserveLiveValue'
          ? ('reobserve-live' as const)
          : ('reuse-frozen-with-live-capacity' as const),
    };
  });
}

function taskSemantics(
  task: string,
  parameters: AaaPlanProjection,
): TaskSemantics {
  const contract = aaaTaskSemantics(task);
  const effects = contract.effects.map((effect) => {
    switch (effect) {
      case 'Transfer':
        return 'transfer' as const;
      case 'SupplyBurn':
        return 'burn' as const;
      case 'SupplyMint':
        return 'mint' as const;
      case 'LiquidityMutation':
        return 'liquidity' as const;
      case 'StakingMutation':
        return 'staking' as const;
    }
  });
  const recipients = contract.recipients.flatMap(
    (recipient): AaaStaticRecipient[] => {
      switch (recipient.kind) {
        case 'ActorSovereign':
          return [{ kind: 'ActorSovereign' }];
        case 'AdapterDerived':
          return [{ kind: 'AdapterDerived' }];
        case 'Explicit':
          return semanticPathValues(
            parameters,
            recipient.path,
            `${task}.recipient`,
          ).map((value) => ({ kind: 'Explicit', value }));
      }
    },
  );
  return {
    task: contract.task,
    adapter:
      contract.requiredAdapter === 'None' ? null : contract.requiredAdapter,
    assetsRead: uniqueProjection(
      contract.assetsRead.map((path) =>
        semanticValue(parameters, path, `${task}.assetsRead`),
      ),
    ),
    assetsWritten: uniqueProjection(
      contract.assetsWritten.map((path) =>
        semanticValue(parameters, path, `${task}.assetsWritten`),
      ),
    ),
    adapterDerivedAssetsRead: contract.readsAdapterDerivedAssets,
    adapterDerivedAssetsWritten: contract.writesAdapterDerivedAssets,
    recipients,
    effects,
    availability: contract.availability,
    successfulControl:
      contract.successfulControl === 'CompleteCycle'
        ? 'complete-cycle'
        : 'advance',
    weightOwner: contract.weightOwner,
    boundedInternalAlgorithm: contract.boundedInternalAlgorithm,
    committedNonCompensatedEffects: contract.committedNonCompensatedEffects,
    amountSurfaces: contract.amountSurfaces,
  };
}

function conditionAnalysis(condition: AaaPlanProjection) {
  const parsed = variant(condition, 'Condition');
  const semantics = aaaConditionSemantics(parsed.type);
  const label = `Condition.${semantics.condition}`;
  const readSurface = (() => {
    switch (semantics.readSurface.kind) {
      case 'SpendableAssetBalance':
        return semanticValue(parsed.value, semantics.readSurface.path, label);
      case 'CurrentBlockNumber':
        return 'current-block' as const;
      case 'TypedObservation': {
        const maxAgeBlocks = safeInteger(
          semanticValue(
            parsed.value,
            semantics.readSurface.maxAgeBlocksPath,
            label,
          ),
          `${label}.max_age_blocks`,
        );
        if (maxAgeBlocks === 0) {
          throw new Error(`${label}.max_age_blocks must be nonzero`);
        }
        return {
          feed: semanticValue(
            parsed.value,
            semantics.readSurface.feedPath,
            label,
          ),
          maxAgeBlocks,
          freshness: 'fresh-only' as const,
          nonFreshResult: 'false' as const,
        };
      }
    }
  })();
  const observation = (() => {
    switch (semantics.observation) {
      case 'BalanceComparison':
        return 'balance' as const;
      case 'BlockNumberComparison':
        return 'block-number' as const;
      case 'ScalarObservationComparison':
        return 'scalar-observation' as const;
    }
  })();
  return {
    type: semantics.condition,
    value: parsed.value,
    observation,
    readSurface,
    pure: semantics.pure,
    observationWindow: 'step-attempt-time' as const,
    boundedReadCount: semantics.boundedReadCount,
  };
}

function failureControlsFor(
  policy: AaaStaticStepAnalysis['errorPolicy'],
  mutability: AaaPlanArtifact['mutability'],
): AaaStaticFailureControl[] {
  switch (policy) {
    case 'ContinueNextStep':
      return ['advance'];
    case 'AbortCycle':
      return ['advance', 'terminate'];
    case 'RetryLater':
      return mutability === 'Mutable'
        ? ['terminate', 'stutter-current']
        : ['terminate'];
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
      const parameters = parsedTask.value;
      const semantics = taskSemantics(parsedTask.type, parameters);
      const task = semantics.task;
      const parsedPolicy = errorPolicy(
        member(projectedStep, 'on_error', `Step ${index}`),
      );
      const policy = parsedPolicy.type;
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
        amounts: taskAmounts(semantics, parameters),
        errorPolicy: policy,
        retryMaxAttempts: parsedPolicy.maxAttempts,
        successfulControl: semantics.successfulControl,
        failureControls: failureControlsFor(policy, artifact.mutability),
        availability: semantics.availability,
        weightOwner: semantics.weightOwner,
        boundedInternalAlgorithm: semantics.boundedInternalAlgorithm,
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
          possibleActorSignals: semantics.recipients.filter(
            (recipient) => recipient.kind !== 'ActorSovereign',
          ),
          committedNonCompensatedEffects:
            semantics.committedNonCompensatedEffects,
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
            condition.observation === 'balance' &&
            fingerprint(condition.readSurface as AaaPlanProjection) ===
              fingerprint(written),
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
    recipients: uniqueRecipients(
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
    possibleActorSignals: uniqueRecipients(
      steps.flatMap((step) => step.economicSurface.possibleActorSignals),
    ),
    committedNonCompensatedEffects: steps.some(
      (step) => step.economicSurface.committedNonCompensatedEffects,
    ),
  };
}

function parseTrigger(value: AaaPlanProjection): AaaStaticTriggerAnalysis {
  const schedule = member(value, 'schedule', 'ProgramInput.Active');
  const trigger = variant(
    member(schedule, 'trigger', 'Schedule'),
    'Schedule.trigger',
  );
  const parseSources = (projected: AaaPlanProjection, label: string) => {
    const sourceKinds: AaaStaticTriggerAnalysis['sourceKinds'] = [];
    const observationFeeds: AaaPlanProjection[] = [];
    array(projected, label).forEach((source, index) => {
      const parsed = variant(source, `${label}[${index}]`);
      if (parsed.type === 'Manual') {
        sourceKinds.push('Manual');
        return;
      }
      if (parsed.type === 'OnAddressEvent') {
        sourceKinds.push('AddressEvent');
        return;
      }
      if (parsed.type === 'OnObservationChange') {
        sourceKinds.push('ObservationChange');
        observationFeeds.push(
          member(parsed.value, 'feed', `TriggerSource.OnObservationChange`),
        );
        return;
      }
      throw new Error(`Unsupported TriggerSource variant: ${parsed.type}`);
    });
    return { sourceKinds, observationFeeds };
  };
  if (trigger.type === 'Immediate') {
    const sources = parseSources(
      member(trigger.value, 'sources', 'TriggerPolicy.Immediate'),
      'TriggerPolicy.Immediate.sources',
    );
    return {
      admission: 'Immediate',
      everyBlocks: null,
      sourceCount: sources.sourceKinds.length,
      ...sources,
    };
  }
  if (trigger.type !== 'Cadenced') {
    throw new Error(`Unsupported TriggerPolicy variant: ${trigger.type}`);
  }
  const everyBlocks = member(
    trigger.value,
    'every_blocks',
    'TriggerPolicy.Cadenced',
  );
  const everyBlocksNumber = safeInteger(
    everyBlocks,
    'TriggerPolicy.Cadenced.every_blocks',
  );
  if (everyBlocksNumber < 1) {
    throw new Error('TriggerPolicy.Cadenced.every_blocks must be positive');
  }
  const mode = variant(
    member(trigger.value, 'mode', 'TriggerPolicy.Cadenced'),
    'TriggerPolicy.Cadenced.mode',
  );
  if (mode.type === 'Always') {
    return {
      admission: 'CadencedAlways',
      everyBlocks: everyBlocksNumber,
      sourceCount: 0,
      sourceKinds: [],
      observationFeeds: [],
    };
  }
  if (mode.type !== 'WhenSignalled') {
    throw new Error(`Unsupported CadenceMode variant: ${mode.type}`);
  }
  const sources = parseSources(mode.value, 'CadenceMode.WhenSignalled.sources');
  return {
    admission: 'CadencedWhenSignalled',
    everyBlocks: everyBlocksNumber,
    sourceCount: sources.sourceKinds.length,
    ...sources,
  };
}

function findings(
  trigger: AaaStaticTriggerAnalysis | null,
  steps: AaaStaticStepAnalysis[],
  dependencies: AaaForwardDataDependency[],
  envelopes: AaaStaticSuffixEnvelope[],
  model: AaaStaticWeightModel,
  actorType: AaaPlanArtifact['aaaType'],
  capabilities?: AaaAdapterCapabilityProfile,
  minimumBalanceEvidence?: AaaMinimumBalanceEvidence,
): AaaStaticFinding[] {
  const results: AaaStaticFinding[] = [];
  const triggerAmountSteps = steps
    .filter((step) =>
      step.amounts.some(
        (amount) => amount.resolution === 'PercentageOfTrigger',
      ),
    )
    .map((step) => step.index);
  if (
    triggerAmountSteps.length > 0 &&
    (trigger == null ||
      trigger.admission === 'CadencedAlways' ||
      trigger.sourceKinds.length === 0 ||
      trigger.sourceKinds.some((kind) => kind !== 'AddressEvent'))
  ) {
    results.push({
      kind: 'TriggerAmountCompatibilityViolation',
      steps: triggerAmountSteps,
      sourceKinds: trigger?.sourceKinds ?? [],
      reason: 'AddressEventOnlyRequired',
    });
  }
  if (trigger?.admission === 'CadencedAlways') {
    results.push({
      kind: 'PeriodicAdmission',
      everyBlocks: trigger.everyBlocks as number,
    });
  } else if (trigger != null) {
    results.push({
      kind: 'ExternallySignalledAdmission',
      gate: trigger.admission === 'Immediate' ? 'Immediate' : 'Cadenced',
      sourceKinds: trigger.sourceKinds,
    });
  }
  for (const step of steps) {
    if (
      actorType === 'System' &&
      (step.task === 'SwapIn' || step.task === 'SwapOut')
    ) {
      results.push({
        kind: 'SystemReferenceDeviationGuard',
        step: step.index,
        reference: 'FreshEmaOrDirectReserve',
        localExecutionGuard: true,
        fairPriceProof: false,
        orderingProtection: false,
      });
    }
    if (step.task === 'SplitTransfer') {
      results.push({
        kind: 'SplitTransferDepositPreflight',
        step: step.index,
        failureClass: 'Temporary',
        atomic: true,
      });
      const amount = variant(
        member(step.parameters, 'amount', 'SplitTransfer'),
        'SplitTransfer.amount',
      );
      if (amount.type === 'Fixed' && minimumBalanceEvidence != null) {
        const asset = member(step.parameters, 'asset', 'SplitTransfer');
        const evidence = minimumBalanceEvidence.entries.find(
          (entry) => fingerprint(entry.asset) === fingerprint(asset),
        );
        if (evidence != null) {
          const total = unsignedBigInt(
            amount.value,
            'SplitTransfer.amount.Fixed',
          );
          const minimumBalance = evidenceBalance(
            evidence.minimumBalance,
            'SplitTransfer minimum balance',
          );
          array(
            member(step.parameters, 'legs', 'SplitTransfer'),
            'SplitTransfer.legs',
          ).forEach((leg, legIndex) => {
            const recipient = member(
              leg,
              'to',
              `SplitTransfer.legs[${legIndex}]`,
            );
            const recipientEvidence = evidence.recipientBalances.find(
              (candidate) =>
                fingerprint(candidate.recipient) === fingerprint(recipient),
            );
            if (
              recipientEvidence == null ||
              evidenceBalance(
                recipientEvidence.balance,
                `SplitTransfer.legs[${legIndex}] recipient balance`,
              ) !== 0n
            ) {
              return;
            }
            const share = unsignedBigInt(
              member(leg, 'share', `SplitTransfer.legs[${legIndex}]`),
              `SplitTransfer.legs[${legIndex}].share`,
            );
            const legAmount = (total * share) / 1_000_000_000n;
            if (legAmount > 0n && legAmount < minimumBalance) {
              results.push({
                kind: 'SplitTransferLegBelowKnownMinimum',
                step: step.index,
                leg: legIndex,
                asset,
                recipient,
                amount: legAmount.toString(),
                minimumBalance: minimumBalance.toString(),
                evidenceIdentity: minimumBalanceEvidence.identity,
                evidenceBlockHash: minimumBalanceEvidence.blockHash,
              });
            }
          });
        }
      }
    }
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
  minimumBalanceEvidence?: AaaMinimumBalanceEvidence;
}): ProgramStaticAnalysis {
  validateModel(input.weightModel);
  if (input.minimumBalanceEvidence != null) {
    validateMinimumBalanceEvidence(input.minimumBalanceEvidence);
  }
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
  let trigger: AaaStaticTriggerAnalysis | null = null;
  let completionPolicy: 'Persistent' | 'CloseAfterProductiveRun' | null = null;
  let steps: AaaStaticStepAnalysis[] = [];
  let forecastInputs: AaaStepCostInput[] = [];
  if (program.type === 'Active') {
    trigger = parseTrigger(program.value);
    const projectedPolicy = variant(
      member(program.value, 'completion_policy', 'ProgramInput.Active'),
      'ProgramInput.Active.completion_policy',
    );
    if (
      projectedPolicy.type !== 'Persistent' &&
      projectedPolicy.type !== 'CloseAfterProductiveRun'
    ) {
      throw new Error(`Unsupported completion policy: ${projectedPolicy.type}`);
    }
    completionPolicy = projectedPolicy.type;
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
      minimumBalanceEvidenceIdentity:
        input.minimumBalanceEvidence?.identity ?? null,
      minimumBalanceEvidenceBlockHash:
        input.minimumBalanceEvidence?.blockHash ?? null,
      analyzerVersion: AAA_STATIC_ANALYZER_VERSION,
    },
    program: program.type,
    actorType: input.artifact.aaaType,
    mutability: input.artifact.mutability,
    completionPolicy,
    trigger,
    steps,
    economicSurface: aggregateEconomicSurface(steps),
    dataDependencies: dependencies,
    suffixEnvelopes: envelopes,
    findings: findings(
      trigger,
      steps,
      dependencies,
      envelopes,
      input.weightModel,
      input.artifact.aaaType,
      input.adapterCapabilities,
      input.minimumBalanceEvidence,
    ),
  };
}
