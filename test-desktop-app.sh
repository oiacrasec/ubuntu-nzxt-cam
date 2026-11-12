#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo clippy --locked --no-default-features --all-targets -- -D warnings
cargo test --locked --no-default-features
bash -n install-desktop-app.sh uninstall-desktop-app.sh test-desktop-app.sh
