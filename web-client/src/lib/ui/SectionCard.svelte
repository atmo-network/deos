<!--
Domain: UI Kit primitive
Owns: Section-level card composition with title, subtitle, actions, and body slots.
Excludes: Product section ordering, domain state, and widget-specific copy.
Zone: Foundation UI; composes UI Kit Card and caller snippets only.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ClassValue } from 'svelte/elements';

  import Card from './Card.svelte';
  import { mergeClasses } from './class';

  type Props = {
    title?: string;
    subtitle?: string;
    actions?: Snippet;
    class?: ClassValue | null;
    children: Snippet;
  };

  let { title, subtitle, actions, class: cls = '', children }: Props = $props();
</script>

<Card class={mergeClasses('bg-(--mono-bg) p-3 grid gap-3', cls)}>
  {#if title || subtitle || actions}
    <div class="flex flex-wrap items-start justify-between gap-2">
      <div class="min-w-0 flex-1">
        {#if title}
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
            {title}
          </div>
        {/if}
        {#if subtitle}
          <div class="text-xs text-(--mono-text)">{subtitle}</div>
        {/if}
      </div>
      {#if actions}
        <div class="flex max-w-full flex-wrap items-center justify-end gap-2">
          {@render actions()}
        </div>
      {/if}
    </div>
  {/if}
  {@render children()}
</Card>
