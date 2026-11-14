# Aggressive cleanup evaluation

Evaluated 2026-11-14 with the selected Qwen3-4B-Instruct-2507 Q4_K_M GGUF, CUDA llama.cpp build 10655, 24 GPU layers, temperature 0, 4096-token context, and thinking disabled. Every exact input bypassed ASR and ran in a newly started server, without reading application history or changing the clipboard.

The baseline is main commit `a572c64`. Baseline: **5/10** cases passed the content checks. Revised prompt: **10/10**. Both output sets use the same final checks. Trailing Markdown hard-break spaces are omitted below; text and line breaks are unchanged. Outputs were also read for meaning, correction placement, unrelated tasks, politeness, and formatting. These examples do not prove universal fidelity; wording may differ from the reference examples.

The existing nineteen model checks also passed, covering scope, technical/numeric constraints, Light/Standard/Custom editing, and twelve question-preservation cases. Workspace tests (seven), Clippy, formatting, and release builds passed.

The application already forced English in both the ASR startup flag and request. The model remains unchanged. This selects English decoding; it does not reliably reject Arabic or other non-English audio. No language detector was added, and Arabic-audio rejection was not verified.

Run: `cargo run -p rafiflow-core --example cleanup_regressions -- /tmp/cleanup-report.json`

| Case | Before | After |
| --- | --- | --- |
| 1. numbered-list | PASS | PASS |
| 2. replace-second-feature | FAIL | PASS |
| 3. bullet-list | FAIL | PASS |
| 4. filenames-no-invented-restriction | FAIL | PASS |
| 5. final-plan-only-intention | PASS | PASS |
| 6. keep-please | FAIL | PASS |
| 7. do-not-add-please | PASS | PASS |
| 8. scratch-whole-instruction | PASS | PASS |
| 9. scratch-partial-instruction | FAIL | PASS |
| 10. no-invented-pr-restrictions | PASS | PASS |

## 1. numbered-list

Exact raw input:

> Okay, I need three things done. First, fix the player movement. Second, add jumping. And third, make the camera follow the player smoothly.

Before:

```text
Do three things:

1. Fix the player movement.
2. Add jumping.
3. Make the camera follow the player smoothly.
```

After:

```text
1. Fix the player movement.
2. Add jumping.
3. Make the camera follow the player smoothly.
```

Reference example (equivalent wording is allowed):

```text
1. Fix the player movement.
2. Add jumping.
3. Make the camera follow the player smoothly.
```

## 2. replace-second-feature

Exact raw input:

> So, um, I want four features. First, add shadows. Second, add reflections. Actually, no, scratch reflections, add ambient occlusion instead. Third, add better lighting. And fourth, add a debug overlay.

Before:

```text
Add the following four features:

1. Shadows.
2. Ambient occlusion.
3. Better lighting.
4. A debug overlay.

Scratch reflections.
```

After:

```text
1. Add shadows.
2. Add ambient occlusion.
3. Add better lighting.
4. Add a debug overlay.
```

Reference example (equivalent wording is allowed):

```text
I want four features:

1. Add shadows.
2. Add ambient occlusion.
3. Add better lighting.
4. Add a debug overlay.
```

Baseline check failures: Unwanted: reflection; Unwanted: scratch.

## 3. bullet-list

Exact raw input:

> Okay, so for the app I want it to start automatically, and, uh, I want it to minimize to the tray. Also it should remember my settings, and it needs to work without internet, and I don't want notifications popping up all the time.

Before:

```text
The app should:

- Start automatically at boot.
- Minimize to the system tray.
- Remember user settings.
- Function without an internet connection.
- Avoid frequent notifications.

Do not modify anything yet.
```

After:

```text
- The app should start automatically.
- The app should minimize to the tray.
- It should remember my settings.
- It needs to work without internet.
- I don't want notifications popping up all the time.
```

Reference example (equivalent wording is allowed):

```text
The app should:

- Start automatically.
- Minimize to the tray.
- Remember user settings.
- Function without internet.
- Avoid frequent notifications.
```

Baseline check failures: Unwanted: do not modify.

## 4. filenames-no-invented-restriction

Exact raw input:

> I need you to update three files, agents dot M D, cargo dot toml, and main dot R S. In the agents file, update the build instructions. In cargo, add the dependency. And in main, initialize the new system.

Before:

```text
Update three files:

1. In `agents.md`, update the build instructions.
2. In `cargo.toml`, add the dependency.
3. In `main.rs`, initialize the new system.
```

After:

```text
Update three files:

- `AGENTS.md`: Update the build instructions.
- `Cargo.toml`: Add the dependency.
- `main.rs`: Initialize the new system.
```

Reference example (equivalent wording is allowed):

```text
Update the following files:

- `AGENTS.md`: Update the build instructions.
- `Cargo.toml`: Add the dependency.
- `main.rs`: Initialize the new system.
```

Baseline check failures: Missing: AGENTS.md; Missing: Cargo.toml.

## 5. final-plan-only-intention

Exact raw input:

> Okay, make a plan to add multiplayer, matchmaking, and dedicated servers. Actually wait, forget multiplayer entirely. I only want single-player AI bots, better enemy pathfinding, and a difficulty setting. Don't implement anything yet.

Before:

```text
Make a plan to add:

- Single-player AI bots
- Better enemy pathfinding
- A difficulty setting

Do not implement anything yet.
```

After:

```text
Make a plan for the following features:

1. Add single-player AI bots.
2. Improve enemy pathfinding.
3. Add a difficulty setting.

Do not implement anything yet.
```

Reference example (equivalent wording is allowed):

```text
Make a plan to add single-player AI bots with improved enemy pathfinding and difficulty setting.

Do not implement anything yet.
```

## 6. keep-please

Exact raw input:

> Please fix the collision system and add some tests.

Before:

```text
Fix the collision system and add some tests.
```

After:

```text
Please fix the collision system and add some tests.
```

Reference example (equivalent wording is allowed):

```text
Please fix the collision system and add tests.
```

Baseline check failures: Missing: Please.

## 7. do-not-add-please

Exact raw input:

> Fix the collision system and add tests.

Before:

```text
Fix the collision system and add tests.
```

After:

```text
Fix the collision system and add tests.
```

Reference example (equivalent wording is allowed):

```text
Fix the collision system and add tests.
```

## 8. scratch-whole-instruction

Exact raw input:

> Add multiplayer support. Scratch that. Make it single-player with AI bots.

Before:

```text
Make it single-player with AI bots.
```

After:

```text
Make it single-player with AI bots.
```

Reference example (equivalent wording is allowed):

```text
Make the game single-player with AI bots.
```

## 9. scratch-partial-instruction

Exact raw input:

> Add shadows, reflections, and better lighting. Scratch reflections, use ambient occlusion instead.

Before:

```text
Add shadows, reflections, and better lighting. Scratch reflections; use ambient occlusion instead.
```

After:

```text
- Add shadows.
- Add ambient occlusion.
- Add better lighting.
```

Reference example (equivalent wording is allowed):

```text
- Add shadows.
- Add ambient occlusion.
- Improve lighting.
```

Baseline check failures: Unwanted: reflection; Unwanted: scratch; Unwanted: instead.

## 10. no-invented-pr-restrictions

Exact raw input:

> Fix the player movement and create a pull request.

Before:

```text
Fix the player movement and create a pull request.
```

After:

```text
Fix the player movement and create a pull request.
```

Reference example (equivalent wording is allowed):

```text
Fix the player movement and create a pull request.
```

The baseline reproduced the trailing scratch sentence and removal of “please.” It added an unspoken restriction in the app-settings case; the file-update case reproduced lowercase filenames but did not append the restriction in this independent run. The revised output keeps “some tests,” a harmless wording difference from the reference. No heuristic correction post-processing or inference-setting changes were added.

## Context before a new-topic list regression

The eleven-case suite now includes the exact Graphy Flow/Linux dictation. The baseline reproduced the missing introductory context. A focused Aggressive prompt change preserves statements, questions, and transitions while retaining numbered enumeration after narrative context. No examples, model settings, or correction post-processing changed.

Raw input:

> Okay, so I worked on it a little. It's called Graphy Flow, it's getting very good. I've ran some tests. How about I'll give you a good test now? I will say I use Linux because first it is faster, second, it's more customizable. Third, it's free and open source. Fourth, it has way less bloat. Fifth, it's a lot more stable.

Before:

```text
I use Linux because:
- It is faster.
- It is more customizable.
- It is free and open source.
- It has way less bloat.
- It is a lot more stable.
```

After:

```text
Okay, so I worked on it a little. It's called Graphy Flow, and it's getting very good. I've run some tests.

How about I give you a good test now? I use Linux because:
1. It is faster.
2. It's more customizable.
3. It's free and open source.
4. It has way less bloat.
5. It's a lot more stable.
```

Run just this case: `cargo run -p rafiflow-core --example cleanup_regressions -- /tmp/context.json context-before-new-topic-list`. The content checks require every meaningful point, the question mark, paragraph separation, and all five numbered reasons. All eleven cases passed in independent fresh CUDA-backed Qwen processes after the change.

## Mandatory active cleanup for short dictations

Baseline: `3b24ff8`. Same selected Qwen GGUF, temperature zero, 24 CUDA layers, and a fresh local server for each case. All fourteen current cases pass after the change; the older nineteen model checks also pass. The rough-input checks explicitly reject unchanged output, while an already-clean statement is checked for exact preservation. The old politeness assertions were removed to match the user-requested removal of politeness rules; task content is still checked.

`State::finish` has no short-input bypass. Only Raw mode skips cleanup. Cleanup errors are attached to the preserved raw transcript and return before automatic insertion. A unit regression verifies short, long, and clean Aggressive inputs all invoke the backend and consume its output. These tests verify specific examples, not universal fidelity of generated text.

### context-before-new-topic-list

Raw:

```text
Okay, so I worked on it a little. It's called Graphy Flow, it's getting very good. I've ran some tests. How about I'll give you a good test now? I will say I use Linux because first it is faster, second, it's more customizable. Third, it's free and open source. Fourth, it has way less bloat. Fifth, it's a lot more stable.
```

Before:

```text
I use Linux because:
1. It is faster.
2. It is more customizable.
3. It is free and open source.
4. It has way less bloat.
5. It is a lot more stable.
```

After:

```text
I worked on it a little. It's called Graphy Flow, and it's getting very good. I've run some tests.

How about I give you a good test now?

I use Linux because:

1. It is faster.
2. It is more customizable.
3. It is free and open source.
4. It has way less bloat.
5. It is a lot more stable.
```

### short-awkward-update

Raw:

```text
Okay, so I worked on it a little. RafiFlow, it's looking good.
```

Before:

```text
Okay, so I worked on it a little. RafiFlow, it's looking good.
```

After:

```text
I worked on RafiFlow a little, and it's looking good.
```

### short-grammar-and-filler

Raw:

```text
Um, I've ran the tests, uh, they all passes.
```

Before:

```text
I've run the tests; they all pass.
```

After:

```text
I ran the tests, and they all pass.
```

### already-clean-statement

Raw:

```text
The build passed.
```

Before:

```text
The build passed.
```

After:

```text
The build passed.
```
