# Repository Workflow

- This project is a GitHub repository.
- For implementation changes, create a pull request and merge it after appropriate verification passes.
- Do not use the `babysit-pr` skill. There are no reviewers, so merge without waiting for a review.

# Project Philosophy (Human-Owned)

## Introduction

I'm building rafiFlow becuse there are no good free, open source and local alternative to wispr flow.

The idea is to click a button speak and get a transcription with wispr flow style lists and 'scratch that'.

I want this to be come a personal daily-use tool.

This section represents my vision for the project. Agents can suggest improvements, but they shouldn't change my goals, preferences, or decisions without my approval.

## What I'm trying to achieve

The main thing i want is to get fast transcription and good quality.

When someone uses this application. i want the experience to feel fast, smiple, and polished.

A good example of how it should work is:

I hold the key-bind for transcription,
Speak what i want,
Let go of the key-bind,
What i said cleaned up gets put in my clipboard and pasted automatically.

What I don't want is it getting names and words wrong and waiting a long time for my text.

## How i want it to work

These are the main things i care about:

- Accurate speech recognition, Fast, reliable transcription that understands normal speech, accents, and technical vocabulary.
- Intelligent AI cleanup, Remove filler words, repetitions, abandoned thoughts, and self-corrections. Rewrite everything as if I had my final intention clear from the beginning.
- Automatic formatting, Turn speech into properly formatted paragraphs, numbered lists, bullet points, and Markdown when appropriate.
- Wispr Flow-style overlay, A beautiful, minimal floating recording indicator with animated audio visualization, smooth transitions, and recording status.
- Custom vocabulary, Teach the application technical terms, project names, abbreviations, and unusual words.
- Configurable cleanup, Choose between raw transcription, light cleanup, normal cleanup, aggressive rewriting, and custom instructions.
- Dictation history, View, search, copy, and compare original and cleaned transcripts.

## The experience

I want the UI to feel like wispr flow.

I particularly care about responsiveness and animations.

I don't want to spent hours trying to configure it to work well.

I don't want [things you dislike about other applications].

# Technical Reference (Agent-Maintained)

## Architecture

- Cargo workspace: `crates/core` owns recording, inference, persistence, and Hyprland/clipboard integration; `crates/desktop` owns Tauri 2 windows, tray, commands, and local control socket; `crates/ui` is Yew compiled to WASM by Trunk. No Node toolchain is required.
- One worker thread serializes recording and inference. Bounded control requests prevent overlapping jobs. Press/release commands use a private Unix socket; an advisory file lock prevents duplicate instances per data directory.
- `SpeechBackend` and `CleanupBackend` are separate traits in `backend.rs`. Current implementations own local server processes and authenticated loopback clients. Replacements belong in this module without changing recording or UI.
- ASR: Cohere Transcribe 03-2026 Q8_0 GGUF from `cstr`, using pinned CUDA CrispASR revision `1c7f6f79e05bd06ec614d203af05d5ae337ec254`. It cannot be loaded by PyTorch or regular llama.cpp. The previously considered Python worker was removed after the user selected GGUF.
- Cleanup: user-selected `yuko29hu/Qwen3-4B-Instruct-2507-Q4_K_M-GGUF` through CUDA llama-server. Non-thinking Instruct variant, temperature zero, 4096-token context, 1536-token output limit, and 24 GPU layers by default. Prompt examples distinguish editing dictated requests from fulfilling them.
- Model processes remain warm for 300 idle seconds after a job completes. Recording prevents idle eviction. Failures invalidate the failed backend; child processes are killed/reaped on drop and receive a Linux parent-death signal.
- PipeWire records raw mono s16 at 16 kHz. A reader thread writes WAV and exposes RMS for the overlay. RMS and classical WebRTC VAD gate non-speech. There is no third inference model.
- Raw transcripts are persisted before cleanup when history is enabled. Atomic JSON replacement preserves up to 500 raw/cleaned entries. Corrupt history is surfaced rather than overwritten. Cleanup failure never automatically pastes raw text.
- Modern Hyprland gets a named runtime rule for the recording overlay, without editing user config. Compositor press/release bindings invoke `rafiflow --control start|stop|cancel`. Native Hyprland shortcut dispatch handles paste; terminal-tagged windows use Shift+Insert. Focus mismatch copies but refuses paste.

## Build and run

- Arch native dependencies: Rust/Cargo, wasm32-unknown-unknown, Trunk, webkit2gtk-4.1, libayatana-appindicator, PipeWire, wl-clipboard, base-devel/pkgconf, CMake, CUDA, Git. CUDA llama-server must be installed separately.
- `scripts/download-models.sh` uses curl, immutable Hugging Face revisions, and SHA-256 checks. Existing files are verified, not replaced. Both requested files have been downloaded and hash-verified locally.
- `scripts/build-asr.sh` builds the pinned CrispASR checkout under the application data directory with GGML_CUDA. Its default parallelism is four jobs. The tested host compiler is GCC 16 with CUDA 13.3, targeting NVIDIA SM 89.
- `scripts/build.sh`: Trunk release frontend, then Cargo release desktop. Run `target/release/rafiflow`; install to ~/.local/bin if desired. Build the frontend before the desktop because generate_context embeds `dist/`.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`. `cargo run -p rafiflow-core --example evaluate -- /path/to/mono-16khz-s16.wav` exercises actual ASR and cleanup with content assertions. Without a path it evaluates cleanup only.
- `desktop_smoke` records a short microphone sample by default; its optional argument is an explicit focused test-window address for a real paste check. It changes the clipboard. Use isolated application state and an expendable text field.
- Settings/history/socket are under $XDG_DATA_HOME/rafiflow, normally ~/.local/share/rafiflow. Data directories use mode 0700; JSON uses 0600. Models remain external to Git. Never commit recordings, personal history, or model weights.
- Omarchy Lua shortcut example: `scripts/rafiflow-hyprland.lua`. Check existing bindings before enabling it. Validate local config edits with `hyprctl reload` and `hyprctl configerrors`.

## Verified behavior and limits

- Tested on Arch/Omarchy, Hyprland 0.56.2, an RTX 4070 Laptop GPU with 8 GB VRAM, and CUDA llama.cpp build 10655. This is representative of the requested memory class, not an RTX 4060 benchmark.
- Cohere Q8_0 selected CUDA0 and transcribed an 11.4-second synthesized English correction/list recording in roughly 0.33 seconds after startup. Qwen preserved the tested filename/number/API constraints, resolved corrections, produced Markdown lists, and handled the final plan-only decision. Natural microphone speech accuracy and accents still need user evaluation; synthesized speech is not proof of daily-use quality.
- Both models ran concurrently. Observed process VRAM was approximately 2674 MiB for Cohere and 2202 MiB for Qwen with the defaults. Other GPU applications reduce available headroom. Running a second inference evaluator while the GUI already owns warm models can exhaust 8 GB; stop the GUI or let it unload before GPU evaluation.
- Native UI rendering, studio cleanup, settings changes, history persistence, microphone capture, recording overlay visibility, cancellation, and temporary F24 press/release bindings were exercised with isolated XDG_DATA_HOME. Test bindings were removed.
- Native compositor paste inserted the expected identifiers and numbers into an isolated GTK text field. Focus mismatch refused insertion. wtype shortcut injection failed this GTK test despite a zero exit status, so production paste uses the native compositor dispatcher.
- WebKit DMA-BUF rendering on this host fails due to a missing Mesa LLVM library and a Wayland protocol error. `WEBKIT_DISABLE_DMABUF_RENDERER=1` permits native UI testing and does not disable CUDA. The app does not force it on other hosts.
- The recording window is fixed at 340x90 logical pixels around the 326x76 capsule. GTK resizable mode with identical minimum/maximum bounds avoids the previous 200-pixel natural-height allocation. Both HTML root/body and native GTK backgrounds are transparent. Native GDK input regions are emptied on realize/map, so pointer events pass through the capsule and its margins. Hyprland rules remove decoration, border, blur, and shadow; the Lua helper reapplies them on config reload. Other compositor versions and HiDPI layouts need additional testing.
- Recording limit: 90 seconds, then discard. ASR requests time out after 180 seconds; cleanup after 120 seconds. Oversized context or truncated/empty/reasoning cleanup output produces an error, with raw transcript retained.
- VAD reduces non-speech hallucinations but cannot eliminate them. Vocabulary currently guides cleanup rather than ASR biasing. Custom model prompts and fine-tunes require rerunning the real evaluator.
- Automatic paste requires Hyprland. Clipboard-only recording can be used elsewhere by disabling automatic paste. Legacy Hyprland dispatch and non-Omarchy terminal tagging have not been verified. Build scripts do not install desktop packaging or an updater. User-requested login startup can use an XDG autostart entry with --background.

- Final real evaluator passed with WebRTC VAD enabled, including ASR-to-cleanup preservation of the global target, four correction/constraint/list cases, and Light/Standard/Custom modes. Six automated core tests, workspace Clippy, and frontend/desktop release builds passed. The GUI's real idle timeout was also observed to release both GPU processes.
- Known performance follow-up from Sol review: ordinary silent/VAD-rejected recordings currently invalidate the ASR backend, so the next recording may reload it before the five-minute timer expires.

- Ctrl+Alt+Space push-to-talk uses a held flag and a non-consuming Space-release handler with ignore_mods and transparent flags. Hyprland shadowing otherwise suppresses release when modifiers go up first. Both release orders were verified through actual compositor key events, and a physical click through the visible capsule reached an underlying GTK button.

- The built-in DEFAULT_CLEANUP_SYSTEM_PROMPT in crates/core/src/lib.rs applies to every AI cleanup mode. It separates the editor from the addressed recipient and forbids answering, calculating, acknowledging, or fulfilling dictation. Shared question examples reinforce that role. The real CUDA cleanup evaluator verifies three questions across Light, Standard, Aggressive, and Custom, including the reported “Is this working?” case; the previous prompt failed by answering an arithmetic question.

- --background sets main-window visibility before Tauri creates it, leaving the tray and dictation controls running without a startup flash. A normal second launch routes a show request to the existing instance. The configured XDG autostart entry is registered through the active xdg-desktop-autostart.target.
- The overlay is shown only in recording phase. Stop/cancel admission leaves recording phase before WAV finalization, and inference runs without overlay content or a transcribing animation/message. A persistent overlay-page element keeps the canvas transparent even while no capsule is rendered. A delayed-worker native test verified hiding during an unfinished job, background startup, and reopening the same instance.
