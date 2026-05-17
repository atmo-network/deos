<!--
Domain: UI Kit primitive
Owns: Label/value detail-row presentation for dense information surfaces.
Excludes: Value derivation, domain semantics, and row grouping policy.
Zone: Foundation UI; depends only on caller-provided strings/classes.
-->
<script lang="ts">
  import type { ClassValue } from 'svelte/elements';

  import { mergeClasses } from './class';

  type Props = {
    label: string;
    value: string;
    class?: ClassValue | null;
    valueClass?: ClassValue | null;
  };

  let {
    label,
    value,
    class: cls = '',
    valueClass = 'text-(--mono-text)',
  }: Props = $props();
</script>

<div class={mergeClasses('flex justify-between gap-3', cls)}>
  <span>{label}</span>
  <span class={mergeClasses('text-right', valueClass)}>{value}</span>
</div>
