<!--
Domain: Account widget
Owns: Wallet account selection panel, signer status presentation, and account input controls.
Excludes: Signer discovery implementation, adapter lifecycle, portfolio balances, and layout state.
Zone: Presentation widget; consumes local wallet state, system connection readiness, and UI Kit primitives.
-->
<script lang="ts">
  import { systemStore } from '$lib/system/index.svelte';
  import {
    Button,
    DisclosureSection,
    Notice,
    type NoticeVariant,
    SectionCard,
    SelectableTile,
    StatCard,
    TextField,
  } from '$lib/ui';
  import { walletStore } from '$lib/wallet/index.svelte';

  type CustomAccountState = {
    disabled: boolean;
    message: string;
    variant: NoticeVariant;
  };

  let customAccountInput = $state(walletStore.state.accountInput);

  const chainConnected = $derived(
    systemStore.connectionState?.status === 'connected',
  );
  const customAccountState = $derived.by((): CustomAccountState => {
    const input = customAccountInput.trim();
    if (input.length === 0) {
      return {
        disabled: true,
        message:
          'Enter Alice, //Alice, or a valid DEOS address to change the account context',
        variant: 'muted',
      };
    }
    if (!walletStore.canSelectAccountInput(input)) {
      return {
        disabled: true,
        message:
          'Use a built-in dev alias like Alice / //Alice or a valid DEOS ss58 address',
        variant: 'warn',
      };
    }
    return {
      disabled: false,
      message: chainConnected
        ? 'Selecting this account will refresh connected system and governance views'
        : 'Account selection stays local; chain-backed views update after the network reconnects',
      variant: 'muted',
    };
  });

  async function refreshConnectedViews(): Promise<void> {
    if (!chainConnected) {
      return;
    }
    const { governanceStore } = await import('$lib/governance/index.svelte');
    await Promise.all([systemStore.refresh(), governanceStore.refresh()]);
  }

  async function chooseAccount(input: string): Promise<void> {
    if (!walletStore.canSelectAccountInput(input)) {
      return;
    }
    walletStore.setSelectedAddress(input);
    customAccountInput = walletStore.state.accountInput;
    await refreshConnectedViews();
  }

  async function handleCustomAccountKeydown(
    event: KeyboardEvent,
  ): Promise<void> {
    if (event.key !== 'Enter' || customAccountState.disabled) {
      return;
    }
    event.preventDefault();
    await chooseAccount(customAccountInput);
  }

  async function connectInjectedAccounts(): Promise<void> {
    await walletStore.connectInjectedAccounts();
    await refreshConnectedViews();
  }
</script>

<section class="@container grid gap-3">
  <SectionCard title="Account status" class="text-xs">
    <div
      class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,9rem),1fr))] gap-2"
    >
      <StatCard label="Account" value={walletStore.state.selectedLabel} />
      <StatCard
        label="Signer"
        value={walletStore.state.signerStatus === 'readonly'
          ? 'watch-only'
          : walletStore.state.signerStatus}
      />
    </div>
    <StatCard
      label="Address"
      value={walletStore.state.selectedAddress}
      class="break-all"
    />
    <div class="text-2xs text-(--mono-muted)">
      {walletStore.state.signerMessage}
    </div>
    {#if !chainConnected}
      <Notice variant="muted">
        Account selection, address validation, and signer discovery remain
        local. Chain-backed widgets refresh after a network connection becomes
        available.
      </Notice>
    {/if}
  </SectionCard>

  <DisclosureSection title="Zombienet presets" class="text-xs">
    <div
      class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,13rem),1fr))] gap-2"
    >
      {#each walletStore.state.devAccounts as account}
        <SelectableTile
          onclick={() => chooseAccount(account.label)}
          selected={account.address === walletStore.state.selectedAddress}
        >
          <div class="font-medium text-(--mono-text)">{account.label}</div>
          <div class="text-2xs text-(--mono-muted)">{account.suri}</div>
        </SelectableTile>
      {/each}
    </div>
  </DisclosureSection>

  <DisclosureSection title="Injected wallets" class="text-xs">
    {#if walletStore.state.availability.status === 'available'}
      <Button
        size="sm"
        class="justify-self-start"
        onclick={connectInjectedAccounts}
        disabled={walletStore.state.loadingInjectedAccounts}
      >
        {walletStore.state.loadingInjectedAccounts
          ? 'Connecting...'
          : 'Connect wallet'}
      </Button>
    {/if}
    {#if walletStore.state.lastError}
      <Notice variant="warn">{walletStore.state.lastError}</Notice>
    {/if}
    {#if walletStore.state.availability.status !== 'available'}
      <Notice variant="muted">{walletStore.state.availability.message}</Notice>
    {/if}
    {#if walletStore.state.injectedAccounts.length === 0}
      <Notice>
        {walletStore.state.availability.status === 'available'
          ? 'No injected accounts loaded yet. Click Connect to request extension access.'
          : 'Injected account discovery is unavailable until a supported wallet extension is present.'}
      </Notice>
    {:else}
      <div
        class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,13rem),1fr))] gap-2"
      >
        {#each walletStore.state.injectedAccounts as account}
          <SelectableTile
            onclick={() => chooseAccount(account.address)}
            selected={account.address === walletStore.state.selectedAddress}
          >
            <div class="font-medium text-(--mono-text)">{account.label}</div>
            <div class="text-2xs text-(--mono-muted)">
              {account.extensionName}
            </div>
            <div class="break-all text-2xs text-(--mono-muted)">
              {account.address}
            </div>
          </SelectableTile>
        {/each}
      </div>
    {/if}
  </DisclosureSection>

  <DisclosureSection title="Custom account" class="text-xs">
    <div class="grid gap-2 @md:grid-cols-[minmax(0,1fr)_auto] @md:items-end">
      <TextField
        bind:value={customAccountInput}
        placeholder="Alice, //Alice, or DEOS address"
        onkeydown={handleCustomAccountKeydown}
      />
      <Button
        size="sm"
        class="w-full @lg:w-auto @lg:min-w-28"
        onclick={() => chooseAccount(customAccountInput)}
        disabled={customAccountState.disabled}>Use account</Button
      >
    </div>
    <Notice variant={customAccountState.variant}
      >{customAccountState.message}</Notice
    >
  </DisclosureSection>
</section>
