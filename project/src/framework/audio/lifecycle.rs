use std::collections::HashMap;

use bevy::{prelude::*, window::AppLifecycle};

use super::{mixer::AudioMixer, scope::AudioBus};

pub const DEFAULT_BACKGROUND_PAUSED_BUSES: [AudioBus; 3] =
    [AudioBus::Music, AudioBus::Sfx, AudioBus::Battle];

#[derive(Clone, Debug, Resource)]
pub struct AudioLifecyclePausePolicy {
    pub pause_on_background: bool,
    pub paused_buses: Vec<AudioBus>,
}

impl Default for AudioLifecyclePausePolicy {
    fn default() -> Self {
        Self {
            pause_on_background: true,
            paused_buses: DEFAULT_BACKGROUND_PAUSED_BUSES.to_vec(),
        }
    }
}

impl AudioLifecyclePausePolicy {
    pub fn with_paused_buses(paused_buses: impl Into<Vec<AudioBus>>) -> Self {
        Self {
            paused_buses: paused_buses.into(),
            ..default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecyclePauseSnapshot {
    PausedByPolicy,
    AlreadyPaused,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct AudioLifecyclePauseState {
    pause_snapshot_by_bus: HashMap<AudioBus, LifecyclePauseSnapshot>,
}

impl AudioLifecyclePauseState {
    pub fn is_paused_by_policy(&self, bus: AudioBus) -> bool {
        self.pause_snapshot_by_bus
            .get(&bus)
            .is_some_and(|snapshot| *snapshot == LifecyclePauseSnapshot::PausedByPolicy)
    }
}

pub fn handle_audio_lifecycle_pause_policy(
    mut lifecycle_events: MessageReader<AppLifecycle>,
    policy: Res<AudioLifecyclePausePolicy>,
    mut lifecycle_state: ResMut<AudioLifecyclePauseState>,
    mut mixer: ResMut<AudioMixer>,
) {
    if !policy.pause_on_background {
        for _ in lifecycle_events.read() {}
        return;
    }

    for event in lifecycle_events.read() {
        match event {
            AppLifecycle::WillSuspend | AppLifecycle::Suspended => {
                apply_background_pause_policy(&policy, &mut lifecycle_state, &mut mixer);
            }
            AppLifecycle::WillResume | AppLifecycle::Running => {
                restore_background_pause_policy(&mut lifecycle_state, &mut mixer);
            }
            AppLifecycle::Idle => {}
        }
    }
}

fn apply_background_pause_policy(
    policy: &AudioLifecyclePausePolicy,
    lifecycle_state: &mut AudioLifecyclePauseState,
    mixer: &mut AudioMixer,
) {
    for bus in &policy.paused_buses {
        lifecycle_state
            .pause_snapshot_by_bus
            .entry(*bus)
            .or_insert_with(|| {
                if mixer.bus_state(*bus).paused {
                    LifecyclePauseSnapshot::AlreadyPaused
                } else {
                    mixer.set_bus_paused(*bus, true);
                    LifecyclePauseSnapshot::PausedByPolicy
                }
            });
    }
}

fn restore_background_pause_policy(
    lifecycle_state: &mut AudioLifecyclePauseState,
    mixer: &mut AudioMixer,
) {
    let paused_buses = lifecycle_state
        .pause_snapshot_by_bus
        .drain()
        .collect::<Vec<_>>();

    for (bus, snapshot) in paused_buses {
        if snapshot == LifecyclePauseSnapshot::PausedByPolicy {
            mixer.set_bus_paused(bus, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_pauses_mobile_background_buses_but_not_ui() {
        let policy = AudioLifecyclePausePolicy::default();

        assert_eq!(policy.paused_buses, DEFAULT_BACKGROUND_PAUSED_BUSES);
        assert!(policy.paused_buses.contains(&AudioBus::Music));
        assert!(policy.paused_buses.contains(&AudioBus::Sfx));
        assert!(policy.paused_buses.contains(&AudioBus::Battle));
        assert!(!policy.paused_buses.contains(&AudioBus::Ui));
    }

    #[test]
    fn lifecycle_policy_restores_only_buses_it_paused() {
        let mut mixer = AudioMixer::default();
        mixer.set_bus_paused(AudioBus::Battle, true);
        let policy = AudioLifecyclePausePolicy::default();
        let mut lifecycle_state = AudioLifecyclePauseState::default();

        apply_background_pause_policy(&policy, &mut lifecycle_state, &mut mixer);

        assert!(mixer.bus_state(AudioBus::Music).paused);
        assert!(mixer.bus_state(AudioBus::Sfx).paused);
        assert!(mixer.bus_state(AudioBus::Battle).paused);
        assert!(!mixer.bus_state(AudioBus::Ui).paused);
        assert!(lifecycle_state.is_paused_by_policy(AudioBus::Music));
        assert!(lifecycle_state.is_paused_by_policy(AudioBus::Sfx));
        assert!(!lifecycle_state.is_paused_by_policy(AudioBus::Battle));

        restore_background_pause_policy(&mut lifecycle_state, &mut mixer);

        assert!(!mixer.bus_state(AudioBus::Music).paused);
        assert!(!mixer.bus_state(AudioBus::Sfx).paused);
        assert!(mixer.bus_state(AudioBus::Battle).paused);
    }

    #[test]
    fn lifecycle_system_handles_suspend_and_resume_events() {
        let mut app = App::new();
        app.add_message::<AppLifecycle>()
            .init_resource::<AudioMixer>()
            .init_resource::<AudioLifecyclePausePolicy>()
            .init_resource::<AudioLifecyclePauseState>()
            .add_systems(Update, handle_audio_lifecycle_pause_policy);

        app.world_mut().write_message(AppLifecycle::WillSuspend);
        app.update();

        let mixer = app.world().resource::<AudioMixer>();
        assert!(mixer.bus_state(AudioBus::Music).paused);
        assert!(mixer.bus_state(AudioBus::Sfx).paused);
        assert!(mixer.bus_state(AudioBus::Battle).paused);

        app.world_mut().write_message(AppLifecycle::WillResume);
        app.update();

        let mixer = app.world().resource::<AudioMixer>();
        assert!(!mixer.bus_state(AudioBus::Music).paused);
        assert!(!mixer.bus_state(AudioBus::Sfx).paused);
        assert!(!mixer.bus_state(AudioBus::Battle).paused);
    }

    #[test]
    fn lifecycle_policy_can_be_disabled() {
        let mut app = App::new();
        app.add_message::<AppLifecycle>()
            .init_resource::<AudioMixer>()
            .insert_resource(AudioLifecyclePausePolicy {
                pause_on_background: false,
                paused_buses: vec![AudioBus::Music],
            })
            .init_resource::<AudioLifecyclePauseState>()
            .add_systems(Update, handle_audio_lifecycle_pause_policy);

        app.world_mut().write_message(AppLifecycle::Suspended);
        app.update();

        assert!(
            !app.world()
                .resource::<AudioMixer>()
                .bus_state(AudioBus::Music)
                .paused
        );
    }
}
