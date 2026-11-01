# Horizon BBS 26.3 (`horizon-bbs`)

A native Rust port of **Blockbuster Studio (BBS 26.3)** for **Horizon's Rust engine** (Minecraft `26.3`, `DataVersion: 5023`, Native Hooks API `hooks: 1`), built with [`horizon-mixins`](https://github.com/horizon-rust/horizon-mixins) and following Horizon's official Native Hooks architecture (`crates/horizon-client/src/game/native_hooks.rs`).

Because `horizon-bbs` targets Horizon's stable `hooks: 1` API (`native_mods`, `native_settings`, `native_settings_click`, `native_tick`, `native_key`, `native_hud`, `native_command`), **Horizon Launcher builds it cleanly across all Horizon 26.3 releases and nightlies** alongside other native mods such as `fps_overlay.hnmod`.

---

## Features & Minecraft 26.3 BBS Engine

- **Minecraft 26.3 Film & Replay Schema (`DataVersion: 5023`, MC `26.3`):**
  - Native 26.3 scene serialization (`Film::from_26_3_json`, `Film::to_26_3_json`) with automatic upgrade for legacy 1.20.1 scenes (`Film::upgrade_to_26_3`, `Film::from_1201_json`).
  - Full `Replay` channel support (`x`, `y`, `z`, `yaw`, `head_yaw`, `body_yaw`, `pitch`, `sneaking`, `sprinting`, `grounded`, `fall_distance`, `visible`, `color`).
  - Complete `Interpolation` curve evaluator (`linear`, `step`, `cubic` Catmull-Rom, `hermite`, `bezier`, `quad_in/out/inout`, `sine_in/out/inout`, `exp_in/out/inout`).
- **26.3 Forms, Data Components, Poses & Main Anchor Orbit Camera:**
  - Supports `bbs:model`, `bbs:block`, `bbs:item`, `bbs:mob`, `bbs:display`, `bbs:particle`, `bbs:extruded`, and `bbs:label` forms with 26.3 item/entity `components` (`minecraft:item_model`, `minecraft:custom_data`), skeletal `Pose`, `pose_overlay`, and `BodyPart` trees.
  - **Orbit Camera Torso Anchor (`Form::main_anchor_bone`):** Locks camera orbit focus onto the primary torso/chest bone (`anchor`, `torso`, `body`, `chest`, `root`) at chest height (`+0.9m`) rather than shifting focus to individual limbs (`left_arm`, `right_arm`, etc.) being edited in pose keyframes.
- **Categories / Groups & Cross-Scene Clipboard:**
  - Creating a category/group (`Film::create_group_with_selected` / `/bbs group <name>`) immediately moves all currently selected replays into the new category.
  - Full group copy & paste (`Film::copy_group`, `Film::paste_group` / `/bbs copy-group <name>`, `/bbs paste-group`) copies a category, its nested subgroups, and all contained replays and pastes them into any scene.
- **Multi-Replay Combined Center Transform Gizmo:**
  - Computes the exact combined center point of any number of selected replays (`Film::combined_replays_center`) and smoothly translates/rotates all selected replay trajectories around their shared center (`Film::transform_selected_replays` / `/bbs transform <dx> <dy> <dz> [dyaw]`).
- **Live In-Game Director HUD, Hotkeys & Options > Mods Page:**
  - **Options > Mods > BBS Studio 26.3**: Configure Director HUD visibility, HUD screen corner (`Top right` default), Playback State, Loop Playback, Camera Lock on Play, and Timeline Speed (`10` / `20` / `40` / `60` TPS), plus customizable keybinds (`P` Play/Pause, `R` Record, `O` Reset Timeline, `U` Toggle HUD).

---

## Launcher & Instance Bundle (`.hnmod`)

Package the deterministic source-only `.hnmod` bundle:

```sh
python3 tools/bundle.py
python3 tools/package.py
```

This writes `target/bbs-0.2.0.hnmod` with `"game": "26.3"` and `"hooks": 1`. Place it in your instance's `horizon-native-mods/bbs.hnmod` (e.g. `%APPDATA%\horizon\instances\bbs-rust\horizon-native-mods\bbs.hnmod`) and enable `mod.bbs:true` in `horizon-mods.txt`.

---

## In-Game Commands (`/bbs`)

- `/bbs` or `/bbs status`: Show 26.3 engine status, active film, replay count, playback mode, and formatted timecode.
- `/bbs hud`: Toggle the live on-screen BBS 26.3 Director HUD overlay.
- `/bbs toggle`, `/bbs on`, `/bbs off`: Enable or disable BBS for your session.
- `/bbs list`: List all loaded films, replay counts, durations, and MC versions.
- `/bbs create <film_id>`: Create and select a new 26.3 BBS film.
- `/bbs play [film_id]`: Start timeline playback.
- `/bbs pause`: Pause timeline playback at the current tick.
- `/bbs stop`: Stop playback and reset cursor to tick `0.0`.
- `/bbs tick <tick>`: Scrub timeline to an exact tick (supports fractional ticks).
- `/bbs record [replay_index]`: Record player movement/rotation into a replay trajectory.
- `/bbs group <group_name>`: Create a category and immediately move selected replays into it.
- `/bbs copy-group <group_name>`: Copy a category and all of its replays to the session clipboard.
- `/bbs paste-group`: Paste the copied category and its replays into the active scene.
- `/bbs transform <dx> <dy> <dz> [dyaw]`: Smoothly translate and rotate all selected replays around their combined center point.
- `/bbs import <path>` (or `/bbs import1201 <path>`): Import a BBS `.json` film file from disk and automatically upgrade it to 26.3 (`DataVersion: 5023`).
- `/bbs reload`: Reload configuration from `HORIZON_BBS_CONFIG` and mod settings.

---

## Build & Validation

```sh
cargo fmt --all --check
rustfmt --edition 2024 --check native/horizon.rs
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
python3 tools/bundle.py --check
python3 tools/test_ci.py
python3 tools/test_package.py
```

## License

[MIT](LICENSE)
