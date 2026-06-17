use bevy::prelude::*;

use crate::framework::ui::{
    core::{UiAnimationSystems, UiFocusSystems, UiMetrics, UiViewport},
    overlays::{
        loading::sync_loading_entry_border_alpha,
        modal::{UiModalResult, handle_modal_action_buttons, sync_confirm_entry_visual_alpha},
        toast::{
            UiToast, UiToastRoot, close_toasts, spawn_toast, sync_toast_border_alpha, tick_toasts,
        },
    },
    style::{UiFontAssets, UiTheme},
};

pub(crate) struct UiOverlayPlugin;

impl Plugin for UiOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiOverlayCommand>()
            .add_message::<UiModalResult>()
            .configure_sets(
                Update,
                UiOverlaySystems::Commands.before(UiAnimationSystems::Tick),
            )
            .add_systems(
                Update,
                (
                    handle_ui_overlay_commands,
                    handle_modal_action_buttons,
                    tick_toasts,
                )
                    .in_set(UiOverlaySystems::Commands)
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    sync_toast_border_alpha,
                    sync_loading_entry_border_alpha,
                    sync_confirm_entry_visual_alpha,
                )
                    .after(UiAnimationSystems::Tick)
                    .after(UiFocusSystems::Visuals),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum UiOverlaySystems {
    Commands,
}

#[derive(Clone, Debug, Message)]
#[allow(dead_code)]
pub(crate) enum UiOverlayCommand {
    ShowToast(UiToast),
}

fn handle_ui_overlay_commands(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    mut overlay_commands: MessageReader<UiOverlayCommand>,
    toast_roots: Query<Entity, With<UiToastRoot>>,
) {
    for command in overlay_commands.read() {
        match command {
            UiOverlayCommand::ShowToast(toast) => {
                close_toasts(&mut commands, &toast_roots);
                spawn_toast(&mut commands, &theme, &metrics, &viewport, &fonts, toast);
            }
        }
    }
}
