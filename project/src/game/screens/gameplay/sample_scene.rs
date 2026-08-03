use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::scene::prelude::SceneEvent;
use crate::game::navigation::{AppUiMode, GameRouteCommand};

pub(super) fn route_to_lobby_on_sample_scene_exit(
    mut scene_events: MessageReader<SceneEvent>,
    current_mode: Res<State<AppUiMode>>,
    mut route_cursor: Local<MessageCursor<GameRouteCommand>>,
    mut route_messages: ResMut<Messages<GameRouteCommand>>,
) {
    let already_routing_to_lobby = route_cursor
        .read(&route_messages)
        .any(is_lobby_route_command);

    let sample_scene_exited = scene_events.read().any(|event| {
        matches!(
            event,
            SceneEvent::Exited(exited)
                if exited.scene_id.as_str() == crate::game::scenes::SAMPLE_DUNGEON_ROOM_SCENE_ID
        )
    });

    if should_route_sample_scene_exit_to_lobby(*current_mode.get(), already_routing_to_lobby)
        && sample_scene_exited
    {
        route_messages.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
    }
}

fn should_route_sample_scene_exit_to_lobby(
    current_mode: AppUiMode,
    already_routing_to_lobby: bool,
) -> bool {
    current_mode == AppUiMode::SampleScene && !already_routing_to_lobby
}

fn is_lobby_route_command(command: &GameRouteCommand) -> bool {
    matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_scene_exit_fallback_only_routes_while_hud_is_active() {
        assert!(should_route_sample_scene_exit_to_lobby(
            AppUiMode::SampleScene,
            false
        ));
        assert!(!should_route_sample_scene_exit_to_lobby(
            AppUiMode::SampleScene,
            true
        ));
        assert!(!should_route_sample_scene_exit_to_lobby(
            AppUiMode::Lobby,
            false
        ));
        assert!(is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::Lobby
        )));
    }
}
