<script lang="ts">
  import { DetailRow } from "$lib/shared/ui";

  type Props = {
    cadenceLabel: string;
    payloadKindLabel: string;
    familyLabel: string;
    executionAuthorityLabel: string;
    executionPathLabel: string;
    openingFeeLabel: string;
    advisoryScopeLabel?: string | null;
    settlementLabel?: string | null;
    fundingSourceLabel?: string | null;
    authorizedRuntimeUpgradeLabel?: string | null;
  };

  let {
    cadenceLabel,
    payloadKindLabel,
    familyLabel,
    executionAuthorityLabel,
    executionPathLabel,
    openingFeeLabel,
    advisoryScopeLabel = null,
    settlementLabel = null,
    fundingSourceLabel = null,
    authorizedRuntimeUpgradeLabel = null,
  }: Props = $props();
</script>

<DetailRow
  label="Cadence"
  value={cadenceLabel}
  valueClass="text-(--mono-text)"
/>
<DetailRow
  label="Payload kind"
  value={payloadKindLabel}
  valueClass="text-(--mono-text)"
/>
<DetailRow label="Family" value={familyLabel} valueClass="text-(--mono-text)" />
<DetailRow
  label="Execution authority"
  value={executionAuthorityLabel}
  valueClass="text-(--mono-text)"
/>
<DetailRow
  label="Execution path"
  value={executionPathLabel}
  valueClass="text-(--mono-text)"
/>
{#if advisoryScopeLabel}
  <DetailRow
    label="Advisory scope"
    value={advisoryScopeLabel}
    valueClass="text-(--mono-text)"
  />
{/if}
{#if settlementLabel}
  <DetailRow
    label="Settlement"
    value={settlementLabel}
    valueClass="text-(--mono-text)"
  />
{/if}
{#if fundingSourceLabel}
  <DetailRow
    label="Funding source"
    value={fundingSourceLabel}
    valueClass="text-(--mono-text)"
  />
{/if}
<DetailRow
  label="Opening fee"
  value={openingFeeLabel}
  valueClass="text-(--mono-text)"
/>
{#if authorizedRuntimeUpgradeLabel}
  <DetailRow
    label="Authorized runtime upgrade"
    value={authorizedRuntimeUpgradeLabel}
    valueClass="text-(--mono-text) break-all"
  />
{/if}
