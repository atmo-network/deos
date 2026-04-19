<script lang="ts">
  type Props = {
    label: string;
    value: string;
    detail?: string;
    toneClass?: string;
    class?: string;
  };

  let {
    label,
    value,
    detail,
    toneClass = "text-(--mono-text)",
    class: cls = "",
  }: Props = $props();
</script>

<div class={["min-w-0 rounded-xl border bg-(--mono-bg) px-3 py-2", cls]}>
  <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">{label}</div>
  <div class={["tabnum break-words text-[13px] font-semibold sm:text-sm", toneClass]}>{value}</div>
  {#if detail}
    <div class="break-words text-[10px] text-(--mono-muted)">{detail}</div>
  {/if}
</div>
