<script lang="ts">
  import { Button as BitsButton } from "bits-ui";
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Props = HTMLButtonAttributes & {
    variant?: "primary" | "secondary" | "ghost";
    size?: "sm" | "md" | "lg" | "icon";
    children: Snippet;
  };

  let { variant = "secondary", size = "md", children, class: cls = "", ...rest }: Props = $props();

  const variants = {
    primary: "bg-(--mono-purple) text-white font-semibold hover:opacity-90",
    secondary: "bg-white hover:bg-(--mono-bg) border border-(--mono-border) text-sm transition-colors shadow-sm",
    ghost: "text-(--mono-muted) hover:text-(--mono-text) transition-colors",
  };

  const sizes = {
    sm: "px-2 py-1 text-xs",
    md: "px-4 py-2",
    lg: "w-full py-3.5 rounded-xl text-sm",
    icon: "h-8 w-8 p-0 inline-flex items-center justify-center",
  };

  const className = $derived(`${variants[variant]} ${sizes[size]} rounded-xl ${cls}`.trim());
</script>

<BitsButton.Root
  class={className}
  {...rest}
>
  {@render children()}
</BitsButton.Root>
