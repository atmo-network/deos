# Спецификация AAA

- **Компонент:** `pallet-aaa` (Account Abstraction Actors)
- **Версия:** `0.1.0`
- **Дата:** Март 2026
- **Статус:** Нормативный

> Ключевые слова **MUST**, **REQUIRED**, **SHALL**, **SHOULD**, **RECOMMENDED**, **MAY** и **OPTIONAL** в этом документе интерпретируются в соответствии с RFC 2119.

---

## 0. Мета-слой ведения спецификации

Спецификация ДОЛЖНА оставаться не длиннее **1080 строк** (без ухудшения форматирования), добавлять новое нормативное содержание только вместе с равным или большим удалением устаревшего, формулировать правила как позитивное исполнимое поведение если негативное ограничение не требуется по соображениям безопасности, держать нормативные факты в одном первичном месте со ссылками вместо дублей, сохранять обязательные пустые строки сверху и снизу у заголовков, и обеспечивать, чтобы каждая строка несла нормативный смысл, трассируемость или обязательный контекст реализации.

---

## 1. Контракт стабильности

1. **Детерминизм:** Идентичные состояние сети и контекст блока ДОЛЖНЫ давать идентичное поведение AAA на всех узлах.
2. **Ограниченная работа:** Каждый путь рантайма (`on_initialize`, `on_idle`, extrinsics) ДОЛЖЕН выполняться за O(1) или O(K) с явными конечными константами `Max*`.
3. **Удаление на месте:** При терминальных условиях состояние актора удаляется атомарно, а балансы остаются на суверенном аккаунте.
4. **Контракт без автовозврата:** Протокол НЕ ДОЛЖЕН выполнять автоматическое разветвлённое распределение возврата активов при закрытии; восстановление баланса выполняет владелец.
5. **Интернализация цены создания:** `create_user_aaa` ДОЛЖЕН взимать невозвратную комиссию открытия в `FeeSink`, чтобы покрывать долгосрочную поддержку заброшенных акторов.
6. **Стейтлесс-планы выполнения:** Шаги независимы и читают состояние в момент исполнения; изменяемое межшаговое состояние запрещено. Разрешен только read-only контекст прогона (например, `reserved_fee_remaining`, `TriggerSnapshot`).
7. **Предсказуемые сбои:** Сбои ДОЛЖНЫ приводить к одному из исходов: `Deferred`, `StepSkipped`, `StepFailed` или `AaaClosed`.
8. **Синхронные мутации:** Мутации slot-allocation ДОЛЖНЫ сохраняться в том же выполнении extrinsic, чтобы исключить гонки внутри блока.
9. **Арифметика с насыщением:** Промежуточная математика fee/limit ДОЛЖНА использовать saturating-семантику. Пользовательское разрешение суммы НЕ ДОЛЖНО молча делать overflow/underflow и ДОЛЖНО разрешаться детерминированно (`Skipped` или явная ошибка).
10. **Корректность контекста исполнения:** Правила ДОЛЖНЫ учитывать семантику FRAME hooks (например, не полагаться на hash текущего блока во время исполнения).
11. **Лимит горизонта defer:** Рантайм ДОЛЖЕН отклонять конфигурации, откладывающие первое допустимое выполнение более чем на десять лет.
12. **Синхронизация spec-impl:** Поведение рантайма ДОЛЖНО соответствовать этому документу в том же релизном окне, и релизный CI ДОЛЖЕН блокировать выпуск, если не пройдены тесты, сопоставленные с инвариантами Section 14.

---

## 2. Модель актора

### 2.1 Экземпляр

- **Терминология:** **План исполнения** — статический ограниченный список шагов, настроенный у актора. **Цикл исполнения** (**Execution Run / Cycle**) — одна допущенная попытка исполнения текущего плана, идентифицируемая `(aaa_id, cycle_nonce)`. Вся внешняя наблюдаемость и корреляция индексаторов ДОЛЖНЫ быть ориентированы на цикл исполнения. Планы исполнения, trigger-фильтры и actor-to-actor потоки активов входят в on-chain поведенческую поверхность AAA, но работают внутри scheduler-, fee-, lifecycle- и safety-контракта этого runtime; в рамках существующих task, adapter и safety-ограничений изменения протокольных workflow СЛЕДУЕТ выражать через перенастройку графа акторов, а не через переписывание runtime.
- **Нативная терминология:** `FeeNativeAsset` обозначает балансную поверхность для `AaaCreationFee`, пошаговых комиссий User, -`MinUserBalance` и резервирования комиссий. `StakeNativeRepresentation` обозначает представление актива, используемое `StakeNative`; оно МОЖЕТ отличаться от `FeeNativeAsset` и ДОЛЖНО рассматриваться рантаймом и UI как отдельное понятие.
- **Стабильная форма плана:** `execution_plan` ДОЛЖЕН быть непустым. `on_close_execution_plan` в текущем стабильном контракте ТОЖЕ ДОЛЖЕН быть непустым; акторы, которым не нужны побочные действия на этапе закрытия, ДОЛЖНЫ использовать явный `Noop`-план закрытия. `Noop` существует для явной наблюдаемости/выравнивания и как канонический план без побочных эффектов, а не как замена отсутствующему плану.

```rust
struct AaaInstance<AccountId, BlockNumber, Balance> {
    aaa_id: u64,
    aaa_type: AaaType,
    sovereign_account: AccountId,
    owner: AccountId,
    owner_slot: u8,
    mutability: Mutability,
    is_paused: bool,
    pause_reason: Option<PauseReason>,
    auto_close_at_cycle_nonce: Option<u64>,
    schedule: Schedule,
    schedule_window: Option<ScheduleWindow>,
    execution_plan: BoundedVec<Step, MaxSteps>,
    on_close_execution_plan: BoundedVec<Step, MaxSteps>,
    cycle_nonce: u64,
    last_cycle_block: BlockNumber,
    consecutive_failures: u32,
    manual_trigger_pending: bool,
    funding_tracked_assets: BoundedBTreeSet<AssetId, MaxFundingTrackedAssets>,
    funding_snapshots: BoundedBTreeMap<AssetId, FundingSnapshot<Balance, BlockNumber>,
  MaxFundingTrackedAssets>,
    cycle_weight_upper: Weight,
    cycle_fee_upper: Balance,
    created_at: BlockNumber,
    updated_at: BlockNumber,
}

struct FundingSnapshot<Balance, BlockNumber> {
    amount: Balance,
    block: BlockNumber,
}
```

### 2.2 Типы и изменяемость

- **User AAA:** Подпадает под комиссии за оценку и исполнение и ограничен `MaxOwnerSlots` через распределение пользовательских слотов.
- **System AAA:** Создается governance, освобожден от модели комиссий User, **ДОЛЖЕН быть Mutable**, НЕ ДОЛЖЕН ограничиваться количеством пользовательских слотов и ДОЛЖЕН хранить/эмитить `owner_slot = 0` как compatibility sentinel.

Правила изменяемости:

- **Mutable:** управляющее происхождение МОЖЕТ вызывать pause/resume/update schedule/update execution plan/update on-close execution plan/set или increment auto-close target.
- **Control origin:** signed owner для обоих типов акторов; governance origin дополнительно валиден только для System AAA.
- **Immutable (только User):** вызовы update и pause/resume ДОЛЖНЫ завершаться `ImmutableAaa`.
- `fund_aaa`, `manual_trigger` и `close_aaa` ДОЛЖНЫ оставаться доступны владельцу для обоих классов изменяемости; governance override над User AAA остается вне рамок стабильного контракта.

### 2.3 Деривация суверенного аккаунта и выделение слотов

1. **User AAA:** `seed = Blake2_256( SCALE(AaaPalletId, owner, owner_slot) )`, `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`.
2. **System AAA:** `seed = Blake2_256( SCALE(AaaPalletId, b"system", aaa_id) )`, `sovereign_account = AccountId::decode(TrailingZeroInput(seed))`. Slotless: НЕ ДОЛЖЕН потреблять биты `OwnerSlotMask`; сохраненный/эмитируемый `owner_slot` ДОЛЖЕН оставаться `0` как compatibility sentinel и ДОЛЖЕН интерпретироваться вместе с `aaa_type`.
3. Бит user slot ДОЛЖЕН очищаться при удалении User AAA.
4. Повторное создание User AAA с тем же `(owner, owner_slot)` или reopen закрытого System AAA с тем же `aaa_id` ДОЛЖНО дать тот же `sovereign_account`.
5. Проверка коллизии ДОЛЖНА защищать только активное AAA-владение тем же суверенным аккаунтом; этот случай ДОЛЖЕН падать с `SovereignAccountCollision`.
6. Предсуществующее состояние выведенного суверенного аккаунта (balances, dust, locks, reserves, third-party transfers) ДОЛЖНО считаться валидным и НЕ ДОЛЖНО считаться коллизией.

- `OwnerSlotMask: Map<AccountId, u8>`
- `MaxOwnerSlots <= 8` (default `8`)
- Биты выше `MaxOwnerSlots` ДОЛЖНЫ быть нулевыми
- `valid_mask(n)` обозначает `u8`-маску с установленными младшими `n` битами
- `create_user_aaa(...)` выбирает минимальный свободный слот; `create_user_aaa_at_slot(owner_slot, ...)` требует точный слот и падает с `InvalidOwnerSlot` или `OwnerSlotOccupied`
  _Примечание для интеграторов: битовая маска кодируется в SCALE little-endian (`(1 << n) - 1`). Перед созданием или пересозданием User AAA клиенту СЛЕДУЕТ заранее вычислять целевой sovereign account и показывать текущие balances/locks/reserves; `create_user_aaa_at_slot` является стабильным recovery-path, когда управление исполнением должно вернуться к исходному sovereign account._

```rust
let mut mask: u8 = OwnerSlotMask::get(&owner);
mask &= valid_mask(MaxOwnerSlots);
let owner_slot = match preferred_slot {
    Some(slot) if slot >= MaxOwnerSlots => return Err(Error::InvalidOwnerSlot),
    Some(slot) if (mask & (1 << slot)) != 0 => return Err(Error::OwnerSlotOccupied),
    Some(slot) => slot,
    None => (0..MaxOwnerSlots).find(|i| (mask & (1 << i)) == 0).ok_or(Error::OwnerSlotCapacityExceeded)?,
};
mask |= 1 << owner_slot;
OwnerSlotMask::insert(&owner, mask & valid_mask(MaxOwnerSlots));
```

Правила идентификаторов System AAA:

1. `create_system_aaa(...)` ДОЛЖЕН выделять новый `aaa_id` из `NextAaaId`; governance НЕ ДОЛЖЕН выбирать явный fresh `aaa_id` через стабильную поверхность.
2. `reopen_system_aaa(aaa_id, ...)` — единственный стабильный explicit-id путь создания System AAA. Он МОЖЕТ переоткрывать только ранее закрытый идентификатор System AAA и ДОЛЖЕН завершаться с `SystemAaaNotClosed` во всех остальных случаях.
3. `reopen_system_aaa` ДОЛЖЕН завершаться с `AaaIdOccupied`, если запрошенный `aaa_id` уже активен.
4. Создание/reopen System AAA ДОЛЖНО атомарно вставлять `SovereignIndex[sovereign_account] = aaa_id` вместе с `AaaInstances`; единственным критерием коллизии остается активная занятость этого sovereign account.
5. `NextAaaId` ДОЛЖЕН оставаться монотонным. Reopen ранее закрытого меньшего id НЕ ДОЛЖЕН его откатывать.

### 2.4 Жизненный цикл

Терминальные условия:

| Условие                                                              | Точка проверки          | Результат                                                                        |
| -------------------------------------------------------------------- | ----------------------- | -------------------------------------------------------------------------------- |
| `fee_native_balance < MinUserBalance`                                | До старта цикла         | `AaaClosed(BalanceExhausted)` (User)                                             |
| `consecutive_failures >= MaxConsecutiveFailures`                     | После сбоя цикла        | `AaaClosed(ConsecutiveFailures)`                                                 |
| `current_block > schedule_window.end`                                | Во всех точках touch    | `AaaClosed(WindowExpired)`                                                       |
| `fee_native_balance < cycle_fee_upper`                               | Допуск планировщиком    | `AaaClosed(FeeBudgetExhausted)` (User)                                           |
| `cycle_nonce == u64::MAX`                                            | После допуска           | User: `AaaClosed(CycleNonceExhausted)`; System: `AaaPaused(CycleNonceExhausted)` |
| `auto_close_at_cycle_nonce = Some(target)` И `cycle_nonce >= target` | После успешного прогона | `AaaClosed(AutoCloseNonceReached)`                                               |

System AAA освобожден от проверок `MinUserBalance` и НЕ ДОЛЖЕН auto-pause-иться на `FundingUnavailable`; неразрешённое funding моделируется как `StepSkipped(FundingUnavailable)`. Конфигурация рантайма ДОЛЖНА обеспечивать `MinUserBalance >= ExistentialDeposit(FeeNativeAsset)`. `close_aaa` от system owner/governance ДОЛЖЕН оставаться безусловно доступным (без mutability/funding/trigger preconditions) для удаления долго-паузящихся акторов.

`WindowExpired` ДОЛЖЕН проверяться во всех lifecycle touch points (scheduler admission, sweep extrinsics, `manual_trigger`, `fund_aaa`, pause/resume, schedule/execution-plan update). Если `current_block > schedule_window.end`, рантайм закрывает актора до других мутаций этого вызова.

Окно расписания включает `end`: `start <= current_block <= end`; закрытие начинается только при `current_block > end`.

Приоритет close reason зависит от checkpoint и ДОЛЖЕН быть детерминированным: `WindowExpired` доминирует во всех external/admission touch points; для unpaused User AAA на admission `BalanceExhausted` доминирует над `FeeBudgetExhausted`; nonce-exhaustion checkpoint выше является единственным after-admission / before-step переходом; после admitted cycle `ConsecutiveFailures` — единственное post-failure terminal close, а `AutoCloseNonceReached` — единственное post-success terminal close.

Перед терминальным удалением состояния рантайм ДОЛЖЕН входить в close-tail execution для `on_close_execution_plan` как на явных, так и на автоматических путях закрытия. Close tail использует ту же семантику task, condition, amount-resolution, error-policy, adapter и weight-upper-bound, что и основной execution plan. Для User AAA рантайм ДОЛЖЕН выводить детерминированные `close_cycle_weight_upper` и `close_cycle_fee_upper`, инициализировать транзиентное close-tail fee reservation как `min(fee_native_balance_at_close_entry, close_cycle_fee_upper)` и строить новый `TriggerSnapshot` по балансам на входе в close. Для System AAA действует та же семантика исполнения, но без списания User fees. Планировщик на автоматических close-путях ДОЛЖЕН резервировать достаточно dispatch budget для допуска этого хвоста либо откладывать закрытие до появления такого бюджета. Close-tail execution НЕ ДОЛЖЕН рекурсивно входить в еще один close.
Если во время admitted close execution заканчивается fee-native balance или ломается fee routing, затронутые close-шаги ДОЛЖНЫ наблюдаемо падать/пропускаться, но финальное закрытие все равно ДОЛЖНО завершаться. Whole-tail skip не является частью активного контракта текущей линии.

При терминальном условии или owner-вызове `close_aaa` рантайм атомарно удаляет состояние/индексы актора, очищает User slot bit (у System slot bits нет), сохраняет суверенные балансы и эмитит `AaaClosed`.

Переходы create/close ДОЛЖНЫ атомарно синхронизировать `AaaInstances` и `SovereignIndex`; состояние очередей (`CurrentQueue`, `NextQueue`, `ActorQueueEpoch`) ДОЛЖНО оставаться детерминированным при тех же переходах.

Recovery выполняется владельцем через `create_user_aaa_at_slot` для User AAA и governance через `reopen_system_aaa(aaa_id, ...)` для ранее закрытого System AAA; прямые post-destruction balance-rescue extrinsics не входят в стабильный контракт.

### 2.5 Снимки финансирования

Карта снимков по активам `funding_snapshots` — канонический baseline для разрешения `PercentageOfLastFunding` (Section 5.3). Рантайм поддерживает `funding_tracked_assets` для оптимизации исполнения.

Требуемое поведение:

1. **Execution-plan scanning:** При создании, `update_execution_plan(aaa_id, execution_plan)` или `update_on_close_execution_plan(aaa_id, on_close_execution_plan)` рантайм ДОЛЖЕН сканировать ОБА execution plan (`execution_plan` + `on_close_execution_plan`) и заполнять `funding_tracked_assets` всеми `AssetId` из шагов с `PercentageOfLastFunding`. Любое обновление execution plan ДОЛЖНО полностью пересчитывать tracked set и удалять устаревшие записи `funding_snapshots`.
2. **Snapshot Update via `fund_aaa`:** Каждый успешный `fund_aaa(_, aaa_id, asset, amount)` с `amount > 0` ДОЛЖЕН обновить `funding_snapshots[asset]` новым amount и `current_block` для User и System AAA, но ТОЛЬКО если `asset` есть в `funding_tracked_assets`. Неотслеживаемые активы ДОЛЖНЫ падать с `SnapshotUnavailable`.
3. **Snapshot Update via `notify_address_event`:** Каждый `notify_address_event(aaa_id, asset, amount, source)` с `amount > 0` ДОЛЖЕН обновлять `funding_snapshots[asset]`, но ТОЛЬКО если актор — System AAA и `asset` отслеживается.
4. Обновление snapshot НЕ ДОЛЖНО зависеть от личности вызывающего (owner/governance/third-party одинаково валидны).
5. Funding events, обновляющие tracked snapshots, ДОЛЖНЫ оставаться валидными независимо от pause state; они НЕ ДОЛЖНЫ автоматически вызывать pause/resume transitions.
6. `FundingUnavailable` — детерминированный non-terminal outcome для User и System AAA; он покрывает missing/zero tracked snapshots и tracked-balance overspend, тогда как untracked assets остаются `SnapshotUnavailable`, а stale tracked snapshots валидны до перезаписи.
7. `cycle_weight_upper` и `cycle_fee_upper` — кэш-поля run-плана, которые ДОЛЖНЫ пересчитываться при create/update execution plan и ДОЛЖНЫ влиять только на эффективность admission/preflight, а не на функциональную семантику исполнения. Upper bounds close tail (`close_cycle_weight_upper`, `close_cycle_fee_upper`) ТОЖЕ ДОЛЖНЫ детерминированно выводиться из `on_close_execution_plan` при create/update — независимо от того, кэшируются они или пересчитываются — и НЕ ДОЛЖНЫ менять функциональную семантику task.

### 2.6 Отслеживание сбоев

1. `consecutive_failures` увеличивается только на cycle abort (`AbortCycle`); если `MaxConsecutiveFailures > 0`, терминальный cutoff включительный (`>=`).
2. `consecutive_failures` сбрасывается при успешном завершении цикла.
3. Deferrals НЕ ДОЛЖНЫ увеличивать `consecutive_failures`.
4. `update_execution_plan` (Mutable) ДОЛЖЕН сбрасывать `consecutive_failures`.
5. `cycle_nonce` увеличивается ровно один раз на admitted cycle start.
6. Deferred cycles НЕ ДОЛЖНЫ увеличивать nonce.
7. `last_cycle_block` ДОЛЖЕН обновляться до `current_block` строго на admitted cycle start (`CycleStarted`), а не на completion и не на deferral.
8. При исчерпании nonce: User AAA ДОЛЖЕН закрываться с `CycleNonceExhausted`; System AAA ДОЛЖЕН ставиться на паузу с `PauseReason::CycleNonceExhausted`.

---

## 3. Адаптеры

Все операции ДОЛЖНЫ проходить через типизированные адаптеры.

### 3.1 AssetOps

```rust
trait AssetOps<AccountId, AssetId, Balance> {
    fn transfer(from: &AccountId, to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn balance(who: &AccountId, asset: AssetId) -> Balance;
    fn minimum_balance(asset: AssetId) -> Balance;
    fn can_deposit(who: &AccountId, asset: AssetId, amount: Balance) -> bool;
    fn total_issuance(asset: AssetId) -> Balance;
}
```

**Семантика баланса:** `balance()` ДОЛЖЕН возвращать adapter-visible immediately transferable баланс актива до применения AAA-local reservation. Для `FeeNativeAsset` это определяется runtime policy (обычно `free_balance` после adapter-level locks/reserves/holds); для активов без hold-семантики это может совпадать с total balance. AAA затем выводит `spendable_fee_native`, вычитая transient `reserved_fee_remaining` только из `balance()` для `FeeNativeAsset`; балансы не-`FeeNativeAsset` проходят без дополнительных AAA-local вычетов.

`Mint` ДОЛЖЕН отклоняться для User AAA. Валидация ДОЛЖНА происходить на этапе **admission плана** (create/update_execution_plan): если execution plan User AAA содержит `Mint`, вызов ДОЛЖЕН упасть с `MintNotAllowedForUserAaa`. Это предотвращает потерю комиссий и запутанный UX от отклонения на этапе исполнения.
`can_deposit`/`minimum_balance` ТРЕБУЮТСЯ для ED-safe нормализации split-transfer (Section 6.2).

### 3.2 DexOps

```rust
trait DexOps<AccountId, AssetId, Balance> {
    fn swap_exact_in(
        who: &AccountId,
        asset_in: AssetId,
        asset_out: AssetId,
        amount_in: Balance,
        slippage_tolerance: Perbill,
    ) -> Result<Balance, DispatchError>;
    fn swap_exact_out(
        who: &AccountId,
        asset_in: AssetId,
        asset_out: AssetId,
        amount_out: Balance,
        slippage_tolerance: Perbill,
    ) -> Result<Balance, DispatchError>;
    fn add_liquidity(who: &AccountId, asset_a: AssetId, asset_b: AssetId, amount_a: Balance, amount_b: Balance)
        -> Result<(Balance, Balance, Balance), DispatchError>;
    fn remove_liquidity(who: &AccountId, lp_asset: AssetId, lp_amount: Balance)
        -> Result<(Balance, Balance), DispatchError>;
}
```

Контракт адаптера:

1. Сложность ДОЛЖНА быть O(1) или bounded O(K) с явными `MaxK`.
2. Итерация по storage (если есть) ДОЛЖНА использовать canonical storage-key порядок.
3. Округление ДОЛЖНО быть фиксированным и детерминированным для каждого метода.
4. `SwapExactIn` получает `slippage_tolerance: Perbill` — адаптер сам считает `min_out` (например `min_out = (1 - tolerance) × quote`). Паллет не трогает pricing logic
5. `SwapExactOut` получает `slippage_tolerance: Perbill` и ДОЛЖЕН детерминированно вычислять required input до исполнения свопа
6. Slippage/routing logic остается внутри DEX adapter; AAA обрабатывает только `DispatchError` через `on_error`.

### 3.3 StakingOps

```rust
trait StakingOps<AccountId, AssetId, Balance> {
    fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
    fn stake_native(
        who: &AccountId,
        amount: Balance,
        operator: &AccountId,
    ) -> Result<(), DispatchError>;
    fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), DispatchError>;
}
```

Текущее направление TMCTOL больше не оставляет нативный AAA staking как generic rejection-only surface.
Явный нативный путь теперь — `StakeNative { amount, operator }`, сохраняя generic `Stake { asset, amount }` для non-native staking и делая требуемый collator/operator context первоклассным для входа в `$NTVE`.
`StakeNative` amount resolution конфигурируется рантаймом относительно `StakeNativeRepresentation`, которое МОЖЕТ отличаться от `FeeNativeAsset` и НЕ ДОЛЖНО выводиться из семантики fee reservation.

### 3.4 Контракт веса задач

Рантайм ДОЛЖЕН предоставлять детерминированные upper bounds худшего случая:

`fn weight_upper_bound(task: Task, params: TaskParams) -> Weight`

Требования:

- State-independent для фиксированных параметров.
- Ограничено настроенными `Max*`.
- Всегда `>=` фактическому исполнению.
- Admission полного цикла использует сумму upper bounds шагов.
- Task-level `weight_upper_bound` ДОЛЖЕН включать worst-case стоимость эмиссии событий, которые генерирует успешное выполнение задачи.
- Admission-учет рантайма ДОЛЖЕН включать детерминированный step/cycle overhead для не-task событий (`CycleStarted`, `StepSkipped`, `StepFailed`, `CycleSummary` и lifecycle events при terminal transitions).

Рантайму СЛЕДУЕТ классифицировать задачи в coarse weight buckets для снижения fragility поддержки:

| Bucket          | Tasks                                     |
| --------------- | ----------------------------------------- |
| `SimpleAssetOp` | `Transfer`, `Burn`, `Mint`                |
| `DexSwap`       | `SwapExactIn` / `SwapExactOut`            |
| `DexLiquidity`  | `AddLiquidity`, `RemoveLiquidity`         |
| `Fanout`        | `SplitTransfer` (parameterized by `legs`) |
| `Noop`          | `Noop`                                    |

---

## 4. Экономика

System AAA освобожден в этом разделе от списания User fees. Все собранные User fees ДОЛЖНЫ маршрутизироваться в `FeeSink`. В совместимых рантаймах routing в `FeeSink` ДОЛЖЕН быть тотальным и deposit-capable для переводов `FeeNativeAsset`. `create_user_aaa` ДОЛЖЕН завершаться ошибкой, если routing в fee sink не удался. Если некорректно сконфигурированный рантайм все же допускает ошибку перевода в fee sink во время cycle или close-tail charging, cycle-path failures ДОЛЖНЫ детерминированно отображаться в `StepFailed` и подчиняться выбранному `StepErrorPolicy`, тогда как close-tail charging failures ДОЛЖНЫ оставаться наблюдаемыми через `OnCloseStepFailed` и НЕ ДОЛЖНЫ блокировать поздние close-шаги или финальное закрытие; этот путь — обработка misconfiguration, а не intended steady state.

### 4.1 Модель комиссий

Исполнение ДОЛЖНО следовать этому порядку:

1. **MinUserBalance Gate**
2. **Pre-flight Fee Admission** (`cycle_fee_upper`)
3. **Cycle Start / Fee Reservation**
4. Для каждого шага: charge evaluation fee → evaluate conditions → resolve task amount → если executable, charge execution fee → dispatch task.

Для User AAA недостаток pre-flight fee budget ведёт к немедленному `AaaClosed(FeeBudgetExhausted)`.

Формулы для одного шага:

- `eval_fee = StepBaseFee + ConditionReadFee × conditions.len()`
- `exec_fee_upper = WeightToFee(weight_upper_bound(task, params))`
- `cycle_fee_upper = Σ(eval_fee_i + exec_fee_upper_i)`
  Execution fee взимается, как только шаг становится executable, даже если dispatch потом падает; шаги, разрешенные как `Skipped` или `FundingUnavailable`, execution fee не платят.

`StepBaseFee` и `ConditionReadFee` взимаются до dispatch задачи и ДОЛЖНЫ быть откалиброваны так, чтобы экономически покрывать non-executable пути (`StepSkipped`, `StepFailed`), которые все равно потребляют reads/writes и эмитят события.

Close-tail admission использует те же формулы поверх `on_close_execution_plan`. Рантайм ДОЛЖЕН выводить детерминированные `close_cycle_weight_upper` и `close_cycle_fee_upper` из close plan, используя те же upper bounds задач и тот же non-task observability overhead, который относится к close-time execution.

### 4.2 Политика без ренты

AAA не использует регулярный rent-механизм. Долгие defer-сценарии остаются валидными в пределах `MaxExecutionDelayBlocks`, если в момент исполнения проходят lifecycle и fee-admission проверки.

### 4.3 Резервирование комиссии

Во время исполнения цикла или допущенного close tail рантайм ДОЛЖЕН держать `reserved_fee_remaining` и считать capacity траты fee-native как:

`spendable_fee_native = max(fee_native_balance - reserved_fee_remaining, 0)`

`reserved_fee_remaining` — transient переменная execution context. Она НЕ ДОЛЖНА сохраняться в `AaaInstance` или storage.

Правила резервирования:

1. На admitted cycle start инициализировать `reserved_fee_remaining = cycle_fee_upper`; на admitted User close-tail start — `reserved_fee_remaining = min(fee_native_balance_at_close_entry, close_cycle_fee_upper)`.
2. Каждое успешное списание evaluation/execution fee ДОЛЖНО уменьшать `reserved_fee_remaining` на списанную сумму.
3. Все пути расходования `FeeNativeAsset` ДОЛЖНЫ разрешать суммы из `spendable_fee_native`, а не из одного только `balance()`.
4. На выходе из цикла или close tail неиспользованный резерв освобождается выбрасыванием transient context; списанные комиссии НЕ возвращаются.
5. Post-dispatch refund по фактически израсходованному весу задач сознательно вне scope: AAA списывает детерминированный upper-bound execution fee для каждого executable step ради предсказуемой admission economics.

### 4.4 Комиссия открытия

`create_user_aaa` ДОЛЖЕН взимать `AaaCreationFee` в `FeeNativeAsset` и отправлять его в `FeeSink`; комиссия открытия невозвратная (никогда не возвращается при `close_aaa`), а создание ДОЛЖНО падать, если transfer в `FeeSink` неуспешен или создатель не покрывает `AaaCreationFee` вместе с обычными tx fees (`InsufficientFee`); `create_system_aaa` освобожден от этого требования.

### 4.5 Допуск хвоста закрытия и прогнозирование

`on_close_execution_plan` — терминальный хвост того же детерминированного конвейера исполнения, а не исключение для бесплатной очистки.

1. Рантайм ДОЛЖЕН выводить `close_cycle_weight_upper` и `close_cycle_fee_upper` из `on_close_execution_plan`, используя те же upper bounds задач, fee formulas и close-time observability overhead, что и при обычном cycle admission.
2. Рантайм ДОЛЖЕН одинаково трактовать entry в close tail на явных и автоматических close-путях: строить новый `TriggerSnapshot`, инициализировать User close reservation как `min(fee_native_balance_at_close_entry, close_cycle_fee_upper)` и повторно использовать нулевое резервирование User fees для System AAA.
3. Планировщик на автоматических close-путях ДОЛЖЕН резервировать dispatch budget достаточно рано, чтобы admitted close tail помещался в bounded `on_idle`; если хвост пока не помещается, рантайм ДОЛЖЕН откладывать закрытие, а не возвращать текущую линию к whole-tail skip semantics.
4. Если во время admitted close execution fee-native balance становится недостаточным, затронутые close-шаги ДОЛЖНЫ наблюдаемо падать/пропускаться, а рантайм ДОЛЖЕН продолжать детерминированное закрытие без retry loops и без terminal-pending state.
5. Финализация close ДОЛЖНА происходить ровно один раз после завершения допущенного close tail.
6. Акторам и tooling СЛЕДУЕТ прогнозировать жизнеспособность close через `predicted_fee_native_residual_after_close = fee_native_balance_before_close - close_cycle_fee_upper - close_task_fee_native_spend_upper`; значения `<= 0` указывают на риск attrition close tail, но НЕ ДОЛЖНЫ блокировать явный owner/governance close.

---

## 5. Выполнение

### 5.1 Шаг

```rust
struct Step {
    conditions: BoundedVec<Condition, MaxConditionsPerStep>,
    task: Task,
    on_error: StepErrorPolicy,
}
```

### 5.2 Условия

Ссылка на тип `Condition` нормативно закреплена в Section 12.1.

- Conditions компонуются через AND.
- Пустой список conditions = unconditional step.
- Ошибки evaluation fail-closed (`StepFailed`).
- Balance conditions оцениваются по **spendable balance** (adapter-visible balance минус reserved fee budget `FeeNativeAsset`, где это применимо), а не по одному только `balance()`. Это дает единый взгляд conditions и amount resolution на доступные средства.

### 5.3 Разрешение суммы

Ссылка на тип `AmountResolution` нормативно закреплена в Section 12.1.

Семантика:

1. `PercentageOfCurrent` использует баланс на момент исполнения шага.
2. `PercentageOfTrigger` использует снимок на старте цикла (Section 5.4).
3. `PercentageOfLastFunding` использует сумму из `funding_snapshots` для расходуемого актива (Section 2.5).
4. `AllBalance` в политике `PreserveSpend` разрешается как `spendable_current - minimum_balance(asset)`. В `ExpendableSpend` и `Mint` `AllBalance` разрешается как полный `spendable_current`.
5. Outcomes разрешения детерминированы: `Resolved(amount)`, `Skipped` (например tiny percentage округлился в ноль), или `FundingUnavailable`.

Политики разрешения — рантайм ДОЛЖЕН применять одну policy на задачу:

| Policy            | Used by                                                                       | `AllBalance`                       | Source sufficiency                 |
| ----------------- | ----------------------------------------------------------------------------- | ---------------------------------- | ---------------------------------- |
| `PreserveSpend`   | `Transfer`, `SplitTransfer`, `SwapExactIn`, `AddLiquidity`, `RemoveLiquidity` | Subtracts `minimum_balance(asset)` | Required against spendable balance |
| `Mint`            | `Mint`, `SwapExactOut`                                                        | No ED subtraction                  | No spendability requirement        |
| `ExpendableSpend` | `Burn`                                                                        | Full spendable balance             | MAY delegate to adapter            |

Для `SwapExactOut` semantics policy `Mint` применяются только к разрешению target output amount. DEX adapter все равно ДОЛЖЕН проверять достаточность фактического input и атомарно падать, если требуемый input не может быть оплачен с баланса актора.

Соответствие разрешения и поведения рантайма:

| Resolution           | Runtime behavior                                                                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------- |
| `Resolved(amount)`   | Task executes normally                                                                                  |
| `Skipped`            | Emit `StepSkipped { reason: ResolutionSkipped }` — non-failing, не увеличивает `consecutive_failures`   |
| `FundingUnavailable` | Emit `StepSkipped { reason: FundingUnavailable }`; non-terminal для обоих классов акторов (Section 2.4) |
| Conditions not met   | Emit `StepSkipped { reason: ConditionsNotMet }`                                                         |

### 5.4 Снимок триггера

`PercentageOfTrigger` разрешается по замороженному снимку баланса, взятому один раз на старте цикла или один раз при входе в допущенный close tail для `on_close_execution_plan`. Это устраняет эффект compound-percentage в многошаговых execution plans.

Пример (суверенный аккаунт держит 1000 Native, execution plan из 3 шагов):

| Step | Task                                | Resolution                | Effect        |
| ---- | ----------------------------------- | ------------------------- | ------------- |
| 0    | Transfer `PercentageOfTrigger(10%)` | `floor(1000 × 10%) = 100` | balance → 900 |
| 1    | Transfer `PercentageOfTrigger(10%)` | `floor(1000 × 10%) = 100` | balance → 800 |
| 2    | Swap `PercentageOfTrigger(50%)`     | `floor(1000 × 50%) = 500` | balance → 300 |

Правила построения:

1. На admitted cycle start после fee reservation либо на admitted close-tail entry после close-tail fee reservation рантайм ДОЛЖЕН собрать transient `TriggerSnapshot: Map<AssetId, Balance>`.
2. Просканировать шаги execution plan и собрать уникальный набор `AssetId`, используемых в `PercentageOfTrigger`.
3. Для каждого актива: `FeeNativeAsset` → `snapshot[FeeNativeAsset] = spendable_fee_native` (по Section 4.3); остальные → `snapshot[asset] = AssetOps::balance(sovereign, asset)`.
4. `TriggerSnapshot` — transient execution context рядом с `reserved_fee_remaining`; НЕ ДОЛЖЕН персиститься; освобождается на выходе из цикла или close tail.
5. Если шаг ссылается на `PercentageOfTrigger` для актива, отсутствующего в snapshot, разрешение ДОЛЖНО вернуть `Skipped`.

Execution-plan scan (шаг 2) ограничен `MaxSteps` и не делает storage I/O.

### 5.5 Политики ошибок и атомарность

```rust
enum StepErrorPolicy {
    AbortCycle,
    ContinueNextStep,
}
```

- `AbortCycle`: немедленно остановить; увеличить `consecutive_failures`.
- `ContinueNextStep`: пропустить неудачный шаг и продолжить.
- `PauseActor` НЕ ДОЛЖЕН быть частью стабильного `StepErrorPolicy`.

Атомарность:

- **Task-level:** атомарно.
- **Execution-plan-level:** неатомарно (успешные предыдущие шаги сохраняются).

Правила task-атомарности:

1. Single-op задачи обеспечивают атомарность через один adapter call.
2. Multi-op задачи (например `SplitTransfer`) ДОЛЖНЫ выполняться в transactional boundary (runtime storage transactions или adapter-level commit/rollback).
3. Если любая sub-operation падает, задача ДОЛЖНА откатить все предыдущие sub-operations. Частичные эффекты НЕ ДОЛЖНЫ сохраняться.

Если ранний шаг меняет composition активов, а поздний падает, post-mutation балансы остаются на суверенном аккаунте. При `ContinueNextStep` после mutating-задач (`SwapExactIn`, liquidity ops) авторам execution plan и UI СЛЕДУЕТ защищать downstream-шаги явными balance-conditions. Симуляция execution plans — off-chain only (RPC dry-run, fork replay).

---

## 6. Задачи

### 6.1 Набор задач и параметры

| Задача            | Описание                                                                 |
| ----------------- | ------------------------------------------------------------------------ |
| `Transfer`        | Перевод одного актива                                                    |
| `SplitTransfer`   | Атомарное ограниченное разветвление перевода (Section 6.2)               |
| `Burn`            | Сжигание актива                                                          |
| `Mint`            | Выпуск актива (только System AAA)                                        |
| `SwapExactIn`     | DEX exact-in со slippage tolerance в `Perbill`                           |
| `SwapExactOut`    | DEX exact-out по целевому выходу с детерминированным разрешением входа   |
| `AddLiquidity`    | Добавление ликвидности                                                   |
| `RemoveLiquidity` | Изъятие ликвидности                                                      |
| `Stake`           | Депозит ненативного актива в staking pool                                |
| `StakeNative`     | Депозит `StakeNativeRepresentation` в staking pool с указанием оператора |
| `Unstake`         | Вывод долей из staking pool                                              |
| `Noop`            | Шаг наблюдаемости/выравнивания                                           |

Контракт параметров `SwapExactIn`:

```rust
SwapExactIn {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
}
```

`slippage_tolerance` передается напрямую в `DexOps`; адаптер сам считает `min_out = (1 - slippage_tolerance) × quote(asset_in, asset_out, amount_in)`. `Perbill::zero()` требует exact quote, `Perbill::one()` принимает любой output. Если quote недоступен (нет пула / нулевая ликвидность), swap падает с `DispatchError`, обрабатываемым через `on_error`.

Контракт параметров `SwapExactOut`:

```rust
SwapExactOut {
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: AmountResolution<Balance>,
    slippage_tolerance: Perbill,
}
```

Для `SwapExactOut` адаптер ДОЛЖЕН детерминированно вычислять требуемый input и ограничивать его через `max_in = (1 + slippage_tolerance) × quote_required_in(asset_in, asset_out, amount_out)`.

### 6.2 SplitTransfer

```rust
struct SplitTransfer {
    asset: AssetId,
    amount: AmountResolution<Balance>,
    legs: BoundedVec<SplitLeg, MaxSplitTransferLegs>,
}
```

Ссылка на тип `SplitLeg` нормативно закреплена в Section 12.1.

Проверка:

1. `2 <= legs.len() <= MaxSplitTransferLegs`
2. Нет zero-share legs (`share > 0`)
3. Нет duplicate recipients
4. `sum(share_i) <= Perbill::one()`; при превышении ДОЛЖНО падать с `InvalidSplitTransfer`

Распределение:

- `leg_i = floor(total × share_i)`
- `distributed = Σ(leg_i)`
- `retained = total - distributed`

Семантика остатка:

- `sum(share_i)` МОЖЕТ быть `< 100%`.
- Любая нераспределенная часть и rounding dust ДОЛЖНЫ оставаться на суверенном балансе AAA.
- Рантайм НЕ ДОЛЖЕН auto-route retained remainder никакому получателю.

Безопасность относительно ED:

1. Перед dispatch transfers рантайм ДОЛЖЕН выполнить детерминированную нормализацию legs.
2. Для каждой leg с `leg_i > 0`, если `AssetOps::can_deposit(to_i, asset, leg_i) == false`, эта leg ДОЛЖНА пропускаться, а сумма добавляться к `retained`.
3. Финальная `retained` сумма ДОЛЖНА оставаться на суверенном балансе AAA.

Весь fan-out остается task-atomic для финального нормализованного набора переводов.

### 6.3 Контракт задачи

Каждая задача ДОЛЖНА определять: validation rules, deterministic error surface, deterministic `weight_upper_bound` и explicit adapter side effects. Задачи НЕ ДОЛЖНЫ dispatch-ить произвольные extrinsics.

---

## 7. Триггеры

### 7.1 Таймер и энтропия

```rust
enum Trigger<AccountId, AssetId> {
    Timer { every_blocks: u32, probability: Option<Perbill> },
    OnAddressEvent {
        source_filter: SourceFilter<AccountId>,
        asset_filter: AssetFilter<AssetId>,
    },
    Manual,
}
```

Ссылка на тип `Schedule` нормативно закреплена в Section 12.1.

Правила cooldown расписания:

1. `cooldown_blocks` ДОЛЖЕН применяться ко всем trigger-классам (`Timer`, `OnAddressEvent`, `Manual`) после первого допущенного цикла.
2. Первый допущенный цикл (`cycle_nonce == 0`) НЕ ДОЛЖЕН блокироваться cooldown.
3. Readiness ДОЛЖЕН проваливаться, когда `current_block - last_cycle_block < cooldown_blocks`.
4. `last_cycle_block` ДОЛЖЕН обновляться на admitted cycle start (`CycleStarted`), поэтому cooldown привязан к admission, а не к завершению.
5. Для `Timer` эффективный межцикловый интервал равен `max(every_blocks, cooldown_blocks)`.
6. `manual_trigger` МОЖЕТ установить `manual_trigger_pending`, но cooldown всё равно ДОЛЖЕН ограничивать admission.

Правила timer:

1. `every_blocks` ДОЛЖЕН удовлетворять `0 < every_blocks <= MaxExecutionDelayBlocks`; иначе ошибка `ExecutionDelayTooLong`
2. `probability: None` означает deterministic cadence; `Some(p)` включает probabilistic gate
3. Каденс `every_blocks <= 1` ДОЛЖЕН использовать queue self-continuation (`NextQueue`) и НЕ ДОЛЖЕН использовать `WakeupIndex`
4. Каденс `every_blocks > 1` ДОЛЖЕН планироваться через детерминированный time-ordered wakeup index (`WakeupIndex`)
5. Для delayed timer (`every_blocks > 1`) СЛЕДУЕТ применять deterministic anti-storm jitter:
   `jitter_window = min(every_blocks / 4, MaxTimerJitterBlocks)`
   `jitter = Blake2_256(aaa_id) % jitter_window`, когда `jitter_window > 0`, иначе `0`; horizon validation применяется к `every_blocks`, а не к `every_blocks + jitter`
   `target_block = current_block + every_blocks + jitter`
6. Если `RequireSecureEntropyForProbabilisticTasks=true`, расписания с `0 < p < 1` и любой задачей, кроме `Noop`, ДОЛЖНЫ требовать `EntropyProvider::is_secure_for_financial_probability()` на этапе admission, иначе `InsecureEntropyProvider`
7. Эти же strict financial schedule ДОЛЖНЫ использовать `EntropyProvider::secure_entropy_for_financial_probability(subject)` на этапе execution и НЕ ДОЛЖНЫ fallback-иться к block hash; если secure entropy недоступна, probabilistic gate считается readiness miss и цикл не исполняется
8. Для non-strict probability sampling entropy fallback chain детерминирован: external provider `(aaa_id, current_block)` → `parent_hash` → `block_hash(current_block - 1)`; `block_hash(current_block)` запрещен в runtime dispatch
9. Probability sampling происходит только после прохождения cadence/cooldown readiness; miss — это readiness miss, а не defer/error/event, и он не меняет nonce/manual/failure state
10. Финальная sampled entropy ДОЛЖНА быть domain-separated по resolved entropy hash, `aaa_id` и `cycle_nonce`; off-chain nondeterministic entropy запрещена

### 7.2 OnAddressEvent

```rust
struct InboxState<BlockNumber> {
    is_pending: bool,
    generation: u64,
    last_event_block: BlockNumber,
}

enum SourceFilter<AccountId> {
    Any,
    OwnerOnly,
    Whitelist(BoundedVec<AccountId, MaxWhitelistSize>),
}

enum AssetFilter<AssetId> {
    Any,
    Whitelist(BoundedVec<AssetId, MaxWhitelistSize>),
}
```

Модель inbox:

1. `AddressEventInbox` ведется per-AAA как pending-latch (не очередь); множественные совпавшие события coalesce-ятся в один pending-сигнал.
2. Совпавшее inbound balance-increase event семантически является сообщением-триггером: оно устанавливает `is_pending = true`, увеличивает `generation` и обновляет `last_event_block`.
3. Coalescing работает на уровне сигнала: один admitted cycle может экономически обработать баланс, накопленный из нескольких совпавших событий с момента предыдущего consume inbox.

Правила:

- `SourceFilter::Whitelist` и `AssetFilter::Whitelist` ДОЛЖНЫ быть непустыми и ограниченными `MaxWhitelistSize`.
- События без concrete source account identifier ДОЛЖНЫ матчиться только `SourceFilter::Any`.
- Scheduler readiness для этого триггера ДОЛЖЕН быть `true` тогда и только тогда, когда `is_pending == true`.
- Когда цикл стартует для актора с `OnAddressEvent`, inbox entry ДОЛЖЕН потребляться атомарно.
- Если новое совпавшее событие приходит после consume, актор ДОЛЖЕН снова становиться ready на последующих проходах scheduler.

Контракт ingress:

1. Runtime-ingress в `OnAddressEvent` ДОЛЖЕН проходить через runtime-configured adapter interface (`AddressEventIngress` или эквивалент), который в итоге вызывает `notify_address_event*`.
2. Ingress strategy ДОЛЖНА быть submit-first там, где есть явные hook points: producer-пути (AAA asset ops, TMC/router routing paths, XCM transactor paths) СЛЕДУЕТ отправлять напрямую через адаптер при успешном transfer/mint.
3. Scanner-ingress МОЖЕТ использоваться как fallback для non-hookable producer-путей (например, generic `pallet-assets` direct transfer/mint extrinsics).
4. Producer и scanner пути НЕ ДОЛЖНЫ напрямую мутировать `AddressEventInbox` или `funding_snapshots`.
5. Source и asset filters ДОЛЖНЫ проверяться в том же state transition, что и inbox update.
6. Source invariant: когда concrete sender доступен, ingress ДОЛЖЕН сохранять его без потерь; `source = None` допустим только для intrinsically source-less путей.
7. Dedup invariant: runtime ДОЛЖЕН применять детерминированный same-block dedup между submit и scanner путями до эффективных inbox/snapshot side effects.
8. Поведение funding snapshot для `notify_address_event` нормативно закреплено в Section 2.5 и ДОЛЖНО оставаться независимым от trigger-filter matching.
9. Boundedness invariant: ingress-реализация ДОЛЖНА иметь явные scan/admission caps и МОЖЕТ использовать bounded carry-over queue для over-cap распознанных событий с приоритетным drain перед новым scan.
10. Вес inbox updates оплачивается либо originating transfer/mint path (submit-first), либо bounded scanner/queue processing в `on_idle` (fallback).

### 7.3 Ручной триггер

`manual_trigger` обходит только timing расписания. Он НЕ ДОЛЖЕН обходить admission или fee checks.

1. Вызов `manual_trigger` на непаузнутом акторе ДОЛЖЕН установить `manual_trigger_pending = true`; вызов на паузе ДОЛЖЕН падать с `AaaPaused`.
2. `manual_trigger_pending` ДОЛЖЕН очищаться строго когда цикл допущен и emit-ится `CycleStarted`.
3. Deferrals НЕ ДОЛЖНЫ очищать `manual_trigger_pending`.
4. Если актор закрывается до admission, флаг удаляется вместе с actor state.
5. Если актор был поставлен на паузу уже после установки флага, `manual_trigger_pending` ДОЛЖЕН сохраняться через `pause_aaa` / `resume_aaa`.

### 7.4 Окно расписания

```rust
struct ScheduleWindow<BlockNumber> {
    start: BlockNumber,
    end: BlockNumber,
}
```

Проверка:

1. `end > start`
2. `end - start >= MinWindowLength`
3. `saturating_sub(start, current_block) <= MaxExecutionDelayBlocks`; иначе fail с `ExecutionDelayTooLong`

Семантика:

- `current_block < start`: not ready.
- `start <= current_block <= end`: eligible.
- `current_block > end`: lifecycle closure выполняется по Section 2.4 (`WindowExpired` на lifecycle touch points).

Контракт отложенного горизонта:

- `MaxExecutionDelayBlocks` ДОЛЖЕН точно соответствовать десяти годам в блоках для target block time рантайма.
- При creation и `update_schedule` рантайм ДОЛЖЕН гарантировать, что first eligible execution не отложен дальше `current_block + MaxExecutionDelayBlocks`.
- Для `ScheduleWindow` bound обеспечивается через `start`.
- Для `Timer` bound обеспечивается через `every_blocks` (Section 7.1).

---

## 8. Планировщик

AAA-рантайм рассматривается как **детерминированный event-driven actor runtime**. Акторы не поллятся глобально; они пробуждаются явными событиями (Timer, AddressEvent, Manual), поступление активов может работать как сообщение-триггер, а более крупные протокольные workflow могут возникать из композиции графов акторов, но все это проходит через один и тот же двухслойный scheduler и bounded admission-модель: active run queue для execution-ready акторов и temporal wakeup layer для future eligibility.

### 8.1 Архитектура: двухслойный планировщик

1. **Active run queue (`CurrentQueue` + `NextQueue`):** каждый `on_idle` pass формирует bounded execution queue из `CurrentQueue` плюс staged `NextQueue`; deferred carry-over в конце блока снова сохраняется в queue storage.
2. **Queue continuation:** каденс `every_blocks <= 1` пере-допускает актора через run-queue continuation (`NextQueue`) вместо timer index.
3. **Temporal wakeup layer (`WakeupIndex` + `MinWakeupBlock`):** управляет deferred eligibility и admitted overdue wakeups для timer-delayed акторов (`every_blocks > 1`) через каноническое block-bucketed wakeup storage, ограниченное `MaxWakeupBucketSize`, плюс actor-keyed live wakeup pointer; closed/missing actors удаляются лениво при drain их due bucket.
4. **Dedup epochs:** `QueueEpoch` инкрементируется каждый блок. `ActorQueueEpoch` фиксирует последний блок, в котором актор был поставлен в очередь, и предотвращает multi-enqueue amplification в рамках блока.

### 8.2 Пайплайн исполнения

Каждый блок ДОЛЖЕН выполнять пайплайн строго по порядку:

1. **Seed Active Queue:** загрузить `CurrentQueue`, смержить staged `NextQueue` и очистить same-epoch dedup markers для этого active set.
2. **Drain overdue timers:** итерировать от `MinWakeupBlock` до `current_block`, извлекать отложенных акторов из `WakeupIndex` и enqueue в active run queue. Ограничение: `MaxWakeupsPerBlock` как по числу admitted wakeups, так и по числу сканируемых просроченных block slots за один `on_idle` проход; close-path cleanup намеренно lazy и не сканирует future buckets.
3. **Ingest Address Events:** обрабатывать coalesced `AddressEvent` как сообщения-триггеры и enqueue подписанных акторов в active run queue.
4. **Execute Actors:** извлекать акторов из active run queue, исполнять до `MaxExecutionsPerBlock`. Deferred и leftover акторы переносятся в queue storage следующего блока.
5. **Persist Carry-Over:** записать deferred backlog в `CurrentQueue`, очистить `NextQueue`, инкрементировать `QueueEpoch`.

### 8.3 Дедупликация и защита от усиления нагрузки

1. **Deduplication:** актор НЕ ДОЛЖЕН присутствовать в очереди более одного раза в пределах блока, а delayed timers ДОЛЖНЫ держать не более одного live future wakeup на актора. Если `ActorQueueEpoch[actor] == QueueEpoch`, enqueue-запрос ДОЛЖЕН игнорироваться.
2. **Amplification limit:** планировщик ДОЛЖЕН применять `MaxQueueInsertionsPerBlock`. При превышении лимита scheduling wakeup ДОЛЖЕН детерминированно пробовать бакеты по порядку: `requested_block`, `requested_block + 1`, ..., `requested_block + MaxSpilloverBlocks`.
3. **Bounded spillover horizon:** в reference pallet `MaxSpilloverBlocks` равен `8` (до 9 проверяемых бакетов включая requested block).
4. **Overflow observability:** если scheduling wakeup не находит емкость в этом горизонте, рантайм ДОЛЖЕН инкрементировать `WakeupScheduleDrops` и эмитить `WakeupScheduleDropped { aaa_id, requested_block }`.
5. **Inbox latch retention:** `WakeupScheduleDropped` НЕ ДОЛЖЕН очищать `AddressEventInbox.is_pending`. Retry требует следующей enqueue-попытки (новое подходящее ingress-событие, manual trigger или явный schedule priming path).

### 8.4 Бюджет и справедливость

1. Рантайм ДОЛЖЕН резервировать минимум `MinOnIdleReservePct` веса блока для путей `on_idle`.
2. Цикл исполнения AAA использует остаточный `on_idle` вес после ограниченного служебного прохода.
3. Порядок исполнения внутри очереди выполнения ДОЛЖЕН быть детерминированным.
4. При исчерпании веса блока или лимита `MaxExecutionsPerBlock` оставшиеся элементы активной очереди выполнения ДОЛЖНЫ переноситься через ограниченное хранилище очереди следующего блока, обеспечивая естественную round-robin справедливость.
5. Для эталонного рантайма целевой показатель справедливости в high-density сценариях stress-lane matrix — spread nonce `<= 3`; starvation (`min_nonce == 0`) запрещен.

### 8.5 Sweep

1. `permissionless_sweep` и `permissionless_sweep_many` — только lifecycle touchpoints: они ДОЛЖНЫ немедленно оценивать terminal liveness и НЕ ДОЛЖНЫ напрямую enqueue/admit/execute циклы.
2. Breaker state НЕ ДОЛЖЕН блокировать sweep-time liveness evaluation или terminal closure; если актор остается живым, вызов возвращается без мутации очередей.
3. `SweepCursor` iteration и batch accounting ДОЛЖНЫ терпимо обрабатывать missing/closed `AaaId` entries и продолжать traversal без abort.

### 8.6 Защита от голодания

Поскольку глобальный опрос отсутствует, продвижение вперёд обеспечивается через bounded double-buffer и явную телеметрию голодания:

1. `MinWakeupBlock` ДОЛЖЕН монотонно продвигаться при drain `WakeupIndex`, включая sparse-gap recovery после длинных halt/gap интервалов.
2. Запланированное исполнение (timer/event) ДОЛЖНО переноситься через queue carry-over и wakeup spillover; если bounded wakeup spillover исчерпан, рантайм ДОЛЖЕН зафиксировать инцидент через `WakeupScheduleDropped` и `WakeupScheduleDrops`, а closed/missing actors МОГУТ оставлять не более одной stale future-bucket записи до lazy due-time drop.
3. `IdleStarvationBlocks` ДОЛЖЕН увеличиваться только когда breaker неактивен и оставшийся `on_idle` budget после bounded housekeeping равен нулю.
4. `IdleStarvationBlocks` ДОЛЖЕН сбрасываться в ноль сразу, как только появляется положительный post-housekeeping execution budget, включая блоки, где ready-акторов нет.
5. `IdleStarvationDetected` ДОЛЖЕН эмититься ровно один раз при пересечении порога и НЕ ДОЛЖЕН повторяться на каждом следующем starvation block.
6. Starvation telemetry является только observability-механизмом; она НЕ ДОЛЖНА запускать emergency cycle execution или любой альтернативный scheduler path.

---

## 9. Хуки рантайма

### 9.1 `on_initialize`

- ДОЛЖЕН оставаться bounded и deterministic.
- НЕ ДОЛЖЕН dispatch-ить AAA cycles.
- МОЖЕТ делать минимальный bookkeeping.

### 9.2 `on_idle`

- ДОЛЖЕН сначала выполнять bounded housekeeping (address-event ingress + zombie sweep).
- При неактивном breaker: исполнять admitted cycles, используя весь оставшийся `on_idle` вес после housekeeping.
- При активном breaker: пропускать cycle execution и выполнять только housekeeping.
- МОЖЕТ выполнять bounded lazy readiness/inbox transitions.
- ДОЛЖЕН исполнять state machine `IdleStarvationBlocks` из Section 8.6 после bounded housekeeping и определения оставшегося execution budget.
- НЕ ДОЛЖЕН содержать unbounded loops.

---

## 10. Экстринсики

### 10.1 Экстринсики владельца / управления

| Экстринсик                                                                                   | Описание                                                                               |
| -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `create_user_aaa(mutability, schedule, schedule_window, execution_plan)`                     | Создать актора и выделить минимальный свободный слот владельца                         |
| `create_user_aaa_at_slot(owner_slot, mutability, schedule, schedule_window, execution_plan)` | Создать актора в точном слоте владельца для детерминированного восстановления          |
| `pause_aaa(aaa_id)`                                                                          | Поставить актора на паузу (только Mutable)                                             |
| `resume_aaa(aaa_id)`                                                                         | Снять актора с паузы (только Mutable)                                                  |
| `manual_trigger(aaa_id)`                                                                     | Установить флаг ручного триггера                                                       |
| `fund_aaa(aaa_id, asset, amount)`                                                            | Пополнить отслеживаемый актив                                                          |
| `close_aaa(aaa_id)`                                                                          | Закрытие по инициативе владельца (удаление на месте)                                   |
| `update_schedule(aaa_id, schedule, schedule_window)`                                         | Обновить расписание/окно (только Mutable)                                              |
| `update_execution_plan(aaa_id, execution_plan)`                                              | Заменить рабочий план исполнения; сбросить `consecutive_failures` (только Mutable)     |
| `set_auto_close_at_cycle_nonce(aaa_id, target)`                                              | Установить/очистить целевой nonce закрытия по циклам (только Mutable, bounded horizon) |
| `increment_auto_close_nonce(aaa_id, by)`                                                     | Продлить целевой nonce закрытия (`by > 0`, checked-add, bounded horizon)               |
| `update_on_close_execution_plan(aaa_id, on_close_execution_plan)`                            | Заменить план исполнения на этапе закрытия (только Mutable)                            |

`execution_plan` — нормативный термин для вектора шагов исполнения.
`pause_aaa`, `resume_aaa`, `manual_trigger`, `close_aaa`, `update_schedule`, `update_execution_plan`, `set_auto_close_at_cycle_nonce`, `increment_auto_close_nonce` и `update_on_close_execution_plan` используют один и тот же control gate: signed owner для обоих типов акторов плюс governance origin только для System AAA; governance НЕ ДОЛЖЕН управлять User AAA через этот путь.

`create_user_aaa` ДОЛЖЕН платить обычные tx fees, взимать `AaaCreationFee` в `FeeSink` (Section 4.4) и соблюдать deferred-horizon cap (Section 7.4).

`create_user_aaa` и `create_system_aaa` ДОЛЖНЫ завершаться с `ActiveAaaCapacityExceeded`, когда количество активных AAA достигает `ActiveActorLimit`.

### 10.2 Экстринсики governance

| Экстринсик                                 | Описание                                                                                             |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `create_system_aaa(...)`                   | Создать System AAA (всегда Mutable)                                                                  |
| `reopen_system_aaa(aaa_id, ...)`           | Переоткрыть ранее закрытый System AAA с тем же `aaa_id`                                              |
| `set_global_circuit_breaker(paused: bool)` | Глобально остановить/возобновить планировщик                                                         |
| `set_active_actor_limit(new_limit: u32)`   | Обновить governance-лимит активных акторов (`0 < new_limit <= min(MaxActiveActors, MaxQueueLength)`) |

`create_system_aaa(...)` ДОЛЖЕН выделять fresh `aaa_id = NextAaaId`. `reopen_system_aaa(aaa_id, ...)` — единственный стабильный explicit-id governance path и ДОЛЖЕН сохранять детерминированное повторное вычисление sovereign account без отката `NextAaaId`; подробные правила id/occupancy заданы в Section 2.3.

### 10.3 Служебные экстринсики

| Экстринсик                       | Описание                                                                |
| -------------------------------- | ----------------------------------------------------------------------- |
| `permissionless_sweep(aaa_id)`   | Принудительно выполнить lifecycle-проверку для одного актора (REQUIRED) |
| `permissionless_sweep_many(ids)` | Ограниченная пакетная lifecycle-проверка (`len <= MaxSweepPerBlock`)    |

### 10.4 Глобальный выключатель

Когда breaker активен:

1. Scheduler ДОЛЖЕН останавливать admitted cycle execution; bounded housekeeping и queue/inbox/wakeup bookkeeping МОГУТ продолжаться.
2. Creation extrinsics ДОЛЖНЫ падать с `GlobalCircuitBreakerActive`.
3. `fund_aaa`, `manual_trigger`, `close_aaa`, `permissionless_sweep` и `permissionless_sweep_many` ДОЛЖНЫ оставаться рабочими; queued work исполняется только после снятия breaker.

---

## 11. Наблюдаемость

### 11.1 События

```rust
AaaClosed { aaa_id, reason: CloseReason }
AaaCreated { aaa_id, owner, owner_slot, aaa_type, mutability, sovereign_account }
AaaFunded { aaa_id, asset, amount }
AaaPaused { aaa_id, reason: PauseReason }
AaaResumed { aaa_id }
ActiveActorLimitSet { old_limit: u32, new_limit: u32 }
AutoCloseNonceIncremented { aaa_id, old_target: Option<u64>, new_target: u64, by: u64 }
AutoCloseNonceSet { aaa_id, target: Option<u64> }
BurnExecuted { aaa_id, asset, amount }
CycleDeferred { aaa_id, reason: DeferReason }
CycleStarted { aaa_id, cycle_nonce }
CycleSummary { aaa_id, cycle_nonce, executed_steps, skipped_conditions, skipped_resolution, skipped_funding_unavailable, failed_steps }
ExecutionPlanUpdated { aaa_id }
GlobalCircuitBreakerSet { paused: bool }
IdleStarvationDetected { consecutive_blocks: u32 }
LiquidityAdded { aaa_id, asset_a, asset_b, lp_minted }
LiquidityRemoved { aaa_id, lp_asset, amount_a, amount_b }
ManualTriggerSet { aaa_id }
MintExecuted { aaa_id, asset, amount }
OnCloseExecutionPlanSummary { aaa_id, executed_steps, skipped_steps, failed_steps }
OnCloseExecutionPlanUpdated { aaa_id }
OnCloseStepFailed { aaa_id, step_index, error: DispatchError }
ScheduleUpdated { aaa_id }
SplitTransferExecuted { aaa_id, asset, total, distributed, retained, legs: u32, effective_legs: u32 }
StakeExecuted { aaa_id, asset, amount }
StakeNativeExecuted { aaa_id, amount, operator }
StepFailed { aaa_id, cycle_nonce, step_index, error: DispatchError }
StepSkipped { aaa_id, cycle_nonce, step_index, reason: StepSkippedReason }
SwapExecuted { aaa_id, asset_in, asset_out, amount_in, amount_out }
SweepBatchProcessed { requested: u32, closed: u32, alive: u32, missing: u32 }
TransferExecuted { aaa_id, asset, amount, to }
UnstakeExecuted { aaa_id, asset, shares }
WakeupRescheduled { aaa_id, requested_block, scheduled_block }
WakeupScheduleDropped { aaa_id, requested_block }
```

### 11.2 Корреляция циклов

Ключ корреляции для индексаторов — `(aaa_id, cycle_nonce)`.

Порядок событий:

1. **Обычное завершение:** `CycleStarted` → step-level events (`StepSkipped` / `StepFailed` / task events) → `CycleSummary`.
2. **Funding depletion:** `CycleStarted` → `StepSkipped { reason: FundingUnavailable }` → `CycleSummary`.
3. **Терминальное закрытие с допущенным close tail:** ноль или больше close-tail task events / `OnCloseStepFailed` events → `OnCloseExecutionPlanSummary` → `AaaClosed`.

Frontend-слою СЛЕДУЕТ собирать per-cycle step-status bitmask из `StepSkipped`/`StepFailed` events. `CycleSummary` авторитетен при наличии.

---

## 12. Справочник типов

### 12.1 Базовые типы

```rust
enum AaaType {
    User,
    System,
}

enum Mutability {
    Mutable,
    Immutable,
}

enum CloseReason {
    AutoCloseNonceReached,
    BalanceExhausted,
    ConsecutiveFailures,
    CycleNonceExhausted,
    FeeBudgetExhausted,
    OwnerInitiated,
    WindowExpired,
}

enum DeferReason {
    InsufficientWeightBudget,
}

enum StepSkippedReason {
    ConditionsNotMet,
    FundingUnavailable,
    ResolutionSkipped,
}

enum PauseReason {
    Manual,
    CycleNonceExhausted,
}

struct Schedule<AccountId, AssetId> {
    trigger: Trigger<AccountId, AssetId>,
    cooldown_blocks: u32,
}

enum AmountResolution<Balance> {
    Fixed(Balance),
    PercentageOfCurrent(Perbill),
    PercentageOfTrigger(Perbill),
    PercentageOfLastFunding(Perbill),
    AllBalance,
}

struct SplitLeg<AccountId> { to: AccountId, share: Perbill }

enum Condition<AssetId, Balance, BlockNumber> {
    BalanceAbove { asset: AssetId, threshold: Balance },
    BalanceBelow { asset: AssetId, threshold: Balance },
    BalanceEquals { asset: AssetId, threshold: Balance },
    BalanceNotEquals { asset: AssetId, threshold: Balance },
    BlockNumberAbove { threshold: BlockNumber },
    BlockNumberBelow { threshold: BlockNumber },
}

struct TaskParams { conditions: u32, split_legs: u32 }

enum Task<AccountId, AssetId, Balance> {
    Transfer { to: AccountId, asset: AssetId, amount: AmountResolution<Balance> },
    SplitTransfer { asset: AssetId, amount: AmountResolution<Balance>, legs: BoundedVec<SplitLeg<AccountId>, MaxSplitTransferLegs> },
    SwapExactIn { asset_in: AssetId, asset_out: AssetId, amount_in: AmountResolution<Balance>, slippage_tolerance: Perbill },
    SwapExactOut { asset_in: AssetId, asset_out: AssetId, amount_out: AmountResolution<Balance>, slippage_tolerance: Perbill },
    Burn { asset: AssetId, amount: AmountResolution<Balance> },
    Mint { asset: AssetId, amount: AmountResolution<Balance> },
    AddLiquidity { asset_a: AssetId, asset_b: AssetId, amount_a: AmountResolution<Balance>, amount_b: AmountResolution<Balance> },
    RemoveLiquidity { lp_asset: AssetId, amount: AmountResolution<Balance> },
    Stake { asset: AssetId, amount: AmountResolution<Balance> },
    StakeNative { amount: AmountResolution<Balance>, operator: AccountId },
    Unstake { asset: AssetId, shares: AmountResolution<Balance> },
    Noop,
}
```

### 12.2 Ошибки

```rust
enum Error {
    AaaIdOccupied,
    AaaIdOverflow,
    AaaNotFound,
    AaaPaused,
    ActiveAaaCapacityExceeded,
    ActiveAaaLimitExceedsQueueCapacity,
    ActiveAaaLimitTooHigh,
    ActiveAaaLimitTooLow,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceIncrementZero,
    AutoCloseNonceOverflow,
    EmptyExecutionPlan,
    ExecutionDelayTooLong,
    ExecutionPlanTooLong,
    GlobalCircuitBreakerActive,
    ImmutableAaa,
    InsecureEntropyProvider,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidAutoCloseNonce,
    InvalidOwnerSlot,
    InvalidScheduleWindow,
    InvalidSplitTransfer,
    InvalidTriggerConfiguration,
    MintNotAllowedForUserAaa,
    NotGovernance,
    NotOwner,
    NotPaused,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    SnapshotUnavailable,
    SovereignAccountCollision,
    SystemAaaNotClosed,
}
```

`AaaIdOccupied` применяется только к explicit-id попыткам reopen System AAA, когда запрошенный `aaa_id` уже активен. `EmptyExecutionPlan` в текущем стабильном контракте применяется и к run execution plan, и к close execution plan.

Resolution-time не-терминальные случаи (`Skipped`, `FundingUnavailable`) моделируются как детерминированные outcomes разрешения, а не как варианты `Error`.

---

## 13. Хранилище

> Все коллекции ДОЛЖНЫ оставаться bounded константами `Max*`. Изменения storage-layout ДОЛЖНЫ выполняться через versioned, idempotent, bounded миграции `OnRuntimeUpgrade`.

Эта таблица определяет stable storage surface. `WakeupIndex` и `MinWakeupBlock` входят в канонический temporal wakeup contract текущей стабильной линии, а не являются просто reference-runtime implementation notes. Runtime support stores, такие как `AaaReadiness`, `ActiveActorLimit`, ingress overflow/dedup state и `LastIngressIngestBlock`, остаются implementation-документированными в architecture docs, пока не будут явно повышены до этого контракта.

| Хранилище              | Тип                                                                 | Описание                                                                                       |
| ---------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `NextAaaId`            | `AaaId`                                                             | Верхняя граница монотонного аллокатора AAA id; reopen никогда не откатывает ее назад           |
| `AaaInstances`         | `Map<Blake2_128Concat(AaaId), AaaInstance>`                         | Полное состояние актора (`execution_plan`, жизненный цикл, funding)                            |
| `CurrentQueue`         | `BoundedVec<AaaId, MaxQueueLength>`                                 | Очередь исполнения текущего блока                                                              |
| `NextQueue`            | `BoundedVec<AaaId, MaxQueueLength>`                                 | Временное хранилище очереди, объединяемое на следующем `on_idle`                               |
| `WakeupIndex`          | `Map<Blake2_128(BlockNum), BoundedVec<AaaId, MaxWakeupBucketSize>>` | Канонический блочно-ведерный time-ordered индекс пробуждений                                   |
| `MinWakeupBlock`       | `BlockNumber`                                                       | Самый ранний неразрешенный блок в каноническом индексе пробуждений                             |
| `WakeupScheduleDrops`  | `u64`                                                               | Счетчик пробуждений, которые не удалось поставить в расписание                                 |
| `ActorQueueEpoch`      | `Map<Blake2_128Concat(AaaId), u64>`                                 | Маркер дедупликации постановки в очередь для каждого актора                                    |
| `QueueEpoch`           | `u64`                                                               | Глобальный счетчик поколений очереди                                                           |
| `AddressEventInbox`    | `Map<Blake2_128Concat(AaaId), InboxState>`                          | Состояние pending-latch для `OnAddressEvent` на уровне одного AAA                              |
| `OwnerSlotMask`        | `Map<Blake2_128Concat(AccountId), u8>`                              | Битовая маска занятости пользовательских слотов                                                |
| `SovereignIndex`       | `Map<Blake2_128Concat(AccountId), AaaId>`                           | Индекс владения активным суверенным аккаунтом                                                  |
| `SweepCursor`          | `AaaId`                                                             | Курсор для zombie sweep                                                                        |
| `GlobalCircuitBreaker` | `bool`                                                              | Глобальная остановка планировщика на уровне паллета                                            |
| `IdleStarvationBlocks` | `u32`                                                               | Последовательные блоки при неактивном breaker с нулевым post-housekeeping AAA execution budget |

---

## 14. Инварианты безопасности

Реализация соответствует, если выполняются все пункты. Каждый инвариант ссылается на нормативный источник:

1. AAA использует весь оставшийся `on_idle` budget после bounded housekeeping, а рантайм обеспечивает dispatchable headroom `MinOnIdleReservePct` (Section 8.4 item 1; Section 9.2)
2. Все loops и queues остаются ограниченными явными константами `Max*` (Section 1 item 2)
3. Slot allocation и active-occupancy мутации синхронны и race-safe (Section 1 item 8; Section 2.3)
4. Детерминизм сохраняется для одинакового state/context, включая entropy fallback order (Section 1 item 1; Section 7.1)
5. Hash текущего блока никогда не используется как entropy в runtime execution (Section 1 item 10; Section 7.1 item 8)
6. Адаптеры детерминированы: canonical iteration и fixed rounding (Section 3.2)
7. Нет recurring rent accrual или touch-based rent debit (Section 4.2)
8. `create_user_aaa` взимает невозвратную `AaaCreationFee` и маршрутизирует ее в `FeeSink` (Section 1 item 5; Section 4.4)
9. First eligible execution ограничен `MaxExecutionDelayBlocks` (Section 1 item 11; Section 7.4)
10. `reserved_fee_remaining` transient, а пути расходования `FeeNativeAsset` используют `spendable_fee_native` (Section 2.1 native-asset terminology; Section 4.3)
11. Weight deferrals сохраняют `cycle_nonce`, `consecutive_failures` и `last_cycle_block`; недостаток User fee на cycle admission терминален (Section 2.6 items 1, 3, 5, 6, 7; Section 4.1)
12. `manual_trigger_pending` очищается на admitted cycle start и сохраняется через deferrals, pause и resume (Section 7.3)
13. `SplitTransfer` сохраняет amount conservation, отклоняет `sum(share_i) > 100%` и детерминированно пропускает ED-unsafe legs (Section 6.2)
14. Amount resolution никогда молча не clamp-ит и при необходимости разрешается через `Skipped` или `FundingUnavailable` (Section 1 item 9; Section 5.3)
15. `OnAddressEvent` updates происходят только через adapter path, с детерминированной и bounded семантикой matching/dedup (Section 7.2)
16. Терминальное закрытие сохраняет суверенные балансы и никогда не делает automatic refund fan-out (Section 1 items 3 and 4; Section 2.4)
17. Close-tail execution использует ту же дисциплину task, condition, amount-resolution, adapter и weight-upper-bound, что и обычное исполнение, и НЕ ДОЛЖНО рекурсивно входить в новый close (Section 2.4; Section 4.1; Section 5.5)
18. Явные и автоматические close-пути текущей линии используют admitted close tail; автоматические close-пути откладываются, пока bounded `on_idle` budget не сможет допустить хвост, а истощение fee во время close вырождается в наблюдаемый per-step failure без блокировки финального закрытия (Section 2.4; Section 4.5; Section 11.1)
19. Circuit breaker останавливает scheduler execution, сохраняя bounded housekeeping и cleanup/tooling paths (Section 10.4)
20. Sweep остается bounded: `permissionless_sweep` — O(1), а `permissionless_sweep_many` — O(K ≤ MaxSweepPerBlock) (Section 8.5; Section 10.3)
21. `on_initialize` никогда не dispatch-ит AAA cycles, а starvation handling остается observability-only (Section 8.6 item 6; Section 9.1)
22. `TriggerSnapshot` собирается один раз на старте цикла или допущенного close tail, остается read-only и никогда не персистится (Section 5.4)
23. `FundingUnavailable` non-terminal, эмитит `StepSkipped` и не увеличивает `consecutive_failures` (Section 2.5 item 6; Section 5.3)
24. Исполнение планировщика строго bounded лимитами `MaxExecutionsPerBlock`, `MaxWakeupsPerBlock`, `MaxQueueInsertionsPerBlock` и `MaxWakeupBucketSize`; инциденты wakeup-overflow наблюдаемы через `WakeupScheduleDropped` / `WakeupScheduleDrops` (Section 8.3; Section 8.4)
25. `ActiveActorLimit` удовлетворяет `0 < limit <= min(MaxActiveActors, MaxQueueLength)`, а создание fail-fast при capacity (Section 10.1; Section 10.2)
26. Event-driven queueing обеспечивает строгую block-local deduplication через `ActorQueueEpoch`, предотвращая amplification-атаки (Section 8.1 item 4; Section 8.3)
27. Governance-обновления `ActiveActorLimit` fail-fast завершаются при `new_limit > MaxQueueLength`; default/effective operational cap остается queue-bounded, чтобы исключить scheduler actor-loss при полной активации (Section 10.2; Section 15)
28. Планирование timer гибридное и детерминированное: каденс `<=1` использует queue continuation, delayed timers используют канонический `WakeupIndex`, а bounded jitter снижает синхронные wakeup bursts (Section 7.1 items 3, 4, 5; Section 8.1 item 3)
29. `IdleStarvationBlocks` увеличивается только при неактивном breaker и нулевом post-housekeeping budget, сбрасывается при положительном budget и эмитит `IdleStarvationDetected` только на пересечении порога (Section 8.6 items 3, 4, 5; Section 9.2)

---

## 15. Константы рантайма

| Константа                                                   | Рекомендуемое значение                             | Описание                                                                           |
| ----------------------------------------------------------- | -------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `AaaCreationFee`                                            | Runtime-specific                                   | Невозвратная комиссия открытия, направляемая в `FeeSink`                           |
| `AaaPalletId`                                               | `PalletId(*b"aaactor0")`                           | 8-байтовый идентификатор паллета для деривации суверенного аккаунта (Section 2.3)  |
| `ActiveActorLimit`                                          | 1..`min(MaxActiveActors, MaxQueueLength)`          | Операционный лимит создания; значение по умолчанию/эффективное ограничено очередью |
| `ConditionReadFee`                                          | 0.0005 Native                                      | Комиссия за чтение одного условия                                                  |
| `MaxActiveActors`                                           | 10,000                                             | Жесткий предел одновременно активных AAA                                           |
| `MaxQueueLength`                                            | 1,024–16,384                                       | Верхняя граница длины `CurrentQueue`/`NextQueue`                                   |
| `MaxWakeupBucketSize`                                       | 1,024–16,384                                       | Верхняя граница одного ведра `WakeupIndex`                                         |
| `MaxQueueInsertionsPerBlock`                                | 64–1,024                                           | Лимит постановок в очередь на блок до spillover пробуждений                        |
| `MaxSpilloverBlocks`                                        | 8                                                  | Горизонт spillover для пробуждений                                                 |
| `MaxWakeupsPerBlock`                                        | 64–1,024                                           | Ограниченная пропускная способность drain просроченных пробуждений                 |
| `MaxConditionsPerStep`                                      | 4                                                  | Максимум условий на шаг                                                            |
| `MaxConsecutiveFailures`                                    | 10                                                 | Терминальный порог                                                                 |
| `MaxExecutionDelayBlocks`                                   | 10 years in blocks (e.g. 52,560,000 @ 6s)          | Максимально допустимая отсрочка первого исполнения                                 |
| `MaxTimerJitterBlocks`                                      | 32–128                                             | Детерминированный лимит jitter для отложенных таймеров                             |
| `MaxExecutionsPerBlock`                                     | 16–64                                              | Глобальный лимит допущенных исполнений на блок                                     |
| `MaxFundingTrackedAssets`                                   | 3–10                                               | Активы, отслеживаемые через `PercentageOfLastFunding` на один AAA                  |
| `MaxIdleStarvationBlocks`                                   | 10–50                                              | Порог нулевого `on_idle` перед сигналом о голодании                                |
| `MaxK`                                                      | Runtime-specific                                   | Предел O(K) для адаптера                                                           |
| `MaxOwnerSlots`                                             | 8                                                  | Пространство пользовательских слотов AAA (`u8`-битовая маска)                      |
| `MaxSplitTransferLegs`                                      | 8                                                  | Максимум получателей в split fan-out                                               |
| `MaxSweepPerBlock`                                          | 5                                                  | Пропускная способность zombie sweep                                                |
| `MaxSystemExecutionPlanSteps` / `MaxUserExecutionPlanSteps` | 10 / 3                                             | Границы числа шагов для System/User AAA                                            |
| `MaxWhitelistSize`                                          | 16                                                 | Максимальная длина whitelist-фильтра источников                                    |
| `MinOnIdleReservePct`                                       | 10%                                                | Минимальный резерв веса блока под пути `on_idle`                                   |
| `MinUserBalance`                                            | Runtime-specific, MUST be `>=` `FeeNativeAsset` ED | Предохранительный минимум пользователя перед циклом и защита от reap               |
| `MinWindowLength`                                           | 100 blocks                                         | Минимальная длина окна расписания                                                  |
| `StepBaseFee`                                               | 0.001 Native                                       | Базовая комиссия оценки одного шага                                                |

---

_Конец спецификации._
