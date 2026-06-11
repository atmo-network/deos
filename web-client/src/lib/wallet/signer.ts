/*
Domain: Wallet signer integration
Owns: Injected-wallet discovery, dev signer presets, address validation, and signer lookup helpers.
Excludes: Selected-account store state, transaction submission policy, balances, and widget rendering.
Zone: Wallet integration helper; browser-facing signer boundary for wallet store and adapters.
*/
import { AccountId, type PolkadotSigner } from 'polkadot-api';
import {
  type InjectedExtension,
  connectInjectedExtension,
  getInjectedExtensions,
} from 'polkadot-api/pjs-signer';

export const DEFAULT_DEOS_DAPP_NAME = 'DEOS Web Client';

export type DeosInjectedSignerAccount = {
  address: string;
  extensionName: string;
  name: string | null;
  type: string | null;
};
export type DeosDevSignerPreset = {
  alias: string;
  label: string;
  suri: string;
  publicKeyHex: string;
  address: string;
};
export type DeosSignerMatch = {
  address: string;
  source: 'injected' | 'dev';
  extensionName: string | null;
  name: string | null;
  type: string | null;
  signer: PolkadotSigner;
  disconnect: () => void;
};
export type DeosInjectedSignerMatch = DeosSignerMatch & {
  source: 'injected';
  extensionName: string;
};
export type DeosInjectedSignerAvailability = {
  status: 'browser-unsupported' | 'no-extension' | 'available';
  extensionNames: string[];
  message: string;
};

type DevSignerSpec = {
  alias: string;
  label: string;
  suri: string;
  publicKeyHex: string;
};

const accountIdCodec = AccountId(42, 32);
const DEV_SIGNER_SPECS: readonly DevSignerSpec[] = [
  {
    alias: 'alice',
    label: 'Alice',
    suri: '//Alice',
    publicKeyHex:
      'd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d',
  },
  {
    alias: 'bob',
    label: 'Bob',
    suri: '//Bob',
    publicKeyHex:
      '8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48',
  },
  {
    alias: 'charlie',
    label: 'Charlie',
    suri: '//Charlie',
    publicKeyHex:
      '90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22',
  },
  {
    alias: 'dave',
    label: 'Dave',
    suri: '//Dave',
    publicKeyHex:
      '306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20',
  },
];

function hexToBytes(hex: string): Uint8Array {
  const normalized = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (normalized.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(normalized)) {
    throw new Error('Dev signer public key must be complete hex bytes');
  }
  const bytes = new Uint8Array(normalized.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    const offset = index * 2;
    bytes[index] = Number(`0x${normalized.slice(offset, offset + 2)}`);
  }
  return bytes;
}

export const DEOS_DEV_SIGNER_PRESETS: DeosDevSignerPreset[] =
  DEV_SIGNER_SPECS.map((preset) => ({
    ...preset,
    address: accountIdCodec.dec(hexToBytes(preset.publicKeyHex)),
  }));

export function isValidDeosAddress(address: string): boolean {
  const normalized = address.trim();
  if (normalized.length === 0) {
    return false;
  }
  try {
    accountIdCodec.enc(normalized);
    return true;
  } catch {
    return false;
  }
}

export function injectedSignerExtensionNames(): string[] {
  return typeof window === 'undefined' ? [] : getInjectedExtensions();
}

export function injectedSignerAvailability(): DeosInjectedSignerAvailability {
  if (typeof window === 'undefined') {
    return {
      status: 'browser-unsupported',
      extensionNames: [],
      message: 'Injected wallet discovery is only available in the browser',
    };
  }
  const extensionNames = injectedSignerExtensionNames();
  if (extensionNames.length === 0) {
    return {
      status: 'no-extension',
      extensionNames,
      message: 'No injected wallet extension detected',
    };
  }
  return {
    status: 'available',
    extensionNames,
    message: `Injected wallet extensions detected: ${extensionNames.join(', ')}`,
  };
}

export async function discoverInjectedSignerAccounts(
  dappName: string = DEFAULT_DEOS_DAPP_NAME,
): Promise<DeosInjectedSignerAccount[]> {
  if (typeof window === 'undefined') {
    return [];
  }
  const discovered: DeosInjectedSignerAccount[] = [];
  const seenAddresses = new Set<string>();
  for (const extensionName of injectedSignerExtensionNames()) {
    let extension: InjectedExtension | null = null;
    try {
      extension = await connectInjectedExtension(extensionName, dappName);
      for (const account of extension.getAccounts()) {
        if (seenAddresses.has(account.address)) {
          continue;
        }
        seenAddresses.add(account.address);
        discovered.push({
          address: account.address,
          extensionName,
          name: account.name ?? null,
          type: account.type ?? null,
        });
      }
    } catch {
      // Ignore broken/incompatible extensions during discovery
    } finally {
      extension?.disconnect();
    }
  }
  return discovered;
}

function matchDevPreset(addressOrAlias: string): DeosDevSignerPreset | null {
  const normalized = addressOrAlias.trim();
  if (normalized.length === 0) {
    return null;
  }
  const lowered = normalized.toLowerCase();
  return (
    DEOS_DEV_SIGNER_PRESETS.find((preset) => {
      return (
        preset.address === normalized ||
        preset.alias === lowered ||
        preset.label.toLowerCase() === lowered ||
        preset.suri.toLowerCase() === lowered ||
        lowered === `//${preset.alias}` ||
        (lowered === 'david' && preset.alias === 'dave')
      );
    }) ?? null
  );
}

export function hasBuiltInDevSigner(addressOrAlias: string | null): boolean {
  if (!addressOrAlias) {
    return false;
  }
  return matchDevPreset(addressOrAlias) !== null;
}

export async function connectDevSigner(
  addressOrAlias: string,
): Promise<DeosSignerMatch | null> {
  if (typeof window === 'undefined') {
    return null;
  }
  const preset = matchDevPreset(addressOrAlias);
  if (!preset) {
    return null;
  }
  const [{ getPolkadotSigner }, { Keyring }, { cryptoWaitReady }] =
    await Promise.all([
      import('@polkadot-api/signer'),
      import('@polkadot/keyring'),
      import('@polkadot/util-crypto'),
    ]);
  await cryptoWaitReady();
  const keyring = new Keyring({ type: 'sr25519', ss58Format: 42 });
  const pair = keyring.createFromUri(
    preset.suri,
    { name: preset.label },
    'sr25519',
  );
  return {
    address: pair.address,
    source: 'dev',
    extensionName: null,
    name: preset.label,
    type: 'sr25519',
    signer: getPolkadotSigner(pair.publicKey, 'Sr25519', (input) =>
      pair.sign(input),
    ),
    disconnect: () => {},
  };
}

export async function connectInjectedSigner(
  address: string,
  dappName: string = DEFAULT_DEOS_DAPP_NAME,
): Promise<DeosInjectedSignerMatch | null> {
  const normalizedAddress = address.trim();
  if (normalizedAddress.length === 0 || typeof window === 'undefined') {
    return null;
  }
  for (const extensionName of injectedSignerExtensionNames()) {
    let extension: InjectedExtension | null = null;
    try {
      extension = await connectInjectedExtension(extensionName, dappName);
      const account = extension
        .getAccounts()
        .find((candidate) => candidate.address === normalizedAddress);
      if (!account) {
        extension.disconnect();
        continue;
      }
      const matchedExtension = extension;
      return {
        address: account.address,
        source: 'injected',
        extensionName,
        name: account.name ?? null,
        type: account.type ?? null,
        signer: account.polkadotSigner,
        disconnect: () => matchedExtension.disconnect(),
      };
    } catch {
      extension?.disconnect();
    }
  }
  return null;
}

export async function connectDeosSigner(
  addressOrAlias: string,
  dappName: string = DEFAULT_DEOS_DAPP_NAME,
): Promise<DeosSignerMatch | null> {
  const injectedSigner = await connectInjectedSigner(addressOrAlias, dappName);
  if (injectedSigner) {
    return injectedSigner;
  }
  return await connectDevSigner(addressOrAlias);
}
