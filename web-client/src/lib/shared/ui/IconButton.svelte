<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  import Button from "./Button.svelte";

  type Props = HTMLButtonAttributes & {
    label: string;
    children: Snippet;
  };

  let { label, children, class: cls = "", ...rest }: Props = $props();
</script>

<Button size="icon" variant="ghost" aria-label={label} title={label} class={cls} {...rest}>
  {@render children()}
</Button>
