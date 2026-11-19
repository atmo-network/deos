#!/usr/bin/env bash

set -euo pipefail
# Bind the bytes actually sourced, not a concurrently edited shared file on disk.
COMMON_SOURCE="$(<"$(dirname "${BASH_SOURCE[0]}")/_common.sh")"
source <(printf '%s\n' "$COMMON_SOURCE")

ZOMBIENET_VERSION="1.3.138"
ZOMBIENET_INTEGRITY="sha512-hZ3n7y4SOwSP2C6sBL880b7qSK+EbSt39R2d/23fu90+VvH4bu3fkYRZb2sk6hio7zgIyBwQHpzqhhLL7DOJ1Q=="
CHAIN_SPEC_BUILDER_VERSION="19.0.0"
ONLY=""
SELF_TEST=0
BUILDER_PROVENANCE_ONLY=0
BUILDER_SOURCE_ONLY=0
BUILDER_INPUTS="$SCRIPT_DIR/inputs/chain-spec-builder"
BUILDER_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/deos-rc-0728/builder-provenance"
TRY_RUNTIME_VERSION="0.10.1"
TRY_RUNTIME_REVISION="6e1c4e95e76c7deee7a19bc05ae2496dda0ee0be"
usage() {
    cat <<'EOF'
Usage: 02-install-tools.sh [OPTIONS]

Installs the exact repository-pinned operator tools.

Options:
  --only NAME       Select zombienet, chain-spec-builder, or try-runtime
  --builder-provenance-only
                    With --only chain-spec-builder, build non-promoting producer
                    evidence in the cache, not a global installation or approval
  --builder-source-only
                    With --only chain-spec-builder, acquire and seal maintained
                    source/lock/graph only; never compile, install or execute
  --self-test       Test narrow/default routing and option refusal without installation
  -h, --help        Show this help message

Tools:
  @zombienet/cli 1.3.138
  staging-chain-spec-builder 19.0.0
  try-runtime-cli 0.10.1 at Git revision 6e1c4e95e76c7deee7a19bc05ae2496dda0ee0be

Inputs:
  Cargo, Node and HTTPS access as needed. npm is selected only by the
  optional network tool (zombienet or the full operator bundle).
  Builder: Rust 1.94.1, native C/C++/OpenSSL prerequisites. Optional absolute
  PROTOC, PERL5LIB, LD_LIBRARY_PATH, LIBCLANG_PATH build inputs are recorded.
  Cache: ${XDG_CACHE_HOME:-$HOME/.cache}/deos-rc-0728/builder-provenance.
  Source-only cache: ${XDG_CACHE_HOME:-$HOME/.cache}/deos-rc-0728/builder-boundary-correction/source-preparation.
  Checked maintenance.json/maintenance.lock explicitly extend, never relabel, the
  original archive19 manifest/lock. Independent source acceptance gates compilation.

Outputs:
  Requested npm/Cargo tools when installing. Builder evidence modes instead
  write a private cache attempt: source-prepared.json for source-only, or a
  receipt/executable after separate source acceptance. Normal builder selection
  only verifies an existing approved image; evidence modes never install it.

Side effects:
  Replaces mismatched tool versions; never accepts an arbitrary command already
  present on PATH and never installs from a mutable Git branch or tag.
  Builder reissue downloads the exact archive and locked dependencies, compiles
  with two jobs in a private target, and records source/config/compiler/env
  before and after. Shared Cargo caches are not a cleanroom. Version-only reuse
  is forbidden. Unknown receipts or unresolved builder security dispositions
  refuse normal installation/reference execution. Linux proof is not macOS
  qualification; macOS acquisition/source-build support remains required.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --self-test) SELF_TEST=1 ;;
            --builder-provenance-only) BUILDER_PROVENANCE_ONLY=1 ;;
            --builder-source-only) BUILDER_SOURCE_ONLY=1 ;;
            --only)
                [[ "${2:-}" =~ ^(zombienet|chain-spec-builder|try-runtime)$ ]] || { log_error 'Unsupported --only tool'; return 1; }
                ONLY="$2"
                shift
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
    if [[ "$BUILDER_PROVENANCE_ONLY" == 1 || "$BUILDER_SOURCE_ONLY" == 1 ]]; then
        [[ "$ONLY" == chain-spec-builder ]] || { log_error 'Builder evidence modes require --only chain-spec-builder'; return 1; }
        [[ "$BUILDER_PROVENANCE_ONLY$BUILDER_SOURCE_ONLY" != 11 ]] || { log_error 'Builder source-only and compilation modes are exclusive'; return 1; }
    fi
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    [[ -z "${NODE_OPTIONS:-}${NODE_PATH:-}${LD_PRELOAD:-}" ]] || { log_error 'Unset Node/loader injection before tool acquisition'; return 1; }
    if [[ "$ONLY" != try-runtime ]]; then activate_pinned_node; fi
    hydrate_local_tool_paths
    if [[ -z "$ONLY" || "$ONLY" == zombienet ]]; then require_commands npm; fi
    [[ "$ONLY" == "zombienet" ]] || require_commands cargo
    log_success "Pinned tool installation prerequisites checked"
}

command_has_version() {
    local cmd="$1"
    local expected="$2"
    command -v "$cmd" &>/dev/null && "$cmd" --version 2>&1 | grep -Fq "$expected"
}

zombienet_has_version() {
    local executable="zombienet"
    if [[ -n "${NPM_CONFIG_PREFIX:-}" ]]; then executable="$NPM_CONFIG_PREFIX/bin/zombienet"; fi
    command -v "$executable" &>/dev/null \
        && "$executable" version 2>&1 | grep -Fq "$ZOMBIENET_VERSION"
}

build_builder_provenance() (
    require_commands node curl tar rustup cc c++ perl make
    [[ -z "${NODE_OPTIONS:-}${NODE_PATH:-}${LD_PRELOAD:-}" ]] || { log_error 'Loader injection is unsupported'; exit 1; }
    local target toolchain_bin work source status=0 cargo_home node
    # Invoke the canonical executable, not an ephemeral version-manager alias.
    # Its exact path/hash remains a reviewed input; no tool-path normalization.
    node="$(node -p 'require("node:fs").realpathSync(process.execPath)')"
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
        Darwin:arm64) target=aarch64-apple-darwin ;;
        *) log_error 'Builder supports Linux x86_64 or macOS arm64 only'; exit 1 ;;
    esac
    if [[ "$BUILDER_SOURCE_ONLY" != 1 ]]; then "$node" "$BUILDER_INPUTS/provenance.mjs" source-review "$target"; fi
    toolchain_bin="$(dirname "$(rustup which --toolchain 1.94.1 rustc)")"
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    [[ "$cargo_home" == /* && ! -e "$cargo_home/config" && ! -e "$cargo_home/config.toml" ]] || {
        log_error 'Builder requires absolute shared CARGO_HOME without config overrides'; exit 1;
    }
    if [[ "$BUILDER_SOURCE_ONLY" == 1 ]]; then
        BUILDER_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/deos-rc-0728/builder-boundary-correction/source-preparation"
    fi
    mkdir -p "$BUILDER_CACHE"
    work="$(mktemp -d "$BUILDER_CACHE/attempt.XXXXXX")"
    log_info "Non-promoting builder preparation: $work"
    printf '%s\n' "$COMMON_SOURCE" >"$work/common-loaded.sh"
    curl --fail --location --proto '=https' --tlsv1.2 \
        https://static.crates.io/crates/staging-chain-spec-builder/staging-chain-spec-builder-19.0.0.crate -o "$work/source.crate"
    [[ "$(deos_sha256_file "$work/source.crate")" == '2d0eaaa88ded7ea3d7257bbcd0e96bd70c030680ef2ca93874a21915d94af9fd' ]] || {
        log_error 'Builder archive checksum mismatch'; exit 1;
    }
    tar -xf "$work/source.crate" -C "$work"
    source="$work/staging-chain-spec-builder-$CHAIN_SPEC_BUILDER_VERSION"
    "$node" "$BUILDER_INPUTS/provenance.mjs" source-prepare "$work"
    mkdir "$work/home" "$work/target"
    local -a build_env=(env -i "HOME=$work/home" "CARGO_HOME=$cargo_home"
        "PATH=$toolchain_bin:/usr/bin:/bin" 'LANG=C' 'LC_ALL=C' 'TZ=UTC'
        "CARGO_BUILD_TARGET=$target" 'CARGO_BUILD_JOBS=2' "CARGO_TARGET_DIR=$work/target"
        'RUSTFLAGS=' 'CARGO_PROFILE_RELEASE_OPT_LEVEL=3' 'CARGO_PROFILE_RELEASE_LTO=false'
        'CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16' 'CARGO_PROFILE_RELEASE_PANIC=unwind'
        'CARGO_PROFILE_RELEASE_DEBUG=false' 'CARGO_PROFILE_RELEASE_STRIP=none'
        'CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=false' 'CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false'
        'CARGO_PROFILE_RELEASE_INCREMENTAL=false')
    local key
    for key in PROTOC PERL5LIB LD_LIBRARY_PATH LIBCLANG_PATH; do
        [[ -z "${!key:-}" ]] || build_env+=("$key=${!key}")
    done
    cd "$source"
    "${build_env[@]}" "$node" "$BUILDER_INPUTS/provenance.mjs" environment "$work"
    "${build_env[@]}" "$toolchain_bin/rustc" -vV >"$work/rustc.txt"
    "${build_env[@]}" "$toolchain_bin/cargo" -vV >"$work/cargo.txt"
    "${build_env[@]}" "$toolchain_bin/rustc" --print cfg --target "$target" >"$work/cfg.txt"
    uname -a >"$work/host.txt"
    { /usr/bin/cc --version; /usr/bin/c++ --version; /usr/bin/perl -V; } >>"$work/host.txt" 2>&1
    if [[ "$BUILDER_SOURCE_ONLY" != 1 ]]; then
        "${build_env[@]}" /usr/bin/perl -MFindBin -MIPC::Cmd -MFile::Compare -MFile::Copy -MText::Template -e 1 >"$work/native-prerequisites.log" 2>&1
        if [[ -n "${PROTOC:-}" ]]; then
            "${build_env[@]}" "$PROTOC" --version >>"$work/native-prerequisites.log" 2>&1
        fi
    fi
    local directory
    directory="$source"
    while :; do
        [[ ! -e "$directory/.cargo/config" && ! -e "$directory/.cargo/config.toml" ]] || {
            log_error "Undeclared Cargo configuration in $directory"; exit 1;
        }
        [[ "$directory" != / ]] || break
        directory="$(dirname "$directory")"
    done
    local -a tools=("$toolchain_bin/rustc" "$toolchain_bin/cargo" "$node" /usr/bin/cc /usr/bin/c++ /usr/bin/perl /usr/bin/make)
    "$node" "$BUILDER_INPUTS/provenance.mjs" tools "$work" "${tools[@]}"
    "${build_env[@]}" "$toolchain_bin/cargo" metadata --locked --format-version 1 --filter-platform "$target" >"$work/metadata.json" 2>"$work/metadata.log"
    "${build_env[@]}" "$toolchain_bin/cargo" tree --locked --edges features,normal,build --target "$target" >"$work/features.txt" 2>"$work/features.log"
    "$node" "$BUILDER_INPUTS/provenance.mjs" before "$work"
    if [[ "$BUILDER_SOURCE_ONLY" == 1 ]]; then
        "$node" "$BUILDER_INPUTS/provenance.mjs" source-finish "$work"
        log_warning "Source-only receipt: $work/source-prepared.json; independent source review and artifact qualification remain open"
        return
    fi
    # Reconstruct current inputs and compare the independently accepted basis,
    # not merely a new attempt's own snapshot, immediately before Cargo.
    "$node" "$BUILDER_INPUTS/provenance.mjs" build-admission "$work"
    "${build_env[@]}" "$toolchain_bin/cargo" build --frozen --release --bin chain-spec-builder --message-format=json \
        >"$work/build.jsonl" 2>"$work/build.log" || status=$?
    "$node" "$BUILDER_INPUTS/provenance.mjs" finish "$work" "$work/target/$target/release/chain-spec-builder" "$status"
    if command -v readelf >/dev/null; then
        readelf -h -l -d -V -p .comment "$work/target/$target/release/chain-spec-builder" >"$work/elf.txt"
    fi
    log_warning "Producer receipt: $work/receipt.json; 81 security dispositions open. No canonical installation or promotion."
)

install_zombienet() {
    if zombienet_has_version; then
        log_info "Reusing zombienet $ZOMBIENET_VERSION"
        return
    fi
    local registry_integrity
    registry_integrity="$(npm view "@zombienet/cli@$ZOMBIENET_VERSION" dist.integrity)"
    [[ "$registry_integrity" == "$ZOMBIENET_INTEGRITY" ]] || {
        log_error "@zombienet/cli registry integrity does not match the repository pin"
        exit 1
    }
    npm install --global --save-exact "@zombienet/cli@$ZOMBIENET_VERSION"
    zombienet_has_version || {
        log_error "zombienet did not report pinned version $ZOMBIENET_VERSION after installation"
        exit 1
    }
}

install_try_runtime() {
    if command_has_version try-runtime "$TRY_RUNTIME_VERSION"; then
        log_info "Reusing try-runtime $TRY_RUNTIME_VERSION"
        return
    fi
    cargo install --git https://github.com/paritytech/try-runtime-cli \
        --rev "$TRY_RUNTIME_REVISION" --locked --force try-runtime-cli
    command_has_version try-runtime "$TRY_RUNTIME_VERSION" || {
        log_error "try-runtime did not report pinned version $TRY_RUNTIME_VERSION after installation"
        exit 1
    }
}

install_builder() {
    if [[ "$BUILDER_PROVENANCE_ONLY" == 1 || "$BUILDER_SOURCE_ONLY" == 1 ]]; then build_builder_provenance; return; fi
    [[ -z "${NODE_OPTIONS:-}${NODE_PATH:-}${LD_PRELOAD:-}" ]] || { log_error 'Loader injection is unsupported'; return 1; }
    local builder
    builder="$(command -v chain-spec-builder || true)"
    [[ -n "$builder" ]] || { log_error 'No qualified builder installed; use --only chain-spec-builder --builder-provenance-only for producer evidence'; return 1; }
    node "$BUILDER_INPUTS/provenance.mjs" verify "${builder}.producer.json" "$builder"
}

install_tools() {
    phase_banner "Step 2: Install exact tools"
    [[ -n "$ONLY" && "$ONLY" != zombienet ]] || install_zombienet
    [[ -n "$ONLY" && "$ONLY" != chain-spec-builder ]] || install_builder
    [[ -n "$ONLY" && "$ONLY" != try-runtime ]] || install_try_runtime
}

run_self_tests() {
    (
        local calls=""
        phase_banner() { :; }
        install_zombienet() { calls+="zombienet "; }
        install_builder() { calls+="builder "; }
        install_try_runtime() { calls+="try-runtime "; }
        ONLY=zombienet install_tools
        [[ "$calls" == "zombienet " ]] || exit 1
        calls=""
        ONLY="" install_tools
        [[ "$calls" == "zombienet builder try-runtime " ]] || exit 1
        calls=""
        ONLY=chain-spec-builder install_tools
        [[ "$calls" == 'builder ' ]] || exit 1
        calls=""
        ONLY=try-runtime install_tools
        [[ "$calls" == 'try-runtime ' ]] || exit 1
        if (parse_args --only invalid) >/dev/null 2>&1; then exit 1; fi
        if (parse_args --only) >/dev/null 2>&1; then exit 1; fi
        if (ONLY='' parse_args --builder-source-only) >/dev/null 2>&1; then exit 1; fi
        if (parse_args --only chain-spec-builder --builder-source-only --builder-provenance-only) >/dev/null 2>&1; then exit 1; fi
        (parse_args --only chain-spec-builder --builder-source-only) || exit 1
    ) || { log_error "Tool selection self-test failed"; return 1; }
    node "$BUILDER_INPUTS/provenance.mjs" self-test
    log_success "Narrow/default tool installation routing and option refusal passed"
}

print_summary() {
    phase_banner "Summary"
    if [[ "$BUILDER_PROVENANCE_ONLY" == 1 || "$BUILDER_SOURCE_ONLY" == 1 ]]; then
        log_warning 'Diagnostic evidence only: security qualification and canonical installation remain open'
    else
        log_success "Requested repository tools match pinned identities"
    fi
}

main() {
    parse_args "$@"
    if [[ "$SELF_TEST" == "1" ]]; then run_self_tests; return; fi
    phase_banner "DEOS pinned tool installation"
    check_prerequisites
    install_tools
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
