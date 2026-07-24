<!--
Domain: Governance archive boundary section
Owns: Materialized governance archive provider status and archive-boundary entry rendering.
Excludes: Active proposal voting, finalized on-chain retention cards, provider implementation, and store mutation.
Zone: Governance presentation component; makes the indexed/archive boundary explicit in UI.
-->
<script lang="ts">
  import type { GovernanceMaterializedArchiveEntry } from '$lib/governance';
  import type { ReadModelValue } from '$lib/read-model';
  import { Badge, DetailRow, Notice, SectionCard } from '$lib/ui';

  type ArchiveProviderView = {
    label(): string;
    message(): string | null;
  };

  type Props = {
    provider: ArchiveProviderView;
    archive: ReadModelValue<GovernanceMaterializedArchiveEntry[]>;
  };

  let { provider, archive }: Props = $props();
</script>

<SectionCard
  title="Governance archive"
  subtitle="Explicit materialized provider boundary"
>
  {#snippet actions()}
    <Badge variant="info">future provider</Badge>
  {/snippet}
  <Notice variant="muted">
    Recent finalized cards above are bounded on-chain retention. Full archive
    search and ballot timelines belong to a separate materialized/indexed
    provider, not to expanded consensus-state retention.
  </Notice>
  <div class="rounded-xl bg-white px-3 py-2 grid gap-1">
    <DetailRow
      label="Provider"
      value={provider.label()}
      valueClass="text-(--mono-text)"
    />
    <DetailRow
      label="Contract"
      value="Indexed archive/search provider"
      valueClass="text-(--mono-text)"
    />
    <DetailRow
      label="Current status"
      value={provider.message() ?? 'Configured'}
      valueClass="text-(--mono-muted)"
    />
  </div>
  {#if archive.value.length > 0}
    <div class="grid gap-2">
      {#each archive.value as entry}
        <div class="rounded-xl bg-white px-3 py-2 grid gap-1">
          <DetailRow
            label={`#${entry.itemId}`}
            value={entry.title}
            valueClass="text-(--mono-text)"
          />
          <DetailRow
            label="Outcome"
            value={entry.outcomeLabel}
            valueClass="text-(--mono-muted)"
          />
          <DetailRow
            label="Summary"
            value={entry.summary}
            valueClass="text-(--mono-muted)"
          />
        </div>
      {/each}
    </div>
  {:else}
    <Notice>
      No materialized governance archive backend is configured on the current
      reference line
    </Notice>
  {/if}
</SectionCard>
