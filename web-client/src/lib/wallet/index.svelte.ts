import {
  DEFAULT_TMCTOL_DAPP_NAME,
  TMCTOL_DEV_SIGNER_PRESETS,
  discoverInjectedSignerAccounts,
  hasBuiltInDevSigner,
  injectedSignerAvailability,
  isValidTmctolAddress,
  type TmctolInjectedSignerAccount,
  type TmctolInjectedSignerAvailability,
} from "$lib/adapters/blockchain/signer";
import { readStoredString, writeStoredString } from "$lib/shared/persistence";

export type WalletAccountSource = "dev" | "injected" | "custom";
export type WalletSignerStatus = "available" | "readonly" | "unavailable";
export type WalletAccountOption = {
  address: string;
  label: string;
  source: WalletAccountSource;
  extensionName: string | null;
  name: string | null;
  note: string | null;
  suri: string | null;
};
export type WalletState = {
  selectedAddress: string;
  selectedLabel: string;
  selectedSource: WalletAccountSource;
  accountInput: string;
  signerStatus: WalletSignerStatus;
  signerMessage: string;
  availability: TmctolInjectedSignerAvailability;
  injectedAccounts: WalletAccountOption[];
  devAccounts: WalletAccountOption[];
  loadingInjectedAccounts: boolean;
  lastError: string | null;
};

function buildDevAccount(
  spec: (typeof TMCTOL_DEV_SIGNER_PRESETS)[number],
): WalletAccountOption {
  return {
    address: spec.address,
    label: spec.label,
    source: "dev",
    extensionName: null,
    name: spec.label,
    note: `Zombienet preset ${spec.suri}`,
    suri: spec.suri,
  };
}

function shortenAddress(address: string): string {
  if (address.length <= 14) {
    return address;
  }
  return `${address.slice(0, 6)}…${address.slice(-6)}`;
}

function injectedAccountLabel(account: TmctolInjectedSignerAccount): string {
  if (account.name && account.name.trim().length > 0) {
    return account.name.trim();
  }
  return `${account.extensionName} · ${shortenAddress(account.address)}`;
}

const DEV_ACCOUNTS = TMCTOL_DEV_SIGNER_PRESETS.map(buildDevAccount);
const DEFAULT_DEV_ACCOUNT = DEV_ACCOUNTS[0];
const WALLET_STORAGE_KEY = "tmctol.wallet.selected-address";

class WalletStore {
  state: WalletState = $state({
    selectedAddress: DEFAULT_DEV_ACCOUNT.address,
    selectedLabel: DEFAULT_DEV_ACCOUNT.label,
    selectedSource: DEFAULT_DEV_ACCOUNT.source,
    accountInput: DEFAULT_DEV_ACCOUNT.address,
    signerStatus: "available",
    signerMessage: `In-browser dev signer enabled for ${DEFAULT_DEV_ACCOUNT.suri}. Use this only for local Zombienet-style testing.`,
    availability: injectedSignerAvailability(),
    injectedAccounts: [],
    devAccounts: DEV_ACCOUNTS,
    loadingInjectedAccounts: false,
    lastError: null,
  });

  async init(): Promise<void> {
    const persistedAddress = readStoredString(WALLET_STORAGE_KEY)?.trim() ?? "";
    if (persistedAddress.length > 0) {
      this.setSelectedAddress(persistedAddress);
    }
    this.refreshAvailability();
  }

  refreshAvailability(): void {
    this.state.availability = injectedSignerAvailability();
    this.updateSignerSupport();
  }

  async connectInjectedAccounts(): Promise<void> {
    this.state.loadingInjectedAccounts = true;
    this.state.lastError = null;
    try {
      const accounts = await discoverInjectedSignerAccounts(
        DEFAULT_TMCTOL_DAPP_NAME,
      );
      this.state.injectedAccounts = accounts.map((account) => ({
        address: account.address,
        label: injectedAccountLabel(account),
        source: "injected",
        extensionName: account.extensionName,
        name: account.name,
        note: `Injected by ${account.extensionName}`,
        suri: null,
      }));
      this.promoteSelectedAccountFromConnectedSources();
      this.refreshAvailability();
    } catch (error) {
      this.state.lastError =
        error instanceof Error
          ? error.message
          : "Unknown wallet discovery error";
    } finally {
      this.state.loadingInjectedAccounts = false;
      this.updateSignerSupport();
    }
  }

  canSelectAccountInput(input: string): boolean {
    const normalized = input.trim();
    return (
      normalized.length > 0 &&
      (hasBuiltInDevSigner(normalized) || isValidTmctolAddress(normalized))
    );
  }

  setSelectedAddress(input: string): void {
    const normalized = input.trim();
    if (normalized.length === 0) {
      this.selectAccount(DEFAULT_DEV_ACCOUNT);
      return;
    }
    const alias = normalized.toLowerCase();
    if (alias === "david") {
      const dave = this.state.devAccounts.find(
        (account) => account.label === "Dave",
      );
      if (dave) {
        this.selectAccount(dave);
        return;
      }
    }
    const matchedDev = this.state.devAccounts.find((account) => {
      return (
        account.address === normalized ||
        account.label.toLowerCase() === alias ||
        account.suri?.toLowerCase() === alias ||
        alias === `//${account.label.toLowerCase()}`
      );
    });
    if (matchedDev) {
      this.selectAccount(matchedDev);
      return;
    }
    const matchedInjected = this.state.injectedAccounts.find(
      (account) => account.address === normalized,
    );
    if (matchedInjected) {
      this.selectAccount(matchedInjected);
      return;
    }
    this.selectAccount({
      address: normalized,
      label: shortenAddress(normalized),
      source: "custom",
      extensionName: null,
      name: null,
      note: null,
      suri: null,
    });
  }

  selectInjectedAccount(address: string): void {
    const account = this.state.injectedAccounts.find(
      (candidate) => candidate.address === address,
    );
    if (account) {
      this.selectAccount(account);
    }
  }

  selectDevAccount(label: string): void {
    const account = this.state.devAccounts.find(
      (candidate) => candidate.label === label,
    );
    if (account) {
      this.selectAccount(account);
    }
  }

  private selectAccount(account: WalletAccountOption): void {
    this.state.selectedAddress = account.address;
    this.state.selectedLabel = account.label;
    this.state.selectedSource = account.source;
    this.state.accountInput = account.address;
    writeStoredString(WALLET_STORAGE_KEY, account.address);
    this.updateSignerSupport();
  }

  private promoteSelectedAccountFromConnectedSources(): void {
    const selectedAddress = this.state.selectedAddress.trim();
    if (selectedAddress.length === 0) {
      return;
    }
    const matchedInjected = this.state.injectedAccounts.find(
      (account) => account.address === selectedAddress,
    );
    if (matchedInjected) {
      this.state.selectedLabel = matchedInjected.label;
      this.state.selectedSource = matchedInjected.source;
      return;
    }
    const matchedDev = this.state.devAccounts.find(
      (account) => account.address === selectedAddress,
    );
    if (matchedDev) {
      this.state.selectedLabel = matchedDev.label;
      this.state.selectedSource = matchedDev.source;
    }
  }

  private updateSignerSupport(): void {
    const selectedAddress = this.state.selectedAddress.trim();
    if (selectedAddress.length === 0) {
      this.state.signerStatus = "unavailable";
      this.state.signerMessage =
        "Select or paste an account before sending transactions";
      return;
    }
    const matchedInjected = this.state.injectedAccounts.find(
      (account) => account.address === selectedAddress,
    );
    if (matchedInjected) {
      this.state.signerStatus = "available";
      this.state.signerMessage = `Injected signer ready via ${matchedInjected.extensionName}`;
      return;
    }
    const matchedDev = this.state.devAccounts.find(
      (account) => account.address === selectedAddress,
    );
    if (matchedDev) {
      this.state.signerStatus = "available";
      this.state.signerMessage = `In-browser dev signer enabled for ${matchedDev.suri}. Use this only for local Zombienet-style testing.`;
      return;
    }
    if (this.state.selectedSource === "custom") {
      this.state.signerStatus = "readonly";
      this.state.signerMessage =
        this.state.availability.status === "available"
          ? "Custom address loaded in watch-only mode. Connect an injected signer for the same address to submit transactions."
          : `Custom address loaded in watch-only mode. ${this.state.availability.message}`;
      return;
    }
    if (this.state.availability.status === "no-extension") {
      this.state.signerStatus = "unavailable";
      this.state.signerMessage = "No injected wallet extension detected";
      return;
    }
    if (this.state.availability.status === "browser-unsupported") {
      this.state.signerStatus = "unavailable";
      this.state.signerMessage = this.state.availability.message;
      return;
    }
    this.state.signerStatus = "unavailable";
    this.state.signerMessage =
      "Selected address is not present in the connected injected wallet accounts";
  }

  get selectedAddress(): string {
    return this.state.selectedAddress;
  }
}

export const walletStore = new WalletStore();
