use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use crate::framework::devtools::live_preview::{
    LivePreviewClock, LivePreviewCollectionBuffer, LivePreviewSnapshotHub, LivePreviewTimeline,
    LivePreviewTimelineEvent, LivePreviewTimelineSeverity, LivePreviewTimelineType,
    PlayerAttributesPreview, PlayerElementValuesPreview, PlayerPreviewState, PreviewSection,
    StablePreviewId,
};
use crate::game::myserver::{
    CharacterAttributes, CharacterElementsCache, CharacterSelectionState, CharacterSummary,
    MyServerSession,
};
use crate::game::scenes::{
    main_world_entry::{MainWorldEntryPhase, MainWorldEntryState},
    main_world_movement::MainWorldMovementRuntime,
    main_world_players::{MainWorldPlayer, MainWorldPlayerOwnership},
};

/// Adapter boundary for the game-owned player/session facts. It intentionally
/// exposes only the already-redacted preview DTO, never the whole session.
pub trait PlayerPreviewAdapter {
    fn collect_player_preview(&self) -> PlayerPreviewState;
}

pub(in crate::game) struct MyServerPlayerPreviewAdapter<'a> {
    pub session: &'a MyServerSession,
    pub entry: Option<&'a MainWorldEntryState>,
    pub movement: Option<&'a MainWorldMovementRuntime>,
    pub local_player: Option<(&'a MainWorldPlayer, &'a Transform)>,
    pub now: SystemTime,
}

impl PlayerPreviewAdapter for MyServerPlayerPreviewAdapter<'_> {
    fn collect_player_preview(&self) -> PlayerPreviewState {
        collect_player_preview_state(
            self.session,
            self.entry,
            self.movement,
            self.local_player,
            self.now,
        )
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub(in crate::game) struct PlayerPreviewCollectorState {
    last_state: Option<PlayerPreviewState>,
    revision: u64,
}

pub(crate) struct GameLivePreviewPlugin;

impl Plugin for GameLivePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerPreviewCollectorState>()
            .add_systems(
                PostUpdate,
                collect_player_preview
                    .in_set(crate::framework::devtools::live_preview::LivePreviewSet::Collect),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_player_preview(
    clock: Res<LivePreviewClock>,
    session: Option<Res<MyServerSession>>,
    entry: Option<Res<MainWorldEntryState>>,
    movement: Option<Res<MainWorldMovementRuntime>>,
    players: Query<(&MainWorldPlayer, &Transform)>,
    mut collector_state: ResMut<PlayerPreviewCollectorState>,
    mut buffer: ResMut<LivePreviewCollectionBuffer>,
    hub: Res<LivePreviewSnapshotHub>,
    mut timeline: ResMut<LivePreviewTimeline>,
) {
    let now = SystemTime::now();
    let next_state = session.as_deref().map_or_else(
        || PlayerPreviewState {
            movement_state: Some("not_applicable".to_owned()),
            ..Default::default()
        },
        |session| {
            let local_player = session.character_id.as_deref().and_then(|character_id| {
                players.iter().find(|(player, _)| {
                    player.ownership == MainWorldPlayerOwnership::Local
                        && player.character_id == character_id
                })
            });
            MyServerPlayerPreviewAdapter {
                session,
                entry: entry.as_deref(),
                movement: movement.as_deref(),
                local_player,
                now,
            }
            .collect_player_preview()
        },
    );

    if collector_state.last_state.as_ref() == Some(&next_state) && collector_state.revision != 0 {
        return;
    }

    let previous = collector_state.last_state.replace(next_state.clone());
    collector_state.revision = collector_state.revision.saturating_add(1).max(1);
    buffer.set_player(PreviewSection::available(
        collector_state.revision,
        next_state.clone(),
    ));

    if let Some(previous) = previous.as_ref() {
        record_player_changes(
            previous,
            &next_state,
            &mut timeline,
            clock.monotonic_ms(),
            next_publish_sequence(hub.read().sequence),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn collect_player_preview_state(
    session: &MyServerSession,
    entry: Option<&MainWorldEntryState>,
    movement: Option<&MainWorldMovementRuntime>,
    local_player: Option<(&MainWorldPlayer, &Transform)>,
    now: SystemTime,
) -> PlayerPreviewState {
    let character_id = session
        .character_id
        .as_deref()
        .and_then(non_empty)
        .map(StablePreviewId::new);
    let selected = selected_character(session);
    let display_name = selected.map(|character| character.name.clone());
    let world_id = session
        .world_id
        .or_else(|| selected.and_then(|character| character.world_id))
        .map(|world_id| StablePreviewId::new(world_id.to_string()));
    let attributes = collect_attributes(session, selected, now);
    let gameplay = local_gameplay_state(entry, movement, local_player);

    PlayerPreviewState {
        character_id,
        display_name,
        world_id,
        selection_state: Some(selection_state_label(session.character_selection_state).to_owned()),
        attributes: attributes
            .as_ref()
            .map(|attributes| attributes.value.clone()),
        attributes_source: attributes
            .as_ref()
            .map(|attributes| attributes.source.to_owned()),
        attributes_snapshot_refreshed_at_ms: attributes
            .as_ref()
            .and_then(|attributes| attributes.refreshed_at_ms),
        attributes_push_sequence: attributes
            .as_ref()
            .and_then(|attributes| attributes.push_sequence),
        attributes_revision: attributes
            .as_ref()
            .and_then(|attributes| attributes.revision),
        attributes_freshness: attributes
            .as_ref()
            .map(|attributes| attributes.freshness.to_owned()),
        position: gameplay.as_ref().map(|gameplay| gameplay.position),
        direction: gameplay.as_ref().map(|gameplay| gameplay.direction),
        movement_state: Some(
            gameplay
                .as_ref()
                .map_or("not_applicable", |gameplay| gameplay.movement_state)
                .to_owned(),
        ),
        authority_frame: gameplay
            .as_ref()
            .and_then(|gameplay| gameplay.authority_frame),
    }
}

struct CollectedAttributes {
    value: PlayerAttributesPreview,
    source: &'static str,
    refreshed_at_ms: Option<u64>,
    push_sequence: Option<u64>,
    revision: Option<u64>,
    freshness: &'static str,
}

fn collect_attributes(
    session: &MyServerSession,
    selected: Option<&CharacterSummary>,
    now: SystemTime,
) -> Option<CollectedAttributes> {
    let character_id = session.character_id.as_deref().and_then(non_empty)?;
    let cache_matches = session
        .character_elements
        .character_id
        .as_deref()
        .is_some_and(|cached| cached == character_id)
        && session.character_elements.snapshot_refreshed_at.is_some();
    if cache_matches {
        let cache = &session.character_elements;
        return Some(collected_from_cache(cache, now));
    }

    let profile_attributes = selected.and_then(|character| character.attributes.as_ref());
    profile_attributes.map(|attributes| CollectedAttributes {
        value: attributes_preview(attributes),
        source: "profile",
        refreshed_at_ms: None,
        push_sequence: None,
        revision: None,
        freshness: "unknown",
    })
}

fn collected_from_cache(cache: &CharacterElementsCache, now: SystemTime) -> CollectedAttributes {
    let refreshed_at_ms = cache.snapshot_refreshed_at.and_then(system_time_millis);
    let source = if cache.last_push_sequence.is_some() || cache.last_push_revision.is_some() {
        "elements_push"
    } else {
        "elements_snapshot"
    };
    let freshness = cache
        .snapshot_refreshed_at
        .map(|refreshed| freshness_label(refreshed, now))
        .unwrap_or("unknown");
    CollectedAttributes {
        value: PlayerAttributesPreview {
            affinity: element_values_preview(cache.affinity),
            mastery: element_values_preview(cache.mastery),
        },
        source,
        refreshed_at_ms,
        push_sequence: cache.last_push_sequence,
        revision: cache.last_push_revision,
        freshness,
    }
}

fn attributes_preview(attributes: &CharacterAttributes) -> PlayerAttributesPreview {
    PlayerAttributesPreview {
        affinity: element_values_preview(attributes.affinity),
        mastery: element_values_preview(attributes.mastery),
    }
}

fn element_values_preview(
    values: crate::game::myserver::ElementValues,
) -> PlayerElementValuesPreview {
    PlayerElementValuesPreview {
        earth: values.earth,
        fire: values.fire,
        water: values.water,
        wind: values.wind,
    }
}

struct LocalGameplayState {
    position: [f32; 3],
    direction: [f32; 3],
    movement_state: &'static str,
    authority_frame: Option<u64>,
}

fn local_gameplay_state(
    entry: Option<&MainWorldEntryState>,
    movement: Option<&MainWorldMovementRuntime>,
    local_player: Option<(&MainWorldPlayer, &Transform)>,
) -> Option<LocalGameplayState> {
    let movement = movement.filter(|movement| {
        !movement.input_frozen
            && movement.session_id.is_some()
            && entry.is_some_and(|entry| {
                entry.phase == MainWorldEntryPhase::Active && !entry.input_frozen
            })
    })?;
    let position = local_player
        .map(|(_, transform)| transform.translation)
        .unwrap_or(movement.predicted.position);
    let direction = movement.predicted.direction;
    let authority_frame = local_player
        .map(|(player, _)| player.last_authoritative_frame as u64)
        .or_else(|| {
            movement
                .authority_baseline
                .map(|baseline| baseline.frame.0 as u64)
        });
    Some(LocalGameplayState {
        position: [position.x, position.y, position.z],
        direction: [direction.x, 0.0, direction.y],
        movement_state: if movement.predicted.moving {
            "moving"
        } else {
            "idle"
        },
        authority_frame,
    })
}

fn selected_character(session: &MyServerSession) -> Option<&CharacterSummary> {
    let character_id = session.character_id.as_deref()?;
    session
        .current_character
        .as_ref()
        .filter(|character| character.character_id == character_id)
        .or_else(|| {
            session
                .character_profile
                .as_ref()
                .filter(|profile| profile.character.character_id == character_id)
                .map(|profile| &profile.character)
        })
        .or_else(|| {
            session
                .characters
                .iter()
                .find(|character| character.character_id == character_id)
        })
}

fn selection_state_label(state: CharacterSelectionState) -> &'static str {
    match state {
        CharacterSelectionState::NotLoaded => "not_loaded",
        CharacterSelectionState::Loading => "loading",
        CharacterSelectionState::NoCharacters => "no_characters",
        CharacterSelectionState::Creating => "creating",
        CharacterSelectionState::AwaitingSelection => "awaiting_selection",
        CharacterSelectionState::LoadingProfile => "loading_profile",
        CharacterSelectionState::Selecting => "selecting",
        CharacterSelectionState::Selected => "selected",
        CharacterSelectionState::Blocked => "blocked",
        CharacterSelectionState::SelectionFailed => "selection_failed",
    }
}

fn freshness_label(refreshed: SystemTime, now: SystemTime) -> &'static str {
    match now.duration_since(refreshed) {
        Ok(age) if age <= std::time::Duration::from_secs(2) => "fresh",
        Ok(age) if age <= std::time::Duration::from_secs(30) => "stale",
        Ok(_) => "expired",
        Err(_) => "future",
    }
}

fn system_time_millis(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn record_player_changes(
    previous: &PlayerPreviewState,
    next: &PlayerPreviewState,
    timeline: &mut LivePreviewTimeline,
    timestamp_ms: u64,
    snapshot_sequence: u64,
) {
    let mut push = |summary: &str, detail: Option<String>| {
        timeline.push(LivePreviewTimelineEvent::new(
            LivePreviewTimelineType::Player,
            LivePreviewTimelineSeverity::Info,
            timestamp_ms,
            snapshot_sequence,
            summary,
            detail,
        ));
    };
    if previous.character_id != next.character_id {
        push(
            "player character changed",
            next.character_id
                .as_ref()
                .map(|character_id| format!("character={}", character_id.as_str())),
        );
    }
    if previous.attributes != next.attributes
        || previous.attributes_source != next.attributes_source
        || previous.attributes_push_sequence != next.attributes_push_sequence
        || previous.attributes_revision != next.attributes_revision
    {
        let detail = next.attributes_source.as_ref().map(|source| {
            format!(
                "source={source} sequence={} revision={}",
                next.attributes_push_sequence
                    .map_or_else(|| "none".to_owned(), |value| value.to_string()),
                next.attributes_revision
                    .map_or_else(|| "none".to_owned(), |value| value.to_string())
            )
        });
        push("player attributes changed", detail);
    }
    if previous.movement_state != next.movement_state {
        push(
            "player movement state changed",
            next.movement_state
                .as_ref()
                .map(|movement| format!("state={movement}")),
        );
    }
}

fn next_publish_sequence(current_sequence: u64) -> u64 {
    current_sequence.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::myserver::{AccountLoginState, CharacterProfile, ElementValues};
    use std::collections::HashMap;

    fn character_summary(
        character_id: &str,
        name: &str,
        world_id: Option<i64>,
        attributes: Option<CharacterAttributes>,
    ) -> CharacterSummary {
        CharacterSummary {
            character_id: character_id.to_owned(),
            character_id_short: None,
            display_discriminator: None,
            same_name_hint: None,
            name: name.to_owned(),
            world_id,
            status: None,
            appearance_json: None,
            created_at: None,
            last_login_at: None,
            deleted_at: None,
            position: None,
            attributes,
            lifecycle: None,
            extra: HashMap::new(),
        }
    }

    fn session() -> MyServerSession {
        MyServerSession {
            account_login_state: AccountLoginState::LoggedIn,
            character_selection_state: CharacterSelectionState::Selected,
            character_id: Some("character-1".to_owned()),
            player_id: Some("account-only".to_owned()),
            current_character: Some(character_summary("character-1", "Rin", Some(7), None)),
            ..Default::default()
        }
    }

    #[test]
    fn session_adapter_maps_identity_without_account_sensitive_fields() {
        let mut session = session();
        session.access_token = Some("access-secret".to_owned());
        session.ticket = Some("ticket-secret".to_owned());
        let state = MyServerPlayerPreviewAdapter {
            session: &session,
            entry: None,
            movement: None,
            local_player: None,
            now: UNIX_EPOCH,
        }
        .collect_player_preview();

        assert_eq!(
            state.character_id.as_ref().map(StablePreviewId::as_str),
            Some("character-1")
        );
        assert_eq!(state.display_name.as_deref(), Some("Rin"));
        assert_eq!(
            state.world_id.as_ref().map(StablePreviewId::as_str),
            Some("7")
        );
        assert_eq!(state.selection_state.as_deref(), Some("selected"));
        assert_eq!(state.movement_state.as_deref(), Some("not_applicable"));
        let serialized = serde_json::to_string(&state).unwrap();
        assert!(!serialized.contains("access-secret"));
        assert!(!serialized.contains("ticket-secret"));
        assert!(!serialized.contains("account-only"));
    }

    #[test]
    fn push_attributes_win_with_freshness_and_sequence_metadata() {
        let mut session = session();
        let refreshed = UNIX_EPOCH + std::time::Duration::from_secs(10);
        session.apply_character_elements_snapshot(
            "character-1".to_owned(),
            crate::game::myserver::CharacterElements {
                affinity: ElementValues {
                    earth: 1,
                    fire: 2,
                    water: 3,
                    wind: 4,
                },
                mastery: ElementValues::default(),
            },
            refreshed,
        );
        session.character_elements.last_push_sequence = Some(12);
        session.character_elements.last_push_revision = Some(4);
        let state = collect_player_preview_state(
            &session,
            None,
            None,
            None,
            UNIX_EPOCH + std::time::Duration::from_secs(11),
        );

        assert_eq!(state.attributes_source.as_deref(), Some("elements_push"));
        assert_eq!(state.attributes_push_sequence, Some(12));
        assert_eq!(state.attributes_revision, Some(4));
        assert_eq!(state.attributes_freshness.as_deref(), Some("fresh"));
        assert_eq!(state.attributes.as_ref().unwrap().affinity.fire, 2);
    }

    #[test]
    fn profile_attributes_are_fallback_when_elements_cache_is_missing() {
        let mut session = session();
        session.current_character = None;
        session.character_profile = Some(CharacterProfile {
            character: character_summary(
                "character-1",
                "Profile Name",
                None,
                Some(CharacterAttributes {
                    affinity: ElementValues {
                        water: 9,
                        ..Default::default()
                    },
                    mastery: ElementValues::default(),
                }),
            ),
            same_name: None,
            equipped_title: None,
            discipline: None,
            profile_sources: None,
        });
        let state = collect_player_preview_state(&session, None, None, None, UNIX_EPOCH);

        assert_eq!(state.attributes_source.as_deref(), Some("profile"));
        assert_eq!(state.attributes_freshness.as_deref(), Some("unknown"));
        assert_eq!(state.attributes.as_ref().unwrap().affinity.water, 9);
        assert_eq!(state.display_name.as_deref(), Some("Profile Name"));
    }

    #[test]
    fn active_local_gameplay_maps_position_direction_movement_and_authority_frame() {
        let session = session();
        let entry = MainWorldEntryState {
            phase: MainWorldEntryPhase::Active,
            input_frozen: false,
            ..Default::default()
        };
        let mut movement = MainWorldMovementRuntime::default();
        movement.input_frozen = false;
        movement.session_id = Some("scene-session".into());
        movement.predicted.position = Vec3::new(1.0, 2.0, 3.0);
        movement.predicted.direction = Vec2::new(0.0, 1.0);
        movement.predicted.moving = true;
        movement.authority_baseline = Some(
            crate::game::scenes::main_world_movement::MainWorldAuthorityBaseline {
                frame: crate::game::scenes::main_world_contract::MainWorldAuthorityFrame(41),
                ..Default::default()
            },
        );
        let player = MainWorldPlayer {
            character_id: "character-1".to_owned(),
            server_entity_id: 1,
            ownership: MainWorldPlayerOwnership::Local,
            scene_session_id: "scene-session".into(),
            last_authoritative_frame: 42,
        };
        let transform = Transform::from_xyz(4.0, 5.0, 6.0);
        let state = collect_player_preview_state(
            &session,
            Some(&entry),
            Some(&movement),
            Some((&player, &transform)),
            UNIX_EPOCH,
        );

        assert_eq!(state.position, Some([4.0, 5.0, 6.0]));
        assert_eq!(state.direction, Some([0.0, 0.0, 1.0]));
        assert_eq!(state.movement_state.as_deref(), Some("moving"));
        assert_eq!(state.authority_frame, Some(42));
    }

    #[test]
    fn player_timeline_ignores_continuous_position_but_records_identity_and_attributes() {
        let previous = PlayerPreviewState {
            character_id: Some(StablePreviewId::from("character-1")),
            position: Some([1.0, 0.0, 1.0]),
            ..Default::default()
        };
        let next = PlayerPreviewState {
            character_id: previous.character_id.clone(),
            position: Some([2.0, 0.0, 1.0]),
            ..Default::default()
        };
        let mut timeline = LivePreviewTimeline::default();
        record_player_changes(&previous, &next, &mut timeline, 1, 2);
        assert!(timeline.is_empty());

        let changed = PlayerPreviewState {
            character_id: Some(StablePreviewId::from("character-2")),
            ..next
        };
        record_player_changes(&previous, &changed, &mut timeline, 2, 3);
        assert!(
            timeline
                .iter()
                .any(|event| event.summary == "player character changed")
        );
    }
}
