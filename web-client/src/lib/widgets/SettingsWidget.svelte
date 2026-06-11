<!--
Domain: Settings widget
Owns: Endpoint/domain/layout settings controls and local settings presentation.
Excludes: Endpoint constants, governance session ownership, layout mutation internals, and adapter lifecycle.
Zone: Presentation widget; consumes system/layout/governance helpers and UI Kit controls.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import { parseUnsignedDecimalNumber } from '$lib/format/numeric';
  import { getGovernanceDomainId } from '$lib/governance/session';
  import { isValidGovernanceDomainId } from '$lib/governance/session';
  import { layoutStore } from '$lib/layout/index.svelte';
  import { getBlockchainEndpoint } from '$lib/system/endpoint';
  import { Button, Notice, NumberInput, SelectField, TextField } from '$lib/ui';

  type SidebarEdgeValue = 'left' | 'right';

  const sidebarEdgeOptions = [
    { value: 'left', label: 'Left lane' },
    { value: 'right', label: 'Right lane' },
  ];

  let rootEl = $state<HTMLElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let endpoint = $state(getBlockchainEndpoint());
  let domainId = $state(getGovernanceDomainId());
  let sidebarEdge = $state<SidebarEdgeValue>(layoutStore.frame.sidebar.edge);

  const compactPane = $derived(viewport.width > 0 && viewport.width < 430);
  const parsedDomainId = $derived.by(() => {
    const parsed = parseUnsignedDecimalNumber(String(domainId));
    return parsed !== null && isValidGovernanceDomainId(parsed) ? parsed : null;
  });

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0, height: 0 };
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
      height: rootEl.clientHeight,
    };
  }

  function setSidebarEdgeValue(value: string) {
    if (value === 'left' || value === 'right') {
      sidebarEdge = value;
    }
  }

  async function apply() {
    if (parsedDomainId === null) {
      return;
    }
    const [{ governanceStore }, { systemStore }] = await Promise.all([
      import('$lib/governance/index.svelte'),
      import('$lib/system/index.svelte'),
    ]);
    await governanceStore.setEndpoint(endpoint.trim());
    governanceStore.setDomainId(parsedDomainId);
    layoutStore.setSidebarEdge(sidebarEdge);
    await Promise.all([governanceStore.refresh(), systemStore.refresh()]);
  }

  onMount(() => {
    syncViewport();
    if (!rootEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<section bind:this={rootEl} class="grid gap-3 text-xs">
  <div>
    <div class="text-xs font-medium text-(--mono-text)">On-chain settings</div>
    <div class="text-[10px] text-(--mono-muted)">
      Connection and layout overrides for the local client shell
    </div>
  </div>
  <div class="grid gap-3">
    <span
      class="text-[10px] text-(--mono-muted) uppercase tracking-wider font-medium"
      >Connection</span
    >
    <TextField
      bind:value={endpoint}
      label="PAPI Endpoint"
      placeholder="ws://127.0.0.1:9988"
    />
    <div class={['grid gap-3', !compactPane && 'grid-cols-2']}>
      <NumberInput label="Domain ID" bind:value={domainId} step={1} min={0} />
      <SelectField
        label="Sidebar edge"
        value={sidebarEdge}
        onchange={(event) => setSidebarEdgeValue(event.currentTarget.value)}
      >
        {#each sidebarEdgeOptions as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </SelectField>
    </div>
    {#if parsedDomainId === null}
      <Notice variant="warn">
        Domain ID must be a non-negative whole number.
      </Notice>
    {/if}
    <Button
      variant="primary"
      class={compactPane ? 'w-full' : 'justify-self-end min-w-36'}
      onclick={apply}
      disabled={parsedDomainId === null}
    >
      Apply Changes
    </Button>
  </div>
</section>
