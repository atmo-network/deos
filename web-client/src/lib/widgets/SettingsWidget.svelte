<!--
Domain: Settings widget
Owns: Endpoint/domain/layout settings controls and local settings presentation.
Excludes: Endpoint constants, governance session ownership, layout mutation internals, and adapter lifecycle.
Zone: Presentation widget; consumes system/layout/governance helpers and UI Kit controls.
-->
<script lang="ts">
  import { parseUnsignedDecimalNumber } from '$lib/format/numeric';
  import { getGovernanceDomainId } from '$lib/governance/session';
  import { isValidGovernanceDomainId } from '$lib/governance/session';
  import { layoutStore } from '$lib/layout/index.svelte';
  import { suppressTabFlipForLayoutResize } from '$lib/layout/tab-flip';
  import { getBlockchainEndpoint } from '$lib/system/endpoint';
  import { Button, Notice, NumberInput, SectionCard, TextField } from '$lib/ui';

  let endpoint = $state(getBlockchainEndpoint());
  let domainId = $state(getGovernanceDomainId());
  let applying = $state(false);
  let applyError = $state<string | null>(null);

  const parsedDomainId = $derived.by(() => {
    const parsed = parseUnsignedDecimalNumber(String(domainId));
    return parsed !== null && isValidGovernanceDomainId(parsed) ? parsed : null;
  });

  function resetWorkspaceLayout(): void {
    suppressTabFlipForLayoutResize();
    layoutStore.resetLayout();
  }

  async function apply() {
    if (parsedDomainId === null || applying) {
      return;
    }
    applying = true;
    applyError = null;
    try {
      const [{ governanceStore }, { systemStore }] = await Promise.all([
        import('$lib/governance/index.svelte'),
        import('$lib/system/index.svelte'),
      ]);
      await governanceStore.setEndpoint(endpoint.trim());
      governanceStore.setDomainId(parsedDomainId);
      await Promise.all([governanceStore.refresh(), systemStore.refresh()]);
    } catch (error) {
      applyError =
        error instanceof Error ? error.message : 'Settings update failed';
    } finally {
      applying = false;
    }
  }
</script>

<section class="@container grid gap-3 text-xs">
  <SectionCard title="Network">
    <TextField
      bind:value={endpoint}
      label="PAPI endpoint"
      placeholder="ws://127.0.0.1:9988"
    />
    <NumberInput label="Domain ID" bind:value={domainId} step={1} min={0} />
    {#if parsedDomainId === null}
      <Notice variant="warn">
        Domain ID must be a non-negative whole number.
      </Notice>
    {/if}
  </SectionCard>

  {#if applyError}
    <Notice variant="warn">{applyError}</Notice>
  {/if}
  <Button
    variant="primary"
    class="w-full @lg:w-auto @lg:justify-self-end @lg:min-w-36"
    onclick={apply}
    disabled={parsedDomainId === null || applying}
  >
    {applying ? 'Applying…' : 'Apply changes'}
  </Button>

  <SectionCard title="Workspace">
    <p class="text-2xs text-(--mono-muted)">
      Restore the reference pane tree, mobile order, expansion, and sidebar
      state.
    </p>
    <Button class="w-full @lg:w-auto" onclick={resetWorkspaceLayout}>
      Reset layout
    </Button>
  </SectionCard>
</section>
