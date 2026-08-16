# Native Russian Wiki Style

## Scope
- Apply this contract to all 47 `wiki/**/*.ru.md` concepts, Russian strings in shared `_meta` manifests, and Russian WikiWidget copy.
- Preserve semantic claims, uncertainty, provenance, canonical page IDs, locale mirrors, links, graph topology, and frontend behavior.
- Preserve exact project and code identities when translation would make lookup or protocol meaning less precise: DEOS, TMCTOL, named DEOS subsystems, Actors, Actor roles, code symbols, asset symbols, protocol types, Polkadot SDK, XCM, FRAME, SCALE, XYK, LP, UI Kit, and Domain DAG.

## Prose
- Write each Russian page as independent Russian prose after checking semantic parity with its English mirror.
- Prefer established Russian terms and natural verbal clauses over English noun chains, transliterated jargon, literal syntax, and word-for-word headings.
- Translate ordinary architecture and product vocabulary when precision survives: model of reading, execution environment, governance, provenance, onboarding, downstream/upstream, flow, surface, claim, threshold, and similar context words.
- Keep code and identifiers in backticks when the reader must search for the exact symbol. Do not use backticks merely to hide avoidable English from the audit.

## Deterministic Gate
- `native-russian-style.json` owns contextual guidance for reviewed calques, avoidable borrowings, and English noun chains; guidance is not a context-free replacement dictionary.
- The scanner masks fenced and inline code, Markdown link destinations, URLs, paths, source fragments, exact canonical identifiers, and exact canonical display fields without changing source length or newline positions. It never removes an allowlisted substring from a larger word.
- Pages retain physical line and column locations. Manifest display strings retain JSON Pointer locations. Russian alias keys are classified as search evidence rather than display prose, and WikiWidget contributes only extracted Russian string values rather than TypeScript or Svelte identifiers.
- Overlapping rules produce raw occurrence evidence but one longest-span actionable occurrence. Reports separate raw heuristic occurrences, unique source locations, affected files, evidence classes, and source cohorts.
- Fixtures prove detection, structural exclusions, positions, boundary behavior, overlap handling, false-positive probes, and reviewed false negatives. The audit is a heuristic regression guard, not evidence that prose is fluent, complete, or semantically equivalent.

## Contextual Terminology Guidance
- Translate read-model language contextually as «проекция данных для чтения» or «данные для чтения», materialized views as «материализованное представление», and on-chain provenance as «в блокчейне», «в состоянии блокчейна», or «непосредственно из блокчейна».
- Translate downstream and upstream by actual direction: «производная экосистема», «проект на базе DEOS», «исходный DEOS», or «вышестоящий проект».
- Prefer «пул с учётом долей», «контуры проверки» or «доказательная база», «правила обращения токена», «однонаправленная эмиссия», «ликвидность, принадлежащая казне», «ликвидность во владении протокола», and «правила распределения по корзинам» where those meanings apply.
- Preserve canonical English expansions and project terms in exact title, definition, lookup, or code contexts; ordinary surrounding prose still needs a Russian explanation.

## Independent Review Handoff
A bilingual reviewer receives the final diff and checks every Russian concept plus the localized manifests and WikiWidget strings. The reviewer must report:
- Reviewed page count (`47/47`) and whether every English mirror was consulted.
- Any strengthened, weakened, omitted, or newly ambiguous claim.
- Unnatural syntax, avoidable borrowing, calque, or inconsistent term that escaped the deterministic inventory.
- Canonical identifiers that were translated by mistake and ordinary words retained as English without a precision reason.
- A final `APPROVE` or exact file-and-line blockers.

Do not mark the backlog item complete on heuristic success alone. Close it only after the independent bilingual handoff reports `APPROVE` and all structural, locale, frontend, context, completion, and diff gates pass.
