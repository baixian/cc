#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
expected="$repo_root/apps/desktop/src/generated/ipc"
generated="$(mktemp -d)"
trap 'rm -rf "$generated"' EXIT

TS_RS_EXPORT_DIR="$generated" cargo test --manifest-path "$repo_root/apps/desktop/src-tauri/Cargo.toml"

if [[ "${1:-}" == '--write' ]]; then
  mkdir -p "$expected"
  find "$expected" -maxdepth 1 -type f -name '*.ts' -delete
  cp "$generated"/*.ts "$expected"/
  exit 0
fi

if ! diff -ru "$expected" "$generated"; then
  echo 'Rust 生成的 IPC 类型与提交的 TypeScript 类型不同。请在 Mac 上运行 bash scripts/check-contracts.sh --write 并提交变更。' >&2
  for file in "$generated"/*.ts; do
    echo "===== $(basename "$file") =====" >&2
    cat "$file" >&2
  done
  exit 1
fi
