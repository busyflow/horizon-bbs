# Horizon BBS (`horizon-bbs`)

A Rust port of **Blockbuster Studio (BBS 1.20.1)** for **Horizon's Rust engine** (`d91cab68ded29bd2b004f7050c468fb98c41a00b` / Minecraft 26.3), built with [`horizon-mixins`](https://github.com/horizon-rust/horizon-mixins) 0.6.0 and following the native mod architecture established by [`horizon-veinminer`](https://github.com/levzzz5154/horizon-veinminer) and [`horizon-native-mod-template`](https://github.com/horizon-rust/horizon-native-mod-template).

The committed [TOML bundle](mixins/bbs/horizon-mixin.toml) declares two cancellable `HEAD` hooks (`run_command` and `guest_command`) and one shared runtime module (`mixins/bbs/runtime.rs`). The original Horizon checkout remains completely untouched.

---

## Features & 1.20.1 BBS Compatibility

- **1.20.1 Film & Replay Schema (`DataVersion: 3465`, MC `1.20.1`):**
  - Import, validate, evaluate, and export 1.20.1 BBS scenes (`Film::from_1201_json`, `Film::to_1201_json`).
  - Full `Replay` channel support (`x`, `y`, `z`, `yaw`, `head_yaw`, `body_yaw`, `pitch`, `sneaking`, `sprinting`, `grounded`, `fall_distance`, `visible`, `color`).
  - Complete `Interpolation` curve evaluator (`linear`, `step`, `cubic` Catmull-Rom, `hermite`, `bezier`, `quad_in/out/inout`, `sine_in/out/inout`, `exp_in/out/inout`).
- **BBS Forms, Poses & Main Anchor Orbit Camera:**
  - Supports `bbs:model`, `bbs:block`, `bbs:item`, `bbs:mob`, `bbs:particle`, `bbs:extruded`, and `bbs:label` forms with skeletal `Pose`, `pose_overlay`, and `BodyPart` trees.
  - **Orbit Camera Torso Anchor (`Form::main_anchor_bone`):** Automatically locks camera orbit focus onto the primary torso/chest bone (`anchor`, `torso`, `body`, `chest`, `root`) at chest height (`+0.9m`) rather than shifting focus to individual limbs (`left_arm`, `right_arm`, etc.) being edited in pose keyframes.
- **Categories / Groups & Cross-Scene Clipboard:**
  - Creating a category/group (`Film::create_group_with_selected` / `/bbs group <name>`) immediately moves all currently selected replays into the new category.
  - Full group copy & paste (`Film::copy_group`, `Film::paste_group` / `/bbs copy-group <name>`, `/bbs paste-group`) copies a category, its nested subgroups, and all contained replays and pastes them into any scene.
- **Multi-Replay Combined Center Transform Gizmo:**
  - Computes the exact combined center point of any number of selected replays (`Film::combined_replays_center`) and smoothly translates/rotates all selected replay trajectories around their shared center (`Film::transform_selected_replays` / `/bbs transform <dx> <dy> <dz> [dyaw]`).
- **Random Texture Pool Assignment:**
  - Deterministically applies random textures from an asset folder pool across all selected replays (`Film::apply_random_textures`).
- **Camera Clips:**
  - Evaluates `bbs:idle`, `bbs:dolly`, `bbs:circular`, `bbs:path`, `bbs:look`, `bbs:orbit`, `bbs:shake`, and `bbs:curve` camera clips across timeline layers.

---

## Launcher & Registry Bundle (`.hnmod`)

Version `0.1.0` targets Horizon Nightly `d91cab68ded29bd2b004f7050c468fb98c41a00b` and Horizon Mixins `0.6.0` (`4342a0bfbe71d4d14185528d64e496d6022f16a1`).

Package the source-only `.hnmod` bundle for Horizon Launcher:

```sh
python3 tools/bundle.py
python3 tools/package.py
```

This writes `target/bbs-0.1.0.hnmod` and prints its SHA-256. Copy `target/bbs-0.1.0.hnmod` into your Horizon Launcher `horizon-native-mods/` directory, enable **Settings → Game → Native mods (experimental)**, and click **Build**.

---

## In-Game Commands (`/bbs`)

- `/bbs` or `/bbs status`: Show engine status, active film, replay count, playback mode, and formatted timecode.
- `/bbs toggle`, `/bbs on`, `/bbs off`: Enable or disable BBS for your session.
- `/bbs list`: List all loaded films, replay counts, and durations.
- `/bbs create <film_id>`: Create and select a new BBS film.
- `/bbs play [film_id]`: Start timeline playback.
- `/bbs pause`: Pause timeline playback at the current tick.
- `/bbs stop`: Stop playback and reset cursor to tick `0.0`.
- `/bbs tick <tick>`: Scrub timeline to an exact tick (supports fractional ticks).
- `/bbs record [replay_index]`: Record player movement/rotation into a replay trajectory.
- `/bbs group <group_name>`: Create a category and immediately move selected replays into it.
- `/bbs copy-group <group_name>`: Copy a category and all of its replays to the session clipboard.
- `/bbs paste-group`: Paste the copied category and its replays into the active scene.
- `/bbs transform <dx> <dy> <dz> [dyaw]`: Smoothly translate and rotate all selected replays around their combined center point.
- `/bbs import1201 <path>`: Import a 1.20.1 BBS `.json` film file from disk.
- `/bbs reload`: Reload configuration from `HORIZON_BBS_CONFIG` (requires permission level 2).

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
python3 tools/test_stage.py
python3 tools/horizon.py check
python3 tools/horizon.py test
```

## License

[MIT](LICENSE)
