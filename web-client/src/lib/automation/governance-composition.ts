/*
Domain: AAA control-plane call composition
Owns: Metadata-bound AAA RuntimeCall bytes, preimage identity, origin requirements, and governance-admission classification.
Excludes: Proposal advocacy, signing, preimage noting, submission, voting, enactment, and runtime mutation.
Zone: Automation domain capability; current governance support remains derived from shipped DEOS payload contracts.
*/
import {
  getDynamicBuilder,
  getLookupFn,
} from '@polkadot-api/metadata-builders';
import {
  decAnyMetadata,
  unifyMetadata,
} from '@polkadot-api/substrate-bindings';
import { blake2AsHex } from '@polkadot/util-crypto';

import {
  type AaaPlanArtifact,
  type AaaPlanHex,
  type AaaPlanRuntimeIdentity,
  inspectAaaPlanArtifact,
} from './plan-artifact.ts';

export type AaaCompositionTarget =
  | { type: 'Create'; owner?: string; ownerSlot?: number }
  | { type: 'Activate'; aaaId: bigint }
  | { type: 'ReopenSystem'; aaaId: bigint; owner: string };

export type AaaGovernanceComposition = {
  planId: AaaPlanHex;
  runtime: AaaPlanRuntimeIdentity & { metadataHash: AaaPlanHex };
  call: {
    pallet: string;
    method:
      | 'create_user_aaa'
      | 'create_user_aaa_at_slot'
      | 'create_system_aaa'
      | 'activate_aaa'
      | 'reopen_system_aaa';
    bytes: AaaPlanHex;
    byteLength: number;
    hash: AaaPlanHex;
  };
  authority: {
    requiredOrigin: 'OwnerSigned' | 'Root';
    governanceDomain: 'StrategicNative' | null;
  };
  preimage: {
    bytes: AaaPlanHex;
    hash: AaaPlanHex;
    governanceAdmission: 'DirectCallOnly' | 'UnsupportedAaaRootCall';
    reason: string;
  };
};

function bytesToHex(bytes: Uint8Array): AaaPlanHex {
  let value = '0x';
  for (const byte of bytes) value += byte.toString(16).padStart(2, '0');
  return value as AaaPlanHex;
}

function validateAaaId(value: bigint) {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error('aaaId must fit the runtime u64 contract');
  }
}

function validateOwner(owner: string | undefined) {
  if (owner == null || owner.trim().length === 0) {
    throw new Error('System AAA composition requires an owner account');
  }
  return owner.trim();
}

export function composeAaaRuntimeCall(input: {
  artifact: AaaPlanArtifact;
  metadataBytes: Uint8Array;
  runtime: AaaPlanRuntimeIdentity;
  target: AaaCompositionTarget;
}): AaaGovernanceComposition {
  const inspection = inspectAaaPlanArtifact(
    input.artifact,
    input.metadataBytes,
    input.runtime,
  );
  if (!inspection.valid) {
    throw new Error(
      `Invalid AAA plan artifact: ${inspection.errors.join('; ')}`,
    );
  }

  const mutability = {
    type: input.artifact.mutability,
    value: undefined,
  };
  const program = inspection.runtimeValue;
  let method: AaaGovernanceComposition['call']['method'];
  let callValue: unknown;
  let requiredOrigin: AaaGovernanceComposition['authority']['requiredOrigin'];

  switch (input.target.type) {
    case 'Create':
      if (input.artifact.aaaType === 'User') {
        if (input.target.owner != null) {
          throw new Error('User AAA ownership derives from the signer');
        }
        if (input.target.ownerSlot == null) {
          method = 'create_user_aaa';
          callValue = { mutability, program };
        } else {
          if (
            !Number.isSafeInteger(input.target.ownerSlot) ||
            input.target.ownerSlot < 0 ||
            input.target.ownerSlot > 0xff
          ) {
            throw new Error('ownerSlot must fit the runtime u8 contract');
          }
          method = 'create_user_aaa_at_slot';
          callValue = {
            owner_slot: input.target.ownerSlot,
            mutability,
            program,
          };
        }
        requiredOrigin = 'OwnerSigned';
      } else {
        if (input.target.ownerSlot != null) {
          throw new Error('System AAA creation does not accept an owner slot');
        }
        method = 'create_system_aaa';
        callValue = {
          owner: validateOwner(input.target.owner),
          mutability,
          program,
        };
        requiredOrigin = 'Root';
      }
      break;
    case 'Activate':
      validateAaaId(input.target.aaaId);
      method = 'activate_aaa';
      callValue = { aaa_id: input.target.aaaId, program };
      requiredOrigin =
        input.artifact.aaaType === 'User' ? 'OwnerSigned' : 'Root';
      break;
    case 'ReopenSystem':
      if (input.artifact.aaaType !== 'System') {
        throw new Error('Only a System AAA artifact can reopen a System AAA');
      }
      validateAaaId(input.target.aaaId);
      method = 'reopen_system_aaa';
      callValue = {
        aaa_id: input.target.aaaId,
        owner: validateOwner(input.target.owner),
        mutability,
        program,
      };
      requiredOrigin = 'Root';
      break;
  }

  const metadata = unifyMetadata(decAnyMetadata(input.metadataBytes));
  const aaaPallets = metadata.pallets.filter((pallet) => {
    if (pallet.calls == null) return false;
    return (
      metadata.lookup[pallet.calls.type]?.path?.join('::') ===
      'pallet_aaa::pallet::Call'
    );
  });
  if (aaaPallets.length !== 1) {
    throw new Error(
      'Runtime metadata must expose exactly one pallet-aaa call surface',
    );
  }
  if (!('outerEnums' in metadata)) {
    throw new Error('AAA call composition requires V15+ outer-enum metadata');
  }
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    metadata.outerEnums.call,
  );
  const bytes = codec.enc({
    type: aaaPallets[0].name,
    value: { type: method, value: callValue },
  });
  const roundTrip = codec.enc(codec.dec(bytes));
  const callBytes = bytesToHex(bytes);
  if (bytesToHex(roundTrip) !== callBytes) {
    throw new Error(
      'RuntimeCall must decode and re-encode to exact SCALE bytes',
    );
  }
  const callHash = blake2AsHex(bytes, 256) as AaaPlanHex;
  const directOwnerCall = requiredOrigin === 'OwnerSigned';

  return {
    planId: input.artifact.planId,
    runtime: {
      ...input.runtime,
      metadataHash: input.artifact.metadataHash,
    },
    call: {
      pallet: aaaPallets[0].name,
      method,
      bytes: callBytes,
      byteLength: bytes.length,
      hash: callHash,
    },
    authority: {
      requiredOrigin,
      governanceDomain: requiredOrigin === 'Root' ? 'StrategicNative' : null,
    },
    preimage: directOwnerCall
      ? {
          bytes: callBytes,
          hash: callHash,
          governanceAdmission: 'DirectCallOnly',
          reason:
            'Owner-controlled User AAA calls require a signer and do not enter governance.',
        }
      : {
          bytes: callBytes,
          hash: callHash,
          governanceAdmission: 'UnsupportedAaaRootCall',
          reason:
            'Current L1RootAction accepts only the dedicated runtime-upgrade payload, not arbitrary AAA RuntimeCall bytes.',
        },
  };
}
