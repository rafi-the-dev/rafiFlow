# rafiFlow

Local English dictation for Linux, built with Rust, Tauri 2, and Yew. Hold a shortcut, speak, release, and receive edited text in the focused application. No runtime accounts, telemetry, hosted inference, or model downloads.

## Build on Arch / Omarchy

Install Rust, the WASM target, Trunk, and the native build dependencies:

```sh
sudo pacman -S --needed base-devel pkgconf webkit2gtk-4.1 libayatana-appindicator pipewire wl-clipboard cmake git cuda
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

Install a CUDA-enabled `llama-server` from [llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md#cuda), available on PATH. CPU-only llama.cpp builds do not provide GPU acceleration.

From the repository root:

```sh
./scripts/download-models.sh
./scripts/build-asr.sh
./scripts/build.sh
install -Dm755 target/release/rafiflow ~/.local/bin/rafiflow
~/.local/bin/rafiflow
```

Setup needs internet access and approximately 15 GB free space for models and build outputs. Runtime is offline. Downloads are pinned and SHA-256 verified. Existing model files are checked, never silently replaced.

The supplied Cohere Q8_0 GGUF uses **CrispASR**, not PyTorch or llama.cpp. Qwen3-4B-Instruct-2507 uses Q4_K_M with llama.cpp and is a non-thinking model. The initially considered Python worker was removed after the model selection changed to GGUF.

## First dictation

1. Open Settings & setup. Defaults point to the downloaded models and built CrispASR executable. Confirm the CUDA `llama-server` path.
2. On Hyprland 0.55+, rafiFlow installs a runtime rule for its own recording window. The helper also installs this rule on configuration reload, keeping the overlay borderless. Copy the supplied `scripts/rafiflow-hyprland.lua` somewhere stable and include it from your Omarchy `~/.config/hypr/hyprland.lua`. Check for conflicting Ctrl + Alt + Space bindings first. Validate changes with `hyprctl reload` and `hyprctl configerrors`.
3. Focus a text field in another app. Hold **Ctrl + Alt + Space**, speak, then release Space. The helper tracks the press and catches release even if Ctrl or Alt is released first. Ordinary Space releases do not send recording commands.
4. Keep the target window focused. If focus changes, rafiFlow copies the result but skips paste. History retains both the raw and edited text.
5. The overlay disappears on release. Transcription and cleanup continue silently, and the finished text is inserted when ready.
6. Closing the main window keeps rafiFlow running. Open it or quit from its tray menu. Launching rafiFlow again opens the existing instance.

Use `rafiflow --background` for login startup without opening the main window. An XDG autostart entry under `~/.config/autostart/` can run the installed executable with this argument. The main app-menu launcher should omit it so it opens the window when requested.

For Hyprland versions using `.conf` configuration, use compositor press/release bindings that call the same `rafiflow --control start` and `rafiflow --control stop` commands. The supplied Lua file targets Omarchy's newer Lua setup. Pasting uses Hyprland’s native shortcut dispatcher, with Shift+Insert for windows tagged as terminals. Other desktops can use these commands with their own shortcut tools; safe automatic paste currently requires Hyprland. Disable automatic paste to use clipboard-only dictation elsewhere.

The in-app Record button is useful for testing. Use the global shortcut with the receiving app focused for normal dictation. Clipboard copying replaces the current clipboard and does not restore it.

## Editing and privacy

- Raw bypasses Qwen entirely.
- Light fixes grammar, punctuation, and filler.
- Standard also resolves corrections and repetition.
- Aggressive is the default: moderate rewriting that preserves conversational tone and spoken politeness, applies corrections in place, and formats paragraphs and Markdown lists. It should retain explicit restrictions without inventing new ones.
- Custom uses your own editing instructions.

Every AI cleanup mode uses a built-in editing system prompt. Dictated questions stay questions, and dictated requests are edited rather than answered or executed. Custom instructions change editing style within that role. Use Cleanup studio to compare results under different settings. Vocabulary hints guide cleanup, but do not fine-tune ASR or guarantee spelling. Model output can still make mistakes; compare the original transcript in history.

Settings and history live under `$XDG_DATA_HOME/rafiflow`, normally `~/.local/share/rafiflow`, with private directory and file permissions. History stores up to 500 text entries and can be disabled or deleted. Audio is temporary and removed after transcription or cancellation. Local inference servers use random loopback ports and per-process API keys. No browser code can reach them directly.

Both model processes stay loaded for five idle minutes after the last job, then are terminated to release memory. New dictations reset the timer. Inference is serialized on a worker thread; the UI remains independent. ASR and cleanup implement separate Rust traits.

## Memory and limits

Qwen defaults to **24 GPU layers**, with remaining layers on CPU, a 4096-token context, and small batches. This leaves more room for Cohere and the desktop on an 8 GB GPU. Adjust in Settings based on measured usage. No silent model substitution or automatic quantization occurs. CUDA allocation errors are surfaced, and cleanup failure retains the raw transcript without pasting it.

ASR is forced to English (`-l en` and `language=en`). The selected Cohere model is multilingual: an English decoding tag is not a reliable spoken-language detector. Arabic or other non-English speech may still produce text, including English guesses or transliterations. Strictly rejecting such audio is not implemented; no model has been replaced or added.

Recordings are limited to 90 seconds and discarded if that limit is exceeded. An RMS gate and the classical WebRTC voice-activity detector reject quiet or non-speech recordings without adding another model. Noise or music can still cause ASR hallucinations. Long transcripts or custom prompts can exceed the cleanup context; truncated output is rejected rather than pasted.

On this machine, WebKit's DMA-BUF renderer fails with a Wayland protocol error and an unavailable Mesa LLVM library. Launching with `WEBKIT_DISABLE_DMABUF_RENDERER=1 rafiflow` allows UI testing. This is a host graphics-stack limitation and does not disable CUDA inference. It is not forced on other installations.

## Verification

```sh
cargo test -p rafiflow-core
NO_COLOR=true trunk build
cargo build -p rafiflow
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p rafiflow-core --example evaluate
cargo run -p rafiflow-core --example cleanup_regressions -- /tmp/cleanup-report.json
cargo run -p rafiflow-core --example evaluate -- /path/to/mono-16khz-s16.wav
```

Stop rafiFlow or wait for its models to unload before running the GPU evaluator, to avoid loading duplicate copies on an 8 GB GPU.

The cleanup regression evaluator uses the ten exact user-supplied raw dictations with a fresh Qwen server per case, bypassing ASR and isolating prompt caches. It writes every output and content-check failure to JSON, then fails if any check failed. Inspect the outputs too: token assertions cannot establish semantic fidelity.

The evaluator invokes real local models, logs the raw and cleaned results, and checks correction, formatting, and technical constraints. It never copies or pastes. See the technical reference in AGENTS.md for verified outcomes and remaining limitations.
