<!--
Domain: UI Kit provenance primitive
Owns: Compact read-model provenance badge presentation and tooltip copy.
Excludes: Provenance creation, provider selection, and domain read-model ownership.
Zone: Foundation UI; depends on read-model contract labels only.
-->
<script lang="ts">
  import type { ClassValue } from 'svelte/elements';

  import type { ReadModelProvenance } from '$lib/read-model';
  import { getReadModelDescription, getReadModelLabel } from '$lib/read-model';

  import { mergeClasses } from './class';

  type Props = {
    provenance?: ReadModelProvenance | null;
    tone?: 'solid' | 'subtle';
    class?: ClassValue | null;
  };

  let { provenance = null, tone = 'solid', class: cls = '' }: Props = $props();

  const label = $derived.by(() =>
    provenance ? getReadModelLabel(provenance) : null,
  );
  const description = $derived.by(() =>
    provenance ? getReadModelDescription(provenance) : null,
  );
  const toneClass = $derived(
    tone === 'subtle'
      ? 'border border-(--mono-border) bg-(--mono-bg) text-(--mono-muted)'
      : 'bg-(--mono-border) text-white',
  );
</script>

{#if label}
  <span
    class={mergeClasses(
      'inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium text-nowrap',
      toneClass,
      cls,
    )}
    title={description ?? undefined}
    aria-label={description ?? label}
  >
    {label}
  </span>
{/if}
