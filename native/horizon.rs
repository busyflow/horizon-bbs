// Native adapter for Horizon 26.3 (`hooks: 1` Native Hooks API).
use super::Game;
use super::native_hooks::NativeMod;
use horizon_client::{extras::Corner, extras_hud::Corners, hud::HudBatch};

mod engine {
    include!("bbs_engine.rs");
}
use engine::{
    CameraPoint, Config, EvaluatedReplay, Film, Host, PlaybackMode, PlayerSnapshot, Replay,
    Runtime,
};

const ID: &str = "bbs";

#[derive(Default)]
pub(super) struct State {
    runtime: Runtime,
    config: Option<Config>,
    tick_counter: u64,
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
        ("play_pause", "Play / Pause Film", "key.keyboard.p"),
        ("record", "Record Active Replay", "key.keyboard.r"),
        ("scrub_zero", "Reset Timeline to 0", "key.keyboard.o"),
        ("hud", "Toggle Director HUD", "key.keyboard.u"),
    ];
    mods.push(NativeMod {
        id: ID.into(),
        title: "BBS Studio 26.3".into(),
        description: "Blockbuster Studio 26.3 — cinematic director, keyframe replay, multi-replay transform & camera engine.".into(),
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
                cfg.settings.hud_overlay = !cfg.settings.hud_overlay;
                game.native_set_setting(
                    ID,
                    "hud_overlay",
                    if cfg.settings.hud_overlay { "1" } else { "0" },
                );
            }
            1 => {
                cfg.settings.hud_corner = (cfg.settings.hud_corner + 1) % 4;
                game.native_set_setting(ID, "hud_corner", &cfg.settings.hud_corner.to_string());
            }
            2 => {
                let s = state.runtime.session(LOCAL_ACTOR);
                match s.mode {
                    PlaybackMode::Playing => state.runtime.pause(LOCAL_ACTOR),
                    _ => state.runtime.play(LOCAL_ACTOR, None),
                }
            }
            3 => {
                cfg.settings.loop_playback = !cfg.settings.loop_playback;
                game.native_set_setting(
                    ID,
                    "loop_playback",
                    if cfg.settings.loop_playback { "1" } else { "0" },
                );
            }
            4 => {
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
            5 => {
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

pub(super) fn key(game: &mut Game<'_>, key_name: &str, pressed: bool) {
    if !pressed || !game.native_playing() || !game.native_on(ID) {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        let play_key = game.native_mod_key(ID, "play_pause", "key.keyboard.p");
        let rec_key = game.native_mod_key(ID, "record", "key.keyboard.r");
        let zero_key = game.native_mod_key(ID, "scrub_zero", "key.keyboard.o");
        let hud_key = game.native_mod_key(ID, "hud", "key.keyboard.u");

        if key_name == play_key {
            let s = state.runtime.session(LOCAL_ACTOR);
            if s.mode == PlaybackMode::Playing {
                state.runtime.pause(LOCAL_ACTOR);
                game.native_chat("§b[BBS 26.3]§r Playback paused.");
            } else {
                state.runtime.play(LOCAL_ACTOR, None);
                game.native_chat(&format!(
                    "§a[BBS 26.3]§r Playing '{}' from tick {:.1}.",
                    s.active_film, s.cursor_tick
                ));
            }
        } else if key_name == rec_key {
            let s = state.runtime.session(LOCAL_ACTOR);
            if matches!(s.mode, PlaybackMode::Recording { .. }) {
                state.runtime.pause(LOCAL_ACTOR);
                game.native_chat("§e[BBS 26.3]§r Recording finished and saved to replay.");
            } else {
                let idx = s.selected_replays.first().copied().unwrap_or(0);
                state.runtime.start_recording(LOCAL_ACTOR, idx);
                game.native_chat(&format!(
                    "§c[BBS 26.3]§r Recording replay #{idx} in '{}'...",
                    s.active_film
                ));
            }
        } else if key_name == zero_key {
            state.runtime.stop(LOCAL_ACTOR);
            game.native_chat("§7[BBS 26.3]§r Timeline reset to 00:00.00 (tick 0.0).");
        } else if key_name == hud_key {
            let cfg = state.config.as_mut().unwrap();
            cfg.settings.hud_overlay = !cfg.settings.hud_overlay;
            game.native_set_setting(
                ID,
                "hud_overlay",
                if cfg.settings.hud_overlay { "1" } else { "0" },
            );
            game.native_chat(&format!(
                "§b[BBS 26.3]§r Director HUD: {}",
                if cfg.settings.hud_overlay {
                    "ON"
                } else {
                    "OFF"
                }
            ));
        }
    });
}

pub(super) fn hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_on(ID) {
        return;
    }
    with_state(game, |game, state| {
        ensure_config(game, state);
        let cfg = state.config.as_ref().unwrap();
        if !cfg.settings.hud_overlay {
            return;
        }
        let corner = corner_from_u8(cfg.settings.hud_corner);
        if !game.native_corner_shown(corner) {
            return;
        }
        let lines = state.runtime.hud_lines(LOCAL_ACTOR, cfg);
        if lines.is_empty() {
            return;
        }
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
            [] | ["status"] => {
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
                "§e[BBS 26.3]§r Usage: /bbs [status|hud|list|create|play|pause|stop|tick|record|group|copy-group|paste-group|transform|import|reload]".into()
            }
        };
        game.native_chat(&reply);
    });
}

#[cfg(test)]
mod engine_tests {
    use super::engine as horizon_bbs;
    include!("bbs_tests.rs");
}
