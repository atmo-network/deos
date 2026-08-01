import aaaAbiManifest from './aaa-abi-manifest.json' with { type: 'json' };

import {
  parseUnsignedHexByte,
  parseUnsignedHexNumber,
} from '../format/numeric.ts';

function publicU32Constant(name: string): number {
  const constant = aaaAbiManifest.pallet.constants.find(
    (candidate) => candidate.name === name,
  );
  if (!constant || !/^0x[0-9a-f]{8}$/i.test(constant.value)) {
    throw new Error(`AAA ABI manifest lacks a valid u32 ${name} constant`);
  }
  // SCALE encodes u32 little-endian; decode each byte via the shared boundary.
  const bytes = Uint8Array.from(
    constant.value
      .slice(2)
      .match(/.{2}/gu)!
      .map((byte) => {
        const decoded = parseUnsignedHexByte(byte);
        if (decoded === null) {
          throw new Error(`AAA ABI manifest u32 ${name} byte does not decode`);
        }
        return decoded;
      }),
  );
  return new DataView(bytes.buffer).getUint32(0, true);
}

function publicU8Constant(name: string): number {
  const constant = aaaAbiManifest.pallet.constants.find(
    (candidate) => candidate.name === name,
  );
  if (!constant || !/^0x[0-9a-f]{2}$/i.test(constant.value)) {
    throw new Error(`AAA ABI manifest lacks a valid u8 ${name} constant`);
  }
  const parsed = parseUnsignedHexNumber(constant.value.slice(2));
  if (parsed === null) {
    throw new Error(`AAA ABI manifest u8 ${name} constant does not decode`);
  }
  return parsed;
}

export const AAA_MAX_EXECUTION_PLAN_STEPS = publicU32Constant(
  'MaxExecutionPlanSteps',
);
export const AAA_MAX_RETRY_ATTEMPTS = publicU32Constant('MaxRetryAttempts');
export const AAA_MAX_OWNER_SLOTS = publicU8Constant('MaxOwnerSlots');
