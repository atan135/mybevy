use std::fmt;

use bevy::{input::keyboard::Key, prelude::*};

use crate::game::{
    navigation::GameRouteSystems,
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiViewport, UiWidthClass},
        overlays::{
            loading::{UiLoading, spawn_loading},
            modal::{UiConfirmModal, spawn_confirm_modal},
        },
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemeRootNodeRole,
                UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{screen_label, screen_title},
    },
};

pub(in crate::game) struct UiPanelPlugin;

impl Plugin for UiPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiPanelCommand>()
            .init_resource::<UiCurrentOwner>()
            .init_resource::<UiPanelStack>()
            .configure_sets(Update, UiPanelSystems::Commands)
            .add_systems(
                Update,
                (write_close_top_on_return_input, handle_panel_commands)
                    .chain()
                    .in_set(UiPanelSystems::Commands)
                    .after(GameRouteSystems::Commands),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiPanelSystems {
    Commands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) struct UiPanelId(&'static str);

impl UiPanelId {
    pub(in crate::game) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(in crate::game) const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for UiPanelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) struct UiOwnerId(&'static str);

impl UiOwnerId {
    pub(in crate::game) const fn new(value: &'static str) -> Self {
        Self(value)
    }
}

impl fmt::Display for UiOwnerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

pub(in crate::game) const UI_PANEL_GLOBAL_LOADING: UiPanelId = UiPanelId::new("global_loading");
pub(in crate::game) const UI_PANEL_CONFIRM_MODAL: UiPanelId = UiPanelId::new("confirm_modal");

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
    pub owner: Option<UiOwnerId>,
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
    CloseAllForOwner(UiOwnerId),
}

#[derive(Clone, Copy, Debug, Default, Resource)]
pub(in crate::game) struct UiCurrentOwner {
    pub owner: Option<UiOwnerId>,
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
    current_owner: Res<UiCurrentOwner>,
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
                    current_owner.owner,
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
                        current_owner.owner,
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
            UiPanelCommand::CloseAllForOwner(owner) => {
                close_panels_for_owner(&mut commands, &panel_roots, *owner);
                stack.open_order.retain(|entry| {
                    !panel_roots
                        .iter()
                        .any(|(_, panel, _)| panel.id == entry.id && panel.owner == Some(*owner))
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
    owner: Option<UiOwnerId>,
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
            spawn_loading(commands, theme, metrics, viewport, fonts, loading, owner);
        }
        UiPanelRequest::Confirm(confirm) => {
            spawn_confirm_modal(commands, theme, metrics, viewport, fonts, confirm, owner);
        }
        UiPanelRequest::Floating(floating) => {
            spawn_floating_panel(commands, theme, metrics, viewport, fonts, floating, owner);
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

fn close_panels_for_owner(
    commands: &mut Commands,
    panel_roots: &Query<(Entity, &UiPanelRoot, Option<&UiBlockingOverlay>)>,
    owner: UiOwnerId,
) {
    for (entity, panel, _) in panel_roots {
        if panel.owner == Some(owner) {
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
            UiPanelRequest::Loading(_) => UI_PANEL_GLOBAL_LOADING,
            UiPanelRequest::Confirm(_) => UI_PANEL_CONFIRM_MODAL,
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
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    floating: &UiFloatingPanel,
    owner: Option<UiOwnerId>,
) {
    commands
        .spawn((
            UiPanelRoot {
                id: floating.id,
                kind: UiPanelKind::Floating,
                owner,
            },
            UiLayerRoot {
                layer: UiLayer::Floating,
            },
            Button,
            UiThemeRootNodeRole::FloatingPanel,
            floating_panel_node(theme, metrics, viewport),
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

fn floating_panel_node(theme: &UiTheme, metrics: &UiMetrics, viewport: &UiViewport) -> Node {
    Node {
        position_type: PositionType::Absolute,
        right: px(metrics.page_padding + viewport.safe_area.right),
        top: px(floating_panel_top(metrics, viewport)),
        width: px(floating_panel_width(metrics, viewport)),
        max_width: percent(94),
        max_height: percent(floating_panel_max_height_percent(viewport)),
        flex_direction: FlexDirection::Column,
        row_gap: px(metrics.control_gap),
        padding: UiRect::all(px(metrics.panel_padding)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

fn floating_panel_width(metrics: &UiMetrics, viewport: &UiViewport) -> f32 {
    let safe_horizontal = viewport.safe_area.left + viewport.safe_area.right;
    let available =
        (viewport.logical_width - safe_horizontal - metrics.page_padding * 2.0).max(1.0);
    metrics.dialog_max_width.min(420.0).min(available)
}

fn floating_panel_top(metrics: &UiMetrics, viewport: &UiViewport) -> f32 {
    metrics.page_padding + viewport.safe_area.top + metrics.touch_target_min
}

fn floating_panel_max_height_percent(viewport: &UiViewport) -> f32 {
    match viewport.width_class {
        UiWidthClass::Compact => 72.0,
        UiWidthClass::Medium | UiWidthClass::Expanded => 82.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_panel_width_uses_metrics_dialog_width() {
        let theme = UiTheme::default();
        let viewport = UiViewport::default();
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &theme);
        let node = floating_panel_node(&theme, &metrics, &viewport);

        assert_eq!(node.width, px(floating_panel_width(&metrics, &viewport)));
        assert_eq!(node.width, px(metrics.dialog_max_width.min(420.0)));
    }

    #[test]
    fn floating_panel_position_uses_metrics_and_safe_area() {
        let theme = UiTheme::default();
        let viewport = UiViewport {
            safe_area: crate::game::ui::core::UiSafeArea {
                top: 7.0,
                right: 11.0,
                ..default()
            },
            ..default()
        };
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &theme);
        let node = floating_panel_node(&theme, &metrics, &viewport);

        assert_eq!(node.right, px(metrics.page_padding + 11.0));
        assert_eq!(
            node.top,
            px(metrics.page_padding + 7.0 + metrics.touch_target_min)
        );
    }
}
