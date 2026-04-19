<script lang="ts">
  type Props = {
    label: string;
    value: string;
    class?: string;
    valueClass?: string;
  };

  let {
    label,
    value,
    class: cls = "",
    valueClass = "text-(--mono-text)",
  }: Props = $props();
</script>

<div class={["flex justify-between gap-3", cls]}>
  <span>{label}</span>
  <span class={["text-right", valueClass]}>{value}</span>
</div>
