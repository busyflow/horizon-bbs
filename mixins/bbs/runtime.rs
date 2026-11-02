// Native adapter for Horizon 26.3 (`hooks: 1` Native Hooks API).
use super::Game;
use super::native_hooks::NativeMod;
use horizon_client::{extras::Corner, extras_hud::Corners, hud::HudBatch};

mod engine {
    // BBS (Blockbuster Studio) 26.3 Core Engine for Horizon.
// Native Minecraft 26.3 (`DataVersion: 5023`) film, replay, form, keyframe, and camera clip engine
// with seamless backward-compatible import/upgrade support for Minecraft 1.20.1 (`DataVersion: 3465`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const BBS_MC_VERSION: &str = "26.3";
pub const BBS_DATA_VERSION_26_3: u32 = 5023;
pub const BBS_DATA_VERSION_1201: u32 = 3465;
pub const MAX_KEYFRAMES_PER_CHANNEL: usize = 65_536;
pub const MAX_REPLAYS_PER_FILM: usize = 2_048;

pub type ActorId = [u32; 4];
pub type Position = [i32; 3];
pub type Vec3 = [f64; 3];
pub type Euler = [f32; 3];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    #[default]
    Linear,
    Step,
    Cubic,
    Hermite,
    Bezier,
    QuadIn,
    QuadOut,
    QuadInOut,
    SineIn,
    SineOut,
    SineInOut,
    ExpIn,
    ExpOut,
    ExpInOut,
}

impl Interpolation {
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear | Self::Cubic | Self::Hermite => t,
            Self::Step => {
                if t < 1.0 {
                    0.0
                } else {
                    1.0
                }
            }
            Self::Bezier => t * t * (3.0 - 2.0 * t),
            Self::QuadIn => t * t,
            Self::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
            Self::SineIn => 1.0 - (t * std::f64::consts::FRAC_PI_2).cos(),
            Self::SineOut => (t * std::f64::consts::FRAC_PI_2).sin(),
            Self::SineInOut => -((std::f64::consts::PI * t).cos() - 1.0) * 0.5,
            Self::ExpIn => {
                if t <= 0.0 {
                    0.0
                } else {
                    2.0_f64.powf(10.0 * t - 10.0)
                }
            }
            Self::ExpOut => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2.0_f64.powf(-10.0 * t)
                }
            }
            Self::ExpInOut => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else if t < 0.5 {
                    2.0_f64.powf(20.0 * t - 10.0) * 0.5
                } else {
                    (2.0 - 2.0_f64.powf(-20.0 * t + 10.0)) * 0.5
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyframe {
    pub tick: f64,
    pub value: f64,
    #[serde(default)]
    pub interp: Interpolation,
    #[serde(default)]
    pub lx: f64,
    #[serde(default)]
    pub ly: f64,
    #[serde(default)]
    pub rx: f64,
    #[serde(default)]
    pub ry: f64,
}

impl Keyframe {
    pub fn new(tick: f64, value: f64, interp: Interpolation) -> Self {
        Self {
            tick,
            value,
            interp,
            lx: -5.0,
            ly: 0.0,
            rx: 5.0,
            ry: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyframeChannel {
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
}

impl KeyframeChannel {
    pub fn insert(&mut self, kf: Keyframe) {
        if !kf.tick.is_finite() || !kf.value.is_finite() {
            return;
        }
        if let Some(existing) = self
            .keyframes
            .iter_mut()
            .find(|k| (k.tick - kf.tick).abs() < 1e-6)
        {
            *existing = kf;
        } else {
            self.keyframes.push(kf);
            self.keyframes
                .sort_by(|a, b| a.tick.partial_cmp(&b.tick).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    pub fn offset_value(&mut self, delta: f64) {
        if !delta.is_finite() {
            return;
        }
        for kf in &mut self.keyframes {
            kf.value += delta;
        }
    }

    pub fn offset_tick(&mut self, delta_tick: f64) {
        if !delta_tick.is_finite() {
            return;
        }
        for kf in &mut self.keyframes {
            kf.tick += delta_tick;
        }
    }

    pub fn evaluate(&self, tick: f64) -> Option<f64> {
        let len = self.keyframes.len();
        if len == 0 {
            return None;
        }
        if len == 1 || tick <= self.keyframes[0].tick {
            return Some(self.keyframes[0].value);
        }
        let last = &self.keyframes[len - 1];
        if tick >= last.tick {
            return Some(last.value);
        }
        let idx = self
            .keyframes
            .partition_point(|kf| kf.tick <= tick)
            .saturating_sub(1);
        let a = &self.keyframes[idx];
        let b = &self.keyframes[idx + 1];
        let span = b.tick - a.tick;
        if span <= 1e-9 {
            return Some(b.value);
        }
        let raw_t = ((tick - a.tick) / span).clamp(0.0, 1.0);
        match a.interp {
            Interpolation::Cubic => {
                let v0 = if idx > 0 {
                    self.keyframes[idx - 1].value
                } else {
                    a.value
                };
                let v1 = a.value;
                let v2 = b.value;
                let v3 = if idx + 2 < len {
                    self.keyframes[idx + 2].value
                } else {
                    b.value
                };
                Some(catmull_rom(v0, v1, v2, v3, raw_t))
            }
            Interpolation::Hermite => {
                let m0 = a.ry;
                let m1 = b.ly;
                let t2 = raw_t * raw_t;
                let t3 = t2 * raw_t;
                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + raw_t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;
                Some(h00 * a.value + h10 * m0 + h01 * b.value + h11 * m1)
            }
            other => {
                let u = other.apply(raw_t);
                Some(a.value + (b.value - a.value) * u)
            }
        }
    }
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    #[serde(default)]
    pub translate: [f32; 3],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
    #[serde(default)]
    pub rotate: [f32; 3],
    #[serde(default)]
    pub rotate2: [f32; 3],
    #[serde(default)]
    pub pivot: [f32; 3],
}

fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translate: [0.0, 0.0, 0.0],
            scale: default_scale(),
            rotate: [0.0, 0.0, 0.0],
            rotate2: [0.0, 0.0, 0.0],
            pivot: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    #[serde(default)]
    pub bones: BTreeMap<String, Transform>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPart {
    #[serde(default)]
    pub limb: String,
    #[serde(default = "default_true")]
    pub use_target: bool,
    #[serde(default)]
    pub transform: Transform,
    pub form: Option<Box<Form>>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// BBS 26.3 form ID (`bbs:model`, `bbs:block`, `bbs:item`, `bbs:mob`, `bbs:display`, `bbs:particle`, `bbs:extruded`, `bbs:label`).
    pub id: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub animatable: bool,
    #[serde(default)]
    pub track_name: String,
    #[serde(default)]
    pub shader_shadow: bool,
    #[serde(default = "default_one_f32")]
    pub ui_scale: f32,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub texture: Option<String>,
    #[serde(default)]
    pub block_state: Option<String>,
    #[serde(default)]
    pub item_stack: Option<String>,
    /// Minecraft 26.3 item/entity data components (`minecraft:item_model`, `minecraft:custom_data`, etc.).
    #[serde(default)]
    pub components: BTreeMap<String, String>,
    #[serde(default)]
    pub mob_id: Option<String>,
    #[serde(default)]
    pub mob_nbt: Option<String>,
    #[serde(default = "default_color")]
    pub color: u32,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub pose: Pose,
    #[serde(default)]
    pub pose_overlay: Pose,
    #[serde(default)]
    pub body_parts: Vec<BodyPart>,
}

fn default_one_f32() -> f32 {
    1.0
}

fn default_color() -> u32 {
    0xFFFF_FFFF
}

impl Default for Form {
    fn default() -> Self {
        Self {
            id: "bbs:model".into(),
            visible: true,
            animatable: true,
            track_name: String::new(),
            shader_shadow: true,
            ui_scale: 1.0,
            model: "player/steve".into(),
            texture: Some("assets:textures/entity/steve.png".into()),
            block_state: None,
            item_stack: None,
            components: BTreeMap::new(),
            mob_id: None,
            mob_nbt: None,
            color: default_color(),
            transform: Transform::default(),
            pose: Pose::default(),
            pose_overlay: Pose::default(),
            body_parts: Vec::new(),
        }
    }
}

impl Form {
    /// Determine the primary torso/chest anchor bone for Orbit Camera mode without shifting to edited limbs.
    pub fn main_anchor_bone(&self, available_bones: &[String]) -> Option<String> {
        if available_bones.is_empty() {
            return None;
        }
        for preferred in ["anchor", "torso", "body", "chest", "root", "waist", "hips"] {
            if let Some(found) = available_bones
                .iter()
                .find(|b| b.eq_ignore_ascii_case(preferred))
            {
                return Some(found.clone());
            }
        }
        available_bones.first().cloned()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayKeyframes {
    #[serde(default)]
    pub x: KeyframeChannel,
    #[serde(default)]
    pub y: KeyframeChannel,
    #[serde(default)]
    pub z: KeyframeChannel,
    #[serde(default)]
    pub yaw: KeyframeChannel,
    #[serde(default)]
    pub head_yaw: KeyframeChannel,
    #[serde(default)]
    pub body_yaw: KeyframeChannel,
    #[serde(default)]
    pub pitch: KeyframeChannel,
    #[serde(default)]
    pub sneaking: KeyframeChannel,
    #[serde(default)]
    pub sprinting: KeyframeChannel,
    #[serde(default)]
    pub grounded: KeyframeChannel,
    #[serde(default)]
    pub fall_distance: KeyframeChannel,
    #[serde(default)]
    pub visible: KeyframeChannel,
    #[serde(default)]
    pub color: KeyframeChannel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub uuid: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub group: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub actor: bool,
    #[serde(default)]
    pub shadow: bool,
    #[serde(default = "default_shadow_size")]
    pub shadow_size: f32,
    #[serde(default)]
    pub looping: i32,
    #[serde(default)]
    pub name_tag: String,
    #[serde(default)]
    pub form: Option<Form>,
    #[serde(default)]
    pub keyframes: ReplayKeyframes,
}

fn default_shadow_size() -> f32 {
    0.5
}

impl Default for Replay {
    fn default() -> Self {
        Self {
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            label: "player/steve".into(),
            group: String::new(),
            enabled: true,
            actor: false,
            shadow: true,
            shadow_size: default_shadow_size(),
            looping: 0,
            name_tag: String::new(),
            form: Some(Form::default()),
            keyframes: ReplayKeyframes::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedReplay {
    pub uuid: String,
    pub label: String,
    pub group: String,
    pub position: Vec3,
    pub yaw: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
    pub pitch: f32,
    pub sneaking: bool,
    pub sprinting: bool,
    pub visible: bool,
    pub texture: Option<String>,
}

impl Replay {
    pub fn evaluate(&self, tick: f64) -> EvaluatedReplay {
        let effective_tick = if self.looping > 0 && tick > 0.0 {
            tick % (self.looping as f64)
        } else {
            tick
        };
        let x = self.keyframes.x.evaluate(effective_tick).unwrap_or(0.0);
        let y = self.keyframes.y.evaluate(effective_tick).unwrap_or(0.0);
        let z = self.keyframes.z.evaluate(effective_tick).unwrap_or(0.0);
        let yaw = self.keyframes.yaw.evaluate(effective_tick).unwrap_or(0.0) as f32;
        let head_yaw = self
            .keyframes
            .head_yaw
            .evaluate(effective_tick)
            .map_or(yaw, |v| v as f32);
        let body_yaw = self
            .keyframes
            .body_yaw
            .evaluate(effective_tick)
            .map_or(yaw, |v| v as f32);
        let pitch = self.keyframes.pitch.evaluate(effective_tick).unwrap_or(0.0) as f32;
        let sneaking = self
            .keyframes
            .sneaking
            .evaluate(effective_tick)
            .unwrap_or(0.0)
            >= 0.5;
        let sprinting = self
            .keyframes
            .sprinting
            .evaluate(effective_tick)
            .unwrap_or(0.0)
            >= 0.5;
        let visible = self
            .keyframes
            .visible
            .evaluate(effective_tick)
            .unwrap_or(1.0)
            >= 0.5;
        let texture = self.form.as_ref().and_then(|f| f.texture.clone());
        EvaluatedReplay {
            uuid: self.uuid.clone(),
            label: self.label.clone(),
            group: self.group.clone(),
            position: [x, y, z],
            yaw,
            head_yaw,
            body_yaw,
            pitch,
            sneaking,
            sprinting,
            visible,
            texture,
        }
    }

    pub fn record_sample(
        &mut self,
        tick: f64,
        pos: Vec3,
        yaw: f32,
        head_yaw: f32,
        body_yaw: f32,
        pitch: f32,
        sneaking: bool,
    ) {
        self.keyframes
            .x
            .insert(Keyframe::new(tick, pos[0], Interpolation::Linear));
        self.keyframes
            .y
            .insert(Keyframe::new(tick, pos[1], Interpolation::Linear));
        self.keyframes
            .z
            .insert(Keyframe::new(tick, pos[2], Interpolation::Linear));
        self.keyframes
            .yaw
            .insert(Keyframe::new(tick, yaw as f64, Interpolation::Linear));
        self.keyframes.head_yaw.insert(Keyframe::new(
            tick,
            head_yaw as f64,
            Interpolation::Linear,
        ));
        self.keyframes.body_yaw.insert(Keyframe::new(
            tick,
            body_yaw as f64,
            Interpolation::Linear,
        ));
        self.keyframes
            .pitch
            .insert(Keyframe::new(tick, pitch as f64, Interpolation::Linear));
        self.keyframes.sneaking.insert(Keyframe::new(
            tick,
            if sneaking { 1.0 } else { 0.0 },
            Interpolation::Step,
        ));
    }

    pub fn translate_trajectory(&mut self, delta: Vec3, delta_yaw: f32) {
        self.keyframes.x.offset_value(delta[0]);
        self.keyframes.y.offset_value(delta[1]);
        self.keyframes.z.offset_value(delta[2]);
        if delta_yaw.abs() > 1e-6 {
            let dy = delta_yaw as f64;
            self.keyframes.yaw.offset_value(dy);
            self.keyframes.head_yaw.offset_value(dy);
            self.keyframes.body_yaw.offset_value(dy);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayGroup {
    pub id: String,
    pub title: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupClipboard {
    pub group_path: String,
    pub subgroups: Vec<ReplayGroup>,
    pub replays: Vec<Replay>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraClipKind {
    #[default]
    Idle,
    Dolly,
    Circular,
    Path,
    Look,
    Orbit,
    Shake,
    Curve,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraPoint {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fov: f32,
}

impl Default for CameraPoint {
    fn default() -> Self {
        Self {
            position: [0.0, 64.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov: 70.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraClip {
    /// BBS 26.3 clip type (`bbs:idle`, `bbs:dolly`, `bbs:circular`, `bbs:path`, `bbs:orbit`, etc.).
    pub id: String,
    #[serde(default)]
    pub kind: CameraClipKind,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub layer: u32,
    pub tick: u32,
    pub duration: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub start: CameraPoint,
    #[serde(default)]
    pub end: CameraPoint,
    #[serde(default)]
    pub path_points: Vec<CameraPoint>,
    #[serde(default)]
    pub distance: f64,
    #[serde(default)]
    pub circles: f64,
    #[serde(default)]
    pub interp: Interpolation,
    #[serde(default)]
    pub target_replay: Option<usize>,
}

impl Default for CameraClip {
    fn default() -> Self {
        Self {
            id: "bbs:idle".into(),
            kind: CameraClipKind::Idle,
            title: "Shot 1".into(),
            layer: 0,
            tick: 0,
            duration: 100,
            enabled: true,
            start: CameraPoint::default(),
            end: CameraPoint::default(),
            path_points: Vec::new(),
            distance: 5.0,
            circles: 1.0,
            interp: Interpolation::Linear,
            target_replay: None,
        }
    }
}

impl CameraClip {
    pub fn contains_tick(&self, tick: f64) -> bool {
        let start = self.tick as f64;
        let end = (self.tick.saturating_add(self.duration)) as f64;
        self.enabled && self.duration > 0 && tick >= start && tick < end
    }

    pub fn evaluate(&self, tick: f64, replays: &[Replay]) -> CameraPoint {
        let span = (self.duration.max(1)) as f64;
        let local_t = ((tick - self.tick as f64) / span).clamp(0.0, 1.0);
        let u = self.interp.apply(local_t);
        match self.kind {
            CameraClipKind::Idle => self.start.clone(),
            CameraClipKind::Dolly | CameraClipKind::Curve => CameraPoint {
                position: [
                    self.start.position[0] + (self.end.position[0] - self.start.position[0]) * u,
                    self.start.position[1] + (self.end.position[1] - self.start.position[1]) * u,
                    self.start.position[2] + (self.end.position[2] - self.start.position[2]) * u,
                ],
                yaw: self.start.yaw + (self.end.yaw - self.start.yaw) * (u as f32),
                pitch: self.start.pitch + (self.end.pitch - self.start.pitch) * (u as f32),
                roll: self.start.roll + (self.end.roll - self.start.roll) * (u as f32),
                fov: self.start.fov + (self.end.fov - self.start.fov) * (u as f32),
            },
            CameraClipKind::Circular => {
                let angle = u * self.circles * std::f64::consts::TAU
                    + (self.start.yaw as f64).to_radians();
                let r = self.distance.max(0.01);
                CameraPoint {
                    position: [
                        self.start.position[0] + angle.cos() * r,
                        self.start.position[1],
                        self.start.position[2] + angle.sin() * r,
                    ],
                    yaw: (angle.to_degrees() as f32) + 90.0,
                    pitch: self.start.pitch,
                    roll: self.start.roll,
                    fov: self.start.fov,
                }
            }
            CameraClipKind::Path => {
                if self.path_points.is_empty() {
                    return self.start.clone();
                }
                if self.path_points.len() == 1 {
                    return self.path_points[0].clone();
                }
                let max_idx = (self.path_points.len() - 1) as f64;
                let scaled = u * max_idx;
                let idx = (scaled.floor() as usize).min(self.path_points.len() - 2);
                let seg_t = scaled - (idx as f64);
                let a = &self.path_points[idx];
                let b = &self.path_points[idx + 1];
                CameraPoint {
                    position: [
                        a.position[0] + (b.position[0] - a.position[0]) * seg_t,
                        a.position[1] + (b.position[1] - a.position[1]) * seg_t,
                        a.position[2] + (b.position[2] - a.position[2]) * seg_t,
                    ],
                    yaw: a.yaw + (b.yaw - a.yaw) * (seg_t as f32),
                    pitch: a.pitch + (b.pitch - a.pitch) * (seg_t as f32),
                    roll: a.roll + (b.roll - a.roll) * (seg_t as f32),
                    fov: a.fov + (b.fov - a.fov) * (seg_t as f32),
                }
            }
            CameraClipKind::Orbit | CameraClipKind::Look => {
                let target_pos = self
                    .target_replay
                    .and_then(|idx| replays.get(idx))
                    .map(|r| {
                        let ev = r.evaluate(tick);
                        // Focus on the main torso/chest anchor (approx +0.9m above feet for Steve/Alex)
                        [ev.position[0], ev.position[1] + 0.9, ev.position[2]]
                    })
                    .unwrap_or(self.start.position);
                let yaw_rad = (self.start.yaw as f64).to_radians();
                let pitch_rad = (self.start.pitch as f64).to_radians();
                let dist = self.distance.max(0.0);
                let horiz = dist * pitch_rad.cos();
                CameraPoint {
                    position: [
                        target_pos[0] - yaw_rad.sin() * horiz,
                        target_pos[1] + dist * pitch_rad.sin(),
                        target_pos[2] + yaw_rad.cos() * horiz,
                    ],
                    yaw: self.start.yaw,
                    pitch: self.start.pitch,
                    roll: self.start.roll,
                    fov: self.start.fov,
                }
            }
            CameraClipKind::Shake => {
                let phase = tick * 0.85;
                let jitter_yaw = (phase.sin() * 1.5) as f32;
                let jitter_pitch = ((phase * 1.37).cos() * 1.2) as f32;
                CameraPoint {
                    position: self.start.position,
                    yaw: self.start.yaw + jitter_yaw,
                    pitch: self.start.pitch + jitter_pitch,
                    roll: self.start.roll,
                    fov: self.start.fov,
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Film {
    pub id: String,
    #[serde(default = "default_mc_version")]
    pub mc_version: String,
    #[serde(default = "default_data_version")]
    pub data_version: u32,
    #[serde(default)]
    pub groups: Vec<ReplayGroup>,
    #[serde(default)]
    pub replays: Vec<Replay>,
    #[serde(default)]
    pub camera_clips: Vec<CameraClip>,
}

fn default_mc_version() -> String {
    BBS_MC_VERSION.into()
}

fn default_data_version() -> u32 {
    BBS_DATA_VERSION_26_3
}

impl Default for Film {
    fn default() -> Self {
        Self {
            id: "scene_1".into(),
            mc_version: default_mc_version(),
            data_version: default_data_version(),
            groups: Vec::new(),
            replays: vec![Replay::default()],
            camera_clips: vec![CameraClip::default()],
        }
    }
}

impl Film {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.id.len() > 128 {
            return Err("film id must be 1..=128 characters".into());
        }
        if self.replays.len() > MAX_REPLAYS_PER_FILM {
            return Err(format!(
                "film '{}' exceeds max replays ({MAX_REPLAYS_PER_FILM})",
                self.id
            ));
        }
        let mut group_ids = BTreeSet::new();
        for g in &self.groups {
            if g.id.trim().is_empty() {
                return Err("group id cannot be empty".into());
            }
            if !group_ids.insert(&g.id) {
                return Err(format!("duplicate group '{}'", g.id));
            }
        }
        for r in &self.replays {
            for ch in [
                &r.keyframes.x,
                &r.keyframes.y,
                &r.keyframes.z,
                &r.keyframes.yaw,
                &r.keyframes.head_yaw,
                &r.keyframes.body_yaw,
                &r.keyframes.pitch,
            ] {
                if ch.keyframes.len() > MAX_KEYFRAMES_PER_CHANNEL {
                    return Err(format!("replay '{}' channel exceeds max keyframes", r.uuid));
                }
                if ch
                    .keyframes
                    .iter()
                    .any(|k| !k.tick.is_finite() || !k.value.is_finite())
                {
                    return Err(format!("replay '{}' has non-finite keyframe values", r.uuid));
                }
            }
        }
        Ok(())
    }

    /// Upgrade a film imported from Minecraft 1.20.1 (`DataVersion: 3465`) to Minecraft 26.3 (`DataVersion: 5023`).
    pub fn upgrade_to_26_3(&mut self) {
        self.mc_version = BBS_MC_VERSION.into();
        self.data_version = BBS_DATA_VERSION_26_3;
    }

    pub fn from_26_3_json(source: &str) -> Result<Self, String> {
        let mut film: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        film.validate()?;
        film.upgrade_to_26_3();
        Ok(film)
    }

    pub fn to_26_3_json(&self) -> Result<String, String> {
        self.validate()?;
        let mut copy = self.clone();
        copy.upgrade_to_26_3();
        serde_json::to_string_pretty(&copy).map_err(|e| e.to_string())
    }

    pub fn from_1201_json(source: &str) -> Result<Self, String> {
        let film: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        film.validate()?;
        Ok(film)
    }

    pub fn to_1201_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn duration_ticks(&self) -> u32 {
        let max_clip = self
            .camera_clips
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.tick.saturating_add(c.duration))
            .max()
            .unwrap_or(0);
        let max_replay = self
            .replays
            .iter()
            .filter(|r| r.enabled)
            .flat_map(|r| {
                [
                    &r.keyframes.x,
                    &r.keyframes.y,
                    &r.keyframes.z,
                    &r.keyframes.yaw,
                    &r.keyframes.pitch,
                ]
            })
            .filter_map(|ch| ch.keyframes.last().map(|k| k.tick.ceil().max(0.0) as u32))
            .max()
            .unwrap_or(0);
        max_clip.max(max_replay).max(1)
    }

    /// Create a category/group and immediately move all `selected_indices` replays into it.
    pub fn create_group_with_selected(
        &mut self,
        group_path: &str,
        selected_indices: &[usize],
    ) -> Result<usize, String> {
        let clean = group_path.trim().trim_matches('/').to_string();
        if clean.is_empty() {
            return Err("category name cannot be empty".into());
        }
        if !self.groups.iter().any(|g| g.id == clean) {
            let title = clean.rsplit('/').next().unwrap_or(&clean).to_string();
            self.groups.push(ReplayGroup {
                id: clean.clone(),
                title,
                visible: true,
                locked: false,
            });
        }
        let mut moved = 0;
        for &idx in selected_indices {
            if let Some(replay) = self.replays.get_mut(idx) {
                replay.group = clean.clone();
                moved += 1;
            }
        }
        Ok(moved)
    }

    /// Copy a group/category and all its nested subgroups and replays for pasting into any scene.
    pub fn copy_group(&self, group_path: &str) -> Option<GroupClipboard> {
        let clean = group_path.trim().trim_matches('/');
        if clean.is_empty() {
            return None;
        }
        let prefix = format!("{clean}/");
        let subgroups: Vec<ReplayGroup> = self
            .groups
            .iter()
            .filter(|g| g.id == clean || g.id.starts_with(&prefix))
            .cloned()
            .collect();
        let replays: Vec<Replay> = self
            .replays
            .iter()
            .filter(|r| r.group == clean || r.group.starts_with(&prefix))
            .cloned()
            .collect();
        if subgroups.is_empty() && replays.is_empty() {
            return None;
        }
        Some(GroupClipboard {
            group_path: clean.to_string(),
            subgroups,
            replays,
        })
    }

    /// Paste a copied `GroupClipboard` into this film, preserving hierarchy and returning new replay indices.
    pub fn paste_group(&mut self, clipboard: &GroupClipboard) -> Vec<usize> {
        for g in &clipboard.subgroups {
            if !self.groups.iter().any(|existing| existing.id == g.id) {
                self.groups.push(g.clone());
            }
        }
        let mut inserted = Vec::new();
        for r in &clipboard.replays {
            if self.replays.len() >= MAX_REPLAYS_PER_FILM {
                break;
            }
            let mut copy = r.clone();
            copy.uuid = format!("{}-copy-{}", r.uuid, self.replays.len() + 1);
            let idx = self.replays.len();
            self.replays.push(copy);
            inserted.push(idx);
        }
        inserted
    }

    /// Compute the combined estimated center point of multiple selected replays at `tick` for the Transform Gizmo.
    pub fn combined_replays_center(&self, selected_indices: &[usize], tick: f64) -> Option<Vec3> {
        let mut sum = [0.0, 0.0, 0.0];
        let mut count = 0usize;
        for &idx in selected_indices {
            if let Some(r) = self.replays.get(idx) {
                let ev = r.evaluate(tick);
                sum[0] += ev.position[0];
                sum[1] += ev.position[1];
                sum[2] += ev.position[2];
                count += 1;
            }
        }
        if count == 0 {
            return None;
        }
        let inv = 1.0 / (count as f64);
        Some([sum[0] * inv, sum[1] * inv, sum[2] * inv])
    }

    /// Smoothly translate and rotate all selected replays around their combined center across their entire trajectories.
    pub fn transform_selected_replays(
        &mut self,
        selected_indices: &[usize],
        tick: f64,
        translation: Vec3,
        yaw_degrees: f32,
    ) -> usize {
        let Some(center) = self.combined_replays_center(selected_indices, tick) else {
            return 0;
        };
        let rad = (yaw_degrees as f64).to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();
        let mut updated = 0usize;
        for &idx in selected_indices {
            let Some(replay) = self.replays.get_mut(idx) else {
                continue;
            };
            if yaw_degrees.abs() > 1e-6 {
                let ev = replay.evaluate(tick);
                let dx = ev.position[0] - center[0];
                let dz = ev.position[2] - center[2];
                let rot_dx = dx * cos_a - dz * sin_a - dx;
                let rot_dz = dx * sin_a + dz * cos_a - dz;
                replay.translate_trajectory(
                    [
                        translation[0] + rot_dx,
                        translation[1],
                        translation[2] + rot_dz,
                    ],
                    yaw_degrees,
                );
            } else {
                replay.translate_trajectory(translation, 0.0);
            }
            updated += 1;
        }
        updated
    }

    /// Apply textures deterministically from a chosen texture pool across selected replays.
    pub fn apply_random_textures(
        &mut self,
        selected_indices: &[usize],
        textures: &[String],
        seed: u64,
    ) -> usize {
        if textures.is_empty() {
            return 0;
        }
        let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut applied = 0usize;
        for &idx in selected_indices {
            let Some(replay) = self.replays.get_mut(idx) else {
                continue;
            };
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let pick = ((state.wrapping_mul(0x2545_F491_4F6C_DD1D)) as usize) % textures.len();
            let form = replay.form.get_or_insert_with(Form::default);
            form.texture = Some(textures[pick].clone());
            applied += 1;
        }
        applied
    }

    pub fn evaluate_camera(&self, tick: f64) -> Option<CameraPoint> {
        let active = self
            .camera_clips
            .iter()
            .filter(|c| c.contains_tick(tick))
            .max_by_key(|c| (c.layer, c.tick))?;
        Some(active.evaluate(tick, &self.replays))
    }

    pub fn evaluate_replays(&self, tick: f64) -> Vec<EvaluatedReplay> {
        self.replays
            .iter()
            .filter(|r| r.enabled)
            .map(|r| r.evaluate(tick))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub hud_overlay: bool,
    #[serde(default)]
    pub hud_corner: u8,
    #[serde(default = "default_true")]
    pub lock_camera_on_play: bool,
    #[serde(default)]
    pub loop_playback: bool,
    #[serde(default = "default_tick_rate")]
    pub ticks_per_second: u32,
}

fn default_tick_rate() -> u32 {
    20
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            hud_overlay: true,
            hud_corner: 1, // Top-Right default so it doesn't overlap Top-Left FPS Overlay
            lock_camera_on_play: true,
            loop_playback: false,
            ticks_per_second: default_tick_rate(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default = "default_films")]
    pub films: BTreeMap<String, Film>,
}

fn default_films() -> BTreeMap<String, Film> {
    let film = Film::default();
    BTreeMap::from([(film.id.clone(), film)])
}

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            films: default_films(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.settings.ticks_per_second == 0 || self.settings.ticks_per_second > 240 {
            return Err("ticks_per_second must be between 1 and 240".into());
        }
        for (key, film) in &self.films {
            if key != &film.id {
                return Err(format!(
                    "film map key '{key}' does not match film.id '{}'",
                    film.id
                ));
            }
            film.validate()?;
        }
        Ok(())
    }

    pub fn from_json(source: &str) -> Result<Self, String> {
        let cfg: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        cfg.validate()?;
        Ok(cfg)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSnapshot {
    pub actor: ActorId,
    pub name: String,
    pub world_tick: u64,
    pub position: Vec3,
    pub yaw: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
    pub pitch: f32,
    pub sneaking: bool,
}

pub trait Host {
    type Error;
    fn player(&mut self) -> PlayerSnapshot;
    fn apply_camera(&mut self, point: &CameraPoint) -> Result<(), Self::Error>;
    fn apply_replays(&mut self, replays: &[EvaluatedReplay]) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackMode {
    #[default]
    Stopped,
    Playing,
    Paused,
    Recording {
        replay_index: usize,
    },
}

#[derive(Clone, Debug)]
pub struct SessionState {
    pub enabled: bool,
    pub active_film: String,
    pub mode: PlaybackMode,
    pub cursor_tick: f64,
    pub last_world_tick: Option<u64>,
    pub selected_replays: Vec<usize>,
    pub clipboard: Option<GroupClipboard>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            enabled: true,
            active_film: "scene_1".into(),
            mode: PlaybackMode::Stopped,
            cursor_tick: 0.0,
            last_world_tick: None,
            selected_replays: vec![0],
            clipboard: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepReport {
    pub film_id: String,
    pub mode: PlaybackMode,
    pub tick: f64,
    pub evaluated_replays: usize,
    pub camera_applied: bool,
    pub recorded_sample: bool,
}

#[derive(Default)]
pub struct Runtime {
    sessions: BTreeMap<ActorId, SessionState>,
}

impl Runtime {
    pub fn session(&self, actor: ActorId) -> SessionState {
        self.sessions.get(&actor).cloned().unwrap_or_default()
    }

    pub fn session_mut(&mut self, actor: ActorId) -> &mut SessionState {
        self.sessions.entry(actor).or_default()
    }

    pub fn enabled(&self, actor: ActorId) -> bool {
        self.sessions.get(&actor).is_none_or(|s| s.enabled)
    }

    pub fn set_enabled(&mut self, actor: ActorId, on: bool) {
        self.session_mut(actor).enabled = on;
    }

    pub fn play(&mut self, actor: ActorId, film_id: Option<&str>) {
        let s = self.session_mut(actor);
        if let Some(id) = film_id {
            s.active_film = id.to_string();
        }
        s.mode = PlaybackMode::Playing;
        s.last_world_tick = None;
    }

    pub fn pause(&mut self, actor: ActorId) {
        let s = self.session_mut(actor);
        s.mode = PlaybackMode::Paused;
        s.last_world_tick = None;
    }

    pub fn stop(&mut self, actor: ActorId) {
        let s = self.session_mut(actor);
        s.mode = PlaybackMode::Stopped;
        s.cursor_tick = 0.0;
        s.last_world_tick = None;
    }

    pub fn seek(&mut self, actor: ActorId, tick: f64) {
        let s = self.session_mut(actor);
        s.cursor_tick = tick.max(0.0);
        s.last_world_tick = None;
    }

    pub fn start_recording(&mut self, actor: ActorId, replay_index: usize) {
        let s = self.session_mut(actor);
        s.mode = PlaybackMode::Recording { replay_index };
        s.cursor_tick = 0.0;
        s.last_world_tick = None;
    }

    pub fn step<H: Host>(
        &mut self,
        host: &mut H,
        config: &mut Config,
    ) -> Result<StepReport, H::Error> {
        let player = host.player();
        let s = self.sessions.entry(player.actor).or_default();
        if !config.settings.enabled || !s.enabled {
            return Ok(StepReport {
                film_id: s.active_film.clone(),
                mode: s.mode,
                tick: s.cursor_tick,
                evaluated_replays: 0,
                camera_applied: false,
                recorded_sample: false,
            });
        }
        let Some(film) = config.films.get_mut(&s.active_film) else {
            return Ok(StepReport {
                film_id: s.active_film.clone(),
                mode: s.mode,
                tick: s.cursor_tick,
                evaluated_replays: 0,
                camera_applied: false,
                recorded_sample: false,
            });
        };

        let delta = match s.last_world_tick {
            Some(prev) if player.world_tick >= prev => (player.world_tick - prev) as f64,
            _ => 0.0,
        };
        s.last_world_tick = Some(player.world_tick);

        if matches!(s.mode, PlaybackMode::Playing | PlaybackMode::Recording { .. }) {
            let speed_scale = (config.settings.ticks_per_second as f64) / 20.0;
            s.cursor_tick += delta * speed_scale;
        }

        let duration = film.duration_ticks() as f64;
        if s.mode == PlaybackMode::Playing && s.cursor_tick >= duration {
            if config.settings.loop_playback {
                s.cursor_tick %= duration.max(1.0);
            } else {
                s.cursor_tick = duration;
                s.mode = PlaybackMode::Paused;
            }
        }

        let mut recorded_sample = false;
        if let PlaybackMode::Recording { replay_index } = s.mode
            && let Some(replay) = film.replays.get_mut(replay_index)
        {
            replay.record_sample(
                s.cursor_tick,
                player.position,
                player.yaw,
                player.head_yaw,
                player.body_yaw,
                player.pitch,
                player.sneaking,
            );
            recorded_sample = true;
        }

        let evaluated = film.evaluate_replays(s.cursor_tick);
        let evaluated_count = evaluated.len();
        host.apply_replays(&evaluated)?;

        let mut camera_applied = false;
        if config.settings.lock_camera_on_play
            && matches!(s.mode, PlaybackMode::Playing | PlaybackMode::Paused)
            && let Some(cam) = film.evaluate_camera(s.cursor_tick)
        {
            host.apply_camera(&cam)?;
            camera_applied = true;
        }

        Ok(StepReport {
            film_id: s.active_film.clone(),
            mode: s.mode,
            tick: s.cursor_tick,
            evaluated_replays: evaluated_count,
            camera_applied,
            recorded_sample,
        })
    }

    pub fn hud_lines(&self, actor: ActorId, config: &Config) -> Vec<(String, u32)> {
        let s = self.session(actor);
        let mode_badge = match s.mode {
            PlaybackMode::Playing => ("PLAY", 0x55FF55),
            PlaybackMode::Paused => ("PAUSE", 0xFFFF55),
            PlaybackMode::Recording { .. } => ("REC", 0xFF5555),
            PlaybackMode::Stopped => ("IDLE", 0xAAAAAA),
        };
        let mut lines = Vec::with_capacity(4);
        let timecode = Self::format_timecode(s.cursor_tick, config.settings.ticks_per_second);
        lines.push((
            format!("BBS 26.3 [{}] {}", mode_badge.0, s.active_film),
            mode_badge.1,
        ));
        lines.push((format!("Time: {timecode}"), 0xFFFFFF));
        if let Some(film) = config.films.get(&s.active_film) {
            let evaluated = film.evaluate_replays(s.cursor_tick);
            if let Some(primary) = evaluated.first() {
                lines.push((
                    format!(
                        "Actor {}: {:.1}, {:.1}, {:.1} (yaw {:.0})",
                        primary.label,
                        primary.position[0],
                        primary.position[1],
                        primary.position[2],
                        primary.yaw
                    ),
                    0x88DDFF,
                ));
            }
            if let Some(cam) = film.evaluate_camera(s.cursor_tick) {
                lines.push((
                    format!(
                        "Cam: {:.1}, {:.1}, {:.1} FOV {:.0}",
                        cam.position[0], cam.position[1], cam.position[2], cam.fov
                    ),
                    0xFFCC66,
                ));
            }
        }
        lines
    }

    pub fn format_timecode(tick: f64, tps: u32) -> String {
        let tps = tps.max(1) as f64;
        let total_seconds = (tick / tps).max(0.0);
        let minutes = (total_seconds / 60.0).floor() as u32;
        let seconds = total_seconds % 60.0;
        format!("{minutes:02}:{seconds:05.2} (tick {tick:.1})")
    }
}

}
use engine::{
    CameraClipKind, CameraPoint, Config, EvaluatedReplay, Film, Form, Host, PlaybackMode,
    PlayerSnapshot, Replay, Runtime,
};

const ID: &str = "bbs";

const MENU_ITEMS: [&str; 12] = [
    "[1] Play / Pause Film Timeline",
    "[2] Record Active Replay Trajectory",
    "[3] Stop & Reset Timeline (00:00.00)",
    "[4] Scrub Timeline (-10t Left / +10t Right)",
    "[5] Add Replay Actor (Steve / Alex / Mob)",
    "[6] Select Replay Actor (Left / Right / Enter)",
    "[7] Create Category Group & Move Selected",
    "[8] Copy / Paste Category Group Across Scenes",
    "[9] Transform Selected Replays (Combined Center)",
    "[0] Cycle Camera Clip (Idle/Dolly/Orbit Torso/Path/Circular/Shake)",
    "[-] Apply Random Textures from Explorer Pool",
    "[=] Switch Scene / Create New 26.3 Scene",
];

#[derive(Default)]
pub(super) struct State {
    runtime: Runtime,
    config: Option<Config>,
    tick_counter: u64,
    menu_open: bool,
    menu_cursor: usize,
    category_seq: u32,
    texture_seed: u64,
}

thread_local! {
    static STATE: std::cell::RefCell<State> = std::cell::RefCell::new(State::default());
}

fn with_state<T>(game: &mut Game<'_>, run: impl FnOnce(&mut Game<'_>, &mut State) -> T) -> T {
    struct Lease {
        state: State,
    }
    impl Drop for Lease {
        fn drop(&mut self) {
            STATE.with(|cell| {
                *cell.borrow_mut() = std::mem::take(&mut self.state);
            });
        }
    }
    let state = STATE.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
    let mut lease = Lease { state };
    run(game, &mut lease.state)
}

fn config_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HORIZON_BBS_CONFIG").map(Into::into)
}

fn load_config(game: &Game<'_>) -> std::result::Result<Config, String> {
    let mut cfg = match config_path() {
        Some(path) => Config::from_json(
            &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        )?,
        None => Config::default(),
    };
    if let Some(v) = game.native_setting(ID, "hud_overlay") {
        cfg.settings.hud_overlay = v != "0" && v != "false";
    }
    if let Some(v) = game.native_setting(ID, "hud_corner")
        && let Ok(c) = v.parse::<u8>()
    {
        cfg.settings.hud_corner = c % 4;
    }
    if let Some(v) = game.native_setting(ID, "loop_playback") {
        cfg.settings.loop_playback = v == "1" || v == "true";
    }
    if let Some(v) = game.native_setting(ID, "lock_camera") {
        cfg.settings.lock_camera_on_play = v != "0" && v != "false";
    }
    if let Some(v) = game.native_setting(ID, "tps")
        && let Ok(tps) = v.parse::<u32>()
        && (1..=240).contains(&tps)
    {
        cfg.settings.ticks_per_second = tps;
    }
    Ok(cfg)
}

fn ensure_config(game: &Game<'_>, state: &mut State) {
    if state.config.is_none() {
        state.config = Some(load_config(game).unwrap_or_else(|error| {
            eprintln!("BBS 26.3 config error (using defaults): {error}");
            Config::default()
        }));
    }
}

fn corner_from_u8(c: u8) -> Corner {
    match c % 4 {
        0 => Corner::TopLeft,
        1 => Corner::TopRight,
        2 => Corner::BottomLeft,
        _ => Corner::BottomRight,
    }
}

fn corner_name(c: u8) -> &'static str {
    match c % 4 {
        0 => "Top left",
        1 => "Top right",
        2 => "Bottom left",
        _ => "Bottom right",
    }
}

const LOCAL_ACTOR: [u32; 4] = [0, 0, 0, 1];

struct Native<'g, 'r> {
    game: &'g mut Game<'r>,
    world_tick: u64,
}

impl Host for Native<'_, '_> {
    type Error = String;

    fn player(&mut self) -> PlayerSnapshot {
        let p = self.game.native_player();
        let (pos, yaw, pitch) = match p {
            Some(np) => ([np.x, np.y, np.z], np.yaw, np.pitch),
            None => ([0.0, 64.0, 0.0], 0.0, 0.0),
        };
        let sneaking = self
            .game
            .native_mapping("key.sneak")
            .is_some_and(|(down, _)| down);
        PlayerSnapshot {
            actor: LOCAL_ACTOR,
            name: "Director".into(),
            world_tick: self.world_tick,
            position: pos,
            yaw,
            head_yaw: yaw,
            body_yaw: yaw,
            pitch,
            sneaking,
        }
    }

    fn apply_camera(&mut self, _point: &CameraPoint) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    fn apply_replays(
        &mut self,
        _replays: &[EvaluatedReplay],
    ) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) fn list(mods: &mut Vec<NativeMod>) {
    let keys = [
        ["menu", "Open BBS Studio Menu", "key.keyboard.right.shift"],
        ["play_pause", "Play / Pause Film", "key.keyboard.p"],
        ["record", "Record Active Replay", "key.keyboard.r"],
        ["scrub_zero", "Reset Timeline to 0", "key.keyboard.o"],
        ["hud", "Toggle Director HUD", "key.keyboard.u"],
    ];
    mods.push(NativeMod {
        id: ID.into(),
        title: "BBS Studio 26.3".into(),
        description: "Blockbuster Studio 26.3 — cinematic director, Right-Shift Studio Dashboard, keyframe replay, multi-replay transform & camera engine.".into(),
        keys: keys.map(|k| k.map(String::from)).into(),
    });
}

pub(super) fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ID {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        let cfg = state.config.as_ref().unwrap();
        let s = state.runtime.session(LOCAL_ACTOR);
        let on_off = |b: bool| if b { "ON" } else { "OFF" };
        let mode_str = match s.mode {
            PlaybackMode::Playing => "Playing",
            PlaybackMode::Paused => "Paused",
            PlaybackMode::Recording { replay_index } => {
                if replay_index == 0 {
                    "Recording (#0)"
                } else {
                    "Recording"
                }
            }
            PlaybackMode::Stopped => "Stopped",
        };
        buttons.extend([
            format!("Studio Menu (Right Shift): {}", on_off(state.menu_open)),
            format!("Director HUD: {}", on_off(cfg.settings.hud_overlay)),
            format!("HUD Corner: {}", corner_name(cfg.settings.hud_corner)),
            format!("Playback State: {mode_str}"),
            format!("Loop Playback: {}", on_off(cfg.settings.loop_playback)),
            format!(
                "Camera Lock on Play: {}",
                on_off(cfg.settings.lock_camera_on_play)
            ),
            format!("Timeline Speed: {} TPS", cfg.settings.ticks_per_second),
        ]);
    });
}

pub(super) fn click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        let cfg = state.config.as_mut().unwrap();
        match button {
            0 => {
                state.menu_open = !state.menu_open;
            }
            1 => {
                cfg.settings.hud_overlay = !cfg.settings.hud_overlay;
                game.native_set_setting(
                    ID,
                    "hud_overlay",
                    if cfg.settings.hud_overlay { "1" } else { "0" },
                );
            }
            2 => {
                cfg.settings.hud_corner = (cfg.settings.hud_corner + 1) % 4;
                game.native_set_setting(ID, "hud_corner", &cfg.settings.hud_corner.to_string());
            }
            3 => {
                let s = state.runtime.session(LOCAL_ACTOR);
                match s.mode {
                    PlaybackMode::Playing => state.runtime.pause(LOCAL_ACTOR),
                    _ => state.runtime.play(LOCAL_ACTOR, None),
                }
            }
            4 => {
                cfg.settings.loop_playback = !cfg.settings.loop_playback;
                game.native_set_setting(
                    ID,
                    "loop_playback",
                    if cfg.settings.loop_playback { "1" } else { "0" },
                );
            }
            5 => {
                cfg.settings.lock_camera_on_play = !cfg.settings.lock_camera_on_play;
                game.native_set_setting(
                    ID,
                    "lock_camera",
                    if cfg.settings.lock_camera_on_play {
                        "1"
                    } else {
                        "0"
                    },
                );
            }
            6 => {
                cfg.settings.ticks_per_second = match cfg.settings.ticks_per_second {
                    10 => 20,
                    20 => 40,
                    40 => 60,
                    _ => 10,
                };
                game.native_set_setting(ID, "tps", &cfg.settings.ticks_per_second.to_string());
            }
            _ => {}
        }
    });
}

pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_window() || !game.native_on(ID) {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        state.tick_counter = state.tick_counter.wrapping_add(1);
        let world_tick = state.tick_counter;
        let cfg = state.config.as_mut().unwrap();
        let _ = state.runtime.step(
            &mut Native {
                game,
                world_tick,
            },
            cfg,
        );
    });
}

fn execute_menu_action(game: &mut Game<'_>, state: &mut State, action_idx: usize, step_dir: i32) {
    let actor = LOCAL_ACTOR;
    let world_tick = state.tick_counter;
    match action_idx {
        0 => {
            let s = state.runtime.session(actor);
            if s.mode == PlaybackMode::Playing {
                state.runtime.pause(actor);
                game.native_chat("§b[BBS Studio 26.3]§r Timeline paused.");
            } else {
                state.runtime.play(actor, None);
                game.native_chat(&format!(
                    "§a[BBS Studio 26.3]§r Playing '{}' from tick {:.1}.",
                    s.active_film, s.cursor_tick
                ));
            }
        }
        1 => {
            let s = state.runtime.session(actor);
            if matches!(s.mode, PlaybackMode::Recording { .. }) {
                state.runtime.pause(actor);
                game.native_chat("§e[BBS Studio 26.3]§r Recording finished & saved to replay.");
            } else {
                let idx = s.selected_replays.first().copied().unwrap_or(0);
                state.runtime.start_recording(actor, idx);
                game.native_chat(&format!(
                    "§c[BBS Studio 26.3]§r Recording replay #{idx} in '{}'...",
                    s.active_film
                ));
            }
        }
        2 => {
            state.runtime.stop(actor);
            game.native_chat("§7[BBS Studio 26.3]§r Timeline reset to 00:00.00 (tick 0.0).");
        }
        3 => {
            let delta = if step_dir < 0 { -10.0 } else { 10.0 };
            let next_tick = (state.runtime.session(actor).cursor_tick + delta).max(0.0);
            state.runtime.seek(actor, next_tick);
            let _ = state.runtime.step(
                &mut Native { game, world_tick },
                state.config.as_mut().unwrap(),
            );
            game.native_chat(&format!(
                "§b[BBS Studio 26.3]§r Scrubbed timeline to tick {next_tick:.1}."
            ));
        }
        4 => {
            let s = state.runtime.session(actor);
            if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                let idx = film.replays.len();
                let preset = match idx % 3 {
                    0 => ("bbs:model", "player/steve", "minecraft:skins/steve"),
                    1 => ("bbs:model", "player/alex", "minecraft:skins/alex"),
                    _ => ("bbs:mob", "minecraft:villager", "minecraft:entity/villager/villager"),
                };
                let mut r = Replay::default();
                r.uuid = format!("actor-{}", idx + 1);
                r.label = format!("Actor {} ({})", idx + 1, preset.1);
                r.form = Some(Form {
                    id: preset.0.into(),
                    model: preset.1.into(),
                    texture: Some(preset.2.into()),
                    ..Form::default()
                });
                if let Some(np) = game.native_player() {
                    r.record_sample(s.cursor_tick, [np.x, np.y, np.z], np.yaw, np.yaw, np.yaw, np.pitch, false);
                }
                film.replays.push(r);
                state.runtime.session_mut(actor).selected_replays = vec![idx];
                game.native_chat(&format!(
                    "§a[BBS Studio 26.3]§r Added Replay #{idx} ({}) at current position.",
                    preset.1
                ));
            }
        }
        5 => {
            let s = state.runtime.session(actor);
            let count = state
                .config
                .as_ref()
                .unwrap()
                .films
                .get(&s.active_film)
                .map_or(0, |f| f.replays.len());
            if count > 0 {
                if step_dir == 0 && count > 1 {
                    let all: Vec<usize> = (0..count).collect();
                    state.runtime.session_mut(actor).selected_replays = all;
                    game.native_chat(&format!(
                        "§b[BBS Studio 26.3]§r Selected ALL {count} replays for multi-transform/group."
                    ));
                } else {
                    let cur = s.selected_replays.first().copied().unwrap_or(0);
                    let next = if step_dir < 0 {
                        (cur + count - 1) % count
                    } else {
                        (cur + 1) % count
                    };
                    state.runtime.session_mut(actor).selected_replays = vec![next];
                    game.native_chat(&format!(
                        "§b[BBS Studio 26.3]§r Selected replay #{next} (press Enter on row 6 to select all)."
                    ));
                }
            }
        }
        6 => {
            state.category_seq = state.category_seq.wrapping_add(1);
            let group_name = format!("squad_{}", state.category_seq);
            let s = state.runtime.session(actor);
            if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                if let Ok(moved) = film.create_group_with_selected(&group_name, &s.selected_replays)
                {
                    game.native_chat(&format!(
                        "§a[BBS Studio 26.3]§r Created category '{group_name}' & moved {moved} selected replay(s) instantly!"
                    ));
                }
            }
        }
        7 => {
            let s = state.runtime.session(actor);
            if let Some(clip) = s.clipboard.clone() {
                if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                    let inserted = film.paste_group(&clip);
                    state.runtime.session_mut(actor).selected_replays = inserted.clone();
                    game.native_chat(&format!(
                        "§a[BBS Studio 26.3]§r Pasted group '{}' ({} replays) into '{}'.",
                        clip.group_path,
                        inserted.len(),
                        film.id
                    ));
                }
            } else if let Some(film) = state.config.as_ref().unwrap().films.get(&s.active_film) {
                let group_path = film
                    .groups
                    .last()
                    .map(|g| g.id.clone())
                    .unwrap_or_else(|| "squad_1".into());
                if let Some(clip) = film.copy_group(&group_path) {
                    let count = clip.replays.len();
                    state.runtime.session_mut(actor).clipboard = Some(clip);
                    game.native_chat(&format!(
                        "§a[BBS Studio 26.3]§r Copied group '{group_path}' ({count} replays) to clipboard (press again to paste)."
                    ));
                } else {
                    game.native_chat("§e[BBS Studio 26.3]§r Create a category first ([7]) before copying.");
                }
            }
        }
        8 => {
            let s = state.runtime.session(actor);
            let dx = if step_dir < 0 { -1.5 } else { 1.5 };
            let dyaw = if step_dir == 0 { 15.0 } else { 0.0 };
            if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                let center = film
                    .combined_replays_center(&s.selected_replays, s.cursor_tick)
                    .unwrap_or([0.0, 64.0, 0.0]);
                let moved = film.transform_selected_replays(
                    &s.selected_replays,
                    s.cursor_tick,
                    [dx, 0.0, 0.0],
                    dyaw,
                );
                game.native_chat(&format!(
                    "§a[BBS Studio 26.3]§r Smoothly transformed {moved} replay(s) around combined center ({:.1}, {:.1}, {:.1}).",
                    center[0], center[1], center[2]
                ));
            }
        }
        9 => {
            let s = state.runtime.session(actor);
            if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                if film.camera_clips.is_empty() {
                    film.camera_clips.push(engine::CameraClip::default());
                }
                let clip = &mut film.camera_clips[0];
                clip.kind = match clip.kind {
                    CameraClipKind::Idle => CameraClipKind::Dolly,
                    CameraClipKind::Dolly => CameraClipKind::Orbit,
                    CameraClipKind::Orbit => CameraClipKind::Path,
                    CameraClipKind::Path => CameraClipKind::Circular,
                    CameraClipKind::Circular => CameraClipKind::Shake,
                    _ => CameraClipKind::Idle,
                };
                clip.target_replay = Some(s.selected_replays.first().copied().unwrap_or(0));
                if let Some(np) = game.native_player() {
                    clip.start.position = [np.x, np.y, np.z];
                    clip.start.yaw = np.yaw;
                    clip.start.pitch = np.pitch;
                    clip.end.position = [np.x + 6.0, np.y + 1.5, np.z + 6.0];
                    clip.end.yaw = np.yaw + 25.0;
                }
                game.native_chat(&format!(
                    "§b[BBS Studio 26.3]§r Camera Clip mode -> {:?} (Orbit focuses main torso anchor +0.9m).",
                    clip.kind
                ));
            }
        }
        10 => {
            state.texture_seed = state.texture_seed.wrapping_add(1);
            let pool = vec![
                "assets:Skins/Skin Packs/Bank/Banker1.png".to_string(),
                "assets:Skins/Skin Packs/Bank/Banker2.png".to_string(),
                "assets:Skins/Skin Packs/Police/Officer1.png".to_string(),
                "assets:Skins/Skin Packs/Beach/Swimmer1.png".to_string(),
                "minecraft:skins/steve".to_string(),
                "minecraft:skins/alex".to_string(),
            ];
            let s = state.runtime.session(actor);
            if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                let count =
                    film.apply_random_textures(&s.selected_replays, &pool, state.texture_seed);
                game.native_chat(&format!(
                    "§a[BBS Studio 26.3]§r Applied random textures from Visual Explorer pool to {count} selected replay(s)."
                ));
            }
        }
        11 => {
            let cfg = state.config.as_mut().unwrap();
            let next_id = if cfg.films.len() == 1 {
                let id = "scene_2".to_string();
                let mut f = Film::default();
                f.id = id.clone();
                cfg.films.insert(id.clone(), f);
                id
            } else {
                let cur = state.runtime.session(actor).active_film;
                let keys: Vec<String> = cfg.films.keys().cloned().collect();
                let pos = keys.iter().position(|k| k == &cur).unwrap_or(0);
                keys[(pos + 1) % keys.len()].clone()
            };
            state.runtime.session_mut(actor).active_film = next_id.clone();
            game.native_chat(&format!(
                "§b[BBS Studio 26.3]§r Switched active 26.3 film scene to '{next_id}'."
            ));
        }
        _ => {}
    }
}

pub(super) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if *claimed || !game.native_playing() || !game.native_on(ID) {
        return;
    }
    with_state(game, |_game, state| {
        if !state.menu_open || steps.abs() < 0.01 {
            return;
        }
        *claimed = true;
        let len = MENU_ITEMS.len();
        if steps < 0.0 {
            state.menu_cursor = (state.menu_cursor + 1) % len;
        } else {
            state.menu_cursor = (state.menu_cursor + len - 1) % len;
        }
    });
}

pub(super) fn key(game: &mut Game<'_>, key_name: &str, pressed: bool) {
    if !pressed || !game.native_playing() || !game.native_on(ID) {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        let menu_key = game.native_mod_key(ID, "menu", "key.keyboard.right.shift");
        let play_key = game.native_mod_key(ID, "play_pause", "key.keyboard.p");
        let rec_key = game.native_mod_key(ID, "record", "key.keyboard.r");
        let zero_key = game.native_mod_key(ID, "scrub_zero", "key.keyboard.o");
        let hud_key = game.native_mod_key(ID, "hud", "key.keyboard.u");

        if key_name == menu_key || key_name == "key.keyboard.right.shift" {
            state.menu_open = !state.menu_open;
            return;
        }

        if state.menu_open {
            let len = MENU_ITEMS.len();
            match key_name {
                "key.keyboard.up" => {
                    state.menu_cursor = (state.menu_cursor + len - 1) % len;
                    return;
                }
                "key.keyboard.down" => {
                    state.menu_cursor = (state.menu_cursor + 1) % len;
                    return;
                }
                "key.keyboard.left" => {
                    let idx = state.menu_cursor;
                    execute_menu_action(game, state, idx, -1);
                    return;
                }
                "key.keyboard.right" => {
                    let idx = state.menu_cursor;
                    execute_menu_action(game, state, idx, 1);
                    return;
                }
                "key.keyboard.enter" | "key.keyboard.kp.enter" => {
                    let idx = state.menu_cursor;
                    execute_menu_action(game, state, idx, 0);
                    return;
                }
                "key.keyboard.1" => {
                    state.menu_cursor = 0;
                    execute_menu_action(game, state, 0, 0);
                    return;
                }
                "key.keyboard.2" => {
                    state.menu_cursor = 1;
                    execute_menu_action(game, state, 1, 0);
                    return;
                }
                "key.keyboard.3" => {
                    state.menu_cursor = 2;
                    execute_menu_action(game, state, 2, 0);
                    return;
                }
                "key.keyboard.4" => {
                    state.menu_cursor = 3;
                    execute_menu_action(game, state, 3, 1);
                    return;
                }
                "key.keyboard.5" => {
                    state.menu_cursor = 4;
                    execute_menu_action(game, state, 4, 0);
                    return;
                }
                "key.keyboard.6" => {
                    state.menu_cursor = 5;
                    execute_menu_action(game, state, 5, 1);
                    return;
                }
                "key.keyboard.7" => {
                    state.menu_cursor = 6;
                    execute_menu_action(game, state, 6, 0);
                    return;
                }
                "key.keyboard.8" => {
                    state.menu_cursor = 7;
                    execute_menu_action(game, state, 7, 0);
                    return;
                }
                "key.keyboard.9" => {
                    state.menu_cursor = 8;
                    execute_menu_action(game, state, 8, 1);
                    return;
                }
                "key.keyboard.0" => {
                    state.menu_cursor = 9;
                    execute_menu_action(game, state, 9, 0);
                    return;
                }
                "key.keyboard.minus" => {
                    state.menu_cursor = 10;
                    execute_menu_action(game, state, 10, 0);
                    return;
                }
                "key.keyboard.equal" => {
                    state.menu_cursor = 11;
                    execute_menu_action(game, state, 11, 0);
                    return;
                }
                _ => {}
            }
        }

        if key_name == play_key {
            execute_menu_action(game, state, 0, 0);
        } else if key_name == rec_key {
            execute_menu_action(game, state, 1, 0);
        } else if key_name == zero_key {
            execute_menu_action(game, state, 2, 0);
        } else if key_name == hud_key {
            let cfg = state.config.as_mut().unwrap();
            cfg.settings.hud_overlay = !cfg.settings.hud_overlay;
            game.native_set_setting(
                ID,
                "hud_overlay",
                if cfg.settings.hud_overlay { "1" } else { "0" },
            );
            game.native_chat(&format!(
                "§b[BBS Studio 26.3]§r Director HUD: {}",
                if cfg.settings.hud_overlay {
                    "ON"
                } else {
                    "OFF"
                }
            ));
        }
    });
}

fn draw_studio_menu(game: &mut Game<'_>, state: &State, batch: &mut HudBatch) {
    let cfg = state.config.as_ref().unwrap();
    let s = state.runtime.session(LOCAL_ACTOR);
    let film = cfg.films.get(&s.active_film);
    let duration = film.map_or(100, |f| f.duration_ticks()).max(1);
    let replay_count = film.map_or(0, |f| f.replays.len());
    let group_count = film.map_or(0, |f| f.groups.len());
    let cam_mode = film
        .and_then(|f| f.camera_clips.first())
        .map(|c| format!("{:?}", c.kind))
        .unwrap_or_else(|| "Idle".into());
    let timecode = Runtime::format_timecode(s.cursor_tick, cfg.settings.ticks_per_second);
    let mode_badge = match s.mode {
        PlaybackMode::Playing => ("PLAYING", 0x55FF55),
        PlaybackMode::Paused => ("PAUSED", 0xFFFF55),
        PlaybackMode::Recording { .. } => ("RECORDING", 0xFF5555),
        PlaybackMode::Stopped => ("READY", 0x88DDFF),
    };

    let x = 24.0_f32;
    let y = 20.0_f32;
    let w = 360.0_f32;
    let h = 232.0_f32;

    // Outer dark glass frame + crimson BBS Studio accent header
    game.native_fill(batch, [x - 2.0, y - 2.0, w + 4.0, h + 4.0], 0xD0E1_1D48);
    game.native_fill(batch, [x, y, w, h], 0xE610_1218);
    game.native_fill(batch, [x, y, w, 24.0], 0xF09F_1239);

    game.native_text(
        batch,
        "BBS STUDIO 26.3 — DIRECTOR SUITE",
        [x + 8.0, y + 4.0],
        0xFFFFFF,
        true,
    );
    game.native_text(
        batch,
        &format!("[{}] {}", mode_badge.0, timecode),
        [x + 8.0, y + 14.0],
        mode_badge.1,
        true,
    );

    // Status bar & timeline progress scrubber
    let info_line = format!(
        "Scene: {} (MC 26.3 DV {}) | Replays: {} (Sel {:?}) | Groups: {} | Cam: {}",
        s.active_film,
        engine::BBS_DATA_VERSION_26_3,
        replay_count,
        s.selected_replays,
        group_count,
        cam_mode
    );
    game.native_text(batch, &info_line, [x + 8.0, y + 29.0], 0xCBD5E1, true);

    let bar_x = x + 8.0;
    let bar_y = y + 41.0;
    let bar_w = w - 16.0;
    game.native_fill(batch, [bar_x, bar_y, bar_w, 6.0], 0xFF1E_293B);
    let progress = ((s.cursor_tick / (duration as f64)).clamp(0.0, 1.0)) as f32;
    game.native_fill(
        batch,
        [bar_x, bar_y, (bar_w * progress).max(3.0), 6.0],
        0xFFF4_3F5E,
    );

    // Interactive menu rows
    let list_y = y + 52.0;
    for (idx, label) in MENU_ITEMS.iter().enumerate() {
        let row_y = list_y + (idx as f32) * 13.0;
        let selected = idx == state.menu_cursor;
        if selected {
            game.native_fill(batch, [x + 6.0, row_y - 1.0, w - 12.0, 12.0], 0xCCBE_123C);
        }
        let prefix = if selected { "> " } else { "  " };
        let color = if selected { 0xFFFFFF } else { 0xE2E8F0 };
        game.native_text(
            batch,
            &format!("{prefix}{label}"),
            [x + 10.0, row_y + 1.0],
            color,
            true,
        );
    }

    // Footer instructions
    game.native_fill(batch, [x, y + h - 18.0, w, 18.0], 0xFF09_0A0F);
    game.native_text(
        batch,
        "Scroll / Up-Down: Select | Enter / Left-Right: Run | Right Shift: Close | Options > Mods",
        [x + 8.0, y + h - 13.0],
        0x94A3B8,
        true,
    );
}

pub(super) fn hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_on(ID) {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        if state.menu_open {
            draw_studio_menu(game, state, batch);
        }
        let cfg = state.config.as_ref().unwrap();
        if !cfg.settings.hud_overlay {
            return;
        }
        let corner = corner_from_u8(cfg.settings.hud_corner);
        if !game.native_corner_shown(corner) {
            return;
        }
        let mut lines = state.runtime.hud_lines(LOCAL_ACTOR, cfg);
        lines.push(("Right Shift: BBS Studio Menu".into(), 0xFDA4AF));
        let width = lines
            .iter()
            .map(|(text, _)| game.native_text_width(text))
            .fold(0.0_f32, f32::max);
        let box_w = width + 8.0;
        let box_h = lines.len() as f32 * 10.0 + 4.0;
        let (x, y) = corners.place(corner, box_w, box_h);
        game.native_fill(batch, [x, y, box_w, box_h], 0x7800_0000);
        for (i, (text, rgb)) in lines.iter().enumerate() {
            let ty = y + 3.0 + i as f32 * 10.0;
            game.native_text(batch, text, [x + 4.0, ty], *rgb, true);
        }
    });
}

pub(super) fn command(game: &mut Game<'_>, raw: &str, ran: &mut bool) {
    if *ran || !game.native_on(ID) {
        return;
    }
    let mut words = raw.split_whitespace();
    if words.next() != Some("bbs") {
        return;
    }
    *ran = true;
    let args: Vec<_> = words.collect();
    with_state(game, |game, state| {
        ensure_config(game, state);
        let actor = LOCAL_ACTOR;
        let world_tick = state.tick_counter;
        let reply = match args.as_slice() {
            [] | ["menu"] => {
                state.menu_open = !state.menu_open;
                format!(
                    "§b[BBS Studio 26.3]§r Studio Menu {}",
                    if state.menu_open {
                        "OPENED (use Scroll/Arrows + Enter, or Right Shift to close)"
                    } else {
                        "CLOSED"
                    }
                )
            }
            ["status"] => {
                let s = state.runtime.session(actor);
                let cfg = state.config.as_ref().unwrap();
                let timecode =
                    Runtime::format_timecode(s.cursor_tick, cfg.settings.ticks_per_second);
                let replay_count = cfg
                    .films
                    .get(&s.active_film)
                    .map_or(0, |f| f.replays.len());
                format!(
                    "§b[BBS 26.3]§r {} | film='{}' ({replay_count} replays, DataVersion {}) | mode={:?} | {timecode}",
                    if s.enabled { "ON" } else { "OFF" },
                    s.active_film,
                    engine::BBS_DATA_VERSION_26_3,
                    s.mode
                )
            }
            ["toggle"] | ["on"] | ["off"] => {
                let on = match args[0] {
                    "on" => true,
                    "off" => false,
                    _ => !state.runtime.enabled(actor),
                };
                state.runtime.set_enabled(actor, on);
                format!("§b[BBS 26.3]§r Engine {}", if on { "ON" } else { "OFF" })
            }
            ["hud"] => {
                let cfg = state.config.as_mut().unwrap();
                cfg.settings.hud_overlay = !cfg.settings.hud_overlay;
                game.native_set_setting(
                    ID,
                    "hud_overlay",
                    if cfg.settings.hud_overlay { "1" } else { "0" },
                );
                format!(
                    "§b[BBS 26.3]§r Director HUD {}",
                    if cfg.settings.hud_overlay { "ON" } else { "OFF" }
                )
            }
            ["list"] => {
                let cfg = state.config.as_ref().unwrap();
                let list: Vec<_> = cfg
                    .films
                    .values()
                    .map(|f| {
                        format!(
                            "{} ({} replays, {}t, MC {})",
                            f.id,
                            f.replays.len(),
                            f.duration_ticks(),
                            f.mc_version
                        )
                    })
                    .collect();
                format!("§b[BBS 26.3]§r Films: {}", list.join(", "))
            }
            ["create", film_id] => {
                let cfg = state.config.as_mut().unwrap();
                let film = Film {
                    id: (*film_id).to_string(),
                    ..Film::default()
                };
                match film.validate() {
                    Ok(()) => {
                        cfg.films.insert((*film_id).to_string(), film);
                        state.runtime.session_mut(actor).active_film = (*film_id).to_string();
                        format!("§a[BBS 26.3]§r Created and selected 26.3 film '{film_id}'")
                    }
                    Err(err) => format!("§c[BBS 26.3]§r {err}"),
                }
            }
            ["play"] => {
                state.runtime.play(actor, None);
                let _ = state.runtime.step(
                    &mut Native { game, world_tick },
                    state.config.as_mut().unwrap(),
                );
                let s = state.runtime.session(actor);
                format!(
                    "§a[BBS 26.3]§r Playing '{}' at tick {:.1}",
                    s.active_film, s.cursor_tick
                )
            }
            ["play", film_id] => {
                if !state.config.as_ref().unwrap().films.contains_key(*film_id) {
                    format!("§c[BBS 26.3]§r Film '{film_id}' not found")
                } else {
                    state.runtime.play(actor, Some(film_id));
                    let _ = state.runtime.step(
                        &mut Native { game, world_tick },
                        state.config.as_mut().unwrap(),
                    );
                    format!("§a[BBS 26.3]§r Playing '{film_id}'")
                }
            }
            ["pause"] => {
                state.runtime.pause(actor);
                "§e[BBS 26.3]§r Playback paused".into()
            }
            ["stop"] => {
                state.runtime.stop(actor);
                "§7[BBS 26.3]§r Playback stopped (tick 0.0)".into()
            }
            ["tick", raw_tick] => match raw_tick.parse::<f64>() {
                Ok(t) => {
                    state.runtime.seek(actor, t);
                    let _ = state.runtime.step(
                        &mut Native { game, world_tick },
                        state.config.as_mut().unwrap(),
                    );
                    format!("§b[BBS 26.3]§r Scrubbed timeline to tick {t:.1}")
                }
                Err(_) => "§c[BBS 26.3]§r Invalid tick number".into(),
            },
            ["record"] => {
                state.runtime.start_recording(actor, 0);
                let _ = state.runtime.step(
                    &mut Native { game, world_tick },
                    state.config.as_mut().unwrap(),
                );
                "§c[BBS 26.3]§r Recording replay #0".into()
            }
            ["record", raw_idx] => match raw_idx.parse::<usize>() {
                Ok(idx) => {
                    let film_id = state.runtime.session(actor).active_film;
                    if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&film_id) {
                        while film.replays.len() <= idx {
                            let mut r = Replay::default();
                            r.uuid = format!("actor-{}", film.replays.len() + 1);
                            film.replays.push(r);
                        }
                    }
                    state.runtime.start_recording(actor, idx);
                    let _ = state.runtime.step(
                        &mut Native { game, world_tick },
                        state.config.as_mut().unwrap(),
                    );
                    format!("§c[BBS 26.3]§r Recording replay #{idx}")
                }
                Err(_) => "§c[BBS 26.3]§r Invalid replay index".into(),
            },
            ["group", group_name] => {
                let s = state.runtime.session(actor);
                match state.config.as_mut().unwrap().films.get_mut(&s.active_film) {
                    Some(film) => {
                        match film.create_group_with_selected(group_name, &s.selected_replays) {
                            Ok(moved) => format!(
                                "§a[BBS 26.3]§r Created category '{group_name}' and moved {moved} selected replay(s)"
                            ),
                            Err(err) => format!("§c[BBS 26.3]§r {err}"),
                        }
                    }
                    None => "§c[BBS 26.3]§r No active BBS film".into(),
                }
            }
            ["copy-group", group_name] => {
                let s = state.runtime.session(actor);
                match state
                    .config
                    .as_ref()
                    .unwrap()
                    .films
                    .get(&s.active_film)
                    .and_then(|f| f.copy_group(group_name))
                {
                    Some(clip) => {
                        let count = clip.replays.len();
                        state.runtime.session_mut(actor).clipboard = Some(clip);
                        format!(
                            "§a[BBS 26.3]§r Copied group '{group_name}' ({count} replay(s)) to clipboard"
                        )
                    }
                    None => format!("§c[BBS 26.3]§r Group '{group_name}' not found or empty"),
                }
            }
            ["paste-group"] => {
                let s = state.runtime.session(actor);
                match s.clipboard.clone() {
                    Some(clip) => {
                        if let Some(film) =
                            state.config.as_mut().unwrap().films.get_mut(&s.active_film)
                        {
                            let inserted = film.paste_group(&clip);
                            state.runtime.session_mut(actor).selected_replays = inserted.clone();
                            format!(
                                "§a[BBS 26.3]§r Pasted group '{}' ({} replay(s)) into '{}'",
                                clip.group_path,
                                inserted.len(),
                                film.id
                            )
                        } else {
                            "§c[BBS 26.3]§r No active BBS film".into()
                        }
                    }
                    None => "§c[BBS 26.3]§r Group clipboard is empty".into(),
                }
            }
            ["transform", dx, dy, dz] => {
                if let (Ok(dx), Ok(dy), Ok(dz)) =
                    (dx.parse::<f64>(), dy.parse::<f64>(), dz.parse::<f64>())
                {
                    let s = state.runtime.session(actor);
                    if let Some(film) =
                        state.config.as_mut().unwrap().films.get_mut(&s.active_film)
                    {
                        let count = film.transform_selected_replays(
                            &s.selected_replays,
                            s.cursor_tick,
                            [dx, dy, dz],
                            0.0,
                        );
                        format!(
                            "§a[BBS 26.3]§r Transformed {count} selected replay(s) by ({dx}, {dy}, {dz})"
                        )
                    } else {
                        "§c[BBS 26.3]§r No active BBS film".into()
                    }
                } else {
                    "§c[BBS 26.3]§r Usage: /bbs transform <dx> <dy> <dz> [dyaw]".into()
                }
            }
            ["transform", dx, dy, dz, dyaw] => {
                if let (Ok(dx), Ok(dy), Ok(dz), Ok(dyaw)) = (
                    dx.parse::<f64>(),
                    dy.parse::<f64>(),
                    dz.parse::<f64>(),
                    dyaw.parse::<f32>(),
                ) {
                    let s = state.runtime.session(actor);
                    if let Some(film) =
                        state.config.as_mut().unwrap().films.get_mut(&s.active_film)
                    {
                        let count = film.transform_selected_replays(
                            &s.selected_replays,
                            s.cursor_tick,
                            [dx, dy, dz],
                            dyaw,
                        );
                        format!(
                            "§a[BBS 26.3]§r Transformed {count} selected replay(s) around combined center by ({dx}, {dy}, {dz}), yaw {dyaw}"
                        )
                    } else {
                        "§c[BBS 26.3]§r No active BBS film".into()
                    }
                } else {
                    "§c[BBS 26.3]§r Usage: /bbs transform <dx> <dy> <dz> [dyaw]".into()
                }
            }
            ["import", path] | ["import1201", path] => {
                match std::fs::read_to_string(path)
                    .map_err(|e| format!("{path}: {e}"))
                    .and_then(|raw| Film::from_26_3_json(&raw))
                {
                    Ok(film) => {
                        let id = film.id.clone();
                        state.config.as_mut().unwrap().films.insert(id.clone(), film);
                        state.runtime.session_mut(actor).active_film = id.clone();
                        format!("§a[BBS 26.3]§r Imported and upgraded film '{id}' to 26.3 from {path}")
                    }
                    Err(error) => format!("§c[BBS 26.3]§r {error}"),
                }
            }
            ["reload"] => match load_config(game) {
                Ok(config) => {
                    state.config = Some(config);
                    "§a[BBS 26.3]§r Configuration reloaded".into()
                }
                Err(error) => format!("§c[BBS 26.3]§r {error}"),
            },
            _ => {
                "§e[BBS 26.3]§r Usage: /bbs [menu|status|hud|list|create|play|pause|stop|tick|record|group|copy-group|paste-group|transform|import|reload]".into()
            }
        };
        game.native_chat(&reply);
    });
}

#[cfg(test)]
mod engine_tests {
    use super::engine as horizon_bbs;
    use horizon_bbs::*;

struct MockWorld {
    player: PlayerSnapshot,
    applied_camera: Option<CameraPoint>,
    applied_replays: Vec<EvaluatedReplay>,
    fail_camera: bool,
}

impl MockWorld {
    fn new() -> Self {
        Self {
            player: PlayerSnapshot {
                actor: [0, 0, 0, 1],
                name: "Steve".into(),
                world_tick: 0,
                position: [10.0, 64.0, -5.0],
                yaw: 45.0,
                head_yaw: 50.0,
                body_yaw: 40.0,
                pitch: -10.0,
                sneaking: false,
            },
            applied_camera: None,
            applied_replays: Vec::new(),
            fail_camera: false,
        }
    }
}

impl Host for MockWorld {
    type Error = &'static str;

    fn player(&mut self) -> PlayerSnapshot {
        self.player.clone()
    }

    fn apply_camera(&mut self, point: &CameraPoint) -> Result<(), Self::Error> {
        if self.fail_camera {
            return Err("camera locked by external controller");
        }
        self.applied_camera = Some(point.clone());
        Ok(())
    }

    fn apply_replays(&mut self, replays: &[EvaluatedReplay]) -> Result<(), Self::Error> {
        self.applied_replays = replays.to_vec();
        Ok(())
    }
}

#[test]
fn keyframe_channel_supports_linear_step_bezier_cubic_and_hermite() {
    let mut ch = KeyframeChannel::default();
    ch.insert(Keyframe::new(0.0, 0.0, Interpolation::Linear));
    ch.insert(Keyframe::new(10.0, 100.0, Interpolation::Step));
    ch.insert(Keyframe::new(20.0, 200.0, Interpolation::Bezier));
    ch.insert(Keyframe::new(30.0, 300.0, Interpolation::Cubic));

    assert_eq!(ch.evaluate(-5.0), Some(0.0));
    assert_eq!(ch.evaluate(5.0), Some(50.0));
    assert_eq!(ch.evaluate(15.0), Some(100.0));
    let bez = ch.evaluate(25.0).unwrap();
    assert!((bez - 250.0).abs() < 1e-6);
    assert_eq!(ch.evaluate(35.0), Some(300.0));
}

#[test]
fn category_creation_instantly_moves_selected_replays_and_supports_scene_copy_paste() {
    let mut film_a = Film::default();
    let mut r2 = Replay::default();
    r2.uuid = "actor-2".into();
    r2.label = "player/alex".into();
    let mut r3 = Replay::default();
    r3.uuid = "actor-3".into();
    r3.label = "guard".into();
    film_a.replays.push(r2);
    film_a.replays.push(r3);

    let moved = film_a
        .create_group_with_selected("police_squad", &[0, 2])
        .unwrap();
    assert_eq!(moved, 2);
    assert_eq!(film_a.replays[0].group, "police_squad");
    assert_eq!(film_a.replays[1].group, "");
    assert_eq!(film_a.replays[2].group, "police_squad");

    let clipboard = film_a.copy_group("police_squad").expect("clipboard");
    assert_eq!(clipboard.replays.len(), 2);

    let mut film_b = Film {
        id: "scene_2".into(),
        replays: Vec::new(),
        groups: Vec::new(),
        ..Film::default()
    };
    let pasted = film_b.paste_group(&clipboard);
    assert_eq!(pasted.len(), 2);
    assert_eq!(film_b.groups.len(), 1);
    assert_eq!(film_b.groups[0].id, "police_squad");
    assert_eq!(film_b.replays[0].group, "police_squad");
    assert_eq!(film_b.replays[1].group, "police_squad");
}

#[test]
fn multi_replay_transform_computes_combined_center_and_smoothly_shifts_all_trajectories() {
    let mut film = Film {
        id: "transform_test".into(),
        replays: Vec::new(),
        ..Film::default()
    };
    let mut a = Replay::default();
    a.record_sample(0.0, [0.0, 64.0, 0.0], 0.0, 0.0, 0.0, 0.0, false);
    a.record_sample(20.0, [0.0, 64.0, 10.0], 0.0, 0.0, 0.0, 0.0, false);

    let mut b = Replay::default();
    b.record_sample(0.0, [10.0, 64.0, 0.0], 0.0, 0.0, 0.0, 0.0, false);
    b.record_sample(20.0, [10.0, 64.0, 10.0], 0.0, 0.0, 0.0, 0.0, false);

    film.replays.push(a);
    film.replays.push(b);

    let center = film.combined_replays_center(&[0, 1], 0.0).unwrap();
    assert_eq!(center, [5.0, 64.0, 0.0]);

    let count = film.transform_selected_replays(&[0, 1], 0.0, [15.0, 2.0, -5.0], 0.0);
    assert_eq!(count, 2);

    let ev_a0 = film.replays[0].evaluate(0.0);
    let ev_a20 = film.replays[0].evaluate(20.0);
    let ev_b0 = film.replays[1].evaluate(0.0);
    assert_eq!(ev_a0.position, [15.0, 66.0, -5.0]);
    assert_eq!(ev_a20.position, [15.0, 66.0, 5.0]);
    assert_eq!(ev_b0.position, [25.0, 66.0, -5.0]);
}

#[test]
fn orbit_camera_focuses_on_main_anchor_bone_and_chest_height() {
    let form = Form::default();
    let bones = vec![
        "left_arm".to_string(),
        "right_arm".to_string(),
        "anchor".to_string(),
        "head".to_string(),
    ];
    assert_eq!(form.main_anchor_bone(&bones).as_deref(), Some("anchor"));

    let mut replay = Replay::default();
    replay.record_sample(0.0, [100.0, 70.0, 200.0], 0.0, 0.0, 0.0, 0.0, false);

    let clip = CameraClip {
        id: "bbs:orbit".into(),
        kind: CameraClipKind::Orbit,
        title: "Orbit Hero".into(),
        layer: 0,
        tick: 0,
        duration: 40,
        enabled: true,
        start: CameraPoint {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov: 70.0,
        },
        end: CameraPoint::default(),
        path_points: Vec::new(),
        distance: 4.0,
        circles: 1.0,
        interp: Interpolation::Linear,
        target_replay: Some(0),
    };

    let cam = clip.evaluate(10.0, &[replay]);
    assert!((cam.position[0] - 100.0).abs() < 1e-6);
    assert!((cam.position[1] - 70.9).abs() < 1e-6);
    assert!((cam.position[2] - 204.0).abs() < 1e-6);
}

#[test]
fn random_textures_and_26_3_json_roundtrip_preserve_film_data() {
    let mut film = Film::default();
    let mut r2 = Replay::default();
    r2.uuid = "replay-2".into();
    film.replays.push(r2);

    let pool = vec![
        "assets:Skins/Bank/Banker1.png".to_string(),
        "assets:Skins/Bank/Banker2.png".to_string(),
    ];
    let count = film.apply_random_textures(&[0, 1], &pool, 42);
    assert_eq!(count, 2);
    assert!(pool.contains(film.replays[0].form.as_ref().unwrap().texture.as_ref().unwrap()));

    let json = film.to_26_3_json().unwrap();
    let decoded = Film::from_26_3_json(&json).unwrap();
    assert_eq!(decoded, film);
    assert_eq!(decoded.mc_version, "26.3");
    assert_eq!(decoded.data_version, 5023);
}

#[test]
fn runtime_recording_and_playback_step_through_timeline_accurately() {
    let mut world = MockWorld::new();
    let mut config = Config::default();
    let mut rt = Runtime::default();
    let actor = world.player.actor;

    rt.start_recording(actor, 0);
    world.player.world_tick = 100;
    world.player.position = [1.0, 64.0, 2.0];
    let rep1 = rt.step(&mut world, &mut config).unwrap();
    assert!(rep1.recorded_sample);
    assert_eq!(rep1.tick, 0.0);

    world.player.world_tick = 110;
    world.player.position = [11.0, 64.0, 22.0];
    let rep2 = rt.step(&mut world, &mut config).unwrap();
    assert_eq!(rep2.tick, 10.0);

    rt.play(actor, Some("scene_1"));
    rt.seek(actor, 5.0);
    world.player.world_tick = 200;
    let play_rep = rt.step(&mut world, &mut config).unwrap();
    assert_eq!(play_rep.mode, PlaybackMode::Playing);
    assert!(play_rep.camera_applied);
    assert_eq!(world.applied_replays.len(), 1);
    assert_eq!(world.applied_replays[0].position, [6.0, 64.0, 12.0]);
}

}
