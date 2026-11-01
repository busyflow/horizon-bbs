# BBS 1.20.1 Compatibility & Configuration

`horizon-bbs` supports Minecraft `1.20.1` (`DataVersion: 3465`) BBS film and replay definitions while targeting Horizon's Rust engine (`26.3` / `d91cab68ded29bd2b004f7050c468fb98c41a00b`).

## Supported 1.20.1 Features

| Subsystem | Supported Features |
| --- | --- |
| **Keyframe Channels** | `x`, `y`, `z`, `yaw`, `head_yaw`, `body_yaw`, `pitch`, `sneaking`, `sprinting`, `grounded`, `fall_distance`, `visible`, `color` |
| **Interpolations** | `linear`, `step`, `cubic`, `hermite`, `bezier`, `quad_in`, `quad_out`, `quad_in_out`, `sine_in`, `sine_out`, `sine_in_out`, `exp_in`, `exp_out`, `exp_in_out` |
| **Forms** | `bbs:model`, `bbs:block`, `bbs:item`, `bbs:mob`, `bbs:particle`, `bbs:extruded`, `bbs:label` |
| **Camera Clips** | `bbs:idle`, `bbs:dolly`, `bbs:circular`, `bbs:path`, `bbs:look`, `bbs:orbit`, `bbs:shake`, `bbs:curve` |
| **Scene & Workflow Enhancements** | Instant category assignment on creation, cross-scene category copy/paste, multi-replay combined-center trajectory transform, random texture pool assignment, and main torso/chest anchor orbit focus |

## Exporting Default Configuration

```sh
cargo run --quiet --locked --offline --bin bbs-config > my-bbs.json
HORIZON_BBS_CONFIG="$PWD/my-bbs.json" /path/to/custom/horizon-client <game-arguments>
```
