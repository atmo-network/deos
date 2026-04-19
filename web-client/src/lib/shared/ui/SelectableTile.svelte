<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Props = HTMLButtonAttributes & {
    selected?: boolean;
    children: Snippet;
  };

  let { selected = false, children, class: cls = "", ...rest }: Props = $props();
</script>

<button
  class={[
    "rounded-xl border px-3 py-2 text-left transition-colors",
    selected
      ? "border-(--mono-purple) bg-(--mono-bg)"
      : "border-(--mono-border) bg-white hover:border-(--mono-purple)/40",
    cls,
  ]}
  {...rest}
>
  {@render children()}
</button>
