<!--
Domain: Governance finalized proposals section
Owns: Recent finalized proposal card rendering and execution receipt rows for governance UI.
Excludes: Governance store mutation, active proposal voting, payload submission forms, and adapter transport.
Zone: Governance presentation component; consumes typed governance contracts and UI Kit primitives.
-->
<script lang="ts">
  import type {
    GovernanceAuthorizedRuntimeUpgrade,
    GovernanceRetainedFinalizedProposal,
  } from '$lib/governance';
  import {
    advisoryScopeLabel,
    cadenceModeLabel,
    executionAuthorityLabel,
    executionPathLabel,
    finalizedExecutionDetailRows,
    finalizedExecutionStateLabel,
    finalizedOutcomeLabel,
    openingFeeLabel,
    payloadAvailabilityLabel,
    payloadFamilyLabel,
    payloadKindLabel,
    primaryTrackFamilyLabel,
    retainedWinningPrimaryOptionLabel,
    runtimeUpgradeOperatorPathLabel,
    submissionAuthorityLabel,
    treasurySettlementLabel,
    urgentEligibilityLabel,
    urgentExecutionContractLabel,
  } from '$lib/governance/labels';
  import type { ReadModelProvenance } from '$lib/read-model';
  import {
    Badge,
    DetailRow,
    Notice,
    ReadModelBadge,
    SectionCard,
  } from '$lib/ui';

  type Props = {
    proposals: GovernanceRetainedFinalizedProposal[];
    provenance: ReadModelProvenance | null;
    authorizedRuntimeUpgrade: GovernanceAuthorizedRuntimeUpgrade | null;
  };

  let { proposals, provenance, authorizedRuntimeUpgrade }: Props = $props();

  function finalizedRuntimeUpgradeApplicationLabel(
    proposal: GovernanceRetainedFinalizedProposal,
  ) {
    const detail = proposal.executionDetail;
    if (
      !detail ||
      detail.kind !== 'Executed' ||
      detail.detail.kind !== 'RuntimeUpgradeAuthorized'
    ) {
      return null;
    }
    if (!authorizedRuntimeUpgrade) {
      return 'No pending authorized upgrade';
    }
    return authorizedRuntimeUpgrade.codeHash === detail.detail.codeHash
      ? 'Pending authorized code relay'
      : 'Different authorization is pending';
  }
</script>

<SectionCard title="Recent finalized" subtitle="Bounded recent outcomes">
  {#snippet actions()}
    <ReadModelBadge {provenance} />
    <Badge variant="info">{proposals.length} retained</Badge>
  {/snippet}
  <Notice variant="muted">
    Bounded retained on-chain history only, not a full governance archive
  </Notice>
  {#if proposals.length > 0}
    <div class="grid gap-2">
      {#each proposals as proposal}
        <div class="rounded-xl border bg-(--mono-bg) px-3 py-2 grid gap-1">
          <DetailRow
            label={`#${proposal.itemId}`}
            value={finalizedOutcomeLabel(proposal)}
            valueClass="text-(--mono-muted)"
          />
          {#if proposal.metadata}
            <DetailRow
              label="Cadence"
              value={cadenceModeLabel(proposal.metadata.cadenceMode)}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Family"
              value={payloadFamilyLabel(proposal.metadata.payloadKind)}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Primary track"
              value={primaryTrackFamilyLabel(proposal.primaryTrackFamily)}
              valueClass="text-(--mono-text)"
            />
            {#if retainedWinningPrimaryOptionLabel(proposal)}
              <DetailRow
                label="Winning primary"
                value={retainedWinningPrimaryOptionLabel(proposal) ??
                  'Unavailable'}
                valueClass="text-(--mono-text)"
              />
            {/if}
            <DetailRow
              label="Payload"
              value={payloadKindLabel(proposal.metadata.payloadKind)}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Payload hash"
              value={proposal.metadata.payloadHash}
              valueClass="text-(--mono-text) break-all"
            />
            {#if advisoryScopeLabel(proposal.metadata.payloadKind)}
              <DetailRow
                label="Advisory scope"
                value={advisoryScopeLabel(proposal.metadata.payloadKind) ??
                  'Unavailable'}
                valueClass="text-(--mono-text)"
              />
            {/if}
            <DetailRow
              label="Urgent path"
              value={urgentEligibilityLabel(
                proposal.urgentEligibility,
                proposal.metadata.payloadKind,
              )}
              valueClass="text-(--mono-text)"
            />
            {#if urgentExecutionContractLabel(proposal.metadata.payloadKind)}
              <DetailRow
                label="Urgent execution"
                value={urgentExecutionContractLabel(
                  proposal.metadata.payloadKind,
                ) ?? 'Unavailable'}
                valueClass="text-(--mono-text)"
              />
            {/if}
          {/if}
          {#if proposal.executionAuthority}
            <DetailRow
              label="Authority"
              value={executionAuthorityLabel(proposal.executionAuthority)}
              valueClass="text-(--mono-text)"
            />
          {/if}
          {#if proposal.submissionAuthority}
            <DetailRow
              label="Submission"
              value={submissionAuthorityLabel(proposal.submissionAuthority)}
              valueClass="text-(--mono-text)"
            />
            {#if proposal.submissionAuthority === 'Signed'}
              <DetailRow
                label="Opening fee"
                value={openingFeeLabel(
                  proposal.submissionAuthority,
                  proposal.openingFee,
                )}
                valueClass="text-(--mono-text)"
              />
            {/if}
          {/if}
          {#if proposal.metadata && proposal.payloadAvailability}
            <DetailRow
              label="Payload record"
              value={payloadAvailabilityLabel(
                proposal.metadata.payloadKind,
                proposal.payloadAvailability,
              )}
              valueClass="text-(--mono-text)"
            />
          {/if}
          <DetailRow
            label="Execution state"
            value={finalizedExecutionStateLabel(proposal)}
            valueClass="text-(--mono-text)"
          />
          {#if proposal.metadata}
            <DetailRow
              label="Execution path"
              value={executionPathLabel(
                proposal.metadata.payloadKind,
                proposal.primaryTrackFamily,
              )}
              valueClass="text-(--mono-text)"
            />
            {#if treasurySettlementLabel(proposal.metadata.payloadKind, proposal.primaryTrackFamily)}
              <DetailRow
                label="Treasury settlement"
                value={treasurySettlementLabel(
                  proposal.metadata.payloadKind,
                  proposal.primaryTrackFamily,
                ) ?? 'Unavailable'}
                valueClass="text-(--mono-text)"
              />
            {/if}
          {/if}
          {#each finalizedExecutionDetailRows(proposal.executionDetail) as row}
            <DetailRow
              label={row.label}
              value={row.value}
              valueClass={row.mono
                ? 'text-(--mono-text) break-all'
                : 'text-(--mono-text)'}
            />
          {/each}
          {#if finalizedRuntimeUpgradeApplicationLabel(proposal)}
            <DetailRow
              label="Upgrade application"
              value={finalizedRuntimeUpgradeApplicationLabel(proposal) ??
                'Unavailable'}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Operator path"
              value={runtimeUpgradeOperatorPathLabel()}
              valueClass="text-(--mono-text) break-all"
            />
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <Notice>No retained finalized proposals yet</Notice>
  {/if}
</SectionCard>
