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
  import {
    navigateToSwap,
    subscribeToWidgetDeepLinks,
  } from '$lib/navigation/hash-navigation';
  import { portfolioStore } from '$lib/portfolio/index.svelte';
  import { resolveChainSurfaceState } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Badge,
    Button,
    Card,
    Icon,
    Notice,
    NumberInput,
    RichSelect,
    Tooltip,
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
    balance: bigint;
    label: string;
  };
  type ParsedInputState = {
    amount: bigint | null;
  };
  type QuoteState = 'idle' | 'loading' | 'ready' | 'no-route' | 'error';

  const NATIVE_SWAP_FEE_HEADROOM = 100_000_000_000n;
  const QUICK_FILL_PERCENTAGES = [25, 50, 75] as const;

  let amountInputEl = $state<HTMLInputElement | null>(null);
  let inputValue = $state('');
  let amountInputFocused = $state(false);
  let slippagePercent = $state('0.50');
  let submitting = $state(false);
  let currentQuote = $state<Quote | null>(null);
  let networkFeeNative = $state<bigint | null>(null);
  let networkFeeLoading = $state(false);
  let quoteState = $state<QuoteState>('idle');
  let quoteError = $state<string | null>(null);
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
    if (marketStore.direction !== nextDirection) {
      marketStore.flipDirection();
      inputValue = '';
    }
    navigateToSwap(
      nextDirection === 'buy' ? 'foreign' : 'native',
      nextDirection === 'buy' ? 'native' : 'foreign',
    );
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

  function focusAmountInputFromShell(event: PointerEvent) {
    const target = event.target;
    if (
      target instanceof HTMLElement &&
      target.closest('button, input, select, a, [role="button"]')
    ) {
      return;
    }
    amountInputEl?.focus();
  }

  function onInputAssetOpenChange(open: boolean): void {
    if (open) {
      amountInputEl?.blur();
    }
  }

  function inputEventValue(event: Event) {
    const target = event.currentTarget;
    return target instanceof HTMLInputElement ? target.value : '';
  }

  function toSwapSelectItem(asset: SwapAssetOption) {
    return {
      value: asset.value,
      label: asset.label,
      detail: systemStore.snapshot
        ? `Balance: ${fmt(toFloat(asset.balance))}`
        : 'Balance: —',
    };
  }

  function flipTokens() {
    applyDirection(marketStore.direction === 'buy' ? 'sell' : 'buy');
  }

  function amountAtPercentage(balance: bigint, percentage: number) {
    return (balance * BigInt(percentage)) / 100n;
  }

  function setInputPercentage(percentage: number) {
    inputValue = fmtInputAmount(
      toFloat(amountAtPercentage(safeInputBalance, percentage)),
    );
  }

  function setMax() {
    setInputPercentage(100);
  }

  function outputSafeBalanceValue(nextDirection: 'buy' | 'sell') {
    if (nextDirection !== 'sell') {
      return outBalance;
    }
    return outBalance > NATIVE_SWAP_FEE_HEADROOM
      ? outBalance - NATIVE_SWAP_FEE_HEADROOM
      : 0n;
  }

  function setOutputPercentage(percentage: number) {
    const nextDirection = marketStore.direction === 'buy' ? 'sell' : 'buy';
    const nextSafeBalance = outputSafeBalanceValue(nextDirection);
    applyDirection(nextDirection);
    inputValue = fmtInputAmount(
      toFloat(amountAtPercentage(nextSafeBalance, percentage)),
    );
  }

  function setOutputMax() {
    setOutputPercentage(100);
  }

  function parseInputState(value: string) {
    const amount = parseTokenInputAmount(value);
    return { amount };
  }

  function assetOptionsValue() {
    const nativeOption: SwapAssetOption = {
      value: 'native',
      balance: portfolioStore.userBalance.native,
      label: nativeSymbol(),
    };
    const foreignOption: SwapAssetOption = {
      value: 'foreign',
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

  function blockedSwapLabel(): string {
    switch (chainSurface.status) {
      case 'stale':
        return 'Reconnect to refresh and swap';
      case 'unconfigured':
        return 'Connect a network to swap';
      case 'error':
        return 'Swap data unavailable';
      default:
        return 'Waiting for swap data';
    }
  }

  function buttonStateValue() {
    const snapshot = systemStore.snapshot;
    if (submitting) {
      return { text: 'Submitting swap...', disabled: true };
    }
    if (!chainCanQuery) {
      return { text: blockedSwapLabel(), disabled: true };
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
    if (quoteState === 'loading') {
      return { text: 'Fetching quote...', disabled: true };
    }
    if (quoteState === 'error') {
      return { text: 'Quote query failed', disabled: true };
    }
    if (quoteState === 'no-route' || !currentQuote) {
      return { text: 'No route available', disabled: true };
    }
    return {
      text: `${chainSurface.status === 'preview' ? 'Preview' : 'Swap'} ${inputSymbol} for ${outputSymbol}`,
      disabled: false,
    };
  }

  function percentageOfInput(amount: bigint, input: bigint | null): string {
    if (!input || input <= 0n) {
      return '—';
    }
    const percentage = Number((amount * 1_000_000n) / input) / 10_000;
    return `${percentage.toLocaleString(undefined, {
      minimumFractionDigits: percentage === 0 ? 0 : 2,
      maximumFractionDigits: 4,
    })}%`;
  }

  function priceImpactLabel(priceImpactPpb: bigint): string {
    const percentage = Number(priceImpactPpb) / 10_000_000;
    return `${percentage.toLocaleString(undefined, {
      minimumFractionDigits: percentage === 0 ? 0 : 2,
      maximumFractionDigits: 4,
    })}%`;
  }

  function liquidityProviderFeeValue(): bigint {
    if (!currentQuote || currentQuote.totalFee <= currentQuote.fee) {
      return 0n;
    }
    return currentQuote.totalFee - currentQuote.fee;
  }

  function minimumReceivedForQuote(quote: Quote, slippageBps: number): bigint {
    return quote.out > 0n
      ? (quote.out * BigInt(Math.max(0, 10_000 - slippageBps))) / 10_000n
      : 0n;
  }

  function minimumReceivedValue() {
    if (!currentQuote || !slippageState.valid || slippageState.bps === null) {
      return null;
    }
    return minimumReceivedForQuote(currentQuote, slippageState.bps);
  }

  function advisoriesValue() {
    const snapshot = systemStore.snapshot;
    const items: string[] = [];
    if (!snapshot) {
      return items;
    }
    if (!foreignAssetIsCanonical() && snapshot.trackedForeignAssetCount > 0) {
      items.push(`Showing fallback foreign surface for ${nativeSymbol()}`);
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
    return items;
  }

  const chainSurface = $derived(
    resolveChainSurfaceState(
      systemStore.connectionState,
      systemStore.snapshot !== null,
    ),
  );
  const chainCanQuery = $derived(
    systemStore.snapshot !== null &&
      systemStore.connectionState?.status === 'connected',
  );
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
  const fillMaxAvailable = $derived(chainCanQuery && safeInputBalance > 0n);
  const outputMaxAvailable = $derived(
    chainCanQuery &&
      (isBuy ? outBalance > NATIVE_SWAP_FEE_HEADROOM : outBalance > 0n),
  );
  const assetOptions = $derived(assetOptionsValue());
  const assetSelectItems = $derived(assetOptions.map(toSwapSelectItem));
  const parsedInput = $derived(parseInputState(inputValue));
  const slippageState = $derived(slippageStateValue());
  const buttonState = $derived(buttonStateValue());
  const minimumReceived = $derived(minimumReceivedValue());
  const liquidityProviderFee = $derived(liquidityProviderFeeValue());
  const advisories = $derived(advisoriesValue());

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
    if (!chainCanQuery || !snapshot || !amount) {
      quoteRequestId += 1;
      currentQuote = null;
      quoteState = 'idle';
      quoteError = null;
      networkFeeNative = null;
      networkFeeLoading = false;
      return;
    }
    const quoteEligible =
      snapshot.hasNativeCurve &&
      snapshot.trackedForeignAssetCount > 0 &&
      (isBuy || snapshot.hasPool) &&
      amount <= inBalance &&
      (!isBuy || amount >= snapshot.minForeignSwapAmount) &&
      (isBuy || amount <= safeInputBalance) &&
      slippageState.valid &&
      slippageState.bps !== null;
    if (!quoteEligible) {
      quoteRequestId += 1;
      currentQuote = null;
      quoteState = 'idle';
      quoteError = null;
      networkFeeNative = null;
      networkFeeLoading = false;
      return;
    }
    const requestId = ++quoteRequestId;
    const quoteDirection = isBuy ? 'buy' : 'sell';
    const slippageBps = slippageState.bps ?? 0;
    quoteState = 'loading';
    quoteError = null;
    currentQuote = null;
    networkFeeNative = null;
    networkFeeLoading = false;
    const request = isBuy
      ? marketStore.getQuoteBuy(amount)
      : marketStore.getQuoteSell(amount);
    void Promise.resolve(request)
      .then((nextQuote) => {
        if (requestId !== quoteRequestId) {
          return;
        }
        currentQuote = nextQuote;
        quoteState = nextQuote ? 'ready' : 'no-route';
        if (!nextQuote) {
          return;
        }
        networkFeeLoading = true;
        const minAmountOut = minimumReceivedForQuote(nextQuote, slippageBps);
        void marketStore
          .estimateSwapNetworkFee(quoteDirection, amount, minAmountOut)
          .then((fee) => {
            if (requestId !== quoteRequestId) {
              return;
            }
            networkFeeNative = fee;
            networkFeeLoading = false;
          });
      })
      .catch((error: unknown) => {
        if (requestId !== quoteRequestId) {
          return;
        }
        currentQuote = null;
        quoteState = 'error';
        quoteError =
          error instanceof Error ? error.message : 'Quote provider failed';
        networkFeeNative = null;
        networkFeeLoading = false;
      });
  });

  onMount(() =>
    subscribeToWidgetDeepLinks((link) => {
      if (link?.widget !== 'swap') {
        return;
      }
      applyDirection(link.input === 'foreign' ? 'buy' : 'sell');
    }),
  );
</script>

<Card class="h-full min-h-full w-full">
  <div class="swap-container h-full min-h-0 pt-1 pb-3 [container-type:size]">
    <section
      class="swap-surface grid gap-3 rounded-xl bg-white p-2.5 shadow-[0_2px_8px_rgba(44,50,30,0.04)]"
    >
      {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
        <Notice variant="warn" class="swap-connection-notice grid gap-0.5">
          <strong>{chainSurface.title}</strong>
          <span>{chainSurface.detail}</span>
          {#if chainSurface.status === 'stale'}
            <span
              >Reconnect before requesting a quote or submitting a swap.</span
            >
          {/if}
        </Notice>
      {/if}
      <div class="swap-layout grid gap-3">
        <div class="swap-legs grid gap-y-2 gap-x-3.5">
          <div
            class={[
              'swap-leg grid cursor-text gap-2 overflow-hidden rounded-xl border bg-white px-2.5 py-2',
              amountInputFocused
                ? 'border-(--mono-purple)'
                : 'border-(--mono-border)',
            ]}
            onpointerdown={focusAmountInputFromShell}
            role="group"
            aria-labelledby="swap-sell-label"
          >
            <div class="swap-leg-meta flex items-center justify-between gap-2">
              <span
                id="swap-sell-label"
                class="text-2xs uppercase tracking-wider text-(--mono-muted)"
                >Sell</span
              >
              <span class="text-2xs text-(--mono-border) tabnum">
                {isBuy ? 'Balance' : 'Safe max'}: {systemStore.snapshot
                  ? fmt(toFloat(isBuy ? inBalance : safeInputBalance))
                  : '—'}
              </span>
            </div>
            <div
              class="swap-value-row grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-2"
            >
              <NumberInput
                bind:ref={amountInputEl}
                value={inputValue}
                placeholder="0.00"
                min="0"
                step="any"
                oninput={(event) => (inputValue = inputEventValue(event))}
                onfocus={() => (amountInputFocused = true)}
                onblur={() => (amountInputFocused = false)}
                wrapperClass="min-w-0"
                class="min-w-0 border-0 bg-transparent px-0 py-0 text-base font-semibold tabnum placeholder-(--mono-border) focus:placeholder-transparent focus:outline-none @xs:text-lg"
              />
              <div
                class={[
                  'swap-quick-fill flex items-center gap-0.5',
                  amountInputFocused && 'invisible pointer-events-none',
                ]}
              >
                {#each QUICK_FILL_PERCENTAGES as percentage}
                  <Button
                    size="sm"
                    variant="ghost"
                    class="swap-fraction-button px-1 py-0 text-2xs text-(--mono-purple)"
                    onclick={() => setInputPercentage(percentage)}
                    disabled={!fillMaxAvailable}
                  >
                    {percentage}%
                  </Button>
                {/each}
                <Button
                  size="sm"
                  variant="ghost"
                  class="swap-max-button px-1 py-0 text-2xs text-(--mono-purple)"
                  onclick={setMax}
                  disabled={!fillMaxAvailable}
                >
                  Max
                </Button>
              </div>
              <RichSelect
                value={inputAssetValue}
                items={assetSelectItems}
                label="Select input asset"
                dense
                triggerClass="swap-asset-trigger max-w-24 border-black bg-black px-1 text-white hover:border-black data-[state=open]:border-black"
                onOpenChange={onInputAssetOpenChange}
                onValueChange={selectInputAsset}
              />
            </div>
          </div>

          <div class="swap-flip z-1 flex -my-4 justify-center">
            <Button
              size="icon"
              variant="ghost"
              onclick={flipTokens}
              class="swap-flip-button flex h-8 w-8 items-center justify-center rounded-full border-(--mono-border) bg-(--mono-border) text-white shadow-sm hover:bg-(--mono-border) hover:text-white"
              label="Flip swap direction"
            >
              <Icon
                icon={ArrowUpDown}
                class="swap-direction-icon text-white transition-transform duration-200"
              />
            </Button>
          </div>

          <div
            class="swap-leg grid gap-2 overflow-hidden rounded-xl border border-(--mono-border) bg-white px-2.5 py-2"
          >
            <div class="swap-leg-meta flex items-center justify-between gap-2">
              <span
                class="text-2xs uppercase tracking-wider text-(--mono-muted)"
                >Buy</span
              >
              <span class="text-2xs text-(--mono-border) tabnum"
                >Balance: {systemStore.snapshot
                  ? fmt(toFloat(outBalance))
                  : '—'}</span
              >
            </div>
            <div
              class="swap-value-row grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-2"
            >
              <div
                class="flex h-full min-w-0 items-center truncate text-base font-semibold tabnum text-(--mono-muted) @xs:text-lg"
              >
                {currentQuote
                  ? fmtOut(toFloat(currentQuote.out))
                  : quoteState === 'loading'
                    ? 'Fetching…'
                    : quoteState === 'no-route'
                      ? 'No route'
                      : quoteState === 'error'
                        ? 'Unavailable'
                        : '—'}
              </div>
              <div
                class={[
                  'swap-quick-fill flex items-center gap-0.5',
                  amountInputFocused && 'invisible pointer-events-none',
                ]}
              >
                {#each QUICK_FILL_PERCENTAGES as percentage}
                  <Button
                    size="sm"
                    variant="ghost"
                    class="swap-fraction-button px-1 py-0 text-2xs text-(--mono-purple)"
                    onclick={() => setOutputPercentage(percentage)}
                    disabled={!outputMaxAvailable}
                    label={`Use ${percentage}% of this asset balance as swap input`}
                  >
                    {percentage}%
                  </Button>
                {/each}
                <Button
                  size="sm"
                  variant="ghost"
                  class="swap-max-button px-1 py-0 text-2xs text-(--mono-purple)"
                  onclick={setOutputMax}
                  disabled={!outputMaxAvailable}
                  label="Use the maximum balance of this asset as swap input"
                >
                  Max
                </Button>
              </div>
              <RichSelect
                value={outputAssetValue}
                items={assetSelectItems}
                label="Select output asset"
                dense
                triggerClass="swap-asset-trigger max-w-24 border-black bg-black px-1 text-white hover:border-black data-[state=open]:border-black"
                onValueChange={selectOutputAsset}
              />
            </div>
          </div>
        </div>
      </div>

      <section
        class="swap-support swap-details swap-details-grid grid gap-2.5 rounded-xl bg-(--mono-bg) p-2.5"
        aria-label="Swap parameters and protection"
      >
        <div class="grid content-start gap-0.5">
          <span class="text-3xs uppercase tracking-wider text-(--mono-muted)"
            >Minimum received</span
          >
          <strong class="tabnum text-sm text-(--mono-text)">
            {minimumReceived === null
              ? '—'
              : `${fmtOut(toFloat(minimumReceived))} ${outputSymbol}`}
          </strong>
        </div>
        <div class="grid content-start gap-0.5">
          <span class="text-3xs uppercase text-(--mono-muted)"
            >Route / rate</span
          >
          <div class="flex min-w-0 items-center gap-1.5">
            {#if currentQuote}
              <Badge variant="info">{currentQuote.route}</Badge>
              <span class="tabnum min-w-0 truncate text-(--mono-text)">
                {fmtPrice(currentQuote.effectivePrice)}
              </span>
            {:else}
              <span>—</span>
            {/if}
          </div>
        </div>
        <div class="grid content-start gap-0.5">
          <span class="text-3xs uppercase text-(--mono-muted)"
            >Price impact</span
          >
          <span class="tabnum text-(--mono-text)">
            {currentQuote ? priceImpactLabel(currentQuote.priceImpactPpb) : '—'}
          </span>
        </div>
        <div class="grid content-start gap-0.5">
          <span class="text-3xs uppercase text-(--mono-muted)">Fees</span>
          {#if currentQuote}
            <div class="grid gap-0.5 text-3xs text-(--mono-muted)">
              <span class="tabnum">
                Router {percentageOfInput(currentQuote.fee, parsedInput.amount)} ·
                {fmt(toFloat(currentQuote.fee))}
                {inputSymbol}
              </span>
              <span class="tabnum">
                LP {percentageOfInput(liquidityProviderFee, parsedInput.amount)} ·
                {fmt(toFloat(liquidityProviderFee))}
                {inputSymbol}
              </span>
              <span class="tabnum">
                Estimated network {networkFeeLoading
                  ? 'Estimating…'
                  : networkFeeNative === null
                    ? 'Unavailable'
                    : `${fmt(toFloat(networkFeeNative))} ${nativeSymbol()}`}
              </span>
            </div>
          {:else}
            <span>—</span>
          {/if}
        </div>
        <div class="grid content-start justify-start gap-1">
          <span
            id="swap-slippage-label"
            class="text-3xs uppercase tracking-wider text-(--mono-muted)"
            >Maximum slippage</span
          >
          <NumberInput
            id="swap-slippage"
            aria-labelledby="swap-slippage-label"
            value={slippagePercent}
            min="0.01"
            max="50"
            step="0.01"
            suffix="%"
            wrapperClass="w-24"
            oninput={(event) => (slippagePercent = inputEventValue(event))}
            class="rounded-lg border border-(--mono-border) bg-white px-2.5 py-1.5 text-compact focus:border-(--mono-purple) focus:outline-none"
          />
        </div>
      </section>

      {#if quoteState === 'no-route'}
        <Notice variant="muted" class="text-3xs">
          The connected router returned no viable route for this amount.
        </Notice>
      {:else if quoteState === 'error'}
        <Notice variant="warn" class="grid gap-0.5 text-3xs">
          <strong>Quote query failed</strong>
          <span>{quoteError ?? 'The connected quote provider failed.'}</span>
        </Notice>
      {/if}

      {#if advisories.length > 0}
        <div class="advisories grid gap-1.5">
          {#each advisories as advisory}
            <Notice variant="warn" class="text-3xs">{advisory}</Notice>
          {/each}
        </div>
      {/if}

      <div class="swap-submit-wrap w-full">
        <Button
          variant="primary"
          onclick={executeSwap}
          aria-disabled={buttonState.disabled}
          class={[
            'swap-submit swap-submit-full-action w-full rounded-xl bg-(--mono-border) py-2.5 text-xs font-semibold text-white transition-opacity',
            buttonState.disabled && 'cursor-not-allowed',
          ]}
          style={`opacity: ${buttonState.disabled ? 0.5 : 1}`}
        >
          {buttonState.text}
        </Button>
        <Tooltip
          aria-label={buttonState.disabled ? buttonState.text : 'Swap'}
          contentClass="max-w-64 text-center"
          side="top"
        >
          {#snippet child({ props })}
            <Button
              {...props}
              variant="primary"
              onclick={executeSwap}
              aria-disabled={buttonState.disabled}
              class={[
                'swap-submit swap-submit-compact-action w-full items-center justify-center rounded-xl bg-(--mono-border) py-2.5 text-center text-xs font-semibold text-white transition-opacity cursor-help',
                buttonState.disabled && 'cursor-not-allowed',
              ]}
              style={`opacity: ${buttonState.disabled ? 0.5 : 1}`}
            >
              Swap
            </Button>
          {/snippet}
          {#snippet content()}
            {buttonState.text}
          {/snippet}
        </Tooltip>
      </div>
    </section>
  </div>
</Card>

<style>
  :global(.swap-direction-icon) {
    transform: rotate(0deg);
  }
  :global(.swap-flip-button:hover .swap-direction-icon) {
    transform: rotate(180deg);
  }
  :global(.swap-fraction-button),
  :global(.swap-submit-compact-action) {
    display: none;
  }
  @container (max-width: 255px) {
    :global(.swap-max-button) {
      display: none;
    }
  }
  @container (max-width: 352px) {
    .swap-surface {
      gap: 0.5rem;
      padding: 0.5rem;
    }
    .swap-details {
      padding: 0.5rem;
    }
  }
  .swap-details-grid {
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 7rem), 1fr));
  }
  @container (min-width: 416px) {
    :global(.swap-fraction-button) {
      display: inline-flex;
    }
  }
  @container (min-width: 768px) {
    .advisories {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @container (min-width: 1152px) {
    .swap-legs {
      grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
      align-items: center;
    }
    .swap-flip {
      margin: 0 -0.75rem;
    }
    :global(.swap-direction-icon) {
      transform: rotate(90deg);
    }
    :global(.swap-flip-button:hover .swap-direction-icon) {
      transform: rotate(270deg);
    }
  }
  @container (max-height: 176px) {
    .swap-container {
      padding-block: 0;
    }
    .swap-surface {
      height: 100%;
      grid-template-columns: auto minmax(0, 1fr) auto;
      align-items: center;
      gap: 0.5rem;
      padding: 0.25rem;
      overflow: hidden;
    }
    .swap-layout {
      grid-column: 2;
      min-width: 0;
    }
    .swap-legs {
      grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
      align-items: center;
      gap: 0.35rem;
    }
    .swap-leg {
      height: 2.5rem;
      grid-template-rows: 1.875rem;
      gap: 0;
      padding: 0.125rem 0.35rem 0.375rem;
    }
    .swap-value-row {
      height: 1.875rem;
      align-items: center;
      gap: 0.25rem;
      padding-inline: 0;
    }
    .swap-leg-meta,
    .swap-support,
    .advisories {
      display: none;
    }
    .swap-flip {
      margin: 0 -0.2rem;
    }
    :global(.swap-direction-icon) {
      transform: rotate(90deg);
    }
    :global(.swap-flip-button:hover .swap-direction-icon) {
      transform: rotate(270deg);
    }
    .swap-submit-wrap {
      grid-column: 3;
      width: 3.5rem;
    }
    :global(.swap-fraction-button),
    :global(.swap-max-button),
    :global(.swap-submit-full-action) {
      display: none;
    }
    :global(.swap-submit-compact-action) {
      display: inline-flex;
    }
    :global(.swap-asset-trigger) {
      width: 3.25rem;
      gap: 0;
    }
    :global(.swap-submit) {
      width: auto;
      height: 2.5rem;
      min-width: 3.5rem;
      padding: 0.4rem 0.375rem;
      white-space: nowrap;
    }
  }
  @container (max-width: 576px) and (max-height: 176px) {
    .swap-surface {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .swap-layout {
      grid-column: 1;
    }
    .swap-submit-wrap {
      grid-column: 2;
    }
    :global(.swap-connection-notice) {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }
  }
</style>
