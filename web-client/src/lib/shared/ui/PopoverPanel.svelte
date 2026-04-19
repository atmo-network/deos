<script lang="ts">
  import { Popover } from "bits-ui";
  import type { Snippet } from "svelte";

  type Props = {
    open: boolean;
    trigger: Snippet;
    children: Snippet;
    class?: string;
    side?: "top" | "right" | "bottom" | "left";
    align?: "start" | "center" | "end";
    sideOffset?: number;
  };

  let {
    open = $bindable(),
    trigger,
    children,
    class: cls = "",
    side = "bottom",
    align = "end",
    sideOffset = 8,
  }: Props = $props();
</script>

<Popover.Root bind:open>
  <Popover.Trigger>
    {@render trigger()}
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content
      side={side}
      align={align}
      sideOffset={sideOffset}
      class={`z-50 rounded-2xl border bg-[linear-gradient(135deg,#ffffff_0%,#f7fbef_46%,#edf6fa_100%)] p-3 shadow-[0_8px_24px_rgba(44,50,30,0.06)] ${cls}`.trim()}
    >
      {@render children()}
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
