<script lang="ts">
  import type { Snippet } from "svelte";

  import Card from "./Card.svelte";

  type Props = {
    title?: string;
    subtitle?: string;
    actions?: Snippet;
    class?: string;
    children: Snippet;
  };

  let {
    title,
    subtitle,
    actions,
    class: cls = "",
    children,
  }: Props = $props();
</script>

<Card class={`border bg-white p-3 grid gap-3 shadow-[0_2px_8px_rgba(44,50,30,0.04)] ${cls}`.trim()}>
  {#if title || subtitle || actions}
    <div class="flex flex-wrap items-start justify-between gap-2">
      <div class="min-w-0 flex-1">
        {#if title}
          <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">{title}</div>
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
