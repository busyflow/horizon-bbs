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
