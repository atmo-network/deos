# Runtime Weight Delta Ledger

## Evidence Boundary

This generated ledger compares the production Weight implementations in Git tag `v0.7.26` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.

Candidate release: `0.7.26`. The candidate production runtime was built with `./scripts/03-build-runtime.sh`; compact Wasm SHA-256 is `91c23b2f77b57265e8e32ec8645610a32b7d4763b0ced31ab56ef64e8c0f5a2d`. The accepted benchmark owners use `frame-omni-bencher 0.22.0` / CLI `58.0.0`, `50` steps, `20` repeats, compiled Wasm execution, RocksDB, 1,024 MiB cache, host `fedora`, and CPU `AMD Ryzen 7 4800H with Radeon Graphics`; each generated method records date, reads, writes, measured ProofSize, and conservative ProofSize in its authoritative source. The benchmark-runtime Wasm and production Wasm are distinct evidence identities. Exact candidate commit/tree identity remains unavailable until the validated worktree is committed through the authorized release gate.

Interpretation codes classify changed paths only: `I` identity guard; `C` correctness; `P` bounded service topology; `M` merged canonical work; `O` measured optimization.

## Changed Production Paths

| Pallet | Weight method | RefTime: v0.7.26 → 0.7.26 candidate | Base delta | ProofSize: v0.7.26 → 0.7.26 candidate | Reads: v0.7.26 → 0.7.26 candidate | Writes: v0.7.26 → 0.7.26 candidate | Code |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Actors | `create_user_actor` | `563,908,000 → 479,817,000` | -14.91% | `81,886 → 16,032` | `63 → 24` | `55 → 15` | C |
| Actors | `create_user_actor_at_slot` | `604,277,000 → 476,814,000` | -21.09% | `81,886 → 16,032` | `63 → 24` | `55 → 15` | C |
| Actors | `create_system_actor` | `594,079,000 → 378,825,000` | -36.23% | `81,886 → 16,032` | `61 → 23` | `53 → 14` | C |
| Actors | `create_system_actor_at_sovereign_id` | `743,612,000 → 339,015,000` | -54.41% | `81,886 → 16,032` | `59 → 21` | `51 → 12` | C |
| Actors | `create_dormant_system_actor` | `139,895,000 → 83,881,000` | -40.04% | `7,535 → 4,990` | `17 → 15` | `7 → 8` | C |
| Actors | `activate_actor` | `878,618,000 → 341,041,000` | -61.18% | `81,886 → 16,032` | `55 → 17` | `47 → 8` | C |
| Actors | `deactivate_actor` | `1,255,487,000 → 460,191,000` | -63.35% | `81,886 → 16,032` | `58 → 24` | `57 → 15` | C |
| Actors | `pause_actor` | `140,663,000 → 171,952,000` | +22.24% | `5,736 → 6,058` | `8 → 12` | `1 → 2` | C |
| Actors | `resume_actor` | `123,482,000 → 180,822,000` | +46.44% | `5,736 → 6,058` | `8 → 12` | `2 → 2` | C |
| Actors | `manual_trigger` | `243,890,000 → 609,165,000` | +149.77% | `15,106 → 82,014` | `14 → 72` | `7 → 25` | C |
| Actors | `address_event_trigger_occurrence` | `292,779,000 → 872,472,000` | +198.00% | `8,394 → 88,739` | `14 → 72` | `7 → 25` | C |
| Actors | `close_actor` | `973,883,000 → 756,393,000` | -22.33% | `81,886 → 16,032` | `66 → 29` | `65 → 20` | C |
| Actors | `close_actor_policy_bearing` | `— → 931,069,000` | new | `— → 16,032` | `— → 32` | `— → 24` | C |
| Actors | `behavior_service_inspection` | `— → 81,995,000` | new | `— → 5,461` | `— → 4` | `— → 0` | C |
| Actors | `update_contract` | `1,352,289,000 → 846,141,000` | -37.43% | `162,782 → 16,032` | `101 → 24` | `94 → 15` | C |
| Actors | `set_global_circuit_breaker` | `7,683,000 → 7,264,000` | -5.45% | `0 → 0` | `0 → 0` | `1 → 1` | C |
| Actors | `dependency_scan_source_probe` | `— → 6,635,000` | new | `— → 1,498` | `— → 1` | `— → 0` | C |
| Actors | `process_dependency_scan_unit` | `— → 75,918,000` | new | `— → 4,570` | `— → 11` | `— → 6` | C |
| Actors | `process_dependency_scan_completion_unit` | `— → 23,048,000` | new | `— → 3,523` | `— → 3` | `— → 3` | C |
| Actors | `complete_cycle_to_parked_balance` | `— → 211,762,000` | new | `— → 4,990` | `— → 28` | `— → 20` | C |
| Actors | `process_pending_parked_balance_event` | `— → 712,183,000` | new | `— → 43,950` | `— → 92` | `— → 74` | C |
| Actors | `process_due_parked_balance_review` | `— → 878,617,000` | new | `— → 43,950` | `— → 122` | `— → 13` | C |
| Actors | `process_due_parked_balance_review_deep_index` | `— → 2,379,531,000` | new | `— → 66,094` | `— → 164` | `— → 53` | C |
| Actors | `classify_due_block_deadline` | `— → 30,591,000` | new | `— → 4,096` | `— → 4` | `— → 0` | C |
| Actors | `classify_due_tick_deadline` | `— → 169,368,000` | new | `— → 4,096` | `— → 5` | `— → 0` | C |
| Actors | `deadline_destination_search` | `— → 19,846,251 + 29,277·p` | new | `— → 5,074 + 2·p` | `— → 2` | `— → 0` | C |
| Actors | `return_due_block_deadline_to_service_deep_index` | `— → 988,270,000` | new | `— → 51,480` | `— → 44` | `— → 32` | C |
| Actors | `return_due_block_deadline_to_service` | `— → 150,022,000` | new | `— → 13,414` | `— → 16` | `— → 9` | C |
| Actors | `set_active_actor_limit` | `11,454,000 → 11,384,000` | -0.61% | `1,489 → 1,489` | `2 → 2` | `0 → 0` | C |
| Actors | `permissionless_sweep` | `89,608,000 → 79,131,000` | -11.69% | `5,736 → 6,058` | `8 → 11` | `0 → 0` | C |
| Actors | `permissionless_sweep_many` | `86,378,643 + 173,665,591·n → 8,310,952 + 392,697,762·n` | -90.38% | `1,489 + 4,746·n → 1,489 + 4,000·n` | `3 + 15·n → 3 + 16·n` | `2 + 13·n → 2 + 9·n` | C |
| Actors | `fee_collection` | `50,007,000 → 49,169,000` | -1.68% | `6,196 → 6,196` | `2 → 2` | `2 → 2` | O |
| Actors | `action_invocation_receipt` | `4,680,000 → 4,330,000` | -7.48% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `task_transfer` | `468,433,000 → 388,533,000` | -17.06% | `29,222 → 6,196` | `25 → 19` | `12 → 6` | C |
| Actors | `task_burn` | `20,045,000 → 28,147,000` | +40.42% | `3,593 → 3,593` | `1 → 2` | `1 → 1` | C |
| Actors | `task_mint` | `343,415,000 → 345,860,000` | +0.71% | `29,222 → 6,058` | `22 → 17` | `10 → 5` | C |
| Actors | `predicate_set_evaluation` | `6,906,425 + 11,187,530·c → 6,946,281 + 11,153,385·c` | +0.58% | `3,839 + 1,740·c → 3,840 + 1,690·c` | `0 + 2·c → 0 + 2·c` | `0 → 0` | C |
| Actors | `task_stop_cycle` | `6,705,000 → 4,261,000` | -36.45% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `xcm_asset_deposit` | `458,445,000 → 383,295,000` | -16.39% | `29,222 → 6,058` | `25 → 19` | `12 → 7` | C |
| Actors | `task_add_liquidity` | `316,317,000 → 349,421,000` | +10.47% | `34,255 → 34,255` | `16 → 18` | `15 → 16` | C |
| Actors | `task_donate_liquidity` | `181,242,000 → 179,565,000` | -0.93% | `14,035 → 14,035` | `9 → 9` | `8 → 8` | C |
| Actors | `task_remove_liquidity` | `164,758,000 → 175,793,000` | +6.70% | `8,817 → 8,817` | `8 → 9` | `6 → 7` | C |
| Actors | `task_stake` | `91,005,000 → 97,361,000` | +6.98% | `8,817 → 8,817` | `7 → 8` | `7 → 8` | C |
| Actors | `task_unstake` | `112,935,000 → 114,472,000` | +1.36% | `8,817 → 8,817` | `7 → 8` | `7 → 8` | C |
| Actors | `task_dex_exact_in` | `718,678,000 → 742,634,000` | +3.33% | `19,253 → 19,253` | `41 → 39` | `17 → 17` | O |
| Actors | `task_dex_exact_out` | `714,697,000 → 773,784,000` | +8.27% | `19,253 → 19,253` | `40 → 38` | `17 → 17` | O |
| Actors | `scheduler_on_initialize_cutoff` | `12,292,000 → 9,498,000` | -22.73% | `1,560 → 1,560` | `2 → 2` | `2 → 2` | C |
| Actors | `scheduler_on_idle_base` | `19,626,000 → 21,162,000` | +7.83% | `1,560 → 1,658` | `7 → 6` | `2 → 2` | C |
| Actors | `service_member_publish_empty` | `— → 22,909,000` | new | `— → 3,948` | `— → 5` | `— → 3` | C |
| Actors | `service_member_publish_populated` | `— → 31,988,000` | new | `— → 6,086` | `— → 6` | `— → 4` | C |
| Actors | `service_member_retire_singleton` | `— → 31,010,000` | new | `— → 3,948` | `— → 5` | `— → 3` | C |
| Actors | `service_member_retire_pair_cursor` | `— → 38,692,000` | new | `— → 6,086` | `— → 6` | `— → 4` | C |
| Actors | `service_member_retire_interior` | `— → 43,931,000` | new | `— → 8,634` | `— → 7` | `— → 5` | C |
| Actors | `service_member_insert_populated` | `— → 29,963,000` | new | `— → 6,086` | `— → 6` | `— → 3` | C |
| Actors | `service_member_to_deadline_new_key` | `— → 816,109,000` | new | `— → 61,834` | `— → 35` | `— → 31` | C |
| Actors | `service_round_begin_populated` | `— → 9,429,000` | new | `— → 1,511` | `— → 1` | `— → 1` | C |
| Actors | `service_round_probe_eligible` | `— → 16,203,000` | new | `— → 3,550` | `— → 3` | `— → 0` | C |
| Actors | `service_round_admit_eligible` | `— → 22,839,000` | new | `— → 3,550` | `— → 3` | `— → 3` | C |
| Actors | `dependency_publication_begun_empty_source_list` | `— → 18,439,000` | new | `— → 3,523` | `— → 4` | `— → 3` | C |
| Actors | `dependency_publication_begun_populated_source_list` | `— → 24,235,000` | new | `— → 6,020` | `— → 5` | `— → 4` | C |
| Actors | `dependency_publication_coalesced_active_source` | `— → 15,715,000` | new | `— → 3,523` | `— → 2` | `— → 1` | C |
| Actors | `contract_geometry_create` | `35,052,172 + 6,036,087·c → 20,263,698 + 5,452,956·c` | -42.19% | `4,499 + 2,475·c → 4,307 + 2,475·c` | `4 + 1·c → 2 + 1·c` | `1 + 1·c → 1 + 1·c` | C |
| Actors | `contract_geometry_close` | `34,244,305 + 8,784,131·c → 26,825,089 + 8,088,398·c` | -21.67% | `4,740 + 2,669·c → 4,439 + 2,178·c` | `3 + 1·c → 2 + 1·c` | `2 + 1·c → 1 + 1·c` | C |
| Actors | `contract_geometry_reconstruct` | `31,311,645 + 6,659,243·c → 24,168,590 + 5,771,500·c` | -22.81% | `4,740 + 2,669·c → 4,891 + 2,673·c` | `3 + 1·c → 2 + 1·c` | `0 → 0` | C |
| Actors | `current_step_load_head` | `30,451,000 → 21,512,000` | -29.36% | `4,731 → 4,883` | `3 → 2` | `0 → 0` | C |
| Actors | `current_step_load_tail` | `39,333,653 → 26,658,269 + 148,130·s` | -32.23% | `4,913 + 14·s → 5,062 + 15·s` | `4 → 3` | `0 → 0` | C |
| Actors | `current_step_plan_opening_head` | `81,297,000 → 35,410,000` | -56.44% | `4,968 → 4,918` | `5 → 3` | `0 → 0` | C |
| Actors | `current_step_plan_suspended_head` | `123,132,000 → 48,611,000` | -60.52% | `5,707 → 5,253` | `6 → 3` | `0 → 0` | C |
| Actors | `current_step_plan_running_tail` | `117,787,560 + 1,082,421·s → 49,745,421 + 390,182·s` | -57.77% | `5,796 + 73·s → 5,577 + 148·s` | `7 → 4` | `0 → 0` | C |
| Actors | `predicate_asset_evaluation` | `8,967,155 + 8,774,724·p → 8,036,224 + 8,864,603·p` | -10.38% | `1,344 + 2,838·p → 1,345 + 2,838·p` | `0 + 2·p → 0 + 2·p` | `0 → 0` | C |
| Actors | `predicate_observation_heavy_evaluation` | `20,339,756 + 10,257,124·o → 20,626,128 + 10,071,584·o` | +1.41% | `4,364 + 2,673·o → 4,364 + 2,673·o` | `2 + 2·o → 2 + 2·o` | `0 → 0` | C |
| Actors | `scheduler_actor_state_probe` | `114,890,000 → 81,576,000` | -29.00% | `15,106 → 7,471` | `7 → 11` | `0 → 0` | M |
| Actors | `at_time_trigger_occurrence` | `349,561,000 → 1,477,516,000` | +322.68% | `8,451 → 107,705` | `20 → 84` | `9 → 34` | C |
| Actors | `cadenced_trigger_occurrence` | `536,041,000 → 1,094,220,000` | +104.13% | `8,450 → 41,769` | `27 → 35` | `16 → 15` | C |
| Actors | `cadenced_running_rearm` | `— → 1,555,879,000` | new | `— → 76,978` | `— → 52` | `— → 32` | C |
| Actors | `cadenced_suspended_service_rearm` | `— → 1,572,920,000` | new | `— → 76,983` | `— → 52` | `— → 32` | C |
| Actors | `cadenced_suspended_deadline_rearm` | `— → 1,598,204,000` | new | `— → 78,251` | `— → 53` | `— → 32` | C |
| Actors | `scheduler_inner_zero_step_complete` | `58,109,000 → 113,354,000` | +95.07% | `4,388 → 5,207` | `2 → 9` | `3 → 4` | C |
| Actors | `scheduler_paged_execute_opening_max` | `593,660,000 → 335,732,000` | -43.45% | `14,158 → 9,786` | `24 → 17` | `13 → 7` | C |
| Actors | `scheduler_inner_opening_failed_min` | `109,517,654 + 25,013,024·t → 144,724,269 + 15,900,167·t` | +32.15% | `5,237 + 3,000·t → 5,714 + 3,029·t` | `6 + 1·t → 11 + 1·t` | `5 → 5` | C |
| Actors | `scheduler_inner_opening_retry_min` | `195,144,203 + 29,296,426·t → 215,816,512 + 20,293,894·t` | +10.59% | `5,946 + 2,950·t → 6,301 + 2,965·t` | `15 + 1·t → 19 + 1·t` | `11 → 11` | C |
| Actors | `scheduler_inner_opening_failed_max` | `165,770,741 + 94,996,343·t → 171,762,413 + 27,835,895·t` | +3.61% | `12,845 + 10,106·t → 8,868 + 3,438·t` | `12 + 8·t → 14 + 1·t` | `5 → 5` | C |
| Actors | `scheduler_inner_opening_retry_max` | `228,451,910 + 47,226,607·t → 242,750,683 + 26,860,789·t` | +6.26% | `8,291 + 5,515·t → 6,602 + 3,294·t` | `18 + 5·t → 20 + 1·t` | `11 → 11` | C |
| Actors | `scheduler_inner_opening_complete_min` | `68,359,831 + 11,019,188·t → 143,448,867 + 16,975,175·t` | +109.84% | `4,398 → 5,276 + 2,840·t` | `2 → 10 + 1·t` | `3 → 4` | C |
| Actors | `scheduler_inner_opening_user_complete_header_max` | `175,025,000 → 217,280,000` | +24.14% | `7,990 → 8,794` | `9 → 13` | `5 → 6` | C |
| Actors | `scheduler_inner_opening_user_complete_header_max_tail` | `221,092,770 + 17,027,764·t → 246,948,012 + 3,931,642·t` | +11.69% | `10,677 + 2,834·t → 8,794` | `11 + 1·t → 13` | `8 → 6` | C |
| Actors | `scheduler_inner_opening_progress_min` | `185,578,808 + 14,888,868·t → 160,212,226 + 19,974,447·t` | -13.67% | `6,363 + 2,957·t → 6,481 + 2,961·t` | `11 + 1·t → 12 + 1·t` | `7 → 5` | C |
| Actors | `scheduler_inner_opening_complete_max` | `153,228,555 + 90,160,651·t → 167,573,242 + 26,983,125·t` | +9.36% | `9,978 + 10,416·t → 6,335 + 3,293·t` | `11 + 8·t → 13 + 1·t` | `3 → 4` | C |
| Actors | `scheduler_inner_opening_progress_max` | `313,030,635 + 15,870,597·t → 201,239,551 + 16,991,532·t` | -35.71% | `49,359 + 3,202·t → 14,707 + 3,520·t` | `44 + 1·t → 18 + 1·t` | `7 → 5` | C |
| Actors | `scheduler_inner_running_complete` | `133,921,853 + 2,058,487·s + 20,721,195·p → 179,179,994 + 13,595,611·p` | +33.79% | `10,952 + 2,914·p + 11·s → 5,591 + 2,824·p + 59·s` | `11 + 2·p → 10 + 2·p` | `4 → 5` | C |
| Actors | `scheduler_inner_running_progress` | `284,303,244 + 6,766,806·p → 822,841,832` | +189.42% | `12,546 + 2,220·p → 7,454 + 2,159·p + 1,596·s` | `18 + 2·p → 14 + 1·p` | `5 → 5` | C |
| Actors | `scheduler_inner_suspended_tail_retry` | `224,662,450 + 3,771,914·s + 15,334,382·p → 208,792,886 + 1,303,133·s + 8,971,106·p` | -7.06% | `6,937 + 2,210·p + 101·s → 6,934 + 2,187·p` | `15 + 2·p → 18 + 2·p` | `9 → 11` | C |
| Actors | `scheduler_inner_suspended_tail_complete` | `213,795,680 + 15,584,645·p → 200,074,224 + 2,082,969·s + 12,153,092·p` | -6.42% | `8,925 + 2,207·p + 204·s → 9,486 + 2,185·p + 230·s` | `8 + 2·p → 15 + 2·p` | `8 → 9` | C |
| Actors | `scheduler_inner_suspended_tail_progress` | `244,666,435 + 853,649·s + 21,015,964·p → 219,083,655 + 12,129,889·p` | -10.46% | `9,954 + 2,192·p + 491·s → 10,137 + 2,158·p + 400·s` | `14 + 2·p → 15 + 2·p` | `9 → 9` | C |
| Actors | `scheduler_inner_suspended_head_retry` | `233,830,626 + 244,723·n + 9,611,569·p → 221,558,027 + 164,629·n + 10,561,882·p` | -5.25% | `6,848 + 2,793·p → 7,042 + 1·n + 2,848·p` | `14 + 2·p → 18 + 2·p` | `9 → 11` | C |
| Actors | `scheduler_inner_suspended_head_complete` | `145,300,991 + 1,898,621·p → 159,494,588 + 7,817,974·p` | +9.77% | `6,004 + 2,701·p → 6,430 + 2,697·p` | `6 + 2·p → 12 + 2·p` | `4 → 5` | C |
| Actors | `scheduler_inner_suspended_head_progress` | `224,981,329 + 39,366·n + 11,797,163·p → 180,969,209 + 146,640·n + 9,928,965·p` | -19.56% | `7,784 + 2,793·p → 7,261 + 1·n + 2,848·p` | `12 + 2·p → 12 + 2·p` | `5 → 5` | C |
| Actors | `scheduler_inner_suspended_head_opening_retry` | `273,071,818 + 6,709,813·p → 239,674,363 + 123,116·n + 8,992,572·p` | -12.23% | `6,832 + 2,786·p → 9,910 + 1·n + 2,841·p` | `14 + 2·p → 20 + 2·p` | `9 → 11` | C |
| Actors | `scheduler_inner_suspended_head_opening_complete` | `128,652,913 + 4,574,010·p → 175,274,624 + 6,331,998·p` | +36.24% | `6,017 + 2,694·p → 9,139 + 2,688·p` | `6 + 2·p → 14 + 2·p` | `4 → 5` | C |
| Actors | `scheduler_inner_suspended_head_opening_progress` | `236,649,836 + 11,253,145·p → 183,914,037 + 662,865·n + 13,580,472·p` | -22.28% | `7,919 + 2,736·p → 10,129 + 1·n + 2,841·p` | `12 + 2·p → 14 + 2·p` | `5 → 5` | C |
| Actors | `scheduler_service_successful_interior` | `— → 248,569,000` | new | `— → 7,558` | `— → 17` | `— → 5` | C |
| Actors | `scheduler_service_retry_to_deadline` | `— → 314,361,000` | new | `— → 11,294` | `— → 22` | `— → 9` | C |
| Actors | `scheduler_service_retry_to_deadline_new_key` | `— → 1,272,459,000` | new | `— → 63,894` | `— → 43` | `— → 33` | C |
| Actors | `scheduler_service_late_refusal_rollback` | `— → 82,624,000` | new | `— → 6,135` | `— → 12` | `— → 0` | C |
| Actors | `scheduler_service_terminal_retain_close` | `— → 270,360,000` | new | `— → 6,380` | `— → 21` | `— → 13` | C |
| Actors | `scheduler_service_minimal_apoptosis` | `— → 291,243,000` | new | `— → 9,148` | `— → 22` | `— → 13` | C |
| Actors | `scheduler_due_deadline_to_service` | `— → 349,492,000` | new | `— → 22,609` | `— → 36` | `— → 19` | C |
| Actors | `scheduler_due_deadline_to_service_deep_index` | `— → 1,288,942,000` | new | `— → 88,132` | `— → 64` | `— → 42` | C |
| Actors | `run_progress` | `279,510,000 → 113,285,000` | -59.47% | `6,456 → 6,995` | `14 → 12` | `9 → 2` | C |
| Actors | `run_suspend` | `286,075,000 → 107,068,000` | -62.57% | `5,871 → 6,113` | `13 → 11` | `9 → 2` | C |
| Actors | `run_complete` | `201,984,000 → 103,158,000` | -48.93% | `6,237 → 6,995` | `8 → 12` | `6 → 2` | C |
| Actors | `run_cancel` | `486,871,000 → 434,769,000` | -10.70% | `43,539 → 20,433` | `22 → 23` | `10 → 9` | C |
| Actors | `transaction_extension_ingress_base` | `14,667,000 → 15,575,000` | +6.19% | `6,052 → 6,052` | `2 → 2` | `0 → 0` | C |
| Actors | `transaction_extension_ingress_notify` | `473,182,000 → 299,903,000` | -36.62% | `20,625 → 20,433` | `13 → 16` | `2 → 0` | C |
| Actors | `maximum_context_inherent` | `204,517,874,000 → 200,541,308,000` | -1.94% | `3,517 → 3,517` | `13 → 13` | `23 → 23` | C |
| Actors | `block_resource_finalize` | `9,010,000 → 8,171,000` | -9.31% | `1,560 → 1,560` | `1 → 1` | `2 → 2` | C |
| Actors | `block_resource_meter_extension` | `11,035,000 → 10,477,000` | -5.06% | `1,560 → 1,560` | `1 → 1` | `1 → 1` | C |
| Actors | `maximum_xcm_version_discovery` | `465,709,000 → 450,413,000` | -3.28% | `248,490 → 248,490` | `102 → 102` | `1 → 1` | C |
| Oracle | `register_feed_existing_producer` | `146,111,000 → 144,225,000` | -1.29% | `20,532 → 20,532` | `3 → 3` | `3 → 3` | C |
| Oracle | `register_feed_new_producer` | `215,185,000 → 208,620,000` | -3.05% | `44,394 → 45,174` | `4 → 4` | `4 → 4` | C |
| Oracle | `pause_feed` | `15,366,000 → 17,251,000` | +12.27% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `resume_feed` | `15,435,000 → 17,251,000` | +11.77% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `deactivate_feed` | `15,365,000 → 17,181,000` | +11.82% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `publish_last_value` | `33,733,000 → 20,464,000` | -39.34% | `3,559 → 3,551` | `6 → 2` | `1 → 1` | C |
| Oracle | `publish_ema_changed` | `37,156,000 → 22,210,000` | -40.22% | `3,559 → 3,551` | `6 → 2` | `1 → 1` | C |
| Oracle | `publish_ema_refresh` | `22,280,000 → 22,350,000` | +0.31% | `3,551 → 3,551` | `2 → 2` | `1 → 1` | C |
| Router | `direct_xyk_exact_input` | `495,601,000 → 495,113,000` | -0.10% | `15,106 → 8,817` | `31 → 26` | `18 → 11` | C |
| Router | `direct_mint_exact_input` | `561,603,000 → 546,517,000` | -2.69% | `21,862 → 21,862` | `38 → 38` | `14 → 13` | C |
| Router | `native_anchored_exact_input` | `646,251,000 → 648,138,000` | +0.29% | `19,253 → 19,253` | `45 → 35` | `27 → 16` | C |
| Router | `direct_xyk_exact_output` | `481,842,000 → 511,945,000` | +6.25% | `15,106 → 8,817` | `30 → 25` | `18 → 11` | C |
| Router | `native_anchored_exact_output` | `656,309,000 → 643,947,000` | -1.88% | `19,253 → 19,253` | `44 → 34` | `27 → 16` | C |
| Router | `create_pool` | `151,628,000 → 145,063,000` | -4.33% | `34,255 → 34,255` | `13 → 12` | `10 → 10` | C |
| Router | `update_router_fee` | `8,661,000 → 8,870,000` | +2.41% | `1,489 → 1,489` | `1 → 1` | `1 → 1` | C |
| TMC | `create_curve` | `26,610,000 → 27,658,000` | +3.94% | `6,360 → 6,360` | `3 → 3` | `1 → 1` | C |
| TMC | `mint_with_distribution` | `254,994,000 → 198,492,000` | -22.16% | `7,535 → 6,208` | `15 → 13` | `4 → 4` | C |

## Interpretation

Every listed dimension requires review against the owning implementation and benchmark evidence. Positive deltas remain unexplained until the release candidate records their measured reason; this generated comparison does not accept them by itself.

## Retired Weight Owners

- Actors `create_user_actor_crossing_new_page`
- Actors `pipeline_admission_apoptosis`
- Actors `record_crossing_worker_fault`
- Actors `record_observation_fanout_worker_fault`
- Actors `record_wakeup_worker_fault`
- Actors `clear_crossing_worker_fault`
- Actors `clear_observation_fanout_worker_fault`
- Actors `clear_wakeup_worker_fault`
- Actors `task_split_transfer`
- Actors `materialization_coordinator_base`
- Actors `opening_snapshot_traversal`
- Actors `opening_snapshot_capture`
- Actors `opening_target_snapshot_capture`
- Actors `opening_share_mixed_capture`
- Actors `opening_predicate_traversal`
- Actors `opening_predicate_capture`
- Actors `opening_max_encoded_balance_capture`
- Actors `opening_observation_heavy_capture`
- Actors `scheduler_paged_append_existing_page`
- Actors `scheduler_paged_append_new_page`
- Actors `scheduler_wakeup_append_existing_page`
- Actors `scheduler_wakeup_append_new_page`
- Actors `scheduler_wakeup_replace_exact`
- Actors `scheduler_wakeup_invalidate_middle_page`
- Actors `scheduler_wakeup_drain_partial_page`
- Actors `scheduler_wakeup_drain_full_page`
- Actors `scheduler_wakeup_drain_dense_boundary`
- Actors `scheduler_wakeup_drain_stale_page`
- Actors `scheduler_wakeup_cursor_insert`
- Actors `scheduler_wakeup_cursor_pop_min`
- Actors `scheduler_wakeup_cursor_remove_exact`
- Actors `scheduler_wakeup_cursor_worker_partial`
- Actors `scheduler_wakeup_cursor_worker_remove`
- Actors `scheduler_wakeup_cursor_worker_future`
- Actors `scheduler_paged_consume_preserve_page`
- Actors `scheduler_paged_consume_delete_page`
- Actors `scheduler_paged_tombstone_drain`
- Actors `scheduler_paged_mixed_scan`
- Actors `scheduler_paged_zero_step_user_crossing_unavailable`
- Actors `scheduler_paged_execute_cheap`
- Actors `scheduler_paged_execute_cheap_mixed`
- Actors `observation_change_trigger_occurrence`
- Actors `observation_change_ingress`
- Actors `observation_fanout_base`
- Actors `observation_fanout_branch_probe`
- Actors `observation_fanout_page`
- Actors `observation_fanout_wakeup_page`
- Actors `observation_fanout_coalesced_page`
- Actors `observation_fanout_terminal`
- Actors `observation_fanout_blocked_page`
- Actors `crossing_worker_base`
- Actors `crossing_work_probe`
- Actors `crossing_selection_probe`
- Actors `observation_crossing_trigger_occurrence`
- Actors `crossing_search_probe`
- Actors `crossing_fire_probe`
- Actors `crossing_fire_pair_probe`
- Actors `crossing_tail_refill_probe`
- Actors `crossing_fire_cohort_preflight`
- Actors `crossing_coalesced_cohort_preflight`
- Actors `crossing_terminal_cohort_preflight`
- Actors `crossing_skip_cohort_preflight`
- Actors `crossing_rearm_cohort_preflight`
- Actors `crossing_rearm_pair_probe`
- Actors `crossing_skip_pair_probe`
- Actors `crossing_transition_unit`
- Actors `crossing_leaf_unit`
- Actors `crossing_page_unit`
- Actors `crossing_rearm_unit`
- Actors `crossing_rearm_pair_unit`
- Actors `crossing_coalesced_unit`
- Actors `crossing_coalesced_pair_unit`
- Actors `crossing_placed_unit`
- Actors `crossing_placed_pair_unit`
- Actors `crossing_placed_maximum_unit`
- Actors `crossing_placed_non_tail_emptied_unit`
- Actors `crossing_placed_non_tail_trimmed_unit`
- Actors `crossing_skip_unit`
- Actors `crossing_skip_pair_unit`
- Actors `crossing_actor_unit`
- Actors `funding_snapshot_open`
- Oracle `publish_ema_changed_primary_first`
- Oracle `publish_ema_changed_primary_existing`
- Oracle `publish_ema_changed_secondary_first`
- Oracle `publish_ema_changed_secondary_existing`
- Oracle `publish_ema_changed_combined`
- Oracle `publish_ema_changed_secondary_capacity`

Any retired owner requires implementation review before release acceptance; absence from the candidate alone does not prove safe replacement.

## Reproduction

- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`
- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`
- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Architecture Experiments Skill; focused outputs do not replace complete generated pallet files.

Candidate weight source identity: `85cadccea3ae6f4d52d642dbabe244bd9126e3588a14ca39da8651f15d9cac0d`.

