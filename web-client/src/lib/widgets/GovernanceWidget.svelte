<!--
Domain: Governance widget
Owns: Browser composition of governance proposal viewing, voting, and signed payload submission UX.
Excludes: Governance label policy, transport adapters, payload encoding primitives, and UI Kit implementation.
Zone: Presentation widget; consumes governance store/contracts and UI Kit without importing concrete chain internals.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import { parseUnsignedDecimalNumber } from '$lib/format/numeric';
  import type {
    GovernanceMaterializedArchiveEntry,
    GovernanceProposalPayloadKind,
  } from '$lib/governance';
  import ActionReviewCard from '$lib/governance/ActionReviewCard.svelte';
  import ActiveProposalCards from '$lib/governance/ActiveProposalCards.svelte';
  import AuthorizedRuntimeUpgradeNotice from '$lib/governance/AuthorizedRuntimeUpgradeNotice.svelte';
  import FinalizedProposalsSection from '$lib/governance/FinalizedProposalsSection.svelte';
  import GovernanceArchiveSection from '$lib/governance/GovernanceArchiveSection.svelte';
  import ProposalSemanticsRows from '$lib/governance/ProposalSemanticsRows.svelte';
  import {
    GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES,
    GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES,
    deriveGovernanceAdvisoryPayloadDraftState,
  } from '$lib/governance/advisory-payload';
  import { governanceStore } from '$lib/governance/index.svelte';
  import {
    advisoryPayloadHashPreimageStatusLabel,
    advisoryPayloadPreimageNoteCostLabel,
    advisoryScopeLabel,
    executionAuthorityLabel,
    executionPathLabel,
    openingFeeLabel,
    payloadFamilyLabel,
    payloadKindLabel,
    publicSubmissionButtonLabel,
    publicSubmissionPayloadLabel,
    publicSubmissionPurposeNotice,
    publicSubmissionSummaryPlaceholder,
    treasurySettlementLabel,
  } from '$lib/governance/labels';
  import { GovernanceUnavailableMaterializedProvider } from '$lib/governance/materialized';
  import { createPayloadReview } from '$lib/governance/payload-review.svelte';
  import { deriveGovernanceTreasuryPayloadDraftState } from '$lib/governance/treasury-payload';
  import { fromMaterialized } from '$lib/read-model';
  import {
    chainSurfaceIsBlocking,
    resolveChainSurfaceState,
  } from '$lib/system/connection-surface';
  import {
    Badge,
    Button,
    Card,
    DetailRow,
    Notice,
    NumberInput,
    SectionCard,
    SelectField,
    TextArea,
    TextField,
  } from '$lib/ui';

  type ReferencedPayloadSuggestion = {
    payloadHash: string;
    label: string;
  };

  const materializedArchiveProvider =
    new GovernanceUnavailableMaterializedProvider();
  const materializedArchivePlaceholder = fromMaterialized<
    GovernanceMaterializedArchiveEntry[]
  >([], 'archive-api', 'Governance materialized archive provider', 'archive');

  const governanceChainSurface = $derived(
    resolveChainSurfaceState(
      governanceStore.state.providerState,
      (governanceStore.state.providerState.status === 'connected' ||
        governanceStore.state.providerState.status === 'mock') &&
        !governanceStore.state.loading &&
        governanceStore.state.error === null,
    ),
  );
  const governanceChainBlocked = $derived(
    chainSurfaceIsBlocking(governanceChainSurface),
  );
  const voteWriteAvailability = $derived(
    governanceStore.state.writeSurfaceAvailability.castVote,
  );
  const submitWriteAvailability = $derived(
    governanceStore.state.writeSurfaceAvailability.submitProposal,
  );
  const preimageWriteAvailability = $derived(
    governanceStore.state.writeSurfaceAvailability.noteProposalPreimage,
  );
  let submitItemIdInput = $state('');
  let submitSummaryInput = $state('');
  let submitDocCidInput = $state('');
  let submitReferencedPayloadHashInput = $state('');
  let selectedSubmitPayloadKind = $state<GovernanceProposalPayloadKind | null>(
    null,
  );
  let treasurySubmitItemIdInput = $state('');
  let treasuryBeneficiaryInput = $state('');
  let treasuryPayoutAssetInput = $state('');
  let treasuryBaseAmountInput = $state('');

  const advisoryReview = createPayloadReview(
    () => advisoryPayloadDraft.encoding?.payloadBytes ?? null,
  );
  const treasuryReview = createPayloadReview(
    () => treasuryPayloadDraft.encoding?.payloadBytes ?? null,
  );

  function isAdvisorySubmissionKind(
    payloadKind: GovernanceProposalPayloadKind,
  ) {
    return payloadKind === 'Intent' || payloadKind === 'L2SignalToL1';
  }

  const advisorySubmissionOptions = $derived(
    governanceStore.state.submissionOptions.filter((option) =>
      isAdvisorySubmissionKind(option.payloadKind),
    ),
  );
  const treasurySubmissionOption = $derived(
    governanceStore.state.submissionOptions.find(
      (option) => option.payloadKind === 'L2TreasurySpend',
    ) ?? null,
  );
  const unsupportedSignedSubmissionOptions = $derived(
    governanceStore.state.submissionOptions.filter(
      (option) =>
        !isAdvisorySubmissionKind(option.payloadKind) &&
        option.payloadKind !== 'L2TreasurySpend',
    ),
  );

  $effect(() => {
    const options = advisorySubmissionOptions;
    if (
      selectedSubmitPayloadKind &&
      options.some((option) => option.payloadKind === selectedSubmitPayloadKind)
    ) {
      return;
    }
    selectedSubmitPayloadKind = options[0]?.payloadKind ?? null;
  });

  const treasuryPayloadDraft = $derived(
    deriveGovernanceTreasuryPayloadDraftState({
      beneficiary: treasuryBeneficiaryInput,
      payoutAsset: treasuryPayoutAssetInput,
      baseAmount: treasuryBaseAmountInput,
    }),
  );

  function authorizedRuntimeUpgradeLabel() {
    const authorization = governanceStore.state.authorizedRuntimeUpgrade;
    if (!authorization) {
      return 'None';
    }
    return authorization.checkVersion
      ? `${authorization.codeHash} · version checked`
      : `${authorization.codeHash} · no version check`;
  }

  function parsePositiveItemIdInput(value: string, fallback: number) {
    if (value.trim().length === 0) {
      return fallback;
    }
    return parseUnsignedDecimalNumber(value, { min: 1 });
  }

  function parsedSubmitItemId() {
    return parsePositiveItemIdInput(submitItemIdInput, suggestedSubmitItemId);
  }

  function applyReferencedPayloadSuggestion(payloadHash: string) {
    submitReferencedPayloadHashInput = payloadHash;
  }

  function clearReferencedPayloadSuggestion() {
    submitReferencedPayloadHashInput = '';
  }

  function handleReferencedPayloadSuggestionChange(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLSelectElement) || target.value.length === 0) {
      return;
    }
    applyReferencedPayloadSuggestion(target.value);
    target.value = '';
  }

  const selectedSubmissionOption = $derived(
    advisorySubmissionOptions.find(
      (option) => option.payloadKind === selectedSubmitPayloadKind,
    ) ?? null,
  );
  const treasurySubmissionOptionLabel = $derived(
    treasurySubmissionOption
      ? publicSubmissionPayloadLabel(treasurySubmissionOption.payloadKind)
      : 'Unavailable',
  );
  const suggestedSubmitItemId = $derived.by(() => {
    let highestSeenItemId = 0;
    for (const itemId of governanceStore.state.activeProposalIds) {
      if (itemId > highestSeenItemId) {
        highestSeenItemId = itemId;
      }
    }
    for (const proposal of governanceStore.state.recentFinalizedProposals) {
      if (proposal.itemId > highestSeenItemId) {
        highestSeenItemId = proposal.itemId;
      }
    }
    return highestSeenItemId + 1;
  });
  const referencedPayloadSuggestions = $derived.by(() => {
    const suggestions: ReferencedPayloadSuggestion[] = [];
    const seen = new Set<string>();
    for (const proposal of governanceStore.state.activeProposals) {
      const payloadHash = proposal.metadata?.payloadHash;
      if (!payloadHash || seen.has(payloadHash)) {
        continue;
      }
      seen.add(payloadHash);
      suggestions.push({
        payloadHash,
        label: `Active #${proposal.itemId} · ${payloadKindLabel(proposal.metadata?.payloadKind)}`,
      });
    }
    for (const proposal of governanceStore.state.recentFinalizedProposals) {
      const payloadHash = proposal.metadata?.payloadHash;
      if (!payloadHash || seen.has(payloadHash)) {
        continue;
      }
      seen.add(payloadHash);
      suggestions.push({
        payloadHash,
        label: `Retained #${proposal.itemId} · ${payloadKindLabel(proposal.metadata?.payloadKind)}`,
      });
    }
    return suggestions;
  });
  const selectedReferencedPayloadSuggestion = $derived(
    referencedPayloadSuggestions.find(
      (suggestion) =>
        suggestion.payloadHash === submitReferencedPayloadHashInput.trim(),
    ) ?? null,
  );
  const referencedPayloadSourceLabel = $derived.by(() => {
    const trimmed = submitReferencedPayloadHashInput.trim();
    if (trimmed.length === 0) {
      return null;
    }
    if (selectedReferencedPayloadSuggestion) {
      return selectedReferencedPayloadSuggestion.label;
    }
    if (/^0x[0-9a-fA-F]{64}$/.test(trimmed)) {
      return 'Manual / not in visible bounded set';
    }
    return null;
  });
  const advisoryPayloadDraft = $derived(
    deriveGovernanceAdvisoryPayloadDraftState({
      summary: submitSummaryInput,
      docCid: submitDocCidInput,
      referencedPayloadHash: submitReferencedPayloadHashInput,
    }),
  );
  const submitPayloadReady = $derived(
    advisoryPayloadDraft.encoding !== null &&
      advisoryReview.payloadHash !== null,
  );
  const submitItemReady = $derived(parsedSubmitItemId() !== null);
  const treasurySubmitPayloadReady = $derived(
    treasuryPayloadDraft.encoding !== null &&
      treasuryReview.payloadHash !== null,
  );
  const treasurySubmitItemReady = $derived(
    parsedTreasurySubmitItemId() !== null,
  );
  const submitPublicProposalDisabled = $derived(
    submitWriteAvailability.providerStatus !== 'available' ||
      selectedSubmitPayloadKind == null ||
      !submitItemReady ||
      !submitPayloadReady ||
      advisoryReview.payloadHashLoading,
  );
  const notePreimageDisabled = $derived(
    preimageWriteAvailability.providerStatus !== 'available' ||
      advisoryPayloadDraft.encoding == null ||
      advisoryReview.payloadHashLoading ||
      advisoryReview.payloadHashPreimageStatusLoading ||
      advisoryReview.payloadPreimageNoteCostLoading ||
      advisoryReview.payloadHashPreimageStatus?.havePreimage === true,
  );
  const treasurySubmitProposalDisabled = $derived(
    submitWriteAvailability.providerStatus !== 'available' ||
      treasurySubmissionOption == null ||
      !treasurySubmitItemReady ||
      !treasurySubmitPayloadReady ||
      treasuryReview.payloadHashLoading,
  );
  const treasuryNotePreimageDisabled = $derived(
    preimageWriteAvailability.providerStatus !== 'available' ||
      treasuryPayloadDraft.encoding == null ||
      treasuryReview.payloadHashLoading ||
      treasuryReview.payloadHashPreimageStatusLoading ||
      treasuryReview.payloadPreimageNoteCostLoading ||
      treasuryReview.payloadHashPreimageStatus?.havePreimage === true,
  );

  function submitReviewStatusLabel() {
    if (selectedSubmitPayloadKind == null) {
      return 'Pick a signed advisory payload kind';
    }
    if (!submitItemReady) {
      return 'Enter a valid positive item id';
    }
    if (!advisoryPayloadDraft.summaryValid) {
      return 'Fix the advisory summary';
    }
    if (!advisoryPayloadDraft.docCidValid) {
      return 'Fix the doc CID length';
    }
    if (!advisoryPayloadDraft.referencedPayloadHashValid) {
      return 'Fix the referenced payload hash';
    }
    if (advisoryReview.payloadHashLoading) {
      return 'Computing the advisory payload hash';
    }
    if (submitWriteAvailability.providerStatus !== 'available') {
      return submitWriteAvailability.reason;
    }
    return 'Ready to submit signed advisory proposal';
  }

  function preimageReviewStatusLabel() {
    if (advisoryReview.payloadHashLoading) {
      return 'Computing the advisory payload hash';
    }
    if (
      advisoryReview.payloadHashPreimageStatusLoading ||
      advisoryReview.payloadPreimageNoteCostLoading
    ) {
      return 'Checking preimage status and note cost';
    }
    if (
      advisoryPayloadDraft.encoding == null ||
      advisoryReview.payloadHash == null
    ) {
      return 'Finish the advisory draft first';
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return 'These exact bytes are already noted on-chain';
    }
    if (preimageWriteAvailability.providerStatus !== 'available') {
      return preimageWriteAvailability.reason;
    }
    if (advisoryReview.payloadHashPreimageStatus?.preimageRequested) {
      return 'Optional separate note path can satisfy an existing request';
    }
    return 'Optional separate preimage note path is available';
  }

  function submitReviewSummaryLine() {
    const payloadKind = selectedSubmitPayloadKind;
    if (payloadKind == null) {
      return 'Unavailable';
    }
    return `${publicSubmissionButtonLabel(payloadKind)} · ${executionPathLabel(payloadKind, 'Binary')}`;
  }

  function preimageReviewSummaryLine() {
    if (advisoryPayloadDraft.encoding == null) {
      return 'Unavailable';
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return 'No extra preimage note is needed';
    }
    return `Optional Preimage.note_preimage on ${advisoryPayloadDraft.encoding.payloadByteLength} bytes`;
  }

  function submitReviewResultLine() {
    const itemId = parsedSubmitItemId();
    if (itemId == null || advisoryReview.payloadHash == null) {
      return 'Unavailable';
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return `Creates proposal #${itemId} with payload hash ${advisoryReview.payloadHash} and already-noted bytes`;
    }
    return `Creates proposal #${itemId} with payload hash ${advisoryReview.payloadHash} only`;
  }

  function preimageReviewResultLine() {
    if (advisoryReview.payloadHash == null) {
      return 'Unavailable';
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return `No chain change needed because bytes for ${advisoryReview.payloadHash} are already noted`;
    }
    if (advisoryReview.payloadHashPreimageStatus?.preimageRequested) {
      return `Publishes payload bytes for ${advisoryReview.payloadHash} and satisfies an existing on-chain request`;
    }
    return `Publishes payload bytes for later on-chain lookup by ${advisoryReview.payloadHash}`;
  }

  function parsedTreasurySubmitItemId() {
    return parsePositiveItemIdInput(
      treasurySubmitItemIdInput,
      suggestedSubmitItemId,
    );
  }

  function treasurySubmitReviewStatusLabel() {
    if (treasurySubmissionOption == null) {
      return 'Signed tactical treasury submission is unavailable';
    }
    if (!treasurySubmitItemReady) {
      return 'Enter a valid positive item id';
    }
    if (!treasuryPayloadDraft.beneficiaryValid) {
      return 'Enter a valid beneficiary address';
    }
    if (!treasuryPayloadDraft.payoutAssetValid) {
      return 'Enter a valid payout asset id';
    }
    if (!treasuryPayloadDraft.baseAmountValid) {
      return 'Enter a valid positive base amount';
    }
    if (treasuryReview.payloadHashLoading) {
      return 'Computing the treasury payload hash';
    }
    if (submitWriteAvailability.providerStatus !== 'available') {
      return submitWriteAvailability.reason;
    }
    return 'Ready to submit tactical treasury invoice proposal';
  }

  function treasuryPreimageReviewStatusLabel() {
    if (treasuryReview.payloadHashLoading) {
      return 'Computing the treasury payload hash';
    }
    if (
      treasuryReview.payloadHashPreimageStatusLoading ||
      treasuryReview.payloadPreimageNoteCostLoading
    ) {
      return 'Checking preimage status and note cost';
    }
    if (
      treasuryPayloadDraft.encoding == null ||
      treasuryReview.payloadHash == null
    ) {
      return 'Finish the treasury payload draft first';
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return 'These exact bytes are already noted on-chain';
    }
    if (preimageWriteAvailability.providerStatus !== 'available') {
      return preimageWriteAvailability.reason;
    }
    if (treasuryReview.payloadHashPreimageStatus?.preimageRequested) {
      return 'Optional separate note path can satisfy an existing request';
    }
    return 'Optional separate preimage note path is available';
  }

  function treasurySubmitReviewResultLine() {
    const itemId = parsedTreasurySubmitItemId();
    if (itemId == null || treasuryReview.payloadHash == null) {
      return 'Unavailable';
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return `Creates proposal #${itemId} with payload hash ${treasuryReview.payloadHash} and already-noted bytes`;
    }
    return `Creates proposal #${itemId} with payload hash ${treasuryReview.payloadHash} only`;
  }

  function treasuryPreimageReviewResultLine() {
    if (treasuryReview.payloadHash == null) {
      return 'Unavailable';
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return `No chain change needed because bytes for ${treasuryReview.payloadHash} are already noted`;
    }
    if (treasuryReview.payloadHashPreimageStatus?.preimageRequested) {
      return `Publishes payload bytes for ${treasuryReview.payloadHash} and satisfies an existing on-chain request`;
    }
    return `Publishes payload bytes for later on-chain lookup by ${treasuryReview.payloadHash}`;
  }

  function treasuryPreimageReviewSummaryLine() {
    if (treasuryPayloadDraft.encoding == null) {
      return 'Unavailable';
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return 'No extra preimage note is needed';
    }
    return `Optional Preimage.note_preimage on ${treasuryPayloadDraft.encoding.payloadByteLength} bytes`;
  }

  function applySuggestedSubmitItemId() {
    submitItemIdInput = suggestedSubmitItemId.toString();
  }

  function applySuggestedTreasurySubmitItemId() {
    treasurySubmitItemIdInput = suggestedSubmitItemId.toString();
  }

  async function submitPublicProposal() {
    const itemId = parsedSubmitItemId();
    if (
      selectedSubmitPayloadKind == null ||
      itemId == null ||
      advisoryReview.payloadHash == null
    ) {
      return;
    }
    await governanceStore.submitProposal({
      itemId,
      cadenceMode: 'Ordinary',
      payloadKind: selectedSubmitPayloadKind,
      payloadHash: advisoryReview.payloadHash,
    });
  }

  async function noteProposalPreimage() {
    const payloadBytes = advisoryPayloadDraft.encoding?.payloadBytes ?? null;
    if (payloadBytes == null) {
      return;
    }
    await governanceStore.noteProposalPreimage(payloadBytes);
  }

  async function submitTreasuryProposal() {
    const itemId = parsedTreasurySubmitItemId();
    if (
      treasurySubmissionOption == null ||
      itemId == null ||
      treasuryReview.payloadHash == null
    ) {
      return;
    }
    await governanceStore.submitProposal({
      itemId,
      cadenceMode: 'Ordinary',
      payloadKind: treasurySubmissionOption.payloadKind,
      payloadHash: treasuryReview.payloadHash,
    });
  }

  async function noteTreasuryProposalPreimage() {
    const payloadBytes = treasuryPayloadDraft.encoding?.payloadBytes ?? null;
    if (payloadBytes == null) {
      return;
    }
    await governanceStore.noteProposalPreimage(payloadBytes);
  }

  onMount(() => {
    void governanceStore.init();
  });
</script>

<Card class="min-h-full p-2">
  <div class="@container grid gap-3 pb-2">
    <div class="governance-grid grid gap-3">
      <SectionCard
        title="Active proposals"
        subtitle="Vote here, receipts appear in Log"
      >
        {#snippet actions()}
          <Badge variant="info">
            {governanceStore.state.error || governanceChainBlocked
              ? '—'
              : `${governanceStore.state.activeProposals.length} open`}
          </Badge>
        {/snippet}
        {#if governanceStore.state.error}
          <Notice variant="warn">
            Governance query failed: {governanceStore.state.error}
          </Notice>
        {:else if !governanceChainBlocked}
          {#if governanceChainSurface.status === 'preview' || governanceChainSurface.status === 'stale'}
            <Notice variant="warn" class="grid gap-0.5">
              <strong>{governanceChainSurface.title}</strong>
              <span>{governanceChainSurface.detail}</span>
            </Notice>
          {/if}
          {#if voteWriteAvailability.providerStatus !== 'available'}
            <Notice variant="warn">{voteWriteAvailability.reason}</Notice>
          {/if}
          {#if governanceStore.state.writeError}
            <Notice variant="warn">{governanceStore.state.writeError}</Notice>
          {/if}
          {#if governanceStore.state.authorizedRuntimeUpgrade}
            <AuthorizedRuntimeUpgradeNotice
              authorization={governanceStore.state.authorizedRuntimeUpgrade}
            />
          {/if}
          {#if governanceStore.state.loading}
            <Notice>Loading proposals...</Notice>
          {:else if governanceStore.state.activeProposals.length === 0}
            <Notice>No active proposals</Notice>
          {:else}
            <ActiveProposalCards
              proposals={governanceStore.state.activeProposals}
              {voteWriteAvailability}
              onCastVote={(itemId, voteKind) =>
                governanceStore.castVote(itemId, voteKind)}
            />
          {/if}
          <details class="rounded-xl bg-(--mono-bg) text-(--mono-muted)">
            <summary
              class="cursor-pointer px-3 py-2 text-2xs font-semibold uppercase tracking-wider text-(--mono-text)"
            >
              Compose a proposal
            </summary>
            <div class="composition-stack grid gap-3 px-3 pb-3">
              <div
                class="rounded-xl bg-white p-3 grid gap-2 text-2xs text-(--mono-muted)"
              >
                <div
                  class="text-2xs uppercase tracking-wider text-(--mono-muted)"
                >
                  Public advisory submit
                </div>
                {#if unsupportedSignedSubmissionOptions.length > 0}
                  <Notice variant="muted"
                    >Signed non-advisory payload kinds exist on-chain for this
                    domain, but this browser surface still composes advisory
                    kinds only: {unsupportedSignedSubmissionOptions
                      .map((option) =>
                        publicSubmissionPayloadLabel(option.payloadKind),
                      )
                      .join(', ')}</Notice
                  >
                {/if}
                {#if advisorySubmissionOptions.length === 0}
                  <Notice variant="muted"
                    >No signed advisory proposal kinds are available for this
                    domain</Notice
                  >
                {:else}
                  <ProposalSemanticsRows
                    cadenceLabel="Ordinary only"
                    payloadKindLabel={selectedSubmitPayloadKind
                      ? publicSubmissionPayloadLabel(selectedSubmitPayloadKind)
                      : 'Unavailable'}
                    familyLabel={selectedSubmitPayloadKind
                      ? payloadFamilyLabel(selectedSubmitPayloadKind)
                      : 'Unavailable'}
                    executionAuthorityLabel={executionAuthorityLabel(
                      'NonExecutable',
                    )}
                    executionPathLabel={selectedSubmitPayloadKind
                      ? executionPathLabel(selectedSubmitPayloadKind, 'Binary')
                      : 'Unavailable'}
                    openingFeeLabel={selectedSubmissionOption
                      ? openingFeeLabel(
                          'Signed',
                          selectedSubmissionOption.openingFee,
                        )
                      : 'Unavailable'}
                    advisoryScopeLabel={advisoryScopeLabel(
                      selectedSubmitPayloadKind,
                    )}
                    authorizedRuntimeUpgradeLabel={authorizedRuntimeUpgradeLabel()}
                  />
                  <SelectField
                    label="Payload kind"
                    bind:value={selectedSubmitPayloadKind}
                    selectClass="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  >
                    {#each advisorySubmissionOptions as option}
                      <option value={option.payloadKind}
                        >{publicSubmissionPayloadLabel(
                          option.payloadKind,
                        )}</option
                      >
                    {/each}
                  </SelectField>
                  {#if publicSubmissionPurposeNotice(selectedSubmitPayloadKind)}
                    <Notice variant="muted"
                      >{publicSubmissionPurposeNotice(
                        selectedSubmitPayloadKind,
                      ) ?? 'Unavailable'}</Notice
                    >
                  {/if}
                  <NumberInput
                    label="Item id"
                    bind:value={submitItemIdInput}
                    min="1"
                    step="1"
                    placeholder={suggestedSubmitItemId.toString()}
                    class="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <DetailRow
                    label="Suggested item id"
                    value={`#${suggestedSubmitItemId}`}
                    valueClass="text-(--mono-text)"
                  />
                  {#if submitItemIdInput.trim() !== suggestedSubmitItemId.toString()}
                    <Button
                      size="sm"
                      variant="secondary"
                      class="justify-center"
                      onclick={applySuggestedSubmitItemId}
                      >Use suggested item id</Button
                    >
                  {/if}
                  <TextField
                    label={`Summary · ${advisoryPayloadDraft.summaryByteLength}/${GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES} bytes`}
                    bind:value={submitSummaryInput}
                    placeholder={publicSubmissionSummaryPlaceholder(
                      selectedSubmitPayloadKind,
                    )}
                    inputClass="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <TextField
                    label={`Doc CID (optional) · ${advisoryPayloadDraft.docCidByteLength}/${GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES} bytes`}
                    bind:value={submitDocCidInput}
                    placeholder="bafy…"
                    inputClass="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <TextField
                    label="Referenced payload hash (optional)"
                    bind:value={submitReferencedPayloadHashInput}
                    placeholder="0x…64 hex chars"
                    inputClass="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  {#if referencedPayloadSuggestions.length > 0}
                    <SelectField
                      label="Quick fill from visible proposals"
                      onchange={handleReferencedPayloadSuggestionChange}
                      selectClass="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                    >
                      <option value="">Select a visible payload hash</option>
                      {#each referencedPayloadSuggestions as suggestion}
                        <option value={suggestion.payloadHash}
                          >{suggestion.label}</option
                        >
                      {/each}
                    </SelectField>
                  {/if}
                  {#if referencedPayloadSourceLabel}
                    <DetailRow
                      label="Referenced source"
                      value={referencedPayloadSourceLabel}
                      valueClass="text-(--mono-text)"
                    />
                    <Button
                      size="sm"
                      variant="secondary"
                      class="justify-center"
                      onclick={clearReferencedPayloadSuggestion}
                      >Clear referenced payload</Button
                    >
                  {/if}
                  <DetailRow
                    label="Derived payload hash"
                    value={advisoryReview.payloadHashLoading
                      ? 'Computing...'
                      : (advisoryReview.payloadHash ?? 'Unavailable')}
                    valueClass="text-(--mono-text) break-all"
                  />
                  <DetailRow
                    label="Derived payload bytes"
                    value={advisoryPayloadDraft.encoding
                      ? `${advisoryPayloadDraft.encoding.payloadByteLength} bytes`
                      : 'Unavailable'}
                    valueClass="text-(--mono-text)"
                  />
                  <DetailRow
                    label="Summary bytes"
                    value={`${advisoryPayloadDraft.summaryByteLength}/${GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES}`}
                    valueClass="text-(--mono-text)"
                  />
                  <DetailRow
                    label="Doc CID bytes"
                    value={`${advisoryPayloadDraft.docCidByteLength}/${GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES}`}
                    valueClass="text-(--mono-text)"
                  />
                  <DetailRow
                    label="Referenced payload"
                    value={advisoryPayloadDraft.encoding
                      ?.referencedPayloadHash ?? 'None'}
                    valueClass="text-(--mono-text) break-all"
                  />
                  <DetailRow
                    label="Derived preimage status"
                    value={advisoryPayloadHashPreimageStatusLabel(
                      advisoryReview.payloadHashPreimageStatus,
                      advisoryReview.payloadHashPreimageStatusLoading,
                    )}
                    valueClass="text-(--mono-text)"
                  />
                  <DetailRow
                    label="Preimage note cost"
                    value={advisoryPayloadPreimageNoteCostLabel(
                      advisoryReview.payloadPreimageNoteCost,
                      advisoryReview.payloadPreimageNoteCostLoading,
                    )}
                    valueClass="text-(--mono-text)"
                  />
                  {#if advisoryPayloadDraft.encoding}
                    <TextArea
                      label="Derived payload hex"
                      value={advisoryPayloadDraft.encoding.payloadHex}
                      textareaClass="min-h-20 rounded-lg px-2 py-1 text-compact text-(--mono-text)"
                      readonly
                    />
                    <Notice variant="muted"
                      >The chain stores only the payload hash unless these same
                      bounded payload bytes are separately noted as a preimage</Notice
                    >
                    <Notice variant="muted"
                      >`Intent` stays inside the current governance domain,
                      while `L2SignalToL1` records the domain's upward signal
                      toward L1 without dispatching privileged state transitions
                      by itself</Notice
                    >
                    <Notice variant="muted"
                      >Optional preimage noting uses the generic Preimage pallet
                      and the quoted note cost is reserved against the noting
                      account until the preimage is requested or cleared under
                      pallet rules</Notice
                    >
                  {/if}
                  {#if advisoryReview.payloadHashPreimageStatus?.havePreimage}
                    <Notice variant="muted"
                      >These exact payload bytes are already noted on-chain, so
                      the extra preimage step is unnecessary</Notice
                    >
                  {/if}
                  {#if advisoryReview.payloadHashPreimageStatus?.preimageRequested && !advisoryReview.payloadHashPreimageStatus.havePreimage}
                    <Notice variant="muted"
                      >This payload hash is already requested on-chain but the
                      bytes are not yet noted, so supplying the preimage should
                      avoid a new user-held note deposit</Notice
                    >
                  {/if}
                  <ActionReviewCard
                    title="Submission review"
                    submitActionLabel={submitReviewSummaryLine()}
                    submitStatusLabel={submitReviewStatusLabel()}
                    submitOpeningFeeLabel={selectedSubmissionOption
                      ? openingFeeLabel(
                          'Signed',
                          selectedSubmissionOption.openingFee,
                        )
                      : 'Unavailable'}
                    submitResultLabel={submitReviewResultLine()}
                    submitButtonLabel={publicSubmissionButtonLabel(
                      selectedSubmitPayloadKind,
                    )}
                    submitDisabled={submitPublicProposalDisabled}
                    submitOnClick={submitPublicProposal}
                    preimageActionLabel={preimageReviewSummaryLine()}
                    preimageStatusLabel={preimageReviewStatusLabel()}
                    preimageNoteCostLabel={advisoryPayloadPreimageNoteCostLabel(
                      advisoryReview.payloadPreimageNoteCost,
                      advisoryReview.payloadPreimageNoteCostLoading,
                    )}
                    preimageResultLabel={preimageReviewResultLine()}
                    preimageButtonLabel={advisoryReview.payloadHashPreimageStatusLoading ||
                    advisoryReview.payloadPreimageNoteCostLoading
                      ? 'Checking preimage quote'
                      : advisoryReview.payloadHashPreimageStatus?.havePreimage
                        ? 'Preimage already noted'
                        : 'Note advisory preimage'}
                    preimageDisabled={notePreimageDisabled}
                    preimageOnClick={noteProposalPreimage}
                  />
                  {#if !submitItemReady}
                    <Notice variant="muted"
                      >Enter a positive integer item id</Notice
                    >
                  {/if}
                  {#if !advisoryPayloadDraft.summaryValid}
                    <Notice variant="muted"
                      >Enter a non-empty summary within {GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES}
                      UTF-8 bytes</Notice
                    >
                  {/if}
                  {#if !advisoryPayloadDraft.docCidValid}
                    <Notice variant="muted"
                      >Doc CID must stay within {GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES}
                      UTF-8 bytes</Notice
                    >
                  {/if}
                  {#if !advisoryPayloadDraft.referencedPayloadHashValid}
                    <Notice variant="muted"
                      >Referenced payload hash must be blank or `0x` + 64 hex
                      chars</Notice
                    >
                  {/if}
                  {#if submitWriteAvailability.providerStatus !== 'available'}
                    <Notice variant="warn"
                      >{submitWriteAvailability.reason}</Notice
                    >
                  {/if}
                  {#if preimageWriteAvailability.providerStatus !== 'available'}
                    <Notice variant="muted"
                      >{preimageWriteAvailability.reason}</Notice
                    >
                  {/if}
                {/if}
              </div>
              {#if treasurySubmissionOption}
                <div
                  class="rounded-xl bg-(--mono-bg) p-3 grid gap-2 text-2xs text-(--mono-muted)"
                >
                  <div
                    class="text-2xs uppercase tracking-wider text-(--mono-muted)"
                  >
                    Public tactical treasury submit
                  </div>
                  <ProposalSemanticsRows
                    cadenceLabel="Ordinary only"
                    payloadKindLabel={treasurySubmissionOptionLabel}
                    familyLabel={payloadFamilyLabel('L2TreasurySpend')}
                    executionAuthorityLabel={executionAuthorityLabel(
                      'DomainTreasury',
                    )}
                    executionPathLabel={executionPathLabel(
                      'L2TreasurySpend',
                      'Invoice',
                    )}
                    openingFeeLabel={openingFeeLabel(
                      'Signed',
                      treasurySubmissionOption.openingFee,
                    )}
                    settlementLabel={treasurySettlementLabel(
                      'L2TreasurySpend',
                      'Invoice',
                    )}
                    fundingSourceLabel="BLDR treasury"
                  />
                  <Notice variant="muted"
                    >This is the smallest live browser composition slice for the
                    signed tactical invoice path. It always encodes
                    `BldrTreasury` as the funding source and leaves richer
                    treasury-routing policy for later work.</Notice
                  >
                  <NumberInput
                    label="Item id"
                    bind:value={treasurySubmitItemIdInput}
                    min="1"
                    step="1"
                    placeholder={suggestedSubmitItemId.toString()}
                    class="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <DetailRow
                    label="Suggested item id"
                    value={`#${suggestedSubmitItemId}`}
                    valueClass="text-(--mono-text)"
                  />
                  {#if treasurySubmitItemIdInput.trim() !== suggestedSubmitItemId.toString()}
                    <Button
                      size="sm"
                      variant="secondary"
                      class="justify-center"
                      onclick={applySuggestedTreasurySubmitItemId}
                      >Use suggested item id</Button
                    >
                  {/if}
                  <TextField
                    label="Beneficiary"
                    bind:value={treasuryBeneficiaryInput}
                    placeholder="5..."
                    inputClass="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <NumberInput
                    label="Payout asset id"
                    bind:value={treasuryPayoutAssetInput}
                    min="0"
                    step="1"
                    placeholder="268435458"
                    class="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <NumberInput
                    label="Base amount"
                    bind:value={treasuryBaseAmountInput}
                    min="1"
                    step="1"
                    placeholder="1000000000000"
                    class="rounded-lg px-2 py-1 text-xs text-(--mono-text)"
                  />
                  <DetailRow
                    label="Derived payload hash"
                    value={treasuryReview.payloadHashLoading
                      ? 'Computing...'
                      : (treasuryReview.payloadHash ?? 'Unavailable')}
                    valueClass="text-(--mono-text) break-all"
                  />
                  <DetailRow
                    label="Derived payload bytes"
                    value={treasuryPayloadDraft.encoding
                      ? `${treasuryPayloadDraft.encoding.payloadByteLength} bytes`
                      : 'Unavailable'}
                    valueClass="text-(--mono-text)"
                  />
                  <DetailRow
                    label="Derived preimage status"
                    value={advisoryPayloadHashPreimageStatusLabel(
                      treasuryReview.payloadHashPreimageStatus,
                      treasuryReview.payloadHashPreimageStatusLoading,
                    )}
                    valueClass="text-(--mono-text)"
                  />
                  <DetailRow
                    label="Preimage note cost"
                    value={advisoryPayloadPreimageNoteCostLabel(
                      treasuryReview.payloadPreimageNoteCost,
                      treasuryReview.payloadPreimageNoteCostLoading,
                    )}
                    valueClass="text-(--mono-text)"
                  />
                  {#if treasuryPayloadDraft.encoding}
                    <TextArea
                      label="Derived payload hex"
                      value={treasuryPayloadDraft.encoding.payloadHex}
                      textareaClass="min-h-20 rounded-lg px-2 py-1 text-compact text-(--mono-text)"
                      readonly
                    />
                    <Notice variant="muted"
                      >The chain stores only the payload hash unless these same
                      bounded payload bytes are separately noted as a preimage.</Notice
                    >
                  {/if}
                  {#if treasuryReview.payloadHashPreimageStatus?.havePreimage}
                    <Notice variant="muted"
                      >These exact treasury payload bytes are already noted
                      on-chain, so the extra preimage step is unnecessary.</Notice
                    >
                  {/if}
                  {#if treasuryReview.payloadHashPreimageStatus?.preimageRequested && !treasuryReview.payloadHashPreimageStatus.havePreimage}
                    <Notice variant="muted"
                      >This treasury payload hash is already requested on-chain
                      but the bytes are not yet noted, so supplying the preimage
                      should avoid a new user-held note deposit.</Notice
                    >
                  {/if}
                  <ActionReviewCard
                    title="Treasury submission review"
                    submitActionLabel={`Submit treasury invoice · ${executionPathLabel('L2TreasurySpend', 'Invoice')}`}
                    submitStatusLabel={treasurySubmitReviewStatusLabel()}
                    submitOpeningFeeLabel={openingFeeLabel(
                      'Signed',
                      treasurySubmissionOption.openingFee,
                    )}
                    submitResultLabel={treasurySubmitReviewResultLine()}
                    submitButtonLabel="Submit treasury invoice"
                    submitDisabled={treasurySubmitProposalDisabled}
                    submitOnClick={submitTreasuryProposal}
                    preimageActionLabel={treasuryPreimageReviewSummaryLine()}
                    preimageStatusLabel={treasuryPreimageReviewStatusLabel()}
                    preimageNoteCostLabel={advisoryPayloadPreimageNoteCostLabel(
                      treasuryReview.payloadPreimageNoteCost,
                      treasuryReview.payloadPreimageNoteCostLoading,
                    )}
                    preimageResultLabel={treasuryPreimageReviewResultLine()}
                    preimageButtonLabel={treasuryReview.payloadHashPreimageStatusLoading ||
                    treasuryReview.payloadPreimageNoteCostLoading
                      ? 'Checking preimage quote'
                      : treasuryReview.payloadHashPreimageStatus?.havePreimage
                        ? 'Preimage already noted'
                        : 'Note treasury preimage'}
                    preimageDisabled={treasuryNotePreimageDisabled}
                    preimageOnClick={noteTreasuryProposalPreimage}
                  />
                  {#if !treasurySubmitItemReady}
                    <Notice variant="muted"
                      >Enter a positive integer item id.</Notice
                    >
                  {/if}
                  {#if !treasuryPayloadDraft.beneficiaryValid}
                    <Notice variant="muted"
                      >Enter a valid beneficiary address.</Notice
                    >
                  {/if}
                  {#if !treasuryPayloadDraft.payoutAssetValid}
                    <Notice variant="muted"
                      >Payout asset id must be a valid `u32` asset identifier.</Notice
                    >
                  {/if}
                  {#if !treasuryPayloadDraft.baseAmountValid}
                    <Notice variant="muted"
                      >Base amount must be a positive integer that fits inside
                      `u128`.</Notice
                    >
                  {/if}
                </div>
              {/if}
            </div>
          </details>
        {/if}
      </SectionCard>

      <div class="grid content-start gap-3">
        {#if governanceStore.state.error}
          <SectionCard title="Recent finalized proposals">
            <Notice variant="warn">
              Governance query failed before bounded finalized state became
              available.
            </Notice>
          </SectionCard>
        {:else if !governanceChainBlocked}
          <FinalizedProposalsSection
            proposals={governanceStore.state.recentFinalizedProposals}
            authorizedRuntimeUpgrade={governanceStore.state
              .authorizedRuntimeUpgrade}
          />
        {/if}

        <GovernanceArchiveSection
          provider={materializedArchiveProvider}
          archive={materializedArchivePlaceholder}
        />
      </div>
    </div>
  </div>
</Card>

<style>
  @container (min-width: 896px) {
    .governance-grid {
      grid-template-columns: minmax(0, 1.35fr) minmax(16rem, 0.65fr);
      align-items: start;
    }
  }
</style>
