<!--
Domain: Swap widget
Owns: Swap form composition, quote display, route diagnostics, and swap execution controls.
Excludes: Market store ownership, adapter transport, asset registry logic, and UI Kit primitive implementation.
Zone: Presentation widget; consumes market/portfolio/system state and UI Kit without importing concrete chain internals.
-->
<script lang="ts">
  import { ArrowUpDown } from '@lucide/svelte';
  import { onMount } from 'svelte';

  import { parseUnsignedDecimalFloat } from '$lib/format/numeric';
  import { logStore } from '$lib/log/index.svelte';
  import { marketStore } from '$lib/market/index.svelte';
  import type { Quote } from '$lib/market/types';
  import { portfolioStore } from '$lib/portfolio/index.svelte';
  import type { ReadModelProvenance } from '$lib/read-model';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Badge,
    Button,
    Card,
    IconButton,
    Notice,
    NumberInput,
    ReadModelBadge,
    RichSelect,
    StatCard,
  } from '$lib/ui';
  import {
    fmt,
    fmtInputAmount,
    fmtOut,
    fmtPrice,
    parseTokenInputAmount,
    toFloat,
  } from '$lib/ui/format';
  import { walletStore } from '$lib/wallet/index.svelte';

  type SwapAssetValue = 'native' | 'foreign';
  type SwapAssetOption = {
    value: SwapAssetValue;
    badge: string;
    badgeClass: string;
    balance: bigint;
    label: string;
  };
  type SwapSelectItem = {
    value: string;
    label: string;
    badge: string;
    badgeClass: string;
    detail: string;
  };
  type QuoteStatView = {
    label: string;
    value: string;
  };
  type CompactQuoteStripView = {
    facts: string[];
    provenance: ReadModelProvenance | null;
    route: string | null;
    visible: boolean;
  };
  type AssetSelectView = {
    dense: boolean;
    items: SwapSelectItem[];
    label: string;
    onValueChange: (value: string) => void;
    selectedValue: SwapAssetValue;
    surfaceClass: string;
  };
  type SwapInputLegView = {
    amountValue: string;
    assetSelect: AssetSelectView;
    balanceLabel: string;
    balanceText: string;
    dense: boolean;
    fillMaxEnabled: boolean;
    onAmountInput: (value: string) => void;
    onFillMax: () => void;
  };
  type SwapOutputLegView = {
    amountText: string;
    assetSelect: AssetSelectView;
    balanceText: string;
    dense: boolean;
  };
  type DiagnosticsPanelView = {
    compact: boolean;
    dense: boolean;
    onSlippageInput: (value: string) => void;
    provenance: ReadModelProvenance | null;
    slippageLabel: string;
    slippageValue: string;
    stats: QuoteStatView[];
    warnings: string[];
  };
  type ParsedInputState = {
    amount: bigint | null;
  };

  const NATIVE_SWAP_FEE_HEADROOM = 100_000_000_000n;

  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0 });
  let inputValue = $state('');
  let slippagePercent = $state('0.50');
  let submitting = $state(false);
  let currentQuote = $state<Quote | null>(null);
  let quoteLoading = $state(false);
  let quoteRequestId = 0;

  function nativeSymbol() {
    return systemStore.snapshot?.nativeAsset.symbol ?? 'NTVE';
  }

  function foreignSymbol() {
    return systemStore.snapshot?.foreignAsset.symbol ?? 'FOREIGN';
  }

  function foreignAssetIsCanonical() {
    return systemStore.snapshot?.foreignAsset.isCanonical ?? false;
  }

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0 };
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
    };
  }

  function parseAssetValue(value: string): SwapAssetValue | null {
    if (value === 'native' || value === 'foreign') {
      return value;
    }
    return null;
  }

  function nextDirectionForInput(value: SwapAssetValue): 'buy' | 'sell' {
    return value === 'foreign' ? 'buy' : 'sell';
  }

  function nextDirectionForOutput(value: SwapAssetValue): 'buy' | 'sell' {
    return value === 'native' ? 'buy' : 'sell';
  }

  function applyDirection(nextDirection: 'buy' | 'sell') {
    if (marketStore.direction === nextDirection) {
      return;
    }
    marketStore.flipDirection();
    inputValue = '';
  }

  function selectInputAsset(value: string) {
    const asset = parseAssetValue(value);
    if (!asset) {
      return;
    }
    applyDirection(nextDirectionForInput(asset));
  }

  function selectOutputAsset(value: string) {
    const asset = parseAssetValue(value);
    if (!asset) {
      return;
    }
    applyDirection(nextDirectionForOutput(asset));
  }

  function inputEventValue(event: Event) {
    const target = event.currentTarget;
    return target instanceof HTMLInputElement ? target.value : '';
  }

  function updateInputValue(value: string) {
    inputValue = value;
  }

  function updateSlippagePercent(value: string) {
    slippagePercent = value;
  }

  function toSwapSelectItem(asset: SwapAssetOption) {
    return {
      value: asset.value,
      label: asset.label,
      badge: asset.badge,
      badgeClass: asset.badgeClass,
      detail: `Balance: ${fmt(toFloat(asset.balance))}`,
    };
  }

  function flipTokens() {
    applyDirection(marketStore.direction === 'buy' ? 'sell' : 'buy');
  }

  function setMax() {
    inputValue = fmtInputAmount(toFloat(safeInputBalance));
  }

  function parseInputState(value: string) {
    const amount = parseTokenInputAmount(value);
    return { amount };
  }

  function assetOptionsValue() {
    const nativeOption: SwapAssetOption = {
      value: 'native',
      badge: '◆',
      badgeClass: 'bg-(--mono-green)/20 text-(--mono-green)',
      balance: portfolioStore.userBalance.native,
      label: nativeSymbol(),
    };
    const foreignOption: SwapAssetOption = {
      value: 'foreign',
      badge: String.fromCharCode(36),
      badgeClass: 'bg-(--mono-orange)/20 text-(--mono-orange)',
      balance: portfolioStore.userBalance.foreign,
      label: foreignSymbol() + (foreignAssetIsCanonical() ? '' : '*'),
    };
    return [nativeOption, foreignOption];
  }

  function safeInputBalanceValue() {
    if (isBuy) {
      return inBalance;
    }
    return inBalance > NATIVE_SWAP_FEE_HEADROOM
      ? inBalance - NATIVE_SWAP_FEE_HEADROOM
      : 0n;
  }

  function slippageStateValue() {
    const raw = parseUnsignedDecimalFloat(slippagePercent);
    if (raw === null) {
      return {
        valid: false,
        percent: null,
        bps: null,
        label: '—',
        reason: 'Enter slippage 0.01-50%',
      };
    }
    if (raw < 0.01 || raw > 50) {
      return {
        valid: false,
        percent: raw,
        bps: null,
        label: `${raw}%`,
        reason: 'Slippage must stay between 0.01% and 50%',
      };
    }
    return {
      valid: true,
      percent: raw,
      bps: Math.round(raw * 100),
      label: `${raw.toFixed(raw < 1 ? 2 : raw < 10 ? 1 : 0)}%`,
      reason: null,
    };
  }

  function buttonStateValue() {
    const snapshot = systemStore.snapshot;
    if (submitting) {
      return { text: 'Submitting swap...', disabled: true };
    }
    if (!snapshot) {
      return { text: 'Waiting for chain state', disabled: true };
    }
    if (!snapshot.hasNativeCurve) {
      return { text: 'Native curve not bootstrapped', disabled: true };
    }
    if (snapshot.trackedForeignAssetCount === 0) {
      return { text: 'No foreign collateral registered', disabled: true };
    }
    if (walletStore.state.signerStatus === 'readonly') {
      return { text: 'Selected account is watch-only', disabled: true };
    }
    if (walletStore.state.signerStatus !== 'available') {
      return {
        text:
          walletStore.state.signerMessage ===
          'No injected wallet extension detected'
            ? 'No injected wallet extension detected'
            : 'Open the sidebar and connect a signer',
        disabled: true,
      };
    }
    if (safeInputBalance === 0n) {
      return {
        text: `No spendable ${inputSymbol} balance available`,
        disabled: true,
      };
    }
    if (!parsedInput.amount) {
      return { text: 'Enter an amount', disabled: true };
    }
    if (!slippageState.valid) {
      return { text: slippageState.reason, disabled: true };
    }
    if (isBuy && parsedInput.amount < snapshot.minForeignSwapAmount) {
      return {
        text: `Minimum buy is ${fmt(toFloat(snapshot.minForeignSwapAmount))} ${foreignSymbol()}`,
        disabled: true,
      };
    }
    if (parsedInput.amount > inBalance) {
      return {
        text: `Insufficient ${inputSymbol} balance`,
        disabled: true,
      };
    }
    if (!isBuy && parsedInput.amount > safeInputBalance) {
      return {
        text: `Leave some ${nativeSymbol()} for fees`,
        disabled: true,
      };
    }
    if (!isBuy && !snapshot.hasPool) {
      return { text: 'Pool not initialized yet', disabled: true };
    }
    if (quoteLoading) {
      return { text: 'Fetching quote...', disabled: true };
    }
    if (!currentQuote) {
      return { text: 'No route available', disabled: true };
    }
    return { text: `Swap ${inputSymbol} for ${outputSymbol}`, disabled: false };
  }

  function routeInfoValue() {
    const snapshot = systemStore.snapshot;
    if (!snapshot) {
      return null;
    }
    const last =
      marketStore.history.length > 0
        ? marketStore.history[marketStore.history.length - 1]
        : null;
    const tmcPrice = last?.priceEffTMC ?? 0;
    const xykPrice = last?.priceXYK ?? 0;
    const routerPrice = last?.priceRouter ?? null;
    const bestRoute =
      last?.routeRouter ??
      (xykPrice > 0 && xykPrice < tmcPrice ? 'XYK' : 'TMC');
    return { tmcPrice, xykPrice, routerPrice, bestRoute };
  }

  function minimumReceivedValue() {
    if (!currentQuote || !slippageState.valid || slippageState.bps === null) {
      return null;
    }
    return currentQuote.out > 0n
      ? (currentQuote.out * BigInt(Math.max(0, 10_000 - slippageState.bps))) /
          10_000n
      : 0n;
  }

  function compactQuoteFactsValue() {
    if (!currentQuote) {
      return [];
    }
    return [
      `${fmtPrice(currentQuote.effectivePrice)}`,
      `fee ${fmt(toFloat(currentQuote.fee))} ${inputSymbol}`,
      `min ${minimumReceived !== null ? fmtOut(toFloat(minimumReceived)) : '—'} ${outputSymbol}`,
    ];
  }

  function quoteStatsValue() {
    return [
      {
        label: 'Route',
        value: currentQuote
          ? currentQuote.route
          : (routeInfo?.bestRoute ?? '—'),
      },
      {
        label: 'Rate',
        value: currentQuote
          ? `${fmtPrice(currentQuote.effectivePrice)}`
          : routeInfo?.routerPrice
            ? `${fmtPrice(routeInfo.routerPrice)}`
            : '—',
      },
      {
        label: 'Input fee',
        value: currentQuote
          ? `${fmt(toFloat(currentQuote.fee))} ${inputSymbol}`
          : '—',
      },
      {
        label: 'Min receive',
        value:
          minimumReceived !== null
            ? `${fmtOut(toFloat(minimumReceived))} ${outputSymbol}`
            : '—',
      },
    ];
  }

  function warningsValue() {
    const snapshot = systemStore.snapshot;
    const items: string[] = [];
    if (!snapshot) {
      return items;
    }
    if (!snapshot.hasNativeCurve) {
      items.push('Local chain has no native curve yet');
    }
    if (walletStore.state.signerStatus === 'readonly') {
      items.push(
        'Selected account is watch-only. Open the sidebar and connect an injected signer for the same address before submitting a live swap',
      );
    } else if (walletStore.state.signerStatus !== 'available') {
      items.push(
        walletStore.state.signerMessage ===
          'No injected wallet extension detected'
          ? 'No injected wallet extension is available in this browser. Use a built-in local dev signer or install a supported wallet extension before submitting a live swap'
          : 'Open the sidebar and connect an injected wallet or local dev signer before submitting a live swap',
      );
    }
    if (snapshot.trackedForeignAssetCount === 0) {
      items.push('No foreign collateral is registered yet');
    }
    if (!isBuy && !snapshot.hasPool) {
      items.push('Pool not initialized yet');
    }
    if (!foreignAssetIsCanonical() && snapshot.trackedForeignAssetCount > 0) {
      items.push(`Showing fallback foreign surface for ${nativeSymbol()}`);
    }
    if (safeInputBalance === 0n) {
      items.push(
        `No spendable ${inputSymbol} balance is currently available for swap input`,
      );
    }
    if (!slippageState.valid && parsedInput.amount !== null) {
      items.push(slippageState.reason ?? 'Enter a valid slippage cap');
    }
    if (
      slippageState.valid &&
      slippageState.percent !== null &&
      slippageState.percent > 5
    ) {
      items.push(
        `Wide slippage cap ${slippageState.label}; minimum receive can move materially before inclusion`,
      );
    }
    if (
      isBuy &&
      parsedInput.amount !== null &&
      parsedInput.amount < snapshot.minForeignSwapAmount
    ) {
      items.push(
        `Minimum buy is ${fmt(toFloat(snapshot.minForeignSwapAmount))} ${foreignSymbol()}`,
      );
    }
    if (parsedInput.amount !== null && parsedInput.amount > inBalance) {
      items.push(
        `Selected amount exceeds the available ${inputSymbol} balance`,
      );
    } else if (
      !isBuy &&
      parsedInput.amount !== null &&
      parsedInput.amount > safeInputBalance
    ) {
      items.push(
        `Native sells still need some ${nativeSymbol()} left in the account for fees, so amounts above the safe max can fail`,
      );
    }
    const quoteEligible =
      snapshot.hasNativeCurve &&
      snapshot.trackedForeignAssetCount > 0 &&
      (isBuy || snapshot.hasPool) &&
      parsedInput.amount !== null &&
      parsedInput.amount <= inBalance &&
      (!isBuy || parsedInput.amount >= snapshot.minForeignSwapAmount) &&
      (isBuy || parsedInput.amount <= safeInputBalance) &&
      slippageState.valid;
    if (
      quoteEligible &&
      quoteLoading &&
      snapshot.hasNativeCurve &&
      snapshot.trackedForeignAssetCount > 0
    ) {
      items.push('Fetching a live route quote from chain state');
    }
    if (quoteEligible && !quoteLoading && !currentQuote) {
      items.push('No route available for this size yet');
    }
    return items;
  }

  function compactQuoteViewValue() {
    return {
      facts: compactQuoteFacts,
      provenance: quoteProvenance,
      route: currentQuote?.route ?? null,
      visible: compactPane && currentQuote !== null,
    };
  }

  function inputAssetSelectViewValue() {
    return {
      dense: densePane,
      items: assetSelectItems,
      label: 'Select input asset',
      onValueChange: selectInputAsset,
      selectedValue: inputAssetValue,
      surfaceClass: 'bg-white',
    };
  }

  function outputAssetSelectViewValue() {
    return {
      dense: densePane,
      items: assetSelectItems,
      label: 'Select output asset',
      onValueChange: selectOutputAsset,
      selectedValue: outputAssetValue,
      surfaceClass: 'bg-(--mono-bg)',
    };
  }

  function payLegViewValue() {
    return {
      amountValue: inputValue,
      assetSelect: inputAssetSelectView,
      balanceLabel: isBuy ? 'Balance' : 'Safe max',
      balanceText: fmt(toFloat(isBuy ? inBalance : safeInputBalance)),
      dense: densePane,
      fillMaxEnabled: fillMaxAvailable,
      onAmountInput: updateInputValue,
      onFillMax: setMax,
    };
  }

  function receiveLegViewValue() {
    return {
      amountText: currentQuote ? fmtOut(toFloat(currentQuote.out)) : '0.00',
      assetSelect: outputAssetSelectView,
      balanceText: fmt(toFloat(outBalance)),
      dense: densePane,
    };
  }

  function diagnosticsPanelViewValue() {
    return {
      compact: compactPane,
      dense: densePane,
      onSlippageInput: updateSlippagePercent,
      provenance: currentQuote ? quoteProvenance : null,
      slippageLabel: slippageState.label,
      slippageValue: slippagePercent,
      stats: quoteStats,
      warnings,
    };
  }

  const compactPane = $derived(viewport.width > 0 && viewport.width < 430);
  const densePane = $derived(viewport.width > 0 && viewport.width < 340);
  const isBuy = $derived(marketStore.direction === 'buy');
  const inputAssetValue: SwapAssetValue = $derived(
    isBuy ? 'foreign' : 'native',
  );
  const outputAssetValue: SwapAssetValue = $derived(
    isBuy ? 'native' : 'foreign',
  );
  const inputSymbol = $derived(isBuy ? foreignSymbol() : nativeSymbol());
  const outputSymbol = $derived(isBuy ? nativeSymbol() : foreignSymbol());
  const inBalance = $derived(
    isBuy
      ? portfolioStore.userBalance.foreign
      : portfolioStore.userBalance.native,
  );
  const outBalance = $derived(
    isBuy
      ? portfolioStore.userBalance.native
      : portfolioStore.userBalance.foreign,
  );
  const safeInputBalance = $derived(safeInputBalanceValue());
  const fillMaxAvailable = $derived(safeInputBalance > 0n);
  const assetOptions = $derived(assetOptionsValue());
  const assetSelectItems = $derived(assetOptions.map(toSwapSelectItem));
  const parsedInput = $derived(parseInputState(inputValue));
  const slippageState = $derived(slippageStateValue());
  const buttonState = $derived(buttonStateValue());
  const quoteProvenance = $derived(marketStore.quoteView?.provenance ?? null);
  const routeInfo = $derived(routeInfoValue());
  const minimumReceived = $derived(minimumReceivedValue());
  const compactQuoteFacts = $derived(compactQuoteFactsValue());
  const quoteStats = $derived(quoteStatsValue());
  const warnings = $derived(warningsValue());
  const compactQuoteView = $derived(compactQuoteViewValue());
  const inputAssetSelectView = $derived(inputAssetSelectViewValue());
  const outputAssetSelectView = $derived(outputAssetSelectViewValue());
  const payLegView = $derived(payLegViewValue());
  const receiveLegView = $derived(receiveLegViewValue());
  const diagnosticsPanelView = $derived(diagnosticsPanelViewValue());

  function formatSwapAssetAmount(
    amount: bigint | undefined,
    symbol: string,
  ): string {
    return amount === undefined
      ? `unknown ${symbol}`
      : `${fmt(toFloat(amount))} ${symbol}`;
  }

  async function executeSwap() {
    if (
      !parsedInput.amount ||
      buttonState.disabled ||
      !slippageState.valid ||
      slippageState.bps === null ||
      !currentQuote
    ) {
      return;
    }
    submitting = true;
    try {
      const snapshot = systemStore.snapshot;
      if (!snapshot) {
        throw new Error('Waiting for chain state');
      }
      if (!snapshot.hasNativeCurve) {
        throw new Error('Local chain has no native curve yet');
      }
      if (snapshot.trackedForeignAssetCount === 0) {
        throw new Error('No foreign collateral is registered yet');
      }
      if (walletStore.state.signerStatus === 'readonly') {
        throw new Error(
          'Selected account is watch-only. Open the sidebar and connect an injected signer for the same address before submitting a live swap',
        );
      }
      if (walletStore.state.signerStatus !== 'available') {
        throw new Error(
          walletStore.state.signerMessage ===
            'No injected wallet extension detected'
            ? 'No injected wallet extension is available in this browser. Use a built-in local dev signer or install a supported wallet extension before submitting a live swap'
            : 'Open the sidebar and connect an injected wallet or local dev signer before submitting a live swap',
        );
      }
      if (isBuy && parsedInput.amount < snapshot.minForeignSwapAmount) {
        throw new Error(
          `Minimum buy is ${fmt(toFloat(snapshot.minForeignSwapAmount))} ${foreignSymbol()}`,
        );
      }
      if (parsedInput.amount > inBalance) {
        throw new Error(
          `Selected amount exceeds the available ${inputSymbol} balance`,
        );
      }
      if (!isBuy && parsedInput.amount > safeInputBalance) {
        throw new Error(
          `Native sells still need some ${nativeSymbol()} left in the account for fees, so amounts above the safe max can fail`,
        );
      }
      if (!isBuy && !snapshot.hasPool) {
        throw new Error('Pool not initialized yet');
      }
      if (isBuy) {
        const result = await marketStore.buyNative(
          parsedInput.amount,
          slippageState.bps,
        );
        logStore.add(
          `Bought ${formatSwapAssetAmount(result.native_out, nativeSymbol())} for ${fmt(toFloat(parsedInput.amount))} ${foreignSymbol()} via ${result.route}`,
          'buy',
          {
            blockNumber: systemStore.snapshot?.blockNumber ?? null,
            step: marketStore.history.length - 1,
          },
        );
      } else {
        const result = await marketStore.sellNative(
          parsedInput.amount,
          slippageState.bps,
        );
        logStore.add(
          `Sold ${fmt(toFloat(parsedInput.amount))} ${nativeSymbol()} for ${formatSwapAssetAmount(result.foreign_out, foreignSymbol())} via ${result.route}`,
          'sell',
          {
            blockNumber: systemStore.snapshot?.blockNumber ?? null,
            step: marketStore.history.length - 1,
          },
        );
      }
      inputValue = '';
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : 'Swap failed';
      logStore.add(message, 'error', {
        blockNumber: systemStore.snapshot?.blockNumber ?? null,
        step: marketStore.history.length - 1,
      });
    } finally {
      submitting = false;
    }
  }

  $effect(() => {
    const snapshot = systemStore.snapshot;
    const amount = parsedInput.amount;
    if (!snapshot || !amount) {
      quoteRequestId += 1;
      currentQuote = null;
      quoteLoading = false;
      return;
    }
    const quoteEligible =
      snapshot.hasNativeCurve &&
      snapshot.trackedForeignAssetCount > 0 &&
      (isBuy || snapshot.hasPool) &&
      amount <= inBalance &&
      (!isBuy || amount >= snapshot.minForeignSwapAmount) &&
      (isBuy || amount <= safeInputBalance) &&
      slippageState.valid;
    if (!quoteEligible) {
      quoteRequestId += 1;
      currentQuote = null;
      quoteLoading = false;
      return;
    }
    const requestId = ++quoteRequestId;
    quoteLoading = true;
    currentQuote = null;
    const request = isBuy
      ? marketStore.getQuoteBuy(amount)
      : marketStore.getQuoteSell(amount);
    void Promise.resolve(request)
      .then((nextQuote) => {
        if (requestId !== quoteRequestId) {
          return;
        }
        currentQuote = nextQuote;
        quoteLoading = false;
      })
      .catch(() => {
        if (requestId !== quoteRequestId) {
          return;
        }
        currentQuote = null;
        quoteLoading = false;
      });
  });

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

<Card class="min-h-full w-full py-1">
  <div bind:this={rootEl} class="@container grid gap-2.5 pb-2">
    <div
      class={[
        'grid gap-2.5',
        !compactPane && '@xl:grid-cols-[minmax(0,1.1fr)_minmax(12.5rem,0.9fr)]',
      ]}
    >
      <section
        class={[
          'grid rounded-xl bg-white shadow-[0_2px_8px_rgba(44,50,30,0.04)]',
          densePane ? 'gap-2 p-2' : 'gap-2.5 p-2.5',
        ]}
      >
        {#snippet compactQuoteStrip(view: CompactQuoteStripView)}
          {#if view.visible && view.route}
            <div class="flex flex-wrap items-center gap-1.5 text-[10px]">
              <ReadModelBadge provenance={view.provenance} />
              <Badge variant="info">{view.route}</Badge>
              {#each view.facts as fact}
                <span
                  class="rounded-full border border-(--mono-border) bg-(--mono-bg) px-2 py-0.5 tabnum text-(--mono-text)"
                >
                  {fact}
                </span>
              {/each}
            </div>
          {/if}
        {/snippet}
        {@render compactQuoteStrip(compactQuoteView)}
        {#snippet assetSelect(view: AssetSelectView)}
          <RichSelect
            value={view.selectedValue}
            items={view.items}
            label={view.label}
            dense={view.dense}
            triggerClass={view.surfaceClass}
            onValueChange={view.onValueChange}
          />
        {/snippet}
        {#snippet inputLeg(view: SwapInputLegView)}
          <div
            class={[
              'grid gap-2 overflow-hidden rounded-xl border bg-(--mono-bg)',
              view.dense ? 'px-2 py-1.5' : 'px-2.5 py-2',
            ]}
          >
            <div class="flex items-center justify-between gap-2">
              <span
                class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
                >You pay</span
              >
              <Button
                size="sm"
                variant="ghost"
                onclick={view.onFillMax}
                class={view.fillMaxEnabled
                  ? 'px-0 py-0 text-[10px] text-(--mono-purple) hover:underline tabnum'
                  : 'px-0 py-0 text-[10px] text-(--mono-muted) opacity-60 tabnum'}
                disabled={!view.fillMaxEnabled}
              >
                {view.balanceLabel}: {view.balanceText}
              </Button>
            </div>
            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <NumberInput
                value={view.amountValue}
                placeholder="0.00"
                min="0"
                step="any"
                oninput={(event) => view.onAmountInput(inputEventValue(event))}
                class={[
                  'min-w-0 border-none bg-transparent px-0 py-0 font-semibold tabnum placeholder-(--mono-border) focus:outline-none',
                  view.dense ? 'text-base' : 'text-lg',
                ]}
              />
              {@render assetSelect(view.assetSelect)}
            </div>
          </div>
        {/snippet}
        {#snippet outputLeg(view: SwapOutputLegView)}
          <div
            class={[
              'grid gap-2 overflow-hidden rounded-xl border bg-white',
              view.dense ? 'px-2 py-1.5' : 'px-2.5 py-2',
            ]}
          >
            <div class="flex items-center justify-between gap-2">
              <span
                class="text-[10px] uppercase tracking-wider text-(--mono-muted)"
                >You receive</span
              >
              <span class="text-[10px] text-(--mono-border) tabnum"
                >Balance: {view.balanceText}</span
              >
            </div>
            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <div
                class={[
                  'min-w-0 truncate font-semibold tabnum text-(--mono-muted)',
                  view.dense ? 'text-base' : 'text-lg',
                ]}
              >
                {view.amountText}
              </div>
              {@render assetSelect(view.assetSelect)}
            </div>
          </div>
        {/snippet}
        <div
          class={[
            'grid gap-2.5',
            compactPane
              ? 'grid-cols-1'
              : '@2xl:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] @2xl:items-center',
          ]}
        >
          {@render inputLeg(payLegView)}
          <div
            class={[
              'flex -my-4 z-1',
              compactPane
                ? 'justify-center'
                : 'justify-center @2xl:justify-self-center',
            ]}
          >
            <IconButton
              onclick={flipTokens}
              class="flex h-8 w-8 items-center justify-center rounded-full border bg-white shadow-sm"
              label="Flip swap direction"
            >
              <ArrowUpDown
                class="transition-all duration-200 hover:rotate-180 hover:text-(--mono-purple)"
                size={14}
              />
            </IconButton>
          </div>
          {@render outputLeg(receiveLegView)}
        </div>
        <Button
          variant="primary"
          onclick={executeSwap}
          disabled={buttonState.disabled}
          class={[
            'w-full rounded-xl font-semibold bg-(--mono-border) text-white transition-opacity',
            densePane ? 'py-2 text-[11px]' : 'py-2.5 text-xs',
          ]}
          style={`opacity: ${buttonState.disabled ? 0.5 : 1}`}
        >
          {buttonState.text}
        </Button>
      </section>
      {#snippet diagnosticsPanel(view: DiagnosticsPanelView)}
        <section
          class={[
            'grid rounded-xl bg-white shadow-[0_2px_8px_rgba(44,50,30,0.04)]',
            view.dense ? 'gap-2 p-2' : 'gap-2 p-2.5',
          ]}
        >
          <div class="flex justify-end">
            <ReadModelBadge provenance={view.provenance} />
          </div>
          <div
            class={[
              'grid gap-1.5 text-xs',
              view.dense ? 'grid-cols-1 @xs:grid-cols-2' : 'grid-cols-2',
            ]}
          >
            {#each view.stats as stat}
              <StatCard label={stat.label} value={stat.value} />
            {/each}
          </div>
          <div
            class={[
              'rounded-xl border bg-(--mono-bg)',
              view.compact
                ? 'grid gap-2 px-2.5 py-2'
                : 'grid gap-2 px-2.5 py-2 text-xs',
            ]}
          >
            <div
              class={[
                view.compact
                  ? 'grid gap-1 @xs:grid-cols-[auto_minmax(0,1fr)_auto] @xs:items-center'
                  : 'flex items-center justify-between gap-2',
              ]}
            >
              <span
                class="text-[9px] uppercase tracking-wider text-(--mono-muted)"
                >Slippage</span
              >
              <NumberInput
                value={view.slippageValue}
                min="0.01"
                max="50"
                step="0.01"
                oninput={(event) =>
                  view.onSlippageInput(inputEventValue(event))}
                class={[
                  'rounded-lg border border-(--mono-border) bg-white focus:border-(--mono-purple) focus:outline-none',
                  view.compact
                    ? 'w-full px-2 py-1 text-[11px]'
                    : 'w-full px-2.5 py-1.5 text-[10px]',
                ]}
              />
              <span class="tabnum text-[10px] text-(--mono-text)"
                >{view.slippageLabel}</span
              >
            </div>
          </div>
          {#if view.warnings.length > 0}
            <div class={['grid gap-1.5', !view.dense && '@2xl:grid-cols-2']}>
              {#each view.warnings as warning}
                <Notice variant="warn" class="text-[9px]">{warning}</Notice>
              {/each}
            </div>
          {/if}
        </section>
      {/snippet}
      {@render diagnosticsPanel(diagnosticsPanelView)}
    </div>
  </div>
</Card>
