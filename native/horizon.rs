// Native adapter for Horizon d91cab68ded29bd2b004f7050c468fb98c41a00b (Minecraft 26.3 / BBS 1.20.1 port).
use super::Game;
mod engine {
    include!("bbs_engine.rs");
}
use engine::{
    CameraPoint, Config, EvaluatedReplay, Film, Host, PlaybackMode, PlayerSnapshot, Replay, Runtime,
};

#[derive(Default)]
pub(super) struct State {
    runtime: Runtime,
    config: Option<Config>,
}

thread_local! {
    static STATES: std::cell::RefCell<std::collections::BTreeMap<std::path::PathBuf, State>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

fn with_state<T>(game: &mut Game<'_>, run: impl FnOnce(&mut Game<'_>, &mut State) -> T) -> T {
    struct Lease {
        key: std::path::PathBuf,
        state: State,
    }
    impl Drop for Lease {
        fn drop(&mut self) {
            STATES.with(|states| {
                states
                    .borrow_mut()
                    .insert(self.key.clone(), std::mem::take(&mut self.state));
            });
        }
    }
    let key = game.world_root.clone();
    let state = STATES.with(|states| states.borrow_mut().remove(&key).unwrap_or_default());
    let mut lease = Lease { key, state };
    run(game, &mut lease.state)
}

fn config_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HORIZON_BBS_CONFIG").map(Into::into)
}

fn load_config() -> std::result::Result<Config, String> {
    match config_path() {
        Some(path) => Config::from_json(
            &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        ),
        None => Ok(Config::default()),
    }
}

fn ensure_config(state: &mut State) {
    if state.config.is_none() {
        state.config = Some(load_config().unwrap_or_else(|error| {
            eprintln!("BBS config error (using defaults): {error}");
            Config::default()
        }));
    }
}

struct Native<'g, 'r> {
    game: &'g mut Game<'r>,
}

impl Host for Native<'_, '_> {
    type Error = String;

    fn player(&mut self) -> PlayerSnapshot {
        PlayerSnapshot {
            actor: self.game.acting_player_uuid(),
            name: self
                .game
                .lan_acting_name
                .as_deref()
                .unwrap_or(super::chat::PLAYER_NAME)
                .to_owned(),
            world_tick: self.game.world.game_time,
            position: [0.0, 64.0, 0.0],
            yaw: 0.0,
            head_yaw: 0.0,
            body_yaw: 0.0,
            pitch: 0.0,
            sneaking: self
                .game
                .keys
                .contains(&winit::keyboard::KeyCode::ShiftLeft),
        }
    }

    fn apply_camera(&mut self, _point: &CameraPoint) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    fn apply_replays(&mut self, _replays: &[EvaluatedReplay]) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl Game<'_> {
    pub(super) fn bbs_command(&mut self, command: &str) -> Option<bool> {
        with_state(self, |game, state| {
            let mut words = command.split_whitespace();
            if words.next()? != "bbs" {
                return None;
            }
            ensure_config(state);
            let args: Vec<_> = words.collect();
            let actor = game.acting_player_uuid();
            let message = match args.as_slice() {
                [] | ["status"] => {
                    let s = state.runtime.session(actor);
                    let cfg = state.config.as_ref().unwrap();
                    let timecode = Runtime::format_timecode(s.cursor_tick, cfg.settings.ticks_per_second);
                    let replay_count = cfg
                        .films
                        .get(&s.active_film)
                        .map_or(0, |f| f.replays.len());
                    format!(
                        "BBS 1.20.1 Engine: {} | film='{}' ({replay_count} replays) | mode={:?} | {timecode}",
                        if s.enabled { "on" } else { "off" },
                        s.active_film,
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
                    format!("BBS: {}", if on { "on" } else { "off" })
                }
                ["list"] => {
                    let cfg = state.config.as_ref().unwrap();
                    let list: Vec<_> = cfg
                        .films
                        .values()
                        .map(|f| format!("{} ({} replays, {}t)", f.id, f.replays.len(), f.duration_ticks()))
                        .collect();
                    format!("BBS films: {}", list.join(", "))
                }
                ["create", film_id] => {
                    let cfg = state.config.as_mut().unwrap();
                    let film = Film {
                        id: (*film_id).to_string(),
                        ..Film::default()
                    };
                    if let Err(err) = film.validate() {
                        game.command_failure(horizon_client::text::Text::literal(err));
                        return Some(false);
                    }
                    cfg.films.insert((*film_id).to_string(), film);
                    state.runtime.session_mut(actor).active_film = (*film_id).to_string();
                    format!("BBS created and selected film '{film_id}'")
                }
                ["play"] => {
                    state.runtime.play(actor, None);
                    let _ = state.runtime.step(&mut Native { game }, state.config.as_mut().unwrap());
                    let s = state.runtime.session(actor);
                    format!("BBS playing '{}' at tick {:.1}", s.active_film, s.cursor_tick)
                }
                ["play", film_id] => {
                    if !state.config.as_ref().unwrap().films.contains_key(*film_id) {
                        game.command_failure(horizon_client::text::Text::literal(format!(
                            "BBS film '{film_id}' not found"
                        )));
                        return Some(false);
                    }
                    state.runtime.play(actor, Some(film_id));
                    let _ = state.runtime.step(&mut Native { game }, state.config.as_mut().unwrap());
                    format!("BBS playing '{film_id}'")
                }
                ["pause"] => {
                    state.runtime.pause(actor);
                    "BBS playback paused".into()
                }
                ["stop"] => {
                    state.runtime.stop(actor);
                    "BBS playback stopped (tick 0.0)".into()
                }
                ["tick", raw_tick] => {
                    let Ok(tick) = raw_tick.parse::<f64>() else {
                        game.command_failure(horizon_client::text::Text::literal("Invalid tick number"));
                        return Some(false);
                    };
                    state.runtime.seek(actor, tick);
                    let _ = state.runtime.step(&mut Native { game }, state.config.as_mut().unwrap());
                    format!("BBS scrubbed timeline to tick {tick:.1}")
                }
                ["record"] => {
                    state.runtime.start_recording(actor, 0);
                    let _ = state.runtime.step(&mut Native { game }, state.config.as_mut().unwrap());
                    "BBS recording replay #0".into()
                }
                ["record", raw_idx] => {
                    let Ok(idx) = raw_idx.parse::<usize>() else {
                        game.command_failure(horizon_client::text::Text::literal("Invalid replay index"));
                        return Some(false);
                    };
                    let film_id = state.runtime.session(actor).active_film;
                    if let Some(film) = state.config.as_mut().unwrap().films.get_mut(&film_id) {
                        while film.replays.len() <= idx {
                            let mut r = Replay::default();
                            r.uuid = format!("actor-{}", film.replays.len() + 1);
                            film.replays.push(r);
                        }
                    }
                    state.runtime.start_recording(actor, idx);
                    let _ = state.runtime.step(&mut Native { game }, state.config.as_mut().unwrap());
                    format!("BBS recording replay #{idx}")
                }
                ["group", group_name] => {
                    let s = state.runtime.session(actor);
                    let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) else {
                        game.command_failure(horizon_client::text::Text::literal("No active BBS film"));
                        return Some(false);
                    };
                    match film.create_group_with_selected(group_name, &s.selected_replays) {
                        Ok(moved) => format!("BBS created group '{group_name}' and moved {moved} selected replay(s)"),
                        Err(err) => {
                            game.command_failure(horizon_client::text::Text::literal(err));
                            return Some(false);
                        }
                    }
                }
                ["copy-group", group_name] => {
                    let s = state.runtime.session(actor);
                    let Some(film) = state.config.as_ref().unwrap().films.get(&s.active_film) else {
                        game.command_failure(horizon_client::text::Text::literal("No active BBS film"));
                        return Some(false);
                    };
                    match film.copy_group(group_name) {
                        Some(clip) => {
                            let count = clip.replays.len();
                            state.runtime.session_mut(actor).clipboard = Some(clip);
                            format!("BBS copied group '{group_name}' ({count} replay(s)) to clipboard")
                        }
                        None => {
                            game.command_failure(horizon_client::text::Text::literal(format!(
                                "Group '{group_name}' not found or empty"
                            )));
                            return Some(false);
                        }
                    }
                }
                ["paste-group"] => {
                    let s = state.runtime.session(actor);
                    let Some(clip) = s.clipboard.clone() else {
                        game.command_failure(horizon_client::text::Text::literal("BBS group clipboard is empty"));
                        return Some(false);
                    };
                    let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) else {
                        game.command_failure(horizon_client::text::Text::literal("No active BBS film"));
                        return Some(false);
                    };
                    let inserted = film.paste_group(&clip);
                    state.runtime.session_mut(actor).selected_replays = inserted.clone();
                    format!(
                        "BBS pasted group '{}' ({} replay(s)) into '{}'",
                        clip.group_path,
                        inserted.len(),
                        film.id
                    )
                }
                ["transform", dx, dy, dz] => {
                    let (Ok(dx), Ok(dy), Ok(dz)) = (dx.parse::<f64>(), dy.parse::<f64>(), dz.parse::<f64>()) else {
                        game.command_failure(horizon_client::text::Text::literal(
                            "Usage: /bbs transform <dx> <dy> <dz> [dyaw]",
                        ));
                        return Some(false);
                    };
                    let s = state.runtime.session(actor);
                    let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) else {
                        game.command_failure(horizon_client::text::Text::literal("No active BBS film"));
                        return Some(false);
                    };
                    let count = film.transform_selected_replays(&s.selected_replays, s.cursor_tick, [dx, dy, dz], 0.0);
                    format!("BBS transformed {count} selected replay(s) by ({dx}, {dy}, {dz})")
                }
                ["transform", dx, dy, dz, dyaw] => {
                    let (Ok(dx), Ok(dy), Ok(dz), Ok(dyaw)) = (
                        dx.parse::<f64>(),
                        dy.parse::<f64>(),
                        dz.parse::<f64>(),
                        dyaw.parse::<f32>(),
                    ) else {
                        game.command_failure(horizon_client::text::Text::literal(
                            "Usage: /bbs transform <dx> <dy> <dz> [dyaw]",
                        ));
                        return Some(false);
                    };
                    let s = state.runtime.session(actor);
                    let Some(film) = state.config.as_mut().unwrap().films.get_mut(&s.active_film) else {
                        game.command_failure(horizon_client::text::Text::literal("No active BBS film"));
                        return Some(false);
                    };
                    let count = film.transform_selected_replays(&s.selected_replays, s.cursor_tick, [dx, dy, dz], dyaw);
                    format!(
                        "BBS transformed {count} selected replay(s) around combined center by ({dx}, {dy}, {dz}), yaw {dyaw}"
                    )
                }
                ["import1201", path] => {
                    match std::fs::read_to_string(path)
                        .map_err(|e| format!("{path}: {e}"))
                        .and_then(|raw| Film::from_1201_json(&raw))
                    {
                        Ok(film) => {
                            let id = film.id.clone();
                            state.config.as_mut().unwrap().films.insert(id.clone(), film);
                            state.runtime.session_mut(actor).active_film = id.clone();
                            format!("BBS imported 1.20.1 film '{id}' from {path}")
                        }
                        Err(error) => {
                            game.command_failure(horizon_client::text::Text::literal(error));
                            return Some(false);
                        }
                    }
                }
                ["reload"] => {
                    if game.ui.registries.permission < 2 {
                        game.command_failure(horizon_client::text::Text::literal(
                            "BBS reload requires permission level 2",
                        ));
                        return Some(false);
                    }
                    match load_config() {
                        Ok(config) => {
                            state.config = Some(config);
                            "BBS configuration reloaded".into()
                        }
                        Err(error) => {
                            game.command_failure(horizon_client::text::Text::literal(error));
                            return Some(false);
                        }
                    }
                }
                _ => {
                    game.command_failure(horizon_client::text::Text::literal(
                        "Usage: /bbs [status|list|create|play|pause|stop|tick|record|group|copy-group|paste-group|transform|import1201|reload]",
                    ));
                    return Some(false);
                }
            };
            let _ = PlaybackMode::Stopped;
            game.system_message(horizon_client::text::Text::literal(message));
            Some(true)
        })
    }
}

#[cfg(test)]
mod engine_tests {
    use super::engine as horizon_bbs;
    include!("bbs_tests.rs");
}
