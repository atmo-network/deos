#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

# Pin the published executable bundle independently of source/worker qualification.
POLKADOT_SDK_RELEASE="polkadot-stable2609-2"
REPOSITORY="paritytech/polkadot-sdk"
BASE_URL="https://github.com/$REPOSITORY/releases/download/$POLKADOT_SDK_RELEASE"
BINARY_NAMES=(
    polkadot
    polkadot-omni-node
    polkadot-prepare-worker
    polkadot-execute-worker
    frame-omni-bencher
)
ASSET_SUFFIX=""
CHECKSUMS=()
CHECKSUM_COMMAND=()
CHECK_ONLY=0
SELECTED_INDICES=()
SELF_TEST=0
OMNI_NODE_PATH="${DEOS_OMNI_NODE_PATH:-}"
OMNI_NODE_SHA256="${DEOS_OMNI_NODE_SHA256:-}"
BINARY_STAGING_DIR=""

usage() {
    cat <<'EOF'
Usage: 01-download-binaries.sh [OPTIONS]

Downloads the pinned Polkadot SDK executable bundle required by DEOS operations.

Options:
  --check           Verify the existing selection without downloading
  --only NAME       Select one pinned binary; repeat for a subset (default: all)
  --self-test       Test selection/publication/checksum contracts offline
  -h, --help        Show this help message

Environment:
  DEOS_BINARY_DIR=<directory>  Bundle and selected-node output directory
  DEOS_OMNI_NODE_PATH=<path>   Optional independently qualified source-built Omni
  DEOS_OMNI_NODE_SHA256=<sha>  Required expected digest when selecting that node

Inputs:
  Network access to the pinned paritytech/polkadot-sdk release when the local
  bundle is absent or checksum-mismatched, plus a supported host platform.

Authority:
  GitHub release: paritytech/polkadot-sdk polkadot-stable2609-2
  Workspace source: polkadot-stable2609-2; local corrections remain separate
  Compatibility: DEOS workspace Polkadot SDK 2609 package line; worker/node
  qualification is separate from download/checksum verification
  Supported hosts: Linux x86_64 and macOS arm64 (checksum-pinned release assets)

Outputs:
  ./bin/polkadot
  ./bin/polkadot-omni-node
  ./bin/polkadot-prepare-worker
  ./bin/polkadot-execute-worker
  ./bin/frame-omni-bencher
  ./bin/.polkadot-sdk-release only when the complete bundle is published
  ./bin/polkadot-omni-node-deos and ./bin/.deos-omni-node.json when selected

Side effects:
  Downloads only missing or checksum-mismatched assets, verifies every pinned
  SHA-256 digest before publication, and replaces only the selected files.
  Partial selection never writes a full-bundle marker or qualifies unselected
  files. --check without --only still requires the complete marked bundle.
  Names: polkadot, polkadot-omni-node, polkadot-prepare-worker,
  polkadot-execute-worker, frame-omni-bencher. Corrected-node environment
  selection requires a complete bundle; it is rejected in partial mode.
  Optional node selection atomically copies an operator-supplied artifact and
  records its digest/SDK line; it rejects the known unpatched release Omni.
  These identity checks do not establish the supplied node's source correction,
  caller qualification or published-build-profile equivalence.
EOF
}

parse_args() {
    local argument_count=$# index found
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --check)
                CHECK_ONLY=1
                ;;
            --only)
                [[ $# -ge 2 ]] || { log_error "--only requires a pinned binary name"; exit 2; }
                found=0
                for index in "${!BINARY_NAMES[@]}"; do
                    if [[ "$2" == "${BINARY_NAMES[$index]}" ]]; then
                        case " ${SELECTED_INDICES[*]} " in
                            *" $index "*) ;;
                            *) SELECTED_INDICES+=("$index") ;;
                        esac
                        found=1
                        break
                    fi
                done
                (( found == 1 )) || { log_error "Unknown pinned binary: $2"; exit 2; }
                shift 2
                continue
                ;;
            --self-test)
                SELF_TEST=1
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown argument: $1"
                usage
                exit 1
                ;;
        esac
        shift
    done
    if (( SELF_TEST == 1 )); then
        (( argument_count == 1 )) || { log_error "--self-test must be used alone"; exit 2; }
        return
    fi
    if (( ${#SELECTED_INDICES[@]} == 0 )); then
        SELECTED_INDICES=("${!BINARY_NAMES[@]}")
    fi
    if ! complete_selection && [[ -n "$OMNI_NODE_PATH" || -n "$OMNI_NODE_SHA256" ]]; then
        log_error "Corrected Omni selection requires the complete pinned bundle"
        exit 2
    fi
}

complete_selection() {
    (( ${#SELECTED_INDICES[@]} == ${#BINARY_NAMES[@]} ))
}

resolve_platform() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64)
            require_commands sha256sum
            CHECKSUM_COMMAND=(sha256sum)
            CHECKSUMS=(
                7c21c5831eb0c093e3b91e23857cfaecf30ba15b4d14256c13f942a43542c21b
                2066854bdaf9a2cdfb4a331f9684953e4a8000a1aa96bb684e112f50fbc91c4e
                b33b8a288b386ff5d1bbd28eb6a3ee70e0ada968e93c369888ee0cf67303e771
                16ecbd306afa6562d911a8d533e976e194c6d5e3f404321ea83ec4171f7fe5f6
                2c78e5105272a6e23f1def5dd4003080358df15845530736a65a82415aea7c28
            )
            ;;
        Darwin:arm64)
            require_commands shasum
            CHECKSUM_COMMAND=(shasum -a 256)
            ASSET_SUFFIX="-aarch64-apple-darwin"
            CHECKSUMS=(
                2d556b07a703010ee139732763f6e367865c589928a1c91bb681a0b075c88fba
                abb5b18beb1fdf148c117dcf7564336a3d80b3e83b19b9e6fe16060854ee4d41
                29dd86a180e2eea20b4faff9626bf7153b8904caa9651a9658c63041b20c6167
                76d21993ef47e942f25a1137dd6160f767634247de677d00691686e910ccb683
                0b37b9835a9580c555489e787f8b4449d710118464d2433396181edb4a88a416
            )
            ;;
        *)
            log_error "Unsupported host: $(uname -s) $(uname -m)"
            exit 1
            ;;
    esac
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_commands curl mktemp chmod cp cut mkdir mv rm uname jq
    resolve_platform
    if [[ -n "$OMNI_NODE_PATH" || -n "$OMNI_NODE_SHA256" ]]; then
        [[ -n "$OMNI_NODE_PATH" && "$OMNI_NODE_SHA256" =~ ^[a-f0-9]{64}$ && -x "$OMNI_NODE_PATH" ]] || {
            log_error "DEOS_OMNI_NODE_PATH and its 64-character expected SHA-256 are required together"
            exit 1
        }
        checksum_matches "$OMNI_NODE_PATH" "$OMNI_NODE_SHA256" || { log_error "Supplied Omni checksum mismatch"; exit 1; }
        if is_unqualified_omni_checksum "$OMNI_NODE_SHA256"; then
            log_error "Published Omni artifacts are not qualified DEOS selections; select an independently qualified source build"
            exit 1
        fi
    fi
    log_success "Pinned binary prerequisites checked"
}

# Keep known prior-release Omni artifacts excluded when the current pin advances.
is_unqualified_omni_checksum() {
    case "$1" in
        "${CHECKSUMS[1]}"|a30cc63c7794eec8c22d92bbb103c6757b8dcab6c6f550eb480bec0edd5716cb|2cd1ddb414bc6bba46be19052ff8f4b95172761ff8b465d8c5a506e9c9b4f0ce) return 0 ;;
        *) return 1 ;;
    esac
}

checksum_matches() {
    local path="$1"
    local expected="$2"
    [[ -f "$path" ]] && [[ "$("${CHECKSUM_COMMAND[@]}" "$path" | cut -d ' ' -f 1)" == "$expected" ]]
}

cleanup_binary_staging() {
    [[ -z "$BINARY_STAGING_DIR" ]] || rm -rf -- "$BINARY_STAGING_DIR"
}

download_bundle() {
    phase_banner "Step 2: Download and verify bundle"
    local staging_dir
    staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/deos-polkadot-binaries.XXXXXX")"
    BINARY_STAGING_DIR="$staging_dir"
    trap cleanup_binary_staging EXIT

    local index name asset destination expected
    for index in "${SELECTED_INDICES[@]}"; do
        name="${BINARY_NAMES[$index]}"
        asset="$name$ASSET_SUFFIX"
        destination="$staging_dir/$name"
        expected="${CHECKSUMS[$index]}"
        if checksum_matches "$BIN_DIR/$name" "$expected"; then
            log_info "Reusing verified $name"
            cp "$BIN_DIR/$name" "$destination"
        else
            log_info "Downloading $asset from $POLKADOT_SDK_RELEASE"
            curl --fail --location --silent --show-error --retry 3 --retry-all-errors \
                --output "$destination" "$BASE_URL/$asset"
        fi
        if ! checksum_matches "$destination" "$expected"; then
            log_error "SHA-256 mismatch for $asset"
            exit 1
        fi
        chmod 0755 "$destination"
    done

    mkdir -p "$BIN_DIR"
    for index in "${SELECTED_INDICES[@]}"; do
        name="${BINARY_NAMES[$index]}"
        mv "$staging_dir/$name" "$BIN_DIR/$name"
    done
    if complete_selection; then
        printf '%s\n' "$POLKADOT_SDK_RELEASE" >"$BIN_DIR/.polkadot-sdk-release"
    fi
    trap - EXIT
    rm -rf -- "$staging_dir"
    BINARY_STAGING_DIR=""
    log_success "Pinned selection published to $BIN_DIR (${#SELECTED_INDICES[@]} binaries)"
}

install_omni_selection() {
    [[ -n "$OMNI_NODE_PATH" ]] || return 0
    phase_banner "Step 3: Select qualified Omni artifact"
    BINARY_STAGING_DIR="$(mktemp -d "$BIN_DIR/.omni-selection.XXXXXX")"
    trap cleanup_binary_staging EXIT
    cp -- "$OMNI_NODE_PATH" "$BINARY_STAGING_DIR/polkadot-omni-node-deos"
    checksum_matches "$BINARY_STAGING_DIR/polkadot-omni-node-deos" "$OMNI_NODE_SHA256" || { log_error "Selected Omni copy checksum mismatch"; exit 1; }
    chmod 0755 "$BINARY_STAGING_DIR/polkadot-omni-node-deos"
    jq -n --arg release "$POLKADOT_SDK_RELEASE" --arg selected "$OMNI_NODE_SHA256" --arg upstream "${CHECKSUMS[1]}" \
        '{schema_version: 1, sdk_release: $release, selected_sha256: $selected, upstream_omni_sha256: $upstream}' \
        >"$BINARY_STAGING_DIR/.deos-omni-node.json"
    mv "$BINARY_STAGING_DIR/polkadot-omni-node-deos" "$BIN_DIR/polkadot-omni-node-deos"
    mv "$BINARY_STAGING_DIR/.deos-omni-node.json" "$BIN_DIR/.deos-omni-node.json"
    cleanup_binary_staging
    BINARY_STAGING_DIR=""
    trap - EXIT
}

verify_bundle() {
    phase_banner "Step 3: Verify executables"
    if complete_selection; then
        [[ -f "$BIN_DIR/.polkadot-sdk-release" ]] || {
            log_error "Pinned bundle release marker is missing"
            exit 1
        }
        [[ "$(<"$BIN_DIR/.polkadot-sdk-release")" == "$POLKADOT_SDK_RELEASE" ]] || {
            log_error "Pinned bundle release marker does not match $POLKADOT_SDK_RELEASE"
            exit 1
        }
    fi
    local index name
    for index in "${SELECTED_INDICES[@]}"; do
        name="${BINARY_NAMES[$index]}"
        checksum_matches "$BIN_DIR/$name" "${CHECKSUMS[$index]}" || {
            log_error "Published checksum mismatch for $name"
            exit 1
        }
        [[ -x "$BIN_DIR/$name" ]] || { log_error "$name is not executable"; exit 1; }
    done
    for index in "${SELECTED_INDICES[@]}"; do
        name="${BINARY_NAMES[$index]}"
        case "$name" in
            polkadot|polkadot-omni-node|frame-omni-bencher) "$BIN_DIR/$name" --version ;;
        esac
    done
    if ! complete_selection; then
        log_success "Verified selected binaries only; complete bundle and corrected Omni not qualified"
        return
    fi
    if [[ -e "$BIN_DIR/polkadot-omni-node-deos" || -e "$BIN_DIR/.deos-omni-node.json" ]]; then
        verify_deos_omni_node
        "$BIN_DIR/polkadot-omni-node-deos" --version
    else
        log_info "No qualified Omni selected; 05-spawn-zombienet.sh requires a separate source-built artifact"
    fi
    log_success "Pinned Polkadot SDK executable bundle verified"
}

self_test() (
    local fixture name index bad_asset=""
    fixture="$(mktemp -d "${TMPDIR:-/tmp}/deos-binary-self-test.XXXXXX")"
    trap 'rm -rf -- "$fixture"' EXIT
    BIN_DIR="$fixture/published"
    CHECKSUM_COMMAND=(sha256sum)
    CHECKSUMS=()
    OMNI_NODE_PATH=""
    OMNI_NODE_SHA256=""
    mkdir -p "$fixture/source"
    for name in "${BINARY_NAMES[@]}"; do
        printf '#!/usr/bin/env bash\nprintf "fixture %s\\n"\n' "$name" >"$fixture/source/$name"
        CHECKSUMS+=("$(sha256sum "$fixture/source/$name" | cut -d ' ' -f 1)")
    done
    local checksum
    for checksum in "${CHECKSUMS[1]}" a30cc63c7794eec8c22d92bbb103c6757b8dcab6c6f550eb480bec0edd5716cb 2cd1ddb414bc6bba46be19052ff8f4b95172761ff8b465d8c5a506e9c9b4f0ce; do
        if ! is_unqualified_omni_checksum "$checksum"; then
            log_error "Published Omni checksum escaped corrected-node refusal"
            exit 1
        fi
    done
    if is_unqualified_omni_checksum "${CHECKSUMS[0]}"; then
        log_error "Omni refusal incorrectly classified a distinct artifact"
        exit 1
    fi
    curl() {
        local destination="" asset=""
        while [[ $# -gt 0 ]]; do
            if [[ "$1" == "--output" ]]; then
                destination="$2"
                shift 2
            else
                asset="${1##*/}"
                shift
            fi
        done
        printf '%s\n' "$asset" >>"$fixture/calls"
        cp "$fixture/source/$asset" "$destination"
        [[ "$asset" != "$bad_asset" ]] || printf 'corrupt\n' >>"$destination"
    }
    select_args() {
        SELECTED_INDICES=()
        CHECK_ONLY=0
        SELF_TEST=0
        parse_args "$@"
    }
    expect_refusal() {
        if (select_args "$@") >/dev/null 2>&1; then
            log_error "Expected argument refusal: $*"
            exit 1
        fi
    }
    select_args
    complete_selection
    expect_refusal --only
    expect_refusal --only unknown
    expect_refusal --only ""
    expect_refusal --only '../frame-omni-bencher'
    expect_refusal --self-test --check
    if (OMNI_NODE_PATH="fixture"; select_args --only frame-omni-bencher) >/dev/null 2>&1; then
        log_error "Partial selection accepted corrected-node environment"
        exit 1
    fi
    select_args --only frame-omni-bencher --only frame-omni-bencher --check
    [[ "${SELECTED_INDICES[*]}" == "4" && "$CHECK_ONLY" == "1" ]]
    (download_bundle)
    [[ "$(<"$fixture/calls")" == "frame-omni-bencher" && ! -e "$BIN_DIR/.polkadot-sdk-release" ]]
    [[ ! -e "$BIN_DIR/polkadot" && ! -e "$BIN_DIR/polkadot-prepare-worker" ]]
    verify_bundle
    if (select_args --check; verify_bundle) >/dev/null 2>&1; then
        log_error "Partial publication qualified a full bundle"
        exit 1
    fi
    select_args --only frame-omni-bencher --only polkadot-prepare-worker
    bad_asset="polkadot-prepare-worker"
    if (download_bundle) >/dev/null 2>&1; then
        log_error "Checksum corruption was accepted"
        exit 1
    fi
    cmp "$fixture/source/frame-omni-bencher" "$BIN_DIR/frame-omni-bencher"
    [[ ! -e "$BIN_DIR/polkadot-prepare-worker" && ! -e "$BIN_DIR/.polkadot-sdk-release" ]]
    bad_asset=""
    select_args --only polkadot-prepare-worker --only polkadot-execute-worker
    (download_bundle)
    select_args --check --only polkadot-prepare-worker --only polkadot-execute-worker
    verify_bundle
    [[ ! -e "$BIN_DIR/.polkadot-sdk-release" && ! -e "$BIN_DIR/polkadot" ]]
    select_args --only frame-omni-bencher --only polkadot-execute-worker \
        --only polkadot-prepare-worker --only polkadot-omni-node --only polkadot
    complete_selection
    select_args
    (download_bundle)
    verify_bundle
    for index in "${!BINARY_NAMES[@]}"; do
        checksum_matches "$BIN_DIR/${BINARY_NAMES[$index]}" "${CHECKSUMS[$index]}"
    done
    [[ "$(<"$BIN_DIR/.polkadot-sdk-release")" == "$POLKADOT_SDK_RELEASE" ]]
    printf 'other-release\n' >"$BIN_DIR/.polkadot-sdk-release"
    if (verify_bundle) >/dev/null 2>&1; then
        log_error "A stale release marker qualified the current bundle"
        exit 1
    fi
    printf '%s\n' "$POLKADOT_SDK_RELEASE" >"$BIN_DIR/.polkadot-sdk-release"
    printf 'corrupt\n' >>"$BIN_DIR/frame-omni-bencher"
    if (select_args --check --only frame-omni-bencher; verify_bundle) >/dev/null 2>&1; then
        log_error "Partial verification accepted corrupted published bytes"
        exit 1
    fi
    log_success "Pinned binary selection/publication/checksum self-test passed (offline fixtures only)"
)

main() {
    parse_args "$@"
    if (( SELF_TEST == 1 )); then
        self_test
        return
    fi
    phase_banner "DEOS executable bundle download"
    check_prerequisites
    if [[ "$CHECK_ONLY" == "0" ]]; then
        download_bundle
        install_omni_selection
    fi
    verify_bundle
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
