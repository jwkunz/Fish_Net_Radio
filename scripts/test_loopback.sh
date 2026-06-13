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

usage() {
    cat <<'EOF'
Usage: test_loopback.sh [--config PATH] [--payload TEXT] [--noise VOLTS] [--doppler HZ] [--timeout SECONDS] [--iterations N]
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

run_once() {
    local iteration="$1"
    local iteration_payload="${PAYLOAD} [${iteration}/${ITERATIONS}]"

    : >"$RX_LOG"
    : >"$LOOPBACK_LOG"
    : >"$TX_LOG"

    "$LOOPBACK_BIN" \
        --config "$CONFIG_PATH" \
        --noise "$NOISE_VOLTAGE" \
        --doppler "$DOPPLER_HZ" \
        >"$LOOPBACK_LOG" 2>&1 &
    LOOPBACK_PID=$!

    sleep 0.5

    "$RX_BIN" --config "$CONFIG_PATH" >"$RX_LOG" 2>&1 &
    RX_PID=$!

    sleep 0.5

    if ! printf '%s\n' "$iteration_payload" | "$TX_BIN" --config "$CONFIG_PATH" >"$TX_LOG" 2>&1; then
        echo "TX binary failed on iteration ${iteration}" >&2
        return 1
    fi

    local deadline=$((SECONDS + TIMEOUT_SECONDS))
    while ((SECONDS < deadline)); do
        if grep -Fq "$iteration_payload" "$RX_LOG"; then
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

    if ! grep -Fq "$iteration_payload" "$RX_LOG"; then
        echo "RX log did not contain expected payload on iteration ${iteration}: $iteration_payload" >&2
        echo "--- RX log ---" >&2
        cat "$RX_LOG" >&2
        echo "--- Loopback log ---" >&2
        cat "$LOOPBACK_LOG" >&2
        echo "--- TX log ---" >&2
        cat "$TX_LOG" >&2
        return 1
    fi

    echo "Iteration ${iteration}/${ITERATIONS} passed."
}

for iteration in $(seq 1 "$ITERATIONS"); do
    run_once "$iteration"
done

echo "Loopback test passed: all ${ITERATIONS} iterations found payload in RX output."
