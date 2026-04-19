<script lang="ts">
  import { Dialog } from "bits-ui";
  import { X } from "@lucide/svelte";
  import type { Snippet } from "svelte";

  type Props = {
    open: boolean;
    title: string;
    description?: string;
    class?: string;
    children: Snippet;
  };

  let {
    open = $bindable(),
    title,
    description,
    class: cls = "",
    children,
  }: Props = $props();
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-50 bg-black/35 backdrop-blur-[1px]" />
    <Dialog.Content class={`fixed right-0 top-0 bottom-0 z-50 w-80 border-l border-(--mono-border) bg-white p-5 shadow-2xl overflow-y-auto ${cls}`.trim()}>
      <div class="flex flex-col gap-5">
        <div class="flex items-center justify-between">
          <div>
            <Dialog.Title class="text-sm font-semibold text-(--mono-text)">{title}</Dialog.Title>
            {#if description}
              <Dialog.Description class="text-[10px] text-(--mono-muted)">{description}</Dialog.Description>
            {/if}
          </div>
          <Dialog.Close class="rounded-lg p-1 text-(--mono-muted) transition-colors hover:bg-(--mono-bg) hover:text-(--mono-text)" aria-label="Close panel">
            <X size={16} />
          </Dialog.Close>
        </div>
        {@render children()}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
