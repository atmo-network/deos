<script lang="ts">
  import type { Snippet } from "svelte";
  import { fmt, toFloat } from "$lib/shared/format";
  import type {
    NativeCollatorLpPositionProjection,
    NativeGovernanceCustodyPositionProjection,
  } from "$lib/shared/types";
  import { fromClientBoundedProjection } from "$lib/shared/read-model";
  import { Button, Notice, ReadModelBadge, SectionCard, StatCard, TextField } from "$lib/shared/ui";
  import { portfolioStore } from "$lib/portfolio/index.svelte";
  import { systemStore } from "$lib/system/index.svelte";
  import { walletStore } from "$lib/wallet/index.svelte";

  type Props = {
    compactPane: boolean;
    densePane: boolean;
  };

  let { compactPane, densePane }: Props = $props();
  let rewardEpochInput = $state("");
  let compoundOperatorInput = $state("");
  let lpAmountInput = $state("");
  let lpOperatorInput = $state("");
  let lpTargetOperatorInput = $state("");
  let governanceCustodyAmountInput = $state("");
  let governanceCustodyAssetKind = $state<"native" | "staked">("native");
  let stakingActionBusy = $state(false);
  let stakingActionError = $state<string | null>(null);
  let collatorPositionDetail = $state<NativeCollatorLpPositionProjection | null>(null);
  let governanceCustodyDetail = $state<NativeGovernanceCustodyPositionProjection | null>(null);
  let nominationRewardClaimable = $state<bigint | null>(null);
  let nominationRewardClaimableEpoch = $state<number | null>(null);

  const nativeStakingProvenance = fromClientBoundedProjection(
    true,
    "stakingWidget.nativeStaking <- staking runtime view functions",
  ).provenance;

  function fixedU128ToFloat(value: bigint | null): number | null {
    if (value == null) {
      return null;
    }
    return Number(value) / 1e18;
  }

  function optionalBlockLabel(block: number | null): string {
    return block == null ? "—" : `#${block}`;
  }

  function optionalAssetLabel(assetId: number | null): string {
    return assetId == null ? "—" : `#${assetId}`;
  }

  function parseRewardEpoch(): number | null {
    const trimmed = rewardEpochInput.trim();
    if (!/^\d+$/.test(trimmed)) {
      return null;
    }
    const epoch = Number(trimmed);
    return Number.isSafeInteger(epoch) ? epoch : null;
  }

  function parseTokenAmount(value: string): bigint | null {
    const trimmed = value.trim();
    const match = /^(\d+)(?:\.(\d{0,12}))?$/.exec(trimmed);
    if (!match) {
      return null;
    }
    const whole = BigInt(match[1]);
    const fraction = BigInt((match[2] ?? "").padEnd(12, "0"));
    const amount = whole * 1_000_000_000_000n + fraction;
    return amount > 0n ? amount : null;
  }

  function formatTokenInput(value: bigint): string {
    const whole = value / 1_000_000_000_000n;
    const fraction = (value % 1_000_000_000_000n).toString().padStart(12, "0").replace(/0+$/u, "");
    return fraction.length > 0 ? `${whole}.${fraction}` : whole.toString();
  }

  const snap = $derived(systemStore.snapshot);
  const nativeStakingCards = $derived.by(() => {
    if (!snap?.nativeStaking.isAvailable) {
      return [];
    }
    const pool = snap.nativeStaking.pool;
    const position = snap.nativeStaking.accountPosition;
    const exchangeRate = fixedU128ToFloat(snap.nativeStaking.exchangeRate);
    return [
      { label: "stNTVE rate", value: exchangeRate != null ? `${fmt(exchangeRate)} NTVE` : "—", detail: "Per stNTVE receipt" },
      { label: "NTVE reserve", value: pool ? fmt(toFloat(pool.reserveNative)) : "—", detail: pool ? `LP #${pool.lpAssetId}` : "Pool unavailable" },
      { label: "stNTVE reserve", value: pool ? fmt(toFloat(pool.reserveStaked)) : "—", detail: pool ? `st asset #${pool.stakedAssetId}` : "Pool unavailable" },
      { label: "Locked LP", value: position ? fmt(toFloat(position.totalLockedLp)) : "—", detail: "Selected account custody" },
      { label: "Collator LP", value: position ? fmt(toFloat(position.collatorLockedLp)) : "—", detail: "Nomination reward base" },
      { label: "LP native value", value: position?.conservativeNativeValue != null ? fmt(toFloat(position.conservativeNativeValue)) : "—", detail: "Conservative equivalent" },
    ];
  });
  const nativeWalletBalances = $derived.by(() => {
    const pool = snap?.nativeStaking.pool;
    const assets = portfolioStore.knownAssets;
    if (!pool) {
      return { native: 0n, staked: 0n, lp: 0n };
    }
    return {
      native: assets.find((asset) => asset.assetId === pool.nativeAssetId)?.balance ?? 0n,
      staked: assets.find((asset) => asset.assetId === pool.stakedAssetId)?.balance ?? 0n,
      lp: assets.find((asset) => asset.assetId === pool.lpAssetId)?.balance ?? 0n,
    };
  });
  const nativeWalletBalanceCards = $derived.by(() => {
    const pool = snap?.nativeStaking.pool;
    if (!pool) {
      return [];
    }
    return [
      { label: "Wallet NTVE", value: fmt(toFloat(nativeWalletBalances.native)), detail: `Asset #${pool.nativeAssetId}` },
      { label: "Wallet stNTVE", value: fmt(toFloat(nativeWalletBalances.staked)), detail: `Asset #${pool.stakedAssetId}` },
      { label: "Wallet LP", value: fmt(toFloat(nativeWalletBalances.lp)), detail: `LP #${pool.lpAssetId}` },
    ];
  });
  const rewardEpoch = $derived(parseRewardEpoch());
  const lpAmount = $derived(parseTokenAmount(lpAmountInput));
  const governanceCustodyAmount = $derived(parseTokenAmount(governanceCustodyAmountInput));
  const selectedGovernanceAssetId = $derived.by(() => {
    const pool = snap?.nativeStaking.pool;
    if (!pool) {
      return null;
    }
    return governanceCustodyAssetKind === "native" ? pool.nativeAssetId : pool.stakedAssetId;
  });
  const selectedGovernanceAssetLabel = $derived(governanceCustodyAssetKind === "native" ? "NTVE" : "stNTVE");
  const stakingSignerUnavailable = $derived(walletStore.state.signerStatus !== "available");
  const nativeStakingPoolUnavailable = $derived(!snap?.nativeStaking.pool);
  const stakingWriteDisabled = $derived(stakingActionBusy || stakingSignerUnavailable || rewardEpoch === null);
  const compoundDisabled = $derived(stakingWriteDisabled || compoundOperatorInput.trim().length === 0);
  const lpAmountActionDisabled = $derived(stakingActionBusy || stakingSignerUnavailable || nativeStakingPoolUnavailable || lpAmount === null || lpOperatorInput.trim().length === 0);
  const lpWithdrawDisabled = $derived(stakingActionBusy || stakingSignerUnavailable || lpOperatorInput.trim().length === 0);
  const lpRedelegateDisabled = $derived(lpAmountActionDisabled || lpTargetOperatorInput.trim().length === 0);
  const governanceCustodyAmountDisabled = $derived(stakingActionBusy || stakingSignerUnavailable || nativeStakingPoolUnavailable || governanceCustodyAmount === null);
  const governanceAssetActionDisabled = $derived(governanceCustodyAmountDisabled || selectedGovernanceAssetId === null);
  const governanceAssetWithdrawDisabled = $derived(stakingActionBusy || stakingSignerUnavailable || selectedGovernanceAssetId === null);

  async function runStakingAction(action: () => Promise<void>, fallbackMessage: string) {
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      await action();
    } catch (error) {
      stakingActionError = error instanceof Error ? error.message : fallbackMessage;
    } finally {
      stakingActionBusy = false;
    }
  }

  async function loadCollatorPositionDetail() {
    const operator = lpOperatorInput.trim();
    if (operator.length === 0) {
      stakingActionError = "Enter a collator/operator address first";
      return;
    }
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      collatorPositionDetail = await systemStore.getNativeCollatorLpPosition(operator);
    } catch (error) {
      stakingActionError = error instanceof Error ? error.message : "Native collator LP detail load failed";
    } finally {
      stakingActionBusy = false;
    }
  }

  async function loadGovernanceCustodyDetail() {
    const assetId = selectedGovernanceAssetId;
    if (assetId === null) {
      stakingActionError = "Wait for the native staking pool view before loading governance custody detail";
      return;
    }
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      governanceCustodyDetail = await systemStore.getNativeGovernanceCustodyPosition(assetId);
    } catch (error) {
      stakingActionError = error instanceof Error ? error.message : "Native governance custody detail load failed";
    } finally {
      stakingActionBusy = false;
    }
  }

  async function loadNominationRewardClaimability() {
    const epoch = rewardEpoch;
    if (epoch === null) {
      stakingActionError = "Enter a closed reward epoch first";
      return;
    }
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      nominationRewardClaimable = await systemStore.getNativeNominationRewardClaimable(epoch);
      nominationRewardClaimableEpoch = epoch;
    } catch (error) {
      stakingActionError = error instanceof Error ? error.message : "Native nomination reward claimability load failed";
    } finally {
      stakingActionBusy = false;
    }
  }

  function useMaxLpAmount() {
    lpAmountInput = formatTokenInput(nativeWalletBalances.lp);
  }

  function useMaxGovernanceCustodyAmount() {
    const balance = governanceCustodyAssetKind === "native" ? nativeWalletBalances.native : nativeWalletBalances.staked;
    governanceCustodyAmountInput = formatTokenInput(balance);
  }

  async function claimNominationReward() {
    const epoch = rewardEpoch;
    if (epoch === null) {
      stakingActionError = "Enter a closed reward epoch first";
      return;
    }
    await runStakingAction(
      () => systemStore.claimNominationReward(epoch),
      "Native nomination claim failed",
    );
  }

  async function compoundNominationReward() {
    const epoch = rewardEpoch;
    if (epoch === null) {
      stakingActionError = "Enter a closed reward epoch first";
      return;
    }
    await runStakingAction(
      () => systemStore.claimAndCompoundNominationReward(epoch, compoundOperatorInput),
      "Native nomination compound failed",
    );
  }

  async function lockNativeLpForCollator() {
    const amount = lpAmount;
    if (amount === null) {
      stakingActionError = "Enter an LP amount first";
      return;
    }
    await runStakingAction(
      () => systemStore.lockNativeLpForCollator(amount, lpOperatorInput),
      "Native LP collator lock failed",
    );
  }

  async function requestUnlockNativeLp() {
    const amount = lpAmount;
    if (amount === null) {
      stakingActionError = "Enter an LP amount first";
      return;
    }
    await runStakingAction(
      () => systemStore.requestUnlockNativeLp(lpOperatorInput, amount),
      "Native LP unlock request failed",
    );
  }

  async function withdrawUnlockedNativeLp() {
    await runStakingAction(
      () => systemStore.withdrawUnlockedNativeLp(lpOperatorInput),
      "Native LP withdrawal failed",
    );
  }

  async function redelegateNativeLp() {
    const amount = lpAmount;
    if (amount === null) {
      stakingActionError = "Enter an LP amount first";
      return;
    }
    await runStakingAction(
      () => systemStore.redelegateNativeLp(lpOperatorInput, lpTargetOperatorInput, amount),
      "Native LP redelegation failed",
    );
  }

  async function lockNativeLpForGovernance() {
    const amount = governanceCustodyAmount;
    if (amount === null) {
      stakingActionError = "Enter a governance custody amount first";
      return;
    }
    await runStakingAction(
      () => systemStore.lockNativeLpForGovernance(amount),
      "Native governance LP lock failed",
    );
  }

  async function requestUnlockNativeLpForGovernance() {
    const amount = governanceCustodyAmount;
    if (amount === null) {
      stakingActionError = "Enter a governance custody amount first";
      return;
    }
    await runStakingAction(
      () => systemStore.requestUnlockNativeLpForGovernance(amount),
      "Native governance LP unlock request failed",
    );
  }

  async function withdrawUnlockedNativeLpForGovernance() {
    await runStakingAction(
      () => systemStore.withdrawUnlockedNativeLpForGovernance(),
      "Native governance LP withdrawal failed",
    );
  }

  async function lockNativeAssetForGovernance() {
    const amount = governanceCustodyAmount;
    const assetId = selectedGovernanceAssetId;
    if (amount === null || assetId === null) {
      stakingActionError = "Enter a governance custody amount and wait for the native staking pool view";
      return;
    }
    await runStakingAction(
      () => systemStore.lockNativeAssetForGovernance(assetId, amount),
      "Native governance asset lock failed",
    );
  }

  async function requestUnlockNativeAssetForGovernance() {
    const amount = governanceCustodyAmount;
    const assetId = selectedGovernanceAssetId;
    if (amount === null || assetId === null) {
      stakingActionError = "Enter a governance custody amount and wait for the native staking pool view";
      return;
    }
    await runStakingAction(
      () => systemStore.requestUnlockNativeAssetForGovernance(assetId, amount),
      "Native governance asset unlock request failed",
    );
  }

  async function withdrawUnlockedNativeAssetForGovernance() {
    const assetId = selectedGovernanceAssetId;
    if (assetId === null) {
      stakingActionError = "Wait for the native staking pool view before withdrawing governance custody";
      return;
    }
    await runStakingAction(
      () => systemStore.withdrawUnlockedNativeAssetForGovernance(assetId),
      "Native governance asset withdrawal failed",
    );
  }
</script>

{#snippet actionSection(title: string, description: string, children: Snippet)}
  <details class="rounded-xl border bg-white p-3 group" open>
    <summary class="cursor-pointer list-none">
      <div class="grid gap-1">
        <div class="flex items-start justify-between gap-2">
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">{title}</div>
          <div class="text-[10px] text-(--mono-muted) group-open:rotate-180 transition-transform">⌄</div>
        </div>
        <div class="text-[10px] text-(--mono-muted)">{description}</div>
      </div>
    </summary>
    <div class="mt-3 grid gap-3">
      {@render children()}
    </div>
  </details>
{/snippet}

{#snippet nominationRewardsSection()}
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "@lg:grid-cols-[minmax(0,0.65fr)_minmax(0,1fr)]",
  ]}>
    <TextField label="Reward epoch" bind:value={rewardEpochInput} inputmode="numeric" placeholder="Closed epoch" />
    <TextField label="Compound operator" bind:value={compoundOperatorInput} placeholder="Collator address for compound lock" />
  </div>
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "grid-cols-3",
  ]}>
    <Button size="sm" variant="ghost" disabled={stakingActionBusy || rewardEpoch === null} onclick={loadNominationRewardClaimability}>Check claimable</Button>
    <Button size="sm" variant="secondary" disabled={stakingWriteDisabled} onclick={claimNominationReward}>{stakingActionBusy ? "Submitting..." : "Claim liquid NTVE"}</Button>
    <Button size="sm" variant="primary" disabled={compoundDisabled} onclick={compoundNominationReward}>{stakingActionBusy ? "Submitting..." : "Claim + compound LP"}</Button>
  </div>
  {#if nominationRewardClaimableEpoch !== null}
    <Notice variant="muted">Epoch {nominationRewardClaimableEpoch} claimable: {nominationRewardClaimable === null ? "not claimable" : `${fmt(toFloat(nominationRewardClaimable))} NTVE`}</Notice>
  {/if}
{/snippet}

{#snippet collatorLpSection()}
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "@lg:grid-cols-[minmax(0,0.55fr)_minmax(0,1fr)_minmax(0,1fr)]",
  ]}>
    <TextField label="LP amount" bind:value={lpAmountInput} inputmode="decimal" placeholder="0.0" />
    <TextField label="Operator / source" bind:value={lpOperatorInput} placeholder="Current collator" />
    <TextField label="Redelegate target" bind:value={lpTargetOperatorInput} placeholder="New collator" />
  </div>
  {#if nativeStakingPoolUnavailable}
    <Notice variant="warn">The canonical NTVE/stNTVE pool view is required before LP custody writes can be submitted.</Notice>
  {/if}
  {#if walletStore.state.signerStatus !== "available"}
    <Notice variant="warn">A signer is required before native staking writes can be submitted.</Notice>
  {/if}
  {#if stakingActionError}
    <Notice variant="warn">{stakingActionError}</Notice>
  {/if}
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "grid-cols-2",
  ]}>
    <Button size="sm" variant="ghost" disabled={nativeWalletBalances.lp <= 0n} onclick={useMaxLpAmount}>Use wallet LP max</Button>
    <Button size="sm" variant="ghost" disabled={stakingActionBusy || lpOperatorInput.trim().length === 0} onclick={loadCollatorPositionDetail}>Load operator position detail</Button>
  </div>
  {#if collatorPositionDetail}
    <div class={[
      "grid gap-2",
      densePane ? "grid-cols-1" : "grid-cols-2 @2xl:grid-cols-5",
    ]}>
      <StatCard label="Operator LP" value={fmt(toFloat(collatorPositionDetail.lockedLp))} detail={`LP ${optionalAssetLabel(collatorPositionDetail.lpAssetId)}`} />
      <StatCard label="Pending unlock" value={fmt(toFloat(collatorPositionDetail.pendingUnlockLp))} detail={optionalBlockLabel(collatorPositionDetail.pendingUnlockBlock)} />
      <StatCard label="Operator value" value={collatorPositionDetail.conservativeNativeValue == null ? "—" : fmt(toFloat(collatorPositionDetail.conservativeNativeValue))} detail="Conservative NTVE" />
    </div>
  {/if}
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "grid-cols-2 @2xl:grid-cols-4",
  ]}>
    <Button size="sm" variant="primary" disabled={lpAmountActionDisabled} onclick={lockNativeLpForCollator}>{stakingActionBusy ? "Submitting..." : "Lock LP"}</Button>
    <Button size="sm" variant="secondary" disabled={lpAmountActionDisabled} onclick={requestUnlockNativeLp}>{stakingActionBusy ? "Submitting..." : "Request unlock"}</Button>
    <Button size="sm" variant="secondary" disabled={lpWithdrawDisabled} onclick={withdrawUnlockedNativeLp}>{stakingActionBusy ? "Submitting..." : "Withdraw"}</Button>
    <Button size="sm" variant="secondary" disabled={lpRedelegateDisabled} onclick={redelegateNativeLp}>{stakingActionBusy ? "Submitting..." : "Redelegate"}</Button>
  </div>
{/snippet}

{#snippet governanceCustodySection()}
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "@lg:grid-cols-[minmax(0,0.7fr)_minmax(0,1fr)]",
  ]}>
    <TextField label="Custody amount" bind:value={governanceCustodyAmountInput} inputmode="decimal" placeholder="0.0" />
    <div class="grid gap-1">
      <div class="text-xs text-(--mono-muted)">Asset custody source</div>
      <div class="grid grid-cols-2 gap-2">
        <Button size="sm" variant={governanceCustodyAssetKind === "native" ? "primary" : "secondary"} onclick={() => governanceCustodyAssetKind = "native"}>NTVE</Button>
        <Button size="sm" variant={governanceCustodyAssetKind === "staked" ? "primary" : "secondary"} onclick={() => governanceCustodyAssetKind = "staked"}>stNTVE</Button>
      </div>
      <div class="text-[10px] text-(--mono-muted)">Selected: {selectedGovernanceAssetLabel}{selectedGovernanceAssetId === null ? " · pool view unavailable" : ` · asset #${selectedGovernanceAssetId}`}</div>
    </div>
  </div>
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "grid-cols-2",
  ]}>
    <Button size="sm" variant="ghost" disabled={(governanceCustodyAssetKind === "native" ? nativeWalletBalances.native : nativeWalletBalances.staked) <= 0n} onclick={useMaxGovernanceCustodyAmount}>Use wallet {selectedGovernanceAssetLabel} max</Button>
    <Button size="sm" variant="ghost" disabled={stakingActionBusy || selectedGovernanceAssetId === null} onclick={loadGovernanceCustodyDetail}>Load NativeVotePower custody detail</Button>
  </div>
  {#if governanceCustodyDetail}
    <div class={[
      "grid gap-2",
      densePane ? "grid-cols-1" : "grid-cols-2 @2xl:grid-cols-4",
    ]}>
      <StatCard label="Governance LP" value={fmt(toFloat(governanceCustodyDetail.governanceLockedLp))} detail={`LP ${optionalAssetLabel(governanceCustodyDetail.lpAssetId)}`} />
      <StatCard label="Pending LP unlock" value={fmt(toFloat(governanceCustodyDetail.pendingGovernanceLpUnlock))} detail={optionalBlockLabel(governanceCustodyDetail.pendingGovernanceLpUnlockBlock)} />
      <StatCard label={`${selectedGovernanceAssetLabel} locked`} value={fmt(toFloat(governanceCustodyDetail.assetLocked))} detail={`Asset #${governanceCustodyDetail.assetId}`} />
      <StatCard label="Pending asset unlock" value={fmt(toFloat(governanceCustodyDetail.pendingAssetUnlock))} detail={optionalBlockLabel(governanceCustodyDetail.pendingAssetUnlockBlock)} />
    </div>
  {/if}
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "grid-cols-3",
  ]}>
    <Button size="sm" variant="primary" disabled={governanceAssetActionDisabled} onclick={lockNativeAssetForGovernance}>{stakingActionBusy ? "Submitting..." : `Lock ${selectedGovernanceAssetLabel}`}</Button>
    <Button size="sm" variant="secondary" disabled={governanceAssetActionDisabled} onclick={requestUnlockNativeAssetForGovernance}>{stakingActionBusy ? "Submitting..." : `Unlock ${selectedGovernanceAssetLabel}`}</Button>
    <Button size="sm" variant="secondary" disabled={governanceAssetWithdrawDisabled} onclick={withdrawUnlockedNativeAssetForGovernance}>{stakingActionBusy ? "Submitting..." : `Withdraw ${selectedGovernanceAssetLabel}`}</Button>
  </div>
  <div class={[
    "grid gap-2",
    densePane ? "grid-cols-1" : "grid-cols-3",
  ]}>
    <Button size="sm" variant="primary" disabled={governanceCustodyAmountDisabled} onclick={lockNativeLpForGovernance}>{stakingActionBusy ? "Submitting..." : "Lock governance LP"}</Button>
    <Button size="sm" variant="secondary" disabled={governanceCustodyAmountDisabled} onclick={requestUnlockNativeLpForGovernance}>{stakingActionBusy ? "Submitting..." : "Unlock governance LP"}</Button>
    <Button size="sm" variant="secondary" disabled={stakingActionBusy || stakingSignerUnavailable} onclick={withdrawUnlockedNativeLpForGovernance}>{stakingActionBusy ? "Submitting..." : "Withdraw governance LP"}</Button>
  </div>
{/snippet}

<SectionCard title="Staking" subtitle="Bounded NTVE/stNTVE read model">
  {#snippet actions()}
    <ReadModelBadge provenance={nativeStakingProvenance} tone="subtle" />
  {/snippet}
  {#if nativeStakingCards.length > 0}
    <div class="grid gap-3">
      <div class={[
        "grid gap-2",
        densePane ? "grid-cols-1" : compactPane ? "grid-cols-2" : "grid-cols-2 @2xl:grid-cols-3",
      ]}>
        {#each nativeStakingCards as card}
          <StatCard label={card.label} value={card.value} detail={card.detail} />
        {/each}
      </div>
      {#if nativeWalletBalanceCards.length > 0}
        <div class={[
          "grid gap-2",
          densePane ? "grid-cols-1" : "grid-cols-3",
        ]}>
          {#each nativeWalletBalanceCards as card}
            <StatCard label={card.label} value={card.value} detail={card.detail} />
          {/each}
        </div>
      {/if}
      {@render actionSection(
        "Nomination rewards",
        "Claim liquid NTVE or compound a closed epoch into fresh NTVE/stNTVE LP locked to a collator.",
        nominationRewardsSection,
      )}
      {@render actionSection(
        "Collator LP nomination",
        "Lock, unlock, withdraw, or redelegate canonical NTVE/stNTVE LP. Unlock requests stop operator backing immediately and withdraw only after the runtime delay.",
        collatorLpSection,
      )}
      {@render actionSection(
        "NativeVotePower custody",
        "Lock NTVE, stNTVE, or standalone NTVE/stNTVE LP for governance-only NativeVotePower. These locks do not add collator backing or nomination reward base.",
        governanceCustodySection,
      )}
    </div>
  {:else}
    <Notice>Native staking runtime views are unavailable in the current metadata or the canonical NTVE/stNTVE pool is not initialized.</Notice>
  {/if}
</SectionCard>
