//! Game-layer scene definitions and registration.
//!
//! Reusable scene framework code stays in `framework::scene`. Concrete game
//! scene ids, scene registration adapters, and scene-specific composition live
//! here as the project grows.

mod catalog;
mod sample_dungeon_room;

use crate::framework::audio::prelude::{
    AudioBus, AudioCatalog, AudioClipId, AudioCueClip, AudioCueEntry, AudioCueId, AudioCuePlayback,
    AudioCueRules, AudioScope, SceneAudioAdapterConfig, SceneAudioCue, SceneAudioEntry,
    SceneAudioPlayback,
};
use bevy::prelude::*;

pub(in crate::game) use sample_dungeon_room::SAMPLE_DUNGEON_ROOM_SCENE_ID;

const SAMPLE_DUNGEON_ROOM_AMBIENCE_CLIP_ID: &str = "ambience.sample_dungeon_room";
const SAMPLE_DUNGEON_ROOM_AMBIENCE_CUE_ID: &str = "scene.sample_dungeon_room.ambience";
const SAMPLE_DUNGEON_ROOM_AMBIENCE_PATH: &str = "audio/ambience/light_rain_loop.wav";

pub(in crate::game) struct GameScenesPlugin;

impl Plugin for GameScenesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            catalog::GameSceneCatalogPlugin,
            sample_dungeon_room::SampleDungeonRoomPlugin,
        ))
        .add_systems(Startup, register_game_scene_audio);
    }
}

fn register_game_scene_audio(
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

    scene_audio.register(
        SAMPLE_DUNGEON_ROOM_SCENE_ID,
        SceneAudioEntry::from_playback(SceneAudioPlayback::Cue(
            SceneAudioCue::ambience(cue_id)
                .with_volume(1.0)
                .with_fade_in_seconds(0.25),
        ))
        .with_exit_fade_out_seconds(0.2),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{
            audio::prelude::{AudioPlugin, SceneAudioAdapterConfig},
            scene::prelude::{SceneContentSource, SceneId, SceneKind, ScenePlugin, SceneRegistry},
        },
        game::navigation::GameRouteCommand,
    };
    use bevy::asset::AssetPlugin;

    #[test]
    fn scene_plugins_register_sample_dungeon_room_from_first_package_catalog() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .add_message::<GameRouteCommand>()
            .add_plugins(GameScenesPlugin);

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
    fn scene_plugins_register_sample_dungeon_room_audio_adapter() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AudioPlugin,
            ScenePlugin,
        ))
        .add_message::<GameRouteCommand>()
        .add_plugins(GameScenesPlugin);

        app.update();

        let scene_audio = app.world().resource::<SceneAudioAdapterConfig>();
        let entry = scene_audio
            .get(&SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID))
            .unwrap();
        assert_eq!(entry.on_enter.len(), 1);
        assert_eq!(entry.exit_fade_out_seconds, Some(0.2));
        assert!(matches!(
            &entry.on_enter[0],
            SceneAudioPlayback::Cue(cue)
                if cue.cue_id.as_str() == SAMPLE_DUNGEON_ROOM_AMBIENCE_CUE_ID
                    && cue.bus == Some(AudioBus::Sfx)
                    && cue.looped
        ));
    }
}
