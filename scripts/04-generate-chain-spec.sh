#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

CHAIN_TYPE="${CHAIN_TYPE:-Development}"
PARA_ID="${PARA_ID:-2000}"
RELAY_CHAIN="${RELAY_CHAIN:-rococo-local}"
LOCAL_WEB_CLIENT_NATIVE_STAKING_ID="${LOCAL_WEB_CLIENT_NATIVE_STAKING_ID:-0}"
LOCAL_WEB_CLIENT_FOREIGN_ID="${LOCAL_WEB_CLIENT_FOREIGN_ID:-4026531841}"
LOCAL_WEB_CLIENT_INITIAL_PRICE="${LOCAL_WEB_CLIENT_INITIAL_PRICE:-1000000000000}"
LOCAL_WEB_CLIENT_SLOPE="${LOCAL_WEB_CLIENT_SLOPE:-1000000}"
LOCAL_WEB_CLIENT_FOREIGN_BALANCE="${LOCAL_WEB_CLIENT_FOREIGN_BALANCE:-1152921504606846976}"
RUNTIME_WASM_PATH="${RUNTIME_WASM_PATH:-$TEMPLATE_DIR/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm}"
CHAIN_SPEC_PATH="${CHAIN_SPEC_PATH:-$TEMPLATE_DIR/chain_spec.json}"

usage() {
    cat <<'EOF'
Usage: 04-generate-chain-spec.sh [OPTIONS]

Generates and patches template/chain_spec.json from the built runtime WASM.

Options:
  -h, --help        Show this help message

Environment:
  CHAIN_TYPE=Development|Local|Live
  PARA_ID=2000
  RELAY_CHAIN=rococo-local
  LOCAL_WEB_CLIENT_NATIVE_STAKING_ID=0
  LOCAL_WEB_CLIENT_FOREIGN_ID=4026531841
  LOCAL_WEB_CLIENT_INITIAL_PRICE=1000000000000
  LOCAL_WEB_CLIENT_SLOPE=1000000
  LOCAL_WEB_CLIENT_FOREIGN_BALANCE=1152921504606846976
  RUNTIME_WASM_PATH=template/target/release/wbuild/deos-runtime/deos_runtime.compact.compressed.wasm
  CHAIN_SPEC_PATH=template/chain_spec.json

Inputs:
  Selected DEOS runtime Wasm, chain-spec-builder, Python, and profile values.

Outputs:
  The selected CHAIN_SPEC_PATH.

Side effects:
  Replaces the generated local chain spec; never deploys or starts a network.

Notes:
  CHAIN_TYPE=Live only switches the metadata/profile surface. Final production
  authorities/accounts still need to be replaced before deployment.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
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

resolve_chain_profile() {
    case "$CHAIN_TYPE" in
        Development)
            PRESET="development"
            CHAIN_NAME="DEOS Development"
            CHAIN_ID="deos-dev"
            ;;
        Local)
            PRESET="local_testnet"
            CHAIN_NAME="DEOS Local Testnet"
            CHAIN_ID="deos-local"
            ;;
        Live)
            PRESET="development"
            CHAIN_NAME="DEOS"
            CHAIN_ID="deos"
            ;;
        *)
            log_error "Unknown CHAIN_TYPE: $CHAIN_TYPE (expected: Development, Local, Live)"
            exit 1
            ;;
    esac
}

check_prerequisites() {
    phase_banner "Step 1: Prerequisites"
    require_directory "$TEMPLATE_DIR" "Template directory"
    hydrate_local_tool_paths
    require_commands chain-spec-builder python3 du cut mv mkdir dirname mktemp rm
    log_success "Chain spec prerequisites checked"
}

generate_chain_spec() {
    phase_banner "Step 2: Generate chain spec"
    local generation_dir generated_path
    generation_dir="$(mktemp -d "${TMPDIR:-/tmp}/deos-chain-spec.XXXXXX")"
    generated_path="$generation_dir/chain_spec.json"

    log_info "Generating chain specification"
    echo "  Chain type: $CHAIN_TYPE"
    echo "  Preset: $PRESET"
    echo "  Para ID: $PARA_ID"
    echo "  Relay chain: $RELAY_CHAIN"
    echo "  WASM: $RUNTIME_WASM_PATH"
    echo "  Output: $CHAIN_SPEC_PATH"
    echo ""

    if [[ "$CHAIN_TYPE" == "Live" ]]; then
        log_warning "CHAIN_TYPE=Live does not produce a final production authority set by itself"
    fi

    if [[ ! -f "$RUNTIME_WASM_PATH" ]]; then
        log_error "Runtime WASM artifact not found."
        echo "  Expected: $RUNTIME_WASM_PATH"
        exit 1
    fi

    if ! (
        cd "$generation_dir"
        chain-spec-builder create \
            -c "$RELAY_CHAIN" \
            -p "$PARA_ID" \
            -r "$RUNTIME_WASM_PATH" \
            named-preset "$PRESET"
    ); then
        rm -rf "$generation_dir"
        log_error "chain-spec-builder failed"
        exit 1
    fi

    if [[ ! -f "$generated_path" ]]; then
        rm -rf "$generation_dir"
        log_error "chain-spec-builder did not produce $generated_path"
        exit 1
    fi
    mkdir -p "$(dirname "$CHAIN_SPEC_PATH")"
    mv "$generated_path" "$CHAIN_SPEC_PATH"
    rm -rf "$generation_dir"

    patch_chain_spec "$CHAIN_SPEC_PATH"

    log_success "Chain specification generated"
}

patch_chain_spec() {
    local spec_path="$1"
    log_info "Patching chain spec metadata (chainType=$CHAIN_TYPE, name=$CHAIN_NAME, id=$CHAIN_ID)"

    python3 -c "
import json, sys
spec_path, chain_type, chain_name, chain_id, native_staking_id, foreign_id, initial_price, slope, foreign_balance = sys.argv[1:10]
native_staking_id = int(native_staking_id)
foreign_id = int(foreign_id)
initial_price = int(initial_price)
slope = int(slope)
foreign_balance = int(foreign_balance)
with open(spec_path, 'r') as f:
    spec = json.load(f)
spec['chainType'] = chain_type
spec['name'] = chain_name
spec['id'] = chain_id
patch = spec.setdefault('genesis', {}).setdefault('runtimeGenesis', {}).setdefault('patch', {})
patch.pop('sudo', None)
assets = patch.setdefault('assets', {})
assets.setdefault('nextAssetId', None)
assets.setdefault('reserves', [])
asset_entries = assets.setdefault('assets', [])
metadata_entries = assets.setdefault('metadata', [])
account_entries = assets.setdefault('accounts', [])
balances = patch.get('balances', {}).get('balances', [])
bootstrap_asset_owner = None
if balances:
    first_balance_entry = balances[0]
    if isinstance(first_balance_entry, list) and first_balance_entry:
        bootstrap_asset_owner = first_balance_entry[0]
if bootstrap_asset_owner is None:
    raise SystemExit('Chain spec patching requires at least one endowed balance account to own local dev bootstrap assets')
native_staking_asset_entry = [native_staking_id, bootstrap_asset_owner, True, 1]
if native_staking_asset_entry not in asset_entries:
    asset_entries.append(native_staking_asset_entry)
native_staking_metadata_entry = [native_staking_id, list(b'Native Staking Token'), list(b'NTVE'), 12]
if native_staking_metadata_entry not in metadata_entries:
    metadata_entries.append(native_staking_metadata_entry)
native_staking_account_entry = [native_staking_id, bootstrap_asset_owner, foreign_balance]
if native_staking_account_entry not in account_entries:
    account_entries.append(native_staking_account_entry)
foreign_asset_entry = [foreign_id, bootstrap_asset_owner, True, 1]
if foreign_asset_entry not in asset_entries:
    asset_entries.append(foreign_asset_entry)
foreign_metadata_entry = [foreign_id, list(b'Foreign Token'), list(b'FRGN'), 12]
if foreign_metadata_entry not in metadata_entries:
    metadata_entries.append(foreign_metadata_entry)
foreign_account_entry = [foreign_id, bootstrap_asset_owner, foreign_balance]
if foreign_account_entry not in account_entries:
    account_entries.append(foreign_account_entry)
patch['tokenMintingCurve'] = {
    'curves': [['Native', {'Foreign': foreign_id}, initial_price, slope]],
}
patch['staking'] = {
    'registeredAssets': [native_staking_id],
}
with open(spec_path, 'w') as f:
    json.dump(spec, f, indent=2)
    f.write('\n')
" "$spec_path" "$CHAIN_TYPE" "$CHAIN_NAME" "$CHAIN_ID" "$LOCAL_WEB_CLIENT_NATIVE_STAKING_ID" "$LOCAL_WEB_CLIENT_FOREIGN_ID" "$LOCAL_WEB_CLIENT_INITIAL_PRICE" "$LOCAL_WEB_CLIENT_SLOPE" "$LOCAL_WEB_CLIENT_FOREIGN_BALANCE"
}

verify_output() {
    phase_banner "Step 3: Verify output"
    if [[ -f "$CHAIN_SPEC_PATH" ]]; then
        local size
        size=$(du -h "$CHAIN_SPEC_PATH" | cut -f1)
        log_success "Chain spec file verified"
        echo "  Path: $CHAIN_SPEC_PATH"
        echo "  Size: $size"
        echo "  Chain type: $CHAIN_TYPE"
        echo "  Name: $CHAIN_NAME"
        echo "  ID: $CHAIN_ID"
        echo "  Para ID: $PARA_ID"
        echo "  Relay chain: $RELAY_CHAIN"
    else
        log_error "Chain specification not generated"
        exit 1
    fi
}

print_summary() {
    phase_banner "Summary"
    log_success "Chain spec generation completed successfully"
}

main() {
    parse_args "$@"
    phase_banner "DEOS chain spec generation"
    resolve_chain_profile
    check_prerequisites
    generate_chain_spec
    verify_output
    print_summary
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
