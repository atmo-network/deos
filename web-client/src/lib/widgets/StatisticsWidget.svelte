<script lang="ts">
  import { onMount } from "svelte";

  import { governanceStore } from "$lib/governance/index.svelte";
  import { marketStore } from "$lib/market/index.svelte";
  import { fmt, fmtPrice, toFloat } from "$lib/shared/format";
  import { fromClientBoundedProjection } from "$lib/shared/read-model";
  import {
      Card,
      Notice,
      ReadModelBadge,
      SectionCard,
      Sparkline,
      StatCard,
  } from "$lib/shared/ui";
  import { systemStore } from "$lib/system/index.svelte";

  type TrendCard = {
    label: string;
    value: string;
    detail: string;
    values: number[];
    stroke: string;
    toneClass: string;
  };

  const RECENT_POINTS = 72;
  const TARGET_GRAVITY_WELL_PERCENT = 15;

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

  function finiteValues(values: Array<number | null | undefined>): number[] {
    return values.filter((value): value is number => value != null && Number.isFinite(value));
  }

  const snap = $derived(systemStore.snapshot);
  const recentHistory = $derived.by(() => marketStore.history.slice(-RECENT_POINTS));
  const historyProvenance = $derived(marketStore.historyView?.provenance ?? null);
  const snapshotProvenance = fromClientBoundedProjection(
    true,
    "statisticsWidget.protocolSnapshot <- system snapshot + governance counts",
  ).provenance;
  const liquidityProvenance = fromClientBoundedProjection(
    true,
    "statisticsWidget.liquidityPosture <- snapshot reserves + governance counts",
  ).provenance;

  const overviewCards = $derived.by(() => {
    if (!snap) {
      return [
        { label: "Active proposals", value: governanceStore.state.activeProposalIds.length.toString(), toneClass: "text-(--mono-text)" },
        { label: "Recent finalized", value: governanceStore.state.recentFinalizedProposals.length.toString(), toneClass: "text-(--mono-text)" },
      ];
    }
    const supply = toFloat(snap.supply);
    const burned = snap.totalBurned != null ? toFloat(snap.totalBurned) : null;
    const xykPrice = snap.priceXyk ? toFloat(snap.priceXyk) : 0;
    const latest = recentHistory.at(-1) ?? null;
    const mintPrice = latest?.priceEffTMC ?? 0;
    const marketCap = supply * mintPrice;
    return [
      { label: "Market cap", value: `$${fmt(marketCap)}`, toneClass: "text-(--mono-text)" },
      { label: "Supply", value: fmt(supply), toneClass: "text-(--mono-cyan)" },
      { label: "Mint price", value: `$${fmtPrice(mintPrice)}`, toneClass: "text-(--mono-green)" },
      { label: "XYK price", value: snap.hasPool ? `$${fmtPrice(xykPrice)}` : "—", toneClass: "text-(--mono-purple)" },
      { label: "Burned", value: burned != null ? fmt(burned) : "—", toneClass: "text-(--mono-pink)" },
      { label: "Finalized", value: snap.blockNumber?.toString() ?? "—", toneClass: "text-(--mono-text)" },
    ];
  });

  const trendCards = $derived.by<TrendCard[]>(() => {
    const routerValues = finiteValues(recentHistory.map((point) => point.priceRouter));
    const tmcValues = finiteValues(recentHistory.map((point) => point.priceEffTMC));
    const supplyValues = finiteValues(recentHistory.map((point) => point.supply));
    const marketCapValues = finiteValues(recentHistory.map((point) => point.supply * point.priceEffTMC));
    return [
      {
        label: "Router price",
        value: routerValues.length > 0 ? `$${fmtPrice(routerValues[routerValues.length - 1])}` : "—",
        detail: "Recent routed execution sample",
        values: routerValues,
        stroke: "#f5861f",
        toneClass: "text-(--mono-orange)",
      },
      {
        label: "TMC price",
        value: tmcValues.length > 0 ? `$${fmtPrice(tmcValues[tmcValues.length - 1])}` : "—",
        detail: "Curve-side mint trace",
        values: tmcValues,
        stroke: "#8abf0f",
        toneClass: "text-(--mono-green)",
      },
      {
        label: "Supply",
        value: supplyValues.length > 0 ? fmt(supplyValues[supplyValues.length - 1]) : "—",
        detail: "Recent issuance level",
        values: supplyValues,
        stroke: "#25b8d1",
        toneClass: "text-(--mono-cyan)",
      },
      {
        label: "Market cap",
        value: marketCapValues.length > 0 ? `$${fmt(marketCapValues[marketCapValues.length - 1])}` : "—",
        detail: "Supply × mint price sample",
        values: marketCapValues,
        stroke: "#6d5dfc",
        toneClass: "text-(--mono-purple)",
      },
    ];
  });

  const routeMix = $derived.by(() => {
    let tmc = 0;
    let xyk = 0;
    for (const point of recentHistory) {
      if (point.routeRouter === "TMC") {
        tmc += 1;
        continue;
      }
      if (point.routeRouter === "XYK") {
        xyk += 1;
      }
    }
    const total = tmc + xyk;
    return {
      tmc,
      xyk,
      total,
      tmcPercent: total > 0 ? (tmc / total) * 100 : 0,
      xykPercent: total > 0 ? (xyk / total) * 100 : 0,
    };
  });

  const liquidityCards = $derived.by(() => {
    if (!snap) {
      return [];
    }
    return [
      { label: "Native reserve", value: fmt(toFloat(snap.reserveNative)) },
      { label: "Foreign reserve", value: fmt(toFloat(snap.reserveForeign)) },
      { label: "LP supply", value: fmt(toFloat(snap.supplyLp)) },
      { label: "Tracked foreign", value: snap.trackedForeignAssetCount.toString() },
      { label: "Active proposals", value: governanceStore.state.activeProposalIds.length.toString() },
      { label: "Recent finalized", value: governanceStore.state.recentFinalizedProposals.length.toString() },
    ];
  });

  const gravityWellPercent = $derived.by(() => {
    if (!snap) {
      return 0;
    }
    return Math.max(0, Math.min(100, snap.gravityWellRatio * 100));
  });
  const compactPane = $derived(
    viewport.width > 0 && viewport.width < 430,
  );
  const densePane = $derived(
    viewport.width > 0 && viewport.width < 340,
  );

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
      <SectionCard title="Protocol snapshot">
        {#snippet actions()}
          <ReadModelBadge provenance={snapshotProvenance} tone="subtle" />
        {/snippet}
        <div class={[
          "grid gap-2",
          densePane ? "grid-cols-1" : compactPane ? "grid-cols-2" : "grid-cols-2 @xl:grid-cols-3",
        ]}>
          {#each overviewCards as card}
            <StatCard label={card.label} value={card.value} toneClass={card.toneClass} />
          {/each}
        </div>
      </SectionCard>

      <SectionCard title="Recent traces" subtitle="Session-derived market sample">
        {#snippet actions()}
          <ReadModelBadge provenance={historyProvenance} />
        {/snippet}
        {#if trendCards.some((card) => card.values.length > 0)}
          <div class={[
            "grid gap-2",
            !compactPane && "@2xl:grid-cols-2",
          ]}>
            {#each trendCards as card}
              <div class={[
                "rounded-xl border bg-(--mono-bg) grid gap-2",
                densePane ? "p-2" : "p-3",
              ]}>
                <div class="flex items-start justify-between gap-2">
                  <div>
                    <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">{card.label}</div>
                    <div class="text-[10px] text-(--mono-muted)">{card.detail}</div>
                  </div>
                  <div class={["tabnum text-sm font-semibold", card.toneClass]}>{card.value}</div>
                </div>
                <Sparkline values={card.values} stroke={card.stroke} />
              </div>
            {/each}
          </div>
        {:else}
          <Notice>Awaiting recent chain samples</Notice>
        {/if}
      </SectionCard>

      <div class={[
        "grid gap-3",
        !compactPane && "@xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]",
      ]}>
        <SectionCard title="Route mix" subtitle="Recent router preference">
          {#snippet actions()}
            <ReadModelBadge provenance={historyProvenance} tone="subtle" />
          {/snippet}
          {#if routeMix.total > 0}
            <div class="grid gap-3">
              <div class="overflow-hidden rounded-full border border-(--mono-border) bg-(--mono-bg)">
                <div class="flex h-3 w-full">
                  <div class="bg-(--mono-green)" style:width={`${routeMix.tmcPercent}%`}></div>
                  <div class="bg-(--mono-purple)" style:width={`${routeMix.xykPercent}%`}></div>
                </div>
              </div>
              <div class={[
                "grid gap-2",
                densePane ? "grid-cols-1" : "grid-cols-2",
              ]}>
                <StatCard label="TMC" value={`${routeMix.tmc}`} detail={`${routeMix.tmcPercent.toFixed(1)}% of sampled routes`} toneClass="text-(--mono-green)" />
                <StatCard label="XYK" value={`${routeMix.xyk}`} detail={`${routeMix.xykPercent.toFixed(1)}% of sampled routes`} toneClass="text-(--mono-purple)" />
              </div>
            </div>
          {:else}
            <Notice>No recent routed samples yet</Notice>
          {/if}
        </SectionCard>

        <SectionCard title="Liquidity posture" subtitle="Current reserve and gravity well balance">
          {#snippet actions()}
            <ReadModelBadge provenance={liquidityProvenance} tone="subtle" />
          {/snippet}
          <div class="grid gap-3">
            <div class="rounded-xl border bg-(--mono-bg) px-3 py-2 grid gap-2">
              <div class="flex items-center justify-between gap-2 text-[10px] uppercase tracking-wider text-(--mono-muted)">
                <span>Gravity well</span>
                <span class="tabnum text-(--mono-text)">{gravityWellPercent.toFixed(2)}%</span>
              </div>
              <div class="relative overflow-hidden rounded-full border border-(--mono-border) bg-white">
                <div class="h-3 bg-(--mono-cyan)" style:width={`${gravityWellPercent}%`}></div>
                <div class="absolute top-0 bottom-0 w-0.5 bg-(--mono-purple)" style:left={`${TARGET_GRAVITY_WELL_PERCENT}%`}></div>
              </div>
              <div class="text-[10px] text-(--mono-muted)">Target marker {TARGET_GRAVITY_WELL_PERCENT}% · current live ratio from the on-chain snapshot</div>
            </div>
            {#if liquidityCards.length > 0}
              <div class={[
                "grid gap-2",
                densePane ? "grid-cols-1" : compactPane ? "grid-cols-2" : "grid-cols-2 @2xl:grid-cols-3",
              ]}>
                {#each liquidityCards as card}
                  <StatCard label={card.label} value={card.value} />
                {/each}
              </div>
            {:else}
              <Notice>Waiting for liquidity state</Notice>
            {/if}
          </div>
        </SectionCard>
      </div>
    </div>
  </div>
</Card>
