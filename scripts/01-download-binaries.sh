#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

POLKADOT_SDK_RELEASE="polkadot-stable2606-1"
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

usage() {
    cat <<'EOF'
Usage: 01-download-binaries.sh [OPTIONS]

Downloads the pinned Polkadot SDK executable bundle required by DEOS operations.

Options:
  --check           Verify the existing pinned bundle without downloading
  -h, --help        Show this help message

Inputs:
  Network access to the pinned paritytech/polkadot-sdk release when the local
  bundle is absent or checksum-mismatched, plus a supported host platform.

Authority:
  GitHub release: paritytech/polkadot-sdk polkadot-stable2606-1
  Compatibility: DEOS workspace Polkadot SDK 2606 package line
  Supported hosts: Linux x86_64 and macOS arm64

Outputs:
  ./bin/polkadot
  ./bin/polkadot-omni-node
  ./bin/polkadot-prepare-worker
  ./bin/polkadot-execute-worker
  ./bin/frame-omni-bencher
  ./bin/.polkadot-sdk-release

Side effects:
  Downloads only missing or checksum-mismatched assets, verifies every pinned
  SHA-256 digest before publication, and replaces the complete local bundle.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --check)
                CHECK_ONLY=1
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
}

resolve_platform() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64)
            require_commands sha256sum
            CHECKSUM_COMMAND=(sha256sum)
            CHECKSUMS=(
                53c9f450f619d680578dbeed6685de102a9632db7b134631650450b84ea83567
                ff8e5253e8a3e30b421c83d938a3245bdc5de222d807aaf3648575ae029faece
                5e67a05516e24d5e9b9616bacb3a2d58235beb3392de14dfbe51ff6914244267
                cc642041ef2582d972071cd4f7122e9803703bc7775e8d432b2d7626f5011b21
                501f92ba8f1dd7eabfe84aa3990f517fd448c3d5e0de6f408b29656933e39576
            )
            ;;
        Darwin:arm64)
            require_commands shasum
            CHECKSUM_COMMAND=(shasum -a 256)
            ASSET_SUFFIX="-aarch64-apple-darwin"
            CHECKSUMS=(
                e6b3926024c86dddeb3f249942d17e7b8428b8c506919dff9cc9915d9e201a0e
                a05f64056b45af27a3fdca9f2c90f5cf5c4f12c0621003cfa029617494e20104
                3658c4315c1a6762e5984b1f7bde89c22d7c7390e4aa50b6e4257c0c91527cb2
                5125a83632a9c975a32b158aec2542e2f65a37cffc6e50230fdda6dc281d5159
                5fac0fed05278899eef17e613ee865510011e106b529e966d17cd0eb20bd91ab
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
    require_commands curl mktemp chmod cp cut mkdir mv rm uname
    resolve_platform
    log_success "Pinned binary prerequisites checked"
}

checksum_matches() {
    local path="$1"
    local expected="$2"
    [[ -f "$path" ]] && [[ "$("${CHECKSUM_COMMAND[@]}" "$path" | cut -d ' ' -f 1)" == "$expected" ]]
}

download_bundle() {
    phase_banner "Step 2: Download and verify bundle"
    local staging_dir
    staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/deos-polkadot-binaries.XXXXXX")"
    trap "rm -rf -- '$staging_dir'" EXIT

    local index name asset destination expected
    for index in "${!BINARY_NAMES[@]}"; do
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
    for name in "${BINARY_NAMES[@]}"; do
        mv "$staging_dir/$name" "$BIN_DIR/$name"
    done
    printf '%s\n' "$POLKADOT_SDK_RELEASE" >"$BIN_DIR/.polkadot-sdk-release"
    trap - EXIT
    rm -rf -- "$staging_dir"
    log_success "Pinned executable bundle published to $BIN_DIR"
}

verify_bundle() {
    phase_banner "Step 3: Verify executables"
    [[ -f "$BIN_DIR/.polkadot-sdk-release" ]] || {
        log_error "Pinned bundle release marker is missing"
        exit 1
    }
    [[ "$(<"$BIN_DIR/.polkadot-sdk-release")" == "$POLKADOT_SDK_RELEASE" ]] || {
        log_error "Pinned bundle release marker does not match $POLKADOT_SDK_RELEASE"
        exit 1
    }
    local index name
    for index in "${!BINARY_NAMES[@]}"; do
        name="${BINARY_NAMES[$index]}"
        checksum_matches "$BIN_DIR/$name" "${CHECKSUMS[$index]}" || {
            log_error "Published checksum mismatch for $name"
            exit 1
        }
        [[ -x "$BIN_DIR/$name" ]] || { log_error "$name is not executable"; exit 1; }
    done
    "$BIN_DIR/polkadot" --version
    "$BIN_DIR/polkadot-omni-node" --version
    "$BIN_DIR/frame-omni-bencher" --version
    log_success "Pinned Polkadot SDK executable bundle verified"
}

main() {
    parse_args "$@"
    phase_banner "DEOS executable bundle download"
    check_prerequisites
    if [[ "$CHECK_ONLY" == "0" ]]; then
        download_bundle
    fi
    verify_bundle
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
