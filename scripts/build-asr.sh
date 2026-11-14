#!/usr/bin/env bash
set -euo pipefail
asr_dir="${XDG_DATA_HOME:-$HOME/.local/share}/rafiflow/CrispASR"
asr_revision=1c7f6f79e05bd06ec614d203af05d5ae337ec254
if [[ ! -d "$asr_dir" ]]; then
  git clone --no-checkout https://github.com/CrispStrobe/CrispASR.git "$asr_dir"
  git -C "$asr_dir" checkout "$asr_revision"
fi
if [[ "$(git -C "$asr_dir" rev-parse HEAD)" != "$asr_revision" ]]; then
  echo "Existing CrispASR checkout differs from the verified revision. Preserve it and choose another XDG_DATA_HOME." >&2
  exit 1
fi
git -C "$asr_dir" submodule update --init --recursive
cmake -S "$asr_dir" -B "$asr_dir/build" -DGGML_CUDA=ON -DCRISPASR_BUILD_TESTS=OFF -DCRISPASR_BUILD_SERVER=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build "$asr_dir/build" --target crispasr-cli -j "${CMAKE_BUILD_PARALLEL_LEVEL:-4}"
