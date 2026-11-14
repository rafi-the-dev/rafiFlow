#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
NO_COLOR=true trunk build --release
cargo build --release -p rafiflow
