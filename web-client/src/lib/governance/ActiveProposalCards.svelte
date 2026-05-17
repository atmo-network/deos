<!--
Domain: Governance active proposal cards
Owns: Active proposal state, tally, semantics, and vote-button card rendering.
Excludes: Proposal submission forms, governance store mutation ownership, finalized history, and adapter transport.
Zone: Governance presentation component; receives typed proposal data and vote callback from widget composition.
-->
<script lang="ts">
  import type {
    GovernancePanelProposal,
    GovernanceVoteKind,
    GovernanceWriteCapability,
  } from '$lib/governance';
  import {
    advisoryScopeLabel,
    cadenceModeLabel,
    executionAuthorityLabel,
    executionPathLabel,
    frozenBallotLabel,
    governanceLockLabel,
    hasDecliningPower,
    openingFeeLabel,
    payloadAvailabilityLabel,
    payloadFamilyLabel,
    payloadKindLabel,
    pendingEnactmentLabel,
    primaryTrackFamilyLabel,
    primaryTrackLeaderLabel,
    primaryTrackPositiveWeightLabel,
    primaryTrackPowerLabel,
    profileLabel,
    statusLabel,
    submissionAuthorityLabel,
    timingWindowLabel,
    treasurySettlementLabel,
    urgentEligibilityLabel,
    urgentExecutionContractLabel,
    voteButtons,
  } from '$lib/governance/labels';
  import { Badge, Button, DetailRow, Notice } from '$lib/ui';

  type Props = {
    proposals: GovernancePanelProposal[];
    compactPane: boolean;
    densePane: boolean;
    voteWriteAvailability: GovernanceWriteCapability;
    onCastVote: (itemId: number, voteKind: GovernanceVoteKind) => void;
  };

  let {
    proposals,
    compactPane,
    densePane,
    voteWriteAvailability,
    onCastVote,
  }: Props = $props();
</script>

<div class="grid gap-3 @2xl:grid-cols-2">
  {#each proposals as proposal}
    <article
      class={[
        'rounded-xl border bg-(--mono-bg) transition-colors hover:border-(--mono-purple)/40',
        densePane ? 'grid gap-2 p-2' : 'grid gap-3 p-3',
      ]}
    >
      <div class="flex flex-wrap items-start justify-between gap-2">
        <div>
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
            Proposal
          </div>
          <div class="text-sm font-semibold text-(--mono-text)">
            #{proposal.itemId}
          </div>
        </div>
        <Badge variant="info">{statusLabel(proposal)}</Badge>
      </div>

      {#snippet tallyRows()}
        {#if proposal.tally}
          {#if proposal.primaryTrackTally?.kind === 'Invoice'}
            <DetailRow
              label="Amplify"
              value={proposal.tally.amplifyWeight.toLocaleString()}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Approve"
              value={proposal.tally.approveWeight.toLocaleString()}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Reduce"
              value={proposal.tally.reduceWeight.toLocaleString()}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Nay"
              value={proposal.tally.nayWeight.toLocaleString()}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Positive total"
              value={primaryTrackPositiveWeightLabel(
                proposal.primaryTrackTally,
              )}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Leading positive"
              value={primaryTrackLeaderLabel(proposal.primaryTrackTally)}
              valueClass="text-(--mono-text)"
            />
          {:else}
            <DetailRow
              label="Aye"
              value={proposal.tally.ayeWeight.toLocaleString()}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Nay"
              value={proposal.tally.nayWeight.toLocaleString()}
              valueClass="text-(--mono-text)"
            />
            <DetailRow
              label="Leading"
              value={primaryTrackLeaderLabel(proposal.primaryTrackTally)}
              valueClass="text-(--mono-text)"
            />
          {/if}
          <DetailRow
            label="Veto"
            value={proposal.tally.vetoWeight.toLocaleString()}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="Pass"
            value={proposal.tally.passWeight.toLocaleString()}
            valueClass="text-(--mono-text)"
          />
        {:else}
          <div class="text-(--mono-muted)">No tally exposed yet</div>
        {/if}
      {/snippet}

      {#snippet semanticsRows(powerLabel: string, vetoLabel: string)}
        <DetailRow
          label="Cadence"
          value={cadenceModeLabel(proposal.metadata?.cadenceMode)}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Family"
          value={payloadFamilyLabel(proposal.metadata?.payloadKind)}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Primary track"
          value={primaryTrackFamilyLabel(proposal.primaryTrackFamily)}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Payload"
          value={payloadKindLabel(proposal.metadata?.payloadKind)}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Payload hash"
          value={proposal.metadata?.payloadHash ?? 'Unavailable'}
          valueClass="text-(--mono-text) break-all"
        />
        {#if advisoryScopeLabel(proposal.metadata?.payloadKind)}
          <DetailRow
            label="Advisory scope"
            value={advisoryScopeLabel(proposal.metadata?.payloadKind) ??
              'Unavailable'}
            valueClass="text-(--mono-text)"
          />
        {/if}
        <DetailRow
          label="Authority"
          value={executionAuthorityLabel(proposal.executionAuthority)}
          valueClass="text-(--mono-text)"
        />
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
        <DetailRow
          label="Payload record"
          value={payloadAvailabilityLabel(
            proposal.metadata?.payloadKind,
            proposal.payloadAvailability,
          )}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Execution path"
          value={executionPathLabel(
            proposal.metadata?.payloadKind,
            proposal.primaryTrackFamily,
          )}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Urgent path"
          value={urgentEligibilityLabel(
            proposal.urgentEligibility,
            proposal.metadata?.payloadKind,
          )}
          valueClass="text-(--mono-text)"
        />
        {#if urgentExecutionContractLabel(proposal.metadata?.payloadKind)}
          <DetailRow
            label="Urgent execution"
            value={urgentExecutionContractLabel(
              proposal.metadata?.payloadKind,
            ) ?? 'Unavailable'}
            valueClass="text-(--mono-text)"
          />
        {/if}
        {#if treasurySettlementLabel(proposal.metadata?.payloadKind, proposal.primaryTrackFamily)}
          <DetailRow
            label="Treasury settlement"
            value={treasurySettlementLabel(
              proposal.metadata?.payloadKind,
              proposal.primaryTrackFamily,
            ) ?? 'Unavailable'}
            valueClass="text-(--mono-text)"
          />
        {/if}
        <DetailRow
          label="Protection window"
          value={timingWindowLabel(
            proposal.timing?.protectionOpenEpoch,
            proposal.timing?.protectionCloseEpoch,
          )}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Primary window"
          value={timingWindowLabel(
            proposal.timing?.effectivePrimaryOpenEpoch,
            proposal.timing?.effectivePrimaryCloseEpoch,
          )}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label="Pending enactment"
          value={pendingEnactmentLabel(proposal.timing?.pendingEnactmentEpoch)}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label={powerLabel}
          value={primaryTrackPowerLabel(proposal)}
          valueClass="text-(--mono-text)"
        />
        <DetailRow
          label={vetoLabel}
          value={profileLabel(proposal.votePowerProfiles.veto)}
          valueClass="text-(--mono-text)"
        />
        {#if proposal.accountPowerView}
          <DetailRow
            label="Account lock"
            value={governanceLockLabel(
              proposal.accountPowerView.governanceLockUntil,
            )}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="My current primary"
            value={proposal.accountPowerView.currentOrdinaryWeight.toLocaleString()}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="My current protection"
            value={proposal.accountPowerView.currentProtectionWeight.toLocaleString()}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="Protection raw"
            value={proposal.accountPowerView.currentProtectionRawPower.toLocaleString()}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="Frozen primary"
            value={frozenBallotLabel(
              proposal.accountPowerView.frozenOrdinaryBallot,
            )}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="Frozen protection"
            value={frozenBallotLabel(
              proposal.accountPowerView.frozenProtectionBallot,
            )}
            valueClass="text-(--mono-text)"
          />
        {/if}
        {#if hasDecliningPower(proposal)}
          <Notice variant="warn">Early ballots carry more weight</Notice>
        {/if}
      {/snippet}

      {#if compactPane}
        <div
          class="rounded-xl border bg-white px-3 py-2 grid gap-1 text-[10px] text-(--mono-muted)"
        >
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
            Proposal state
          </div>
          {@render tallyRows()}
          {@render semanticsRows('Primary track power', 'Veto / Pass power')}
        </div>
      {:else}
        <div class="grid gap-3 @sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
          <div
            class="rounded-xl border bg-white p-3 grid gap-1 text-[10px] text-(--mono-muted)"
          >
            <div
              class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
            >
              Tally
            </div>
            {@render tallyRows()}
          </div>
          <div
            class="rounded-xl border bg-white p-3 grid gap-1 text-[10px] text-(--mono-muted)"
          >
            <div
              class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
            >
              Vote power & semantics
            </div>
            {@render semanticsRows('Primary track', 'Veto / Pass')}
          </div>
        </div>
      {/if}

      <div
        class={[
          'grid gap-2',
          densePane
            ? 'grid-cols-2'
            : 'grid-cols-2 @sm:grid-cols-4 @xl:grid-cols-6',
        ]}
      >
        {#each voteButtons(proposal) as button}
          <Button
            size="sm"
            variant="secondary"
            class="h-auto w-full py-1 text-[10px]"
            disabled={voteWriteAvailability.providerStatus !== 'available'}
            onclick={() => onCastVote(proposal.itemId, button.voteKind)}
            >{button.label}</Button
          >
        {/each}
      </div>
    </article>
  {/each}
</div>
