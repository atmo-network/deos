#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

WEIGHTS_DIR="$TEMPLATE_DIR/runtime/src/weights"
STEPS=50
REPEAT=20
HEAP_PAGES=4096
CHAIN="dev"
INCLUDE_EXTRA_BENCHMARKS=0
PALLETS=(
    "pallet_aaa"
    "pallet_axial_router"
    "pallet_tmc"
    "pallet_burning_manager"
    "pallet_zap_manager"
    "pallet_treasury_owned_liquidity"
    "pallet_asset_registry"
    "pallet_governance"
    "pallet_staking"
    "pallet_xcm"
)

BENCHER_MODE=""
ACTION=""
TARGET_PALLET=""

usage() {
    cat <<EOF2
Usage: $(basename "$0") [OPTIONS] [PALLET_NAME]

Run benchmarks and generate weight files for the current DEOS reference runtime pallets.

Options:
  --steps N       Number of steps per benchmark (default: $STEPS)
  --repeat N      Number of repetitions per benchmark (default: $REPEAT)
  --all           Benchmark all custom pallets
  --list          List available pallets
  --check         Only verify benchmarks compile (no execution)
  --extra         Include AAA circular-chain diagnostic benchmarks excluded from production weights
  -h, --help      Show this help message

Arguments:
  PALLET_NAME     Specific pallet to benchmark (e.g., pallet_axial_router)
                  If omitted and --all not set, the script exits with usage.

Examples:
  $(basename "$0") --all                      # Benchmark all pallets
  $(basename "$0") pallet_axial_router        # Benchmark one pallet
  $(basename "$0") --check                    # Verify compilation only
  $(basename "$0") --extra pallet_aaa         # Include AAA circular-chain diagnostics
  $(basename "$0") --steps 100 --repeat 50 --all  # Production-quality run

Environment:
  INCLUDE_EXTRA_BENCHMARKS=0|1
EOF2
    exit 0
}

parse_args() {
    ACTION=""
    TARGET_PALLET=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --steps)
                STEPS="$2"
                shift 2
                ;;
            --repeat)
                REPEAT="$2"
                shift 2
                ;;
            --all)
                ACTION="all"
                shift
                ;;
            --list)
                ACTION="list"
                shift
                ;;
            --check)
                ACTION="check"
                shift
                ;;
            --extra)
                INCLUDE_EXTRA_BENCHMARKS=1
                shift
                ;;
            -h|--help)
                usage
                ;;
            *)
                TARGET_PALLET="$1"
                shift
                ;;
        esac
    done
}

check_prerequisites() {
    phase_banner "Step 1: Benchmark prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    require_directory "$WEIGHTS_DIR" "Runtime weights directory"
    hydrate_local_tool_paths
    require_commands cargo sed grep wc sort awk head

    if ! command -v frame-omni-bencher &>/dev/null; then
        log_warning "frame-omni-bencher not found. Install with:"
        echo "  cargo install --locked frame-omni-bencher --tag polkadot-v1.22.3"
        echo ""
        log_info "Falling back to 'cargo test --features runtime-benchmarks' mode"
        BENCHER_MODE="cargo"
    else
        local bencher_version minimum_bencher_version
        bencher_version="$(frame-omni-bencher --version | awk '{print $2}')"
        minimum_bencher_version="0.22.0"
        if [[ "$(printf '%s\n%s\n' "$minimum_bencher_version" "$bencher_version" | sort -V | head -n1)" != "$minimum_bencher_version" ]]; then
            log_error "frame-omni-bencher $bencher_version is incompatible with the current SDK; install >= $minimum_bencher_version"
            return 1
        fi
        BENCHER_MODE="omni"
        log_success "frame-omni-bencher $bencher_version found"
    fi
}

build_benchmarks() {
    phase_banner "Step 2: Build benchmark runtime"
    run_shell_step \
        "Build deos-runtime with runtime-benchmarks" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo build --release --features runtime-benchmarks -p deos-runtime"
}

check_only() {
    phase_banner "Step 2: Benchmark compilation check"
    run_shell_step \
        "Verify benchmark compilation" \
        "" \
        "cd \"$TEMPLATE_DIR\" && cargo check --features runtime-benchmarks"
    log_success "All benchmarks compile successfully"
}

resolve_runtime_wasm_path() {
    local candidates=(
        "$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm"
    )

    for candidate in "${candidates[@]}"; do
        if [[ -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    log_error "Benchmark runtime WASM not found in known wbuild output paths"
    return 1
}

# frame-omni-bencher generates files with bare `frame_system`/`frame_support` imports
# and `WeightInfo<T>` struct names. Normalize to project conventions.
normalize_weight_file() {
    local file="$1"
    sed -i 's/use frame_support::/use polkadot_sdk::frame_support::/g' "$file"
    sed -i 's/pub struct WeightInfo/pub struct SubstrateWeight/' "$file"
    sed -i 's/impl<T: frame_system::Config>/impl<T: polkadot_sdk::frame_system::Config>/' "$file"
    sed -i 's/for WeightInfo<T>/for SubstrateWeight<T>/' "$file"
    sed -i 's/ pallet_xcm::WeightInfo/ polkadot_sdk::pallet_xcm::WeightInfo/' "$file"
    sed -i "s#${TEMPLATE_DIR}#template#g" "$file"
    log_info "  Normalized imports, struct name, and local paths"
}

verify_weight_file_contract() {
    local pallet_name="$1"
    local output_file="$2"

    if [[ "$pallet_name" != "pallet_aaa" ]]; then
        return 0
    fi

    local benchmark_file="$TEMPLATE_DIR/pallets/aaa/src/benchmarking.rs"
    local diagnostic_benchmarks=(
        "process_remove_liquidity_max_k"
        "scheduler_cooldown_ineligible_idle"
        "scheduler_wakeup_sparse_gap_recovery"
        "close_aaa_on_close_execution_plan_complex"
    )

    for benchmark in "${diagnostic_benchmarks[@]}"; do
        if ! grep -q "fn ${benchmark}" "$benchmark_file"; then
            continue
        fi
        if ! grep -q -- '--exclude-extrinsics' "$output_file" || ! grep -q "pallet_aaa::${benchmark}" "$output_file"; then
            log_error "Weight file contract check failed for pallet_aaa: missing exclude marker for ${benchmark}"
            return 1
        fi
    done

    local required_runtime_benchmarks=(
        "scheduler_actor_probe"
        "scheduler_queue_bootstrap"
        "scheduler_paged_append_existing_page"
        "scheduler_paged_append_new_page"
        "scheduler_paged_consume_preserve_page"
        "scheduler_paged_consume_delete_page"
        "scheduler_wakeup_dense_due_drain"
        "compatibility_ingress_drain"
        "transaction_extension_ingress_base"
        "transaction_extension_ingress_notify"
    )
    for benchmark in "${required_runtime_benchmarks[@]}"; do
        if ! grep -q "fn ${benchmark}" "$output_file"; then
            log_error "Weight file contract check failed for pallet_aaa: missing generated ${benchmark}"
            return 1
        fi
    done

    if grep -q 'fn compatibility_ingress_scan_notify' "$output_file"; then
        log_error "Weight file contract check failed for pallet_aaa: retired event-vector scan weight remains"
        return 1
    fi

    if ! grep -q 'The range of component `n` is `\[1, 5\]`.' "$output_file"; then
        log_error "Weight file contract check failed for pallet_aaa: permissionless_sweep_many must cover MaxSweepPerBlock=5"
        return 1
    fi

    if ! grep -q 'Storage: `AssetConversion::Pools` (r:1 w:1)' "$output_file" \
        || ! grep -q 'Storage: `AssetConversion::NextPoolAssetId` (r:1 w:1)' "$output_file"; then
        log_error "Weight file contract check failed for pallet_aaa: task_add_liquidity must cover missing-pool creation"
        return 1
    fi
}

run_pallet_benchmark() {
    local pallet_name="$1"
    local output_file="$WEIGHTS_DIR/${pallet_name}.rs"
    local exclude_args=()

    if [[ "$pallet_name" == "pallet_aaa" ]]; then
        exclude_args=(
            --exclude-extrinsics "pallet_aaa::process_remove_liquidity_max_k"
            --exclude-extrinsics "pallet_aaa::scheduler_cooldown_ineligible_idle"
            --exclude-extrinsics "pallet_aaa::scheduler_wakeup_sparse_gap_recovery"
            --exclude-extrinsics "pallet_aaa::close_aaa_on_close_execution_plan_complex"
        )

        if [[ "$INCLUDE_EXTRA_BENCHMARKS" != "1" ]]; then
            exclude_args+=(
                --exclude-extrinsics "pallet_aaa::circular_chain_stress"
                --exclude-extrinsics "pallet_aaa::circular_chain_stress_100k"
                --exclude-extrinsics "pallet_aaa::circular_chain_100"
                --exclude-extrinsics "pallet_aaa::circular_chain_1000"
                --exclude-extrinsics "pallet_aaa::circular_chain_10000"
            )
        fi
    fi

    log_info "Benchmarking: $pallet_name (steps=$STEPS, repeat=$REPEAT)"

    if [[ "$BENCHER_MODE" == "omni" ]]; then
        local template_file="$TEMPLATE_DIR/.maintain/frame-weight-template.hbs"
        local runtime_wasm
        runtime_wasm="$(resolve_runtime_wasm_path)" || return 1
        local bencher_args=(
            --runtime "$runtime_wasm"
            --pallet "$pallet_name"
            --extrinsic "*"
            "${exclude_args[@]}"
            --steps "$STEPS"
            --repeat "$REPEAT"
            --heap-pages "$HEAP_PAGES"
            --output "$output_file"
        )

        if [[ "$INCLUDE_EXTRA_BENCHMARKS" == "1" ]]; then
            bencher_args+=(--extra)
        fi

        if [[ -f "$template_file" ]]; then
            bencher_args+=(--template "$template_file")
        fi

        frame-omni-bencher v1 benchmark pallet "${bencher_args[@]}" 2>&1
    else
        log_warning "Running benchmark tests (dry run without weight generation)"
        if [[ "$INCLUDE_EXTRA_BENCHMARKS" == "1" ]]; then
            log_warning "--extra requires frame-omni-bencher for actual extra-benchmark execution; cargo fallback remains compile-only"
        fi
        cd "$TEMPLATE_DIR"
        cargo test --release --features runtime-benchmarks -p deos-runtime -- "benchmark" --nocapture 2>&1 || true
        log_warning "Weight files NOT updated (frame-omni-bencher required for weight generation)"
        return 0
    fi

    if [[ -f "$output_file" ]]; then
        normalize_weight_file "$output_file"
        verify_weight_file_contract "$pallet_name" "$output_file"
        log_success "$pallet_name -> $output_file"
    else
        log_error "Weight file not generated for $pallet_name"
        return 1
    fi
}

run_all_benchmarks() {
    local failed=0
    local succeeded=0
    local start_time
    local end_time
    local total_duration

    log_info "Running benchmarks for ${#PALLETS[@]} pallets..."

    start_time=$(date +%s)
    for pallet in "${PALLETS[@]}"; do
        if run_pallet_benchmark "$pallet"; then
            ((succeeded++))
        else
            ((failed++))
            log_error "Failed: $pallet"
        fi
        echo ""
    done
    end_time=$(date +%s)
    total_duration=$((end_time - start_time))

    phase_banner "Summary"
    echo "  Succeeded: $succeeded / ${#PALLETS[@]}"
    echo "  Failed:    $failed / ${#PALLETS[@]}"
    echo "  Duration:  ${total_duration}s"
    echo "  Steps:     $STEPS"
    echo "  Repeat:    $REPEAT"

    if [[ $failed -gt 0 ]]; then
        log_error "Some benchmarks failed"
        exit 1
    fi

    log_success "All benchmarks completed successfully"
}

list_pallets() {
    phase_banner "Available benchmark pallets"
    echo "Available pallets for benchmarking:"
    for pallet in "${PALLETS[@]}"; do
        local weight_file="$WEIGHTS_DIR/${pallet}.rs"
        if [[ -f "$weight_file" ]]; then
            echo "  - $pallet (weights: $(wc -l < "$weight_file") lines)"
        else
            echo "  - $pallet (no weight file)"
        fi
    done
}

is_known_pallet() {
    local candidate="$1"
    local pallet

    for pallet in "${PALLETS[@]}"; do
        if [[ "$pallet" == "$candidate" ]]; then
            return 0
        fi
    done
    return 1
}

main() {
    parse_args "$@"
    phase_banner "DEOS benchmark workflow"

    if [[ "$ACTION" == "list" ]]; then
        list_pallets
        exit 0
    fi

    if [[ "$ACTION" == "check" ]]; then
        check_only
        exit 0
    fi

    check_prerequisites

    if [[ "$BENCHER_MODE" == "omni" ]]; then
        build_benchmarks
    fi

    if [[ -n "$TARGET_PALLET" ]]; then
        if ! is_known_pallet "$TARGET_PALLET"; then
            log_error "Unknown pallet: $TARGET_PALLET"
            echo ""
            list_pallets
            exit 1
        fi
        phase_banner "Step 3: Run pallet benchmark"
        run_pallet_benchmark "$TARGET_PALLET"
    elif [[ "$ACTION" == "all" ]]; then
        phase_banner "Step 3: Run pallet benchmark suite"
        run_all_benchmarks
    else
        log_error "Specify a pallet name or use --all"
        echo ""
        usage
    fi
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
