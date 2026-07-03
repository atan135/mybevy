//! Game-layer scene definitions and registration.
//!
//! Reusable scene framework code stays in `framework::scene`. Concrete game
//! scene ids, scene registration adapters, and scene-specific composition live
//! here as the project grows.

mod catalog;
mod fangyuan_home;
mod robot_sync_arena;
mod sample_dungeon_room;

use crate::framework::audio::prelude::{
    AudioBus, AudioCatalog, AudioClipId, AudioCueClip, AudioCueEntry, AudioCueId, AudioCuePlayback,
    AudioCueRules, AudioScope, SceneAudioAdapterConfig, SceneAudioCue, SceneAudioEntry,
    SceneAudioMusic, SceneAudioPlayback,
};
use crate::framework::scene::prelude::SceneId;
use bevy::prelude::*;
use catalog::GameSceneCatalog;

pub(in crate::game) use fangyuan_home::FANGYUAN_HOME_SCENE_ID;
#[cfg(test)]
pub(in crate::game) use fangyuan_home::FangyuanHomeBlueprintRenderSummary;
pub(in crate::game) use fangyuan_home::{FangyuanHomeBlueprintCommand, FangyuanHomeBlueprintStats};
pub(in crate::game) use robot_sync_arena::ROBOT_SYNC_ARENA_SCENE_ID;
pub(in crate::game) use sample_dungeon_room::SAMPLE_DUNGEON_ROOM_SCENE_ID;

const SAMPLE_DUNGEON_ROOM_AMBIENCE_CLIP_ID: &str = "ambience.sample_dungeon_room";
const SAMPLE_DUNGEON_ROOM_AMBIENCE_CUE_ID: &str = "scene.sample_dungeon_room.ambience";
const SAMPLE_DUNGEON_ROOM_AMBIENCE_PATH: &str = "audio/ambience/light_rain_loop.wav";

pub(in crate::game) struct GameScenesPlugin;

impl Plugin for GameScenesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            catalog::GameSceneCatalogPlugin,
            fangyuan_home::FangyuanHomePlugin,
            robot_sync_arena::RobotSyncArenaPlugin,
            sample_dungeon_room::SampleDungeonRoomPlugin,
        ))
        .add_systems(
            Startup,
            register_game_scene_audio.after(catalog::GameSceneCatalogStartupSet),
        );
    }
}

fn register_game_scene_audio(
    game_scenes: Res<GameSceneCatalog>,
    catalog: Option<ResMut<AudioCatalog>>,
    scene_audio: Option<ResMut<SceneAudioAdapterConfig>>,
) {
    let (Some(mut catalog), Some(mut scene_audio)) = (catalog, scene_audio) else {
        return;
    };

    let clip_id = AudioClipId::try_from(SAMPLE_DUNGEON_ROOM_AMBIENCE_CLIP_ID)
        .expect("sample scene ambience clip id must be valid");
    let cue_id = AudioCueId::try_from(SAMPLE_DUNGEON_ROOM_AMBIENCE_CUE_ID)
        .expect("sample scene ambience cue id must be valid");

    catalog.register_clip(clip_id.clone(), SAMPLE_DUNGEON_ROOM_AMBIENCE_PATH);
    catalog.register_cue(
        cue_id.clone(),
        AudioCueEntry::from_clips([AudioCueClip::new(clip_id)])
            .with_playback(AudioCuePlayback {
                bus: AudioBus::Sfx,
                scope: AudioScope::Global,
                looped: true,
            })
            .with_rules(AudioCueRules {
                volume: 0.45,
                pitch: 1.0,
                cooldown_seconds: None,
                max_concurrent: Some(1),
                priority: 0,
            }),
    );

    let sample_entry = SceneAudioEntry::from_playback(SceneAudioPlayback::Cue(
        SceneAudioCue::ambience(cue_id)
            .with_volume(1.0)
            .with_fade_in_seconds(0.25),
    ))
    .with_exit_fade_out_seconds(0.2);

    scene_audio.register(
        SAMPLE_DUNGEON_ROOM_SCENE_ID,
        scene_entry_with_catalog_music(
            sample_entry,
            game_scenes.find_enabled(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID)),
            &mut catalog,
        ),
    );

    for entry in game_scenes.enabled_entries() {
        if entry.scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID {
            continue;
        }

        if entry.music.is_some() {
            scene_audio.register(
                entry.scene_id.clone(),
                scene_entry_with_catalog_music(
                    SceneAudioEntry {
                        on_enter: Vec::new(),
                        exit_fade_out_seconds: None,
                    },
                    Some(entry),
                    &mut catalog,
                ),
            );
        }
    }
}

fn scene_entry_with_catalog_music(
    mut scene_entry: SceneAudioEntry,
    game_entry: Option<&catalog::GameSceneEntry>,
    catalog: &mut AudioCatalog,
) -> SceneAudioEntry {
    let Some(music) = game_entry.and_then(|entry| entry.music.as_ref()) else {
        return scene_entry;
    };

    catalog.register_clip(music.clip_id.clone(), music.path.clone());

    let mut scene_music = SceneAudioMusic::new(music.clip_id.clone());
    if let Some(volume) = music.volume {
        scene_music = scene_music.with_volume(volume);
    }
    if let Some(fade_in_seconds) = music.fade_in_seconds {
        scene_music = scene_music.with_fade_in_seconds(fade_in_seconds);
    }

    scene_entry
        .on_enter
        .push(SceneAudioPlayback::Music(scene_music));
    if let Some(exit_fade_out_seconds) = music.exit_fade_out_seconds {
        scene_entry.exit_fade_out_seconds = Some(exit_fade_out_seconds);
    }

    scene_entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{
            audio::prelude::{AudioPlugin, SceneAudioAdapterConfig},
            scene::prelude::{
                SceneContentSource, SceneDebugConfig, SceneId, SceneKind, ScenePlugin,
                SceneRegistry,
            },
        },
        game::navigation::GameRouteCommand,
    };
    use bevy::asset::AssetPlugin;

    fn app_with_scene_registration_plugins() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .add_message::<GameRouteCommand>()
            .add_plugins(GameScenesPlugin);
        app.insert_resource(SceneDebugConfig::default());
        app
    }

    fn app_with_scene_audio_registration_plugins() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AudioPlugin,
            ScenePlugin,
        ))
        .add_message::<GameRouteCommand>()
        .add_plugins(GameScenesPlugin);
        app.insert_resource(SceneDebugConfig::default());
        app
    }

    #[test]
    fn scene_plugins_register_sample_dungeon_room_from_first_package_catalog() {
        let mut app = app_with_scene_registration_plugins();

        app.update();

        let registry = app.world().resource::<SceneRegistry>();
        let scene_id = SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID);
        let definition = registry.get(&scene_id).unwrap();

        assert_eq!(definition.scene_id, scene_id);
        assert_eq!(definition.kind, SceneKind::Dungeon);
        assert!(definition.has_world_root);
        assert_eq!(
            definition.manifest_path.as_deref(),
            Some("scenes/sample_dungeon_room/scene.ron")
        );
        assert_eq!(
            definition.content_source,
            SceneContentSource::FirstPackage {
                manifest_path: "scenes/sample_dungeon_room/scene.ron".to_string()
            }
        );
    }

    #[test]
    fn scene_plugins_register_robot_sync_arena_from_first_package_catalog() {
        let mut app = app_with_scene_registration_plugins();

        app.update();

        let registry = app.world().resource::<SceneRegistry>();
        let scene_id = SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID);
        let definition = registry.get(&scene_id).unwrap();

        assert_eq!(definition.scene_id, scene_id);
        assert_eq!(definition.kind, SceneKind::Arena);
        assert!(definition.has_world_root);
        assert_eq!(
            definition.manifest_path.as_deref(),
            Some("scenes/robot_sync_arena/scene.ron")
        );
        assert_eq!(
            definition.content_source,
            SceneContentSource::FirstPackage {
                manifest_path: "scenes/robot_sync_arena/scene.ron".to_string()
            }
        );

        assert!(registry.contains(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID)));
    }

    #[test]
    fn scene_plugins_register_fangyuan_home_from_first_package_catalog() {
        let mut app = app_with_scene_registration_plugins();

        app.update();

        let registry = app.world().resource::<SceneRegistry>();
        let scene_id = SceneId::from(FANGYUAN_HOME_SCENE_ID);
        let definition = registry.get(&scene_id).unwrap();

        assert_eq!(definition.scene_id, scene_id);
        assert_eq!(definition.kind, SceneKind::World);
        assert!(definition.has_world_root);
        assert_eq!(
            definition.manifest_path.as_deref(),
            Some("scenes/fangyuan_home/scene.ron")
        );
        assert_eq!(
            definition.content_source,
            SceneContentSource::FirstPackage {
                manifest_path: "scenes/fangyuan_home/scene.ron".to_string()
            }
        );
    }

    #[test]
    fn scene_plugins_register_sample_dungeon_room_audio_adapter() {
        let mut app = app_with_scene_audio_registration_plugins();

        app.update();

        let scene_audio = app.world().resource::<SceneAudioAdapterConfig>();
        let entry = scene_audio
            .get(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID))
            .unwrap();
        assert_eq!(entry.on_enter.len(), 2);
        assert_eq!(entry.exit_fade_out_seconds, Some(0.4));
        assert!(matches!(
            &entry.on_enter[0],
            SceneAudioPlayback::Cue(cue)
                if cue.cue_id.as_str() == SAMPLE_DUNGEON_ROOM_AMBIENCE_CUE_ID
                    && cue.bus == Some(AudioBus::Sfx)
                    && cue.looped
        ));
        assert!(matches!(
            &entry.on_enter[1],
            SceneAudioPlayback::Music(music)
                if music.clip_id.as_str() == "music.sample_dungeon_room"
                    && music.volume == 0.35
                    && music.looped
                    && music.fade_in_seconds == Some(0.5)
        ));
    }

    #[test]
    fn scene_plugins_register_sample_dungeon_room_music_clip_from_catalog() {
        let mut app = app_with_scene_audio_registration_plugins();

        app.update();

        let audio_catalog = app.world().resource::<AudioCatalog>();
        let music_clip = AudioClipId::try_from("music.sample_dungeon_room").unwrap();
        assert_eq!(
            audio_catalog.clip(&music_clip).unwrap().path,
            "audio/music/stealth_bass_loop.wav"
        );
    }
}
