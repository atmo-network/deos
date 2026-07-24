<!--
Domain: Statistics widget
Owns: Cross-domain dashboard metrics, market/governance summary cards, and sparkline presentation.
Excludes: Market/governance store ownership, runtime query transport, and layout state.
Zone: Presentation widget; consumes domain stores and UI Kit visualization primitives.
-->
<script lang="ts">
  import { marketStore } from '$lib/market/index.svelte';
  import { resolveChainSurfaceState } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Card,
    DisclosureSection,
    Notice,
    SectionCard,
    Sparkline,
    StatCard,
  } from '$lib/ui';
  import { fmt, fmtPrice, toFloat } from '$lib/ui/format';

  import StakingWidget from './StakingWidget.svelte';

  type TrendCard = {
    label: string;
    value: string;
    values: number[];
    stroke: string;
    toneClass: string;
  };

  const RECENT_POINTS = 72;
  const TARGET_GRAVITY_WELL_PERCENT = 15;

  function finiteValues(values: Array<number | null | undefined>): number[] {
    return values.filter(
      (value): value is number => value != null && Number.isFinite(value),
    );
  }

  const snap = $derived(systemStore.snapshot);
  const chainSurface = $derived(
    resolveChainSurfaceState(systemStore.connectionState, snap !== null),
  );
  const recentHistory = $derived.by(() =>
    marketStore.history.slice(-RECENT_POINTS),
  );

  const overviewCards = $derived.by(() => {
    if (!snap) {
      return [
        {
          label: 'Active proposals',
          value: '—',
          toneClass: 'text-(--mono-muted)',
        },
        {
          label: 'Recent finalized',
          value: '—',
          toneClass: 'text-(--mono-muted)',
        },
      ];
    }
    const supply = toFloat(snap.supply);
    const burned = snap.totalBurned != null ? toFloat(snap.totalBurned) : null;
    const xykPrice = snap.priceXyk ? toFloat(snap.priceXyk) : 0;
    const latest = recentHistory.at(-1) ?? null;
    const mintPrice = latest?.priceEffTMC ?? 0;
    return [
      { label: 'Supply', value: fmt(supply), toneClass: 'text-(--mono-cyan)' },
      {
        label: 'Mint price',
        value: `${fmtPrice(mintPrice)}`,
        toneClass: 'text-(--mono-green)',
      },
      {
        label: 'XYK price',
        value: snap.hasPool ? `${fmtPrice(xykPrice)}` : '—',
        toneClass: 'text-(--mono-purple)',
      },
      {
        label: 'Burned',
        value: burned != null ? fmt(burned) : '—',
        toneClass: 'text-(--mono-pink)',
      },
    ];
  });

  const trendCards: TrendCard[] = $derived.by(() => {
    const routerValues = finiteValues(
      recentHistory.map((point) => point.priceRouter),
    );
    const tmcValues = finiteValues(
      recentHistory.map((point) => point.priceEffTMC),
    );
    return [
      {
        label: 'Router price',
        value:
          routerValues.length > 0
            ? `${fmtPrice(routerValues[routerValues.length - 1])}`
            : '—',
        values: routerValues,
        stroke: '#f5861f',
        toneClass: 'text-(--mono-orange)',
      },
      {
        label: 'TMC price',
        value:
          tmcValues.length > 0
            ? `${fmtPrice(tmcValues[tmcValues.length - 1])}`
            : '—',
        values: tmcValues,
        stroke: '#8abf0f',
        toneClass: 'text-(--mono-green)',
      },
    ];
  });

  const routeMix = $derived.by(() => {
    let tmc = 0;
    let xyk = 0;
    for (const point of recentHistory) {
      if (point.routeRouter === 'TMC') {
        tmc += 1;
        continue;
      }
      if (point.routeRouter === 'XYK') {
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
      { label: 'Native reserve', value: fmt(toFloat(snap.reserveNative)) },
      { label: 'Foreign reserve', value: fmt(toFloat(snap.reserveForeign)) },
      { label: 'LP supply', value: fmt(toFloat(snap.supplyLp)) },
    ];
  });

  const gravityWellPercent = $derived.by(() => {
    if (!snap) {
      return null;
    }
    return Math.max(0, Math.min(100, snap.gravityWellRatio * 100));
  });
</script>

<Card class="min-h-full flex flex-col">
  <div class="@container grid gap-3 p-3 text-xs">
    {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
      <Notice variant="warn" class="grid gap-0.5">
        <strong>{chainSurface.title}</strong>
        <span>{chainSurface.detail}</span>
      </Notice>
    {/if}
    <SectionCard title="Protocol snapshot">
      <div class="stats-grid grid gap-2">
        {#each overviewCards as card}
          <StatCard
            label={card.label}
            value={card.value}
            toneClass={card.toneClass}
          />
        {/each}
      </div>
    </SectionCard>

    <DisclosureSection title="Recent traces">
      {#if trendCards.some((card) => card.values.length > 0)}
        <div class="trend-grid grid gap-2">
          {#each trendCards as card}
            <div class="grid gap-2 rounded-xl bg-white p-3">
              <div class="flex items-start justify-between gap-2">
                <div>
                  <div
                    class="text-2xs uppercase tracking-wider text-(--mono-muted)"
                  >
                    {card.label}
                  </div>
                </div>
                <div class={['tabnum text-sm font-semibold', card.toneClass]}>
                  {card.value}
                </div>
              </div>
              <Sparkline values={card.values} stroke={card.stroke} />
            </div>
          {/each}
        </div>
      {:else}
        <Notice>Awaiting recent chain samples</Notice>
      {/if}
    </DisclosureSection>

    <DisclosureSection title="Staking operations">
      <StakingWidget />
    </DisclosureSection>

    <DisclosureSection title="Market posture">
      <div class="market-grid grid gap-3">
        <SectionCard title="Route mix">
          {#if routeMix.total > 0}
            <div class="grid gap-3">
              <div
                class="overflow-hidden rounded-full border border-(--mono-border) bg-(--mono-bg)"
              >
                <div class="flex h-3 w-full">
                  <div
                    class="bg-(--mono-green)"
                    style:width={`${routeMix.tmcPercent}%`}
                  ></div>
                  <div
                    class="bg-(--mono-purple)"
                    style:width={`${routeMix.xykPercent}%`}
                  ></div>
                </div>
              </div>
              <div class="grid grid-cols-2 gap-2">
                <StatCard
                  label="TMC"
                  value={`${routeMix.tmc}`}
                  detail={`${routeMix.tmcPercent.toFixed(1)}% of sampled routes`}
                  toneClass="text-(--mono-green)"
                />
                <StatCard
                  label="XYK"
                  value={`${routeMix.xyk}`}
                  detail={`${routeMix.xykPercent.toFixed(1)}% of sampled routes`}
                  toneClass="text-(--mono-purple)"
                />
              </div>
            </div>
          {:else}
            <Notice>No recent routed samples yet</Notice>
          {/if}
        </SectionCard>

        <SectionCard title="Liquidity posture">
          <div class="grid gap-3">
            <div class="rounded-xl bg-white px-3 py-2 grid gap-2">
              <div
                class="flex items-center justify-between gap-2 text-2xs uppercase tracking-wider text-(--mono-muted)"
              >
                <span>Gravity well</span>
                <span class="tabnum text-(--mono-text)">
                  {gravityWellPercent === null
                    ? '—'
                    : `${gravityWellPercent.toFixed(2)}% · target ${TARGET_GRAVITY_WELL_PERCENT}%`}
                </span>
              </div>
              {#if gravityWellPercent !== null}
                <div
                  class="relative overflow-hidden rounded-full border border-(--mono-border) bg-white"
                >
                  <div
                    class="h-3 bg-(--mono-cyan)"
                    style:width={`${gravityWellPercent}%`}
                  ></div>
                  <div
                    class="absolute top-0 bottom-0 w-0.5 bg-(--mono-purple)"
                    style:left={`${TARGET_GRAVITY_WELL_PERCENT}%`}
                  ></div>
                </div>
              {/if}
            </div>
            {#if liquidityCards.length > 0}
              <div class="stats-grid grid gap-2">
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
    </DisclosureSection>
  </div>
</Card>

<style>
  .stats-grid {
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 9rem), 1fr));
  }
  .trend-grid {
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 15rem), 1fr));
  }
  @container (min-width: 768px) {
    .market-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      align-items: start;
    }
  }
</style>
