#!/usr/bin/env bash
set -euo pipefail
model_dir="${XDG_DATA_HOME:-$HOME/.local/share}/rafiflow/models"
mkdir -p "$model_dir"
download() {
  local file="$1" url="$2" hash="$3"
  if [[ -f "$model_dir/$file" ]]; then
    printf '%s  %s\n' "$hash" "$model_dir/$file" | sha256sum --check
    return
  fi
  curl --fail --location --retry 3 --continue-at - --output "$model_dir/$file.part" "$url"
  printf '%s  %s\n' "$hash" "$model_dir/$file.part" | sha256sum --check
  mv "$model_dir/$file.part" "$model_dir/$file"
}
download cohere-transcribe-q8_0.gguf \
  https://huggingface.co/cstr/cohere-transcribe-03-2026-GGUF/resolve/4e98a8c31c95a92d651e60e3bfae4fe29fc6e026/cohere-transcribe-q8_0.gguf \
  e725a5ec9b5b9c0b7aba07b071753cae9b39ec3fbaf9f6a622c6810d5a11e3f4
download qwen3-4b-instruct-2507-q4_k_m.gguf \
  https://huggingface.co/yuko29hu/Qwen3-4B-Instruct-2507-Q4_K_M-GGUF/resolve/ec3821c4a649ff0e21ea755fa0e78ec9d93e7f0a/qwen3-4b-instruct-2507-q4_k_m.gguf \
  01217501dd8c6741c544c32eb0d18b08e27b95475e1270e955da707fa2821e2c
