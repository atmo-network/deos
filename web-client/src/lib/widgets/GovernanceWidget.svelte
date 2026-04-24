<script lang="ts">
  import { onMount } from "svelte";

  import type {
      GovernancePayloadHashPreimageStatus,
      GovernancePayloadPreimageNoteCost,
      GovernanceProposalPayloadKind,
      GovernanceVoteKind,
      GovernanceMaterializedArchiveEntry,
  } from "$lib/governance";
  import {
      GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES,
      GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES,
      deriveGovernanceAdvisoryPayloadDraftState,
  } from "$lib/governance/advisory-payload";
  import ActionReviewCard from "$lib/governance/ActionReviewCard.svelte";
  import ProposalSemanticsRows from "$lib/governance/ProposalSemanticsRows.svelte";
  import { governanceStore } from "$lib/governance/index.svelte";
  import { GovernanceUnavailableMaterializedProvider } from "$lib/governance/materialized";
  import { createPayloadReview } from "$lib/governance/payload-review.svelte";
  import { deriveGovernanceTreasuryPayloadDraftState } from "$lib/governance/treasury-payload";
  import { fromClientBoundedProjection, fromMaterialized } from "$lib/shared/read-model";
  import { Badge, Button, Card, DetailRow, Notice, ReadModelBadge, SectionCard } from "$lib/shared/ui";

  type FinalizedProposal = (typeof governanceStore.state.recentFinalizedProposals)[number];
  type FinalizedExecutionDetail = FinalizedProposal["executionDetail"];
  type FinalizedExecutionDetailRow = {
    label: string;
    value: string;
    mono?: boolean;
  };
  type ReferencedPayloadSuggestion = {
    payloadHash: string;
    label: string;
  };

  const refresh = () => governanceStore.refresh();
  const materializedArchiveProvider = new GovernanceUnavailableMaterializedProvider();
  const materializedArchivePlaceholder = fromMaterialized<GovernanceMaterializedArchiveEntry[]>(
    [],
    "archive-api",
    "Governance materialized archive provider",
    "archive",
  );

  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0, height: 0 };
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
      height: rootEl.clientHeight,
    };
  }

  const compactPane = $derived(
    viewport.width > 0 && viewport.width < 430,
  );
  const densePane = $derived(
    viewport.width > 0 && viewport.width < 340,
  );
  const activeProposalsProvenance = fromClientBoundedProjection(
    true,
    "governanceStore.activeProposals <- bounded governance runtime reads",
  ).provenance;
  const recentFinalizedProvenance = $derived(
    governanceStore.state.recentFinalizedProposalsView?.provenance ?? null,
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
  let submitItemIdInput = $state("");
  let submitSummaryInput = $state("");
  let submitDocCidInput = $state("");
  let submitReferencedPayloadHashInput = $state("");
  let selectedSubmitPayloadKind = $state<GovernanceProposalPayloadKind | null>(null);
  let treasurySubmitItemIdInput = $state("");
  let treasuryBeneficiaryInput = $state("");
  let treasuryPayoutAssetInput = $state("");
  let treasuryBaseAmountInput = $state("");

  const advisoryReview = createPayloadReview(() => advisoryPayloadDraft.encoding?.payloadBytes ?? null);
  const treasuryReview = createPayloadReview(() => treasuryPayloadDraft.encoding?.payloadBytes ?? null);

  function profileLabel(profile?: string) {
    switch (profile) {
      case "DecliningDirectStake":
        return "Declining domain stake";
      case "DecliningVetoAsset":
        return "Declining VETO asset";
      case "DecliningNativeStake":
        return "Declining native stake";
      case "FlatUrgentDirectStake":
        return "Urgent flat direct stake";
      default:
        return "Unavailable";
    }
  }

  function isAdvisorySubmissionKind(payloadKind: GovernanceProposalPayloadKind) {
    return payloadKind === "Intent" || payloadKind === "L2SignalToL1";
  }

  const advisorySubmissionOptions = $derived(
    governanceStore.state.submissionOptions.filter((option) =>
      isAdvisorySubmissionKind(option.payloadKind),
    ),
  );
  const treasurySubmissionOption = $derived(
    governanceStore.state.submissionOptions.find(
      (option) => option.payloadKind === "L2TreasurySpend",
    ) ?? null,
  );
  const unsupportedSignedSubmissionOptions = $derived(
    governanceStore.state.submissionOptions.filter(
      (option) =>
        !isAdvisorySubmissionKind(option.payloadKind) &&
        option.payloadKind !== "L2TreasurySpend",
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

  function statusLabel(
    proposal: (typeof governanceStore.state.activeProposals)[number],
  ) {
    const status = proposal.status;
    if (!status) {
      return "Unknown";
    }
    if (status.kind === "PendingEnactment") {
      return `Pending enactment · epoch ${status.enactmentEpoch}`;
    }
    if (status.kind === "Finalized") {
      return status.outcome.kind;
    }
    switch (status.resolution.kind) {
      case "VotingWindowOpen":
        return `Voting · epoch ${status.resolution.currentEpoch}/${status.resolution.maturityEpoch}`;
      case "VetoPassing":
        return "Protection track deciding";
      case "PassingAye":
        return "Aye passing";
      case "PassingAmplify":
        return "Amplify passing";
      case "PassingApprove":
        return "Approve passing";
      case "PassingReduce":
        return "Reduce passing";
      case "PassingNay":
        return "Nay passing";
      case "Confirming":
        return `Confirming · ${status.resolution.confirmStartedEpoch}-${status.resolution.confirmEndEpoch}`;
      case "Rejected":
        return `Rejected · ${rejectionReasonLabel(status.resolution.reason)}`;
    }
  }

  function rejectionReasonLabel(reason?: string | null) {
    switch (reason) {
      case "AdminRejected":
        return "Admin rejected";
      case "NoVotes":
        return "No votes recorded";
      case "VoteTie":
        return "Vote tie";
      case "TurnoutBelowMinimum":
        return "Turnout below minimum";
      case "ApprovalThresholdNotMet":
        return "Approval threshold not met";
      default:
        return "Unavailable";
    }
  }

  function executionFailureReasonLabel(reason?: string | null) {
    switch (reason) {
      case "MissingPreimage":
        return "Missing preimage";
      case "InvalidPreimage":
        return "Invalid preimage";
      case "UnsupportedDomain":
        return "Unsupported governance domain";
      case "UnsupportedCall":
        return "Unsupported runtime call";
      case "UnsupportedTarget":
        return "Unsupported delegated target";
      case "UnsupportedPayloadKind":
        return "Unsupported payload kind";
      case "MissingWinningPrimaryOption":
        return "Missing winning primary option";
      case "DispatchFailed":
        return "Dispatch failed";
      default:
        return "Unavailable";
    }
  }

  function payloadKindLabel(kind?: string) {
    switch (kind) {
      case "L1RootAction":
        return "L1 root action";
      case "L2TreasurySpend":
        return "L2 treasury spend";
      case "L2ParameterChange":
        return "L2 parameter change";
      case "Intent":
        return "Intent";
      case "L2SignalToL1":
        return "L2 signal to L1";
      default:
        return "Unavailable";
    }
  }

  function cadenceModeLabel(cadenceMode?: string | null) {
    switch (cadenceMode) {
      case "Ordinary":
        return "Ordinary";
      case "Fast":
        return "Fast";
      default:
        return "Unavailable";
    }
  }

  function executionAuthorityLabel(authority?: string | null) {
    switch (authority) {
      case "Root":
        return "Root";
      case "DomainTreasury":
        return "Domain treasury";
      case "DomainParameters":
        return "Domain parameters";
      case "NonExecutable":
        return "Advisory only";
      default:
        return "Unavailable";
    }
  }

  function submissionAuthorityLabel(authority?: string | null) {
    switch (authority) {
      case "Signed":
        return "Signed public path";
      case "AdminOnly":
        return "Admin only";
      default:
        return "Unavailable";
    }
  }

  function openingFeeLabel(
    authority?: string | null,
    openingFee?: bigint | null,
  ) {
    if (authority !== "Signed") {
      return "No public fee";
    }
    if (openingFee == null) {
      return "Unavailable";
    }
    return `${openingFee.toLocaleString()} native units`;
  }

  function authorizedRuntimeUpgradeLabel() {
    const authorization = governanceStore.state.authorizedRuntimeUpgrade;
    if (!authorization) {
      return "None";
    }
    return authorization.checkVersion
      ? `${authorization.codeHash} · version checked`
      : `${authorization.codeHash} · no version check`;
  }

  function payloadFamilyLabel(kind?: string | null) {
    switch (kind) {
      case "L1RootAction":
        return "Executable · Strategic runtime-upgrade authorization";
      case "L2TreasurySpend":
        return "Executable · Tactical invoice treasury spend";
      case "L2ParameterChange":
        return "Executable · Delegated router-only parameter change";
      case "Intent":
        return "Advisory · Domain-local intent";
      case "L2SignalToL1":
        return "Advisory · Upward tactical signal";
      default:
        return "Unavailable";
    }
  }

  function advisoryScopeLabel(kind?: string | null) {
    switch (kind) {
      case "Intent":
        return "Same governance domain only";
      case "L2SignalToL1":
        return "Upward signal toward L1 only";
      default:
        return null;
    }
  }

  function primaryTrackFamilyLabel(primaryTrackFamily?: string | null) {
    switch (primaryTrackFamily) {
      case "Binary":
        return "Binary Aye / Nay";
      case "Invoice":
        return "Invoice Amplify / Approve / Reduce / Nay";
      default:
        return "Unavailable";
    }
  }

  function primaryTrackLeaderLabel(
    primaryTrackTally?: (typeof governanceStore.state.activeProposals)[number]["primaryTrackTally"] | null,
  ) {
    if (!primaryTrackTally) {
      return "Unavailable";
    }
    if (primaryTrackTally.kind === "Binary") {
      return primaryTrackTally.leadingOption ?? "Tie / none";
    }
    if (!primaryTrackTally.leadingPositiveOption) {
      return "None";
    }
    return `${primaryTrackTally.leadingPositiveOption} · ${primaryTrackTally.leadingPositiveWeight.toLocaleString()}`;
  }

  function primaryTrackPositiveWeightLabel(
    primaryTrackTally?: (typeof governanceStore.state.activeProposals)[number]["primaryTrackTally"] | null,
  ) {
    if (!primaryTrackTally) {
      return "Unavailable";
    }
    if (primaryTrackTally.kind === "Binary") {
      return primaryTrackTally.ayeWeight.toLocaleString();
    }
    return primaryTrackTally.positiveWeight.toLocaleString();
  }

  function payloadAvailabilityLabel(
    payloadKind?: string | null,
    availability?: (typeof governanceStore.state.activeProposals)[number]["payloadAvailability"] | null,
  ) {
    if (!availability) {
      return "Unavailable";
    }
    if (availability.havePreimage) {
      return "Ready";
    }
    if (availability.preimageRequested) {
      return "Requested";
    }
    if (payloadKind === "Intent" || payloadKind === "L2SignalToL1") {
      return "Hash only";
    }
    return "Missing";
  }

  function advisoryPayloadHashPreimageStatusLabel(
    status: GovernancePayloadHashPreimageStatus | null,
    loading: boolean,
  ) {
    if (loading) {
      return "Checking";
    }
    if (!status) {
      return "Unavailable";
    }
    if (status.havePreimage) {
      return status.byteLength == null ? "Already noted" : `Already noted · ${status.byteLength} bytes`;
    }
    if (status.preimageRequested) {
      return "Requested · not yet noted";
    }
    return "Not noted";
  }

  function advisoryPayloadPreimageNoteCostLabel(
    noteCost: GovernancePayloadPreimageNoteCost | null,
    loading: boolean,
  ) {
    if (loading) {
      return "Checking";
    }
    if (noteCost == null) {
      return "Unavailable";
    }
    return `${noteCost.toLocaleString()} native units`;
  }

  function executionPathLabel(
    payloadKind?: string | null,
    primaryTrackFamily?: string | null,
  ) {
    switch (payloadKind) {
      case "L1RootAction":
        return "Authorizes `System.authorize_upgrade { code_hash }` only";
      case "L2TreasurySpend":
        return primaryTrackFamily === "Invoice"
          ? "Executes BLDR-treasury scalar invoice payout from retained winning option"
          : "Executes bounded tactical treasury payout";
      case "L2ParameterChange":
        return "Executes router fee or tracked-asset change only";
      case "Intent":
        return "No dispatch, same-domain advisory only";
      case "L2SignalToL1":
        return "No dispatch, upward tactical signal only";
      default:
        return "Unavailable";
    }
  }

  function treasurySettlementLabel(
    payloadKind?: string | null,
    primaryTrackFamily?: string | null,
  ) {
    if (payloadKind !== "L2TreasurySpend") {
      return null;
    }
    return primaryTrackFamily === "Invoice"
      ? "Scalar invoice transfer"
      : "Direct transfer only";
  }

  function parameterChangeSurfaceLabel(surface?: string | null) {
    switch (surface) {
      case "RouterFee":
        return "Axial Router fee";
      case "TrackedAsset":
        return "Axial Router tracked asset";
      default:
        return "Unavailable";
    }
  }

  function treasurySpendScalarLabel(scalar?: string | null) {
    switch (scalar) {
      case "Amplify":
        return "Amplify";
      case "Approve":
        return "Approve";
      case "Reduce":
        return "Reduce";
      default:
        return "Unavailable";
    }
  }

  function treasurySpendSettlementKindLabel(kind?: string | null) {
    switch (kind) {
      case "InvoiceScalarTransfer":
        return "Scalar invoice transfer";
      case "DirectTransfer":
        return "Direct transfer";
      default:
        return "Unavailable";
    }
  }

  function accountIdentifierLabel(accountId?: string | null) {
    if (!accountId) {
      return "Unavailable";
    }
    return `Account ${accountId}`;
  }

  function assetIdentifierLabel(assetId?: bigint | number | string | null) {
    if (assetId == null) {
      return "Unavailable";
    }
    return `Asset #${assetId.toString()}`;
  }

  function finalizedExecutionDetailRows(
    detail: FinalizedExecutionDetail,
  ): FinalizedExecutionDetailRow[] {
    if (!detail) {
      return [];
    }
    switch (detail.kind) {
      case "Executed": {
        const rows: FinalizedExecutionDetailRow[] = [
          {
            label: "Execution receipt",
            value: finalizedExecutionDetailLabel(detail) ?? `${payloadKindLabel(detail.payloadKind)} executed`,
          },
          {
            label: "Executed epoch",
            value: detail.executedEpoch.toString(),
          },
        ];
        switch (detail.detail.kind) {
          case "Generic":
            return rows;
          case "RuntimeUpgradeAuthorized":
            rows.push({
              label: "Authorized code hash",
              value: detail.detail.codeHash,
              mono: true,
            });
            return rows;
          case "ParameterChangeExecuted":
            rows.push({
              label: "Parameter surface",
              value: parameterChangeSurfaceLabel(detail.detail.surface),
            });
            return rows;
          case "TreasurySpendExecuted":
            rows.push(
              {
                label: "Funding source",
                value: accountIdentifierLabel(detail.detail.fundingSource),
                mono: true,
              },
              {
                label: "Beneficiary",
                value: accountIdentifierLabel(detail.detail.beneficiary),
                mono: true,
              },
              {
                label: "Payout asset",
                value: assetIdentifierLabel(detail.detail.payoutAsset),
                mono: true,
              },
              {
                label: "Base amount",
                value: detail.detail.baseAmount.toLocaleString(),
              },
              {
                label: "Winning scalar",
                value: treasurySpendScalarLabel(detail.detail.scalar),
              },
              {
                label: "Final amount",
                value: detail.detail.finalAmount.toLocaleString(),
              },
              {
                label: "Settlement kind",
                value: treasurySpendSettlementKindLabel(detail.detail.settlementKind),
              },
            );
            return rows;
        }
      }
      case "ExecutionFailed":
        return [
          {
            label: "Execution receipt",
            value: finalizedExecutionDetailLabel(detail) ?? `Failed · ${executionFailureReasonLabel(detail.reason)}`,
          },
          {
            label: "Failed epoch",
            value: detail.failedEpoch.toString(),
          },
          {
            label: "Failure reason",
            value: executionFailureReasonLabel(detail.reason),
          },
        ];
      case "AdvisoryFinalized":
        return [
          {
            label: "Execution receipt",
            value: finalizedExecutionDetailLabel(detail) ?? `Advisory · ${payloadKindLabel(detail.payloadKind)}`,
          },
          {
            label: "Finalized epoch",
            value: detail.finalizedEpoch.toString(),
          },
        ];
    }
  }

  function primaryTrackPowerLabel(
    proposal: (typeof governanceStore.state.activeProposals)[number],
  ) {
    if (proposal.primaryTrackFamily === "Invoice") {
      return profileLabel(
        proposal.votePowerProfiles.approve ?? proposal.votePowerProfiles.nay,
      );
    }
    return profileLabel(proposal.votePowerProfiles.aye);
  }

  function voteKindLabel(voteKind?: GovernanceVoteKind | null) {
    switch (voteKind) {
      case "aye":
        return "Aye";
      case "nay":
        return "Nay";
      case "amplify":
        return "Amplify";
      case "approve":
        return "Approve";
      case "reduce":
        return "Reduce";
      case "veto":
        return "Veto";
      case "pass":
        return "Pass";
      default:
        return "Unavailable";
    }
  }

  function governanceLockLabel(epoch?: number | null) {
    return epoch == null ? "None" : `Until epoch ${epoch}`;
  }

  function frozenBallotLabel(
    ballot?: NonNullable<(typeof governanceStore.state.activeProposals)[number]["accountPowerView"]>["frozenOrdinaryBallot"] | null,
  ) {
    if (!ballot) {
      return "No frozen ballot";
    }
    return `${voteKindLabel(ballot.voteKind)} · ${ballot.weight.toLocaleString()} at epoch ${ballot.voteEpoch}`;
  }

  function voteButtons(
    proposal: (typeof governanceStore.state.activeProposals)[number],
  ): Array<{ voteKind: GovernanceVoteKind; label: string }> {
    const primaryButtons: Array<{ voteKind: GovernanceVoteKind; label: string }> =
      proposal.primaryTrackFamily === "Invoice"
        ? [
            { voteKind: "amplify", label: "Amplify" },
            { voteKind: "approve", label: "Approve" },
            { voteKind: "reduce", label: "Reduce" },
            { voteKind: "nay", label: "Nay" },
          ]
        : [
            { voteKind: "aye", label: "Aye" },
            { voteKind: "nay", label: "Nay" },
          ];
    return [
      ...primaryButtons,
      { voteKind: "veto", label: "Veto" },
      { voteKind: "pass", label: "Pass" },
    ];
  }

  function urgentEligibilityLabel(
    urgentEligibility?: boolean | null,
    payloadKind?: string | null,
  ) {
    if (urgentEligibility == null) {
      return "Unavailable";
    }
    if (!urgentEligibility) {
      return "Disabled on current line";
    }
    if (payloadKind === "L1RootAction") {
      return "Unanimous PASS runtime-upgrade fast path";
    }
    return "Expeditable";
  }

  function urgentExecutionContractLabel(payloadKind?: string | null) {
    if (payloadKind === "L1RootAction") {
      return "Unanimous raw VETO PASS immediately authorizes the runtime upgrade without waiting for a separate primary ballot";
    }
    return null;
  }

  function timingWindowLabel(
    openEpoch?: number | null,
    closeEpoch?: number | null,
  ) {
    if (openEpoch == null || closeEpoch == null) {
      return "Unavailable";
    }
    return `${openEpoch}-${closeEpoch}`;
  }

  function pendingEnactmentLabel(
    pendingEnactmentEpoch?: number | null,
  ) {
    if (pendingEnactmentEpoch == null) {
      return "None";
    }
    return `Epoch ${pendingEnactmentEpoch}`;
  }

  function primaryTrackOptionLabel(option?: string | null) {
    switch (option) {
      case "Aye":
        return "Aye";
      case "Nay":
        return "Nay";
      case "Amplify":
        return "Amplify";
      case "Approve":
        return "Approve";
      case "Reduce":
        return "Reduce";
      default:
        return "Unavailable";
    }
  }

  function retainedWinningPrimaryOptionLabel(
    proposal: FinalizedProposal,
  ) {
    if (proposal.winningPrimaryOption == null) {
      return null;
    }
    return primaryTrackOptionLabel(proposal.winningPrimaryOption);
  }

  function executedStateLabel(detail: FinalizedExecutionDetail) {
    if (!detail || detail.kind !== "Executed") {
      return "Executed";
    }
    switch (detail.detail.kind) {
      case "Generic":
        return `${payloadKindLabel(detail.payloadKind)} executed`;
      case "RuntimeUpgradeAuthorized":
        return "Executed · Runtime upgrade authorized";
      case "ParameterChangeExecuted":
        return `Executed · Parameter change · ${parameterChangeSurfaceLabel(detail.detail.surface)}`;
      case "TreasurySpendExecuted":
        return `Executed · Treasury payout · ${treasurySpendScalarLabel(detail.detail.scalar)}`;
    }
  }

  function finalizedExecutionStateLabel(
    proposal: FinalizedProposal,
  ) {
    switch (proposal.outcome.kind) {
      case "Resolved":
        return "Approved only, no retained execution receipt";
      case "Rejected":
        return "Not executed · Rejected before enactment";
      case "VetoCancelled":
        return "Not executed · Blocked by protection";
      case "Enacted":
        return executedStateLabel(proposal.executionDetail);
      case "ExecutionFailed":
        return proposal.executionDetail?.kind === "ExecutionFailed"
          ? `Execution failed · ${executionFailureReasonLabel(proposal.executionDetail.reason)}`
          : "Execution attempted and failed";
      case "AdvisoryFinalized":
        return proposal.executionDetail?.kind === "AdvisoryFinalized"
          ? `Advisory finalization · ${payloadKindLabel(proposal.executionDetail.payloadKind)}`
          : "Advisory finalization only";
    }
  }

  function hasDecliningPower(
    proposal: (typeof governanceStore.state.activeProposals)[number],
  ) {
    return Object.values(proposal.votePowerProfiles).some((profile) =>
      profile?.startsWith("Declining"),
    );
  }

  function finalizedOutcomeLabel(
    proposal: FinalizedProposal,
  ) {
    const outcome = proposal.outcome;
    switch (outcome.kind) {
      case "Resolved":
        return `Resolved · epoch ${outcome.epoch}`;
      case "Rejected":
        return `Rejected · ${rejectionReasonLabel(outcome.reason)}`;
      case "VetoCancelled":
        return `Veto cancelled · epoch ${outcome.epoch}`;
      case "Enacted":
        return `Enacted · epoch ${outcome.executedEpoch}`;
      case "ExecutionFailed":
        return proposal.executionDetail?.kind === "ExecutionFailed"
          ? `Execution failed · ${executionFailureReasonLabel(proposal.executionDetail.reason)} · epoch ${outcome.failedEpoch}`
          : `Execution failed · epoch ${outcome.failedEpoch}`;
      case "AdvisoryFinalized":
        return proposal.executionDetail?.kind === "AdvisoryFinalized"
          ? `Advisory finalized · ${payloadKindLabel(proposal.executionDetail.payloadKind)} · epoch ${outcome.finalizedEpoch}`
          : `Advisory finalized · epoch ${outcome.finalizedEpoch}`;
    }
  }

  function publicSubmissionPayloadLabel(payloadKind: GovernanceProposalPayloadKind) {
    switch (payloadKind) {
      case "Intent":
        return "Intent";
      case "L2SignalToL1":
        return "L2 signal to L1";
      case "L1RootAction":
        return "L1 root action";
      case "L2TreasurySpend":
        return "L2 treasury spend";
      case "L2ParameterChange":
        return "L2 parameter change";
    }
  }

  function publicSubmissionSummaryPlaceholder(
    payloadKind: GovernanceProposalPayloadKind | null,
  ) {
    switch (payloadKind) {
      case "Intent":
        return "State the domain-local intent";
      case "L2SignalToL1":
        return "State the tactical signal that should be visible to L1";
      default:
        return "Non-empty advisory summary";
    }
  }

  function publicSubmissionButtonLabel(
    payloadKind: GovernanceProposalPayloadKind | null,
  ) {
    switch (payloadKind) {
      case "Intent":
        return "Submit intent";
      case "L2SignalToL1":
        return "Submit L2 signal";
      default:
        return "Submit advisory proposal";
    }
  }

  function publicSubmissionPurposeNotice(
    payloadKind: GovernanceProposalPayloadKind | null,
  ) {
    switch (payloadKind) {
      case "Intent":
        return "Use Intent when the current governance domain wants to record its own non-executable position without escalating authority toward another domain";
      case "L2SignalToL1":
        return "Use L2 signal to L1 when the current tactical domain wants to record an upward non-executable signal for L1 rather than mutate strategic control surfaces directly";
      default:
        return null;
    }
  }

  function parsedSubmitItemId() {
    const trimmed = submitItemIdInput.trim();
    if (trimmed.length === 0) {
      return suggestedSubmitItemId;
    }
    if (!/^\d+$/.test(trimmed)) {
      return null;
    }
    const parsed = Number.parseInt(trimmed, 10);
    return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
  }

  function applyReferencedPayloadSuggestion(payloadHash: string) {
    submitReferencedPayloadHashInput = payloadHash;
  }

  function clearReferencedPayloadSuggestion() {
    submitReferencedPayloadHashInput = "";
  }

  function handleReferencedPayloadSuggestionChange(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLSelectElement) || target.value.length === 0) {
      return;
    }
    applyReferencedPayloadSuggestion(target.value);
    target.value = "";
  }

  const selectedSubmissionOption = $derived(
    advisorySubmissionOptions.find(
      (option) => option.payloadKind === selectedSubmitPayloadKind,
    ) ?? null,
  );
  const treasurySubmissionOptionLabel = $derived(
    treasurySubmissionOption
      ? publicSubmissionPayloadLabel(treasurySubmissionOption.payloadKind)
      : "Unavailable",
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
      (suggestion) => suggestion.payloadHash === submitReferencedPayloadHashInput.trim(),
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
      return "Manual / not in visible bounded set";
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
    advisoryPayloadDraft.encoding !== null && advisoryReview.payloadHash !== null,
  );
  const submitItemReady = $derived(parsedSubmitItemId() !== null);
  const treasurySubmitPayloadReady = $derived(
    treasuryPayloadDraft.encoding !== null && treasuryReview.payloadHash !== null,
  );
  const treasurySubmitItemReady = $derived(parsedTreasurySubmitItemId() !== null);
  const submitPublicProposalDisabled = $derived(
    submitWriteAvailability.providerStatus !== "available" ||
    selectedSubmitPayloadKind == null ||
    !submitItemReady ||
    !submitPayloadReady ||
    advisoryReview.payloadHashLoading,
  );
  const notePreimageDisabled = $derived(
    preimageWriteAvailability.providerStatus !== "available" ||
    advisoryPayloadDraft.encoding == null ||
    advisoryReview.payloadHashLoading ||
    advisoryReview.payloadHashPreimageStatusLoading ||
    advisoryReview.payloadPreimageNoteCostLoading ||
    advisoryReview.payloadHashPreimageStatus?.havePreimage === true,
  );
  const treasurySubmitProposalDisabled = $derived(
    submitWriteAvailability.providerStatus !== "available" ||
    treasurySubmissionOption == null ||
    !treasurySubmitItemReady ||
    !treasurySubmitPayloadReady ||
    treasuryReview.payloadHashLoading,
  );
  const treasuryNotePreimageDisabled = $derived(
    preimageWriteAvailability.providerStatus !== "available" ||
    treasuryPayloadDraft.encoding == null ||
    treasuryReview.payloadHashLoading ||
    treasuryReview.payloadHashPreimageStatusLoading ||
    treasuryReview.payloadPreimageNoteCostLoading ||
    treasuryReview.payloadHashPreimageStatus?.havePreimage === true,
  );

  function submitReviewStatusLabel() {
    if (selectedSubmitPayloadKind == null) {
      return "Pick a signed advisory payload kind";
    }
    if (!submitItemReady) {
      return "Enter a valid positive item id";
    }
    if (!advisoryPayloadDraft.summaryValid) {
      return "Fix the advisory summary";
    }
    if (!advisoryPayloadDraft.docCidValid) {
      return "Fix the doc CID length";
    }
    if (!advisoryPayloadDraft.referencedPayloadHashValid) {
      return "Fix the referenced payload hash";
    }
    if (advisoryReview.payloadHashLoading) {
      return "Computing the advisory payload hash";
    }
    if (submitWriteAvailability.providerStatus !== "available") {
      return submitWriteAvailability.reason;
    }
    return "Ready to submit signed advisory proposal";
  }

  function preimageReviewStatusLabel() {
    if (advisoryReview.payloadHashLoading) {
      return "Computing the advisory payload hash";
    }
    if (advisoryReview.payloadHashPreimageStatusLoading || advisoryReview.payloadPreimageNoteCostLoading) {
      return "Checking preimage status and note cost";
    }
    if (advisoryPayloadDraft.encoding == null || advisoryReview.payloadHash == null) {
      return "Finish the advisory draft first";
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return "These exact bytes are already noted on-chain";
    }
    if (preimageWriteAvailability.providerStatus !== "available") {
      return preimageWriteAvailability.reason;
    }
    if (advisoryReview.payloadHashPreimageStatus?.preimageRequested) {
      return "Optional separate note path can satisfy an existing request";
    }
    return "Optional separate preimage note path is available";
  }

  function submitReviewSummaryLine() {
    const payloadKind = selectedSubmitPayloadKind;
    if (payloadKind == null) {
      return "Unavailable";
    }
    return `${publicSubmissionButtonLabel(payloadKind)} · ${executionPathLabel(payloadKind, "Binary")}`;
  }

  function preimageReviewSummaryLine() {
    if (advisoryPayloadDraft.encoding == null) {
      return "Unavailable";
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return "No extra preimage note is needed";
    }
    return `Optional Preimage.note_preimage on ${advisoryPayloadDraft.encoding.payloadByteLength} bytes`;
  }

  function submitReviewResultLine() {
    const itemId = parsedSubmitItemId();
    if (itemId == null || advisoryReview.payloadHash == null) {
      return "Unavailable";
    }
    if (advisoryReview.payloadHashPreimageStatus?.havePreimage) {
      return `Creates proposal #${itemId} with payload hash ${advisoryReview.payloadHash} and already-noted bytes`;
    }
    return `Creates proposal #${itemId} with payload hash ${advisoryReview.payloadHash} only`;
  }

  function preimageReviewResultLine() {
    if (advisoryReview.payloadHash == null) {
      return "Unavailable";
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
    const trimmed = treasurySubmitItemIdInput.trim();
    if (trimmed.length === 0) {
      return suggestedSubmitItemId;
    }
    if (!/^\d+$/.test(trimmed)) {
      return null;
    }
    const parsed = Number.parseInt(trimmed, 10);
    return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
  }

  function treasurySubmitReviewStatusLabel() {
    if (treasurySubmissionOption == null) {
      return "Signed tactical treasury submission is unavailable";
    }
    if (!treasurySubmitItemReady) {
      return "Enter a valid positive item id";
    }
    if (!treasuryPayloadDraft.beneficiaryValid) {
      return "Enter a valid beneficiary address";
    }
    if (!treasuryPayloadDraft.payoutAssetValid) {
      return "Enter a valid payout asset id";
    }
    if (!treasuryPayloadDraft.baseAmountValid) {
      return "Enter a valid positive base amount";
    }
    if (treasuryReview.payloadHashLoading) {
      return "Computing the treasury payload hash";
    }
    if (submitWriteAvailability.providerStatus !== "available") {
      return submitWriteAvailability.reason;
    }
    return "Ready to submit tactical treasury invoice proposal";
  }

  function treasuryPreimageReviewStatusLabel() {
    if (treasuryReview.payloadHashLoading) {
      return "Computing the treasury payload hash";
    }
    if (treasuryReview.payloadHashPreimageStatusLoading || treasuryReview.payloadPreimageNoteCostLoading) {
      return "Checking preimage status and note cost";
    }
    if (treasuryPayloadDraft.encoding == null || treasuryReview.payloadHash == null) {
      return "Finish the treasury payload draft first";
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return "These exact bytes are already noted on-chain";
    }
    if (preimageWriteAvailability.providerStatus !== "available") {
      return preimageWriteAvailability.reason;
    }
    if (treasuryReview.payloadHashPreimageStatus?.preimageRequested) {
      return "Optional separate note path can satisfy an existing request";
    }
    return "Optional separate preimage note path is available";
  }

  function treasurySubmitReviewResultLine() {
    const itemId = parsedTreasurySubmitItemId();
    if (itemId == null || treasuryReview.payloadHash == null) {
      return "Unavailable";
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return `Creates proposal #${itemId} with payload hash ${treasuryReview.payloadHash} and already-noted bytes`;
    }
    return `Creates proposal #${itemId} with payload hash ${treasuryReview.payloadHash} only`;
  }

  function treasuryPreimageReviewResultLine() {
    if (treasuryReview.payloadHash == null) {
      return "Unavailable";
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
      return "Unavailable";
    }
    if (treasuryReview.payloadHashPreimageStatus?.havePreimage) {
      return "No extra preimage note is needed";
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
      cadenceMode: "Ordinary",
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
    if (treasurySubmissionOption == null || itemId == null || treasuryReview.payloadHash == null) {
      return;
    }
    await governanceStore.submitProposal({
      itemId,
      cadenceMode: "Ordinary",
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

  function finalizedExecutionDetailLabel(
    detail: FinalizedExecutionDetail,
  ) {
    if (!detail) {
      return null;
    }
    switch (detail.kind) {
      case "Executed":
        switch (detail.detail.kind) {
          case "Generic":
            return `${payloadKindLabel(detail.payloadKind)} executed`;
          case "RuntimeUpgradeAuthorized":
            return `Runtime upgrade authorized · ${detail.detail.codeHash}`;
          case "ParameterChangeExecuted":
            return `Parameter change · ${parameterChangeSurfaceLabel(detail.detail.surface)}`;
          case "TreasurySpendExecuted":
            return `Treasury payout · ${treasurySpendScalarLabel(detail.detail.scalar)} · ${detail.detail.baseAmount.toLocaleString()} → ${detail.detail.finalAmount.toLocaleString()} · ${accountIdentifierLabel(detail.detail.fundingSource)}`;
        }
      case "ExecutionFailed":
        return `Failed · ${executionFailureReasonLabel(detail.reason)}`;
      case "AdvisoryFinalized":
        return `Advisory · ${payloadKindLabel(detail.payloadKind)}`;
    }
  }

  function runtimeUpgradeOperatorPathLabel() {
    return "authorized-upgrade-local.sh check → authorized-upgrade-local.sh apply --submit";
  }

  function finalizedRuntimeUpgradeApplicationLabel(
    proposal: FinalizedProposal,
  ) {
    const detail = proposal.executionDetail;
    if (!detail || detail.kind !== "Executed" || detail.detail.kind !== "RuntimeUpgradeAuthorized") {
      return null;
    }
    const authorization = governanceStore.state.authorizedRuntimeUpgrade;
    if (!authorization) {
      return "No pending authorized upgrade";
    }
    return authorization.codeHash === detail.detail.codeHash
      ? "Pending authorized code relay"
      : "Different authorization is pending";
  }

  onMount(() => {
    void governanceStore.init();
    syncViewport();
    if (!rootEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<Card class="min-h-full p-2">
  <div bind:this={rootEl} class="@container grid gap-3 pb-2">
    <div class={[
      "grid gap-3",
      !compactPane && "@xl:grid-cols-[minmax(0,1.35fr)_minmax(15rem,0.65fr)]",
    ]}>
      <SectionCard title="Active proposals" subtitle="Vote here, receipts appear in Log">
        {#snippet actions()}
          <ReadModelBadge provenance={activeProposalsProvenance} tone="subtle" />
          <Badge variant={voteWriteAvailability.providerStatus === "available" ? "tmc" : "info"}>
            {voteWriteAvailability.providerStatus === "available" ? "Vote ready" : "Vote unavailable"}
          </Badge>
          <Badge variant="info">{governanceStore.state.activeProposals.length} open</Badge>
        {/snippet}
        {#if voteWriteAvailability.providerStatus !== "available"}
          <Notice variant="warn">{voteWriteAvailability.reason}</Notice>
        {/if}
        {#if governanceStore.state.writeError}
          <Notice variant="warn">{governanceStore.state.writeError}</Notice>
        {/if}
        {#if governanceStore.state.error && governanceStore.state.error !== governanceStore.state.writeError}
          <Notice variant="warn">{governanceStore.state.error}</Notice>
        {/if}
        {#if governanceStore.state.authorizedRuntimeUpgrade}
          <Notice variant="warn">A governance-authorized runtime upgrade is pending authorized code relay · {authorizedRuntimeUpgradeLabel()}</Notice>
          <Notice variant="muted">System.apply_authorized_upgrade with matching code bytes is a separate system-level relay step that any origin may submit after governance authorization, and this browser intentionally does not expose that live write path</Notice>
          <Notice variant="muted">Any operator may relay the matching code bytes after governance authorization, but this step remains ministerial rather than a second governance decision</Notice>
          <DetailRow label="Operator path" value={runtimeUpgradeOperatorPathLabel()} valueClass="text-(--mono-text) break-all" />
        {/if}
        <div class="rounded-xl border bg-white p-3 grid gap-2 text-[10px] text-(--mono-muted)">
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Public advisory submit</div>
          {#if unsupportedSignedSubmissionOptions.length > 0}
            <Notice variant="muted">Signed non-advisory payload kinds exist on-chain for this domain, but this browser surface still composes advisory kinds only: {unsupportedSignedSubmissionOptions.map((option) => publicSubmissionPayloadLabel(option.payloadKind)).join(", ")}</Notice>
          {/if}
          {#if advisorySubmissionOptions.length === 0}
            <Notice variant="muted">No signed advisory proposal kinds are available for this domain</Notice>
          {:else}
            <ProposalSemanticsRows
              cadenceLabel="Ordinary only"
              payloadKindLabel={selectedSubmitPayloadKind ? publicSubmissionPayloadLabel(selectedSubmitPayloadKind) : "Unavailable"}
              familyLabel={selectedSubmitPayloadKind ? payloadFamilyLabel(selectedSubmitPayloadKind) : "Unavailable"}
              executionAuthorityLabel={executionAuthorityLabel("NonExecutable")}
              executionPathLabel={selectedSubmitPayloadKind ? executionPathLabel(selectedSubmitPayloadKind, "Binary") : "Unavailable"}
              openingFeeLabel={selectedSubmissionOption ? openingFeeLabel("Signed", selectedSubmissionOption.openingFee) : "Unavailable"}
              advisoryScopeLabel={advisoryScopeLabel(selectedSubmitPayloadKind)}
              authorizedRuntimeUpgradeLabel={authorizedRuntimeUpgradeLabel()}
            />
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Payload kind</span>
              <select class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={selectedSubmitPayloadKind}>
                {#each advisorySubmissionOptions as option}
                  <option value={option.payloadKind}>{publicSubmissionPayloadLabel(option.payloadKind)}</option>
                {/each}
              </select>
            </label>
            {#if publicSubmissionPurposeNotice(selectedSubmitPayloadKind)}
              <Notice variant="muted">{publicSubmissionPurposeNotice(selectedSubmitPayloadKind) ?? "Unavailable"}</Notice>
            {/if}
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Item id</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={submitItemIdInput} inputmode="numeric" placeholder={suggestedSubmitItemId.toString()} />
            </label>
            <DetailRow label="Suggested item id" value={`#${suggestedSubmitItemId}`} valueClass="text-(--mono-text)" />
            {#if submitItemIdInput.trim() !== suggestedSubmitItemId.toString()}
              <Button size="sm" variant="secondary" class="justify-center" onclick={applySuggestedSubmitItemId}>Use suggested item id</Button>
            {/if}
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Summary · {advisoryPayloadDraft.summaryByteLength}/{GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES} bytes</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={submitSummaryInput} placeholder={publicSubmissionSummaryPlaceholder(selectedSubmitPayloadKind)} />
            </label>
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Doc CID (optional) · {advisoryPayloadDraft.docCidByteLength}/{GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES} bytes</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={submitDocCidInput} placeholder="bafy…" />
            </label>
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Referenced payload hash (optional)</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={submitReferencedPayloadHashInput} placeholder="0x…64 hex chars" />
            </label>
            {#if referencedPayloadSuggestions.length > 0}
              <label class="grid gap-1">
                <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Quick fill from visible proposals</span>
                <select class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" onchange={handleReferencedPayloadSuggestionChange}>
                  <option value="">Select a visible payload hash</option>
                  {#each referencedPayloadSuggestions as suggestion}
                    <option value={suggestion.payloadHash}>{suggestion.label}</option>
                  {/each}
                </select>
              </label>
            {/if}
            {#if referencedPayloadSourceLabel}
              <DetailRow label="Referenced source" value={referencedPayloadSourceLabel} valueClass="text-(--mono-text)" />
              <Button size="sm" variant="secondary" class="justify-center" onclick={clearReferencedPayloadSuggestion}>Clear referenced payload</Button>
            {/if}
            <DetailRow label="Derived payload hash" value={advisoryReview.payloadHashLoading ? "Computing..." : (advisoryReview.payloadHash ?? "Unavailable")} valueClass="text-(--mono-text) break-all" />
            <DetailRow label="Derived payload bytes" value={advisoryPayloadDraft.encoding ? `${advisoryPayloadDraft.encoding.payloadByteLength} bytes` : "Unavailable"} valueClass="text-(--mono-text)" />
            <DetailRow label="Summary bytes" value={`${advisoryPayloadDraft.summaryByteLength}/${GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES}`} valueClass="text-(--mono-text)" />
            <DetailRow label="Doc CID bytes" value={`${advisoryPayloadDraft.docCidByteLength}/${GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES}`} valueClass="text-(--mono-text)" />
            <DetailRow label="Referenced payload" value={advisoryPayloadDraft.encoding?.referencedPayloadHash ?? "None"} valueClass="text-(--mono-text) break-all" />
            <DetailRow label="Derived preimage status" value={advisoryPayloadHashPreimageStatusLabel(advisoryReview.payloadHashPreimageStatus, advisoryReview.payloadHashPreimageStatusLoading)} valueClass="text-(--mono-text)" />
            <DetailRow label="Preimage note cost" value={advisoryPayloadPreimageNoteCostLabel(advisoryReview.payloadPreimageNoteCost, advisoryReview.payloadPreimageNoteCostLoading)} valueClass="text-(--mono-text)" />
            {#if advisoryPayloadDraft.encoding}
              <label class="grid gap-1">
                <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Derived payload hex</span>
                <textarea class="min-h-20 rounded-lg border px-2 py-1 text-[11px] text-(--mono-text)" readonly>{advisoryPayloadDraft.encoding.payloadHex}</textarea>
              </label>
              <Notice variant="muted">The chain stores only the payload hash unless these same bounded payload bytes are separately noted as a preimage</Notice>
              <Notice variant="muted">`Intent` stays inside the current governance domain, while `L2SignalToL1` records the domain's upward signal toward L1 without dispatching privileged state transitions by itself</Notice>
              <Notice variant="muted">Optional preimage noting uses the generic Preimage pallet and the quoted note cost is reserved against the noting account until the preimage is requested or cleared under pallet rules</Notice>
            {/if}
            {#if advisoryReview.payloadHashPreimageStatus?.havePreimage}
              <Notice variant="muted">These exact payload bytes are already noted on-chain, so the extra preimage step is unnecessary</Notice>
            {/if}
            {#if advisoryReview.payloadHashPreimageStatus?.preimageRequested && !advisoryReview.payloadHashPreimageStatus.havePreimage}
              <Notice variant="muted">This payload hash is already requested on-chain but the bytes are not yet noted, so supplying the preimage should avoid a new user-held note deposit</Notice>
            {/if}
            <ActionReviewCard
              title="Submission review"
              submitActionLabel={submitReviewSummaryLine()}
              submitStatusLabel={submitReviewStatusLabel()}
              submitOpeningFeeLabel={selectedSubmissionOption ? openingFeeLabel("Signed", selectedSubmissionOption.openingFee) : "Unavailable"}
              submitResultLabel={submitReviewResultLine()}
              submitButtonLabel={publicSubmissionButtonLabel(selectedSubmitPayloadKind)}
              submitDisabled={submitPublicProposalDisabled}
              submitOnClick={submitPublicProposal}
              preimageActionLabel={preimageReviewSummaryLine()}
              preimageStatusLabel={preimageReviewStatusLabel()}
              preimageNoteCostLabel={advisoryPayloadPreimageNoteCostLabel(advisoryReview.payloadPreimageNoteCost, advisoryReview.payloadPreimageNoteCostLoading)}
              preimageResultLabel={preimageReviewResultLine()}
              preimageButtonLabel={advisoryReview.payloadHashPreimageStatusLoading || advisoryReview.payloadPreimageNoteCostLoading ? "Checking preimage quote" : advisoryReview.payloadHashPreimageStatus?.havePreimage ? "Preimage already noted" : "Note advisory preimage"}
              preimageDisabled={notePreimageDisabled}
              preimageOnClick={noteProposalPreimage}
            />
            {#if !submitItemReady}
              <Notice variant="muted">Enter a positive integer item id</Notice>
            {/if}
            {#if !advisoryPayloadDraft.summaryValid}
              <Notice variant="muted">Enter a non-empty summary within {GOVERNANCE_ADVISORY_SUMMARY_MAX_BYTES} UTF-8 bytes</Notice>
            {/if}
            {#if !advisoryPayloadDraft.docCidValid}
              <Notice variant="muted">Doc CID must stay within {GOVERNANCE_ADVISORY_DOC_CID_MAX_BYTES} UTF-8 bytes</Notice>
            {/if}
            {#if !advisoryPayloadDraft.referencedPayloadHashValid}
              <Notice variant="muted">Referenced payload hash must be blank or `0x` + 64 hex chars</Notice>
            {/if}
            {#if submitWriteAvailability.providerStatus !== "available"}
              <Notice variant="warn">{submitWriteAvailability.reason}</Notice>
            {/if}
            {#if preimageWriteAvailability.providerStatus !== "available"}
              <Notice variant="muted">{preimageWriteAvailability.reason}</Notice>
            {/if}
          {/if}
        </div>
        {#if treasurySubmissionOption}
          <div class="rounded-xl border bg-white p-3 grid gap-2 text-[10px] text-(--mono-muted)">
            <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Public tactical treasury submit</div>
            <ProposalSemanticsRows
              cadenceLabel="Ordinary only"
              payloadKindLabel={treasurySubmissionOptionLabel}
              familyLabel={payloadFamilyLabel("L2TreasurySpend")}
              executionAuthorityLabel={executionAuthorityLabel("DomainTreasury")}
              executionPathLabel={executionPathLabel("L2TreasurySpend", "Invoice")}
              openingFeeLabel={openingFeeLabel("Signed", treasurySubmissionOption.openingFee)}
              settlementLabel={treasurySettlementLabel("L2TreasurySpend", "Invoice")}
              fundingSourceLabel="BLDR treasury"
            />
            <Notice variant="muted">This is the smallest live browser composition slice for the signed tactical invoice path. It always encodes `BldrTreasury` as the funding source and leaves richer treasury-routing policy for later work.</Notice>
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Item id</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={treasurySubmitItemIdInput} inputmode="numeric" placeholder={suggestedSubmitItemId.toString()} />
            </label>
            <DetailRow label="Suggested item id" value={`#${suggestedSubmitItemId}`} valueClass="text-(--mono-text)" />
            {#if treasurySubmitItemIdInput.trim() !== suggestedSubmitItemId.toString()}
              <Button size="sm" variant="secondary" class="justify-center" onclick={applySuggestedTreasurySubmitItemId}>Use suggested item id</Button>
            {/if}
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Beneficiary</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={treasuryBeneficiaryInput} placeholder="5..." />
            </label>
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Payout asset id</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={treasuryPayoutAssetInput} inputmode="numeric" placeholder="268435458" />
            </label>
            <label class="grid gap-1">
              <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Base amount</span>
              <input class="rounded-lg border px-2 py-1 text-[12px] text-(--mono-text)" bind:value={treasuryBaseAmountInput} inputmode="numeric" placeholder="1000000000000" />
            </label>
            <DetailRow label="Derived payload hash" value={treasuryReview.payloadHashLoading ? "Computing..." : (treasuryReview.payloadHash ?? "Unavailable")} valueClass="text-(--mono-text) break-all" />
            <DetailRow label="Derived payload bytes" value={treasuryPayloadDraft.encoding ? `${treasuryPayloadDraft.encoding.payloadByteLength} bytes` : "Unavailable"} valueClass="text-(--mono-text)" />
            <DetailRow label="Derived preimage status" value={advisoryPayloadHashPreimageStatusLabel(treasuryReview.payloadHashPreimageStatus, treasuryReview.payloadHashPreimageStatusLoading)} valueClass="text-(--mono-text)" />
            <DetailRow label="Preimage note cost" value={advisoryPayloadPreimageNoteCostLabel(treasuryReview.payloadPreimageNoteCost, treasuryReview.payloadPreimageNoteCostLoading)} valueClass="text-(--mono-text)" />
            {#if treasuryPayloadDraft.encoding}
              <label class="grid gap-1">
                <span class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Derived payload hex</span>
                <textarea class="min-h-20 rounded-lg border px-2 py-1 text-[11px] text-(--mono-text)" readonly>{treasuryPayloadDraft.encoding.payloadHex}</textarea>
              </label>
              <Notice variant="muted">The chain stores only the payload hash unless these same bounded payload bytes are separately noted as a preimage.</Notice>
            {/if}
            {#if treasuryReview.payloadHashPreimageStatus?.havePreimage}
              <Notice variant="muted">These exact treasury payload bytes are already noted on-chain, so the extra preimage step is unnecessary.</Notice>
            {/if}
            {#if treasuryReview.payloadHashPreimageStatus?.preimageRequested && !treasuryReview.payloadHashPreimageStatus.havePreimage}
              <Notice variant="muted">This treasury payload hash is already requested on-chain but the bytes are not yet noted, so supplying the preimage should avoid a new user-held note deposit.</Notice>
            {/if}
            <ActionReviewCard
              title="Treasury submission review"
              submitActionLabel={`Submit treasury invoice · ${executionPathLabel("L2TreasurySpend", "Invoice")}`}
              submitStatusLabel={treasurySubmitReviewStatusLabel()}
              submitOpeningFeeLabel={openingFeeLabel("Signed", treasurySubmissionOption.openingFee)}
              submitResultLabel={treasurySubmitReviewResultLine()}
              submitButtonLabel="Submit treasury invoice"
              submitDisabled={treasurySubmitProposalDisabled}
              submitOnClick={submitTreasuryProposal}
              preimageActionLabel={treasuryPreimageReviewSummaryLine()}
              preimageStatusLabel={treasuryPreimageReviewStatusLabel()}
              preimageNoteCostLabel={advisoryPayloadPreimageNoteCostLabel(treasuryReview.payloadPreimageNoteCost, treasuryReview.payloadPreimageNoteCostLoading)}
              preimageResultLabel={treasuryPreimageReviewResultLine()}
              preimageButtonLabel={treasuryReview.payloadHashPreimageStatusLoading || treasuryReview.payloadPreimageNoteCostLoading ? "Checking preimage quote" : treasuryReview.payloadHashPreimageStatus?.havePreimage ? "Preimage already noted" : "Note treasury preimage"}
              preimageDisabled={treasuryNotePreimageDisabled}
              preimageOnClick={noteTreasuryProposalPreimage}
            />
            {#if !treasurySubmitItemReady}
              <Notice variant="muted">Enter a positive integer item id.</Notice>
            {/if}
            {#if !treasuryPayloadDraft.beneficiaryValid}
              <Notice variant="muted">Enter a valid beneficiary address.</Notice>
            {/if}
            {#if !treasuryPayloadDraft.payoutAssetValid}
              <Notice variant="muted">Payout asset id must be a valid `u32` asset identifier.</Notice>
            {/if}
            {#if !treasuryPayloadDraft.baseAmountValid}
              <Notice variant="muted">Base amount must be a positive integer that fits inside `u128`.</Notice>
            {/if}
          </div>
        {/if}
        {#if governanceStore.state.loading}
          <Notice>Loading proposals...</Notice>
        {:else if governanceStore.state.activeProposals.length === 0}
          <Notice>No active proposals</Notice>
        {:else}
          <div class="grid gap-3 @2xl:grid-cols-2">
            {#each governanceStore.state.activeProposals as proposal}
              <article class={[
                "rounded-xl border bg-(--mono-bg) transition-colors hover:border-(--mono-purple)/40",
                densePane ? "grid gap-2 p-2" : "grid gap-3 p-3",
              ]}>
                <div class="flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Proposal</div>
                    <div class="text-sm font-semibold text-(--mono-text)">#{proposal.itemId}</div>
                  </div>
                  <Badge variant="info">{statusLabel(proposal)}</Badge>
                </div>

                {#snippet tallyRows()}
                  {#if proposal.tally}
                    {#if proposal.primaryTrackTally?.kind === "Invoice"}
                      <DetailRow label="Amplify" value={proposal.tally.amplifyWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                      <DetailRow label="Approve" value={proposal.tally.approveWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                      <DetailRow label="Reduce" value={proposal.tally.reduceWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                      <DetailRow label="Nay" value={proposal.tally.nayWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                      <DetailRow label="Positive total" value={primaryTrackPositiveWeightLabel(proposal.primaryTrackTally)} valueClass="text-(--mono-text)" />
                      <DetailRow label="Leading positive" value={primaryTrackLeaderLabel(proposal.primaryTrackTally)} valueClass="text-(--mono-text)" />
                    {:else}
                      <DetailRow label="Aye" value={proposal.tally.ayeWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                      <DetailRow label="Nay" value={proposal.tally.nayWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                      <DetailRow label="Leading" value={primaryTrackLeaderLabel(proposal.primaryTrackTally)} valueClass="text-(--mono-text)" />
                    {/if}
                    <DetailRow label="Veto" value={proposal.tally.vetoWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                    <DetailRow label="Pass" value={proposal.tally.passWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                  {:else}
                    <div class="text-(--mono-muted)">No tally exposed yet</div>
                  {/if}
                {/snippet}

                {#snippet semanticsRows(powerLabel: string, vetoLabel: string)}
                  <DetailRow label="Cadence" value={cadenceModeLabel(proposal.metadata?.cadenceMode)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Family" value={payloadFamilyLabel(proposal.metadata?.payloadKind)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Primary track" value={primaryTrackFamilyLabel(proposal.primaryTrackFamily)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Payload" value={payloadKindLabel(proposal.metadata?.payloadKind)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Payload hash" value={proposal.metadata?.payloadHash ?? "Unavailable"} valueClass="text-(--mono-text) break-all" />
                  {#if advisoryScopeLabel(proposal.metadata?.payloadKind)}
                    <DetailRow label="Advisory scope" value={advisoryScopeLabel(proposal.metadata?.payloadKind) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                  <DetailRow label="Authority" value={executionAuthorityLabel(proposal.executionAuthority)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Submission" value={submissionAuthorityLabel(proposal.submissionAuthority)} valueClass="text-(--mono-text)" />
                  {#if proposal.submissionAuthority === "Signed"}
                    <DetailRow label="Opening fee" value={openingFeeLabel(proposal.submissionAuthority, proposal.openingFee)} valueClass="text-(--mono-text)" />
                  {/if}
                  <DetailRow label="Payload record" value={payloadAvailabilityLabel(proposal.metadata?.payloadKind, proposal.payloadAvailability)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Execution path" value={executionPathLabel(proposal.metadata?.payloadKind, proposal.primaryTrackFamily)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Urgent path" value={urgentEligibilityLabel(proposal.urgentEligibility, proposal.metadata?.payloadKind)} valueClass="text-(--mono-text)" />
                  {#if urgentExecutionContractLabel(proposal.metadata?.payloadKind)}
                    <DetailRow label="Urgent execution" value={urgentExecutionContractLabel(proposal.metadata?.payloadKind) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                  {#if treasurySettlementLabel(proposal.metadata?.payloadKind, proposal.primaryTrackFamily)}
                    <DetailRow label="Treasury settlement" value={treasurySettlementLabel(proposal.metadata?.payloadKind, proposal.primaryTrackFamily) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                  <DetailRow label="Protection window" value={timingWindowLabel(proposal.timing?.protectionOpenEpoch, proposal.timing?.protectionCloseEpoch)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Primary window" value={timingWindowLabel(proposal.timing?.effectivePrimaryOpenEpoch, proposal.timing?.effectivePrimaryCloseEpoch)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Pending enactment" value={pendingEnactmentLabel(proposal.timing?.pendingEnactmentEpoch)} valueClass="text-(--mono-text)" />
                  <DetailRow label={powerLabel} value={primaryTrackPowerLabel(proposal)} valueClass="text-(--mono-text)" />
                  <DetailRow label={vetoLabel} value={profileLabel(proposal.votePowerProfiles.veto)} valueClass="text-(--mono-text)" />
                  {#if proposal.accountPowerView}
                    <DetailRow label="Account lock" value={governanceLockLabel(proposal.accountPowerView.governanceLockUntil)} valueClass="text-(--mono-text)" />
                    <DetailRow label="My current primary" value={proposal.accountPowerView.currentOrdinaryWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                    <DetailRow label="My current protection" value={proposal.accountPowerView.currentProtectionWeight.toLocaleString()} valueClass="text-(--mono-text)" />
                    <DetailRow label="Protection raw" value={proposal.accountPowerView.currentProtectionRawPower.toLocaleString()} valueClass="text-(--mono-text)" />
                    <DetailRow label="Frozen primary" value={frozenBallotLabel(proposal.accountPowerView.frozenOrdinaryBallot)} valueClass="text-(--mono-text)" />
                    <DetailRow label="Frozen protection" value={frozenBallotLabel(proposal.accountPowerView.frozenProtectionBallot)} valueClass="text-(--mono-text)" />
                  {/if}
                  {#if hasDecliningPower(proposal)}
                    <Notice variant="warn">Early ballots carry more weight</Notice>
                  {/if}
                {/snippet}

                {#if compactPane}
                  <div class="rounded-xl border bg-white px-3 py-2 grid gap-1 text-[10px] text-(--mono-muted)">
                    <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Proposal state</div>
                    {@render tallyRows()}
                    {@render semanticsRows("Primary track power", "Veto / Pass power")}
                  </div>
                {:else}
                  <div class="grid gap-3 @sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                    <div class="rounded-xl border bg-white p-3 grid gap-1 text-[10px] text-(--mono-muted)">
                      <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Tally</div>
                      {@render tallyRows()}
                    </div>
                    <div class="rounded-xl border bg-white p-3 grid gap-1 text-[10px] text-(--mono-muted)">
                      <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">Vote power & semantics</div>
                      {@render semanticsRows("Primary track", "Veto / Pass")}
                    </div>
                  </div>
                {/if}

                <div class={[
                  "grid gap-2",
                  densePane ? "grid-cols-2" : "grid-cols-2 @sm:grid-cols-4 @xl:grid-cols-6",
                ]}>
                  {#each voteButtons(proposal) as button}
                    <Button size="sm" variant="secondary" class="h-auto w-full py-1 text-[10px]" disabled={voteWriteAvailability.providerStatus !== "available"} onclick={() => governanceStore.castVote(proposal.itemId, button.voteKind)}>{button.label}</Button>
                  {/each}
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </SectionCard>

      <SectionCard title="Recent finalized" subtitle="Bounded recent outcomes">
        {#snippet actions()}
          <ReadModelBadge provenance={recentFinalizedProvenance} />
          <Badge variant="info">{governanceStore.state.recentFinalizedProposals.length} retained</Badge>
        {/snippet}
        <Notice variant="muted">Bounded retained on-chain history only, not a full governance archive</Notice>
        {#if governanceStore.state.recentFinalizedProposals.length > 0}
          <div class="grid gap-2">
            {#each governanceStore.state.recentFinalizedProposals as proposal}
              <div class="rounded-xl border bg-(--mono-bg) px-3 py-2 grid gap-1">
                <DetailRow label={`#${proposal.itemId}`} value={finalizedOutcomeLabel(proposal)} valueClass="text-(--mono-muted)" />
                {#if proposal.metadata}
                  <DetailRow label="Cadence" value={cadenceModeLabel(proposal.metadata.cadenceMode)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Family" value={payloadFamilyLabel(proposal.metadata.payloadKind)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Primary track" value={primaryTrackFamilyLabel(proposal.primaryTrackFamily)} valueClass="text-(--mono-text)" />
                  {#if retainedWinningPrimaryOptionLabel(proposal)}
                    <DetailRow label="Winning primary" value={retainedWinningPrimaryOptionLabel(proposal) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                  <DetailRow label="Payload" value={payloadKindLabel(proposal.metadata.payloadKind)} valueClass="text-(--mono-text)" />
                  <DetailRow label="Payload hash" value={proposal.metadata.payloadHash} valueClass="text-(--mono-text) break-all" />
                  {#if advisoryScopeLabel(proposal.metadata.payloadKind)}
                    <DetailRow label="Advisory scope" value={advisoryScopeLabel(proposal.metadata.payloadKind) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                  <DetailRow label="Urgent path" value={urgentEligibilityLabel(proposal.urgentEligibility, proposal.metadata.payloadKind)} valueClass="text-(--mono-text)" />
                  {#if urgentExecutionContractLabel(proposal.metadata.payloadKind)}
                    <DetailRow label="Urgent execution" value={urgentExecutionContractLabel(proposal.metadata.payloadKind) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                {/if}
                {#if proposal.executionAuthority}
                  <DetailRow label="Authority" value={executionAuthorityLabel(proposal.executionAuthority)} valueClass="text-(--mono-text)" />
                {/if}
                {#if proposal.submissionAuthority}
                  <DetailRow label="Submission" value={submissionAuthorityLabel(proposal.submissionAuthority)} valueClass="text-(--mono-text)" />
                  {#if proposal.submissionAuthority === "Signed"}
                    <DetailRow label="Opening fee" value={openingFeeLabel(proposal.submissionAuthority, proposal.openingFee)} valueClass="text-(--mono-text)" />
                  {/if}
                {/if}
                {#if proposal.metadata && proposal.payloadAvailability}
                  <DetailRow label="Payload record" value={payloadAvailabilityLabel(proposal.metadata.payloadKind, proposal.payloadAvailability)} valueClass="text-(--mono-text)" />
                {/if}
                <DetailRow label="Execution state" value={finalizedExecutionStateLabel(proposal)} valueClass="text-(--mono-text)" />
                {#if proposal.metadata}
                  <DetailRow label="Execution path" value={executionPathLabel(proposal.metadata.payloadKind, proposal.primaryTrackFamily)} valueClass="text-(--mono-text)" />
                  {#if treasurySettlementLabel(proposal.metadata.payloadKind, proposal.primaryTrackFamily)}
                    <DetailRow label="Treasury settlement" value={treasurySettlementLabel(proposal.metadata.payloadKind, proposal.primaryTrackFamily) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  {/if}
                {/if}
                {#each finalizedExecutionDetailRows(proposal.executionDetail) as row}
                  <DetailRow label={row.label} value={row.value} valueClass={row.mono ? "text-(--mono-text) break-all" : "text-(--mono-text)"} />
                {/each}
                {#if finalizedRuntimeUpgradeApplicationLabel(proposal)}
                  <DetailRow label="Upgrade application" value={finalizedRuntimeUpgradeApplicationLabel(proposal) ?? "Unavailable"} valueClass="text-(--mono-text)" />
                  <DetailRow label="Operator path" value={runtimeUpgradeOperatorPathLabel()} valueClass="text-(--mono-text) break-all" />
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <Notice>No retained finalized proposals yet</Notice>
        {/if}
      </SectionCard>

      <SectionCard title="Governance archive" subtitle="Explicit materialized provider boundary">
        {#snippet actions()}
          <ReadModelBadge provenance={materializedArchivePlaceholder.provenance} />
          <Badge variant="info">future provider</Badge>
        {/snippet}
        <Notice variant="muted">Recent finalized cards above are bounded on-chain retention. Full archive search and ballot timelines belong to a separate materialized/indexed provider, not to expanded consensus-state retention.</Notice>
        <div class="rounded-xl border bg-(--mono-bg) px-3 py-2 grid gap-1">
          <DetailRow label="Provider" value={materializedArchiveProvider.label()} valueClass="text-(--mono-text)" />
          <DetailRow label="Contract" value="Indexed archive/search provider" valueClass="text-(--mono-text)" />
          <DetailRow label="Current status" value={materializedArchiveProvider.message() ?? "Configured"} valueClass="text-(--mono-muted)" />
        </div>
        {#if materializedArchivePlaceholder.value.length > 0}
          <div class="grid gap-2">
            {#each materializedArchivePlaceholder.value as entry}
              <div class="rounded-xl border bg-(--mono-bg) px-3 py-2 grid gap-1">
                <DetailRow label={`#${entry.itemId}`} value={entry.title} valueClass="text-(--mono-text)" />
                <DetailRow label="Outcome" value={entry.outcomeLabel} valueClass="text-(--mono-muted)" />
                <DetailRow label="Summary" value={entry.summary} valueClass="text-(--mono-muted)" />
              </div>
            {/each}
          </div>
        {:else}
          <Notice>No materialized governance archive backend is configured on the current reference line</Notice>
        {/if}
      </SectionCard>
    </div>
  </div>
</Card>
