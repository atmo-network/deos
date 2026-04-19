<script lang="ts">
  import type { ReadModelProvenance } from "$lib/shared/read-model";
  import {
    getReadModelDescription,
    getReadModelLabel,
  } from "$lib/shared/read-model";

  type Props = {
    provenance?: ReadModelProvenance | null;
    tone?: "solid" | "subtle";
    class?: string;
  };

  let {
    provenance = null,
    tone = "solid",
    class: cls = "",
  }: Props = $props();

  const label = $derived.by(() =>
    provenance ? getReadModelLabel(provenance) : null,
  );
  const description = $derived.by(() =>
    provenance ? getReadModelDescription(provenance) : null,
  );
  const toneClass = $derived(
    tone === "subtle"
      ? "border border-(--mono-border) bg-(--mono-bg) text-(--mono-muted)"
      : "bg-(--mono-border) text-white",
  );
</script>

{#if label}
  <span
    class={[
      "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium text-nowrap",
      toneClass,
      cls,
    ]}
    title={description ?? undefined}
    aria-label={description ?? label}
  >
    {label}
  </span>
{/if}
