/*
Domain: Governance session state
Owns: Mutable browser-session governance domain selection helpers.
Excludes: Governance constants, provider lifecycle, proposal data, and persistence policy.
Zone: Governance session helper; consumed by governance store and UX controls.
*/
let governanceDomainId = 1000;

export function getGovernanceDomainId(): number {
  return governanceDomainId;
}

export function isValidGovernanceDomainId(domainId: number): boolean {
  return Number.isSafeInteger(domainId) && domainId >= 0;
}

export function setGovernanceDomainId(domainId: number): boolean {
  if (!isValidGovernanceDomainId(domainId)) {
    return false;
  }
  governanceDomainId = domainId;
  return true;
}
