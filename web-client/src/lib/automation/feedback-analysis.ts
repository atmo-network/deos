/*
Domain: AAA deterministic feedback analysis
Owns: Bounded structural graph projection across analyzed actors, typed observations, shared assets, signals, and declared parameter actuators.
Excludes: Program decoding, runtime execution, economic stability, probability, causal strength, scoring, and consensus rules.
Zone: Automation domain capability; consumes manifest-authoritative ProgramStaticAnalysis output.
*/
import type {
  AaaStaticStepAnalysis,
  ProgramStaticAnalysis,
} from './analysis.ts';
import type { AaaPlanProjection } from './plan-artifact.ts';

export const AAA_FEEDBACK_ANALYZER_VERSION = '2' as const;

export type AaaObservationProvenance = 'Endogenous' | 'Exogenous' | 'Unknown';

export type AaaFeedbackEffectClass =
  | 'Transfer'
  | 'Swap'
  | 'Liquidity'
  | 'Burn'
  | 'Mint'
  | 'Staking';

export type AaaFeedbackActor = {
  id: string;
  analysis: ProgramStaticAnalysis;
  sovereignAccount?: AaaPlanProjection;
};

export type AaaObservationEffectMatcher = {
  actorId?: string;
  effectClasses?: AaaFeedbackEffectClass[];
  assetsWritten?: AaaPlanProjection[];
};

export type AaaFeedbackObservation = {
  id: string;
  feed: AaaPlanProjection;
  provenance: AaaObservationProvenance;
  effectMatchers: AaaObservationEffectMatcher[];
};

export type AaaFeedbackParameterActuator = {
  id: string;
  controlledByActorId: string;
  affectsObservationIds: string[];
  affectsAssets: AaaPlanProjection[];
};

export type AaaFeedbackNode =
  | { id: string; kind: 'Actor'; actorId: string }
  | {
      id: string;
      kind: 'Observation';
      observationId: string;
      provenance: AaaObservationProvenance;
    }
  | { id: string; kind: 'Asset'; asset: AaaPlanProjection }
  | { id: string; kind: 'ParameterActuator'; actuatorId: string };

export type AaaFeedbackEdge = {
  from: string;
  to: string;
  kind:
    | 'ObservationTrigger'
    | 'ObservationConditionRead'
    | 'ActorEffectOnObservation'
    | 'ActorSignal'
    | 'SharedAssetWrite'
    | 'SharedAssetRead'
    | 'ParameterControl'
    | 'ParameterEffectOnObservation'
    | 'ParameterEffectOnAsset';
  actorId?: string;
  step?: number;
};

export type AaaFeedbackComponent = {
  kind: 'ReactiveSelfCycle' | 'ReactiveCrossActorCycle';
  actorIds: string[];
  observationIds: string[];
  observationProvenance: AaaObservationProvenance[];
  assetNodeIds: string[];
  actuatorIds: string[];
  canonicalPath: string[];
  interpretation: 'StructuralPossibility';
  stability: 'Unknown';
  probability: 'Unknown';
  causalStrength: 'Unknown';
};

export type AaaFeedbackEvidenceSnapshot = {
  identity: string;
  runtimeIdentity: string;
  weightIdentity: string;
  cadenceIdentity: string;
  estimatedDeliveryBlocks: number;
  observationCadences: Array<{
    observationId: string;
    minimumUpdateIntervalBlocks: number;
  }>;
  actorPolicies: Array<{
    actorId: string;
    cooldownBlocks: number | null;
    hysteresis: 'Present' | 'Absent' | 'Unknown';
    persistenceBlocks: number | null;
    gain: 'High' | 'NotHigh' | 'Unknown';
    gainEvidenceIdentity?: string;
    reactiveIngressPriority: 'Explicit' | 'Ordinary' | 'Unknown';
  }>;
};

export type AaaReactiveFinding =
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
      interpretation: 'StructuralPossibility';
    }
  | {
      kind: 'ReactiveSelfCycle' | 'ReactiveCrossActorCycle';
      actorIds: string[];
      observationIds: string[];
      canonicalPath: string[];
      interpretation: 'StructuralPossibility';
    }
  | {
      kind: 'ThresholdChatterRisk' | 'MissingHysteresisOrPersistence';
      actorId: string;
      observationId: string;
      steps: number[];
      interpretation: 'StructuralPossibility';
      evidenceIdentity: string;
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

export type AaaFeedbackModel = {
  provenance: 'DeterministicStaticProjection';
  analyzerVersion: typeof AAA_FEEDBACK_ANALYZER_VERSION;
  nodes: AaaFeedbackNode[];
  edges: AaaFeedbackEdge[];
  components: AaaFeedbackComponent[];
  findings: AaaReactiveFinding[];
  evidenceIdentity: string | null;
  evidenceSnapshot: AaaFeedbackEvidenceSnapshot | null;
  limits: {
    maxNodes: number;
    maxEdges: number;
  };
};

export type AaaFeedbackLimits = {
  maxNodes?: number;
  maxEdges?: number;
};

const DEFAULT_MAX_NODES = 256;
const DEFAULT_MAX_EDGES = 2_048;

function fingerprint(value: AaaPlanProjection) {
  return JSON.stringify(value);
}

function uniqueStrings(values: string[]) {
  return [...new Set(values)].sort();
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

function effectClasses(step: AaaStaticStepAnalysis): AaaFeedbackEffectClass[] {
  const effects: AaaFeedbackEffectClass[] = [];
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
  actor: AaaFeedbackActor,
  step: AaaStaticStepAnalysis,
  matcher: AaaObservationEffectMatcher,
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

export function analyzeAaaFeedback(input: {
  actors: AaaFeedbackActor[];
  observations: AaaFeedbackObservation[];
  parameterActuators?: AaaFeedbackParameterActuator[];
  evidence?: AaaFeedbackEvidenceSnapshot;
  limits?: AaaFeedbackLimits;
}): AaaFeedbackModel {
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
    const feed = fingerprint(observation.feed);
    if (observationFeeds.has(feed)) {
      throw new Error('Observation feed projections must be unique');
    }
    observationFeeds.add(feed);
    if (
      observation.provenance === 'Exogenous' &&
      observation.effectMatchers.length > 0
    ) {
      throw new Error('Exogenous observations cannot declare actor effects');
    }
    for (const matcher of observation.effectMatchers) {
      if (matcher.actorId != null && !actorById.has(matcher.actorId)) {
        throw new Error(`Unknown effect-matcher actor: ${matcher.actorId}`);
      }
    }
  }
  const sovereignAccounts = new Set<string>();
  for (const actor of input.actors) {
    if (actor.sovereignAccount != null) {
      const account = fingerprint(actor.sovereignAccount);
      if (sovereignAccounts.has(account)) {
        throw new Error('Actor sovereign accounts must be unique');
      }
      sovereignAccounts.add(account);
    }
  }
  for (const actor of input.actors) {
    if (actor.analysis.provenance !== 'StaticStructuralProjection') {
      throw new Error('Actors must use manifest-authoritative static analysis');
    }
  }
  for (const actuator of actuators) {
    if (!actorById.has(actuator.controlledByActorId)) {
      throw new Error(
        `Unknown actuator controller: ${actuator.controlledByActorId}`,
      );
    }
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
      for (const [label, blocks] of [
        ['cooldownBlocks', policy.cooldownBlocks],
        ['persistenceBlocks', policy.persistenceBlocks],
      ] as const) {
        if (blocks != null && (!Number.isSafeInteger(blocks) || blocks < 0)) {
          throw new Error(
            `${label} must be a non-negative safe integer or null`,
          );
        }
      }
      if (
        policy.gain === 'High' &&
        (policy.gainEvidenceIdentity == null ||
          policy.gainEvidenceIdentity.trim().length === 0)
      ) {
        throw new Error('High gain requires gainEvidenceIdentity');
      }
    }
  }

  const assetByFingerprint = new Map<string, AaaPlanProjection>();
  const collectAsset = (asset: AaaPlanProjection) => {
    assetByFingerprint.set(fingerprint(asset), asset);
  };
  for (const actor of input.actors) {
    actor.analysis.economicSurface.assetsRead.forEach(collectAsset);
    actor.analysis.economicSurface.assetsWritten.forEach(collectAsset);
  }
  actuators.flatMap((actuator) => actuator.affectsAssets).forEach(collectAsset);
  const assetEntries = [...assetByFingerprint.entries()].sort(
    ([left], [right]) => left.localeCompare(right),
  );
  const assetNodeByFingerprint = new Map(
    assetEntries.map(([key], index) => [key, `asset:${index}`]),
  );

  const actorNode = (id: string) => `actor:${id}`;
  const observationNode = (id: string) => `observation:${id}`;
  const actuatorNode = (id: string) => `actuator:${id}`;
  const nodes: AaaFeedbackNode[] = [
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
        provenance: observation.provenance,
      }))
      .sort((left, right) => left.id.localeCompare(right.id)),
    ...assetEntries.map(([, asset], index) => ({
      id: `asset:${index}`,
      kind: 'Asset' as const,
      asset,
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

  const edges: AaaFeedbackEdge[] = [];
  const addEdge = (edge: AaaFeedbackEdge) => edges.push(edge);
  const feedToObservation = new Map(
    input.observations.map((observation) => [
      fingerprint(observation.feed),
      observation.id,
    ]),
  );
  const sovereignToActor = new Map(
    input.actors
      .filter((actor) => actor.sovereignAccount != null)
      .map((actor) => [fingerprint(actor.sovereignAccount!), actor.id]),
  );

  for (const actor of input.actors) {
    for (const feed of actor.analysis.trigger?.observationFeeds ?? []) {
      const observationId = feedToObservation.get(fingerprint(feed));
      if (observationId != null) {
        addEdge({
          from: observationNode(observationId),
          to: actorNode(actor.id),
          kind: 'ObservationTrigger',
          actorId: actor.id,
        });
      }
    }
    for (const step of actor.analysis.steps) {
      for (const condition of step.conditions) {
        if (condition.observation !== 'scalar-observation') continue;
        const surface = condition.readSurface as { feed: AaaPlanProjection };
        const observationId = feedToObservation.get(fingerprint(surface.feed));
        if (observationId != null) {
          addEdge({
            from: observationNode(observationId),
            to: actorNode(actor.id),
            kind: 'ObservationConditionRead',
            actorId: actor.id,
            step: step.index,
          });
        }
      }
      for (const observation of input.observations) {
        if (
          observation.effectMatchers.some((matcher) =>
            matchesEffect(actor, step, matcher),
          )
        ) {
          addEdge({
            from: actorNode(actor.id),
            to: observationNode(observation.id),
            kind: 'ActorEffectOnObservation',
            actorId: actor.id,
            step: step.index,
          });
        }
      }
      for (const recipient of step.economicSurface.possibleActorSignals) {
        if (recipient.kind !== 'Explicit') continue;
        const recipientId = sovereignToActor.get(fingerprint(recipient.value));
        if (recipientId != null) {
          addEdge({
            from: actorNode(actor.id),
            to: actorNode(recipientId),
            kind: 'ActorSignal',
            actorId: actor.id,
            step: step.index,
          });
        }
      }
    }
    for (const asset of actor.analysis.economicSurface.assetsWritten) {
      addEdge({
        from: actorNode(actor.id),
        to: assetNodeByFingerprint.get(fingerprint(asset))!,
        kind: 'SharedAssetWrite',
        actorId: actor.id,
      });
    }
    for (const asset of actor.analysis.economicSurface.assetsRead) {
      addEdge({
        from: assetNodeByFingerprint.get(fingerprint(asset))!,
        to: actorNode(actor.id),
        kind: 'SharedAssetRead',
        actorId: actor.id,
      });
    }
  }

  for (const actuator of actuators) {
    addEdge({
      from: actorNode(actuator.controlledByActorId),
      to: actuatorNode(actuator.id),
      kind: 'ParameterControl',
      actorId: actuator.controlledByActorId,
    });
    for (const observationId of actuator.affectsObservationIds) {
      addEdge({
        from: actuatorNode(actuator.id),
        to: observationNode(observationId),
        kind: 'ParameterEffectOnObservation',
      });
    }
    for (const asset of actuator.affectsAssets) {
      addEdge({
        from: actuatorNode(actuator.id),
        to: assetNodeByFingerprint.get(fingerprint(asset))!,
        kind: 'ParameterEffectOnAsset',
      });
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

  const adjacency = new Map(nodes.map((node) => [node.id, [] as string[]]));
  for (const edge of deduplicatedEdges) adjacency.get(edge.from)!.push(edge.to);
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
    .map((component): AaaFeedbackComponent => {
      const members = component.map((id) => nodeById.get(id)!);
      const actorIds = uniqueStrings(
        members
          .filter(
            (node): node is Extract<AaaFeedbackNode, { kind: 'Actor' }> =>
              node.kind === 'Actor',
          )
          .map((node) => node.actorId),
      );
      const observations = members.filter(
        (node): node is Extract<AaaFeedbackNode, { kind: 'Observation' }> =>
          node.kind === 'Observation',
      );
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
        ) as AaaObservationProvenance[],
        assetNodeIds: uniqueStrings(
          members
            .filter((node) => node.kind === 'Asset')
            .map((node) => node.id),
        ),
        actuatorIds: uniqueStrings(
          members
            .filter(
              (
                node,
              ): node is Extract<
                AaaFeedbackNode,
                { kind: 'ParameterActuator' }
              > => node.kind === 'ParameterActuator',
            )
            .map((node) => node.actuatorId),
        ),
        canonicalPath: canonicalCyclePath(component, adjacency),
        interpretation: 'StructuralPossibility',
        stability: 'Unknown',
        probability: 'Unknown',
        causalStrength: 'Unknown',
      };
    })
    .sort((left, right) =>
      left.canonicalPath.join('|').localeCompare(right.canonicalPath.join('|')),
    );

  const findings: AaaReactiveFinding[] = [];
  for (const component of components) {
    findings.push({
      kind: component.kind,
      actorIds: component.actorIds,
      observationIds: component.observationIds,
      canonicalPath: component.canonicalPath,
      interpretation: 'StructuralPossibility',
    });
    if (component.observationProvenance.includes('Endogenous')) {
      findings.push({
        kind: 'EndogenousObservationFeedback',
        actorIds: component.actorIds,
        observationIds: component.observationIds.filter(
          (id) => observationById.get(id)?.provenance === 'Endogenous',
        ),
        canonicalPath: component.canonicalPath,
        interpretation: 'StructuralPossibility',
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

  if (evidence != null) {
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
            feed: AaaPlanProjection;
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
        if (
          participatesInFeedback &&
          policy?.hysteresis === 'Absent' &&
          (policy.persistenceBlocks == null || policy.persistenceBlocks === 0)
        ) {
          const base = {
            actorId: actor.id,
            observationId,
            steps: [...new Set(steps)].sort((left, right) => left - right),
            interpretation: 'StructuralPossibility' as const,
            evidenceIdentity: evidence.identity,
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
          gainEvidenceIdentity: policy.gainEvidenceIdentity!,
        });
      }
      if (policy?.cooldownBlocks != null) {
        for (const observationId of triggeredObservationIds) {
          const cadence = cadenceByObservation.get(observationId);
          if (
            cadence != null &&
            policy.cooldownBlocks > cadence.minimumUpdateIntervalBlocks
          ) {
            findings.push({
              kind: 'CooldownFeedRateMismatch',
              actorId: actor.id,
              observationId,
              cooldownBlocks: policy.cooldownBlocks,
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
    analyzerVersion: AAA_FEEDBACK_ANALYZER_VERSION,
    nodes,
    edges: deduplicatedEdges,
    components,
    findings,
    evidenceIdentity: evidence?.identity ?? null,
    evidenceSnapshot: evidence ?? null,
    limits: { maxNodes, maxEdges },
  };
}
