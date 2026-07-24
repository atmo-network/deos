<!--
Domain: Compact-width sidebar sheet
Owns: Overlay sidebar placement below the side-lane breakpoint, dismissal, focus containment, and trigger focus return.
Excludes: Sidebar widget internals, persisted open-state policy, wide desktop sidebar placement, and panel loading policy.
Zone: Compact-width layout overlay; composes the reserved sidebar panel through Bits UI Dialog semantics.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Component } from 'svelte';

  import SidebarPanelSkeleton from './SidebarPanelSkeleton.svelte';

  type SidebarPanelComponent = Component<{
    mobile: boolean;
  }>;

  type Props = {
    id: string;
    open: boolean;
    panelComponent: SidebarPanelComponent | null;
    mobile: boolean;
    onclose: () => void;
  };

  let { id, open, panelComponent, mobile, onclose }: Props = $props();

  const SidebarPanel = $derived(panelComponent);

  function getOpen(): boolean {
    return open;
  }

  function setOpen(nextOpen: boolean): void {
    if (!nextOpen && open) {
      onclose();
    }
  }

  function returnFocus(event: Event): void {
    event.preventDefault();
    requestAnimationFrame(() => {
      document.querySelector<HTMLElement>(`[aria-controls="${id}"]`)?.focus();
    });
  }
</script>

<Dialog.Root bind:open={getOpen, setOpen}>
  <Dialog.Portal>
    <Dialog.Overlay
      class="fixed inset-0 z-40 bg-black/35 transition-opacity duration-180 ease-out data-[starting-style]:opacity-0 data-[ending-style]:opacity-0 motion-reduce:transition-none"
    />
    <Dialog.Content
      {id}
      class="fixed inset-x-0 bottom-0 z-50 h-[min(82dvh,48rem)] min-h-0 px-3 pt-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] transition-[transform,opacity] duration-180 ease-out data-[starting-style]:translate-y-4 data-[starting-style]:opacity-0 data-[ending-style]:translate-y-4 data-[ending-style]:opacity-0 motion-reduce:transition-none"
      onCloseAutoFocus={returnFocus}
    >
      <Dialog.Title class="sr-only">Account and settings</Dialog.Title>
      <Dialog.Description class="sr-only">
        Manage the local account, signer, network, and display settings.
      </Dialog.Description>
      <div class="h-full min-h-0">
        {#if SidebarPanel}
          <SidebarPanel {mobile} />
        {:else}
          <SidebarPanelSkeleton />
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
