use bevy::{input::keyboard::Key, prelude::*};

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiViewport},
        overlays::{
            loading::{UiLoading, spawn_loading},
            modal::{UiConfirmModal, spawn_confirm_modal},
            router::UiRouteSystems,
        },
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{screen_label, screen_title},
    },
};

pub(in crate::game) struct UiPanelPlugin;

impl Plugin for UiPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiPanelCommand>()
            .init_resource::<UiPanelStack>()
            .configure_sets(Update, UiPanelSystems::Commands)
            .add_systems(
                Update,
                (write_close_top_on_return_input, handle_panel_commands)
                    .chain()
                    .in_set(UiPanelSystems::Commands)
                    .after(UiRouteSystems::Commands),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiPanelSystems {
    Commands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[allow(dead_code)]
pub(in crate::game) enum UiPanelId {
    LoginPage,
    GameListPage,
    UiGalleryPage,
    GalleryFloating,
    TouchRippleHud,
    TouchRipplePause,
    TouchRippleSettings,
    GlobalLoading,
    ConfirmModal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[allow(dead_code)]
pub(in crate::game) enum UiPanelKind {
    Page,
    Hud,
    Floating,
    Modal,
    BlockingOverlay,
}

#[derive(Component)]
#[allow(dead_code)]
pub(in crate::game) struct UiPanelRoot {
    pub id: UiPanelId,
    pub kind: UiPanelKind,
    pub owner_mode: Option<AppUiMode>,
}

#[derive(Component)]
pub(in crate::game) struct UiBlockingOverlay {
    pub cancellable: bool,
}

#[derive(Clone, Debug)]
pub(in crate::game) enum UiPanelRequest {
    Loading(UiLoading),
    Confirm(UiConfirmModal),
    Floating(UiFloatingPanel),
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiFloatingPanel {
    pub id: UiPanelId,
    pub title: String,
    pub body: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Message)]
#[allow(dead_code)]
pub(in crate::game) enum UiPanelCommand {
    Open(UiPanelRequest),
    Close(UiPanelId),
    Toggle(UiPanelRequest),
    Hide(UiPanelId),
    Show(UiPanelId),
    CloseTop,
    CloseAllForMode(AppUiMode),
}

#[derive(Clone, Copy, Debug)]
struct UiPanelStackEntry {
    id: UiPanelId,
    kind: UiPanelKind,
}

#[derive(Default, Resource)]
struct UiPanelStack {
    open_order: Vec<UiPanelStackEntry>,
}

fn handle_panel_commands(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    current_mode: Res<State<AppUiMode>>,
    mut panel_commands: MessageReader<UiPanelCommand>,
    panel_roots: Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    mut visible_panels: Query<(&UiPanelRoot, &mut Visibility)>,
    mut stack: ResMut<UiPanelStack>,
) {
    for command in panel_commands.read() {
        match command {
            UiPanelCommand::Open(request) => {
                open_panel(
                    &mut commands,
                    &theme,
                    &metrics,
                    &viewport,
                    &fonts,
                    &current_mode,
                    &panel_roots,
                    &mut stack,
                    request,
                );
            }
            UiPanelCommand::Close(id) => {
                close_panel_by_id(&mut commands, &panel_roots, *id);
                remove_from_stack(&mut stack, *id);
            }
            UiPanelCommand::Toggle(request) => {
                let id = request.id();
                if panel_exists(&panel_roots, id) {
                    close_panel_by_id(&mut commands, &panel_roots, id);
                    remove_from_stack(&mut stack, id);
                } else {
                    open_panel(
                        &mut commands,
                        &theme,
                        &metrics,
                        &viewport,
                        &fonts,
                        &current_mode,
                        &panel_roots,
                        &mut stack,
                        request,
                    );
                }
            }
            UiPanelCommand::Hide(id) => {
                set_panel_visibility(&mut visible_panels, *id, Visibility::Hidden);
            }
            UiPanelCommand::Show(id) => {
                set_panel_visibility(&mut visible_panels, *id, Visibility::Visible);
            }
            UiPanelCommand::CloseTop => {
                close_top_panel(&mut commands, &panel_roots, &mut stack);
            }
            UiPanelCommand::CloseAllForMode(mode) => {
                close_panels_for_mode(&mut commands, &panel_roots, *mode);
                stack.open_order.retain(|entry| {
                    !panel_roots.iter().any(|(_, panel, _)| {
                        panel.id == entry.id && panel.owner_mode == Some(*mode)
                    })
                });
            }
        }
    }
}

fn open_panel(
    commands: &mut Commands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    current_mode: &State<AppUiMode>,
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    stack: &mut UiPanelStack,
    request: &UiPanelRequest,
) {
    let id = request.id();
    let kind = request.kind();
    close_panel_by_id(commands, panel_roots, id);
    remove_from_stack(stack, id);

    match request {
        UiPanelRequest::Loading(loading) => {
            spawn_loading(
                commands,
                theme,
                metrics,
                fonts,
                loading,
                Some(*current_mode.get()),
            );
        }
        UiPanelRequest::Confirm(confirm) => {
            spawn_confirm_modal(
                commands,
                theme,
                metrics,
                viewport,
                fonts,
                confirm,
                Some(*current_mode.get()),
            );
        }
        UiPanelRequest::Floating(floating) => {
            spawn_floating_panel(commands, theme, fonts, floating, Some(*current_mode.get()));
        }
    }

    if matches!(
        kind,
        UiPanelKind::Floating | UiPanelKind::Modal | UiPanelKind::BlockingOverlay
    ) {
        stack.open_order.push(UiPanelStackEntry { id, kind });
    }
}

fn close_panel_by_id(
    commands: &mut Commands,
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    id: UiPanelId,
) -> bool {
    let mut closed = false;
    for (entity, panel, _) in panel_roots {
        if panel.id == id {
            commands.entity(entity).try_despawn();
            closed = true;
        }
    }
    closed
}

fn close_panels_for_mode(
    commands: &mut Commands,
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    mode: AppUiMode,
) {
    for (entity, panel, _) in panel_roots {
        if panel.owner_mode == Some(mode) {
            commands.entity(entity).try_despawn();
        }
    }
}

fn close_top_panel(
    commands: &mut Commands,
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    stack: &mut UiPanelStack,
) {
    prune_missing_stack_entries(panel_roots, stack);

    if let Some(entry) = latest_stack_entry_of_kind(stack, UiPanelKind::BlockingOverlay) {
        if is_cancellable_blocking_overlay(panel_roots, entry.id) {
            close_panel_by_id(commands, panel_roots, entry.id);
            remove_from_stack(stack, entry.id);
        }
        return;
    }

    if close_latest_panel_of_kind(commands, panel_roots, stack, UiPanelKind::Modal) {
        return;
    }

    close_latest_panel_of_kind(commands, panel_roots, stack, UiPanelKind::Floating);
}

fn panel_exists(
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    id: UiPanelId,
) -> bool {
    panel_roots.iter().any(|(_, panel, _)| panel.id == id)
}

fn remove_from_stack(stack: &mut UiPanelStack, id: UiPanelId) {
    stack.open_order.retain(|entry| entry.id != id);
}

fn prune_missing_stack_entries(
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    stack: &mut UiPanelStack,
) {
    stack
        .open_order
        .retain(|entry| panel_exists(panel_roots, entry.id));
}

fn latest_stack_entry_of_kind(
    stack: &UiPanelStack,
    kind: UiPanelKind,
) -> Option<UiPanelStackEntry> {
    stack
        .open_order
        .iter()
        .rfind(|entry| entry.kind == kind)
        .copied()
}

fn close_latest_panel_of_kind(
    commands: &mut Commands,
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    stack: &mut UiPanelStack,
    kind: UiPanelKind,
) -> bool {
    if let Some(entry) = latest_stack_entry_of_kind(stack, kind) {
        close_panel_by_id(commands, panel_roots, entry.id);
        remove_from_stack(stack, entry.id);
        return true;
    }

    false
}

fn is_cancellable_blocking_overlay(
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    id: UiPanelId,
) -> bool {
    panel_roots
        .iter()
        .find(|(_, panel, _)| panel.id == id && panel.kind == UiPanelKind::BlockingOverlay)
        .and_then(|(_, _, overlay)| overlay)
        .is_some_and(|overlay| overlay.cancellable)
}

fn set_panel_visibility(
    visible_panels: &mut Query<(&UiPanelRoot, &mut Visibility)>,
    id: UiPanelId,
    visibility: Visibility,
) {
    for (panel, mut panel_visibility) in visible_panels {
        if panel.id == id {
            *panel_visibility = visibility;
        }
    }
}

impl UiPanelRequest {
    fn id(&self) -> UiPanelId {
        match self {
            UiPanelRequest::Loading(_) => UiPanelId::GlobalLoading,
            UiPanelRequest::Confirm(_) => UiPanelId::ConfirmModal,
            UiPanelRequest::Floating(floating) => floating.id,
        }
    }

    fn kind(&self) -> UiPanelKind {
        match self {
            UiPanelRequest::Loading(_) => UiPanelKind::BlockingOverlay,
            UiPanelRequest::Confirm(_) => UiPanelKind::Modal,
            UiPanelRequest::Floating(_) => UiPanelKind::Floating,
        }
    }
}

fn write_close_top_on_return_input(
    key_codes: Res<ButtonInput<KeyCode>>,
    keys: Res<ButtonInput<Key>>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
) {
    if key_codes.just_pressed(KeyCode::Escape) || keys.just_pressed(Key::BrowserBack) {
        panel_commands.write(UiPanelCommand::CloseTop);
    }
}

fn spawn_floating_panel(
    commands: &mut Commands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    floating: &UiFloatingPanel,
    owner_mode: Option<AppUiMode>,
) {
    commands
        .spawn((
            UiPanelRoot {
                id: floating.id,
                kind: UiPanelKind::Floating,
                owner_mode,
            },
            UiLayerRoot {
                layer: UiLayer::Floating,
            },
            Button,
            UiThemeRootNodeRole::FloatingPanel,
            UiThemePanelNodeRole::Standard,
            Node {
                position_type: PositionType::Absolute,
                right: px(theme.layout.screen_padding),
                top: px(96),
                width: px(340),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.layout.card_gap),
                padding: UiRect::all(px(theme.panel.padding)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.panel.radius)),
                ..default()
            },
            ZIndex(80),
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(theme.colors.panel_border),
            UiThemeBackgroundRole::Panel,
            UiThemeBorderRole::Panel,
        ))
        .with_children(|panel| {
            panel.spawn(screen_title(
                theme,
                fonts,
                floating.title.clone(),
                UiThemeTextStyleRole::Subtitle,
            ));
            panel.spawn(screen_label(
                theme,
                fonts,
                floating.body.clone(),
                UiThemeTextStyleRole::Body,
                UiThemeTextColorRole::Primary,
            ));

            if let Some(detail) = &floating.detail {
                panel.spawn(screen_label(
                    theme,
                    fonts,
                    detail.clone(),
                    UiThemeTextStyleRole::Caption,
                    UiThemeTextColorRole::Muted,
                ));
            }
        });
}
