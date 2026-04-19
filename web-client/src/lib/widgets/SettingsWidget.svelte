<script lang="ts">
  import { Check, ChevronDown } from "@lucide/svelte";
  import { Select } from "bits-ui";
  import { onMount } from "svelte";

  import { getGovernanceDomainId } from "$lib/governance/session";
  import { layoutStore } from "$lib/layout/index.svelte";
  import { Button, NumberInput, TextField } from "$lib/shared/ui";
  import { getBlockchainEndpoint } from "$lib/system/endpoint";

  type SidebarEdgeValue = "left" | "right";

  const sidebarEdgeOptions = [
    { value: "left", label: "Left lane" },
    { value: "right", label: "Right lane" },
  ];

  let rootEl = $state<HTMLElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let endpoint = $state(getBlockchainEndpoint());
  let domainId = $state(getGovernanceDomainId());
  let sidebarEdge = $state<SidebarEdgeValue>(layoutStore.frame.sidebar.edge);

  const compactPane = $derived(
    viewport.width > 0 && viewport.width < 430,
  );

  const selectedSidebarEdge = $derived(
    sidebarEdgeOptions.find((option) => option.value === sidebarEdge) ??
      sidebarEdgeOptions[0],
  );

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
    if (value === "left" || value === "right") {
      sidebarEdge = value;
    }
  }

  async function apply() {
    const [{ governanceStore }, { systemStore }] = await Promise.all([
      import("$lib/governance/index.svelte"),
      import("$lib/system/index.svelte"),
    ]);
    await governanceStore.setEndpoint(endpoint.trim());
    governanceStore.setDomainId(Number(domainId));
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
    <div class={[
      "grid gap-3",
      !compactPane && "grid-cols-2",
    ]}>
      <label class="block">
        <span class="text-xs text-(--mono-muted)">Domain ID</span>
        <NumberInput bind:value={domainId} step={1} min={0} class="w-full mt-1" />
      </label>
      <label class="block">
        <span class="text-xs text-(--mono-muted)">Sidebar edge</span>
        <div class="mt-1">
          <Select.Root
            type="single"
            value={sidebarEdge}
            items={sidebarEdgeOptions}
            allowDeselect={false}
            onValueChange={setSidebarEdgeValue}
          >
            <Select.Trigger
              class="inline-flex w-full items-center justify-between gap-2 rounded-lg border border-(--mono-border) bg-white px-2.5 py-2 text-xs text-(--mono-text) transition-colors hover:border-(--mono-purple) data-[state=open]:border-(--mono-purple)"
              aria-label="Select sidebar edge"
            >
              <span>{selectedSidebarEdge.label}</span>
              <ChevronDown size={12} class="shrink-0 text-(--mono-muted)" />
            </Select.Trigger>
            <Select.Portal>
              <Select.Content
                sideOffset={8}
                class="z-50 min-w-40 rounded-xl border border-(--mono-border) bg-[linear-gradient(135deg,#ffffff_0%,#f7fbef_46%,#edf6fa_100%)] p-1 shadow-[0_8px_24px_rgba(44,50,30,0.06)]"
              >
                <Select.Viewport class="grid gap-1">
                  {#each sidebarEdgeOptions as option}
                    <Select.Item
                      value={option.value}
                      label={option.label}
                      class="group grid cursor-default grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded-lg border border-transparent px-2.5 py-2 text-xs outline-none transition-colors data-highlighted:border-(--mono-purple)/20 data-highlighted:bg-(--mono-bg) data-selected:border-(--mono-purple)/25 data-selected:bg-(--mono-bg)"
                    >
                      <span class="truncate font-medium text-(--mono-text)"
                        >{option.label}</span
                      >
                      <Check
                        size={12}
                        class="shrink-0 text-(--mono-purple) opacity-0 transition-opacity group-data-selected:opacity-100"
                      />
                    </Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
        </div>
      </label>
    </div>
    <Button variant="primary" class={compactPane ? "w-full" : "justify-self-end min-w-36"} onclick={apply}>
      Apply Changes
    </Button>
  </div>
</section>
