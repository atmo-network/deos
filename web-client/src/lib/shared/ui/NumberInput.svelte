<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";

  type Props = HTMLInputAttributes & {
    value?: string | number;
  };

  let { value = $bindable(), class: cls = "", ...rest }: Props = $props();
</script>

<input
  type="number"
  bind:value
  class={[
    "bg-white border border-(--mono-border) rounded-xl px-3 py-2 text-sm focus:outline-none focus:border-(--mono-purple)",
    cls,
  ]}
  {...rest}
/>
