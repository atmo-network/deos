---
page_type: getting-started
title: Начните здесь
summary: Короткий onboarding-маршрут, который ведет новичков в один из трех путей - понять DEOS, поднять его локально или безопасно форкнуть и изменить экономику.
locale: ru
canonical_page_id: start-here
translation_of: start-here.en.md
translation_status: localized
available_locales:
  - en
  - ru
sources:
  - ../../README.md
  - ../../docs/README.md
  - ../../template/README.md
  - ../../web-client/README.md
  - ../../scripts/README.md
  - ../../simulator/README.md
status: active
audience: newcomer
tags:
  - onboarding
  - getting-started
  - forkability
  - validation
related:
  - DEOS за 60 секунд
  - Форк DEOS
  - Трехуровневая валидация
last_compiled: 2026-07-20
confidence: 0.85
---

# Начните здесь

## Кратко

У DEOS есть глубокие слои runtime, экономики, клиента, документации и агентской поддержки. Вам не нужно изучать их все, чтобы начать.

Выберите путь под свою цель. Каждый путь дает короткий маршрут, понятное условие завершения и минимальную проверку, которая подтверждает, что вы работаете в правильном слое.

Вам не нужно читать `AGENTS.md`, чтобы оценить или форкнуть DEOS. Считайте `AGENTS.md` операционным контекстом для maintainers и агентов. Для человеческого onboarding используйте эту страницу, wiki и README в рабочих областях репозитория.

## Путь A - понять DEOS за 10 минут

Используйте этот путь, если вы читатель из экосистемы, партнер, инвестор или curious builder и хотите решить, стоит ли глубже оценивать DEOS.

Если вы оцениваете партнерство или форк, сначала откройте [Partner Pitch](partner-pitch.ru.md). Он объясняет, почему DEOS важен, до более глубокого архитектурного маршрута.

Прочитайте:

1. [Partner Pitch](partner-pitch.ru.md), если нужен внешний adoption case
2. [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
3. [Чем DEOS не является](../concepts/what-deos-is-not.ru.md)
4. [Physics vs Politics](../comparisons/physics-vs-politics.ru.md)
5. [Стандарт TMCTOL](../concepts/tmctol-standard.ru.md), начиная с краткого описания и базовой механики
6. [Уровни экономических утверждений](../concepts/economic-claim-levels.ru.md), если нужно отличить claims о механизме от рыночных обещаний

Путь завершен, когда вы можете ответить:

- Что DEOS заменяет в ручном DAO-управлении казначейством?
- Почему TMCTOL — это стандарт поверх DEOS, а не весь фреймворк?
- Какие claims являются детерминированным поведением протокола, а какие зависят от рынка и liveness-условий?

Дальше, если интересно, используйте маршрут оценки внутри [Partner Pitch](partner-pitch.ru.md).

## Путь B - поднять DEOS локально за 30 минут

Используйте этот путь, если вы разработчик и проверяете, запускается ли репозиторий на вашей машине.

Предварительные требования:

- Установлен Rust
- Установлен Node.js
- Достаточно диска и времени на сборку Polkadot SDK runtime workspace

Терминал 1 из корня репозитория:

```sh
./scripts/bootstrap-local-network.sh
```

Ожидаемый результат:

- Локальные Polkadot SDK binaries доступны
- Reference runtime собран
- Локальный chain spec создан
- Zombienet запускает локальную сеть на основе Omni Node
- Parachain начинает производить блоки

Терминал 2 из корня репозитория:

```sh
npm --prefix web-client install
npm --prefix web-client run dev
```

Ожидаемый результат:

- Vite печатает локальный URL
- Reference client открывается
- Wiki-поверхность загружается
- Wallet, chain status и ограниченные live surfaces могут подключиться после готовности локальной сети

Опциональное локальное demo-состояние:

```sh
./scripts/seed-web-client-state.sh
```

Если застряли, используйте:

- [Слой скриптов](../usage/scripts-layer.ru.md), чтобы понять ответственность каждого script
- [Трехуровневая валидация](../development/three-layer-validation.ru.md), чтобы выбрать объем проверок и порядок их расширения
- `./scripts/teardown-local-network.sh`, чтобы остановить локальные сервисы
- `./scripts/clean-local-artifacts.sh`, чтобы удалить сгенерированные локальные артефакты

Путь завершен, когда локальная сеть производит блоки, а web client загружается против нее.

## Путь C - безопасно форкнуть и изменить экономику

Используйте этот путь, если вы партнерская команда или protocol builder и хотите превратить DEOS в downstream-экосистему.

Сначала прочитайте:

1. [Форк DEOS](../usage/forking-deos.ru.md)
2. [Формулы TMCTOL](../math/tmctol-formulas.ru.md)
3. [Трехуровневая валидация](../development/three-layer-validation.ru.md)

Затем сделайте первый экономический эксперимент в simulator до изменения runtime-кода:

```sh
node simulator/tests.js
```

Карта безопасного первого изменения:

| Изменение | Где начать | Что потом трогать | Минимальная проверка |
| --- | --- | --- | --- |
| TMC price/slope | Simulator + formulas | Runtime config после математики | Simulator, затем TMC tests |
| TOL split/reserves | TMCTOL spec + simulator | AAA topology, runtime config, docs | Simulator + runtime tests |
| Router fee policy | Axial Router | Router config, governance bounds | Router tests + claims |
| Governance domains/payloads | Governance overview | Gov pallet/config, client | Governance + client checks |
| UI copy/onboarding | `web-client/` + `wiki/` | Runtime только при смене data contract | Client validate + wiki trust |

Не меняйте легкомысленно:

- Launch-time curve physics после создания кривой
- Формулировки floor, compression или guarantees без обновления claim preconditions
- Bucket accounting invariants
- Разделение read-model между ограниченными on-chain projections и materialized/indexed views
- Границы полномочий governance protection track

Путь завершен, когда вы понимаете, какие решения относятся к product narrative, simulator parameters, runtime constants, governance policy, client presentation или externally materialized data.

## Минимальная проверка по пути

Сначала используйте наименьший осмысленный gate. Не каждый change требует все gates.

| Путь или изменение | Минимальная проверка |
| --- | --- |
| Только понимание | Команды не нужны |
| Wiki/onboarding text | `npm --prefix web-client run validate:wiki` |
| Поведение web client | `npm --prefix web-client run validate` |
| Boundaries web client | `npm --prefix web-client run validate:dag` |
| Tokenomics/formulas | `node simulator/tests.js` |
| TMC runtime | `cargo test --manifest-path template/Cargo.toml -p pallet-tmc --locked` |
| Broad runtime | `cargo test --manifest-path template/Cargo.toml --workspace --locked` |
| Междоменный change | Simulator, cargo tests, client validation, completion gate |

## Связанные страницы

- [DEOS за 60 секунд](deos-in-60-seconds.ru.md)
- [Форк DEOS](../usage/forking-deos.ru.md)
- [Трехуровневая валидация](../development/three-layer-validation.ru.md)
