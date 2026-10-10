/*
Domain: Actors risk and composition warnings
Owns: Bounded projection of shipped runtime facts into typed composition warnings.
Excludes: New protocol policy, scoring, probability, economic stability claims, and runtime mutation.
Zone: Automation domain capability; consumes ActorContractStaticAnalysis and Actor Contract artifacts only.
*/
import type { ActorContractStaticAnalysis } from './analysis.ts';
import type { ActorContractArtifact } from './contract-artifact.ts';

export type ActorCompositionWarningKind =
  | 'ImmutableWithoutReachableTerminal'
  | 'ResidualCustodyThroughLocatorReuse'
  | 'DeepActorGraphAmplification'
  | 'TriggerRevisionCoalescing'
  | 'StrictFifoHeadOfLine'
  | 'CompletedDoesNotImplyAllTasksSuccess';

export type ActorCompositionWarning = {
  kind: ActorCompositionWarningKind;
  severity: 'info' | 'warning' | 'critical';
  message: string;
  evidence: string;
};

export type ActorCompositionWarningInput = {
  artifact: ActorContractArtifact;
  analysis: ActorContractStaticAnalysis;
  strictFifoHeadOfLine?: boolean;
  simulatorStatus?: 'Completed' | 'Failed' | 'Suspended' | 'Closed';
  successfulTaskCount?: number;
  totalTaskCount?: number;
};

/**
 * Project shipped runtime facts into typed composition warnings. Every
 * warning is a projection of an explicit runtime contract (single FIFO,
 * canonical temporal placement, immutable custody, per-step error policy,
 * split-transfer minimum-balance classification), never new protocol policy.
 */
export function projectActorCompositionWarnings(
  input: ActorCompositionWarningInput,
): ActorCompositionWarning[] {
  const warnings: ActorCompositionWarning[] = [];
  const { artifact, analysis } = input;

  if (artifact.mutability === 'Immutable') {
    const hasAuthoredTerminal =
      analysis.autoCloseAtCycleNonce !== null || analysis.closeWhen !== null;
    if (!hasAuthoredTerminal) {
      warnings.push({
        kind: 'ImmutableWithoutReachableTerminal',
        severity: 'critical',
        message:
          'Immutable actor has no authored lifetime terminal; its owner cannot close it early.',
        evidence: `mutability=Immutable, no close_when or auto-close nonce; Cycle abort is not Actor destruction and mandatory safety terminals remain independent`,
      });
    }
  }

  const custodyFindings = analysis.findings.filter(
    (finding) =>
      finding.kind === 'StopCycleFailureMayFallThrough' ||
      finding.kind === 'PreExistingBalanceMixedWithCurrentRunOutput',
  );
  if (custodyFindings.length > 0) {
    warnings.push({
      kind: 'ResidualCustodyThroughLocatorReuse',
      severity: 'warning',
      message:
        'Residual balances may be inherited by the actor locator across runs or locator reuse; custody is not reset by stop/close.',
      evidence: `${custodyFindings.length} custody finding(s) (${custodyFindings.map((f) => f.kind).join(', ')})`,
    });
  }

  const crossActorEdges = analysis.findings.filter(
    (finding) => finding.kind === 'PotentialCrossActorFeedbackEdge',
  );
  if (analysis.steps.length >= 3 || crossActorEdges.length > 0) {
    warnings.push({
      kind: 'DeepActorGraphAmplification',
      severity: 'info',
      message:
        'Deep step graph (or cross-actor feedback edges) amplifies latency, fees, and minimum-balance exposure across hops.',
      evidence: `${analysis.steps.length} step(s), ${crossActorEdges.length} cross-actor edge(s)`,
    });
  }

  const signalFindings = analysis.findings.filter(
    (finding) => finding.kind === 'ExternallySignalledAdmission',
  );
  const hasSameBlockCoalescing =
    analysis.trigger != null &&
    analysis.trigger.sourceKinds.includes('AddressEvent');
  if (hasSameBlockCoalescing || signalFindings.length > 0) {
    warnings.push({
      kind: 'TriggerRevisionCoalescing',
      severity: 'info',
      message:
        'Canonical temporal placement coalesces same-block trigger revisions into one execution per block.',
      evidence: `sources=${analysis.trigger?.sourceKinds.join('/') ?? 'none'}, ${signalFindings.length} signal finding(s)`,
    });
  }

  if (input.strictFifoHeadOfLine === true) {
    warnings.push({
      kind: 'StrictFifoHeadOfLine',
      severity: 'info',
      message:
        'Strict FIFO delivery means one slow head actor delays follower execution in the shared FIFO.',
      evidence: 'shared single FIFO, no per-actor priority',
    });
  }

  if (
    input.simulatorStatus === 'Completed' &&
    input.successfulTaskCount != null &&
    input.totalTaskCount != null &&
    input.successfulTaskCount < input.totalTaskCount
  ) {
    warnings.push({
      kind: 'CompletedDoesNotImplyAllTasksSuccess',
      severity: 'warning',
      message:
        'Simulator status Completed does not mean every task succeeded; some tasks may have failed while the run continued.',
      evidence: `status=Completed, ${input.successfulTaskCount}/${input.totalTaskCount} tasks succeeded`,
    });
  }

  return warnings;
}

export const ACTORS_COMPOSITION_WARNING_KINDS: readonly ActorCompositionWarningKind[] =
  [
    'ImmutableWithoutReachableTerminal',
    'ResidualCustodyThroughLocatorReuse',
    'DeepActorGraphAmplification',
    'TriggerRevisionCoalescing',
    'StrictFifoHeadOfLine',
    'CompletedDoesNotImplyAllTasksSuccess',
  ];
