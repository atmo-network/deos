<script lang="ts">
  import { Button, DetailRow, Notice } from "$lib/shared/ui";

  type Props = {
    title: string;
    submitActionLabel: string;
    submitStatusLabel: string;
    submitOpeningFeeLabel: string;
    submitResultLabel: string;
    submitButtonLabel: string;
    submitDisabled: boolean;
    submitOnClick: () => void;
    preimageActionLabel: string;
    preimageStatusLabel: string;
    preimageNoteCostLabel: string;
    preimageResultLabel: string;
    preimageButtonLabel: string;
    preimageDisabled: boolean;
    preimageOnClick: () => void;
  };

  let {
    title,
    submitActionLabel,
    submitStatusLabel,
    submitOpeningFeeLabel,
    submitResultLabel,
    submitButtonLabel,
    submitDisabled,
    submitOnClick,
    preimageActionLabel,
    preimageStatusLabel,
    preimageNoteCostLabel,
    preimageResultLabel,
    preimageButtonLabel,
    preimageDisabled,
    preimageOnClick,
  }: Props = $props();
</script>

<div
  class="rounded-xl border bg-(--mono-bg) p-3 grid gap-2 text-[10px] text-(--mono-muted)"
>
  <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
    {title}
  </div>
  <div class="grid gap-2 @sm:grid-cols-2">
    <div class="rounded-lg border bg-white p-2 grid gap-1">
      <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
        Submit proposal
      </div>
      <DetailRow
        label="Action"
        value={submitActionLabel}
        valueClass="text-(--mono-text)"
      />
      <DetailRow
        label="Status"
        value={submitStatusLabel}
        valueClass="text-(--mono-text)"
      />
      <DetailRow
        label="Opening fee"
        value={submitOpeningFeeLabel}
        valueClass="text-(--mono-text)"
      />
      <DetailRow
        label="Result"
        value={submitResultLabel}
        valueClass="text-(--mono-text) break-all"
      />
      <Notice variant="muted"
        >Burns the quoted opening fee and stores the bounded payload hash at the
        governance boundary</Notice
      >
      <Button
        size="sm"
        variant="secondary"
        class="justify-center"
        disabled={submitDisabled}
        onclick={submitOnClick}>{submitButtonLabel}</Button
      >
    </div>
    <div class="rounded-lg border bg-white p-2 grid gap-1">
      <div class="text-[10px] uppercase tracking-wider text-(--mono-muted)">
        Note preimage
      </div>
      <DetailRow
        label="Action"
        value={preimageActionLabel}
        valueClass="text-(--mono-text)"
      />
      <DetailRow
        label="Status"
        value={preimageStatusLabel}
        valueClass="text-(--mono-text)"
      />
      <DetailRow
        label="Note cost"
        value={preimageNoteCostLabel}
        valueClass="text-(--mono-text)"
      />
      <DetailRow
        label="Result"
        value={preimageResultLabel}
        valueClass="text-(--mono-text) break-all"
      />
      <Notice variant="muted"
        >Optional separate path that publishes the same bounded payload bytes
        for later on-chain lookup</Notice
      >
      <Button
        size="sm"
        variant="secondary"
        class="justify-center"
        disabled={preimageDisabled}
        onclick={preimageOnClick}>{preimageButtonLabel}</Button
      >
    </div>
  </div>
</div>
