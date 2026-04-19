<script lang="ts">
  import { onMount } from "svelte";

  import { walletStore } from "$lib/wallet/index.svelte";
  import { Button, Notice, SectionCard, SelectableTile, StatCard, TextField } from "$lib/shared/ui";

  let rootEl = $state<HTMLElement | null>(null);
  let viewport = $state({ width: 0, height: 0 });
  let customAccountInput = $state(walletStore.state.accountInput);

  const customAccountState = $derived.by(() => {
    const input = customAccountInput.trim();
    if (input.length === 0) {
      return {
        disabled: true,
        message: "Enter Alice, //Alice, or a valid DEOS address to change the account context",
        variant: "muted" as const,
      };
    }
    if (!walletStore.canSelectAccountInput(input)) {
      return {
        disabled: true,
        message: "Use a built-in dev alias like Alice / //Alice or a valid DEOS ss58 address",
        variant: "warn" as const,
      };
    }
    return {
      disabled: false,
      message: "Selecting this account will refresh the live wallet, system, and governance views",
      variant: "muted" as const,
    };
  });

  const compactPane = $derived(
    viewport.width > 0 && viewport.width < 430,
  );
  const densePane = $derived(
    viewport.width > 0 && viewport.width < 340,
  );

  function syncViewport() {
    if (!rootEl) {
      viewport = { width: 0, height: 0 };
      return;
    }
    viewport = {
      width: rootEl.clientWidth,
      height: rootEl.clientHeight,
    };
  }

  function shortAddress(address: string): string {
    if (address.length <= 18) {
      return address;
    }
    return `${address.slice(0, 8)}…${address.slice(-8)}`;
  }

  async function refreshAll(): Promise<void> {
    const [{ governanceStore }, { systemStore }] = await Promise.all([
      import("$lib/governance/index.svelte"),
      import("$lib/system/index.svelte"),
    ]);
    await Promise.all([systemStore.refresh(), governanceStore.refresh()]);
  }

  async function chooseAccount(input: string): Promise<void> {
    if (!walletStore.canSelectAccountInput(input)) {
      return;
    }
    walletStore.setSelectedAddress(input);
    customAccountInput = walletStore.state.accountInput;
    await refreshAll();
  }

  async function handleCustomAccountKeydown(event: KeyboardEvent): Promise<void> {
    if (event.key !== "Enter" || customAccountState.disabled) {
      return;
    }
    event.preventDefault();
    await chooseAccount(customAccountInput);
  }

  async function connectInjectedAccounts(): Promise<void> {
    await walletStore.connectInjectedAccounts();
    await refreshAll();
  }

  onMount(() => {
    syncViewport();
    if (!rootEl) {
      return;
    }
    const resizeObserver = new ResizeObserver(() => syncViewport());
    resizeObserver.observe(rootEl);
    return () => resizeObserver.disconnect();
  });
</script>

<section bind:this={rootEl} class="grid gap-3">
  <SectionCard title="Account status" class="text-xs">
    <div class={[
      "grid gap-2",
      densePane ? "grid-cols-1" : "grid-cols-2",
    ]}>
      <StatCard label="Account" value={walletStore.state.selectedLabel} />
      <StatCard label="Signer" value={walletStore.state.signerStatus === "readonly" ? "watch-only" : walletStore.state.signerStatus} />
    </div>
    <StatCard
      label="Address"
      value={densePane ? shortAddress(walletStore.state.selectedAddress) : walletStore.state.selectedAddress}
      detail={densePane ? walletStore.state.selectedAddress : undefined}
      class="break-all"
    />
    <div class="text-[10px] text-(--mono-muted)">{walletStore.state.signerMessage}</div>
  </SectionCard>

  <SectionCard title="Zombienet presets" class="text-xs">
    <div class={[
      "grid gap-2",
      densePane ? "grid-cols-1" : "grid-cols-2",
    ]}>
      {#each walletStore.state.devAccounts as account}
        <SelectableTile
          onclick={() => chooseAccount(account.label)}
          selected={account.address === walletStore.state.selectedAddress}
        >
          <div class="font-medium text-(--mono-text)">{account.label}</div>
          <div class="text-[10px] text-(--mono-muted)">{account.suri}</div>
        </SelectableTile>
      {/each}
    </div>
  </SectionCard>

  <SectionCard title="Injected wallets" class="text-xs">
    {#snippet actions()}
      {#if walletStore.state.availability.status === "available"}
        <Button
          size="sm"
          onclick={connectInjectedAccounts}
          disabled={walletStore.state.loadingInjectedAccounts}
        >
          {walletStore.state.loadingInjectedAccounts ? "Connecting..." : "Connect"}
        </Button>
      {/if}
    {/snippet}
    {#if walletStore.state.lastError}
      <Notice variant="warn">{walletStore.state.lastError}</Notice>
    {/if}
    {#if walletStore.state.availability.status !== "available"}
      <Notice variant="muted">{walletStore.state.availability.message}</Notice>
    {/if}
    {#if walletStore.state.injectedAccounts.length === 0}
      <Notice>
        {walletStore.state.availability.status === "available"
          ? "No injected accounts loaded yet. Click Connect to request extension access."
          : "Injected account discovery is unavailable until a supported wallet extension is present."}
      </Notice>
    {:else}
      <div class={[
        "grid gap-2",
        compactPane ? "grid-cols-1" : "grid-cols-2",
      ]}>
        {#each walletStore.state.injectedAccounts as account}
          <SelectableTile
            onclick={() => chooseAccount(account.address)}
            selected={account.address === walletStore.state.selectedAddress}
          >
            <div class="font-medium text-(--mono-text)">{account.label}</div>
            <div class="text-[10px] text-(--mono-muted)">{account.extensionName}</div>
            <div class="break-all text-[10px] text-(--mono-muted)">{account.address}</div>
          </SelectableTile>
        {/each}
      </div>
    {/if}
  </SectionCard>

  <SectionCard title="Custom account" class="text-xs">
    <div class={[
      "grid gap-2",
      compactPane ? "grid-cols-1" : "grid-cols-[minmax(0,1fr)_auto] items-end",
    ]}>
      <TextField bind:value={customAccountInput} placeholder="Alice, //Alice, or DEOS address" onkeydown={handleCustomAccountKeydown} />
      <Button size="sm" class={compactPane ? "w-full" : "min-w-28"} onclick={() => chooseAccount(customAccountInput)} disabled={customAccountState.disabled}>Use account</Button>
    </div>
    <Notice variant={customAccountState.variant}>{customAccountState.message}</Notice>
    <div class="text-[10px] text-(--mono-muted)">
      This widget owns account selection and signer onboarding. Wallet surfaces below only act on the account selected here.
    </div>
  </SectionCard>
</section>
