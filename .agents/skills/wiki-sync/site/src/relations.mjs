/*
Domain: Wiki relation vocabulary adapter
Owns: Reuse of the project vocabulary while the legacy client renderer remains installed.
Excludes: Duplicate labels, HTML rendering and web-client runtime initialization.
*/
export {
  formatRussianRelation,
  russianRelationLabels,
  unknownRussianRelationLabel,
} from '../../../../../web-client/src/lib/wiki/relation-phrases.ts';
