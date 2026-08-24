# Runtime Weight Delta Ledger

## Evidence Boundary

This generated ledger compares the production Weight implementations in Git tag `v0.7.22` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.

Candidate release: `0.7.23`. The locally validated production runtime was generated with `./scripts/03-build-runtime.sh`; compact Wasm SHA-256 is `4b04e98b598cb0e72516e12382b742858ba720631f769b60be433d7e1acd989a`. The accepted benchmark owners use `frame-omni-bencher 0.22.0` / CLI `58.0.0`, `50` steps, `20` repeats, compiled Wasm execution, RocksDB, 1,024 MiB cache, host `fedora`, and CPU `AMD Ryzen 7 4800H with Radeon Graphics`; each generated method records date, reads, writes, measured ProofSize, and conservative ProofSize in its authoritative source. The benchmark-runtime Wasm and production Wasm are distinct evidence identities. Exact candidate commit/tree identity remains unavailable until the validated worktree is committed through the authorized release gate.

Interpretation codes classify changed paths only: `I` identity guard; `C` correctness; `P` bounded service topology; `M` merged canonical work; `O` measured optimization.

## Changed Production Paths

| Pallet | Weight method | RefTime: v0.7.22 → 0.7.23 candidate | Base delta | ProofSize: v0.7.22 → 0.7.23 candidate | Reads: v0.7.22 → 0.7.23 candidate | Writes: v0.7.22 → 0.7.23 candidate | Code |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Actors | `create_user_actor` | `193,393,000 → 535,342,000` | +176.82% | `12,200 → 81,886` | `25 → 65` | `19 → 60` | C |
| Actors | `create_user_actor_at_slot` | `145,691,000 → 537,716,000` | +269.08% | `12,200 → 81,886` | `16 → 65` | `10 → 60` | C |
| Actors | `create_system_actor` | `103,087,000 → 595,895,000` | +478.05% | `12,200 → 81,886` | `16 → 63` | `11 → 58` | C |
| Actors | `create_system_actor_at_sovereign_id` | `91,563,000 → 561,113,000` | +512.82% | `12,200 → 81,886` | `14 → 61` | `9 → 56` | C |
| Actors | `create_user_actor_crossing_new_page` | `— → 562,790,000` | new | `— → 53,350` | `— → 33` | `— → 28` | C |
| Actors | `create_dormant_system_actor` | `67,817,000 → 73,334,000` | +8.14% | `12,200 → 5,736` | `12 → 13` | `7 → 7` | C |
| Actors | `activate_actor` | `124,249,000 → 561,114,000` | +351.60% | `12,200 → 81,886` | `19 → 57` | `14 → 52` | C |
| Actors | `deactivate_actor` | `103,018,000 → 594,010,000` | +476.61% | `12,200 → 81,886` | `9 → 57` | `6 → 55` | C |
| Actors | `pause_actor` | `62,020,000 → 75,290,000` | +21.40% | `12,200 → 5,736` | `7 → 7` | `2 → 2` | C |
| Actors | `resume_actor` | `61,112,000 → 71,658,000` | +17.26% | `12,200 → 5,736` | `7 → 7` | `2 → 2` | C |
| Actors | `manual_trigger` | `84,719,000 → 162,733,000` | +92.09% | `12,200 → 9,635` | `12 → 14` | `5 → 7` | C |
| Actors | `address_event_trigger_occurrence` | `— → 178,307,000` | new | `— → 8,367` | `— → 14` | `— → 7` | C |
| Actors | `pipeline_admission_apoptosis` | `— → 147,996,000` | new | `— → 5,736` | `— → 15` | `— → 15` | C |
| Actors | `close_actor` | `210,365,000 → 742,564,000` | +252.99% | `12,200 → 81,886` | `24 → 64` | `23 → 64` | C |
| Actors | `update_contract` | `274,480,000 → 1,055,528,000` | +284.56% | `12,200 → 81,886` | `25 → 67` | `22 → 63` | C |
| Actors | `set_global_circuit_breaker` | `7,124,000 → 7,194,000` | +0.98% | `0 → 0` | `0 → 0` | `1 → 1` | C |
| Actors | `record_crossing_worker_fault` | `— → 11,454,000` | new | `— → 1,529` | `— → 1` | `— → 1` | C |
| Actors | `record_observation_fanout_worker_fault` | `— → 22,000,000` | new | `— → 4,106` | `— → 3` | `— → 1` | C |
| Actors | `record_wakeup_worker_fault` | `— → 10,337,000` | new | `— → 1,503` | `— → 1` | `— → 1` | C |
| Actors | `clear_crossing_worker_fault` | `— → 14,388,000` | new | `— → 1,529` | `— → 1` | `— → 1` | C |
| Actors | `clear_observation_fanout_worker_fault` | `— → 14,178,000` | new | `— → 1,629` | `— → 1` | `— → 1` | C |
| Actors | `clear_wakeup_worker_fault` | `— → 12,781,000` | new | `— → 1,503` | `— → 1` | `— → 1` | C |
| Actors | `set_active_actor_limit` | `9,289,000 → 10,127,000` | +9.02% | `1,489 → 1,489` | `2 → 2` | `0 → 0` | C |
| Actors | `permissionless_sweep` | `44,350,000 → 50,776,000` | +14.49% | `12,200 → 5,736` | `7 → 7` | `0 → 0` | C |
| Actors | `permissionless_sweep_many` | `23,891,147 + 66,286,454·n → 24,823,652 + 141,011,511·n` | +3.90% | `1,489 + 11,210·n → 1,489 + 4,746·n` | `3 + 11·n → 3 + 14·n` | `2 + 7·n → 2 + 14·n` | C |
| Actors | `fee_collection` | `43,791,000 → 44,070,000` | +0.64% | `3,593 → 3,593` | `1 → 1` | `1 → 1` | O |
| Actors | `task_transfer` | `232,854,000 → 350,609,000` | +50.57% | `12,200 → 18,280` | `18 → 21` | `8 → 8` | C |
| Actors | `task_burn` | `19,067,000 → 19,277,000` | +1.10% | `3,593 → 3,593` | `1 → 1` | `1 → 1` | C |
| Actors | `task_mint` | `197,794,000 → 298,576,000` | +50.95% | `12,200 → 18,280` | `16 → 18` | `6 → 6` | C |
| Actors | `predicate_set_evaluation` | `7,612,000 + 6,444,212·c → 7,612,000 + 6,549,402·c` | 0.00% | `3,675 + 674·c → 3,675 + 674·c` | `1 + 1·c → 1 + 1·c` | `0 → 0` | C |
| Actors | `task_stop_cycle` | `4,260,000 → 4,261,000` | +0.02% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `task_split_transfer` | `80,834,694 + 154,149,994·l → 159,307,195 + 200,071,385·l` | +97.08% | `8,040 + 11,210·l → 18,280 + 4,746·l` | `10 + 7·l → 11 + 9·l` | `4 + 3·l → 4 + 3·l` | C |
| Actors | `xcm_asset_deposit` | `218,117,000 → 335,314,000` | +53.73% | `12,200 → 18,280` | `19 → 21` | `8 → 8` | C |
| Actors | `task_add_liquidity` | `271,337,000 → 273,224,000` | +0.70% | `34,255 → 34,255` | `16 → 16` | `15 → 15` | C |
| Actors | `task_donate_liquidity` | `164,618,000 → 165,317,000` | +0.42% | `14,035 → 14,035` | `9 → 9` | `8 → 8` | C |
| Actors | `task_remove_liquidity` | `155,050,000 → 158,472,000` | +2.21% | `8,817 → 8,817` | `8 → 8` | `6 → 6` | C |
| Actors | `task_stake` | `85,906,000 → 87,722,000` | +2.11% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_unstake` | `101,691,000 → 102,389,000` | +0.69% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_dex_exact_in` | `509,640,000 → 577,736,000` | +13.36% | `19,253 → 19,253` | `40 → 41` | `17 → 17` | O |
| Actors | `task_dex_exact_out` | `502,865,000 → 563,419,000` | +12.04% | `19,253 → 19,253` | `39 → 40` | `17 → 17` | O |
| Actors | `scheduler_on_initialize_cutoff` | `— → 10,895,000` | new | `— → 1,560` | `— → 2` | `— → 2` | C |
| Actors | `scheduler_on_idle_base` | `14,737,000 → 17,042,000` | +15.64% | `1,543 → 1,560` | `6 → 7` | `1 → 2` | C |
| Actors | `materialization_coordinator_base` | `— → 26,889,000` | new | `— → 5,982` | `— → 10` | `— → 1` | C |
| Actors | `contract_geometry_create` | `— → 30,032,003 + 5,254,905·c` | new | `— → 4,494 + 2,475·c` | `— → 3 + 1·c` | `— → 2 + 1·c` | C |
| Actors | `contract_geometry_close` | `— → 28,059,784 + 7,044,916·c` | new | `— → 4,558 + 2,669·c` | `— → 2 + 1·c` | `— → 3 + 1·c` | C |
| Actors | `contract_geometry_reconstruct` | `— → 23,780,966 + 5,510,951·c` | new | `— → 4,557 + 2,670·c` | `— → 2 + 1·c` | `— → 0` | C |
| Actors | `current_step_load_head` | `— → 20,952,000` | new | `— → 4,513` | `— → 2` | `— → 0` | C |
| Actors | `current_step_load_tail` | `— → 26,725,030 + 109,947·s` | new | `— → 4,729 + 14·s` | `— → 3` | `— → 0` | C |
| Actors | `current_step_plan_opening_head` | `— → 44,280,000` | new | `— → 5,223` | `— → 6` | `— → 0` | C |
| Actors | `current_step_plan_suspended_head` | `— → 71,868,000` | new | `— → 8,058` | `— → 7` | `— → 0` | C |
| Actors | `current_step_plan_running_tail` | `— → 77,337,414` | new | `— → 8,238 + 14·s` | `— → 8` | `— → 0` | C |
| Actors | `opening_snapshot_capture` | `— → 2,119,881 + 9,168,258·e` | new | `— → 4,373 + 2,260·e` | `— → 1 + 2·e` | `— → 0` | C |
| Actors | `opening_predicate_capture` | `— → 2,893,842 + 7,596,115·p` | new | `— → 4,235 + 2,525·p` | `— → 1 + 2·p` | `— → 0` | C |
| Actors | `scheduler_actor_state_probe` | `37,715,000 → 69,213,000` | +83.52% | `12,200 → 5,998` | `5 → 7` | `0 → 0` | M |
| Actors | `cycle_orchestration` | `50,426,000 → 54,686,000` | +8.45% | `12,200 → 5,736` | `5 → 5` | `3 → 3` | C |
| Actors | `step_orchestration` | `49,961,778 + 174,432·n → 55,114,428 + 53,460·n` | +10.31% | `12,200 → 5,736` | `5 → 5` | `3 → 3` | C |
| Actors | `scheduler_paged_append_existing_page` | `75,639,000 → 122,783,000` | +62.33% | `7,938 → 14,048` | `10 → 11` | `5 → 5` | C |
| Actors | `scheduler_paged_append_new_page` | `73,335,000 → 117,265,000` | +59.90% | `10,283 → 16,435` | `11 → 12` | `5 → 5` | C |
| Actors | `scheduler_wakeup_append_existing_page` | `78,642,000 → 89,119,000` | +13.32% | `6,694 → 7,566` | `8 → 9` | `3 → 3` | C |
| Actors | `scheduler_wakeup_append_new_page` | `81,995,000 → 104,065,000` | +26.92% | `6,833 → 7,739` | `8 → 9` | `4 → 4` | C |
| Actors | `scheduler_wakeup_replace_exact` | `99,455,000 → 105,811,000` | +6.39% | `7,492 → 8,001` | `10 → 11` | `7 → 7` | C |
| Actors | `scheduler_wakeup_invalidate_middle_page` | `90,097,000 → 127,812,000` | +41.86% | `12,495 → 13,538` | `10 → 11` | `5 → 5` | C |
| Actors | `scheduler_wakeup_drain_partial_page` | `354,451,000 → 460,611,000` | +29.95% | `47,750 → 53,971` | `83 → 99` | `18 → 18` | C |
| Actors | `scheduler_wakeup_drain_full_page` | `648,137,000 → 838,458,000` | +29.36% | `90,034 → 101,637` | `164 → 196` | `36 → 36` | C |
| Actors | `scheduler_wakeup_drain_dense_boundary` | `673,421,000 → 884,763,000` | +31.38% | `92,825 → 104,784` | `170 → 203` | `38 → 38` | C |
| Actors | `scheduler_wakeup_drain_stale_page` | `569,704,000 → 771,689,000` | +35.45% | `89,426 → 101,029` | `164 → 196` | `4 → 4` | C |
| Actors | `scheduler_wakeup_cursor_insert` | `346,837,000 → 350,120,000` | +0.95% | `42,733 → 42,706` | `25 → 25` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_pop_min` | `452,230,000 → 455,372,000` | +0.69% | `55,259 → 55,232` | `34 → 34` | `26 → 26` | C |
| Actors | `scheduler_wakeup_cursor_remove_exact` | `422,267,000 → 439,518,000` | +4.09% | `54,726 → 54,699` | `33 → 33` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_worker_partial` | `112,517,000 → 129,837,000` | +15.39% | `7,498 → 7,976` | `14 → 16` | `8 → 8` | C |
| Actors | `at_time_trigger_occurrence` | `— → 213,647,000` | new | `— → 8,552` | `— → 20` | `— → 9` | C |
| Actors | `cadenced_trigger_occurrence` | `— → 243,471,000` | new | `— → 8,505` | `— → 22` | `— → 11` | C |
| Actors | `scheduler_wakeup_cursor_worker_remove` | `548,262,000 → 583,324,000` | +6.40% | `56,434 → 56,912` | `48 → 50` | `33 → 33` | C |
| Actors | `scheduler_wakeup_cursor_worker_future` | `20,184,000 → 25,422,000` | +25.95% | `6,523 → 6,608` | `5 → 6` | `0 → 0` | C |
| Actors | `scheduler_paged_consume_preserve_page` | `48,540,000 → 58,179,000` | +19.86% | `4,868 → 5,528` | `9 → 10` | `3 → 3` | C |
| Actors | `scheduler_paged_consume_delete_page` | `50,007,000 → 60,553,000` | +21.09% | `4,846 → 5,428` | `9 → 10` | `5 → 5` | C |
| Actors | `scheduler_paged_tombstone_drain` | `43,931,000 + 9,481,801·n → 34,433,000 + 4,050,747·n` | -21.62% | `4,049 + 2,492·n → 3,778 + 2,572·n` | `5 + 5·n → 5 + 2·n` | `4 → 4` | C |
| Actors | `scheduler_paged_mixed_scan` | `47,004,000 + 40,911,571·n → 37,785,000 + 49,051,039·n` | -19.61% | `4,857 + 2,608·n → 5,121 + 2,866·n` | `5 + 5·n → 3 + 4·n` | `3 + 1·n → 3 + 1·n` | C |
| Actors | `scheduler_inner_zero_step_complete` | `— → 65,302,000` | new | `— → 5,537` | `— → 7` | `— → 3` | C |
| Actors | `scheduler_paged_execute_opening_max` | `— → 602,530,000` | new | `— → 27,824` | `— → 30` | `— → 15` | C |
| Actors | `scheduler_inner_opening_close_min` | `— → 166,255,626 + 14,639,590·t` | new | `— → 6,975 + 2,670·t` | `— → 14 + 1·t` | `— → 13 + 1·t` | C |
| Actors | `scheduler_inner_opening_failed_min` | `— → 90,667,173 + 6,290,776·t` | new | `— → 6,729 + 2,670·t` | `— → 6 + 1·t` | `— → 3` | C |
| Actors | `scheduler_inner_opening_retry_min` | `— → 144,484,373 + 6,879,240·t` | new | `— → 6,780 + 2,669·t` | `— → 11 + 1·t` | `— → 8` | C |
| Actors | `scheduler_inner_opening_failed_max` | `— → 134,958,006 + 133,022,701·t` | new | `— → 8,347 + 22,129·t` | `— → 9 + 17·t` | `— → 3` | C |
| Actors | `scheduler_inner_opening_retry_max` | `— → 175,843,602 + 135,539,780·t` | new | `— → 8,179 + 22,125·t` | `— → 13 + 17·t` | `— → 8` | C |
| Actors | `scheduler_inner_opening_complete_min` | `— → 90,437,834 + 6,413,765·t` | new | `— → 6,694 + 2,670·t` | `— → 6 + 1·t` | `— → 3` | C |
| Actors | `scheduler_inner_opening_progress_min` | `— → 160,687,415 + 6,143,902·t` | new | `— → 7,045 + 2,670·t` | `— → 13 + 1·t` | `— → 8` | C |
| Actors | `scheduler_inner_opening_close_max` | `— → 222,786,976 + 158,797,600·t` | new | `— → 8,593 + 22,129·t` | `— → 17 + 17·t` | `— → 13 + 1·t` | C |
| Actors | `scheduler_inner_opening_complete_max` | `— → 129,491,462 + 133,519,432·t` | new | `— → 8,312 + 22,129·t` | `— → 9 + 17·t` | `— → 3` | C |
| Actors | `scheduler_inner_opening_progress_max` | `— → 207,036,514 + 137,351,321·t` | new | `— → 8,640 + 22,090·t` | `— → 15 + 17·t` | `— → 8` | C |
| Actors | `scheduler_inner_running_complete` | `— → 101,569,308 + 2,184,815·s + 10,818,953·p` | new | `— → 7,016 + 2,718·p + 49·s` | `— → 9 + 2·p` | `— → 4` | C |
| Actors | `scheduler_inner_running_progress` | `— → 122,324,329 + 1,383,909·s + 9,153,353·p` | new | `— → 6,762 + 2,693·p + 20·s` | `— → 13 + 2·p` | `— → 6` | C |
| Actors | `scheduler_inner_suspended_tail_retry` | `— → 130,930,125 + 1,548,440·s + 9,050,714·p` | new | `— → 6,244 + 2,693·p + 12·s` | `— → 9 + 2·p` | `— → 6` | C |
| Actors | `scheduler_inner_suspended_tail_complete` | `— → 140,693,328 + 1,409,965·s + 10,195,287·p` | new | `— → 7,294 + 2,718·p + 26·s` | `— → 10 + 2·p` | `— → 4` | C |
| Actors | `scheduler_inner_suspended_tail_progress` | `— → 147,572,410 + 2,290,885·s + 9,300,962·p` | new | `— → 6,764 + 2,693·p + 20·s` | `— → 13 + 2·p` | `— → 6` | C |
| Actors | `scheduler_inner_suspended_head_retry` | `— → 104,441,245 + 629,188·n + 77,615·r + 606,658·f + 8,633,349·p` | new | `— → 5,133 + 21·f + 22·n + 2,693·p + 2·r` | `— → 9 + 2·p` | `— → 6` | C |
| Actors | `scheduler_inner_suspended_head_complete` | `— → 106,659,615 + 563,608·n + 56,231·r + 653,361·f + 8,866,318·p` | new | `— → 6,038 + 21·f + 22·n + 2,716·p + 2·r` | `— → 9 + 2·p` | `— → 4` | C |
| Actors | `scheduler_inner_suspended_head_progress` | `— → 118,701,874 + 788,283·n + 89,582·r + 711,606·f + 8,289,022·p` | new | `— → 5,721 + 21·f + 22·n + 2,693·p + 2·r` | `— → 12 + 2·p` | `— → 6` | C |
| Actors | `scheduler_inner_suspended_head_opening_retry` | `— → 124,942,472 + 757,648·n + 123,257·r + 603,701·f` | new | `— → 11,197 + 21·f + 22·n + 2·r` | `— → 15` | `— → 6` | C |
| Actors | `scheduler_inner_suspended_head_opening_complete` | `— → 141,660,101 + 560,255·n + 45,927·r + 527,155·f` | new | `— → 11,985 + 21·f + 22·n + 2·r` | `— → 14` | `— → 4` | C |
| Actors | `scheduler_inner_suspended_head_opening_progress` | `— → 140,648,325 + 798,378·n + 102,327·r + 840,487·f` | new | `— → 11,578 + 21·f + 22·n + 2·r` | `— → 17` | `— → 6` | C |
| Actors | `scheduler_paged_execute_cheap` | `116,986,000 + 94,182,240·n → 136,961,000 + 126,316,005·n` | +17.07% | `3,716 + 2,733·n → 4,210 + 3,171·n` | `6 + 5·n → 6 + 7·n` | `4 + 3·n → 4 + 3·n` | C |
| Actors | `scheduler_paged_execute_cheap_mixed` | `244,449,000 + 120,287,892·n → 368,768,000 + 192,451,310·n` | +50.86% | `4,918 + 2,798·n → 4,772 + 3,361·n` | `6 + 6·n → 5 + 11·n` | `4 + 4·n → 4 + 5·n` | C |
| Actors | `run_progress` | `— → 106,160,000` | new | `— → 8,567` | `— → 9` | `— → 2` | C |
| Actors | `run_suspend` | `— → 104,554,000` | new | `— → 8,376` | `— → 8` | `— → 2` | C |
| Actors | `run_retry` | `— → 53,639,000` | new | `— → 5,589` | `— → 2` | `— → 1` | C |
| Actors | `run_complete` | `— → 77,246,000` | new | `— → 7,276` | `— → 8` | `— → 4` | C |
| Actors | `run_cancel` | `— → 301,510,000` | new | `— → 9,635` | `— → 19` | `— → 12` | C |
| Actors | `run_suffix_admission` | `— → 1,433,649 + 596·n` | new | `— → 0` | `— → 0` | `— → 0` | C |
| Actors | `observation_change_trigger_occurrence` | `— → 120,199,000` | new | `— → 8,296` | `— → 14` | `— → 8` | C |
| Actors | `observation_change_ingress` | `33,384,000 → 36,178,000` | +8.37% | `6,128 → 6,184` | `5 → 5` | `4 → 4` | C |
| Actors | `observation_fanout_base` | `6,146,000 → 6,565,000` | +6.82% | `1,543 → 1,629` | `1 → 2` | `0 → 0` | C |
| Actors | `observation_fanout_branch_probe` | `— → 13,759,000` | new | `— → 3,587` | `— → 2` | `— → 0` | C |
| Actors | `observation_fanout_page` | `2,644,513,000 → 4,738,388,000` | +79.18% | `718,430 → 304,734` | `332 → 461` | `72 → 201` | C |
| Actors | `observation_fanout_wakeup_page` | `— → 5,132,089,000` | new | `— → 304,734` | `— → 457` | `— → 200` | C |
| Actors | `observation_fanout_coalesced_page` | `— → 2,008,178,000` | new | `— → 304,734` | `— → 389` | `— → 2` | C |
| Actors | `observation_fanout_terminal` | `— → 239,000,000` | new | `— → 5,736` | `— → 27` | `— → 25` | C |
| Actors | `observation_fanout_blocked_page` | `— → 121,065,020,000` | new | `— → 304,734` | `— → 591` | `— → 200` | C |
| Actors | `crossing_worker_base` | `6,146,000 → 7,543,000` | +22.73% | `1,543 → 1,543` | `1 → 2` | `0 → 0` | C |
| Actors | `crossing_work_probe` | `— → 65,373,000` | new | `— → 11,729` | `— → 12` | `— → 0` | C |
| Actors | `observation_crossing_trigger_occurrence` | `— → 530,453,000` | new | `— → 164,204` | `— → 90` | `— → 81` | C |
| Actors | `crossing_search_probe` | `— → 130,256,000` | new | `— → 81,886` | `— → 33` | `— → 0` | C |
| Actors | `crossing_fire_probe` | `— → 70,052,000` | new | `— → 11,729` | `— → 14` | `— → 0` | C |
| Actors | `crossing_fire_pair_probe` | `— → 219,095,000` | new | `— → 11,729` | `— → 25` | `— → 0` | C |
| Actors | `crossing_tail_refill_probe` | `— → 21,162,000` | new | `— → 11,729` | `— → 1` | `— → 0` | C |
| Actors | `crossing_fire_cohort_preflight` | `— → 43,303,000 + 25,902,326·c` | new | `— → 1,493 + 2,699·c` | `— → 2 + 7·c` | `— → 0` | C |
| Actors | `crossing_coalesced_cohort_preflight` | `— → 47,283,000 + 25,665,150·c` | new | `— → 1,493 + 2,699·c` | `— → 2 + 7·c` | `— → 0` | C |
| Actors | `crossing_terminal_cohort_preflight` | `— → 2,884,470 + 25,208,667·c` | new | `— → 1,493 + 2,699·c` | `— → 2 + 7·c` | `— → 0` | C |
| Actors | `crossing_skip_cohort_preflight` | `— → 39,531,000 + 23,675,640·c` | new | `— → 990 + 2,699·c` | `— → 0 + 7·c` | `— → 0` | C |
| Actors | `crossing_rearm_cohort_preflight` | `— → 41,836,000 + 23,957,644·c` | new | `— → 990 + 2,699·c` | `— → 0 + 7·c` | `— → 0` | C |
| Actors | `crossing_rearm_pair_probe` | `— → 96,523,000` | new | `— → 11,729` | `— → 19` | `— → 0` | C |
| Actors | `crossing_skip_pair_probe` | `— → 92,960,000` | new | `— → 11,729` | `— → 19` | `— → 0` | C |
| Actors | `crossing_transition_unit` | `31,988,000 → 34,781,000` | +8.73% | `6,060 → 6,636` | `5 → 5` | `2 → 2` | C |
| Actors | `crossing_leaf_unit` | `425,689,000 → 583,394,000` | +37.05% | `162,782 → 162,782` | `85 → 90` | `77 → 81` | C |
| Actors | `crossing_page_unit` | `426,457,000 → 580,251,000` | +36.06% | `162,782 → 162,782` | `85 → 90` | `77 → 81` | C |
| Actors | `crossing_rearm_unit` | `— → 430,229,000` | new | `— → 162,782` | `— → 80` | `— → 74` | C |
| Actors | `crossing_rearm_pair_unit` | `— → 483,729,000` | new | `— → 162,782` | `— → 86` | `— → 76` | C |
| Actors | `crossing_coalesced_unit` | `— → 454,674,000` | new | `— → 162,782` | `— → 84` | `— → 74` | C |
| Actors | `crossing_coalesced_pair_unit` | `— → 546,238,000` | new | `— → 162,782` | `— → 92` | `— → 76` | C |
| Actors | `crossing_placed_unit` | `— → 574,663,000` | new | `— → 162,782` | `— → 90` | `— → 81` | C |
| Actors | `crossing_placed_pair_unit` | `— → 611,261,000` | new | `— → 162,782` | `— → 97` | `— → 85` | C |
| Actors | `crossing_placed_maximum_unit` | `— → 10,442,486,000` | new | `— → 346,462` | `— → 982` | `— → 466` | C |
| Actors | `crossing_placed_non_tail_emptied_unit` | `— → 4,321,498,000` | new | `— → 327,262` | `— → 563` | `— → 367` | C |
| Actors | `crossing_placed_non_tail_trimmed_unit` | `— → 4,300,965,000` | new | `— → 327,262` | `— → 563` | `— → 369` | C |
| Actors | `crossing_skip_unit` | `— → 175,584,000` | new | `— → 81,886` | `— → 41` | `— → 2` | C |
| Actors | `crossing_skip_pair_unit` | `— → 80,388,000` | new | `— → 11,729` | `— → 12` | `— → 2` | C |
| Actors | `crossing_actor_unit` | `594,359,000 → 757,091,000` | +27.38% | `162,782 → 162,782` | `91 → 97` | `83 → 91` | C |
| Actors | `transaction_extension_ingress_base` | `13,479,000 → 13,759,000` | +2.08% | `6,052 → 6,052` | `2 → 2` | `0 → 0` | C |
| Actors | `transaction_extension_ingress_notify` | `159,939,000 → 303,955,000` | +90.04% | `12,200 → 5,998` | `10 → 13` | `6 → 6` | C |
| Actors | `funding_snapshot_open` | `13,339,290 + 125,929·a → 13,311,775 + 127,773·a` | -0.21% | `3,751 → 4,531` | `1 → 1` | `1 → 1` | C |
| Actors | `maximum_context_inherent` | `— → 196,901,501,000` | new | `— → 3,517` | `— → 13` | `— → 23` | C |
| Actors | `block_resource_finalize` | `— → 8,590,000` | new | `— → 1,560` | `— → 1` | `— → 2` | C |
| Actors | `block_resource_meter_extension` | `— → 10,826,000` | new | `— → 1,560` | `— → 1` | `— → 1` | C |
| Actors | `maximum_xcm_version_discovery` | `— → 456,839,000` | new | `— → 248,490` | `— → 102` | `— → 1` | C |
| Oracle | `register_feed_existing_producer` | `132,142,000 → 146,111,000` | +10.57% | `20,532 → 20,532` | `3 → 3` | `3 → 3` | C |
| Oracle | `register_feed_new_producer` | `194,720,000 → 215,185,000` | +10.51% | `44,394 → 44,394` | `4 → 4` | `4 → 4` | C |
| Oracle | `pause_feed` | `15,365,000 → 15,366,000` | +0.01% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `resume_feed` | `15,365,000 → 15,435,000` | +0.46% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `publish_last_value` | `30,033,000 → 33,733,000` | +12.32% | `3,559 → 3,559` | `5 → 6` | `1 → 1` | C |
| Oracle | `publish_ema_changed` | `33,385,000 → 37,156,000` | +11.30% | `3,559 → 3,559` | `5 → 6` | `1 → 1` | C |
| Oracle | `publish_ema_changed_primary_first` | `— → 46,795,000` | new | `— → 3,559` | `— → 7` | `— → 4` | C |
| Oracle | `publish_ema_changed_primary_existing` | `— → 46,864,000` | new | `— → 3,559` | `— → 6` | `— → 3` | C |
| Oracle | `publish_ema_changed_secondary_first` | `— → 49,239,000` | new | `— → 6,060` | `— → 9` | `— → 4` | C |
| Oracle | `publish_ema_changed_secondary_existing` | `— → 49,099,000` | new | `— → 6,060` | `— → 9` | `— → 4` | C |
| Oracle | `publish_ema_changed_combined` | `— → 58,667,000` | new | `— → 6,060` | `— → 10` | `— → 7` | C |
| Oracle | `publish_ema_changed_secondary_capacity` | `— → 45,188,000` | new | `— → 6,060` | `— → 8` | `— → 0` | C |
| Oracle | `publish_ema_refresh` | `21,931,000 → 22,280,000` | +1.59% | `3,551 → 3,551` | `2 → 2` | `1 → 1` | C |
| Router | `direct_xyk_exact_input` | `294,176,000 → 389,092,000` | +32.27% | `12,200 → 9,635` | `25 → 31` | `12 → 18` | C |
| Router | `direct_mint_exact_input` | `315,129,000 → 390,349,000` | +23.87% | `23,410 → 21,862` | `33 → 34` | `14 → 14` | C |
| Router | `native_anchored_exact_input` | `435,468,000 → 562,930,000` | +29.27% | `19,253 → 19,253` | `36 → 45` | `17 → 27` | C |
| Router | `direct_xyk_exact_output` | `164,898,000 → 387,486,000` | +134.99% | `6,208 → 9,635` | `10 → 30` | `5 → 18` | C |
| Router | `native_anchored_exact_output` | `302,348,000 → 556,714,000` | +84.13% | `16,644 → 19,253` | `21 → 44` | `10 → 27` | C |
| Router | `create_pool` | `— → 144,923,000` | new | `— → 34,255` | `— → 13` | `— → 10` | C |
| TMC | `mint_with_distribution` | `164,130,000 → 164,758,000` | +0.38% | `12,200 → 6,208` | `11 → 11` | `4 → 4` | C |

## Interpretation

Every listed dimension requires review against the owning implementation and benchmark evidence. Positive deltas remain unexplained until the release candidate records their measured reason; this generated comparison does not accept them by itself.

## Retired Weight Owners

- Actors `continuation_suspend`
- Actors `continuation_retry`
- Actors `continuation_complete`
- Actors `continuation_cancel`
- Actors `continuation_suffix_admission`

Any retired owner requires implementation review before release acceptance; absence from the candidate alone does not prove safe replacement.

## Reproduction

- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`
- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`
- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Architecture Experiments Skill; focused outputs do not replace complete generated pallet files.

Candidate weight source identity: `e3615c4896619eabba068ba980988810f7c2ec6ebaa893f3539a9919a1bfa3c9`.

