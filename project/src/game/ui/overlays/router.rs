use bevy::prelude::*;

use crate::game::{
    navigation::{AppUiMode, RouteButton},
    ui::core::{UiAnimationSystems, UiFocusSystems, UiPanelCommand},
    ui::overlays::{
        loading::sync_loading_entry_border_alpha,
        modal::{UiModalResult, handle_modal_action_buttons, sync_confirm_entry_visual_alpha},
        toast::{
            UiToast, UiToastRoot, close_toasts, spawn_toast, sync_toast_border_alpha, tick_toasts,
        },
    },
    ui::style::{UiFontAssets, UiTheme},
    ui::widgets::{DisabledButton, LoadingButton},
};

pub(in crate::game) struct UiRouterPlugin;

impl Plugin for UiRouterPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiRouteCommand>()
            .add_message::<UiModalResult>()
            .configure_sets(
                Update,
                UiRouteSystems::Commands.before(UiAnimationSystems::Tick),
            )
            .add_systems(
                Update,
                (
                    handle_route_buttons,
                    handle_ui_route_commands,
                    handle_modal_action_buttons,
                    tick_toasts,
                )
                    .in_set(UiRouteSystems::Commands)
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
pub(in crate::game) enum UiRouteSystems {
    Commands,
}

#[derive(Clone, Debug, Message)]
#[allow(dead_code)]
pub(in crate::game) enum UiRouteCommand {
    ChangeMode(AppUiMode),
    ShowToast(UiToast),
}

fn handle_route_buttons(
    mut route_commands: MessageWriter<UiRouteCommand>,
    buttons: Query<
        (&Interaction, &RouteButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (interaction, route_button) in &buttons {
        if *interaction == Interaction::Pressed {
            route_commands.write(UiRouteCommand::ChangeMode(route_button.target));
        }
    }
}

fn handle_ui_route_commands(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    mut route_commands: MessageReader<UiRouteCommand>,
    mut next_mode: ResMut<NextState<AppUiMode>>,
    current_mode: Res<State<AppUiMode>>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    toast_roots: Query<Entity, With<UiToastRoot>>,
) {
    for command in route_commands.read() {
        match command {
            UiRouteCommand::ChangeMode(mode) => {
                panel_commands.write(UiPanelCommand::CloseAllForMode(*current_mode.get()));
                next_mode.set(*mode);
            }
            UiRouteCommand::ShowToast(toast) => {
                close_toasts(&mut commands, &toast_roots);
                spawn_toast(&mut commands, &theme, &fonts, toast);
            }
        }
    }
}
