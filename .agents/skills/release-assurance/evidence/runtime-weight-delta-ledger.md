# Runtime Weight Delta Ledger

## Evidence Boundary

This generated ledger compares the production Weight implementations in Git tag `v0.7.22` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.

Candidate release: `0.7.23`. The locally validated production runtime was generated with `./scripts/03-build-runtime.sh`; compact Wasm SHA-256 is `4b04e98b598cb0e72516e12382b742858ba720631f769b60be433d7e1acd989a`. The accepted benchmark owners use `frame-omni-bencher 0.22.0` / CLI `58.0.0`, `50` steps, `20` repeats, compiled Wasm execution, RocksDB, 1,024 MiB cache, host `fedora`, and CPU `AMD Ryzen 7 4800H with Radeon Graphics`; each generated method records date, reads, writes, measured ProofSize, and conservative ProofSize in its authoritative source. The benchmark-runtime Wasm and production Wasm are distinct evidence identities. Exact candidate commit/tree identity remains unavailable until the validated worktree is committed through the authorized release gate.

Interpretation codes classify changed paths only: `I` identity guard; `C` correctness; `P` bounded service topology; `M` merged canonical work; `O` measured optimization.

## Changed Production Paths

| Pallet | Weight method | RefTime: v0.7.22 → 0.7.23 candidate | Base delta | ProofSize: v0.7.22 → 0.7.23 candidate | Reads: v0.7.22 → 0.7.23 candidate | Writes: v0.7.22 → 0.7.23 candidate | Code |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Actors | `create_user_actor` | `193,393,000 → 261,839,000` | +35.39% | `12,200 → 81,886` | `25 → 54` | `19 → 47` | C |
| Actors | `create_user_actor_at_slot` | `145,691,000 → 258,137,000` | +77.18% | `12,200 → 81,886` | `16 → 54` | `10 → 47` | C |
| Actors | `create_system_actor` | `103,087,000 → 210,295,000` | +104.00% | `12,200 → 81,886` | `16 → 53` | `11 → 47` | C |
| Actors | `create_system_actor_at_sovereign_id` | `91,563,000 → 199,540,000` | +117.93% | `12,200 → 81,886` | `14 → 51` | `9 → 45` | C |
| Actors | `create_user_actor_crossing_new_page` | `— → 254,785,000` | new | `— → 12,200` | `— → 22` | `— → 15` | C |
| Actors | `create_dormant_system_actor` | `67,817,000 → 65,861,000` | -2.88% | `12,200 → 12,200` | `12 → 12` | `7 → 7` | C |
| Actors | `activate_actor` | `124,249,000 → 190,181,000` | +53.06% | `12,200 → 81,886` | `19 → 47` | `14 → 41` | C |
| Actors | `deactivate_actor` | `103,018,000 → 312,475,000` | +203.32% | `12,200 → 81,886` | `9 → 46` | `6 → 43` | C |
| Actors | `pause_actor` | `62,020,000 → 59,506,000` | -4.05% | `12,200 → 12,200` | `7 → 7` | `2 → 2` | C |
| Actors | `resume_actor` | `61,112,000 → 59,506,000` | -2.63% | `12,200 → 12,200` | `7 → 7` | `2 → 2` | C |
| Actors | `manual_trigger` | `84,719,000 → 82,764,000` | -2.31% | `12,200 → 12,200` | `12 → 12` | `5 → 5` | C |
| Actors | `close_actor` | `210,365,000 → 355,707,000` | +69.09% | `12,200 → 81,886` | `24 → 51` | `23 → 50` | C |
| Actors | `update_contract` | `274,480,000 → 457,049,000` | +66.51% | `12,200 → 81,886` | `25 → 54` | `22 → 49` | C |
| Actors | `clear_crossing_worker_fault` | `— → 14,248,000` | new | `— → 1,529` | `— → 1` | `— → 1` | C |
| Actors | `clear_observation_fanout_worker_fault` | `— → 13,828,000` | new | `— → 1,516` | `— → 1` | `— → 1` | C |
| Actors | `clear_wakeup_worker_fault` | `— → 12,991,000` | new | `— → 1,503` | `— → 1` | `— → 1` | C |
| Actors | `set_active_actor_limit` | `9,289,000 → 9,429,000` | +1.51% | `1,489 → 1,489` | `2 → 2` | `0 → 0` | C |
| Actors | `permissionless_sweep` | `44,350,000 → 42,465,000` | -4.25% | `12,200 → 12,200` | `7 → 7` | `0 → 0` | C |
| Actors | `permissionless_sweep_many` | `23,891,147 + 66,286,454·n → 20,599,986 + 87,291,781·n` | -13.78% | `1,489 + 11,210·n → 1,489 + 11,210·n` | `3 + 11·n → 3 + 12·n` | `2 + 7·n → 2 + 8·n` | C |
| Actors | `fee_collection` | `43,791,000 → 43,022,000` | -1.76% | `3,593 → 3,593` | `1 → 1` | `1 → 1` | O |
| Actors | `task_transfer` | `232,854,000 → 226,429,000` | -2.76% | `12,200 → 12,200` | `18 → 18` | `8 → 8` | C |
| Actors | `task_burn` | `19,067,000 → 18,997,000` | -0.37% | `3,593 → 3,593` | `1 → 1` | `1 → 1` | C |
| Actors | `task_mint` | `197,794,000 → 191,089,000` | -3.39% | `12,200 → 12,200` | `16 → 16` | `6 → 6` | C |
| Actors | `predicate_set_evaluation` | `7,612,000 + 6,444,212·c → 7,543,000 + 6,543,854·c` | -0.91% | `3,675 + 674·c → 3,675 + 674·c` | `1 + 1·c → 1 + 1·c` | `0 → 0` | C |
| Actors | `task_stop_cycle` | `4,260,000 → 4,330,000` | +1.64% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `task_split_transfer` | `80,834,694 + 154,149,994·l → 78,936,673 + 152,943,219·l` | -2.35% | `8,040 + 11,210·l → 8,040 + 11,210·l` | `10 + 7·l → 10 + 7·l` | `4 + 3·l → 4 + 3·l` | C |
| Actors | `xcm_asset_deposit` | `218,117,000 → 215,464,000` | -1.22% | `12,200 → 12,200` | `19 → 19` | `8 → 8` | C |
| Actors | `task_add_liquidity` | `271,337,000 → 266,588,000` | -1.75% | `34,255 → 34,255` | `16 → 16` | `15 → 15` | C |
| Actors | `task_donate_liquidity` | `164,618,000 → 161,755,000` | -1.74% | `14,035 → 14,035` | `9 → 9` | `8 → 8` | C |
| Actors | `task_remove_liquidity` | `155,050,000 → 151,767,000` | -2.12% | `8,817 → 8,817` | `8 → 8` | `6 → 6` | C |
| Actors | `task_stake` | `85,906,000 → 84,649,000` | -1.46% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_unstake` | `101,691,000 → 100,503,000` | -1.17% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_dex_exact_in` | `509,640,000 → 502,376,000` | -1.43% | `19,253 → 19,253` | `40 → 40` | `17 → 17` | O |
| Actors | `task_dex_exact_out` | `502,865,000 → 495,532,000` | -1.46% | `19,253 → 19,253` | `39 → 39` | `17 → 17` | O |
| Actors | `scheduler_on_idle_base` | `14,737,000 → 14,667,000` | -0.47% | `1,543 → 1,543` | `6 → 6` | `1 → 1` | C |
| Actors | `materialization_coordinator_base` | `— → 25,283,000` | new | `— → 5,982` | `— → 10` | `— → 1` | C |
| Actors | `scheduler_actor_state_probe` | `37,715,000 → 36,109,000` | -4.26% | `12,200 → 12,200` | `5 → 5` | `0 → 0` | M |
| Actors | `cycle_orchestration` | `50,426,000 → 48,750,000` | -3.32% | `12,200 → 12,200` | `5 → 5` | `3 → 3` | C |
| Actors | `step_orchestration` | `49,961,778 + 174,432·n → 48,660,474 + 183,073·n` | -2.60% | `12,200 → 12,200` | `5 → 5` | `3 → 3` | C |
| Actors | `scheduler_paged_append_existing_page` | `75,639,000 → 74,172,000` | -1.94% | `7,938 → 7,979` | `10 → 10` | `5 → 5` | C |
| Actors | `scheduler_paged_append_new_page` | `73,335,000 → 72,008,000` | -1.81% | `10,283 → 10,319` | `11 → 11` | `5 → 5` | C |
| Actors | `scheduler_wakeup_append_existing_page` | `78,642,000 → 76,268,000` | -3.02% | `6,694 → 6,736` | `8 → 8` | `3 → 3` | C |
| Actors | `scheduler_wakeup_append_new_page` | `81,995,000 → 78,573,000` | -4.17% | `6,833 → 6,880` | `8 → 8` | `4 → 4` | C |
| Actors | `scheduler_wakeup_replace_exact` | `99,455,000 → 97,151,000` | -2.32% | `7,492 → 7,521` | `10 → 10` | `7 → 7` | C |
| Actors | `scheduler_wakeup_invalidate_middle_page` | `90,097,000 → 91,843,000` | +1.94% | `12,495 → 12,536` | `10 → 10` | `5 → 5` | C |
| Actors | `scheduler_wakeup_drain_partial_page` | `354,451,000 → 354,939,000` | +0.14% | `47,750 → 48,075` | `83 → 83` | `18 → 18` | C |
| Actors | `scheduler_wakeup_drain_full_page` | `648,137,000 → 658,684,000` | +1.63% | `90,034 → 90,587` | `164 → 164` | `36 → 36` | C |
| Actors | `scheduler_wakeup_drain_dense_boundary` | `673,421,000 → 686,551,000` | +1.95% | `92,825 → 93,394` | `170 → 170` | `38 → 38` | C |
| Actors | `scheduler_wakeup_drain_stale_page` | `569,704,000 → 602,530,000` | +5.76% | `89,426 → 89,979` | `164 → 164` | `4 → 4` | C |
| Actors | `scheduler_wakeup_cursor_insert` | `346,837,000 → 347,047,000` | +0.06% | `42,733 → 42,733` | `25 → 25` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_pop_min` | `452,230,000 → 448,249,000` | -0.88% | `55,259 → 55,259` | `34 → 34` | `26 → 26` | C |
| Actors | `scheduler_wakeup_cursor_remove_exact` | `422,267,000 → 419,753,000` | -0.60% | `54,726 → 54,726` | `33 → 33` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_worker_partial` | `112,517,000 → 116,428,000` | +3.48% | `7,498 → 7,524` | `14 → 15` | `8 → 8` | C |
| Actors | `scheduler_wakeup_cursor_worker_remove` | `548,262,000 → 535,621,000` | -2.31% | `56,434 → 56,463` | `48 → 49` | `33 → 33` | C |
| Actors | `scheduler_wakeup_cursor_worker_future` | `20,184,000 → 25,283,000` | +25.26% | `6,523 → 6,562` | `5 → 6` | `0 → 0` | C |
| Actors | `scheduler_paged_consume_preserve_page` | `48,540,000 → 47,702,000` | -1.73% | `4,868 → 4,893` | `9 → 9` | `3 → 3` | C |
| Actors | `scheduler_paged_consume_delete_page` | `50,007,000 → 48,750,000` | -2.51% | `4,846 → 4,875` | `9 → 9` | `5 → 5` | C |
| Actors | `scheduler_paged_tombstone_drain` | `43,931,000 + 9,481,801·n → 41,975,000 + 9,491,017·n` | -4.45% | `4,049 + 2,492·n → 4,096 + 2,492·n` | `5 + 5·n → 5 + 5·n` | `4 → 4` | C |
| Actors | `scheduler_paged_mixed_scan` | `47,004,000 + 40,911,571·n → 46,096,000 + 39,857,795·n` | -1.93% | `4,857 + 2,608·n → 4,846 + 2,616·n` | `5 + 5·n → 5 + 5·n` | `3 + 1·n → 3 + 1·n` | C |
| Actors | `scheduler_paged_execute_cheap` | `116,986,000 + 94,182,240·n → 115,379,000 + 93,865,117·n` | -1.37% | `3,716 + 2,733·n → 3,735 + 2,750·n` | `6 + 5·n → 6 + 5·n` | `4 + 3·n → 4 + 3·n` | C |
| Actors | `scheduler_paged_execute_cheap_mixed` | `244,449,000 + 120,287,892·n → 248,569,000 + 119,818,614·n` | +1.69% | `4,918 + 2,798·n → 4,729 + 2,829·n` | `6 + 6·n → 6 + 6·n` | `4 + 4·n → 4 + 4·n` | C |
| Actors | `continuation_suspend` | `39,972,061 + 41,737·s → 38,981,444 + 36,264·s` | -2.48% | `4,728 → 4,757` | `5 → 5` | `2 → 2` | C |
| Actors | `continuation_complete` | `40,928,000 → 39,601,000` | -3.24% | `5,154 → 5,183` | `5 → 5` | `2 → 2` | C |
| Actors | `continuation_cancel` | `177,888,000 → 175,514,000` | -1.33% | `12,200 → 12,200` | `16 → 16` | `11 → 11` | C |
| Actors | `continuation_suffix_admission` | `1,432,532 + 854·n → 1,432,976 + 494·n` | +0.03% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `observation_change_ingress` | `33,384,000 → 32,896,000` | -1.46% | `6,128 → 6,128` | `5 → 5` | `4 → 4` | C |
| Actors | `observation_fanout_base` | `6,146,000 → 6,705,000` | +9.10% | `1,543 → 1,543` | `1 → 2` | `0 → 0` | C |
| Actors | `observation_fanout_page` | `2,644,513,000 → 2,651,008,000` | +0.25% | `718,430 → 718,430` | `332 → 332` | `72 → 72` | C |
| Actors | `crossing_worker_base` | `6,146,000 → 6,705,000` | +9.10% | `1,543 → 1,543` | `1 → 2` | `0 → 0` | C |
| Actors | `crossing_work_probe` | `— → 46,305,000` | new | `— → 12,200` | `— → 8` | `— → 0` | C |
| Actors | `crossing_search_probe` | `— → 127,253,000` | new | `— → 81,886` | `— → 33` | `— → 0` | C |
| Actors | `crossing_fire_probe` | `— → 72,427,000` | new | `— → 12,200` | `— → 14` | `— → 0` | C |
| Actors | `crossing_tail_refill_probe` | `— → 17,530,000` | new | `— → 4,561` | `— → 1` | `— → 0` | C |
| Actors | `crossing_fire_pair_probe` | `— → 241,375,000` | new | `— → 23,410` | `— → 25` | `— → 0` | C |
| Actors | `crossing_fire_cohort_preflight` | `— → 21,192,911 + 33,046,169·c` | new | `— → 1,493 + 11,210·c` | `— → 2 + 7·c` | `— → 0` | C |
| Actors | `crossing_coalesced_cohort_preflight` | `— → 23,064,782 + 33,354,579·c` | new | `— → 1,493 + 11,210·c` | `— → 2 + 7·c` | `— → 0` | C |
| Actors | `crossing_terminal_cohort_preflight` | `— → 21,646,700 + 33,085,920·c` | new | `— → 1,493 + 11,210·c` | `— → 2 + 7·c` | `— → 0` | C |
| Actors | `crossing_skip_cohort_preflight` | `— → 6,986,537 + 8,569,450·c` | new | `— → 990 + 2,561·c` | `— → 0 + 2·c` | `— → 0` | C |
| Actors | `crossing_rearm_cohort_preflight` | `— → 12,893,611 + 11,773,740·c` | new | `— → 990 + 11,210·c` | `— → 0 + 3·c` | `— → 0` | C |
| Actors | `crossing_rearm_pair_probe` | `— → 58,249,000` | new | `— → 23,410` | `— → 11` | `— → 0` | C |
| Actors | `crossing_skip_pair_probe` | `— → 46,515,000` | new | `— → 6,112` | `— → 9` | `— → 0` | C |
| Actors | `crossing_transition_unit` | `31,988,000 → 31,918,000` | -0.22% | `6,060 → 6,060` | `5 → 5` | `2 → 2` | C |
| Actors | `crossing_leaf_unit` | `425,689,000 → 439,029,000` | +3.13% | `162,782 → 162,782` | `85 → 87` | `77 → 78` | C |
| Actors | `crossing_page_unit` | `426,457,000 → 440,426,000` | +3.28% | `162,782 → 162,782` | `85 → 87` | `77 → 78` | C |
| Actors | `crossing_rearm_unit` | `— → 397,683,000` | new | `— → 162,782` | `— → 78` | `— → 74` | C |
| Actors | `crossing_rearm_pair_unit` | `— → 430,439,000` | new | `— → 162,782` | `— → 82` | `— → 76` | C |
| Actors | `crossing_coalesced_unit` | `— → 419,403,000` | new | `— → 162,782` | `— → 82` | `— → 74` | C |
| Actors | `crossing_coalesced_pair_unit` | `— → 478,630,000` | new | `— → 162,782` | `— → 89` | `— → 76` | C |
| Actors | `crossing_placed_unit` | `— → 439,379,000` | new | `— → 162,782` | `— → 87` | `— → 78` | C |
| Actors | `crossing_placed_pair_unit` | `— → 510,967,000` | new | `— → 162,782` | `— → 94` | `— → 80` | C |
| Actors | `crossing_placed_maximum_unit` | `— → 712,602,000` | new | `— → 162,782` | `— → 108` | `— → 84` | C |
| Actors | `crossing_placed_non_tail_emptied_unit` | `— → 601,343,000` | new | `— → 81,886` | `— → 78` | `— → 117` | C |
| Actors | `crossing_placed_non_tail_trimmed_unit` | `— → 603,089,000` | new | `— → 81,886` | `— → 78` | `— → 119` | C |
| Actors | `crossing_skip_unit` | `— → 165,038,000` | new | `— → 81,886` | `— → 40` | `— → 2` | C |
| Actors | `crossing_skip_pair_unit` | `— → 70,960,000` | new | `— → 6,112` | `— → 10` | `— → 2` | C |
| Actors | `crossing_actor_unit` | `594,359,000 → 631,585,000` | +6.26% | `162,782 → 162,782` | `91 → 94` | `83 → 85` | C |
| Actors | `transaction_extension_ingress_base` | `13,479,000 → 13,619,000` | +1.04% | `6,052 → 6,052` | `2 → 2` | `0 → 0` | C |
| Actors | `transaction_extension_ingress_notify` | `159,939,000 → 153,584,000` | -3.97% | `12,200 → 12,200` | `10 → 10` | `6 → 6` | C |
| Actors | `funding_snapshot_open` | `13,339,290 + 125,929·a → 11,872,324 + 105,543·a` | -11.00% | `3,751 → 3,767` | `1 → 1` | `1 → 1` | C |
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
| Router | `direct_xyk_exact_input` | `294,176,000 → 343,205,000` | +16.67% | `12,200 → 12,200` | `25 → 30` | `12 → 18` | C |
| Router | `direct_mint_exact_input` | `315,129,000 → 321,275,000` | +1.95% | `23,410 → 23,410` | `33 → 33` | `14 → 14` | C |
| Router | `native_anchored_exact_input` | `435,468,000 → 500,770,000` | +15.00% | `19,253 → 19,253` | `36 → 44` | `17 → 27` | C |
| Router | `direct_xyk_exact_output` | `164,898,000 → 332,170,000` | +101.44% | `6,208 → 12,200` | `10 → 29` | `5 → 18` | C |
| Router | `native_anchored_exact_output` | `302,348,000 → 506,427,000` | +67.50% | `16,644 → 19,253` | `21 → 43` | `10 → 27` | C |

## Interpretation

Every listed dimension requires review against the owning implementation and benchmark evidence. Positive deltas remain unexplained until the release candidate records their measured reason; this generated comparison does not accept them by itself.

## Retired Weight Owners

- None.

Any retired owner requires implementation review before release acceptance; absence from the candidate alone does not prove safe replacement.

## Reproduction

- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`
- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`
- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Benchmarking Skill; focused outputs do not replace complete generated pallet files.

Candidate weight source identity: `7643a3d9d80ef9239dcdc55e291b836f51bcba4338f86f1c41848cbc069ea3ce`.

