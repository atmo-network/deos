<!--
Domain: Staking widget
Owns: Native staking, collator LP custody, governance custody, and staking action presentation.
Excludes: Staking contract ownership, adapter staking transaction implementation, and system refresh lifecycle.
Zone: Presentation widget; consumes staking/system projections and UI Kit/read-model helpers.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  import { parseUnsignedDecimalNumber } from '$lib/format/numeric';
  import { portfolioStore } from '$lib/portfolio/index.svelte';
  import type {
    NativeCollatorLpPositionProjection,
    NativeGovernanceCustodyPositionProjection,
  } from '$lib/staking/types';
  import { resolveChainSurfaceState } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Button,
    DisclosureSection,
    Notice,
    NumberInput,
    SectionCard,
    StatCard,
    TextField,
  } from '$lib/ui';
  import {
    fmt,
    formatTokenInputAmount,
    parseTokenInputAmount,
    toFloat,
  } from '$lib/ui/format';
  import { walletStore } from '$lib/wallet/index.svelte';

  let rewardEpochInput = $state('');
  let compoundOperatorInput = $state('');
  let lpAmountInput = $state('');
  let lpOperatorInput = $state('');
  let lpTargetOperatorInput = $state('');
  let governanceCustodyAmountInput = $state('');
  let governanceCustodyAssetKind = $state<'native' | 'staked'>('native');
  let stakingActionBusy = $state(false);
  let stakingActionError = $state<string | null>(null);
  let collatorPositionDetail =
    $state<NativeCollatorLpPositionProjection | null>(null);
  let governanceCustodyDetail =
    $state<NativeGovernanceCustodyPositionProjection | null>(null);
  let nominationRewardClaimable = $state<bigint | null>(null);
  let nominationRewardClaimableEpoch = $state<number | null>(null);

  function fixedU128ToFloat(value: bigint | null): number | null {
    if (value == null) {
      return null;
    }
    return Number(value) / 1e18;
  }

  function optionalBlockLabel(block: number | null): string {
    return block == null ? '—' : `#${block}`;
  }

  function optionalAssetLabel(assetId: number | null): string {
    return assetId == null ? '—' : `#${assetId}`;
  }

  function parseRewardEpoch(): number | null {
    return parseUnsignedDecimalNumber(rewardEpochInput);
  }

  const snap = $derived(systemStore.snapshot);
  const chainSurface = $derived(
    resolveChainSurfaceState(systemStore.connectionState, snap !== null),
  );
  const stakingChainUnavailable = $derived(
    systemStore.connectionState?.status !== 'connected',
  );
  const nativeStakingCards = $derived.by(() => {
    if (!snap?.nativeStaking.isAvailable) {
      return [];
    }
    const position = snap.nativeStaking.accountPosition;
    const exchangeRate = fixedU128ToFloat(snap.nativeStaking.exchangeRate);
    return [
      {
        label: 'stNTVE rate',
        value: exchangeRate != null ? `${fmt(exchangeRate)} NTVE` : '—',
        detail: 'Per stNTVE receipt',
      },
      {
        label: 'Locked LP',
        value: position ? fmt(toFloat(position.totalLockedLp)) : '—',
        detail: 'Selected account custody',
      },
      {
        label: 'Collator LP',
        value: position ? fmt(toFloat(position.collatorLockedLp)) : '—',
        detail: 'Nomination reward base',
      },
      {
        label: 'LP native value',
        value:
          position?.conservativeNativeValue != null
            ? fmt(toFloat(position.conservativeNativeValue))
            : '—',
        detail: 'Conservative equivalent',
      },
    ];
  });
  const nativeWalletBalances = $derived.by(() => {
    const pool = snap?.nativeStaking.pool;
    const assets = portfolioStore.knownAssets;
    if (!pool) {
      return { native: 0n, staked: 0n, lp: 0n };
    }
    return {
      native:
        assets.find((asset) => asset.assetId === pool.nativeAssetId)?.balance ??
        0n,
      staked:
        assets.find((asset) => asset.assetId === pool.stakedAssetId)?.balance ??
        0n,
      lp:
        assets.find((asset) => asset.assetId === pool.lpAssetId)?.balance ?? 0n,
    };
  });
  const nativeWalletBalanceCards = $derived.by(() => {
    const pool = snap?.nativeStaking.pool;
    if (!pool) {
      return [];
    }
    return [
      {
        label: 'Wallet NTVE',
        value: fmt(toFloat(nativeWalletBalances.native)),
      },
      {
        label: 'Wallet stNTVE',
        value: fmt(toFloat(nativeWalletBalances.staked)),
      },
      {
        label: 'Wallet LP',
        value: fmt(toFloat(nativeWalletBalances.lp)),
      },
    ];
  });
  const rewardEpoch = $derived(parseRewardEpoch());
  const lpAmount = $derived(parseTokenInputAmount(lpAmountInput));
  const governanceCustodyAmount = $derived(
    parseTokenInputAmount(governanceCustodyAmountInput),
  );
  const selectedGovernanceAssetId = $derived.by(() => {
    const pool = snap?.nativeStaking.pool;
    if (!pool) {
      return null;
    }
    return governanceCustodyAssetKind === 'native'
      ? pool.nativeAssetId
      : pool.stakedAssetId;
  });
  const selectedGovernanceAssetLabel = $derived(
    governanceCustodyAssetKind === 'native' ? 'NTVE' : 'stNTVE',
  );
  const stakingSignerUnavailable = $derived(
    walletStore.state.signerStatus !== 'available',
  );
  const nativeStakingPoolUnavailable = $derived(!snap?.nativeStaking.pool);
  const stakingWriteDisabled = $derived(
    stakingActionBusy ||
      stakingChainUnavailable ||
      stakingSignerUnavailable ||
      rewardEpoch === null,
  );
  const compoundDisabled = $derived(
    stakingWriteDisabled || compoundOperatorInput.trim().length === 0,
  );
  const lpAmountActionDisabled = $derived(
    stakingActionBusy ||
      stakingChainUnavailable ||
      stakingSignerUnavailable ||
      nativeStakingPoolUnavailable ||
      lpAmount === null ||
      lpOperatorInput.trim().length === 0,
  );
  const lpWithdrawDisabled = $derived(
    stakingActionBusy ||
      stakingChainUnavailable ||
      stakingSignerUnavailable ||
      lpOperatorInput.trim().length === 0,
  );
  const lpRedelegateDisabled = $derived(
    lpAmountActionDisabled || lpTargetOperatorInput.trim().length === 0,
  );
  const governanceCustodyAmountDisabled = $derived(
    stakingActionBusy ||
      stakingChainUnavailable ||
      stakingSignerUnavailable ||
      nativeStakingPoolUnavailable ||
      governanceCustodyAmount === null,
  );
  const governanceAssetActionDisabled = $derived(
    governanceCustodyAmountDisabled || selectedGovernanceAssetId === null,
  );
  const governanceAssetWithdrawDisabled = $derived(
    stakingActionBusy ||
      stakingChainUnavailable ||
      stakingSignerUnavailable ||
      selectedGovernanceAssetId === null,
  );

  async function runStakingAction(
    action: () => Promise<void>,
    fallbackMessage: string,
  ) {
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      await action();
    } catch (error) {
      stakingActionError =
        error instanceof Error ? error.message : fallbackMessage;
    } finally {
      stakingActionBusy = false;
    }
  }

  async function loadCollatorPositionDetail() {
    const operator = lpOperatorInput.trim();
    if (operator.length === 0) {
      stakingActionError = 'Enter a collator/operator address first';
      return;
    }
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      collatorPositionDetail =
        await systemStore.getNativeCollatorLpPosition(operator);
    } catch (error) {
      stakingActionError =
        error instanceof Error
          ? error.message
          : 'Native collator LP detail load failed';
    } finally {
      stakingActionBusy = false;
    }
  }

  async function loadGovernanceCustodyDetail() {
    const assetId = selectedGovernanceAssetId;
    if (assetId === null) {
      stakingActionError =
        'Wait for the native staking pool view before loading governance custody detail';
      return;
    }
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      governanceCustodyDetail =
        await systemStore.getNativeGovernanceCustodyPosition(assetId);
    } catch (error) {
      stakingActionError =
        error instanceof Error
          ? error.message
          : 'Native governance custody detail load failed';
    } finally {
      stakingActionBusy = false;
    }
  }

  async function loadNominationRewardClaimability() {
    const epoch = rewardEpoch;
    if (epoch === null) {
      stakingActionError = 'Enter a closed reward epoch first';
      return;
    }
    stakingActionBusy = true;
    stakingActionError = null;
    try {
      nominationRewardClaimable =
        await systemStore.getNativeNominationRewardClaimable(epoch);
      nominationRewardClaimableEpoch = epoch;
    } catch (error) {
      stakingActionError =
        error instanceof Error
          ? error.message
          : 'Native nomination reward claimability load failed';
    } finally {
      stakingActionBusy = false;
    }
  }

  function useMaxLpAmount() {
    lpAmountInput = formatTokenInputAmount(nativeWalletBalances.lp);
  }

  function useMaxGovernanceCustodyAmount() {
    const balance =
      governanceCustodyAssetKind === 'native'
        ? nativeWalletBalances.native
        : nativeWalletBalances.staked;
    governanceCustodyAmountInput = formatTokenInputAmount(balance);
  }

  async function claimNominationReward() {
    const epoch = rewardEpoch;
    if (epoch === null) {
      stakingActionError = 'Enter a closed reward epoch first';
      return;
    }
    await runStakingAction(
      () => systemStore.claimNominationReward(epoch),
      'Native nomination claim failed',
    );
  }

  async function compoundNominationReward() {
    const epoch = rewardEpoch;
    if (epoch === null) {
      stakingActionError = 'Enter a closed reward epoch first';
      return;
    }
    await runStakingAction(
      () =>
        systemStore.claimAndCompoundNominationReward(
          epoch,
          compoundOperatorInput,
        ),
      'Native nomination compound failed',
    );
  }

  async function lockNativeLpForCollator() {
    const amount = lpAmount;
    if (amount === null) {
      stakingActionError = 'Enter an LP amount first';
      return;
    }
    await runStakingAction(
      () => systemStore.lockNativeLpForCollator(amount, lpOperatorInput),
      'Native LP collator lock failed',
    );
  }

  async function requestUnlockNativeLp() {
    const amount = lpAmount;
    if (amount === null) {
      stakingActionError = 'Enter an LP amount first';
      return;
    }
    await runStakingAction(
      () => systemStore.requestUnlockNativeLp(lpOperatorInput, amount),
      'Native LP unlock request failed',
    );
  }

  async function withdrawUnlockedNativeLp() {
    await runStakingAction(
      () => systemStore.withdrawUnlockedNativeLp(lpOperatorInput),
      'Native LP withdrawal failed',
    );
  }

  async function redelegateNativeLp() {
    const amount = lpAmount;
    if (amount === null) {
      stakingActionError = 'Enter an LP amount first';
      return;
    }
    await runStakingAction(
      () =>
        systemStore.redelegateNativeLp(
          lpOperatorInput,
          lpTargetOperatorInput,
          amount,
        ),
      'Native LP redelegation failed',
    );
  }

  async function lockNativeLpForGovernance() {
    const amount = governanceCustodyAmount;
    if (amount === null) {
      stakingActionError = 'Enter a governance custody amount first';
      return;
    }
    await runStakingAction(
      () => systemStore.lockNativeLpForGovernance(amount),
      'Native governance LP lock failed',
    );
  }

  async function requestUnlockNativeLpForGovernance() {
    const amount = governanceCustodyAmount;
    if (amount === null) {
      stakingActionError = 'Enter a governance custody amount first';
      return;
    }
    await runStakingAction(
      () => systemStore.requestUnlockNativeLpForGovernance(amount),
      'Native governance LP unlock request failed',
    );
  }

  async function withdrawUnlockedNativeLpForGovernance() {
    await runStakingAction(
      () => systemStore.withdrawUnlockedNativeLpForGovernance(),
      'Native governance LP withdrawal failed',
    );
  }

  async function lockNativeAssetForGovernance() {
    const amount = governanceCustodyAmount;
    const assetId = selectedGovernanceAssetId;
    if (amount === null || assetId === null) {
      stakingActionError =
        'Enter a governance custody amount and wait for the native staking pool view';
      return;
    }
    await runStakingAction(
      () => systemStore.lockNativeAssetForGovernance(assetId, amount),
      'Native governance asset lock failed',
    );
  }

  async function requestUnlockNativeAssetForGovernance() {
    const amount = governanceCustodyAmount;
    const assetId = selectedGovernanceAssetId;
    if (amount === null || assetId === null) {
      stakingActionError =
        'Enter a governance custody amount and wait for the native staking pool view';
      return;
    }
    await runStakingAction(
      () => systemStore.requestUnlockNativeAssetForGovernance(assetId, amount),
      'Native governance asset unlock request failed',
    );
  }

  async function withdrawUnlockedNativeAssetForGovernance() {
    const assetId = selectedGovernanceAssetId;
    if (assetId === null) {
      stakingActionError =
        'Wait for the native staking pool view before withdrawing governance custody';
      return;
    }
    await runStakingAction(
      () => systemStore.withdrawUnlockedNativeAssetForGovernance(assetId),
      'Native governance asset withdrawal failed',
    );
  }
</script>

{#snippet actionSection(title: string, description: string, children: Snippet)}
  <DisclosureSection {title}>
    <p class="action-description text-2xs leading-relaxed text-(--mono-muted)">
      {description}
    </p>
    {@render children()}
  </DisclosureSection>
{/snippet}

{#snippet nominationRewardsSection()}
  <div class="action-fields grid gap-2">
    <NumberInput
      label="Reward epoch"
      bind:value={rewardEpochInput}
      min="0"
      step="1"
      placeholder="Closed epoch"
    />
    <TextField
      label="Compound operator"
      bind:value={compoundOperatorInput}
      placeholder="Collator address for compound lock"
    />
  </div>
  <div class="action-buttons grid gap-2">
    <Button
      size="sm"
      variant="ghost"
      disabled={stakingActionBusy ||
        stakingChainUnavailable ||
        rewardEpoch === null}
      onclick={loadNominationRewardClaimability}>Check claimable</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={stakingWriteDisabled}
      onclick={claimNominationReward}
      >{stakingActionBusy ? 'Submitting...' : 'Claim liquid NTVE'}</Button
    >
    <Button
      size="sm"
      variant="primary"
      disabled={compoundDisabled}
      onclick={compoundNominationReward}
      >{stakingActionBusy ? 'Submitting...' : 'Claim + compound LP'}</Button
    >
  </div>
  {#if nominationRewardClaimableEpoch !== null}
    <Notice variant="muted"
      >Epoch {nominationRewardClaimableEpoch} claimable: {nominationRewardClaimable ===
      null
        ? 'not claimable'
        : `${fmt(toFloat(nominationRewardClaimable))} NTVE`}</Notice
    >
  {/if}
{/snippet}

{#snippet collatorLpSection()}
  <div class="action-fields action-fields-three grid gap-2">
    <NumberInput
      label="LP amount"
      bind:value={lpAmountInput}
      min="0"
      step="any"
      placeholder="0.0"
    />
    <TextField
      label="Operator / source"
      bind:value={lpOperatorInput}
      placeholder="Current collator"
    />
    <TextField
      label="Redelegate target"
      bind:value={lpTargetOperatorInput}
      placeholder="New collator"
    />
  </div>
  {#if nativeStakingPoolUnavailable}
    <Notice variant="warn"
      >The canonical NTVE/stNTVE pool view is required before LP custody writes
      can be submitted.</Notice
    >
  {/if}
  {#if walletStore.state.signerStatus !== 'available'}
    <Notice variant="warn"
      >A signer is required before native staking writes can be submitted.</Notice
    >
  {/if}
  {#if stakingActionError}
    <Notice variant="warn">{stakingActionError}</Notice>
  {/if}
  <div class="action-buttons grid gap-2">
    <Button
      size="sm"
      variant="ghost"
      disabled={nativeWalletBalances.lp <= 0n}
      onclick={useMaxLpAmount}>Use wallet LP max</Button
    >
    <Button
      size="sm"
      variant="ghost"
      disabled={stakingActionBusy ||
        stakingChainUnavailable ||
        lpOperatorInput.trim().length === 0}
      onclick={loadCollatorPositionDetail}>Load operator position detail</Button
    >
  </div>
  {#if collatorPositionDetail}
    <div class="detail-grid grid gap-2">
      <StatCard
        label="Operator LP"
        value={fmt(toFloat(collatorPositionDetail.lockedLp))}
        detail={`LP ${optionalAssetLabel(collatorPositionDetail.lpAssetId)}`}
      />
      <StatCard
        label="Pending unlock"
        value={fmt(toFloat(collatorPositionDetail.pendingUnlockLp))}
        detail={optionalBlockLabel(collatorPositionDetail.pendingUnlockBlock)}
      />
      <StatCard
        label="Operator value"
        value={collatorPositionDetail.conservativeNativeValue == null
          ? '—'
          : fmt(toFloat(collatorPositionDetail.conservativeNativeValue))}
        detail="Conservative NTVE"
      />
    </div>
  {/if}
  <div class="action-buttons grid gap-2">
    <Button
      size="sm"
      variant="primary"
      disabled={lpAmountActionDisabled}
      onclick={lockNativeLpForCollator}
      >{stakingActionBusy ? 'Submitting...' : 'Lock LP'}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={lpAmountActionDisabled}
      onclick={requestUnlockNativeLp}
      >{stakingActionBusy ? 'Submitting...' : 'Request unlock'}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={lpWithdrawDisabled}
      onclick={withdrawUnlockedNativeLp}
      >{stakingActionBusy ? 'Submitting...' : 'Withdraw'}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={lpRedelegateDisabled}
      onclick={redelegateNativeLp}
      >{stakingActionBusy ? 'Submitting...' : 'Redelegate'}</Button
    >
  </div>
{/snippet}

{#snippet governanceCustodySection()}
  <div class="action-fields grid gap-2">
    <NumberInput
      label="Custody amount"
      bind:value={governanceCustodyAmountInput}
      min="0"
      step="any"
      placeholder="0.0"
    />
    <div class="grid gap-1">
      <div class="text-xs text-(--mono-muted)">Asset custody source</div>
      <div class="grid grid-cols-2 gap-2">
        <Button
          size="sm"
          variant={governanceCustodyAssetKind === 'native'
            ? 'primary'
            : 'secondary'}
          onclick={() => (governanceCustodyAssetKind = 'native')}>NTVE</Button
        >
        <Button
          size="sm"
          variant={governanceCustodyAssetKind === 'staked'
            ? 'primary'
            : 'secondary'}
          onclick={() => (governanceCustodyAssetKind = 'staked')}>stNTVE</Button
        >
      </div>
    </div>
  </div>
  <div class="action-buttons grid gap-2">
    <Button
      size="sm"
      variant="ghost"
      disabled={(governanceCustodyAssetKind === 'native'
        ? nativeWalletBalances.native
        : nativeWalletBalances.staked) <= 0n}
      onclick={useMaxGovernanceCustodyAmount}
      >Use wallet {selectedGovernanceAssetLabel} max</Button
    >
    <Button
      size="sm"
      variant="ghost"
      disabled={stakingActionBusy ||
        stakingChainUnavailable ||
        selectedGovernanceAssetId === null}
      onclick={loadGovernanceCustodyDetail}
      >Load NativeVotePower custody detail</Button
    >
  </div>
  {#if governanceCustodyDetail}
    <div class="detail-grid grid gap-2">
      <StatCard
        label="Governance LP"
        value={fmt(toFloat(governanceCustodyDetail.governanceLockedLp))}
        detail={`LP ${optionalAssetLabel(governanceCustodyDetail.lpAssetId)}`}
      />
      <StatCard
        label="Pending LP unlock"
        value={fmt(toFloat(governanceCustodyDetail.pendingGovernanceLpUnlock))}
        detail={optionalBlockLabel(
          governanceCustodyDetail.pendingGovernanceLpUnlockBlock,
        )}
      />
      <StatCard
        label={`${selectedGovernanceAssetLabel} locked`}
        value={fmt(toFloat(governanceCustodyDetail.assetLocked))}
        detail={`Asset #${governanceCustodyDetail.assetId}`}
      />
      <StatCard
        label="Pending asset unlock"
        value={fmt(toFloat(governanceCustodyDetail.pendingAssetUnlock))}
        detail={optionalBlockLabel(
          governanceCustodyDetail.pendingAssetUnlockBlock,
        )}
      />
    </div>
  {/if}
  <div class="action-buttons grid gap-2">
    <Button
      size="sm"
      variant="primary"
      disabled={governanceAssetActionDisabled}
      onclick={lockNativeAssetForGovernance}
      >{stakingActionBusy
        ? 'Submitting...'
        : `Lock ${selectedGovernanceAssetLabel}`}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={governanceAssetActionDisabled}
      onclick={requestUnlockNativeAssetForGovernance}
      >{stakingActionBusy
        ? 'Submitting...'
        : `Unlock ${selectedGovernanceAssetLabel}`}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={governanceAssetWithdrawDisabled}
      onclick={withdrawUnlockedNativeAssetForGovernance}
      >{stakingActionBusy
        ? 'Submitting...'
        : `Withdraw ${selectedGovernanceAssetLabel}`}</Button
    >
  </div>
  <div class="action-buttons grid gap-2">
    <Button
      size="sm"
      variant="primary"
      disabled={governanceCustodyAmountDisabled}
      onclick={lockNativeLpForGovernance}
      >{stakingActionBusy ? 'Submitting...' : 'Lock governance LP'}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={governanceCustodyAmountDisabled}
      onclick={requestUnlockNativeLpForGovernance}
      >{stakingActionBusy ? 'Submitting...' : 'Unlock governance LP'}</Button
    >
    <Button
      size="sm"
      variant="secondary"
      disabled={stakingActionBusy ||
        stakingChainUnavailable ||
        stakingSignerUnavailable}
      onclick={withdrawUnlockedNativeLpForGovernance}
      >{stakingActionBusy ? 'Submitting...' : 'Withdraw governance LP'}</Button
    >
  </div>
{/snippet}

<SectionCard
  title="Staking"
  class="staking-container h-full min-h-0 [container-type:size]"
>
  {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
    <Notice variant="warn" class="grid gap-0.5">
      <strong>{chainSurface.title}</strong>
      <span>{chainSurface.detail}</span>
      {#if chainSurface.status === 'stale'}
        <span>Reconnect before submitting staking or custody operations.</span>
      {/if}
    </Notice>
  {/if}
  {#if nativeStakingCards.length > 0}
    <div class="staking-shell grid gap-3">
      <div class="staking-overview grid gap-2">
        {#each nativeStakingCards as card}
          <StatCard
            label={card.label}
            value={card.value}
            detail={card.detail}
          />
        {/each}
      </div>
      {#if nativeWalletBalanceCards.length > 0}
        <div class="wallet-overview grid gap-2">
          {#each nativeWalletBalanceCards as card}
            <StatCard label={card.label} value={card.value} />
          {/each}
        </div>
      {/if}
      {@render actionSection(
        'Nomination rewards',
        'Claim liquid NTVE or compound a closed epoch into fresh NTVE/stNTVE LP locked to a collator.',
        nominationRewardsSection,
      )}
      {@render actionSection(
        'Collator LP nomination',
        'Lock, unlock, withdraw, or redelegate canonical NTVE/stNTVE LP. Unlock requests stop operator backing immediately and withdraw only after the runtime delay.',
        collatorLpSection,
      )}
      {@render actionSection(
        'NativeVotePower custody',
        'Lock NTVE, stNTVE, or standalone NTVE/stNTVE LP for governance-only NativeVotePower. These locks do not add collator backing or nomination reward base.',
        governanceCustodySection,
      )}
    </div>
  {:else if snap}
    <Notice
      >Native staking runtime views are unavailable in the current metadata or
      the canonical NTVE/stNTVE pool is not initialized.</Notice
    >
  {/if}
</SectionCard>

<style>
  .staking-overview,
  .wallet-overview,
  .detail-grid {
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 9rem), 1fr));
  }
  .action-buttons {
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 8.5rem), 1fr));
  }
  @container (max-height: 224px) {
    .wallet-overview,
    .action-description {
      display: none;
    }
  }
  @container (min-width: 576px) {
    .action-fields {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      align-items: start;
    }
  }
  @container (min-width: 928px) {
    .action-fields-three {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
</style>
