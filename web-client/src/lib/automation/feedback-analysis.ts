/*
Domain: Actors deterministic feedback analysis
Owns: Bounded structural graph projection across analyzed actors, typed observations, shared assets, signals, and declared parameter actuators.
Excludes: Program decoding, runtime execution, economic stability, probability, causal strength, scoring, and consensus rules.
Zone: Automation domain capability; consumes manifest-authoritative ProgramStaticAnalysis output.
*/
import type {
  ActorStaticStepAnalysis,
  ProgramStaticAnalysis,
} from './analysis.ts';
import type { ActorPlanProjection } from './plan-artifact.ts';

import { DEOS_OBSERVATION_RUNTIME_EVIDENCE } from '../observation/runtime-evidence.generated.ts';

export const ACTORS_FEEDBACK_ANALYZER_VERSION = '3' as const;

export type ActorObservationProvenance = 'Endogenous' | 'Exogenous' | 'Unknown';

export type ActorFeedbackEffectClass =
  | 'Transfer'
  | 'Swap'
  | 'Liquidity'
  | 'Burn'
  | 'Mint'
  | 'Staking';

export type ActorFeedbackExactResource = {
  kind: 'Pool' | 'Reserve' | 'Tmc';
  identity: string;
  access: 'Read' | 'Write';
  evidence: ActorFeedbackEvidenceReference;
};

export type ActorFeedbackActor = {
  id: string;
  analysis: ProgramStaticAnalysis;
  exactResources?: ActorFeedbackExactResource[];
  sovereignAccount?: {
    value: ActorPlanProjection;
    evidence: ActorFeedbackEvidenceReference;
  };
};

export type ActorObservationEffectMatcher = {
  actorId?: string;
  effectClasses?: ActorFeedbackEffectClass[];
  assetsWritten?: ActorPlanProjection[];
};

export type ActorFeedbackEvidenceReference = {
  provenance: ActorFeedbackEvidenceProvenance;
  identity: string | null;
};

export type ActorFeedbackObservation = {
  id: string;
  feed: ActorPlanProjection;
  producer: 'AxialRouterPreExecutionReserves' | 'DeclaredExternal' | 'Unknown';
  lifecycle: 'Active' | 'Paused' | 'Deactivated' | 'Unknown';
  evidence: ActorFeedbackEvidenceReference;
  effectMatchers: Array<
    ActorObservationEffectMatcher & { evidenceIdentity: string }
  >;
};

export type ActorFeedbackParameterActuator = {
  id: string;
  evidenceIdentity: string;
  controlledByActorId: string;
  affectsObservationIds: string[];
  affectsAssets: ActorPlanProjection[];
};

export type ActorFeedbackNode =
  | { id: string; kind: 'Actor'; actorId: string }
  | {
      id: string;
      kind: 'Observation';
      observationId: string;
      provenance: ActorObservationProvenance;
      lifecycle: ActorFeedbackObservation['lifecycle'];
      evidence: ActorFeedbackEvidenceReference;
    }
  | {
      id: string;
      kind: 'Resource';
      resourceKind:
        | 'AccountAsset'
        | 'AssetClass'
        | 'Pool'
        | 'Reserve'
        | 'Tmc'
        | 'Unknown';
      resourceIdentity: string;
      asset?: ActorPlanProjection;
      account?: ActorPlanProjection;
      actorId?: string;
    }
  | { id: string; kind: 'ParameterActuator'; actuatorId: string };

export type ActorFeedbackEdgeFamily =
  | 'ReactiveCausal'
  | 'ResourceCoupling'
  | 'Coordination'
  | 'DeclaredExternalCausality';

export type ActorFeedbackEvidenceProvenance =
  | 'RuntimeDerived'
  | 'ArtifactDerived'
  | 'Declared'
  | 'Unknown';

export type ActorFeedbackEdge = {
  from: string;
  to: string;
  family: ActorFeedbackEdgeFamily;
  provenance: ActorFeedbackEvidenceProvenance;
  evidenceIdentities: string[];
  kind:
    | 'ObservationTrigger'
    | 'ObservationConditionRead'
    | 'ActorEffectOnObservation'
    | 'ActorSignal'
    | 'SharedAssetWrite'
    | 'SharedAssetRead'
    | 'ParameterControl'
    | 'ParameterEffectOnObservation'
    | 'ParameterEffectOnAsset'
    | 'ExactResourceRead'
    | 'ExactResourceWrite';
  actorId?: string;
  step?: number;
};

export type ActorFeedbackPathEdge = Pick<
  ActorFeedbackEdge,
  'from' | 'to' | 'kind' | 'family' | 'provenance' | 'evidenceIdentities'
>;

export type ActorFeedbackComponent = {
  kind: 'ReactiveSelfCycle' | 'ReactiveCrossActorCycle';
  actorIds: string[];
  observationIds: string[];
  observationProvenance: ActorObservationProvenance[];
  resourceNodeIds: string[];
  actuatorIds: string[];
  canonicalPath: string[];
  canonicalEdges: ActorFeedbackPathEdge[];
  interpretation: 'StructuralPossibility';
  stability: 'Unknown';
  probability: 'Unknown';
  causalStrength: 'Unknown';
};

export type ActorFeedbackRuntimeVerification = {
  observedIdentity: string;
  scheduler: {
    maxServiceUnitsPerBlock: number;
    maxActiveDirtyFeeds: number;
    maxSubscriberPagesPerFeed: number;
  };
} & (
  | { status: 'Verified'; reasons: [] }
  | { status: 'EvidenceMismatch'; reasons: string[] }
);

export type ActorFeedbackEvidenceSnapshot = {
  identity: string;
  runtimeIdentity: string;
  runtimeVerification: ActorFeedbackRuntimeVerification;
  weightIdentity: string;
  cadenceIdentity: string;
  estimatedDeliveryBlocks: number;
  estimatedDeliveryEvidence: ActorFeedbackEvidenceReference;
  observationCadences: Array<{
    observationId: string;
    minimumUpdateIntervalBlocks: number;
    evidence: ActorFeedbackEvidenceReference;
  }>;
  actorPolicies: Array<{
    actorId: string;
    gain: 'High' | 'NotHigh' | 'Unknown';
    gainEvidence: ActorFeedbackEvidenceReference;
    reactiveIngressPriority: 'Explicit' | 'Ordinary' | 'Unknown';
    reactiveIngressPriorityEvidence: ActorFeedbackEvidenceReference;
  }>;
};

export type ActorReactiveFinding =
  | {
      kind: 'FreshnessWindowBelowEstimatedDeliveryEnvelope';
      actorId: string;
      observationId: string;
      step: number;
      maxAgeBlocks: number;
      estimatedDeliveryBlocks: number;
      evidenceIdentity: string;
    }
  | {
      kind: 'EndogenousObservationFeedback';
      actorIds: string[];
      observationIds: string[];
      canonicalPath: string[];
      canonicalEdges: ActorFeedbackPathEdge[];
      interpretation: 'StructuralPossibility';
    }
  | {
      kind: 'ReactiveSelfCycle' | 'ReactiveCrossActorCycle';
      actorIds: string[];
      observationIds: string[];
      canonicalPath: string[];
      canonicalEdges: ActorFeedbackPathEdge[];
      interpretation: 'StructuralPossibility';
    }
  | {
      kind:
        | 'SharedAssetCoupling'
        | 'SharedPoolCoupling'
        | 'SharedReserveCoupling'
        | 'SharedTmcCoupling'
        | 'PotentialResourceContention';
      resourceNodeId: string;
      resourceKind:
        | 'AccountAsset'
        | 'AssetClass'
        | 'Pool'
        | 'Reserve'
        | 'Tmc'
        | 'Unknown';
      actorIds: string[];
      readerActorIds: string[];
      writerActorIds: string[];
      interpretation: 'ResourceCouplingOnly';
      causalStrength: 'Unknown';
    }
  | {
      kind: 'ThresholdChatterRisk' | 'MissingHysteresisOrPersistence';
      actorId: string;
      observationId: string;
      steps: number[];
      interpretation: 'StructuralPossibility';
      evidenceIdentity: string;
      artifactIdentity: string;
    }
  | {
      kind: 'HighGainActuation';
      actorId: string;
      interpretation: 'DeclaredEvidence';
      evidenceIdentity: string;
      gainEvidenceIdentity: string;
    }
  | {
      kind: 'CooldownFeedRateMismatch';
      actorId: string;
      observationId: string;
      cooldownBlocks: number;
      minimumUpdateIntervalBlocks: number;
      evidenceIdentity: string;
    }
  | {
      kind: 'SharedObservationActuatorContention';
      observationId: string;
      actorIds: string[];
      interpretation: 'StructuralPossibility';
    }
  | {
      kind: 'SystemActorWithoutReactiveIngressPriority';
      actorId: string;
      observationIds: string[];
      evidenceIdentity: string;
    };

export type ActorFeedbackModel = {
  provenance: 'DeterministicStaticProjection';
  analyzerVersion: typeof ACTORS_FEEDBACK_ANALYZER_VERSION;
  nodes: ActorFeedbackNode[];
  edges: ActorFeedbackEdge[];
  components: ActorFeedbackComponent[];
  findings: ActorReactiveFinding[];
  evidenceIdentity: string | null;
  evidenceStatus: 'Absent' | 'Verified' | 'EvidenceMismatch';
  evidenceSnapshot: ActorFeedbackEvidenceSnapshot | null;
  limits: {
    maxNodes: number;
    maxEdges: number;
  };
};

export type ActorFeedbackLimits = {
  maxNodes?: number;
  maxEdges?: number;
};

const DEFAULT_MAX_NODES = 256;
const DEFAULT_MAX_EDGES = 2_048;
const EXPECTED_RUNTIME_EVIDENCE = DEOS_OBSERVATION_RUNTIME_EVIDENCE;
const EXPECTED_OBSERVED_RUNTIME_IDENTITY = `${EXPECTED_RUNTIME_EVIDENCE.runtime.specName}@spec-${EXPECTED_RUNTIME_EVIDENCE.runtime.specVersion} · code:${EXPECTED_RUNTIME_EVIDENCE.runtimeCodeHash} · metadata:${EXPECTED_RUNTIME_EVIDENCE.metadataHash}`;

function edgeEvidence(
  kind: ActorFeedbackEdge['kind'],
): Pick<ActorFeedbackEdge, 'family' | 'provenance'> {
  switch (kind) {
    case 'ObservationTrigger':
    case 'ObservationConditionRead':
      return { family: 'ReactiveCausal', provenance: 'ArtifactDerived' };
    case 'ActorEffectOnObservation':
      return { family: 'ReactiveCausal', provenance: 'Declared' };
    case 'ActorSignal':
      return { family: 'Coordination', provenance: 'ArtifactDerived' };
    case 'ParameterControl':
      return { family: 'Coordination', provenance: 'Declared' };
    case 'ParameterEffectOnObservation':
      return {
        family: 'DeclaredExternalCausality',
        provenance: 'Declared',
      };
    case 'SharedAssetWrite':
    case 'SharedAssetRead':
      return { family: 'ResourceCoupling', provenance: 'ArtifactDerived' };
    case 'ParameterEffectOnAsset':
      return { family: 'ResourceCoupling', provenance: 'Declared' };
    case 'ExactResourceRead':
    case 'ExactResourceWrite':
      return { family: 'ResourceCoupling', provenance: 'Unknown' };
  }
}

function observationProvenance(
  observation: ActorFeedbackObservation,
): ActorObservationProvenance {
  switch (observation.producer) {
    case 'AxialRouterPreExecutionReserves':
      return 'Endogenous';
    case 'DeclaredExternal':
      return 'Exogenous';
    case 'Unknown':
      return 'Unknown';
  }
}

function fingerprint(value: ActorPlanProjection) {
  return JSON.stringify(value);
}

function uniqueStrings(values: string[]) {
  return [...new Set(values)].sort();
}

function requireEvidenceReference(
  evidence: ActorFeedbackEvidenceReference,
  label: string,
) {
  if (evidence.provenance === 'Unknown') {
    if (evidence.identity != null) {
      throw new Error(`${label} unknown evidence cannot claim an identity`);
    }
    return;
  }
  if (evidence.identity == null || evidence.identity.trim().length === 0) {
    throw new Error(`${label} evidence identity is required`);
  }
}

function requireEvidenceProvenance(
  evidence: ActorFeedbackEvidenceReference,
  allowed: ActorFeedbackEvidenceProvenance[],
  expectedIdentity: string | null,
  label: string,
) {
  requireEvidenceReference(evidence, label);
  if (!allowed.includes(evidence.provenance)) {
    throw new Error(`${label} uses disallowed evidence provenance`);
  }
  if (
    evidence.provenance !== 'Unknown' &&
    expectedIdentity != null &&
    evidence.identity !== expectedIdentity
  ) {
    throw new Error(`${label} evidence identity mismatch`);
  }
}

function requireEvidenceIdentity(identity: string, label: string) {
  if (identity.trim().length === 0) {
    throw new Error(`${label} evidence identity is required`);
  }
}

function requireUniqueIds(values: Array<{ id: string }>, label: string) {
  const ids = new Set<string>();
  for (const value of values) {
    if (value.id.trim().length === 0)
      throw new Error(`${label} id is required`);
    if (ids.has(value.id)) throw new Error(`${label} ids must be unique`);
    ids.add(value.id);
  }
}

function effectClasses(
  step: ActorStaticStepAnalysis,
): ActorFeedbackEffectClass[] {
  const effects: ActorFeedbackEffectClass[] = [];
  if (step.task === 'Transfer' || step.task === 'SplitTransfer') {
    effects.push('Transfer');
  }
  if (step.task === 'SwapIn' || step.task === 'SwapOut') effects.push('Swap');
  if (step.economicSurface.liquidityMutation) effects.push('Liquidity');
  if (step.economicSurface.burnExposure) effects.push('Burn');
  if (step.economicSurface.mintExposure) effects.push('Mint');
  if (step.economicSurface.stakingMutation) effects.push('Staking');
  return effects;
}

function matchesEffect(
  actor: ActorFeedbackActor,
  step: ActorStaticStepAnalysis,
  matcher: ActorObservationEffectMatcher,
) {
  if (matcher.actorId != null && matcher.actorId !== actor.id) return false;
  if (
    matcher.effectClasses != null &&
    matcher.effectClasses.length > 0 &&
    !matcher.effectClasses.some((effect) =>
      effectClasses(step).includes(effect),
    )
  ) {
    return false;
  }
  if (matcher.assetsWritten != null && matcher.assetsWritten.length > 0) {
    const written = new Set(
      step.economicSurface.assetsWritten.map((asset) => fingerprint(asset)),
    );
    if (
      !matcher.assetsWritten.some((asset) => written.has(fingerprint(asset)))
    ) {
      return false;
    }
  }
  return (
    matcher.actorId != null ||
    (matcher.effectClasses?.length ?? 0) > 0 ||
    (matcher.assetsWritten?.length ?? 0) > 0
  );
}

function canonicalCyclePath(
  component: string[],
  adjacency: Map<string, string[]>,
): string[] {
  const allowed = new Set(component);
  for (const start of [...component].sort()) {
    const visit = (
      node: string,
      path: string[],
      seen: Set<string>,
    ): string[] | null => {
      for (const next of adjacency.get(node) ?? []) {
        if (!allowed.has(next)) continue;
        if (next === start) return [...path, start];
        if (seen.has(next)) continue;
        const found = visit(next, [...path, next], new Set([...seen, next]));
        if (found != null) return found;
      }
      return null;
    };
    const found = visit(start, [start], new Set([start]));
    if (found != null) return found;
  }
  throw new Error('Feedback component must contain a cycle');
}

function canonicalPathEdges(
  path: string[],
  edges: ActorFeedbackEdge[],
): ActorFeedbackPathEdge[] {
  return path.slice(0, -1).map((from, index) => {
    const to = path[index + 1];
    const matches = edges
      .filter((edge) => edge.from === from && edge.to === to)
      .sort((left, right) => left.kind.localeCompare(right.kind));
    if (matches.length === 0) {
      throw new Error('Canonical feedback path is missing edge evidence');
    }
    const { kind, family, provenance, evidenceIdentities } = matches[0];
    return { from, to, kind, family, provenance, evidenceIdentities };
  });
}

function stronglyConnectedComponents(
  nodeIds: string[],
  adjacency: Map<string, string[]>,
) {
  let index = 0;
  const indices = new Map<string, number>();
  const low = new Map<string, number>();
  const stack: string[] = [];
  const onStack = new Set<string>();
  const components: string[][] = [];

  const connect = (node: string) => {
    indices.set(node, index);
    low.set(node, index);
    index += 1;
    stack.push(node);
    onStack.add(node);
    for (const next of adjacency.get(node) ?? []) {
      if (!indices.has(next)) {
        connect(next);
        low.set(node, Math.min(low.get(node)!, low.get(next)!));
      } else if (onStack.has(next)) {
        low.set(node, Math.min(low.get(node)!, indices.get(next)!));
      }
    }
    if (low.get(node) !== indices.get(node)) return;
    const component: string[] = [];
    while (stack.length > 0) {
      const member = stack.pop()!;
      onStack.delete(member);
      component.push(member);
      if (member === node) break;
    }
    components.push(component.sort());
  };

  for (const node of [...nodeIds].sort()) {
    if (!indices.has(node)) connect(node);
  }
  return components;
}

export function analyzeActorFeedback(input: {
  actors: ActorFeedbackActor[];
  observations: ActorFeedbackObservation[];
  parameterActuators?: ActorFeedbackParameterActuator[];
  evidence?: ActorFeedbackEvidenceSnapshot;
  limits?: ActorFeedbackLimits;
}): ActorFeedbackModel {
  const actuators = input.parameterActuators ?? [];
  const maxNodes = input.limits?.maxNodes ?? DEFAULT_MAX_NODES;
  const maxEdges = input.limits?.maxEdges ?? DEFAULT_MAX_EDGES;
  if (!Number.isSafeInteger(maxNodes) || maxNodes < 1) {
    throw new Error('maxNodes must be a positive safe integer');
  }
  if (!Number.isSafeInteger(maxEdges) || maxEdges < 1) {
    throw new Error('maxEdges must be a positive safe integer');
  }
  requireUniqueIds(input.actors, 'Actor');
  requireUniqueIds(input.observations, 'Observation');
  requireUniqueIds(actuators, 'Parameter actuator');

  const actorById = new Map(input.actors.map((actor) => [actor.id, actor]));
  const observationById = new Map(
    input.observations.map((observation) => [observation.id, observation]),
  );
  const observationFeeds = new Set<string>();
  for (const observation of input.observations) {
    const derivedProvenance = observationProvenance(observation);
    requireEvidenceProvenance(
      observation.evidence,
      observation.producer === 'AxialRouterPreExecutionReserves'
        ? ['RuntimeDerived']
        : observation.producer === 'DeclaredExternal'
          ? ['Declared']
          : ['Unknown'],
      null,
      `Observation ${observation.id}`,
    );
    if (
      (observation.lifecycle === 'Unknown') !==
      (derivedProvenance === 'Unknown')
    ) {
      throw new Error(
        `Observation ${observation.id} lifecycle and producer evidence disagree`,
      );
    }
    const feed = fingerprint(observation.feed);
    if (observationFeeds.has(feed)) {
      throw new Error('Observation feed projections must be unique');
    }
    observationFeeds.add(feed);
    if (
      derivedProvenance === 'Exogenous' &&
      observation.effectMatchers.length > 0
    ) {
      throw new Error('Exogenous observations cannot declare actor effects');
    }
    for (const matcher of observation.effectMatchers) {
      requireEvidenceIdentity(
        matcher.evidenceIdentity,
        `Observation ${observation.id} effect matcher`,
      );
      if (matcher.actorId != null && !actorById.has(matcher.actorId)) {
        throw new Error(`Unknown effect-matcher actor: ${matcher.actorId}`);
      }
    }
  }
  const sovereignAccounts = new Set<string>();
  const sovereignEvidenceIdentities = new Set<string>();
  for (const actor of input.actors) {
    if (actor.sovereignAccount != null) {
      requireEvidenceProvenance(
        actor.sovereignAccount.evidence,
        ['RuntimeDerived'],
        null,
        `Actor ${actor.id} sovereign account`,
      );
      sovereignEvidenceIdentities.add(
        actor.sovereignAccount.evidence.identity!,
      );
      const account = fingerprint(actor.sovereignAccount.value);
      if (sovereignAccounts.has(account)) {
        throw new Error('Actor sovereign accounts must be unique');
      }
      sovereignAccounts.add(account);
    }
    const exactResourceKeys = new Set<string>();
    for (const resource of actor.exactResources ?? []) {
      requireEvidenceIdentity(
        resource.identity,
        `Actor ${actor.id} exact resource`,
      );
      requireEvidenceProvenance(
        resource.evidence,
        ['RuntimeDerived', 'ArtifactDerived'],
        resource.evidence.provenance === 'ArtifactDerived'
          ? actor.analysis.identity.planId
          : null,
        `Actor ${actor.id} ${resource.kind} resource`,
      );
      if (resource.evidence.provenance === 'RuntimeDerived') {
        sovereignEvidenceIdentities.add(resource.evidence.identity!);
      }
      const key = `${resource.kind}|${resource.identity}|${resource.access}`;
      if (exactResourceKeys.has(key)) {
        throw new Error(`Actor ${actor.id} exact resources must be unique`);
      }
      exactResourceKeys.add(key);
    }
  }
  if (sovereignEvidenceIdentities.size > 1) {
    throw new Error(
      'Runtime-derived actor resources must share one state identity',
    );
  }
  for (const actor of input.actors) {
    if (actor.analysis.provenance !== 'StaticStructuralProjection') {
      throw new Error('Actors must use manifest-authoritative static analysis');
    }
    requireEvidenceIdentity(
      actor.analysis.identity.planId,
      `Actor ${actor.id} artifact`,
    );
  }
  const runtimeContexts = new Set(
    input.actors.map((actor) =>
      JSON.stringify({
        genesisHash: actor.analysis.identity.genesisHash,
        metadataHash: actor.analysis.identity.metadataHash,
        specVersion: actor.analysis.identity.specVersion,
        transactionVersion: actor.analysis.identity.transactionVersion,
        runtimeModelIdentity: actor.analysis.identity.runtimeModelIdentity,
        weightModelIdentity: actor.analysis.identity.weightModelIdentity,
        analyzerVersion: actor.analysis.identity.analyzerVersion,
      }),
    ),
  );
  if (runtimeContexts.size > 1) {
    throw new Error('Actor analyses must share one runtime evidence context');
  }
  for (const actuator of actuators) {
    if (!actorById.has(actuator.controlledByActorId)) {
      throw new Error(
        `Unknown actuator controller: ${actuator.controlledByActorId}`,
      );
    }
    requireEvidenceIdentity(
      actuator.evidenceIdentity,
      `Parameter actuator ${actuator.id}`,
    );
    for (const observationId of actuator.affectsObservationIds) {
      if (!observationById.has(observationId)) {
        throw new Error(`Unknown actuator observation: ${observationId}`);
      }
    }
  }
  const evidence = input.evidence;
  if (evidence != null) {
    for (const [label, identity] of [
      ['Evidence', evidence.identity],
      ['Runtime', evidence.runtimeIdentity],
      ['Weight', evidence.weightIdentity],
      ['Cadence', evidence.cadenceIdentity],
    ]) {
      if (identity.trim().length === 0)
        throw new Error(`${label} identity is required`);
    }
    if (
      evidence.runtimeIdentity !== evidence.runtimeVerification.observedIdentity
    ) {
      throw new Error('Runtime verification identity mismatch');
    }
    if (evidence.runtimeVerification.status === 'Verified') {
      if (evidence.runtimeVerification.reasons.length !== 0) {
        throw new Error(
          'Verified runtime evidence cannot carry mismatch reasons',
        );
      }
      if (
        evidence.runtimeIdentity !== EXPECTED_OBSERVED_RUNTIME_IDENTITY ||
        evidence.weightIdentity !== EXPECTED_RUNTIME_EVIDENCE.weightIdentity ||
        evidence.runtimeVerification.scheduler.maxServiceUnitsPerBlock !==
          EXPECTED_RUNTIME_EVIDENCE.fanout.maxServiceUnitsPerBlock ||
        evidence.runtimeVerification.scheduler.maxActiveDirtyFeeds !==
          EXPECTED_RUNTIME_EVIDENCE.fanout.maxActiveDirtyFeeds ||
        evidence.runtimeVerification.scheduler.maxSubscriberPagesPerFeed !==
          EXPECTED_RUNTIME_EVIDENCE.fanout.maxSubscriberPagesPerFeed
      ) {
        throw new Error(
          'Verified runtime evidence differs from generated truth',
        );
      }
    } else if (evidence.runtimeVerification.reasons.length === 0) {
      throw new Error('Runtime evidence mismatch requires reasons');
    }
    requireEvidenceProvenance(
      evidence.estimatedDeliveryEvidence,
      ['RuntimeDerived'],
      evidence.identity,
      'Estimated delivery',
    );
    if (
      !Number.isSafeInteger(evidence.estimatedDeliveryBlocks) ||
      evidence.estimatedDeliveryBlocks < 0
    ) {
      throw new Error(
        'estimatedDeliveryBlocks must be a non-negative safe integer',
      );
    }
    requireUniqueIds(
      evidence.observationCadences.map(({ observationId }) => ({
        id: observationId,
      })),
      'Observation cadence',
    );
    requireUniqueIds(
      evidence.actorPolicies.map(({ actorId }) => ({ id: actorId })),
      'Actor policy',
    );
    for (const cadence of evidence.observationCadences) {
      if (!observationById.has(cadence.observationId)) {
        throw new Error(
          `Unknown cadence observation: ${cadence.observationId}`,
        );
      }
      requireEvidenceProvenance(
        cadence.evidence,
        ['RuntimeDerived'],
        evidence.cadenceIdentity,
        `Observation ${cadence.observationId} cadence`,
      );
      if (
        !Number.isSafeInteger(cadence.minimumUpdateIntervalBlocks) ||
        cadence.minimumUpdateIntervalBlocks < 1
      ) {
        throw new Error(
          'minimumUpdateIntervalBlocks must be a positive safe integer',
        );
      }
    }
    for (const policy of evidence.actorPolicies) {
      if (!actorById.has(policy.actorId)) {
        throw new Error(`Unknown evidence actor: ${policy.actorId}`);
      }
      requireEvidenceProvenance(
        policy.gainEvidence,
        ['Declared', 'Unknown'],
        null,
        `Actor ${policy.actorId} gain`,
      );
      requireEvidenceProvenance(
        policy.reactiveIngressPriorityEvidence,
        ['RuntimeDerived', 'Unknown'],
        evidence.runtimeIdentity,
        `Actor ${policy.actorId} reactive ingress priority`,
      );
      if (
        (policy.gain === 'Unknown') !==
        (policy.gainEvidence.provenance === 'Unknown')
      ) {
        throw new Error(`Actor ${policy.actorId} gain evidence disagrees`);
      }
      if (
        (policy.reactiveIngressPriority === 'Unknown') !==
        (policy.reactiveIngressPriorityEvidence.provenance === 'Unknown')
      ) {
        throw new Error(
          `Actor ${policy.actorId} reactive ingress evidence disagrees`,
        );
      }
    }
  }

  type ResourceDescriptor =
    Extract<ActorFeedbackNode, { kind: 'Resource' }> extends infer Resource
      ? Omit<Resource, 'id' | 'kind'>
      : never;
  const resourceByKey = new Map<string, ResourceDescriptor>();
  const actorResourceKey = (
    actor: ActorFeedbackActor,
    asset: ActorPlanProjection,
  ) => {
    const assetIdentity = fingerprint(asset);
    if (actor.sovereignAccount != null) {
      return `account:${fingerprint(actor.sovereignAccount.value)}|asset:${assetIdentity}`;
    }
    return `unknown-actor:${actor.id}|asset:${assetIdentity}`;
  };
  for (const actor of input.actors) {
    for (const asset of [
      ...actor.analysis.economicSurface.assetsRead,
      ...actor.analysis.economicSurface.assetsWritten,
    ]) {
      const key = actorResourceKey(actor, asset);
      resourceByKey.set(key, {
        resourceKind:
          actor.sovereignAccount == null ? 'Unknown' : 'AccountAsset',
        resourceIdentity: key,
        asset,
        account: actor.sovereignAccount?.value,
        actorId: actor.id,
      });
    }
    for (const resource of actor.exactResources ?? []) {
      const key = `exact:${resource.kind}:${resource.identity}`;
      resourceByKey.set(key, {
        resourceKind: resource.kind,
        resourceIdentity: resource.identity,
      });
    }
  }
  for (const asset of actuators.flatMap((actuator) => actuator.affectsAssets)) {
    resourceByKey.set(`asset-class:${fingerprint(asset)}`, {
      resourceKind: 'AssetClass',
      resourceIdentity: fingerprint(asset),
      asset,
    });
  }
  const resourceEntries = [...resourceByKey.entries()].sort(([left], [right]) =>
    left.localeCompare(right),
  );
  const resourceNodeByKey = new Map(
    resourceEntries.map(([key], index) => [key, `resource:${index}`]),
  );

  const actorNode = (id: string) => `actor:${id}`;
  const observationNode = (id: string) => `observation:${id}`;
  const actuatorNode = (id: string) => `actuator:${id}`;
  const nodes: ActorFeedbackNode[] = [
    ...input.actors
      .map((actor) => ({
        id: actorNode(actor.id),
        kind: 'Actor' as const,
        actorId: actor.id,
      }))
      .sort((left, right) => left.id.localeCompare(right.id)),
    ...input.observations
      .map((observation) => ({
        id: observationNode(observation.id),
        kind: 'Observation' as const,
        observationId: observation.id,
        provenance: observationProvenance(observation),
        lifecycle: observation.lifecycle,
        evidence: observation.evidence,
      }))
      .sort((left, right) => left.id.localeCompare(right.id)),
    ...resourceEntries.map(([, resource], index) => ({
      id: `resource:${index}`,
      kind: 'Resource' as const,
      ...resource,
    })),
    ...actuators
      .map((actuator) => ({
        id: actuatorNode(actuator.id),
        kind: 'ParameterActuator' as const,
        actuatorId: actuator.id,
      }))
      .sort((left, right) => left.id.localeCompare(right.id)),
  ];
  if (nodes.length > maxNodes) throw new Error('Feedback node limit exceeded');

  const edges: ActorFeedbackEdge[] = [];
  const addEdge = (
    edge: Omit<
      ActorFeedbackEdge,
      'family' | 'provenance' | 'evidenceIdentities'
    >,
    suppliedEvidenceIdentities: Array<string | null> = [],
    provenanceOverride?: ActorFeedbackEvidenceProvenance,
  ) => {
    const artifactKinds = new Set<ActorFeedbackEdge['kind']>([
      'ObservationTrigger',
      'ObservationConditionRead',
      'ActorEffectOnObservation',
      'ActorSignal',
      'SharedAssetWrite',
      'SharedAssetRead',
    ]);
    const artifactIdentity =
      edge.actorId != null && artifactKinds.has(edge.kind)
        ? actorById.get(edge.actorId)?.analysis.identity.planId
        : null;
    const evidenceIdentities = uniqueStrings(
      [artifactIdentity, ...suppliedEvidenceIdentities].filter(
        (identity): identity is string => identity != null,
      ),
    );
    if (evidenceIdentities.length === 0) {
      throw new Error(`${edge.kind} edge requires supplying evidence identity`);
    }
    const classification = edgeEvidence(edge.kind);
    edges.push({
      ...edge,
      ...classification,
      provenance: provenanceOverride ?? classification.provenance,
      evidenceIdentities,
    });
  };
  const feedToObservation = new Map(
    input.observations.map((observation) => [
      fingerprint(observation.feed),
      observation.id,
    ]),
  );
  const observationEvidenceIdentity = new Map(
    input.observations.map((observation) => [
      observation.id,
      observation.evidence.identity,
    ]),
  );
  const sovereignToActor = new Map(
    input.actors
      .filter((actor) => actor.sovereignAccount != null)
      .map((actor) => [fingerprint(actor.sovereignAccount!.value), actor.id]),
  );
  const sovereignEvidenceByActor = new Map(
    input.actors
      .filter((actor) => actor.sovereignAccount != null)
      .map((actor) => [actor.id, actor.sovereignAccount!.evidence.identity]),
  );

  for (const actor of input.actors) {
    for (const feed of actor.analysis.trigger?.observationFeeds ?? []) {
      const observationId = feedToObservation.get(fingerprint(feed));
      if (observationId != null) {
        addEdge(
          {
            from: observationNode(observationId),
            to: actorNode(actor.id),
            kind: 'ObservationTrigger',
            actorId: actor.id,
          },
          [observationEvidenceIdentity.get(observationId) ?? null],
        );
      }
    }
    for (const step of actor.analysis.steps) {
      for (const condition of step.conditions) {
        if (condition.observation !== 'scalar-observation') continue;
        const surface = condition.readSurface as { feed: ActorPlanProjection };
        const observationId = feedToObservation.get(fingerprint(surface.feed));
        if (observationId != null) {
          addEdge(
            {
              from: observationNode(observationId),
              to: actorNode(actor.id),
              kind: 'ObservationConditionRead',
              actorId: actor.id,
              step: step.index,
            },
            [observationEvidenceIdentity.get(observationId) ?? null],
          );
        }
      }
      for (const observation of input.observations) {
        if (observation.lifecycle === 'Deactivated') continue;
        const matchingEffects = observation.effectMatchers.filter((matcher) =>
          matchesEffect(actor, step, matcher),
        );
        if (matchingEffects.length > 0) {
          addEdge(
            {
              from: actorNode(actor.id),
              to: observationNode(observation.id),
              kind: 'ActorEffectOnObservation',
              actorId: actor.id,
              step: step.index,
            },
            [
              observation.evidence.identity,
              ...matchingEffects.map((matcher) => matcher.evidenceIdentity),
            ],
          );
        }
      }
      for (const recipient of step.economicSurface.possibleActorSignals) {
        if (recipient.kind !== 'Explicit') continue;
        const recipientId = sovereignToActor.get(fingerprint(recipient.value));
        if (recipientId != null) {
          addEdge(
            {
              from: actorNode(actor.id),
              to: actorNode(recipientId),
              kind: 'ActorSignal',
              actorId: actor.id,
              step: step.index,
            },
            [sovereignEvidenceByActor.get(recipientId) ?? null],
          );
        }
      }
    }
    for (const asset of actor.analysis.economicSurface.assetsWritten) {
      addEdge({
        from: actorNode(actor.id),
        to: resourceNodeByKey.get(actorResourceKey(actor, asset))!,
        kind: 'SharedAssetWrite',
        actorId: actor.id,
      });
    }
    for (const asset of actor.analysis.economicSurface.assetsRead) {
      addEdge({
        from: resourceNodeByKey.get(actorResourceKey(actor, asset))!,
        to: actorNode(actor.id),
        kind: 'SharedAssetRead',
        actorId: actor.id,
      });
    }
    for (const resource of actor.exactResources ?? []) {
      const resourceNode = resourceNodeByKey.get(
        `exact:${resource.kind}:${resource.identity}`,
      )!;
      addEdge(
        {
          from: resource.access === 'Read' ? resourceNode : actorNode(actor.id),
          to: resource.access === 'Read' ? actorNode(actor.id) : resourceNode,
          kind:
            resource.access === 'Read'
              ? 'ExactResourceRead'
              : 'ExactResourceWrite',
          actorId: actor.id,
        },
        [resource.evidence.identity],
        resource.evidence.provenance,
      );
    }
  }

  for (const actuator of actuators) {
    addEdge(
      {
        from: actorNode(actuator.controlledByActorId),
        to: actuatorNode(actuator.id),
        kind: 'ParameterControl',
        actorId: actuator.controlledByActorId,
      },
      [actuator.evidenceIdentity],
    );
    for (const observationId of actuator.affectsObservationIds) {
      if (observationById.get(observationId)?.lifecycle === 'Deactivated') {
        continue;
      }
      addEdge(
        {
          from: actuatorNode(actuator.id),
          to: observationNode(observationId),
          kind: 'ParameterEffectOnObservation',
        },
        [
          actuator.evidenceIdentity,
          observationEvidenceIdentity.get(observationId) ?? null,
        ],
      );
    }
    for (const asset of actuator.affectsAssets) {
      addEdge(
        {
          from: actuatorNode(actuator.id),
          to: resourceNodeByKey.get(`asset-class:${fingerprint(asset)}`)!,
          kind: 'ParameterEffectOnAsset',
        },
        [actuator.evidenceIdentity],
      );
    }
  }

  const deduplicatedEdges = [
    ...new Map(
      edges.map((edge) => [
        `${edge.from}|${edge.to}|${edge.kind}|${edge.actorId ?? ''}|${edge.step ?? ''}`,
        edge,
      ]),
    ).values(),
  ].sort((left, right) =>
    `${left.from}|${left.to}|${left.kind}|${left.step ?? ''}`.localeCompare(
      `${right.from}|${right.to}|${right.kind}|${right.step ?? ''}`,
    ),
  );
  if (deduplicatedEdges.length > maxEdges) {
    throw new Error('Feedback edge limit exceeded');
  }

  const cycleEdges = deduplicatedEdges.filter(
    (edge) => edge.family !== 'ResourceCoupling',
  );
  const adjacency = new Map(nodes.map((node) => [node.id, [] as string[]]));
  for (const edge of cycleEdges) adjacency.get(edge.from)!.push(edge.to);
  for (const targets of adjacency.values()) targets.sort();
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const components = stronglyConnectedComponents(
    nodes.map((node) => node.id),
    adjacency,
  )
    .filter(
      (component) =>
        component.length > 1 ||
        (adjacency.get(component[0]) ?? []).includes(component[0]),
    )
    .filter((component) => {
      const members = new Set(component);
      const internalEdges = cycleEdges.filter(
        (edge) => members.has(edge.from) && members.has(edge.to),
      );
      return (
        component.some((id) => nodeById.get(id)?.kind === 'Observation') &&
        internalEdges.some((edge) => edge.family === 'ReactiveCausal')
      );
    })
    .map((component): ActorFeedbackComponent => {
      const members = component.map((id) => nodeById.get(id)!);
      const actorIds = uniqueStrings(
        members
          .filter(
            (node): node is Extract<ActorFeedbackNode, { kind: 'Actor' }> =>
              node.kind === 'Actor',
          )
          .map((node) => node.actorId),
      );
      const observations = members.filter(
        (node): node is Extract<ActorFeedbackNode, { kind: 'Observation' }> =>
          node.kind === 'Observation',
      );
      const canonicalPath = canonicalCyclePath(component, adjacency);
      return {
        kind:
          actorIds.length <= 1
            ? 'ReactiveSelfCycle'
            : 'ReactiveCrossActorCycle',
        actorIds,
        observationIds: uniqueStrings(
          observations.map((node) => node.observationId),
        ),
        observationProvenance: uniqueStrings(
          observations.map((node) => node.provenance),
        ) as ActorObservationProvenance[],
        resourceNodeIds: uniqueStrings(
          members
            .filter((node) => node.kind === 'Resource')
            .map((node) => node.id),
        ),
        actuatorIds: uniqueStrings(
          members
            .filter(
              (
                node,
              ): node is Extract<
                ActorFeedbackNode,
                { kind: 'ParameterActuator' }
              > => node.kind === 'ParameterActuator',
            )
            .map((node) => node.actuatorId),
        ),
        canonicalPath,
        canonicalEdges: canonicalPathEdges(canonicalPath, cycleEdges),
        interpretation: 'StructuralPossibility',
        stability: 'Unknown',
        probability: 'Unknown',
        causalStrength: 'Unknown',
      };
    })
    .sort((left, right) =>
      left.canonicalPath.join('|').localeCompare(right.canonicalPath.join('|')),
    );

  const findings: ActorReactiveFinding[] = [];
  for (const component of components) {
    findings.push({
      kind: component.kind,
      actorIds: component.actorIds,
      observationIds: component.observationIds,
      canonicalPath: component.canonicalPath,
      canonicalEdges: component.canonicalEdges,
      interpretation: 'StructuralPossibility',
    });
    if (component.observationProvenance.includes('Endogenous')) {
      findings.push({
        kind: 'EndogenousObservationFeedback',
        actorIds: component.actorIds,
        observationIds: component.observationIds.filter(
          (id) =>
            observationProvenance(observationById.get(id)!) === 'Endogenous',
        ),
        canonicalPath: component.canonicalPath,
        canonicalEdges: component.canonicalEdges,
        interpretation: 'StructuralPossibility',
      });
    }
  }

  for (const node of nodes.filter(
    (
      candidate,
    ): candidate is Extract<ActorFeedbackNode, { kind: 'Resource' }> =>
      candidate.kind === 'Resource',
  )) {
    const writerActorIds = uniqueStrings(
      deduplicatedEdges
        .filter(
          (edge) =>
            edge.family === 'ResourceCoupling' &&
            edge.to === node.id &&
            (edge.kind === 'SharedAssetWrite' ||
              edge.kind === 'ExactResourceWrite') &&
            edge.actorId != null,
        )
        .map((edge) => edge.actorId!),
    );
    const readerActorIds = uniqueStrings(
      deduplicatedEdges
        .filter(
          (edge) =>
            edge.family === 'ResourceCoupling' &&
            edge.from === node.id &&
            (edge.kind === 'SharedAssetRead' ||
              edge.kind === 'ExactResourceRead') &&
            edge.actorId != null,
        )
        .map((edge) => edge.actorId!),
    );
    const actorIds = uniqueStrings([...writerActorIds, ...readerActorIds]);
    if (actorIds.length >= 2) {
      const couplingKind =
        node.resourceKind === 'Pool'
          ? 'SharedPoolCoupling'
          : node.resourceKind === 'Reserve'
            ? 'SharedReserveCoupling'
            : node.resourceKind === 'Tmc'
              ? 'SharedTmcCoupling'
              : 'SharedAssetCoupling';
      findings.push({
        kind: couplingKind,
        resourceNodeId: node.id,
        resourceKind: node.resourceKind,
        actorIds,
        readerActorIds,
        writerActorIds,
        interpretation: 'ResourceCouplingOnly',
        causalStrength: 'Unknown',
      });
    }
    if (writerActorIds.length >= 2) {
      findings.push({
        kind: 'PotentialResourceContention',
        resourceNodeId: node.id,
        resourceKind: node.resourceKind,
        actorIds,
        readerActorIds,
        writerActorIds,
        interpretation: 'ResourceCouplingOnly',
        causalStrength: 'Unknown',
      });
    }
  }

  const effectsByObservation = new Map<string, Set<string>>();
  for (const edge of deduplicatedEdges) {
    if (edge.kind !== 'ActorEffectOnObservation' || edge.actorId == null)
      continue;
    const observationId = edge.to.slice('observation:'.length);
    const actors = effectsByObservation.get(observationId) ?? new Set<string>();
    actors.add(edge.actorId);
    effectsByObservation.set(observationId, actors);
  }
  for (const [observationId, actors] of effectsByObservation) {
    if (actors.size < 2) continue;
    findings.push({
      kind: 'SharedObservationActuatorContention',
      observationId,
      actorIds: [...actors].sort(),
      interpretation: 'StructuralPossibility',
    });
  }

  if (evidence?.runtimeVerification.status === 'Verified') {
    const cadenceByObservation = new Map(
      evidence.observationCadences.map((cadence) => [
        cadence.observationId,
        cadence,
      ]),
    );
    const policyByActor = new Map(
      evidence.actorPolicies.map((policy) => [policy.actorId, policy]),
    );
    for (const actor of input.actors) {
      const policy = policyByActor.get(actor.id);
      const triggeredObservationIds = uniqueStrings(
        (actor.analysis.trigger?.observationFeeds ?? [])
          .map((feed) => feedToObservation.get(fingerprint(feed)))
          .filter((id): id is string => id != null),
      );
      const thresholdSteps = new Map<string, number[]>();
      for (const step of actor.analysis.steps) {
        for (const condition of step.conditions) {
          if (condition.observation !== 'scalar-observation') continue;
          const surface = condition.readSurface as {
            feed: ActorPlanProjection;
            maxAgeBlocks: number;
          };
          const observationId = feedToObservation.get(
            fingerprint(surface.feed),
          );
          if (observationId == null) continue;
          if (surface.maxAgeBlocks < evidence.estimatedDeliveryBlocks) {
            findings.push({
              kind: 'FreshnessWindowBelowEstimatedDeliveryEnvelope',
              actorId: actor.id,
              observationId,
              step: step.index,
              maxAgeBlocks: surface.maxAgeBlocks,
              estimatedDeliveryBlocks: evidence.estimatedDeliveryBlocks,
              evidenceIdentity: evidence.identity,
            });
          }
          if (
            condition.type === 'ObservationAbove' ||
            condition.type === 'ObservationBelow'
          ) {
            const steps = thresholdSteps.get(observationId) ?? [];
            steps.push(step.index);
            thresholdSteps.set(observationId, steps);
          }
        }
      }
      for (const [observationId, steps] of thresholdSteps) {
        const participatesInFeedback = components.some(
          (component) =>
            component.actorIds.includes(actor.id) &&
            component.observationIds.includes(observationId),
        );
        if (participatesInFeedback) {
          const base = {
            actorId: actor.id,
            observationId,
            steps: [...new Set(steps)].sort((left, right) => left - right),
            interpretation: 'StructuralPossibility' as const,
            evidenceIdentity: evidence.identity,
            artifactIdentity: actor.analysis.identity.planId,
          };
          findings.push({ kind: 'MissingHysteresisOrPersistence', ...base });
          findings.push({ kind: 'ThresholdChatterRisk', ...base });
        }
      }
      if (policy?.gain === 'High') {
        findings.push({
          kind: 'HighGainActuation',
          actorId: actor.id,
          interpretation: 'DeclaredEvidence',
          evidenceIdentity: evidence.identity,
          gainEvidenceIdentity: policy.gainEvidence.identity!,
        });
      }
      if (actor.analysis.cooldownBlocks != null) {
        for (const observationId of triggeredObservationIds) {
          const cadence = cadenceByObservation.get(observationId);
          if (
            cadence != null &&
            actor.analysis.cooldownBlocks > cadence.minimumUpdateIntervalBlocks
          ) {
            findings.push({
              kind: 'CooldownFeedRateMismatch',
              actorId: actor.id,
              observationId,
              cooldownBlocks: actor.analysis.cooldownBlocks,
              minimumUpdateIntervalBlocks: cadence.minimumUpdateIntervalBlocks,
              evidenceIdentity: evidence.identity,
            });
          }
        }
      }
      if (
        actor.analysis.actorType === 'System' &&
        triggeredObservationIds.length > 0 &&
        policy?.reactiveIngressPriority === 'Ordinary'
      ) {
        findings.push({
          kind: 'SystemActorWithoutReactiveIngressPriority',
          actorId: actor.id,
          observationIds: triggeredObservationIds,
          evidenceIdentity: evidence.identity,
        });
      }
    }
  }
  findings.sort((left, right) =>
    `${left.kind}|${JSON.stringify(left)}`.localeCompare(
      `${right.kind}|${JSON.stringify(right)}`,
    ),
  );

  return {
    provenance: 'DeterministicStaticProjection',
    analyzerVersion: ACTORS_FEEDBACK_ANALYZER_VERSION,
    nodes,
    edges: deduplicatedEdges,
    components,
    findings,
    evidenceIdentity: evidence?.identity ?? null,
    evidenceStatus: evidence?.runtimeVerification.status ?? 'Absent',
    evidenceSnapshot: evidence ?? null,
    limits: { maxNodes, maxEdges },
  };
}
