<!--
Domain: Chart widget
Owns: Market price/liquidity visualization, SVG chart rendering, and chart-local resize lifecycle.
Excludes: Market data ownership, materialized history provider contracts, and layout state.
Zone: Presentation widget; consumes market store/read-model provenance and UI Kit containers.
-->
<script lang="ts">
  import * as d3 from 'd3';
  import { onDestroy, onMount } from 'svelte';

  import { marketStore } from '$lib/market/index.svelte';
  import type { PricePoint } from '$lib/market/types';
  import { resolveChainSurfaceState } from '$lib/system/connection-surface';
  import { systemStore } from '$lib/system/index.svelte';
  import { Button, Card, Notice } from '$lib/ui';
  import { fmtPrice } from '$lib/ui/format';

  let containerEl = $state<HTMLDivElement | null>(null);
  let svgEl = $state<SVGSVGElement | null>(null);
  let resizeObserver: ResizeObserver | null = null;
  let hidden = $state(new Set<string>());

  type ChartSeriesPoint = {
    label: string;
    value: number | null;
    color: string;
  };

  type PresentChartSeriesPoint = ChartSeriesPoint & { value: number };

  type RouterPricePoint = PricePoint & { priceRouter: number };

  function hasVisibleValue(
    item: ChartSeriesPoint,
  ): item is PresentChartSeriesPoint {
    return item.value != null && item.value > 0 && !hidden.has(item.label);
  }

  function hasRouterPrice(point: PricePoint): point is RouterPricePoint {
    return point.priceRouter != null && point.priceRouter > 0;
  }

  function toggleSeries(label: string) {
    const next = new Set(hidden);
    if (next.has(label)) next.delete(label);
    else next.add(label);
    hidden = next;
  }

  const MAX_BLOCKS_BACK = 5000;
  const DEFAULT_MARGIN = { top: 0, right: 12, bottom: 24, left: 32 };
  const COMPACT_MARGIN = { top: 2, right: 8, bottom: 18, left: 24 };

  const LEGEND_ITEMS = [
    { label: 'TMC', color: '#a6e22e' },
    { label: 'Router', color: '#fd971f' },
    { label: 'XYK', color: '#ae81ff' },
  ];

  const COLORS = {
    xyk: '#8c63f4',
    tmc: '#8abf0f',
    router: '#f5861f',
  };

  const chartData = $derived.by(() => {
    const history = marketStore.history;
    if (history.length === 0) return [];
    const latest = history[history.length - 1];
    const latestBlock = latest.blockNumber ?? latest.step ?? 0;
    return history.filter((point) => {
      const block = point.blockNumber ?? point.step ?? 0;
      return latestBlock - block <= MAX_BLOCKS_BACK;
    });
  });
  const latestPoint = $derived.by(() => chartData.at(-1) ?? null);
  const chainSurface = $derived(
    resolveChainSurfaceState(
      systemStore.connectionState,
      marketStore.historyView !== null,
    ),
  );
  const latestSeries = $derived.by(() => {
    if (!latestPoint) {
      return new Map<string, number>();
    }
    const entries: Array<[string, number | null]> = [
      ['TMC', latestPoint.priceEffTMC],
      ['Router', latestPoint.priceRouter],
      ['XYK', latestPoint.priceXYK],
    ];
    return new Map(
      entries.filter(
        (item): item is [string, number] =>
          item[1] != null && item[1] > 0 && Number.isFinite(item[1]),
      ),
    );
  });

  function xValue(d: PricePoint) {
    return d.blockNumber ?? d.step;
  }

  function xTitle(d: PricePoint) {
    return d.blockNumber !== null
      ? `Block ${d.blockNumber}`
      : `Sample ${d.step}`;
  }

  function render() {
    if (!svgEl || !containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    const width = rect.width;
    const height = rect.height;
    const compact = height < 240 || width < 320;
    const margin = compact ? COMPACT_MARGIN : DEFAULT_MARGIN;
    const tickFontSize = compact ? 8 : 9;
    const tickCount = compact ? 4 : 5;
    const xTicks = compact ? 4 : Math.min(chartData.length, 6);
    const svg = d3.select(svgEl).attr('width', width).attr('height', height);
    svg.selectAll('*').remove();
    if (chartData.length === 0) {
      svg
        .append('text')
        .attr('x', width / 2)
        .attr('y', height / 2)
        .attr('text-anchor', 'middle')
        .attr('fill', 'var(--mono-muted)')
        .attr('font-size', compact ? '11px' : '12px')
        .text('Awaiting data...');
      return;
    }

    const w = width - margin.left - margin.right;
    const h = height - margin.top - margin.bottom;
    if (w <= 0 || h <= 0) return;

    const g = svg
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);
    const xExtent = d3.extent(chartData, (d) => xValue(d));
    if (xExtent[0] === undefined || xExtent[1] === undefined) {
      return;
    }
    const xScale = d3
      .scaleLinear()
      .domain([xExtent[0], xExtent[1]])
      .range([0, w]);
    const allPrices = chartData.flatMap((d) =>
      [d.priceXYK, d.priceEffTMC, d.priceRouter].filter(
        (value): value is number =>
          value != null && value > 0 && !Number.isNaN(value),
      ),
    );
    const yMax = d3.max(allPrices) ?? 1;
    const yScale = d3
      .scaleLinear()
      .domain([0, yMax * 1.05])
      .range([h, 0]);
    const gridLines = yScale.ticks(tickCount);

    g.append('g')
      .selectAll('line')
      .data(gridLines)
      .join('line')
      .attr('x1', 0)
      .attr('x2', w)
      .attr('y1', (d) => yScale(d))
      .attr('y2', (d) => yScale(d))
      .attr('stroke', '#d9dcc7')
      .attr('stroke-opacity', 0.5);

    function priceLine(accessor: (d: PricePoint) => number | null) {
      return d3
        .line<PricePoint>()
        .defined((d) => {
          const value = accessor(d);
          return value != null && value > 0 && !Number.isNaN(value);
        })
        .x((d) => xScale(xValue(d)))
        .y((d) => yScale(accessor(d) ?? 0))
        .curve(d3.curveMonotoneX);
    }

    if (!hidden.has('XYK')) {
      g.append('path')
        .datum(chartData)
        .attr(
          'd',
          priceLine((d) => (d.priceXYK > 0 ? d.priceXYK : null)),
        )
        .attr('fill', 'none')
        .attr('stroke', COLORS.xyk)
        .attr('stroke-width', 1.5);
    }

    if (!hidden.has('TMC')) {
      g.append('path')
        .datum(chartData)
        .attr(
          'd',
          priceLine((d) => d.priceEffTMC),
        )
        .attr('fill', 'none')
        .attr('stroke', COLORS.tmc)
        .attr('stroke-width', 1.5);
    }

    if (!hidden.has('Router')) {
      g.append('path')
        .datum(chartData)
        .attr(
          'd',
          priceLine((d) => d.priceRouter),
        )
        .attr('fill', 'none')
        .attr('stroke', COLORS.router)
        .attr('stroke-width', 2);

      g.selectAll('.router-dot')
        .data(chartData.filter(hasRouterPrice))
        .join('circle')
        .attr('cx', (d) => xScale(xValue(d)))
        .attr('cy', (d) => yScale(d.priceRouter))
        .attr('r', compact ? 1.25 : 1.5)
        .attr('fill', COLORS.router);
    }

    g.append('g')
      .call(
        d3
          .axisLeft(yScale)
          .ticks(tickCount)
          .tickSize(0)
          .tickFormat((value) => (+value).toFixed(compact ? 1 : 2)),
      )
      .call((axis) => axis.select('.domain').remove())
      .call((axis) =>
        axis
          .selectAll('.tick text')
          .attr('fill', '#6f7260')
          .attr('font-size', tickFontSize),
      );

    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(
        d3
          .axisBottom(xScale)
          .ticks(xTicks)
          .tickSize(0)
          .tickFormat((value) => `#${Math.round(+value)}`),
      )
      .call((axis) => axis.select('.domain').remove())
      .call((axis) =>
        axis
          .selectAll('.tick text')
          .attr('fill', '#6f7260')
          .attr('font-size', tickFontSize)
          .attr('dy', compact ? 6 : 8),
      );

    const tooltipCandidate = containerEl.querySelector('.d3-tooltip');
    const tooltipEl =
      tooltipCandidate instanceof HTMLElement ? tooltipCandidate : null;
    const crosshair = g
      .append('line')
      .attr('y1', 0)
      .attr('y2', h)
      .attr('stroke', '#6f7260')
      .attr('stroke-dasharray', '2,2')
      .attr('opacity', 0);

    const bisect = d3.bisector<(typeof chartData)[0], number>((d) =>
      xValue(d),
    ).center;

    g.append('rect')
      .attr('width', w)
      .attr('height', h)
      .attr('fill', 'none')
      .attr('pointer-events', 'all')
      .on('mousemove', (event: MouseEvent) => {
        const [mx, my] = d3.pointer(event);
        const step = xScale.invert(mx);
        const idx = bisect(chartData, step);
        const d = chartData[idx];
        if (!d) return;

        const x = xScale(xValue(d));
        crosshair.attr('x1', x).attr('x2', x).attr('opacity', 1);

        const tmc = { label: 'TMC', value: d.priceEffTMC, color: COLORS.tmc };
        const xyk = { label: 'XYK', value: d.priceXYK, color: COLORS.xyk };
        const router = {
          label: 'Router',
          value: d.priceRouter,
          color: COLORS.router,
        };
        const ceiling = (tmc.value ?? 0) >= (xyk.value ?? 0) ? tmc : xyk;
        const floor = ceiling === tmc ? xyk : tmc;
        const items = [ceiling, router, floor].filter(hasVisibleValue);

        if (tooltipEl) {
          tooltipEl.style.opacity = '1';
          const tooltipX = margin.left + x + 12;
          const clampX = Math.min(tooltipX, width - (compact ? 136 : 160));
          const tooltipY = margin.top + my;
          const clampY = Math.min(
            Math.max(tooltipY, 4),
            height - (compact ? 82 : 100),
          );
          tooltipEl.style.left = `${clampX}px`;
          tooltipEl.style.top = `${clampY}px`;
          tooltipEl.innerHTML =
            `<div class="text-(--mono-text) mb-1">${xTitle(d)}</div>` +
            items
              .map(
                (item) =>
                  `<div class="flex items-center gap-1.5">` +
                  `<span style="background:${item.color}" class="w-1.5 h-1.5 rounded-full inline-block shrink-0"></span>` +
                  `<span class="text-(--mono-muted)">${item.label}</span>` +
                  `<span class="text-(--mono-text) ml-auto tabnum">${fmtPrice(item.value)}</span>` +
                  `</div>`,
              )
              .join('');
        }
      })
      .on('mouseleave', () => {
        crosshair.attr('opacity', 0);
        if (tooltipEl) tooltipEl.style.opacity = '0';
      });
  }

  $effect(() => {
    chartData;
    hidden;
    render();
  });

  onMount(() => {
    resizeObserver = new ResizeObserver(() => render());
    if (containerEl) resizeObserver.observe(containerEl);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
  });
</script>

<Card
  class="chart-container flex h-full min-h-full w-full flex-col overflow-hidden [container-type:size]"
>
  {#if chainSurface.status === 'stale' || chainSurface.status === 'preview'}
    <Notice variant="warn" class="mx-2.5 mt-2 grid gap-0.5">
      <strong>{chainSurface.title}</strong>
      <span>{chainSurface.detail}</span>
    </Notice>
  {/if}
  <div
    bind:this={containerEl}
    class="chart-surface relative flex-1 min-h-0 overflow-hidden"
  >
    <svg
      bind:this={svgEl}
      class="block h-full w-full"
      role="img"
      aria-label="TMC, router, and XYK price history"
    ></svg>
    <div
      class="d3-tooltip pointer-events-none absolute z-10 min-w-30 rounded-lg border border-(--mono-border) bg-white/95 px-2.5 py-2 text-compact opacity-0 shadow-sm transition-opacity backdrop-blur-sm"
    ></div>
  </div>

  <div
    class="compact-chart-status hidden px-2.5 pt-2 text-center text-2xs uppercase tracking-wider text-(--mono-muted)"
  >
    {latestPoint ? 'Latest venue prices' : 'Awaiting market data'}
  </div>

  <div
    class="series-controls flex shrink-0 flex-wrap items-center justify-center gap-1.5 px-2.5 py-2"
  >
    {#each LEGEND_ITEMS as item}
      <Button
        size="sm"
        variant="ghost"
        aria-pressed={!hidden.has(item.label)}
        onclick={() => toggleSeries(item.label)}
        class={[
          'inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-2xs cursor-pointer',
          hidden.has(item.label)
            ? 'text-(--mono-muted) opacity-60'
            : 'text-(--mono-text)',
        ]}
      >
        <span
          class="inline-block h-2 w-2 rounded-full"
          style:background={item.color}
        ></span>
        <span class={hidden.has(item.label) ? 'line-through' : ''}>
          {item.label}
        </span>
        {#if latestSeries.has(item.label)}
          <span class="tabnum"
            >${fmtPrice(latestSeries.get(item.label) ?? 0)}</span
          >
        {/if}
      </Button>
    {/each}
  </div>
</Card>

<style>
  @container (max-height: 128px) {
    .chart-surface {
      display: none;
    }
    .compact-chart-status {
      display: block;
    }
    .series-controls {
      flex: 1;
      align-content: center;
    }
  }
</style>
