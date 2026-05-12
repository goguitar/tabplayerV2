#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cargo build -p godot_bridge --manifest-path "${ROOT_DIR}/rust/Cargo.toml"

mkdir -p "${ROOT_DIR}/godot/bin"
cp "${ROOT_DIR}/rust/target/debug/libgodot_bridge.so" "${ROOT_DIR}/godot/bin/libgodot_bridge.so"

echo "Built and copied libgodot_bridge.so to godot/bin"
