#!/usr/bin/env bash
set -euo pipefail

# Ensure Cargo's target directory and temporary directory share the same filesystem
TARGET_DIR=${CARGO_TARGET_DIR:-target}
TMP_DIR=${CARGO_TARGET_TMPDIR:-${TMPDIR:-/tmp}}

mkdir -p "$TARGET_DIR"
mkdir -p "$TMP_DIR"

target_dev=$(stat -c %d "$TARGET_DIR")
tmp_dev=$(stat -c %d "$TMP_DIR")

echo "CARGO_TARGET_DIR=$TARGET_DIR ([dev=$target_dev])"
echo "TMPDIR=$TMP_DIR ([dev=$tmp_dev])"

if [[ "$target_dev" != "$tmp_dev" ]]; then
  echo "error: target dir and tmp dir live on different filesystems" >&2
  echo "       set CARGO_TARGET_DIR and TMPDIR to paths mounted on the same device." >&2
  exit 1
fi

echo "device alignment check passed"
