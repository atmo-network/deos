import Ajv from 'ajv';

export function validateSpdxSchema(schema, document) {
  const ajv = new Ajv({ allErrors: true, strict: true, validateFormats: true });
  let validate;
  try {
    validate = ajv.compile(schema);
  } catch (error) {
    throw new Error(`Pinned SPDX schema could not be compiled: ${error.message}`);
  }
  if (!validate(document)) {
    const details = ajv.errorsText(validate.errors, { separator: '; ' });
    throw new Error(`SPDX schema validation failed: ${details}`);
  }
  return document;
}
