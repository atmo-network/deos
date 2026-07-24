<!--
Domain: UI Kit primitive
Owns: Reusable metric/stat card presentation for label, value, and optional detail.
Excludes: Metric derivation, units policy, and domain state ownership.
Zone: Foundation UI; depends only on caller-provided strings/classes.
-->
<script lang="ts">
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = {
    label: string;
    value: string;
    detail?: string;
    toneClass?: ClassValue | null;
    class?: ClassValue | null;
  };

  let {
    label,
    value,
    detail,
    toneClass = 'text-(--mono-text)',
    class: cls = '',
  }: Props = $props();
</script>

<div class={mergeClasses('min-w-0 rounded-xl bg-white px-3 py-2', cls)}>
  <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
    {label}
  </div>
  <div
    class={mergeClasses(
      'tabnum break-words text-[13px] font-semibold sm:text-sm',
      toneClass,
    )}
  >
    {value}
  </div>
  {#if detail}
    <div class="break-words text-[10px] text-(--mono-muted)">{detail}</div>
  {/if}
</div>
