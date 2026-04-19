<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    variant?: "muted" | "warn" | "success";
    class?: string;
    children: Snippet;
  };

  let { variant = "muted", class: cls = "", children }: Props = $props();

  const variants = {
    muted: "bg-white text-(--mono-muted)",
    warn: "border border-(--mono-orange)/30 bg-(--mono-orange)/10 text-(--mono-orange)",
    success: "border border-(--mono-green)/30 bg-(--mono-green)/10 text-(--mono-green)",
  };
</script>

<div class={["rounded-lg px-2 py-1 text-[10px] leading-4", variants[variant], cls]}>
  {@render children()}
</div>
