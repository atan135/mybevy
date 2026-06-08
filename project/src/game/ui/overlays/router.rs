use bevy::prelude::*;

use crate::game::{
    navigation::{AppUiMode, RouteButton},
    ui::overlays::{
        modal::{
            UiModal, UiModalResult, UiModalRoot, close_modals, handle_modal_action_buttons,
            spawn_modal,
        },
        toast::{UiToast, UiToastRoot, close_toasts, spawn_toast, tick_toasts},
    },
    ui::style::UiTheme,
};

pub(in crate::game) struct UiRouterPlugin;

impl Plugin for UiRouterPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiRouteCommand>()
            .add_message::<UiModalResult>()
            .add_systems(
                Update,
                (
                    handle_route_buttons,
                    handle_ui_route_commands,
                    handle_modal_action_buttons,
                    tick_toasts,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Debug, Message)]
#[allow(dead_code)]
pub(in crate::game) enum UiRouteCommand {
    ChangeMode(AppUiMode),
    OpenModal(UiModal),
    CloseModal,
    ShowToast(UiToast),
}

fn handle_route_buttons(
    mut route_commands: MessageWriter<UiRouteCommand>,
    buttons: Query<(&Interaction, &RouteButton), (Changed<Interaction>, With<Button>)>,
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
    mut route_commands: MessageReader<UiRouteCommand>,
    mut next_mode: ResMut<NextState<AppUiMode>>,
    modal_roots: Query<Entity, With<UiModalRoot>>,
    toast_roots: Query<Entity, With<UiToastRoot>>,
) {
    for command in route_commands.read() {
        match command {
            UiRouteCommand::ChangeMode(mode) => {
                close_modals(&mut commands, &modal_roots);
                next_mode.set(*mode);
            }
            UiRouteCommand::OpenModal(modal) => {
                close_modals(&mut commands, &modal_roots);
                spawn_modal(&mut commands, &theme, modal);
            }
            UiRouteCommand::CloseModal => {
                close_modals(&mut commands, &modal_roots);
            }
            UiRouteCommand::ShowToast(toast) => {
                close_toasts(&mut commands, &toast_roots);
                spawn_toast(&mut commands, &theme, toast);
            }
        }
    }
}
