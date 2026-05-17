<!--
Domain: UI Kit primitive
Owns: Tiny SVG sparkline rendering from caller-provided numeric values.
Excludes: Data sampling, chart history ownership, and domain-specific axes.
Zone: Foundation UI; dependency-free presentation helper.
-->
<script lang="ts">
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = {
    values: number[];
    width?: number;
    height?: number;
    stroke?: string;
    class?: ClassValue | null;
  };

  let {
    values,
    width = 160,
    height = 44,
    stroke = 'var(--mono-purple)',
    class: cls = '',
  }: Props = $props();

  function finiteValues(input: number[]): number[] {
    return input.filter((value) => Number.isFinite(value));
  }

  function toPoints(
    input: number[],
    chartWidth: number,
    chartHeight: number,
  ): string {
    if (input.length === 0) {
      return '';
    }
    const min = Math.min(...input);
    const max = Math.max(...input);
    const range = max - min || 1;
    return input
      .map((value, index) => {
        const x =
          input.length === 1
            ? chartWidth / 2
            : (index / (input.length - 1)) * chartWidth;
        const y = chartHeight - 4 - ((value - min) / range) * (chartHeight - 8);
        return `${x.toFixed(2)},${y.toFixed(2)}`;
      })
      .join(' ');
  }

  const plottedValues = $derived.by(() => finiteValues(values));
  const points = $derived.by(() => toPoints(plottedValues, width, height));
  const baseline = $derived((height / 2).toFixed(2));
</script>

<svg
  viewBox={`0 0 ${width} ${height}`}
  preserveAspectRatio="none"
  aria-hidden="true"
  class={mergeClasses('block h-11 w-full', cls)}
>
  {#if points}
    <polyline
      fill="none"
      {stroke}
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      {points}
    />
  {:else}
    <line
      x1="0"
      x2={width}
      y1={baseline}
      y2={baseline}
      stroke="var(--mono-border)"
      stroke-width="1.5"
      stroke-dasharray="3 3"
    />
  {/if}
</svg>
