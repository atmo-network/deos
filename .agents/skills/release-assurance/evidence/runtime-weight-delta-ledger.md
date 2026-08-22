# Runtime Weight Delta Ledger

## Evidence Boundary

This generated ledger compares the production Weight implementations in Git tag `v0.7.20` with the candidate worktree. RefTime formulas exclude database Weight; reads and writes are therefore recorded independently. ProofSize is the generated conservative estimate. A parameterized formula records its generated slope rather than collapsing it to an unstated component value.

The candidate files were generated with `frame-omni-bencher 0.22.0` against production runtime Wasm at 50 steps and 20 repeats. Asset Registry and Governance used compact Wasm SHA-256 `b87e7eacebd99fe4e272fd5363e23c75c6693bef2b495d68e39ce16623b39a12`; Oracle, Router, Staking, and TMC used `fd9445658d448278e3f78cda80db488c5cdcdff6550121eb4dbb16494e0f857b`; the final Actors cancellation/wakeup refresh used `af07e3836198baff08830b439fcd9697082285bfc10c4b2f95957969d684c1db`. After version and accepted files were integrated, the production release candidate rebuilt as `7117a599485125acf3e20095aea0d42a29900fe6f067dc24681103669108204e`. Exact release identity remains conditional on the final full-evidence gate and signed release attestation.

Interpretation codes: `I` is the reserved-location identity guard remeasurement; `C` is a correctness-driven canonical-state, arithmetic, custody, rollback, or scheduler measurement; `P` is bounded phased Governance service; `M` is the merged complete actor-state probe replacing partial probes; `O` is measured duplicate-work deletion or lazy-read optimization.

## Changed Production Paths

| Pallet | Weight method | RefTime: 0.7.20 → candidate | Base delta | ProofSize: 0.7.20 → candidate | Reads: 0.7.20 → candidate | Writes: 0.7.20 → candidate | Code |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Asset Registry | `register_foreign_asset` | `41,346,000 → 40,928,000` | -1.01% | `4,087 → 4,087` | `5 → 5` | `4 → 4` | I |
| Asset Registry | `register_foreign_asset_with_id` | `43,581,000 → 43,581,000` | 0.00% | `4,087 → 4,087` | `5 → 5` | `4 → 4` | I |
| Asset Registry | `link_existing_asset` | `30,032,000 → 30,032,000` | 0.00% | `4,087 → 4,087` | `4 → 4` | `2 → 2` | I |
| Asset Registry | `migrate_location_key` | `20,534,000 → 20,533,000` | 0.00% | `7,184 → 7,184` | `2 → 2` | `3 → 3` | I |
| Actors | `create_user_actor` | `212,810,000 → 195,908,000` | -7.94% | `12,200 → 12,200` | `27 → 24` | `19 → 19` | C |
| Actors | `create_user_actor_at_slot` | `163,711,000 → 142,269,000` | -13.10% | `12,200 → 12,200` | `18 → 15` | `10 → 10` | C |
| Actors | `create_system_actor` | `79,970,000 → 91,633,000` | +14.58% | `3,593 → 12,200` | `11 → 15` | `11 → 11` | C |
| Actors | `create_system_actor_at_sovereign_id` | `64,674,000 → 82,554,000` | +27.65% | `3,575 → 12,200` | `9 → 13` | `9 → 9` | C |
| Actors | `create_dormant_system_actor` | `58,598,000 → 68,166,000` | +16.33% | `3,593 → 12,200` | `8 → 12` | `7 → 7` | C |
| Actors | `activate_actor` | `94,707,000 → 111,818,000` | +18.07% | `4,106 → 12,200` | `15 → 18` | `14 → 14` | C |
| Actors | `deactivate_actor` | `69,075,000 → 99,386,000` | +43.88% | `12,200 → 12,200` | `7 → 8` | `6 → 6` | C |
| Actors | `pause_actor` | `52,382,000 → 62,509,000` | +19.33% | `12,200 → 12,200` | `6 → 7` | `2 → 2` | C |
| Actors | `resume_actor` | `51,963,000 → 62,439,000` | +20.16% | `12,200 → 12,200` | `6 → 7` | `2 → 2` | C |
| Actors | `manual_trigger` | `66,839,000 → 82,414,000` | +23.30% | `12,200 → 12,200` | `11 → 12` | `5 → 5` | C |
| Actors | `close_actor` | `190,669,000 → 209,807,000` | +10.04% | `12,200 → 12,200` | `23 → 23` | `23 → 23` | C |
| Actors | `update_contract` | `243,262,000 → 281,465,000` | +15.70% | `12,200 → 12,200` | `24 → 24` | `22 → 22` | C |
| Actors | `set_global_circuit_breaker` | `7,193,000 → 7,124,000` | -0.96% | `0 → 0` | `0 → 0` | `1 → 1` | C |
| Actors | `set_active_actor_limit` | `9,498,000 → 9,149,000` | -3.67% | `1,489 → 1,489` | `2 → 2` | `0 → 0` | C |
| Actors | `permissionless_sweep` | `43,721,000 → 45,747,000` | +4.63% | `12,200 → 12,200` | `7 → 7` | `0 → 0` | C |
| Actors | `permissionless_sweep_many` | `21,370,344 + 61,511,649·n → 22,277,584 + 65,935,671·n` | +4.25% | `1,489 + 11,210·n → 1,489 + 11,210·n` | `3 + 10·n → 3 + 10·n` | `2 + 7·n → 2 + 7·n` | C |
| Actors | `fee_collection` | `89,119,000 → 43,302,000` | -51.41% | `12,200 → 3,593` | `9 → 1` | `1 → 1` | O |
| Actors | `task_transfer` | `214,067,000 → 224,683,000` | +4.96% | `12,200 → 12,200` | `18 → 18` | `8 → 8` | C |
| Actors | `task_burn` | `19,486,000 → 19,067,000` | -2.15% | `3,593 → 3,593` | `1 → 1` | `1 → 1` | C |
| Actors | `task_mint` | `158,263,000 → 188,295,000` | +18.98% | `12,200 → 12,200` | `16 → 16` | `6 → 6` | C |
| Actors | `predicate_set_evaluation` | `7,613,000 + 6,706,886·c → 7,543,000 + 6,463,193·c` | -0.92% | `3,675 + 673·c → 3,675 + 674·c` | `1 + 1·c → 1 + 1·c` | `0 → 0` | C |
| Actors | `task_stop_cycle` | `4,261,000 → 4,260,000` | -0.02% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `task_split_transfer` | `89,411,548 + 130,228,663·l → 84,190,134 + 146,914,546·l` | -5.84% | `8,040 + 11,210·l → 8,040 + 11,210·l` | `10 + 7·l → 10 + 7·l` | `4 + 3·l → 4 + 3·l` | C |
| Actors | `xcm_asset_deposit` | `199,679,000 → 209,457,000` | +4.90% | `12,200 → 12,200` | `19 → 19` | `8 → 8` | C |
| Actors | `task_add_liquidity` | `282,163,000 → 279,020,000` | -1.11% | `34,255 → 34,255` | `16 → 16` | `15 → 15` | C |
| Actors | `task_donate_liquidity` | `168,809,000 → 168,040,000` | -0.46% | `14,035 → 14,035` | `9 → 9` | `8 → 8` | C |
| Actors | `task_remove_liquidity` | `159,799,000 → 157,564,000` | -1.40% | `8,817 → 8,817` | `8 → 8` | `6 → 6` | C |
| Actors | `task_stake` | `87,722,000 → 85,906,000` | -2.07% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_unstake` | `103,925,000 → 101,761,000` | -2.08% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Actors | `task_dex_exact_in` | `531,920,000 → 511,176,000` | -3.90% | `19,253 → 19,253` | `38 → 38` | `17 → 17` | O |
| Actors | `task_dex_exact_out` | `528,358,000 → 497,626,000` | -5.82% | `19,253 → 19,253` | `37 → 37` | `17 → 17` | O |
| Actors | `scheduler_on_idle_base` | `14,806,000 → 14,667,000` | -0.94% | `1,543 → 1,543` | `6 → 6` | `1 → 1` | C |
| Actors | `scheduler_actor_state_probe` | `— → 37,296,000` | new | `— → 12,200` | `— → 5` | `— → 0` | M |
| Actors | `cycle_orchestration` | `46,865,000 → 50,566,000` | +7.90% | `12,200 → 12,200` | `4 → 5` | `3 → 3` | C |
| Actors | `step_orchestration` | `46,425,674 + 177,402·n → 51,029,636 + 142,952·n` | +9.92% | `12,200 → 12,200` | `4 → 5` | `3 → 3` | C |
| Actors | `scheduler_paged_append_existing_page` | `55,804,000 → 81,925,000` | +46.81% | `6,472 → 7,938` | `7 → 10` | `5 → 5` | C |
| Actors | `scheduler_paged_append_new_page` | `53,500,000 → 83,183,000` | +55.48% | `8,888 → 10,283` | `8 → 11` | `5 → 5` | C |
| Actors | `scheduler_wakeup_append_existing_page` | `43,233,000 → 86,325,000` | +99.67% | `4,937 → 6,694` | `4 → 8` | `3 → 3` | C |
| Actors | `scheduler_wakeup_append_new_page` | `44,979,000 → 80,389,000` | +78.73% | `4,980 → 6,833` | `4 → 8` | `4 → 4` | C |
| Actors | `scheduler_wakeup_replace_exact` | `67,677,000 → 98,897,000` | +46.13% | `6,795 → 7,492` | `6 → 10` | `7 → 7` | C |
| Actors | `scheduler_wakeup_invalidate_middle_page` | `59,576,000 → 91,563,000` | +53.69% | `10,258 → 12,495` | `6 → 10` | `5 → 5` | C |
| Actors | `scheduler_wakeup_drain_partial_page` | `132,491,000 → 348,723,000` | +163.21% | `43,030 → 47,750` | `19 → 83` | `18 → 18` | C |
| Actors | `scheduler_wakeup_drain_full_page` | `235,649,000 → 658,893,000` | +179.61% | `83,308 → 90,034` | `36 → 164` | `36 → 36` | C |
| Actors | `scheduler_wakeup_drain_dense_boundary` | `263,725,000 → 688,785,000` | +161.18% | `85,914 → 92,825` | `38 → 170` | `38 → 38` | C |
| Actors | `scheduler_wakeup_drain_stale_page` | `159,590,000 → 569,774,000` | +257.02% | `82,700 → 89,426` | `36 → 164` | `4 → 4` | C |
| Actors | `scheduler_wakeup_cursor_insert` | `369,955,000 → 363,181,000` | -1.83% | `42,767 → 42,733` | `25 → 25` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_pop_min` | `485,265,000 → 452,927,000` | -6.66% | `55,259 → 55,259` | `34 → 34` | `26 → 26` | C |
| Actors | `scheduler_wakeup_cursor_remove_exact` | `441,194,000 → 424,363,000` | -3.81% | `54,726 → 54,726` | `33 → 33` | `25 → 25` | C |
| Actors | `scheduler_wakeup_cursor_worker_partial` | `79,550,000 → 112,865,000` | +41.88% | `6,925 → 7,498` | `13 → 14` | `8 → 8` | C |
| Actors | `scheduler_wakeup_cursor_worker_remove` | `547,425,000 → 556,364,000` | +1.63% | `58,698 → 56,434` | `46 → 48` | `33 → 33` | C |
| Actors | `scheduler_wakeup_cursor_worker_future` | `20,533,000 → 20,464,000` | -0.34% | `6,523 → 6,523` | `5 → 5` | `0 → 0` | C |
| Actors | `scheduler_paged_consume_preserve_page` | `26,261,000 → 49,029,000` | +86.70% | `4,120 → 4,868` | `5 → 9` | `3 → 3` | C |
| Actors | `scheduler_paged_consume_delete_page` | `27,169,000 → 50,077,000` | +84.32% | `4,102 → 4,846` | `5 → 9` | `5 → 5` | C |
| Actors | `scheduler_paged_tombstone_drain` | `26,191,000 + 1,928,435·n → 43,861,000 + 9,799,965·n` | +67.47% | `2,984 + 2,492·n → 4,049 + 2,492·n` | `5 + 1·n → 5 + 5·n` | `4 → 4` | C |
| Actors | `scheduler_paged_mixed_scan` | `29,473,000 + 21,038,253·n → 46,795,000 + 41,230,319·n` | +58.77% | `3,981 + 2,548·n → 4,857 + 2,608·n` | `4 + 2·n → 5 + 5·n` | `3 + 1·n → 3 + 1·n` | C |
| Actors | `scheduler_paged_execute_cheap` | `107,278,000 + 83,857,632·n → 117,684,000 + 96,167,844·n` | +9.70% | `3,716 + 2,733·n → 3,716 + 2,733·n` | `6 + 5·n → 6 + 5·n` | `4 + 3·n → 4 + 3·n` | C |
| Actors | `scheduler_paged_execute_cheap_mixed` | `261,421,000 + 121,247,854·n → 250,944,000 + 122,945,961·n` | -4.01% | `6,390 + 2,798·n → 4,918 + 2,798·n` | `13 + 6·n → 6 + 6·n` | `4 + 4·n → 4 + 4·n` | C |
| Actors | `continuation_suspend` | `32,219,709 + 32,099·s → 40,184,544 + 33,216·s` | +24.72% | `4,615 → 4,728` | `3 → 5` | `2 → 2` | C |
| Actors | `continuation_retry` | `20,324,000 → 19,975,000` | -1.72% | `4,283 → 4,283` | `1 → 1` | `1 → 1` | C |
| Actors | `continuation_complete` | `23,746,000 → 40,648,000` | +71.18% | `4,445 → 5,154` | `2 → 5` | `2 → 2` | C |
| Actors | `continuation_cancel` | `80,528,000 → 188,435,000` | +134.00% | `12,200 → 12,200` | `10 → 16` | `7 → 11` | C |
| Actors | `continuation_suffix_admission` | `1,431,500 + 358·n → 1,427,809 + 607·n` | -0.26% | `0 → 0` | `0 → 0` | `0 → 0` | C |
| Actors | `observation_change_ingress` | `35,201,000 → 33,385,000` | -5.16% | `6,128 → 6,128` | `5 → 5` | `4 → 4` | C |
| Actors | `observation_fanout_base` | `6,146,000 → 6,076,000` | -1.14% | `1,543 → 1,543` | `1 → 1` | `0 → 0` | C |
| Actors | `observation_fanout_page` | `1,292,713,000 → 1,898,176,000` | +46.84% | `166,430 → 718,430` | `139 → 331` | `72 → 72` | C |
| Actors | `transaction_extension_ingress_base` | `13,480,000 → 13,410,000` | -0.52% | `6,052 → 6,052` | `2 → 2` | `0 → 0` | C |
| Actors | `transaction_extension_ingress_notify` | `173,559,000 → 197,444,000` | +13.76% | `12,200 → 12,200` | `15 → 15` | `6 → 6` | C |
| Actors | `funding_snapshot_open` | `13,476,730 + 128,476·a → 13,386,014 + 122,508·a` | -0.67% | `3,751 → 3,751` | `1 → 1` | `1 → 1` | C |
| Governance | `record_winning_vote` | `29,404,000 → 29,124,000` | -0.95% | `40,351 → 40,351` | `4 → 4` | `4 → 4` | C |
| Governance | `record_winning_vote_batch` | `29,124,000 + 16,598,408·n → 29,543,000 + 16,277,412·n` | +1.44% | `40,351 + 2,603·n → 40,351 + 2,603·n` | `2 + 2·n → 2 + 2·n` | `2 + 2·n → 2 + 2·n` | C |
| Governance | `submit_proposal` | `528,637,000 → 549,869,000` | +4.02% | `321,912 → 321,912` | `130 → 130` | `6 → 6` | C |
| Governance | `submit_signed_proposal` | `607,560,000 → 630,049,000` | +3.70% | `4,197,809 → 4,197,809` | `138 → 138` | `7 → 7` | C |
| Governance | `cast_vote` | `1,124,323,000 → 1,249,760,000` | +11.16% | `656,094 → 656,094` | `269 → 269` | `271 → 271` | C |
| Governance | `unlock_vote_power` | `68,166,000 → 67,817,000` | -0.51% | `6,208 → 6,208` | `5 → 5` | `5 → 5` | C |
| Governance | `resolve_proposal` | `80,877,000 + 16,554,347·n → 78,643,000 + 16,543,654·n` | -2.76% | `96,703 + 2,603·n → 96,703 + 2,603·n` | `10 + 2·n → 10 + 2·n` | `14 + 2·n → 14 + 2·n` | C |
| Governance | `resolve_proposal_from_votes` | `105,245,341 + 59,043·n → 106,671,658 + 1,037,471·n` | +1.36% | `96,703 → 96,703` | `8 → 8` | `10 → 10` | C |
| Governance | `reject_proposal` | `40,020,000 → 39,600,000` | -1.05% | `11,679 → 11,679` | `4 → 4` | `10 → 10` | C |
| Governance | `force_resolve_proposal_from_votes` | `95,016,040 + 58,166·n → 94,216,612 + 1,004,803·n` | -0.84% | `96,703 → 96,703` | `8 → 8` | `10 → 10` | C |
| Governance | `requeue_proposal_for_auto_finalization` | `17,740,000 → 17,810,000` | +0.39% | `3,518 → 3,518` | `2 → 2` | `1 → 1` | C |
| Governance | `service_epoch_catch_up` | `— → 14,109,000` | new | `— → 40,351` | `— → 6` | `— → 1` | P |
| Governance | `service_maturing_proposals` | `4,088,436,000 + 3,859,989,227·n → 4,256,477,000 + 2,500,144,905·n` | +4.11% | `79,712 + 666,368·n → 40,351 + 666,368·n` | `11 + 519·n → 7 + 519·n` | `7 + 521·n → 6 + 521·n` | P |
| Governance | `service_pending_enactments` | `— → 6,733,859 + 13,019,956·n` | new | `— → 6,046 + 2,634·n` | `— → 2 + 3·n` | `— → 2` | P |
| Governance | `service_finalized_proposal_outcomes` | `22,979,000 + 8,375,858·n → 15,366,000 + 8,206,051·n` | -33.13% | `40,351 → 11,679` | `5 → 1` | `2 + 5·n → 1 + 5·n` | P |
| Governance | `service_expiring_accounts` | `24,375,000 + 7,867,328·n → 16,413,000 + 7,035,354·n` | -32.66% | `40,351 + 2,603·n → 40,351 + 2,603·n` | `5 + 1·n → 1 + 1·n` | `2 + 1·n → 1 + 1·n` | P |
| Oracle | `register_feed_existing_producer` | `147,786,000 → 132,142,000` | -10.59% | `20,532 → 20,532` | `3 → 3` | `3 → 3` | C |
| Oracle | `register_feed_new_producer` | `210,575,000 → 194,720,000` | -7.53% | `44,394 → 44,394` | `4 → 4` | `4 → 4` | C |
| Oracle | `pause_feed` | `21,372,000 → 15,365,000` | -28.11% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `resume_feed` | `19,207,000 → 15,365,000` | -20.00% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `deactivate_feed` | `18,997,000 → 15,365,000` | -19.12% | `3,551 → 3,551` | `1 → 1` | `1 → 1` | C |
| Oracle | `publish_last_value` | `36,318,000 → 30,033,000` | -17.31% | `3,559 → 3,559` | `5 → 5` | `1 → 1` | C |
| Oracle | `publish_ema_changed` | `40,020,000 → 33,385,000` | -16.58% | `3,559 → 3,559` | `5 → 5` | `1 → 1` | C |
| Oracle | `publish_ema_refresh` | `26,959,000 → 21,931,000` | -18.65% | `3,551 → 3,551` | `2 → 2` | `1 → 1` | C |
| Router | `direct_xyk_exact_input` | `302,068,000 → 294,176,000` | -2.61% | `13,998 → 12,200` | `25 → 25` | `12 → 12` | C |
| Router | `direct_mint_exact_input` | `313,452,000 → 315,129,000` | +0.54% | `27,006 → 23,410` | `32 → 33` | `14 → 14` | C |
| Router | `native_anchored_exact_input` | `440,496,000 → 435,468,000` | -1.14% | `19,253 → 19,253` | `36 → 36` | `17 → 17` | C |
| Router | `direct_xyk_exact_output` | `164,340,000 → 164,898,000` | +0.34% | `6,208 → 6,208` | `10 → 10` | `5 → 5` | C |
| Router | `native_anchored_exact_output` | `304,233,000 → 302,348,000` | -0.62% | `16,644 → 16,644` | `21 → 21` | `10 → 10` | C |
| Router | `update_router_fee` | `8,660,000 → 8,591,000` | -0.80% | `1,489 → 1,489` | `1 → 1` | `1 → 1` | C |
| Staking | `register_staking_asset` | `58,807,000 → 58,179,000` | -1.07% | `6,360 → 6,360` | `8 → 8` | `4 → 4` | C |
| Staking | `sync_pool` | `25,772,000 → 25,283,000` | -1.90% | `3,599 → 3,599` | `2 → 2` | `1 → 1` | C |
| Staking | `stake` | `91,074,000 → 89,678,000` | -1.53% | `8,817 → 8,817` | `7 → 7` | `7 → 7` | C |
| Staking | `unstake` | `97,988,000 → 94,916,000` | -3.14% | `8,817 → 8,817` | `6 → 6` | `6 → 6` | C |
| Staking | `recover_unowned_pool` | `72,357,000 → 71,169,000` | -1.64% | `6,208 → 6,208` | `6 → 6` | `6 → 6` | C |
| Staking | `lock_native_lp_for_collator` | `102,180,000 → 99,107,000` | -3.01% | `6,208 → 6,208` | `12 → 12` | `7 → 7` | C |
| Staking | `request_unlock_native_lp` | `55,385,000 → 54,896,000` | -0.88% | `4,687 → 4,687` | `8 → 8` | `5 → 5` | C |
| Staking | `withdraw_unlocked_native_lp` | `73,753,000 → 71,588,000` | -2.94% | `6,208 → 6,208` | `5 → 5` | `5 → 5` | C |
| Staking | `redelegate_native_lp` | `51,334,000 → 50,426,000` | -1.77% | `6,172 → 6,172` | `7 → 7` | `5 → 5` | C |
| Staking | `lock_native_lp_for_governance` | `82,135,000 → 80,318,000` | -2.21% | `6,208 → 6,208` | `8 → 8` | `6 → 6` | C |
| Staking | `request_unlock_native_lp_for_governance` | `35,270,000 → 35,270,000` | 0.00% | `3,537 → 3,537` | `5 → 5` | `4 → 4` | C |
| Staking | `withdraw_unlocked_native_lp_for_governance` | `67,468,000 → 66,770,000` | -1.03% | `6,208 → 6,208` | `5 → 5` | `5 → 5` | C |
| Staking | `lock_native_asset_for_governance` | `75,849,000 → 74,522,000` | -1.75% | `6,208 → 6,208` | `6 → 6` | `5 → 5` | C |
| Staking | `request_unlock_native_asset_for_governance` | `31,010,000 → 31,010,000` | 0.00% | `3,557 → 3,557` | `4 → 4` | `3 → 3` | C |
| Staking | `withdraw_unlocked_native_asset_for_governance` | `68,934,000 → 68,166,000` | -1.11% | `6,208 → 6,208` | `5 → 5` | `5 → 5` | C |
| Staking | `fund_native_security_reward` | `73,893,000 → 72,217,000` | -2.27% | `16,309 → 16,309` | `7 → 7` | `4 → 4` | C |
| Staking | `claim_native_security_reward` | `76,826,000 → 75,849,000` | -1.27% | `16,309 → 16,309` | `7 → 7` | `5 → 5` | C |
| Staking | `claim_native_security_reward_batch` | `58,582,140 + 15,662,697·c → 56,334,094 + 14,954,006·c` | -3.84% | `6,196 + 15,319·c → 6,196 + 15,319·c` | `4 + 3·c → 4 + 3·c` | `3 + 2·c → 3 + 2·c` | C |
| Staking | `claim_and_compound_native_security_reward` | `383,714,000 → 380,082,000` | -0.95% | `19,253 → 19,253` | `26 → 26` | `22 → 22` | C |
| Staking | `expire_native_security_reward` | `207,921,000 → 200,378,000` | -3.63% | `255,290 → 255,290` | `105 → 105` | `105 → 105` | C |
| Staking | `settle_due_native_security_reward` | `200,412,877 + 2,930,207·r → 187,340,522 + 3,228,032·r` | -6.52% | `254,580 + 2,551·r → 254,580 + 2,551·r` | `105 + 1·r → 105 + 1·r` | `105 → 105` | C |
| Staking | `cancel_native_security_epoch_plan` | `9,988,000 → 9,848,000` | -1.40% | `3,534 → 3,534` | `1 → 1` | `2 → 2` | C |
| Staking | `contract_native_security_obligations` | `23,537,000 → 23,258,000` | -1.19% | `14,309 → 14,309` | `4 → 4` | `4 → 4` | C |
| Staking | `open_native_security_epoch` | `164,401,427 + 44,891,248·p → 48,059,040 + 43,698,274·p + 1,804,670·r` | -70.77% | `11,052 + 2,854·p + 2,597·r → 11,052 + 2,854·p + 2,597·r` | `13 + 4·p + 1·r → 13 + 4·p + 1·r` | `2 → 2` | C |
| TMC | `create_curve` | `26,680,000 → 26,680,000` | 0.00% | `6,360 → 6,360` | `3 → 3` | `1 → 1` | C |
| TMC | `mint_with_distribution` | `154,771,000 → 164,130,000` | +6.05% | `13,998 → 12,200` | `10 → 11` | `4 → 4` | C |

## Interpretation

The large scheduler wakeup, page-drain, fanout, and Continuation completion increases are accepted correctness costs rather than regressions on unchanged semantics. The prior paths probed only hot, contract, or selected Continuation partitions; the candidate charges the complete five-partition canonical actor-state classification and corruption rejection on every affected branch. `continuation_cancel` additionally measures exact middle-page wakeup invalidation before a retained pending signal is re-primed, preventing a stale physical slot from conflicting with the new live pointer. Governance service increases similarly pay for chronological phased progress, retained same-epoch suffixes, aggregate custody reconciliation, and checked arithmetic. No database or ProofSize increase is hidden inside a RefTime percentage.

The measured optimization requirement is satisfied independently in multiple production paths. Ledger-only fee collection removes queue signaling and cuts base RefTime by 51.25%. Fresh independent Oracle observations skip duplicate reserve lookup, reducing exact-input Router task RefTime by 6.03% and exact-output by 6.93% in this tag-to-candidate ledger while preserving identical ProofSize and database envelopes. Canonical loaded-state carry also removes repeated actor-state reads from live-head execution; owning Actors architecture evidence records the matched slope comparison.

Asset Registry coefficients are unchanged or lower apart from run minima comments, so the host-reserved `Here` rejection adds no database access. Governance base coefficients without a service-topology change remain within 4.11% except `cast_vote`; its 11.16% increase is explained by checked aggregate custody and replacement-vote reconciliation. The larger per-ballot slopes for `resolve_proposal_from_votes` and its force variant pay for checked tally folds and typed overflow instead of saturating vote totals. The final Oracle refresh lowers every base coefficient without increasing database or ProofSize envelopes. Router direct mint adds one read and 0.54% base RefTime for the independent reference guard. TMC distribution adds one read and 6.05% base RefTime to prevalidate the checked cumulative native-mint total before any mint; the four writes remain unchanged. Staking paths are lower or unchanged at the base except that retained-epoch work is now represented by explicit RefTime slopes in epoch opening/settlement rather than hidden in a fixed coefficient. The candidate rejects any future unexplained positive delta: regenerate this file, inspect each formula and storage annotation, and update semantics or code rather than accepting benchmark noise by default.

## Retired Weight Owners

- Actors `scheduler_actor_hot_probe`
- Actors `scheduler_actor_contract_probe`

The two retired Actors partition probes are replaced by `scheduler_actor_state_probe`; no runtime binding retains either partial owner.

## Reproduction

- Regenerate: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh`
- Verify freshness: `./.agents/skills/release-assurance/scripts/weight-delta-ledger.sh --check`
- Reproduce production weights through `./scripts/benchmarks.sh` and the owning Benchmarking Skill; focused outputs do not replace complete generated pallet files.

Candidate weight source identity: `0aa3a73a81811f4a9cdf5844fab3e6430741a57699c9a0f3e4efa320806eaca6`.

