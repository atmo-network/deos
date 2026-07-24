<!--
Domain: Wallet widget
Owns: Portfolio balance display, transfer/action form presentation, and wallet-adjacent execution feedback.
Excludes: Wallet signer discovery, portfolio store ownership, transaction adapter implementation, and layout state.
Zone: Presentation widget; consumes wallet/portfolio/market/system/log stores and UI Kit controls.
-->
<script lang="ts">
  import { Check, Copy } from '@lucide/svelte';

  import { logStore } from '$lib/log/index.svelte';
  import { marketStore } from '$lib/market/index.svelte';
  import {
    type TransferAssetKey,
    portfolioStore,
  } from '$lib/portfolio/index.svelte';
  import {
    chainSurfaceIsBlocking,
    resolveChainSurfaceState,
  } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    BackButton,
    Button,
    Card,
    Icon,
    Notice,
    type NoticeVariant,
    NumberInput,
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

  let walletView = $state<'assets' | 'send'>('assets');
  let sendRecipient = $state('');
  let sendAmount = $state('');
  let sendAsset = $state<TransferAssetKey>('native');
  let sendPendingDraftKey = $state<string | null>(null);
  let sendError = $state<string | null>(null);
  let sendErrorDraftKey = $state<string | null>(null);
  let copied = $state(false);

  const chainSurface = $derived(
    resolveChainSurfaceState(
      systemStore.connectionState,
      systemStore.snapshot !== null,
    ),
  );
  const chainSurfaceBlocked = $derived(chainSurfaceIsBlocking(chainSurface));
  const chainCanWrite = $derived(
    systemStore.snapshot !== null &&
      systemStore.connectionState?.status === 'connected',
  );
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

  const signerBoundaryNotice = $derived.by((): SignerBoundaryNotice | null => {
    if (signerStatus === 'readonly') {
      return {
        variant: 'muted',
        message:
          'This address is valid but watch-only. Transfers require a signer for the same address from either an injected wallet or a built-in local dev preset',
      };
    }
    if (signerStatus !== 'available') {
      return {
        variant: 'muted',
        message:
          'No signer is currently attached to this selected address. Open the sidebar account surface to connect an injected wallet or choose a local dev preset before sending funds',
      };
    }
    return null;
  });
  const knownAssets = $derived.by(() => portfolioStore.knownAssets);
  const selectedTransferAsset = $derived.by(() =>
    portfolioStore.findAsset(sendAsset),
  );
  const sendBalance = $derived(selectedTransferAsset.balance);

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
  const fillMaxAvailable = $derived(chainCanWrite && safeSendBalance > 0n);

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

  function blockedTransferLabel(): string {
    switch (chainSurface.status) {
      case 'stale':
        return 'Reconnect before sending';
      case 'unconfigured':
        return 'Connect a network before sending';
      case 'error':
        return 'Transfer data unavailable';
      default:
        return 'Waiting for balance data';
    }
  }

  const sendButtonState = $derived.by(() => {
    const recipient = sendRecipient.trim();
    if (sendingCurrentDraft) {
      return { disabled: true, text: 'Submitting transfer...' };
    }
    if (sendPendingDraftKey !== null) {
      return { disabled: true, text: 'Previous transfer still in progress' };
    }
    if (!chainCanWrite) {
      return { disabled: true, text: blockedTransferLabel() };
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
  async function submitTransfer(): Promise<void> {
    if (sendButtonState.disabled || !chainCanWrite) {
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

  function openSend(asset: TransferAssetKey): void {
    sendAsset = asset;
    sendError = null;
    sendErrorDraftKey = null;
    walletView = 'send';
  }

  function closeSend(): void {
    walletView = 'assets';
  }

  function fillMax(): void {
    sendAmount = fmtInputAmount(toFloat(safeSendBalance));
  }
</script>

<Card class="h-full min-h-full flex flex-col">
  <div
    class="wallet-container grid h-full min-h-0 gap-3 p-3 text-xs [container-type:size]"
  >
    {#if walletView === 'assets'}
      <div
        class="wallet-assets grid w-full max-w-3xl justify-self-center content-start gap-3"
      >
        <section class="grid gap-2.5 rounded-xl bg-(--mono-bg) p-3">
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 grid gap-1">
              <div class="flex min-w-0 items-center gap-2">
                <strong class="truncate text-sm text-(--mono-text)"
                  >{selectedLabel}</strong
                >
                <span
                  class="shrink-0 rounded-full bg-(--mono-bg) px-2 py-0.5 text-3xs font-medium text-(--mono-muted)"
                >
                  {accountModeLabel}
                </span>
              </div>
              <div class="break-all font-mono text-2xs text-(--mono-muted)">
                {selectedAddress || 'No account selected'}
              </div>
            </div>
            <Button
              size="icon"
              variant="ghost"
              onclick={copyAddress}
              disabled={!selectedAddress}
              label={copied ? 'Address copied' : 'Copy receive address'}
              class="shrink-0 rounded-lg bg-(--mono-bg) text-(--mono-muted) hover:text-(--mono-text)"
            >
              {#if copied}
                <Icon icon={Check} size="sm" />
              {:else}
                <Icon icon={Copy} size="sm" />
              {/if}
            </Button>
          </div>
          {#if signerBoundaryNotice}
            <Notice variant={signerBoundaryNotice.variant}
              >{signerBoundaryNotice.message}</Notice
            >
          {/if}
        </section>

        {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
          <Notice variant="warn" class="grid gap-0.5">
            <strong>{chainSurface.title}</strong>
            <span>{chainSurface.detail}</span>
            {#if chainSurface.status === 'stale'}
              <span>Reconnect before sending assets.</span>
            {/if}
          </Notice>
        {/if}

        {#if !chainSurfaceBlocked}
          <section class="grid gap-2.5 rounded-xl bg-(--mono-bg) p-3">
            <div class="flex items-center justify-between gap-2">
              <span
                class="text-2xs font-semibold uppercase tracking-wider text-(--mono-muted)"
                >Balances</span
              >
            </div>
            <div class="grid gap-1">
              {#each knownAssets as asset}
                <div
                  class="asset-row grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-xl bg-white px-3 py-2"
                >
                  <span class="truncate font-medium text-(--mono-text)">
                    {asset.symbol}{!asset.isCanonical ? '*' : ''}
                  </span>
                  <div class="flex min-w-0 items-center justify-end gap-3">
                    <strong
                      class="min-w-0 truncate text-sm tabnum text-(--mono-text)"
                    >
                      {fmt(toFloat(asset.balance))}
                    </strong>
                    <Button
                      size="sm"
                      variant="ghost"
                      class="shrink-0 text-(--mono-purple)"
                      disabled={!asset.transferKey}
                      onclick={() =>
                        asset.transferKey && openSend(asset.transferKey)}
                    >
                      Send
                    </Button>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      </div>
    {:else}
      <section
        class="send-panel grid w-full max-w-[56rem] justify-self-center content-start gap-3 rounded-xl bg-(--mono-bg) p-3"
      >
        <div class="flex min-w-0 items-center gap-2">
          <BackButton onclick={closeSend} label="Back to balances" />
          <div class="min-w-0">
            <div
              class="text-2xs font-semibold uppercase tracking-wider text-(--mono-muted)"
            >
              Send asset
            </div>
            <div class="truncate text-base font-semibold text-(--mono-text)">
              {selectedTransferAsset.symbol}
            </div>
          </div>
        </div>

        {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
          <Notice variant="warn" class="grid gap-0.5">
            <strong>{chainSurface.title}</strong>
            <span>{chainSurface.detail}</span>
            <span
              >Recipient and amount stay local until a transfer is submitted.</span
            >
          </Notice>
        {/if}

        <div
          class="flex items-center justify-between gap-3 rounded-lg bg-(--mono-bg) px-2.5 py-2 text-2xs"
        >
          <span class="text-(--mono-muted)"
            >{nativeTransfer ? 'Spendable' : 'Available'}</span
          >
          <span class="tabnum font-medium text-(--mono-text)">
            {systemStore.snapshot ? fmt(toFloat(spendableSendBalance)) : '—'}
            {selectedTransferAsset.symbol}
          </span>
        </div>

        <div class="send-fields grid gap-3">
          <TextField
            label="Recipient"
            bind:value={sendRecipient}
            placeholder="5..."
          />
          <div class="grid gap-1">
            <NumberInput
              label="Amount"
              bind:value={sendAmount}
              min="0"
              placeholder="0.00"
              step="any"
            />
            <Button
              size="sm"
              variant="ghost"
              onclick={fillMax}
              class={fillMaxAvailable
                ? 'justify-self-end px-0 py-0 text-2xs text-(--mono-purple) hover:underline'
                : 'justify-self-end px-0 py-0 text-2xs text-(--mono-muted) opacity-60'}
              disabled={!fillMaxAvailable}
            >
              {nativeTransfer ? 'Use safe max' : 'Use max'}
            </Button>
          </div>
        </div>

        {#if sendError}
          <Notice variant="warn">{sendError}</Notice>
        {/if}
        <Button
          variant="primary"
          class={sendButtonState.disabled ? 'w-full opacity-50' : 'w-full'}
          onclick={submitTransfer}
          disabled={sendButtonState.disabled}
        >
          {sendButtonState.text}
        </Button>
      </section>
    {/if}
  </div>
</Card>

<style>
  @container (max-width: 352px) {
    .send-panel,
    section {
      padding: calc(var(--spacing) * 2);
    }
  }
  @container (min-width: 576px) {
    .send-fields {
      grid-template-columns: minmax(0, 1.25fr) minmax(
          calc(var(--widget-em) * 10.6667),
          0.75fr
        );
      align-items: start;
    }
  }
</style>
