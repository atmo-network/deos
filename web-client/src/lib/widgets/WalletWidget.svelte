<!--
Domain: Wallet widget
Owns: Portfolio balance display, transfer/action form presentation, and wallet-adjacent execution feedback.
Excludes: Wallet signer discovery, portfolio store ownership, transaction adapter implementation, and layout state.
Zone: Presentation widget; consumes wallet/portfolio/market/system/log stores and UI Kit controls.
-->
<script lang="ts">
  import { Check, Copy } from '@lucide/svelte';
  import { onMount } from 'svelte';

  import { logStore } from '$lib/log/index.svelte';
  import { marketStore } from '$lib/market/index.svelte';
  import {
    type TransferAssetKey,
    portfolioStore,
  } from '$lib/portfolio/index.svelte';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Button,
    Card,
    Notice,
    type NoticeVariant,
    NumberInput,
    ReadModelBadge,
    SectionCard,
    SelectableTile,
    StatCard,
    TextField,
  } from '$lib/ui';
  import {
    fmt,
    fmtInputAmount,
    parseTokenInputAmount,
    toFloat,
  } from '$lib/ui/format';
  import { isValidDeosAddress, walletStore } from '$lib/wallet/index.svelte';

  const NATIVE_TRANSFER_FEE_HEADROOM = 100_000_000_000n;

  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let sendRecipient = $state('');
  let sendAmount = $state('');
  let sendAsset = $state<TransferAssetKey>('native');
  let sendPendingDraftKey = $state<string | null>(null);
  let sendError = $state<string | null>(null);
  let sendErrorDraftKey = $state<string | null>(null);
  let copied = $state(false);

  const signerStatus = $derived(walletStore.state.signerStatus);
  const selectedLabel = $derived(walletStore.state.selectedLabel);
  const selectedAddress = $derived(walletStore.state.selectedAddress);
  const selectedSource = $derived(walletStore.state.selectedSource);
  const accountModeLabel = $derived.by(() => {
    if (signerStatus === 'readonly') {
      return 'Watch-only';
    }
    if (selectedSource === 'injected') {
      return 'Injected';
    }
    if (selectedSource === 'dev') {
      return 'Local dev';
    }
    return 'Custom';
  });
  type SignerBoundaryNotice = {
    variant: NoticeVariant;
    message: string;
  };

  const signerBoundaryNotice = $derived.by((): SignerBoundaryNotice => {
    if (signerStatus === 'readonly') {
      return {
        variant: 'muted',
        message:
          'This address is valid but watch-only. Transfers require a signer for the same address from either an injected wallet or a built-in local dev preset',
      };
    }
    if (selectedSource === 'dev') {
      return {
        variant: 'muted',
        message:
          'This wallet is using an in-browser local dev signer. Treat it as a local Zombienet/testing convenience, not a general production wallet path',
      };
    }
    if (selectedSource === 'injected') {
      return {
        variant: 'muted',
        message:
          'This wallet is using an injected browser signer. Transfers and other live writes will prompt through the connected extension for this same address',
      };
    }
    if (signerStatus !== 'available') {
      return {
        variant: 'muted',
        message:
          'No signer is currently attached to this selected address. Open the sidebar account surface to connect an injected wallet or choose a local dev preset before sending funds',
      };
    }
    return {
      variant: 'muted',
      message:
        'A signer is available for this account through the current wallet connection path',
    };
  });
  const knownAssets = $derived.by(() => portfolioStore.knownAssets);
  const knownAssetsProvenance = $derived(
    portfolioStore.knownAssetsView.provenance,
  );
  const transferAssets = $derived.by(() => portfolioStore.transferAssets);
  const selectedTransferAsset = $derived.by(() =>
    portfolioStore.findAsset(sendAsset),
  );
  const sendBalance = $derived(selectedTransferAsset.balance);

  $effect(() => {
    if (transferAssets.some((asset) => asset.transferKey === sendAsset)) {
      return;
    }
    const fallbackTransferKey = transferAssets[0]?.transferKey;
    if (fallbackTransferKey) {
      sendAsset = fallbackTransferKey;
    }
  });
  const nativeTransfer = $derived(sendAsset === 'native');
  const safeSendBalance = $derived.by(() => {
    if (!nativeTransfer) {
      return sendBalance;
    }
    return sendBalance > NATIVE_TRANSFER_FEE_HEADROOM
      ? sendBalance - NATIVE_TRANSFER_FEE_HEADROOM
      : 0n;
  });
  const spendableSendBalance = $derived(
    nativeTransfer ? safeSendBalance : sendBalance,
  );
  const fillMaxAvailable = $derived(safeSendBalance > 0n);
  const compactPane = $derived(viewport.width > 0 && viewport.width < 430);
  const densePane = $derived(viewport.width > 0 && viewport.width < 340);

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

  function shortAddress(address: string): string {
    if (address.length <= 18) {
      return address;
    }
    return `${address.slice(0, 8)}…${address.slice(-8)}`;
  }

  async function copyAddress(): Promise<void> {
    if (typeof navigator === 'undefined' || !selectedAddress) {
      return;
    }
    await navigator.clipboard.writeText(selectedAddress);
    copied = true;
    setTimeout(() => {
      copied = false;
    }, 1200);
  }

  const sendDraftKey = $derived(
    `${selectedAddress ?? ''}|${selectedSource}|${signerStatus}|${sendAsset}|${sendRecipient}|${sendAmount}`,
  );
  const sendingCurrentDraft = $derived(sendPendingDraftKey === sendDraftKey);
  const parsedSendAmount = $derived.by(() => {
    const amount = parseTokenInputAmount(sendAmount);
    return { amount };
  });

  $effect(() => {
    if (!sendError || !sendErrorDraftKey) {
      return;
    }
    if (sendDraftKey === sendErrorDraftKey) {
      return;
    }
    sendError = null;
    sendErrorDraftKey = null;
  });

  const sendButtonState = $derived.by(() => {
    const recipient = sendRecipient.trim();
    if (sendingCurrentDraft) {
      return { disabled: true, text: 'Submitting transfer...' };
    }
    if (sendPendingDraftKey !== null) {
      return { disabled: true, text: 'Previous transfer still in progress' };
    }
    if (signerStatus === 'readonly') {
      return { disabled: true, text: 'Selected account is watch-only' };
    }
    if (signerStatus !== 'available') {
      return {
        disabled: true,
        text:
          walletStore.state.signerMessage ===
          'No injected wallet extension detected'
            ? 'No injected wallet extension detected'
            : 'Open the sidebar and connect a signer',
      };
    }
    if (spendableSendBalance === 0n) {
      return {
        disabled: true,
        text: `No spendable ${selectedTransferAsset.symbol} balance available`,
      };
    }
    if (recipient.length === 0) {
      return { disabled: true, text: 'Enter recipient' };
    }
    if (!isValidDeosAddress(recipient)) {
      return { disabled: true, text: 'Enter valid recipient' };
    }
    if (parsedSendAmount.amount == null) {
      return { disabled: true, text: 'Enter amount' };
    }
    if (parsedSendAmount.amount > sendBalance) {
      return {
        disabled: true,
        text: `Insufficient ${selectedTransferAsset.symbol}`,
      };
    }
    if (nativeTransfer && parsedSendAmount.amount > safeSendBalance) {
      return {
        disabled: true,
        text: `Exceeds safe max ${selectedTransferAsset.symbol}`,
      };
    }
    return { disabled: false, text: `Send ${selectedTransferAsset.symbol}` };
  });
  const sendGuidance = $derived.by((): SignerBoundaryNotice | null => {
    const recipient = sendRecipient.trim();
    if (sendError || sendingCurrentDraft) {
      return null;
    }
    if (sendPendingDraftKey !== null) {
      return {
        variant: 'muted',
        message:
          'A transfer from the previous draft is still in progress. Wait for it to finish before submitting a new draft',
      };
    }
    if (signerStatus === 'readonly') {
      return {
        variant: 'muted',
        message:
          'Selected account is watch-only. Open the sidebar and connect an injected signer for the same address before sending funds',
      };
    }
    if (signerStatus !== 'available') {
      return {
        variant: 'muted',
        message:
          walletStore.state.signerMessage ===
          'No injected wallet extension detected'
            ? 'No injected wallet extension is available in this browser. Use a built-in local dev signer or install a supported wallet extension first'
            : 'Open the sidebar and connect an injected wallet or local dev signer before sending funds',
      };
    }
    if (recipient.length > 0 && !isValidDeosAddress(recipient)) {
      return {
        variant: 'warn',
        message:
          'Recipient must be a valid DEOS ss58 address before the transfer can be signed',
      };
    }
    if (sendAmount.trim().length > 0 && parsedSendAmount.amount == null) {
      return {
        variant: 'warn',
        message: `Enter a positive ${selectedTransferAsset.symbol} amount to prepare the transfer`,
      };
    }
    if (spendableSendBalance === 0n) {
      return {
        variant: 'muted',
        message: `No spendable ${selectedTransferAsset.symbol} balance is currently available for transfer`,
      };
    }
    if (
      parsedSendAmount.amount != null &&
      parsedSendAmount.amount > sendBalance
    ) {
      return {
        variant: 'warn',
        message: `Selected amount exceeds the available ${selectedTransferAsset.symbol} balance`,
      };
    }
    if (
      nativeTransfer &&
      parsedSendAmount.amount != null &&
      parsedSendAmount.amount > safeSendBalance
    ) {
      return {
        variant: 'warn',
        message: `Selected amount exceeds the spendable ${selectedTransferAsset.symbol} balance after fee headroom`,
      };
    }
    return null;
  });

  async function submitTransfer(): Promise<void> {
    if (sendButtonState.disabled) {
      return;
    }
    const submittedDraftKey = sendDraftKey;
    const submittedRecipient = sendRecipient.trim();
    const submittedAsset = sendAsset;
    sendPendingDraftKey = submittedDraftKey;
    sendError = null;
    sendErrorDraftKey = null;
    try {
      if (parsedSendAmount.amount == null) {
        throw new Error('Transfer amount must be greater than zero');
      }
      const submittedAmount = parsedSendAmount.amount;
      const submittedSymbol = selectedTransferAsset.symbol;
      if (submittedAmount > sendBalance) {
        throw new Error(
          `Selected amount exceeds the available ${submittedSymbol} balance`,
        );
      }
      if (nativeTransfer && submittedAmount > safeSendBalance) {
        throw new Error(
          `Selected amount exceeds the spendable ${submittedSymbol} balance after fee headroom`,
        );
      }
      await portfolioStore.transferAsset(
        submittedAsset,
        submittedRecipient,
        submittedAmount,
      );
      logStore.add(
        `TRANSFER ${fmt(toFloat(submittedAmount))} ${submittedSymbol} → ${shortAddress(submittedRecipient)}`,
        'info',
      );
      if (sendDraftKey === submittedDraftKey) {
        sendAmount = '';
        sendRecipient = '';
      }
    } catch (error) {
      const message =
        error instanceof Error ? error.message : 'Transfer failed';
      if (sendDraftKey === submittedDraftKey) {
        sendError = message;
        sendErrorDraftKey = submittedDraftKey;
      }
      logStore.add(message, 'error', {
        blockNumber: systemStore.snapshot?.blockNumber ?? null,
        step: marketStore.history.length - 1,
      });
    } finally {
      if (sendPendingDraftKey === submittedDraftKey) {
        sendPendingDraftKey = null;
      }
    }
  }

  function fillMax(): void {
    sendAmount = fmtInputAmount(toFloat(safeSendBalance));
  }

  onMount(() => {
    syncViewport();
    if (!rootEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<Card class="min-h-full flex flex-col">
  <div bind:this={rootEl} class="h-full flex flex-col min-h-0">
    <div class="@container grid gap-3 p-3 text-xs">
      <SectionCard
        title="Selected account"
        subtitle="Chosen in the sidebar account widget"
      >
        {#snippet actions()}
          <Button
            size="sm"
            variant="secondary"
            onclick={copyAddress}
            class="inline-flex items-center gap-1 rounded-lg bg-(--mono-bg) px-2 py-1 text-[10px] text-(--mono-muted) hover:text-(--mono-text)"
          >
            {#if copied}
              <Check size={12} />
              Copied
            {:else}
              <Copy size={12} />
              Copy
            {/if}
          </Button>
        {/snippet}
        <div
          class={['grid gap-2', densePane ? 'grid-cols-1' : '@md:grid-cols-3']}
        >
          <StatCard label="Account" value={selectedLabel} />
          <StatCard
            label="Mode"
            value={accountModeLabel}
            detail={signerStatus === 'available'
              ? 'signing ready'
              : signerStatus === 'readonly'
                ? 'valid address, no signer'
                : 'signer unavailable'}
          />
          <StatCard
            label="Receive address"
            value={densePane
              ? selectedAddress
                ? shortAddress(selectedAddress)
                : '—'
              : selectedAddress || '—'}
            detail={densePane && selectedAddress ? selectedAddress : undefined}
            class="break-all"
          />
        </div>
        <Notice variant={signerBoundaryNotice.variant}
          >{signerBoundaryNotice.message}</Notice
        >
      </SectionCard>

      <SectionCard
        title="Known client assets"
        subtitle="Bounded current-session wallet asset surface"
      >
        {#snippet actions()}
          <ReadModelBadge provenance={knownAssetsProvenance} />
        {/snippet}
        <div
          class={[
            'grid gap-2',
            compactPane ? 'grid-cols-1' : '@lg:grid-cols-2',
          ]}
        >
          {#each knownAssets as asset}
            <StatCard
              label={asset.assetId !== null
                ? `${asset.kind} #${asset.assetId}`
                : 'Native asset'}
              value={`${asset.symbol}${!asset.isCanonical ? '*' : ''}`}
              detail={[
                fmt(toFloat(asset.balance)),
                asset.isPrimaryRouteAsset ? 'primary route' : null,
              ]
                .filter(Boolean)
                .join(' · ')}
            />
          {/each}
        </div>
      </SectionCard>

      <SectionCard title="Send assets" subtitle="Receipts appear in Log">
        {#snippet actions()}
          <ReadModelBadge provenance={knownAssetsProvenance} />
        {/snippet}
        <div
          class={[
            'grid gap-3',
            !compactPane && '@lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]',
          ]}
        >
          <div class="grid gap-2">
            <div
              class={[
                compactPane
                  ? 'grid grid-flow-col auto-cols-[minmax(7.5rem,1fr)] gap-2 overflow-x-auto pb-1 scrollbar-none'
                  : 'grid gap-2',
              ]}
            >
              {#each transferAssets as asset}
                <SelectableTile
                  onclick={() =>
                    asset.transferKey && (sendAsset = asset.transferKey)}
                  selected={sendAsset === asset.transferKey}
                >
                  <div class="font-medium text-(--mono-text)">
                    {asset.symbol}
                  </div>
                  <div class="text-[10px] text-(--mono-muted)">
                    {asset.kind}
                  </div>
                </SelectableTile>
              {/each}
            </div>
            <StatCard
              label="Selected asset"
              value={selectedTransferAsset.symbol}
              detail={selectedTransferAsset.assetId !== null
                ? `${selectedTransferAsset.kind} #${selectedTransferAsset.assetId}`
                : selectedTransferAsset.kind}
            />
            <StatCard
              label={nativeTransfer ? 'Spendable now' : 'Available'}
              value={fmt(
                toFloat(nativeTransfer ? safeSendBalance : sendBalance),
              )}
              detail={nativeTransfer
                ? `Leaves ${fmt(toFloat(sendBalance - safeSendBalance))} ${selectedTransferAsset.symbol} for fees`
                : undefined}
            />
          </div>

          <div class="grid gap-3">
            <TextField
              label="Recipient"
              bind:value={sendRecipient}
              placeholder="5..."
            />
            <NumberInput
              label="Amount"
              bind:value={sendAmount}
              min="0"
              placeholder="0.00"
              step="any"
              helper={nativeTransfer
                ? safeSendBalance > 0n
                  ? `Safe max ${fmt(toFloat(safeSendBalance))} · leaves ${fmt(toFloat(sendBalance - safeSendBalance))} ${selectedTransferAsset.symbol} for fees`
                  : `No spendable ${selectedTransferAsset.symbol} remains after fee headroom`
                : `Max ${fmt(toFloat(sendBalance))}`}
            />
            <div class="flex justify-end">
              <Button
                size="sm"
                variant="ghost"
                onclick={fillMax}
                class={fillMaxAvailable
                  ? 'px-0 py-0 text-[10px] text-(--mono-purple) hover:underline'
                  : 'px-0 py-0 text-[10px] text-(--mono-muted) opacity-60'}
                disabled={!fillMaxAvailable}
              >
                {nativeTransfer ? 'Fill safe max' : 'Fill max'}
              </Button>
            </div>
            {#if sendError}
              <Notice variant="warn">{sendError}</Notice>
            {:else if sendGuidance}
              <Notice variant={sendGuidance.variant}
                >{sendGuidance.message}</Notice
              >
            {/if}
            <Button
              variant="primary"
              class={sendButtonState.disabled ? 'w-full opacity-50' : 'w-full'}
              onclick={submitTransfer}
              disabled={sendButtonState.disabled}
            >
              {sendButtonState.text}
            </Button>
          </div>
        </div>
      </SectionCard>
    </div>
  </div>
</Card>
