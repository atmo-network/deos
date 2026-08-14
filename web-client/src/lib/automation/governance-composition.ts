/*
Domain: Actors control-plane call composition
Owns: Metadata-bound Actors RuntimeCall bytes, preimage identity, origin requirements, and governance-admission classification.
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

import { ACTORS_MAX_OWNER_SLOTS } from './actors-protocol-bounds.ts';
import {
  type ActorContractArtifact,
  type ActorContractHex,
  type ActorContractRuntimeIdentity,
  inspectActorContractArtifact,
} from './contract-artifact.ts';

export type ActorCompositionTarget =
  | { type: 'Create'; owner?: string; ownerSlot?: number }
  | { type: 'Activate'; actorId: bigint }
  | { type: 'ReattachSystem'; sovereignId: bigint; owner: string };

export type ActorGovernanceComposition = {
  contractId: ActorContractHex;
  runtime: ActorContractRuntimeIdentity & { metadataHash: ActorContractHex };
  call: {
    pallet: string;
    method:
      | 'create_user_actor'
      | 'create_user_actor_at_slot'
      | 'create_system_actor'
      | 'activate_actor'
      | 'create_system_actor_at_sovereign_id';
    bytes: ActorContractHex;
    byteLength: number;
    hash: ActorContractHex;
  };
  authority: {
    requiredOrigin: 'OwnerSigned' | 'Root';
    governanceDomain: 'StrategicNative' | null;
  };
  preimage: {
    bytes: ActorContractHex;
    hash: ActorContractHex;
    governanceAdmission: 'DirectCallOnly' | 'UnsupportedActorRootCall';
    reason: string;
  };
};

function bytesToHex(bytes: Uint8Array): ActorContractHex {
  let value = '0x';
  for (const byte of bytes) value += byte.toString(16).padStart(2, '0');
  return value as ActorContractHex;
}

function validateActorId(value: bigint) {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error('actorId must fit the runtime u64 contract');
  }
}

function validateOwner(owner: string | undefined) {
  if (owner == null || owner.trim().length === 0) {
    throw new Error('System Actors composition requires an owner account');
  }
  return owner.trim();
}

export function composeActorRuntimeCall(input: {
  artifact: ActorContractArtifact;
  metadataBytes: Uint8Array;
  runtime: ActorContractRuntimeIdentity;
  target: ActorCompositionTarget;
}): ActorGovernanceComposition {
  const inspection = inspectActorContractArtifact(
    input.artifact,
    input.metadataBytes,
    input.runtime,
  );
  if (!inspection.valid) {
    throw new Error(
      `Invalid Actors Actor Contract artifact: ${inspection.errors.join('; ')}`,
    );
  }

  const mutability = {
    type: input.artifact.mutability,
    value: undefined,
  };
  const contract = inspection.runtimeValue;
  let method: ActorGovernanceComposition['call']['method'];
  let callValue: unknown;
  let requiredOrigin: ActorGovernanceComposition['authority']['requiredOrigin'];

  switch (input.target.type) {
    case 'Create':
      if (input.artifact.actorType === 'User') {
        if (input.target.owner != null) {
          throw new Error('User Actors ownership derives from the signer');
        }
        if (input.target.ownerSlot == null) {
          method = 'create_user_actor';
          callValue = { mutability, contract };
        } else {
          if (
            !Number.isSafeInteger(input.target.ownerSlot) ||
            input.target.ownerSlot < 0 ||
            input.target.ownerSlot >= ACTORS_MAX_OWNER_SLOTS
          ) {
            throw new Error(
              `ownerSlot must be within 0..${ACTORS_MAX_OWNER_SLOTS - 1} per runtime MaxOwnerSlots`,
            );
          }
          method = 'create_user_actor_at_slot';
          callValue = {
            owner_slot: input.target.ownerSlot,
            mutability,
            contract,
          };
        }
        requiredOrigin = 'OwnerSigned';
      } else {
        if (input.target.ownerSlot != null) {
          throw new Error(
            'System Actors creation does not accept an owner slot',
          );
        }
        method = 'create_system_actor';
        callValue = {
          owner: validateOwner(input.target.owner),
          mutability,
          contract,
        };
        requiredOrigin = 'Root';
      }
      break;
    case 'Activate':
      validateActorId(input.target.actorId);
      method = 'activate_actor';
      callValue = { actor_id: input.target.actorId, contract };
      requiredOrigin =
        input.artifact.actorType === 'User' ? 'OwnerSigned' : 'Root';
      break;
    case 'ReattachSystem':
      if (input.artifact.actorType !== 'System') {
        throw new Error(
          'Only a System Actors artifact can attach to System custody',
        );
      }
      validateActorId(input.target.sovereignId);
      method = 'create_system_actor_at_sovereign_id';
      callValue = {
        sovereign_id: input.target.sovereignId,
        owner: validateOwner(input.target.owner),
        mutability,
        contract,
      };
      requiredOrigin = 'Root';
      break;
  }

  const metadata = unifyMetadata(decAnyMetadata(input.metadataBytes));
  const actorPallets = metadata.pallets.filter((pallet) => {
    if (pallet.calls == null) return false;
    return (
      metadata.lookup[pallet.calls.type]?.path?.join('::') ===
      'pallet_deos_actors::pallet::Call'
    );
  });
  if (actorPallets.length !== 1) {
    throw new Error(
      'Runtime metadata must expose exactly one pallet-deos-actors call surface',
    );
  }
  if (!('outerEnums' in metadata)) {
    throw new Error(
      'Actors call composition requires V15+ outer-enum metadata',
    );
  }
  const codec = getDynamicBuilder(getLookupFn(metadata)).buildDefinition(
    metadata.outerEnums.call,
  );
  const bytes = codec.enc({
    type: actorPallets[0].name,
    value: { type: method, value: callValue },
  });
  const roundTrip = codec.enc(codec.dec(bytes));
  const callBytes = bytesToHex(bytes);
  if (bytesToHex(roundTrip) !== callBytes) {
    throw new Error(
      'RuntimeCall must decode and re-encode to exact SCALE bytes',
    );
  }
  const callHash = blake2AsHex(bytes, 256) as ActorContractHex;
  const directOwnerCall = requiredOrigin === 'OwnerSigned';

  return {
    contractId: input.artifact.contractId,
    runtime: {
      ...input.runtime,
      metadataHash: input.artifact.metadataHash,
    },
    call: {
      pallet: actorPallets[0].name,
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
            'Owner-controlled User Actors calls require a signer and do not enter governance.',
        }
      : {
          bytes: callBytes,
          hash: callHash,
          governanceAdmission: 'UnsupportedActorRootCall',
          reason:
            'Current L1RootAction accepts only the dedicated runtime-upgrade payload, not arbitrary Actors RuntimeCall bytes.',
        },
  };
}
