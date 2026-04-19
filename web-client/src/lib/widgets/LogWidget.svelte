<script lang="ts">
  import { RefreshCw, Trash2 } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { logStore } from "$lib/log/index.svelte";
  import { systemStore } from "$lib/system/index.svelte";
  import { walletStore } from "$lib/wallet/index.svelte";
  import type { LogEntry } from "$lib/shared/types";
  import { Badge, Card, IconButton, ReadModelBadge } from "$lib/shared/ui";

  type LogMode = "account" | "network";
  type BlockRow = {
    id: string;
    blockNumber: number | null;
    step: number;
    entries: LogEntry[];
  };

  const LOG_COLORS: Record<string, string> = {
    info: "text-(--mono-muted)",
    buy: "text-(--mono-green)",
    sell: "text-(--mono-pink)",
    error: "text-(--mono-orange)",
  };
  const COMPACT_HEIGHT_THRESHOLD = 240;

  let mode = $state<LogMode>("account");
  let rootEl = $state<HTMLDivElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let compact = $state(false);

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
  const networkProvenance = $derived(
    logStore.networkLogView?.provenance ?? null,
  );
  const visibleEntries = $derived(
    mode === "account" ? accountEntries : networkEntries,
  );
  const entryCount = $derived(visibleEntries.length);
  const blockRows = $derived.by<BlockRow[]>(() => {
    const rows: BlockRow[] = [];
    const rowByKey = new Map<string, BlockRow>();
    for (const entry of visibleEntries) {
      const blockNumber = entry.blockNumber;
      const step = entry.step;
      const key = `${blockNumber ?? "step"}-${blockNumber ?? step}`;
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
  const tickerItems = $derived.by(() => {
    const items: string[] = [];
    if (mode === "account" && txProgress.kind !== "idle") {
      items.push(formatTransactionTicker(txProgress));
    }
    for (const entry of visibleEntries.slice(0, 16)) {
      items.push(formatTickerEntry(entry));
    }
    return items;
  });
  const tickerLoop = $derived(
    tickerItems.length > 1 ? [...tickerItems, ...tickerItems] : tickerItems,
  );
  const hasReceipt = $derived(mode === "account" && txProgress.kind !== "idle");
  const receiptBlock = $derived(
    "blockNumber" in txProgress ? (txProgress.blockNumber ?? null) : null,
  );
  const receiptEvents = $derived(
    "eventsCount" in txProgress ? (txProgress.eventsCount ?? null) : null,
  );
  const receiptError = $derived(
    "dispatchError" in txProgress ? (txProgress.dispatchError ?? null) : null,
  );
  const receiptHighlights = $derived(
    "highlights" in txProgress && txProgress.highlights
      ? txProgress.highlights.slice(0, 3)
      : [],
  );

  const narrowPane = $derived(viewport.width > 0 && viewport.width < 430);
  const densePane = $derived(viewport.width > 0 && viewport.width < 340);

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0, height: 0 };
      compact = false;
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
      height: rootEl.clientHeight,
    };
    compact = rootEl.clientHeight < COMPACT_HEIGHT_THRESHOLD;
  }

  function entryLabel(entry: LogEntry): string {
    return entry.label ?? entry.type;
  }

  function blockLabel(blockNumber: number | null, step: number): string {
    return blockNumber !== null ? `#${blockNumber}` : `S${step}`;
  }

  function formatTickerEntry(entry: LogEntry): string {
    return `${blockLabel(entry.blockNumber, entry.step)} · ${entryLabel(entry)} · ${entry.message}`;
  }

  function formatTransactionTicker(progress: typeof txProgress): string {
    const actionLabel =
      "actionLabel" in progress
        ? (progress.actionLabel ?? "Transaction")
        : "Transaction";
    const block =
      "blockNumber" in progress && progress.blockNumber !== undefined
        ? ` · ${blockLabel(progress.blockNumber ?? null, -1)}`
        : "";
    return `${actionLabel} · ${progress.kind}${block} · ${progress.message}`;
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

<Card class="min-h-full flex flex-col" level={1}>
  <div bind:this={rootEl} class="h-full flex flex-col min-h-0">
    <div class="shrink-0 px-3 py-2 grid gap-2 text-[11px]">
      <div class="flex max-w-full flex-wrap items-center justify-end gap-2">
        <div
          class={[
            "flex items-center gap-1 rounded-xl border border-(--mono-border) bg-(--mono-bg) p-0.5 text-[10px]",
            densePane && "w-full justify-between",
          ]}
        >
          <button
            onclick={() => (mode = "account")}
            class={[
              "rounded-lg px-2 py-1 transition-colors",
              mode === "account"
                ? "bg-white text-(--mono-text)"
                : "text-(--mono-muted)",
            ]}
          >
            Account
          </button>
          <button
            onclick={() => (mode = "network")}
            class={[
              "rounded-lg px-2 py-1 transition-colors",
              mode === "network"
                ? "bg-white text-(--mono-text)"
                : "text-(--mono-muted)",
            ]}
          >
            Network
          </button>
        </div>
        <span
          class="text-[10px] bg-(--mono-border) text-white px-1.5 py-0.5 rounded-full tabnum"
        >
          {entryCount}
        </span>
        {#if mode === "network"}
          <ReadModelBadge provenance={networkProvenance} />
        {/if}
        {#if mode === "account"}
          <IconButton
            onclick={() => logStore.clear()}
            label="Clear current account log"
          >
            <Trash2 size={12} />
          </IconButton>
        {:else}
          <IconButton
            onclick={() => void systemStore.refresh()}
            label="Refresh network log"
          >
            <RefreshCw size={12} />
          </IconButton>
        {/if}
      </div>
    </div>

    {#if compact}
      <div class="flex-1 min-h-0 overflow-hidden px-3 py-2">
        {#if tickerItems.length > 0}
          <div
            class="flex h-full items-center overflow-hidden rounded-xl border bg-(--mono-bg) px-3"
          >
            <div
              class="log-ticker-track flex min-w-max items-center gap-8 whitespace-nowrap pr-8 font-mono text-[11px] text-(--mono-text)"
            >
              {#each tickerLoop as item, index (`${item}-${index}`)}
                <span class="inline-flex items-center gap-2">
                  <span class="text-(--mono-border)">•</span>
                  <span>{item}</span>
                </span>
              {/each}
            </div>
          </div>
        {:else}
          <div
            class="flex h-full items-center rounded-xl border bg-(--mono-bg) px-3 text-[11px] text-(--mono-muted)"
          >
            {mode === "account"
              ? "No account activity yet"
              : "No finalized network events captured in this session yet"}
          </div>
        {/if}
      </div>
    {:else}
      <div
        class="flex-1 min-h-0 px-3 py-2 grid gap-2 content-start text-[11px]"
      >
        {#if hasReceipt}
          <div
            class={[
              "grid gap-3 rounded-xl border bg-(--mono-bg) font-mono",
              narrowPane
                ? "px-2.5 py-2"
                : "grid-cols-[72px_minmax(0,1fr)] px-3 py-2",
            ]}
          >
            <div
              class={[
                "text-[10px] uppercase tracking-wider text-(--mono-muted)",
                narrowPane
                  ? "flex items-center justify-between gap-2"
                  : "flex flex-col gap-1",
              ]}
            >
              <span>Latest</span>
              <span class="tabnum text-(--mono-text)"
                >{receiptBlock !== null ? `#${receiptBlock}` : "Live"}</span
              >
            </div>
            <div class="grid gap-2">
              <div class="flex flex-wrap items-center gap-2">
                <Badge variant="info">{txProgress.kind}</Badge>
                {#if "actionLabel" in txProgress && txProgress.actionLabel}
                  <span class="text-(--mono-text)"
                    >{txProgress.actionLabel}</span
                  >
                {/if}
                {#if receiptEvents !== null}
                  <span class="text-(--mono-border)"
                    >events {receiptEvents}</span
                  >
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

        {#each blockRows as row (row.id)}
          <div
            class={[
              "grid gap-3 rounded-xl border bg-(--mono-bg) font-mono",
              narrowPane
                ? "px-2.5 py-2"
                : "grid-cols-[72px_minmax(0,1fr)] px-3 py-2",
            ]}
          >
            <div
              class={[
                "text-[10px] uppercase tracking-wider text-(--mono-muted)",
                narrowPane
                  ? "flex items-center justify-between gap-2"
                  : "flex flex-col gap-1",
              ]}
            >
              <span>Block</span>
              <span class="tabnum text-(--mono-text)"
                >{blockLabel(row.blockNumber, row.step)}</span
              >
            </div>
            <div class="grid gap-1.5">
              {#each row.entries as entry (entry.id)}
                <div
                  class={[
                    "grid gap-0.5",
                    narrowPane
                      ? "grid-cols-1"
                      : "sm:grid-cols-[92px_minmax(0,1fr)] sm:gap-3",
                  ]}
                >
                  <div
                    class={[
                      "text-[10px] uppercase tracking-wider",
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
            {mode === "account"
              ? "No account activity yet"
              : "No finalized network events captured in this session yet"}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</Card>

<style>
  .log-ticker-track {
    animation: log-ticker-scroll 24s linear infinite;
  }
  @keyframes log-ticker-scroll {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(-50%);
    }
  }
</style>
