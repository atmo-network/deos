<!--
Domain: Log widget
Owns: Account activity log presentation, live-event feed controls, and log clear/refresh affordances.
Excludes: Log store ownership, wallet account policy, adapter event subscription internals, and layout state.
Zone: Presentation widget; consumes log/system/wallet state and UI Kit primitives.
-->
<script lang="ts">
  import { RefreshCw, Trash2 } from '@lucide/svelte';

  import { logStore } from '$lib/log/index.svelte';
  import type { LogEntry } from '$lib/log/types';
  import {
    chainSurfaceIsBlocking,
    resolveChainSurfaceState,
  } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import { Badge, Button, Card, Icon, Notice } from '$lib/ui';
  import { walletStore } from '$lib/wallet/index.svelte';

  type LogMode = 'account' | 'network';
  type BlockRow = {
    id: string;
    blockNumber: number | null;
    step: number;
    entries: LogEntry[];
  };

  const LOG_COLORS: Record<string, string> = {
    info: 'text-(--mono-muted)',
    buy: 'text-(--mono-green)',
    sell: 'text-(--mono-pink)',
    error: 'text-(--mono-orange)',
  };

  let mode = $state<LogMode>('account');

  const selectedAccount = $derived(walletStore.state.selectedAddress);
  const txProgress = $derived(logStore.txProgress);
  const accountEntries = $derived.by(() =>
    logStore.log.filter((entry) => {
      if (!selectedAccount) {
        return true;
      }
      return entry.accountId === selectedAccount;
    }),
  );
  const networkEntries = $derived(logStore.networkLog);
  const networkChainSurface = $derived(
    resolveChainSurfaceState(
      systemStore.connectionState,
      logStore.networkLogView !== null,
    ),
  );
  const networkChainBlocked = $derived(
    chainSurfaceIsBlocking(networkChainSurface),
  );
  const networkRowsBlocked = $derived(
    mode === 'network' &&
      (networkChainBlocked ||
        (logStore.networkLogView === null &&
          (logStore.networkFeedState.status === 'loading' ||
            logStore.networkFeedState.status === 'error'))),
  );
  const visibleEntries = $derived(
    mode === 'account' ? accountEntries : networkEntries,
  );
  const entryCount = $derived(visibleEntries.length);
  const blockRows: BlockRow[] = $derived.by(() => {
    const rows: BlockRow[] = [];
    const rowByKey = new Map<string, BlockRow>();
    for (const entry of visibleEntries) {
      const blockNumber = entry.blockNumber;
      const step = entry.step;
      const key = `${blockNumber ?? 'step'}-${blockNumber ?? step}`;
      const existing = rowByKey.get(key);
      if (existing) {
        existing.entries.push(entry);
        continue;
      }
      const row: BlockRow = {
        id: key,
        blockNumber,
        step,
        entries: [entry],
      };
      rowByKey.set(key, row);
      rows.push(row);
    }
    return rows;
  });
  const hasReceipt = $derived(mode === 'account' && txProgress.kind !== 'idle');
  const receiptBlock = $derived(
    'blockNumber' in txProgress ? (txProgress.blockNumber ?? null) : null,
  );
  const receiptEvents = $derived(
    'eventsCount' in txProgress ? (txProgress.eventsCount ?? null) : null,
  );
  const receiptError = $derived(
    'dispatchError' in txProgress ? (txProgress.dispatchError ?? null) : null,
  );
  const receiptHighlights = $derived(
    'highlights' in txProgress && txProgress.highlights
      ? txProgress.highlights.slice(0, 3)
      : [],
  );
  const oneLineText = $derived.by(() => {
    if (hasReceipt) {
      const action =
        'actionLabel' in txProgress && txProgress.actionLabel
          ? `${txProgress.actionLabel} · `
          : '';
      return `${action}${txProgress.message}`;
    }
    if (mode === 'network') {
      if (networkChainBlocked) {
        return 'No network events available';
      }
      if (
        logStore.networkFeedState.status === 'loading' &&
        logStore.networkLogView === null
      ) {
        return 'Loading finalized network events';
      }
      if (
        logStore.networkFeedState.status === 'error' &&
        logStore.networkLogView === null
      ) {
        return 'Live network feed unavailable';
      }
    }
    const entry = visibleEntries[0];
    if (!entry) {
      return mode === 'account'
        ? 'No account activity yet'
        : 'No finalized network events captured at the latest refresh';
    }
    const provenance =
      mode === 'network' && networkChainSurface.status === 'stale'
        ? 'Stale · '
        : '';
    return `${provenance}${blockLabel(entry.blockNumber, entry.step)} · ${entryLabel(entry)} · ${entry.message}`;
  });

  function entryLabel(entry: LogEntry): string {
    return entry.label ?? entry.type;
  }

  function blockLabel(blockNumber: number | null, step: number): string {
    return blockNumber !== null ? `#${blockNumber}` : `S${step}`;
  }
</script>

<Card class="h-full min-h-full flex flex-col" level={1}>
  <div class="log-container flex h-full min-h-0 flex-col [container-type:size]">
    <div
      class="log-one-line hidden h-full min-w-0 items-center gap-2 px-2 text-2xs"
    >
      <div
        class="flex shrink-0 items-center gap-0.5 rounded-lg bg-(--mono-bg) p-0.5"
      >
        <Button
          size="sm"
          variant="ghost"
          onclick={() => (mode = 'account')}
          class={mode === 'account'
            ? 'bg-white px-1.5 py-0.5 text-3xs text-(--mono-text)'
            : 'px-1.5 py-0.5 text-3xs'}
        >
          Account
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onclick={() => (mode = 'network')}
          class={mode === 'network'
            ? 'bg-white px-1.5 py-0.5 text-3xs text-(--mono-text)'
            : 'px-1.5 py-0.5 text-3xs'}
        >
          Network
        </Button>
      </div>
      <span class="min-w-0 flex-1 truncate font-mono text-(--mono-text)">
        {oneLineText}
      </span>
      <span
        class="shrink-0 rounded-full bg-(--mono-border) px-1.5 py-0.5 tabnum text-white"
      >
        {entryCount}
      </span>
    </div>

    <div class="log-standard-header shrink-0 grid gap-2 px-3 py-2 text-compact">
      <div class="flex max-w-full flex-wrap items-center justify-end gap-2">
        <div
          class="flex items-center gap-1 rounded-xl bg-(--mono-bg) p-0.5 text-2xs"
        >
          <Button
            size="sm"
            variant="ghost"
            onclick={() => (mode = 'account')}
            class={[
              'rounded-lg px-2 py-1 text-2xs',
              mode === 'account'
                ? 'bg-white text-(--mono-text)'
                : 'text-(--mono-muted)',
            ]}
          >
            Account
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onclick={() => (mode = 'network')}
            class={[
              'rounded-lg px-2 py-1 text-2xs',
              mode === 'network'
                ? 'bg-white text-(--mono-text)'
                : 'text-(--mono-muted)',
            ]}
          >
            Network
          </Button>
        </div>
        <span
          class="rounded-full bg-(--mono-border) px-1.5 py-0.5 text-2xs text-white tabnum"
        >
          {entryCount}
        </span>
        {#if mode === 'account'}
          <Button
            size="icon"
            variant="ghost"
            onclick={() => logStore.clear()}
            label="Clear current account log"
          >
            <Icon icon={Trash2} size="sm" />
          </Button>
        {:else}
          <Button
            size="icon"
            variant="ghost"
            onclick={() => void systemStore.refresh()}
            label="Refresh network log"
            disabled={systemStore.connectionState?.status !== 'connected' ||
              logStore.networkFeedState.status === 'loading'}
          >
            <Icon icon={RefreshCw} size="sm" />
          </Button>
        {/if}
      </div>
    </div>

    <div
      class="log-standard-content min-h-0 flex-1 content-start grid gap-2 px-3 py-2 text-compact"
    >
      {#if mode === 'network' && !networkChainBlocked}
        {#if networkChainSurface.status === 'stale' || networkChainSurface.status === 'preview'}
          <Notice variant="warn" class="grid gap-0.5">
            <strong>{networkChainSurface.title}</strong>
            <span>{networkChainSurface.detail}</span>
          </Notice>
        {/if}
        {#if logStore.networkFeedState.status === 'loading'}
          <Notice>Refreshing finalized network events…</Notice>
        {:else if logStore.networkFeedState.status === 'error'}
          <Notice variant="warn">
            Live feed refresh failed: {logStore.networkFeedState.message ??
              'Unknown provider error'}
          </Notice>
        {/if}
      {/if}

      {#if hasReceipt}
        <div
          class="log-row grid gap-3 rounded-xl bg-(--mono-bg) px-3 py-2 font-mono"
        >
          <div
            class="log-row-label flex items-center justify-between gap-2 text-2xs uppercase tracking-wider text-(--mono-muted)"
          >
            <span>Latest</span>
            <span class="tabnum text-(--mono-text)"
              >{receiptBlock !== null ? `#${receiptBlock}` : 'Live'}</span
            >
          </div>
          <div class="grid gap-2">
            <div class="flex flex-wrap items-center gap-2">
              <Badge variant="info">{txProgress.kind}</Badge>
              {#if 'actionLabel' in txProgress && txProgress.actionLabel}
                <span class="text-(--mono-text)">{txProgress.actionLabel}</span>
              {/if}
              {#if receiptEvents !== null}
                <span class="text-(--mono-border)">events {receiptEvents}</span>
              {/if}
              {#if receiptError}
                <span class="text-(--mono-orange)">{receiptError}</span>
              {/if}
            </div>
            <div class="text-(--mono-text)">{txProgress.message}</div>
            {#if receiptHighlights.length > 0}
              <div class="flex flex-wrap gap-x-3 gap-y-1 text-(--mono-muted)">
                {#each receiptHighlights as highlight}
                  <span>{highlight}</span>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      {/if}

      {#if !networkRowsBlocked}
        {#each blockRows as row (row.id)}
          <div
            class="log-row grid gap-3 rounded-xl bg-(--mono-bg) px-3 py-2 font-mono"
          >
            <div
              class="log-row-label flex items-center justify-between gap-2 text-2xs uppercase tracking-wider text-(--mono-muted)"
            >
              <span>Block</span>
              <span class="tabnum text-(--mono-text)"
                >{blockLabel(row.blockNumber, row.step)}</span
              >
            </div>
            <div class="grid gap-1.5">
              {#each row.entries as entry (entry.id)}
                <div class="entry-row grid gap-0.5">
                  <div
                    class={[
                      'text-2xs uppercase tracking-wider',
                      LOG_COLORS[entry.type] || LOG_COLORS.info,
                    ]}
                  >
                    {entryLabel(entry)}
                  </div>
                  <div class="text-(--mono-text)">{entry.message}</div>
                </div>
              {/each}
            </div>
          </div>
        {:else}
          <div class="py-2 text-(--mono-muted)">
            {mode === 'account'
              ? 'No account activity yet'
              : 'No finalized network events captured in this session yet'}
          </div>
        {/each}
      {/if}
    </div>
  </div>
</Card>

<style>
  @container (max-height: 80px) {
    .log-one-line {
      display: flex;
    }
    .log-standard-header,
    .log-standard-content {
      display: none;
    }
  }
  @container (min-width: 480px) {
    .log-row {
      grid-template-columns: calc(var(--widget-em) * 4.8) minmax(0, 1fr);
    }
    .log-row-label {
      flex-direction: column;
      align-items: flex-start;
      justify-content: flex-start;
    }
  }
  @container (min-width: 544px) {
    .entry-row {
      grid-template-columns: calc(var(--widget-em) * 6.1333) minmax(0, 1fr);
      gap: calc(var(--spacing) * 3);
    }
  }
</style>
