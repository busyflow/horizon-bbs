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
fn random_textures_and_1201_json_roundtrip_preserve_film_data() {
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

    let json = film.to_1201_json().unwrap();
    let decoded = Film::from_1201_json(&json).unwrap();
    assert_eq!(decoded, film);
    assert_eq!(decoded.mc_version, "1.20.1");
    assert_eq!(decoded.data_version, 3465);
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
