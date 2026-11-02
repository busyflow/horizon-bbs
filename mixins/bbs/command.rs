fn bbs_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::bbs::command(game, command, ran);
}
