#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT_DIR/rust"
DEFAULT_CONFIG="$RUST_DIR/src/default_config.yaml"

CONFIG_PATH="$DEFAULT_CONFIG"
PAYLOAD="loopback test payload"
NOISE_VOLTAGE="0"
DOPPLER_HZ="0"
TIMEOUT_SECONDS="20"
ITERATIONS="1"
GAP_SECONDS="1.25"
INITIAL_GAP_SECONDS=""
TAIL_GAP_SECONDS=""
LOOPBACK_VERBOSE="false"

usage() {
    cat <<'EOF'
Usage: test_loopback.sh [--config PATH] [--payload TEXT] [--noise VOLTS] [--doppler HZ] [--timeout SECONDS] [--iterations N] [--gap SECONDS] [--initial-gap SECONDS] [--tail-gap SECONDS] [--loopback-verbose]
EOF
}

while (($#)); do
    case "$1" in
        --config)
            CONFIG_PATH="${2:?missing value for --config}"
            shift 2
            ;;
        --payload)
            PAYLOAD="${2:?missing value for --payload}"
            shift 2
            ;;
        --noise)
            NOISE_VOLTAGE="${2:?missing value for --noise}"
            shift 2
            ;;
        --doppler)
            DOPPLER_HZ="${2:?missing value for --doppler}"
            shift 2
            ;;
        --timeout)
            TIMEOUT_SECONDS="${2:?missing value for --timeout}"
            shift 2
            ;;
        --iterations)
            ITERATIONS="${2:?missing value for --iterations}"
            shift 2
            ;;
        --gap)
            GAP_SECONDS="${2:?missing value for --gap}"
            shift 2
            ;;
        --initial-gap)
            INITIAL_GAP_SECONDS="${2:?missing value for --initial-gap}"
            shift 2
            ;;
        --tail-gap)
            TAIL_GAP_SECONDS="${2:?missing value for --tail-gap}"
            shift 2
            ;;
        --loopback-verbose)
            LOOPBACK_VERBOSE="true"
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unrecognized argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "Configuration file not found: $CONFIG_PATH" >&2
    exit 2
fi

if ! [[ "$ITERATIONS" =~ ^[0-9]+$ ]] || [[ "$ITERATIONS" -lt 1 ]]; then
    echo "--iterations must be a positive integer" >&2
    exit 2
fi

tmp_dir="$(mktemp -d)"
cleanup() {
    local status=$?
    if [[ -n "${RX_PID:-}" ]] && kill -0 "$RX_PID" 2>/dev/null; then
        kill -INT "$RX_PID" 2>/dev/null || true
    fi
    if [[ -n "${LOOPBACK_PID:-}" ]] && kill -0 "$LOOPBACK_PID" 2>/dev/null; then
        kill -INT "$LOOPBACK_PID" 2>/dev/null || true
    fi
    if [[ -n "${TX_PID:-}" ]] && kill -0 "$TX_PID" 2>/dev/null; then
        kill -INT "$TX_PID" 2>/dev/null || true
    fi
    if [[ -n "${TX_INPUT_FD:-}" ]]; then
        eval "exec ${TX_INPUT_FD}>&-" 2>/dev/null || true
    fi
    if [[ -n "${TX_PID:-}" ]]; then
        wait "$TX_PID" 2>/dev/null || true
    fi
    if [[ -n "${RX_PID:-}" ]]; then
        wait "$RX_PID" 2>/dev/null || true
    fi
    if [[ -n "${LOOPBACK_PID:-}" ]]; then
        wait "$LOOPBACK_PID" 2>/dev/null || true
    fi
    rm -rf "$tmp_dir"
    exit "$status"
}
trap cleanup EXIT INT TERM

cd "$RUST_DIR"
cargo build --quiet --bins

TX_BIN="$RUST_DIR/target/debug/tx"
RX_BIN="$RUST_DIR/target/debug/rx"
LOOPBACK_BIN="$RUST_DIR/target/debug/zmq_loopback"

RX_LOG="$tmp_dir/rx.log"
LOOPBACK_LOG="$tmp_dir/loopback.log"
TX_LOG="$tmp_dir/tx.log"

run_loopback_test() {
    local tx_fifo="$tmp_dir/tx.stdin"
    local initial_gap="${INITIAL_GAP_SECONDS:-$GAP_SECONDS}"
    local tail_gap="${TAIL_GAP_SECONDS:-$GAP_SECONDS}"
    local payloads=()

    : >"$RX_LOG"
    : >"$LOOPBACK_LOG"
    : >"$TX_LOG"
    mkfifo "$tx_fifo"

    local loopback_args=(
        --config "$CONFIG_PATH" \
        --noise "$NOISE_VOLTAGE" \
        --doppler "$DOPPLER_HZ"
    )
    if [[ "$LOOPBACK_VERBOSE" == "true" ]]; then
        loopback_args+=(--verbose)
    fi

    "$LOOPBACK_BIN" "${loopback_args[@]}" >"$LOOPBACK_LOG" 2>&1 &
    LOOPBACK_PID=$!

    sleep 0.5

    "$RX_BIN" --config "$CONFIG_PATH" >"$RX_LOG" 2>&1 &
    RX_PID=$!

    sleep 0.5

    "$TX_BIN" --config "$CONFIG_PATH" <"$tx_fifo" >"$TX_LOG" 2>&1 &
    TX_PID=$!
    exec {TX_INPUT_FD}>"$tx_fifo"

    sleep "$initial_gap"

    for iteration in $(seq 1 "$ITERATIONS"); do
        local iteration_payload="${PAYLOAD} [${iteration}/${ITERATIONS}]"
        payloads+=("$iteration_payload")
        printf '%s\n' "$iteration_payload" >&"$TX_INPUT_FD"
        if [[ "$iteration" -lt "$ITERATIONS" ]]; then
            sleep "$GAP_SECONDS"
        fi
    done

    sleep "$tail_gap"

    exec {TX_INPUT_FD}>&-
    TX_INPUT_FD=""
    if ! wait "$TX_PID"; then
        echo "TX binary failed" >&2
        return 1
    fi
    TX_PID=""

    local deadline=$((SECONDS + TIMEOUT_SECONDS))
    while ((SECONDS < deadline)); do
        if all_payloads_seen "${payloads[@]}"; then
            break
        fi
        if ! kill -0 "$RX_PID" 2>/dev/null; then
            break
        fi
        sleep 0.2
    done

    kill -INT "$RX_PID" 2>/dev/null || true
    kill -INT "$LOOPBACK_PID" 2>/dev/null || true

    wait "$RX_PID" 2>/dev/null || true
    wait "$LOOPBACK_PID" 2>/dev/null || true

    if ! all_payloads_seen "${payloads[@]}"; then
        echo "RX log did not contain all expected payloads" >&2
        for payload in "${payloads[@]}"; do
            if ! grep -Fq "$payload" "$RX_LOG"; then
                echo "Missing payload: $payload" >&2
            fi
        done
        echo "--- RX log ---" >&2
        cat "$RX_LOG" >&2
        echo "--- Loopback log ---" >&2
        cat "$LOOPBACK_LOG" >&2
        echo "--- TX log ---" >&2
        cat "$TX_LOG" >&2
        return 1
    fi

    echo "Loopback test passed: all ${ITERATIONS} payloads found in RX output."
}

all_payloads_seen() {
    local payload
    for payload in "$@"; do
        if ! grep -Fq "$payload" "$RX_LOG"; then
            return 1
        fi
    done
    return 0
}

run_loopback_test
