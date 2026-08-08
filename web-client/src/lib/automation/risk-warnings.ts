/*
Domain: AAA risk and composition warnings
Owns: Bounded projection of shipped runtime facts into typed composition warnings.
Excludes: New protocol policy, scoring, probability, economic stability claims, and runtime mutation.
Zone: Automation domain capability; consumes ProgramStaticAnalysis and plan artifacts only.
*/
import type { ProgramStaticAnalysis } from './analysis.ts';
import type { AaaPlanArtifact } from './plan-artifact.ts';

export type AaaCompositionWarningKind =
  | 'ImmutableWithoutReachableTerminal'
  | 'ResidualCustodyThroughLocatorReuse'
  | 'DeepActorGraphAmplification'
  | 'TriggerRevisionCoalescing'
  | 'StrictFifoHeadOfLine'
  | 'CompletedDoesNotImplyAllTasksSuccess';

export type AaaCompositionWarning = {
  kind: AaaCompositionWarningKind;
  severity: 'info' | 'warning' | 'critical';
  message: string;
  evidence: string;
};

export type AaaCompositionWarningInput = {
  artifact: AaaPlanArtifact;
  analysis: ProgramStaticAnalysis;
  strictFifoHeadOfLine?: boolean;
  simulatorStatus?: 'Completed' | 'Failed' | 'Suspended' | 'Closed';
  successfulTaskCount?: number;
  totalTaskCount?: number;
};

function isTerminalStep(step: ProgramStaticAnalysis['steps'][number]): boolean {
  // A step is terminal-reachable when a failure path ends the run without a
  // later resumable step: AbortCycle error policy or a zero-retry boundary.
  return step.errorPolicy === 'AbortCycle' || step.retryMaxAttempts === 0;
}

/**
 * Project shipped runtime facts into typed composition warnings. Every
 * warning is a projection of an explicit runtime contract (single FIFO,
 * canonical temporal placement, immutable custody, per-step error policy,
 * split-transfer minimum-balance classification), never new protocol policy.
 */
export function projectAaaCompositionWarnings(
  input: AaaCompositionWarningInput,
): AaaCompositionWarning[] {
  const warnings: AaaCompositionWarning[] = [];
  const { artifact, analysis } = input;

  if (artifact.mutability === 'Immutable') {
    const reachableTerminal = analysis.steps.some(isTerminalStep);
    if (!reachableTerminal) {
      warnings.push({
        kind: 'ImmutableWithoutReachableTerminal',
        severity: 'critical',
        message:
          'Immutable actor has no reachable terminal condition and will keep custody permanently.',
        evidence: `mutability=Immutable, ${analysis.steps.length} step(s), no AbortCycle/zero-retry terminal step`,
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
    (finding) =>
      finding.kind === 'ExternallySignalledAdmission' ||
      finding.kind === 'TriggerAmountCompatibilityViolation',
  );
  const hasSameBlockCoalescing =
    analysis.trigger != null &&
    analysis.trigger.sourceKinds.some(
      (kind) => kind === 'AddressEvent' || kind === 'ObservationChange',
    );
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

export const AAA_COMPOSITION_WARNING_KINDS: readonly AaaCompositionWarningKind[] =
  [
    'ImmutableWithoutReachableTerminal',
    'ResidualCustodyThroughLocatorReuse',
    'DeepActorGraphAmplification',
    'TriggerRevisionCoalescing',
    'StrictFifoHeadOfLine',
    'CompletedDoesNotImplyAllTasksSuccess',
  ];
