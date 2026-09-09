# Runtime Weight Delta Ledger

## Evidence Boundary

This generated ledger compares the production Weight implementations in Git tag `v0.7.24` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.

Candidate release: `0.7.25`. The locally validated production runtime was generated with `./scripts/03-build-runtime.sh`; compact Wasm SHA-256 is `25b9695fd9e900f17ae1f3fb0b815ac1403830264d9c31f7cee54e29f434b700`. The accepted benchmark owners use `frame-omni-bencher 0.22.0` / CLI `58.0.0`, `50` steps, `20` repeats, compiled Wasm execution, RocksDB, 1,024 MiB cache, host `fedora`, and CPU `AMD Ryzen 7 4800H with Radeon Graphics`; each generated method records date, reads, writes, measured ProofSize, and conservative ProofSize in its authoritative source. The benchmark-runtime Wasm and production Wasm are distinct evidence identities. Exact candidate commit/tree identity remains unavailable until the validated worktree is committed through the authorized release gate.

Interpretation codes classify changed paths only: `I` identity guard; `C` correctness; `P` bounded service topology; `M` merged canonical work; `O` measured optimization.

## Changed Production Paths

| Pallet | Weight method | RefTime: v0.7.24 → 0.7.25 candidate | Base delta | ProofSize: v0.7.24 → 0.7.25 candidate | Reads: v0.7.24 → 0.7.25 candidate | Writes: v0.7.24 → 0.7.25 candidate | Code |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Actors | `create_user_actor` | `489,874,000 → 563,908,000` | +15.11% | `81,886 → 81,886` | `65 → 63` | `60 → 55` | C |
| Actors | `create_user_actor_at_slot` | `481,145,000 → 604,277,000` | +25.59% | `81,886 → 81,886` | `65 → 63` | `60 → 55` | C |
| Actors | `create_system_actor` | `541,837,000 → 594,079,000` | +9.64% | `81,886 → 81,886` | `63 → 61` | `58 → 53` | C |
| Actors | `create_system_actor_at_sovereign_id` | `528,846,000 → 743,612,000` | +40.61% | `81,886 → 81,886` | `61 → 59` | `56 → 51` | C |
| Actors | `create_user_actor_crossing_new_page` | `540,999,000 → 755,066,000` | +39.57% | `53,350 → 27,170` | `33 → 31` | `28 → 23` | C |
| Actors | `create_dormant_system_actor` | `70,751,000 → 139,895,000` | +97.73% | `5,736 → 7,535` | `13 → 17` | `7 → 7` | C |
| Actors | `activate_actor` | `520,675,000 → 878,618,000` | +68.75% | `81,886 → 81,886` | `57 → 55` | `52 → 47` | C |
| Actors | `deactivate_actor` | `561,742,000 → 1,255,487,000` | +123.50% | `81,886 → 81,886` | `57 → 58` | `55 → 57` | C |
| Actors | `pause_actor` | `70,541,000 → 140,663,000` | +99.41% | `5,736 → 5,736` | `7 → 8` | `2 → 1` | C |
| Actors | `resume_actor` | `69,214,000 → 123,482,000` | +78.41% | `5,736 → 5,736` | `7 → 8` | `2 → 2` | C |
| Actors | `manual_trigger` | `155,120,000 → 243,890,000` | +57.23% | `9,635 → 15,106` | `14 → 14` | `7 → 7` | C |
| Actors | `address_event_trigger_occurrence` | `171,114,000 → 292,779,000` | +71.10% | `8,367 → 8,394` | `14 → 14` | `7 → 7` | C |
| Actors | `pipeline_admission_apoptosis` | `140,872,000 → 387,346,000` | +174.96% | `5,736 → 15,106` | `15 → 19` | `15 → 16` | C |
| Actors | `close_actor` | `708,551,000 → 973,883,000` | +37.45% | `81,886 → 81,886` | `64 → 66` | `64 → 65` | C |
| Actors | `update_contract` | `984,638,000 → 1,352,289,000` | +37.34% | `81,886 → 162,782` | `67 → 101` | `63 → 94` | C |
| Actors | `set_global_circuit_breaker` | `6,845,000 → 7,683,000` | +12.24% | `0 → 0` | `0 → 0` | `1 → 1` | C |
| Actors | `record_crossing_worker_fault` | `11,105,000 → 11,874,000` | +6.92% | `1,529 → 1,529` | `1 → 1` | `1 → 1` | C |
| Actors | `record_observation_fanout_worker_fault` | `20,464,000 → 34,851,000` | +70.30% | `4,106 → 4,106` | `3 → 3` | `1 → 1` | C |
| Actors | `record_wakeup_worker_fault` | `10,057,000 → 9,917,000` | -1.39% | `1,503 → 1,503` | `1 → 1` | `1 → 1` | C |
| Actors | `clear_crossing_worker_fault` | `13,759,000 → 14,597,000` | +6.09% | `1,529 → 1,529` | `1 → 1` | `1 → 1` | C |
| Actors | `clear_observation_fanout_worker_fault` | `13,968,000 → 15,155,000` | +8.50% | `1,629 → 1,629` | `1 → 1` | `1 → 1` | C |
| Actors | `clear_wakeup_worker_fault` | `12,501,000 → 12,851,000` | +2.80% | `1,503 → 1,503` | `1 → 1` | `1 → 1` | C |
| Actors | `set_active_actor_limit` | `9,918,000 → 11,454,000` | +15.49% | `1,489 → 1,489` | `2 → 2` | `0 → 0` | C |
| Actors | `permissionless_sweep` | `48,610,000 → 89,608,000` | +84.34% | `5,736 → 5,736` | `7 → 8` | `0 → 0` | C |
| Actors | `permissionless_sweep_many` | `26,061,500 + 140,666,863·n → 86,378,643 + 173,665,591·n` | +231.44% | `1,489 + 4,746·n → 1,489 + 4,746·n` | `3 + 14·n → 3 + 15·n` | `2 + 14·n → 2 + 13·n` | C |
| Actors | `fee_collection` | `43,512,000 → 50,007,000` | +14.93% | `3,593 → 6,196` | `1 → 2` | `1 → 2` | O |
| Actors | `action_invocation_receipt` | `— → 4,680,000` | new | `— → 0` | `— → 0` | `— → 0` | C |
| Actors | `task_transfer` | `338,037,000 → 468,433,000` | +38.57% | `18,280 → 29,222` | `21 → 25` | `8 → 12` | C |
| Actors | `task_burn` | `18,997,000 → 20,045,000` | +5.52% | `3,593 → 3,593` | `1 → 1` | `1 → 1` | C |
| Actors | `task_mint` | `292,011,000 → 343,415,000` | +17.60% | `18,280 → 29,222` | `18 → 22` | `6 → 10` | C |
| Actors | `predicate_set_evaluation` | `7,264,000 + 6,569,170·c → 6,906,425 + 11,187,530·c` | -4.92% | `3,675 + 674·c → 3,839 + 1,740·c` | `1 + 1·c → 0 + 2·c` | `0 → 0` | C |
| Actors | `task_stop_cycle` | `4,260,000 → 6,705,000` | +57.39% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `task_split_transfer` | `157,986,014 + 196,105,670·l → 163,392,331 + 310,098,141·l` | +3.42% | `18,280 + 4,746·l → 29,222 + 4,746·l` | `11 + 9·l → 14 + 10·l` | `4 + 3·l → 7 + 4·l` | C |
| Actors | `xcm_asset_deposit` | `326,303,000 → 458,445,000` | +40.50% | `18,280 → 29,222` | `21 → 25` | `8 → 12` | C |
| Actors | `task_add_liquidity` | `281,953,000 → 316,317,000` | +12.19% | `34,255 → 34,255` | `16 → 16` | `15 → 15` | C |
| Actors | `task_donate_liquidity` | `164,968,000 → 181,242,000` | +9.86% | `14,035 → 14,035` | `9 → 9` | `8 → 8` | C |
| Actors | `task_remove_liquidity` | `152,675,000 → 164,758,000` | +7.91% | `8,817 → 8,817` | `8 → 8` | `6 → 6` | C |
| Actors | `task_stake` | `85,976,000 → 91,005,000` | +5.85% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_unstake` | `101,900,000 → 112,935,000` | +10.83% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_dex_exact_in` | `536,949,000 → 718,678,000` | +33.84% | `19,253 → 19,253` | `41 → 41` | `17 → 17` | O |
| Actors | `task_dex_exact_out` | `530,243,000 → 714,697,000` | +34.79% | `19,253 → 19,253` | `40 → 40` | `17 → 17` | O |
| Actors | `scheduler_on_initialize_cutoff` | `10,616,000 → 12,292,000` | +15.79% | `1,560 → 1,560` | `2 → 2` | `2 → 2` | C |
| Actors | `scheduler_on_idle_base` | `16,832,000 → 19,626,000` | +16.60% | `1,560 → 1,560` | `7 → 7` | `2 → 2` | C |
| Actors | `materialization_coordinator_base` | `26,680,000 → 25,842,000` | -3.14% | `5,982 → 5,982` | `10 → 10` | `1 → 1` | C |
| Actors | `contract_geometry_create` | `29,462,359 + 5,346,654·c → 35,052,172 + 6,036,087·c` | +18.97% | `4,494 + 2,475·c → 4,499 + 2,475·c` | `3 + 1·c → 4 + 1·c` | `2 + 1·c → 1 + 1·c` | C |
| Actors | `contract_geometry_close` | `27,029,761 + 6,953,989·c → 34,244,305 + 8,784,131·c` | +26.69% | `4,557 + 2,670·c → 4,740 + 2,669·c` | `2 + 1·c → 3 + 1·c` | `3 + 1·c → 2 + 1·c` | C |
| Actors | `contract_geometry_reconstruct` | `23,313,535 + 5,451,102·c → 31,311,645 + 6,659,243·c` | +34.31% | `4,557 + 2,669·c → 4,740 + 2,669·c` | `2 + 1·c → 3 + 1·c` | `0 → 0` | C |
| Actors | `current_step_load_head` | `20,254,000 → 30,451,000` | +50.35% | `4,513 → 4,731` | `2 → 3` | `0 → 0` | C |
| Actors | `current_step_load_tail` | `25,950,252 + 79,315·s → 39,333,653` | +51.57% | `4,729 + 14·s → 4,913 + 14·s` | `3 → 4` | `0 → 0` | C |
| Actors | `current_step_plan_opening_head` | `43,442,000 → 81,297,000` | +87.14% | `5,223 → 4,968` | `6 → 5` | `0 → 0` | C |
| Actors | `current_step_plan_suspended_head` | `71,030,000 → 123,132,000` | +73.35% | `8,058 → 5,707` | `7 → 6` | `0 → 0` | C |
| Actors | `current_step_plan_running_tail` | `76,192,916 + 131,564·s → 117,787,560 + 1,082,421·s` | +54.59% | `8,238 + 14·s → 5,796 + 73·s` | `8 → 7` | `0 → 0` | C |
| Actors | `opening_snapshot_traversal` | `— → 1,397,000` | new | `— → 0` | `— → 0` | `— → 0` | C |
| Actors | `opening_snapshot_capture` | `2,403,773 + 9,035,928·e → 6,280,406 + 10,546,366·e` | +161.27% | `4,373 + 2,260·e → 1,366 + 2,836·e` | `1 + 2·e → 0 + 2·e` | `0 → 0` | C |
| Actors | `opening_target_snapshot_capture` | `— → 11,715,105 + 8,545,944·e` | new | `— → 1,377 + 2,834·e` | `— → 0 + 2·e` | `— → 0` | C |
| Actors | `opening_share_mixed_capture` | `— → 6,913,373 + 10,801,886·e` | new | `— → 1,308 + 2,838·e` | `— → 0 + 2·e` | `— → 0` | C |
| Actors | `opening_predicate_traversal` | `— → 1,466,000` | new | `— → 0` | `— → 0` | `— → 0` | C |
| Actors | `opening_predicate_capture` | `3,046,526 + 7,474,282·p → 5,665,087 + 9,642,493·p` | +85.95% | `4,235 + 2,524·p → 4,184 + 2,673·p` | `1 + 2·p → 0 + 2·p` | `0 → 0` | C |
| Actors | `predicate_asset_evaluation` | `— → 8,967,155 + 8,774,724·p` | new | `— → 1,344 + 2,838·p` | `— → 0 + 2·p` | `— → 0` | C |
| Actors | `predicate_observation_heavy_evaluation` | `— → 20,339,756 + 10,257,124·o` | new | `— → 4,364 + 2,673·o` | `— → 2 + 2·o` | `— → 0` | C |
| Actors | `opening_max_encoded_balance_capture` | `— → 8,117,762 + 8,776,187·p` | new | `— → 1,358 + 2,836·p` | `— → 0 + 2·p` | `— → 0` | C |
| Actors | `opening_observation_heavy_capture` | `— → 12,070,595 + 10,272,731·o` | new | `— → 4,329 + 2,667·o` | `— → 2 + 2·o` | `— → 0` | C |
| Actors | `scheduler_actor_state_probe` | `68,586,000 → 114,890,000` | +67.51% | `5,998 → 15,106` | `7 → 7` | `0 → 0` | M |
| Actors | `scheduler_paged_append_existing_page` | `118,313,000 → 124,040,000` | +4.84% | `14,048 → 13,660` | `11 → 5` | `5 → 4` | C |
| Actors | `scheduler_paged_append_new_page` | `111,538,000 → 109,653,000` | -1.69% | `16,435 → 16,446` | `12 → 6` | `5 → 4` | C |
| Actors | `scheduler_wakeup_append_existing_page` | `93,309,000 → 162,035,000` | +73.65% | `7,566 → 14,366` | `9 → 7` | `3 → 5` | C |
| Actors | `scheduler_wakeup_append_new_page` | `98,688,000 → 138,637,000` | +40.48% | `7,739 → 17,169` | `9 → 8` | `4 → 6` | C |
| Actors | `scheduler_wakeup_replace_exact` | `105,043,000 → 202,054,000` | +92.35% | `8,001 → 7,820` | `11 → 17` | `7 → 13` | C |
| Actors | `scheduler_wakeup_invalidate_middle_page` | `132,631,000 → 201,705,000` | +52.08% | `13,538 → 21,681` | `11 → 11` | `5 → 5` | C |
| Actors | `scheduler_wakeup_drain_partial_page` | `453,347,000 → 2,162,182,000` | +376.94% | `53,971 → 56,539` | `99 → 70` | `18 → 19` | C |
| Actors | `scheduler_wakeup_drain_full_page` | `833,220,000 → 3,699,832,000` | +344.04% | `101,637 → 99,272` | `196 → 135` | `36 → 39` | C |
| Actors | `scheduler_wakeup_drain_dense_boundary` | `882,669,000 → 3,698,296,000` | +318.99% | `104,784 → 102,394` | `203 → 140` | `38 → 41` | C |
| Actors | `scheduler_wakeup_drain_stale_page` | `771,200,000 → 1,357,457,000` | +76.02% | `101,029 → 82,600` | `196 → 39` | `4 → 7` | C |
| Actors | `scheduler_wakeup_cursor_insert` | `345,650,000 → 442,243,000` | +27.95% | `42,706 → 41,816` | `25 → 39` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_pop_min` | `450,902,000 → 571,450,000` | +26.73% | `55,232 → 54,358` | `34 → 49` | `26 → 26` | C |
| Actors | `scheduler_wakeup_cursor_remove_exact` | `420,102,000 → 539,812,000` | +28.50% | `54,699 → 53,887` | `33 → 47` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_worker_partial` | `127,323,000 → 235,997,000` | +85.35% | `7,976 → 7,959` | `16 → 21` | `8 → 13` | C |
| Actors | `at_time_trigger_occurrence` | `214,136,000 → 349,561,000` | +63.24% | `8,552 → 8,451` | `20 → 20` | `9 → 9` | C |
| Actors | `cadenced_trigger_occurrence` | `241,445,000 → 536,041,000` | +122.01% | `8,505 → 8,450` | `22 → 27` | `11 → 16` | C |
| Actors | `scheduler_wakeup_cursor_worker_remove` | `575,780,000 → 806,400,000` | +40.05% | `56,912 → 55,857` | `50 → 65` | `33 → 35` | C |
| Actors | `scheduler_wakeup_cursor_worker_future` | `25,283,000 → 24,305,000` | -3.87% | `6,608 → 6,566` | `6 → 6` | `0 → 0` | C |
| Actors | `scheduler_paged_consume_preserve_page` | `57,620,000 → 72,287,000` | +25.45% | `5,528 → 5,118` | `10 → 5` | `3 → 4` | C |
| Actors | `scheduler_paged_consume_delete_page` | `59,086,000 → 67,258,000` | +13.83% | `5,428 → 4,809` | `10 → 5` | `5 → 4` | C |
| Actors | `scheduler_paged_tombstone_drain` | `33,874,000 + 3,778,028·n → 35,340,000 + 323,374·n` | +4.33% | `3,778 + 2,572·n → 3,032 + 79·n` | `5 + 2·n → 4` | `4 → 2` | C |
| Actors | `scheduler_paged_mixed_scan` | `37,156,000 + 47,657,638·n → 35,550,000 + 80,648,201·n` | -4.32% | `5,121 + 2,866·n → 4,675 + 1,505·n` | `3 + 4·n → 2 + 2·n` | `3 + 1·n → 2 + 1·n` | C |
| Actors | `scheduler_inner_zero_step_complete` | `60,973,000 → 58,109,000` | -4.70% | `5,537 → 4,388` | `7 → 2` | `3 → 3` | C |
| Actors | `scheduler_paged_zero_step_user_crossing_unavailable` | `— → 161,755,000` | new | `— → 6,127` | `— → 12` | `— → 0` | C |
| Actors | `scheduler_paged_execute_opening_max` | `549,800,000 → 593,660,000` | +7.98% | `27,824 → 14,158` | `30 → 24` | `15 → 13` | C |
| Actors | `scheduler_inner_opening_failed_min` | `89,459,955 + 6,187,462·t → 109,517,654 + 25,013,024·t` | +22.42% | `6,729 + 2,670·t → 5,237 + 3,000·t` | `6 + 1·t → 6 + 1·t` | `3 → 5` | C |
| Actors | `scheduler_inner_opening_retry_min` | `135,268,177 + 6,539,368·t → 195,144,203 + 29,296,426·t` | +44.26% | `6,780 + 2,669·t → 5,946 + 2,950·t` | `11 + 1·t → 15 + 1·t` | `8 → 11` | C |
| Actors | `scheduler_inner_opening_failed_max` | `129,307,530 + 129,086,543·t → 165,770,741 + 94,996,343·t` | +28.20% | `8,347 + 22,129·t → 12,845 + 10,106·t` | `9 + 17·t → 12 + 8·t` | `3 → 5` | C |
| Actors | `scheduler_inner_opening_retry_max` | `174,001,916 + 131,385,472·t → 228,451,910 + 47,226,607·t` | +31.29% | `8,179 + 22,125·t → 8,291 + 5,515·t` | `13 + 17·t → 18 + 5·t` | `8 → 11` | C |
| Actors | `scheduler_inner_opening_complete_min` | `89,448,696 + 6,148,376·t → 68,359,831 + 11,019,188·t` | -23.58% | `6,694 + 2,670·t → 4,398` | `6 + 1·t → 2` | `3 → 3` | C |
| Actors | `scheduler_inner_opening_user_complete_header_max` | `— → 175,025,000` | new | `— → 7,990` | `— → 9` | `— → 5` | C |
| Actors | `scheduler_inner_opening_user_complete_header_max_tail` | `— → 221,092,770 + 17,027,764·t` | new | `— → 10,677 + 2,834·t` | `— → 11 + 1·t` | `— → 8` | C |
| Actors | `scheduler_inner_opening_progress_min` | `151,658,315 + 6,196,436·t → 185,578,808 + 14,888,868·t` | +22.37% | `7,045 + 2,670·t → 6,363 + 2,957·t` | `13 + 1·t → 11 + 1·t` | `8 → 7` | C |
| Actors | `scheduler_inner_opening_complete_max` | `126,799,350 + 127,843,620·t → 153,228,555 + 90,160,651·t` | +20.84% | `8,312 + 22,129·t → 9,978 + 10,416·t` | `9 + 17·t → 11 + 8·t` | `3 → 3` | C |
| Actors | `scheduler_inner_opening_progress_max` | `202,040,691 + 132,879,336·t → 313,030,635 + 15,870,597·t` | +54.93% | `8,640 + 22,090·t → 49,359 + 3,202·t` | `15 + 17·t → 44 + 1·t` | `8 → 7` | C |
| Actors | `scheduler_inner_running_complete` | `95,782,916 + 2,552,077·s + 11,013,042·p → 133,921,853 + 2,058,487·s + 20,721,195·p` | +39.82% | `7,016 + 2,718·p + 49·s → 10,952 + 2,914·p + 11·s` | `9 + 2·p → 11 + 2·p` | `4 → 4` | C |
| Actors | `scheduler_inner_running_progress` | `112,950,315 + 1,651,829·s + 8,886,496·p → 284,303,244 + 6,766,806·p` | +151.71% | `6,762 + 2,693·p + 20·s → 12,546 + 2,220·p` | `13 + 2·p → 18 + 2·p` | `6 → 5` | C |
| Actors | `scheduler_inner_suspended_tail_retry` | `127,671,072 + 1,427,388·s + 8,247,122·p → 224,662,450 + 3,771,914·s + 15,334,382·p` | +75.97% | `6,244 + 2,693·p + 12·s → 6,937 + 2,210·p + 101·s` | `9 + 2·p → 15 + 2·p` | `6 → 9` | C |
| Actors | `scheduler_inner_suspended_tail_complete` | `135,401,769 + 1,926,065·s + 9,564,179·p → 213,795,680 + 15,584,645·p` | +57.90% | `7,294 + 2,718·p + 26·s → 8,925 + 2,207·p + 204·s` | `10 + 2·p → 8 + 2·p` | `4 → 8` | C |
| Actors | `scheduler_inner_suspended_tail_progress` | `145,948,500 + 2,112,826·s + 9,053,092·p → 244,666,435 + 853,649·s + 21,015,964·p` | +67.64% | `6,764 + 2,693·p + 20·s → 9,954 + 2,192·p + 491·s` | `13 + 2·p → 14 + 2·p` | `6 → 9` | C |
| Actors | `scheduler_inner_suspended_head_retry` | `101,216,218 + 624,373·n + 77,816·r + 605,547·f + 7,436,390·p → 233,830,626 + 244,723·n + 9,611,569·p` | +131.02% | `5,133 + 21·f + 22·n + 2,693·p + 2·r → 6,848 + 2,793·p` | `9 + 2·p → 14 + 2·p` | `6 → 9` | C |
| Actors | `scheduler_inner_suspended_head_complete` | `109,542,628 + 544,051·n + 34,686·r + 520,243·f + 8,244,070·p → 145,300,991 + 1,898,621·p` | +32.64% | `6,038 + 21·f + 22·n + 2,716·p + 2·r → 6,004 + 2,701·p` | `9 + 2·p → 6 + 2·p` | `4 → 4` | C |
| Actors | `scheduler_inner_suspended_head_progress` | `111,036,829 + 803,981·n + 91,680·r + 827,461·f + 7,824,145·p → 224,981,329 + 39,366·n + 11,797,163·p` | +102.62% | `5,721 + 21·f + 22·n + 2,693·p + 2·r → 7,784 + 2,793·p` | `12 + 2·p → 12 + 2·p` | `6 → 5` | C |
| Actors | `scheduler_inner_suspended_head_opening_retry` | `122,805,125 + 662,864·n + 77,292·r + 715,051·f → 273,071,818 + 6,709,813·p` | +122.36% | `11,197 + 21·f + 22·n + 2·r → 6,832 + 2,786·p` | `15 → 14 + 2·p` | `6 → 9` | C |
| Actors | `scheduler_inner_suspended_head_opening_complete` | `128,627,138 + 574,669·n + 64,436·r + 599,401·f → 128,652,913 + 4,574,010·p` | +0.02% | `11,985 + 21·f + 22·n + 2·r → 6,017 + 2,694·p` | `14 → 6 + 2·p` | `4 → 4` | C |
| Actors | `scheduler_inner_suspended_head_opening_progress` | `135,507,223 + 806,693·n + 104,938·r + 820,306·f → 236,649,836 + 11,253,145·p` | +74.64% | `11,578 + 21·f + 22·n + 2·r → 7,919 + 2,736·p` | `17 → 12 + 2·p` | `6 → 5` | C |
| Actors | `scheduler_paged_execute_cheap` | `139,405,000 + 122,349,209·n → 199,820,000 + 251,803,938·n` | +43.34% | `4,210 + 3,171·n → 3,993 + 3,009·n` | `6 + 7·n → 5 + 5·n` | `4 + 3·n → 3 + 3·n` | C |
| Actors | `scheduler_paged_execute_cheap_mixed` | `357,034,000 + 190,370,288·n → 601,203,000 + 358,077,352·n` | +68.39% | `4,772 + 3,361·n → 6,958 + 3,206·n` | `5 + 11·n → 7 + 8·n` | `4 + 5·n → 4 + 4·n` | C |
| Actors | `run_progress` | `104,904,000 → 279,510,000` | +166.44% | `8,567 → 6,456` | `9 → 14` | `2 → 9` | C |
| Actors | `run_suspend` | `103,646,000 → 286,075,000` | +176.01% | `8,376 → 5,871` | `8 → 13` | `2 → 9` | C |
| Actors | `run_complete` | `75,429,000 → 201,984,000` | +167.78% | `7,276 → 6,237` | `8 → 8` | `4 → 6` | C |
| Actors | `run_cancel` | `288,868,000 → 486,871,000` | +68.54% | `9,635 → 43,539` | `19 → 22` | `12 → 10` | C |
| Actors | `observation_change_trigger_occurrence` | `116,846,000 → 191,298,000` | +63.72% | `8,296 → 8,299` | `14 → 13` | `8 → 8` | C |
| Actors | `observation_change_ingress` | `34,852,000 → 43,232,000` | +24.04% | `6,184 → 6,184` | `5 → 5` | `4 → 4` | C |
| Actors | `observation_fanout_base` | `6,285,000 → 6,775,000` | +7.80% | `1,629 → 1,629` | `2 → 2` | `0 → 0` | C |
| Actors | `observation_fanout_branch_probe` | `13,340,000 → 14,247,000` | +6.80% | `3,587 → 3,587` | `2 → 2` | `0 → 0` | C |
| Actors | `observation_fanout_page` | `4,651,713,000 → 7,859,158,000` | +68.95% | `304,734 → 304,734` | `461 → 461` | `201 → 265` | C |
| Actors | `observation_fanout_wakeup_page` | `5,061,897,000 → 10,472,871,000` | +106.90% | `304,734 → 304,734` | `457 → 462` | `200 → 267` | C |
| Actors | `observation_fanout_coalesced_page` | `1,961,593,000 → 4,301,175,000` | +119.27% | `304,734 → 304,734` | `389 → 327` | `2 → 2` | C |
| Actors | `observation_fanout_terminal` | `228,035,000 → 385,810,000` | +69.19% | `5,736 → 15,173` | `27 → 35` | `25 → 30` | C |
| Actors | `observation_fanout_blocked_page` | `116,219,631,000 → 136,174,555,000` | +17.17% | `304,734 → 304,734` | `591 → 467` | `200 → 267` | C |
| Actors | `crossing_worker_base` | `7,543,000 → 6,984,000` | -7.41% | `1,543 → 1,543` | `2 → 2` | `0 → 0` | C |
| Actors | `crossing_work_probe` | `62,090,000 → 80,668,000` | +29.92% | `11,729 → 15,106` | `12 → 11` | `0 → 0` | C |
| Actors | `observation_crossing_trigger_occurrence` | `508,662,000 → 713,371,000` | +40.24% | `164,204 → 164,188` | `90 → 90` | `81 → 81` | C |
| Actors | `crossing_search_probe` | `125,926,000 → 134,168,000` | +6.55% | `81,886 → 81,886` | `33 → 33` | `0 → 0` | C |
| Actors | `crossing_fire_probe` | `66,770,000 → 89,119,000` | +33.47% | `11,729 → 11,729` | `14 → 13` | `0 → 0` | C |
| Actors | `crossing_fire_pair_probe` | `216,651,000 → 350,888,000` | +61.96% | `11,729 → 15,106` | `25 → 26` | `0 → 0` | C |
| Actors | `crossing_tail_refill_probe` | `20,114,000 → 20,324,000` | +1.04% | `11,729 → 11,729` | `1 → 1` | `0 → 0` | C |
| Actors | `crossing_fire_cohort_preflight` | `41,975,000 + 23,857,544·c → 44,350,000 + 26,177,365·c` | +5.66% | `1,493 + 2,699·c → 1,493 + 2,861·c` | `2 + 7·c → 2 + 6·c` | `0 → 0` | C |
| Actors | `crossing_coalesced_cohort_preflight` | `44,280,000 + 24,157,448·c → 49,029,000 + 38,580,241·c` | +10.72% | `1,493 + 2,699·c → 9,858 + 2,699·c` | `2 + 7·c → 3 + 5·c` | `0 → 0` | C |
| Actors | `crossing_terminal_cohort_preflight` | `43,721,000 + 23,827,571·c → 21,381,679 + 25,141,280·c` | -51.10% | `1,493 + 2,699·c → 1,493 + 2,861·c` | `2 + 7·c → 2 + 6·c` | `0 → 0` | C |
| Actors | `crossing_skip_cohort_preflight` | `40,718,000 + 22,509,580·c → 31,379,277 + 23,988,770·c` | -22.94% | `990 + 2,699·c → 990 + 2,861·c` | `0 + 7·c → 0 + 6·c` | `0 → 0` | C |
| Actors | `crossing_rearm_cohort_preflight` | `40,928,000 + 22,797,266·c → 55,735,000 + 36,930,066·c` | +36.18% | `990 + 2,699·c → 9,858 + 2,699·c` | `0 + 7·c → 1 + 5·c` | `0 → 0` | C |
| Actors | `crossing_rearm_pair_probe` | `93,449,000 → 101,690,000` | +8.82% | `11,729 → 15,106` | `19 → 16` | `0 → 0` | C |
| Actors | `crossing_skip_pair_probe` | `89,188,000 → 89,119,000` | -0.08% | `11,729 → 11,729` | `19 → 17` | `0 → 0` | C |
| Actors | `crossing_transition_unit` | `33,105,000 → 46,306,000` | +39.88% | `6,636 → 6,636` | `5 → 5` | `2 → 2` | C |
| Actors | `crossing_leaf_unit` | `553,082,000 → 741,098,000` | +33.99% | `162,782 → 162,782` | `90 → 90` | `81 → 81` | C |
| Actors | `crossing_page_unit` | `553,710,000 → 713,021,000` | +28.77% | `162,782 → 162,782` | `90 → 90` | `81 → 81` | C |
| Actors | `crossing_rearm_unit` | `420,591,000 → 537,717,000` | +27.85% | `162,782 → 162,782` | `80 → 83` | `74 → 74` | C |
| Actors | `crossing_rearm_pair_unit` | `472,623,000 → 667,764,000` | +41.29% | `162,782 → 162,782` | `86 → 91` | `76 → 75` | C |
| Actors | `crossing_coalesced_unit` | `439,728,000 → 575,362,000` | +30.84% | `162,782 → 162,782` | `84 → 85` | `74 → 74` | C |
| Actors | `crossing_coalesced_pair_unit` | `521,862,000 → 748,851,000` | +43.50% | `162,782 → 162,782` | `92 → 93` | `76 → 75` | C |
| Actors | `crossing_placed_unit` | `555,946,000 → 718,679,000` | +29.27% | `162,782 → 162,782` | `90 → 90` | `81 → 81` | C |
| Actors | `crossing_placed_pair_unit` | `586,537,000 → 794,527,000` | +35.46% | `162,782 → 162,782` | `97 → 98` | `85 → 86` | C |
| Actors | `crossing_placed_maximum_unit` | `9,904,350,000 → 20,066,635,000` | +102.60% | `346,462 → 608,478` | `982 → 1,111` | `466 → 595` | C |
| Actors | `crossing_placed_non_tail_emptied_unit` | `4,094,371,000 → 9,507,020,000` | +132.20% | `327,262 → 327,262` | `563 → 627` | `367 → 431` | C |
| Actors | `crossing_placed_non_tail_trimmed_unit` | `4,067,622,000 → 9,394,853,000` | +130.97% | `327,262 → 327,262` | `563 → 627` | `369 → 433` | C |
| Actors | `crossing_skip_unit` | `168,390,000 → 235,090,000` | +39.61% | `81,886 → 81,886` | `41 → 47` | `2 → 2` | C |
| Actors | `crossing_skip_pair_unit` | `77,665,000 → 195,559,000` | +151.80% | `11,729 → 11,729` | `12 → 24` | `2 → 2` | C |
| Actors | `crossing_actor_unit` | `730,831,000 → 877,570,000` | +20.08% | `162,782 → 162,782` | `97 → 97` | `91 → 90` | C |
| Actors | `transaction_extension_ingress_base` | `13,410,000 → 14,667,000` | +9.37% | `6,052 → 6,052` | `2 → 2` | `0 → 0` | C |
| Actors | `transaction_extension_ingress_notify` | `290,055,000 → 473,182,000` | +63.14% | `5,998 → 20,625` | `13 → 13` | `6 → 2` | C |
| Actors | `funding_snapshot_open` | `13,240,086 + 122,803·a → 15,152,818 + 358,026·a` | +14.45% | `4,531 → 4,531` | `1 → 1` | `1 → 1` | C |
| Actors | `maximum_context_inherent` | `191,559,950,000 → 204,517,874,000` | +6.76% | `3,517 → 3,517` | `13 → 13` | `23 → 23` | C |
| Actors | `block_resource_finalize` | `8,590,000 → 9,010,000` | +4.89% | `1,560 → 1,560` | `1 → 1` | `2 → 2` | C |
| Actors | `block_resource_meter_extension` | `10,616,000 → 11,035,000` | +3.95% | `1,560 → 1,560` | `1 → 1` | `1 → 1` | C |
| Actors | `maximum_xcm_version_discovery` | `446,572,000 → 465,709,000` | +4.29% | `248,490 → 248,490` | `102 → 102` | `1 → 1` | C |
| Router | `direct_xyk_exact_input` | `389,092,000 → 495,601,000` | +27.37% | `9,635 → 15,106` | `31 → 31` | `18 → 18` | C |
| Router | `direct_mint_exact_input` | `390,349,000 → 561,603,000` | +43.87% | `21,862 → 21,862` | `34 → 38` | `14 → 14` | C |
| Router | `native_anchored_exact_input` | `562,930,000 → 646,251,000` | +14.80% | `19,253 → 19,253` | `45 → 45` | `27 → 27` | C |
| Router | `direct_xyk_exact_output` | `387,486,000 → 481,842,000` | +24.35% | `9,635 → 15,106` | `30 → 30` | `18 → 18` | C |
| Router | `native_anchored_exact_output` | `556,714,000 → 656,309,000` | +17.89% | `19,253 → 19,253` | `44 → 44` | `27 → 27` | C |
| Router | `create_pool` | `144,923,000 → 151,628,000` | +4.63% | `34,255 → 34,255` | `13 → 13` | `10 → 10` | C |
| Router | `update_router_fee` | `8,591,000 → 8,661,000` | +0.81% | `1,489 → 1,489` | `1 → 1` | `1 → 1` | C |
| TMC | `create_curve` | `26,680,000 → 26,610,000` | -0.26% | `6,360 → 6,360` | `3 → 3` | `1 → 1` | C |
| TMC | `mint_with_distribution` | `164,758,000 → 254,994,000` | +54.77% | `6,208 → 7,535` | `11 → 15` | `4 → 4` | C |

## Interpretation

Every listed dimension requires review against the owning implementation and benchmark evidence. Positive deltas remain unexplained until the release candidate records their measured reason; this generated comparison does not accept them by itself.

## Retired Weight Owners

- Actors `cycle_orchestration`
- Actors `step_orchestration`
- Actors `scheduler_inner_opening_close_min`
- Actors `scheduler_inner_opening_close_max`
- Actors `run_retry`
- Actors `run_suffix_admission`

Any retired owner requires implementation review before release acceptance; absence from the candidate alone does not prove safe replacement.

## Reproduction

- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`
- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`
- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Architecture Experiments Skill; focused outputs do not replace complete generated pallet files.

Candidate weight source identity: `a55b874f82a52fe176f49d1d86c2531031f98dd09c4e895123140ab5f532939d`.

