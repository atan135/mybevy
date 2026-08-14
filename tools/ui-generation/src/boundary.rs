use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use syn::{Expr, Item, visit::Visit};
use toml::Value;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyBoundaryReport {
    pub project_manifest: PathBuf,
    pub tool_manifest: PathBuf,
    pub project_dependency_graph_excludes_tool: bool,
    pub tool_dependency_graph_excludes_project: bool,
    pub project_lock_excludes_tool_package: bool,
    pub tool_lock_excludes_project_package: bool,
    pub crates_are_independent_workspaces: bool,
    pub standalone_preview_target_is_feature_gated: bool,
    pub ui_only_generation_write_scope_is_closed: bool,
    pub formal_business_routes_have_approved_documents: bool,
    pub all_routable_screens_are_classified: bool,
    pub direct_rust_ui_views_match_controlled_exceptions: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiOnlyChangeManifest {
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiOnlyChangeBoundaryReport {
    pub allowed_paths: Vec<String>,
    pub blocked_paths: Vec<String>,
}

pub fn verify_dependency_boundary(
    repository_root: &Path,
) -> Result<DependencyBoundaryReport, TaskFailure> {
    let repository_root = fs::canonicalize(repository_root).map_err(|error| {
        boundary_failure(format!("repository root cannot be resolved: {error}"))
    })?;
    let project_manifest = canonical_manifest(&repository_root.join("project/Cargo.toml"))?;
    let tool_manifest =
        canonical_manifest(&repository_root.join("tools/ui-generation/Cargo.toml"))?;

    let project_dependency_graph_excludes_tool =
        !manifest_graph_reaches(&project_manifest, &tool_manifest)?;
    let tool_dependency_graph_excludes_project =
        !manifest_graph_reaches(&tool_manifest, &project_manifest)?;
    let project_lock_excludes_tool_package = !lock_contains_local_package(
        &project_manifest.with_file_name("Cargo.lock"),
        "ui-generation",
    )?;
    let tool_lock_excludes_project_package =
        !lock_contains_local_package(&tool_manifest.with_file_name("Cargo.lock"), "project")?;
    let project_workspace = enclosing_workspace_root(&project_manifest)?;
    let tool_workspace = enclosing_workspace_root(&tool_manifest)?;
    let crates_are_independent_workspaces = project_workspace != tool_workspace;
    let standalone_preview_target_is_feature_gated =
        preview_target_is_feature_gated(&parse_toml_file(&project_manifest)?);
    let ui_only_generation_write_scope_is_closed = ui_only_generation_write_scope_is_closed();
    let formal_screen_boundary = verify_formal_screen_boundary(&repository_root)?;

    validate_boundary_flags(
        project_dependency_graph_excludes_tool,
        tool_dependency_graph_excludes_project,
        project_lock_excludes_tool_package,
        tool_lock_excludes_project_package,
        crates_are_independent_workspaces,
        standalone_preview_target_is_feature_gated,
        ui_only_generation_write_scope_is_closed,
        formal_screen_boundary.formal_business_routes_have_approved_documents,
        formal_screen_boundary.all_routable_screens_are_classified,
        formal_screen_boundary.direct_rust_ui_views_match_controlled_exceptions,
    )?;

    Ok(DependencyBoundaryReport {
        project_manifest,
        tool_manifest,
        project_dependency_graph_excludes_tool,
        tool_dependency_graph_excludes_project,
        project_lock_excludes_tool_package,
        tool_lock_excludes_project_package,
        crates_are_independent_workspaces,
        standalone_preview_target_is_feature_gated,
        ui_only_generation_write_scope_is_closed,
        formal_business_routes_have_approved_documents: formal_screen_boundary
            .formal_business_routes_have_approved_documents,
        all_routable_screens_are_classified: formal_screen_boundary
            .all_routable_screens_are_classified,
        direct_rust_ui_views_match_controlled_exceptions: formal_screen_boundary
            .direct_rust_ui_views_match_controlled_exceptions,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FormalScreenBoundaryReport {
    formal_business_routes_have_approved_documents: bool,
    all_routable_screens_are_classified: bool,
    direct_rust_ui_views_match_controlled_exceptions: bool,
}

const FORMAL_BUSINESS_DOCUMENTS: [(&str, &str, &str, &str); 10] = [
    (
        "Login",
        "auth.login",
        "project/assets/ui/documents/approved/auth/login.v1.json",
        "project/assets/ui/documents/approved/auth/promotion.v1.json",
    ),
    (
        "CharacterSelect",
        "auth.character_select",
        "project/assets/ui/documents/approved/auth/character_select.v1.json",
        "project/assets/ui/documents/approved/auth/character_select.promotion.v1.json",
    ),
    (
        "Lobby",
        "game.lobby",
        "project/assets/ui/documents/approved/lobby/lobby.v1.json",
        "project/assets/ui/documents/approved/lobby/lobby.promotion.v1.json",
    ),
    (
        "AudioSettings",
        "game.audio_settings",
        "project/assets/ui/documents/approved/audio_settings/audio_settings.v1.json",
        "project/assets/ui/documents/approved/audio_settings/audio_settings.promotion.v1.json",
    ),
    (
        "MainWorld",
        "game.main_world_hud",
        "project/assets/ui/documents/approved/gameplay/main_world_hud.v1.json",
        "project/assets/ui/documents/approved/gameplay/main_world_hud.promotion.v1.json",
    ),
    (
        "WanfaTouchRipple",
        "game.touch_ripple_hud",
        "project/assets/ui/documents/approved/gameplay/touch_ripple_hud.v1.json",
        "project/assets/ui/documents/approved/gameplay/touch_ripple_hud.promotion.v1.json",
    ),
    (
        "SampleScene",
        "game.sample_scene_hud",
        "project/assets/ui/documents/approved/gameplay/sample_scene_hud.v1.json",
        "project/assets/ui/documents/approved/gameplay/sample_scene_hud.promotion.v1.json",
    ),
    (
        "RobotSyncScene",
        "game.robot_sync_hud",
        "project/assets/ui/documents/approved/gameplay/robot_sync_hud.v1.json",
        "project/assets/ui/documents/approved/gameplay/robot_sync_hud.promotion.v1.json",
    ),
    (
        "FangyuanHome",
        "game.fangyuan_home_hud",
        "project/assets/ui/documents/approved/gameplay/fangyuan_home_hud.v1.json",
        "project/assets/ui/documents/approved/gameplay/fangyuan_home_hud.promotion.v1.json",
    ),
    (
        "FangyuanPlayerPreview",
        "game.fangyuan_player_preview_hud",
        "project/assets/ui/documents/approved/gameplay/fangyuan_player_preview_hud.v1.json",
        "project/assets/ui/documents/approved/gameplay/fangyuan_player_preview_hud.promotion.v1.json",
    ),
];

const CONTROLLED_RUST_VIEW_EXCEPTIONS: [(&str, &str); 4] = [
    (
        "AiLoginReference",
        "project/src/game/screens/dev/ai_login_reference.rs",
    ),
    (
        "AudioGallery",
        "project/src/game/screens/dev/audio_gallery.rs",
    ),
    (
        "AudioMonitor",
        "project/src/game/screens/dev/audio_monitor.rs",
    ),
    ("UiGallery", "project/src/game/screens/dev/ui_gallery.rs"),
];

const DECLARATIVE_DEV_SCREEN_VARIANTS: [&str; 2] = ["UiDocumentGallery", "UiGeneratedAcceptance"];

#[derive(Debug, Eq, PartialEq)]
struct RouteClassificationCheck {
    actual: BTreeSet<String>,
    classified: BTreeSet<String>,
    missing: BTreeSet<String>,
    stale: BTreeSet<String>,
    duplicates: BTreeSet<String>,
}

impl RouteClassificationCheck {
    fn exact(&self) -> bool {
        self.missing.is_empty() && self.stale.is_empty() && self.duplicates.is_empty()
    }
}

fn verify_formal_screen_boundary(
    repository_root: &Path,
) -> Result<FormalScreenBoundaryReport, TaskFailure> {
    let formal_business_routes_have_approved_documents =
        FORMAL_BUSINESS_DOCUMENTS
            .iter()
            .all(|(_, document_id, document_path, promotion_path)| {
                document_registration_pair_matches(
                    repository_root,
                    document_id,
                    document_path,
                    promotion_path,
                )
            });

    let navigation_source = fs::read_to_string(
        repository_root.join("project/src/game/navigation/mod.rs"),
    )
    .map_err(|error| boundary_failure(format!("cannot read navigation registry: {error}")))?;
    let route_classifications = route_classification_check(&navigation_source)?;
    let all_routable_screens_are_classified = route_classifications.exact();

    let screens_root = repository_root.join("project/src/game/screens");
    let mut rust_files = Vec::new();
    collect_rust_files(&screens_root, &mut rust_files)?;
    rust_files.sort();
    let mut direct_views = BTreeSet::new();
    for path in rust_files {
        let source = fs::read_to_string(&path).map_err(|error| {
            boundary_failure(format!(
                "cannot read screen source {}: {error}",
                path.display()
            ))
        })?;
        let contains_view = contains_direct_ui_view(&source).map_err(|error| {
            boundary_failure(format!(
                "cannot parse screen source {}: {error}",
                path.display()
            ))
        })?;
        if contains_view {
            let relative = repository_relative(repository_root, &path).ok_or_else(|| {
                boundary_failure(format!(
                    "screen source is outside repository root: {}",
                    path.display()
                ))
            })?;
            direct_views.insert(relative);
        }
    }
    let expected = CONTROLLED_RUST_VIEW_EXCEPTIONS
        .into_iter()
        .map(|(_, path)| path.to_owned())
        .collect::<BTreeSet<_>>();
    let direct_rust_ui_views_match_controlled_exceptions = direct_views == expected;

    if formal_business_routes_have_approved_documents
        && all_routable_screens_are_classified
        && direct_rust_ui_views_match_controlled_exceptions
    {
        Ok(FormalScreenBoundaryReport {
            formal_business_routes_have_approved_documents,
            all_routable_screens_are_classified,
            direct_rust_ui_views_match_controlled_exceptions,
        })
    } else {
        Err(boundary_failure(format!(
            "formal UI screen boundary failed (approved_business_documents={formal_business_routes_have_approved_documents}, classified_routes={all_routable_screens_are_classified}, controlled_rust_views={direct_rust_ui_views_match_controlled_exceptions}, actual_route_variants={}, classified_route_variants={}, missing_route_classifications={}, stale_route_classifications={}, duplicate_route_classifications={}, observed_direct_views={}, expected_direct_views={})",
            format_string_set(&route_classifications.actual),
            format_string_set(&route_classifications.classified),
            format_string_set(&route_classifications.missing),
            format_string_set(&route_classifications.stale),
            format_string_set(&route_classifications.duplicates),
            format_string_set(&direct_views),
            format_string_set(&expected),
        )))
    }
}

fn route_classification_check(source: &str) -> Result<RouteClassificationCheck, TaskFailure> {
    let syntax = syn::parse_file(source).map_err(|error| {
        boundary_failure(format!("cannot parse navigation registry as Rust: {error}"))
    })?;
    let mut app_ui_modes = syntax.items.iter().filter_map(|item| match item {
        Item::Enum(item) if item.ident == "AppUiMode" => Some(item),
        _ => None,
    });
    let app_ui_mode = app_ui_modes.next().ok_or_else(|| {
        boundary_failure("navigation registry must define enum AppUiMode exactly once")
    })?;
    if app_ui_modes.next().is_some() {
        return Err(boundary_failure(
            "navigation registry must define enum AppUiMode exactly once",
        ));
    }

    let mut actual = BTreeSet::new();
    let mut duplicate_actual = BTreeSet::new();
    for variant in &app_ui_mode.variants {
        let variant = variant.ident.to_string();
        if !actual.insert(variant.clone()) {
            duplicate_actual.insert(variant);
        }
    }

    let mut classified = BTreeSet::new();
    let mut duplicates = duplicate_actual;
    for variant in FORMAL_BUSINESS_DOCUMENTS
        .iter()
        .map(|(variant, _, _, _)| *variant)
        .chain(
            CONTROLLED_RUST_VIEW_EXCEPTIONS
                .iter()
                .map(|(variant, _)| *variant),
        )
        .chain(DECLARATIVE_DEV_SCREEN_VARIANTS)
    {
        if !classified.insert(variant.to_owned()) {
            duplicates.insert(variant.to_owned());
        }
    }

    Ok(RouteClassificationCheck {
        missing: actual.difference(&classified).cloned().collect(),
        stale: classified.difference(&actual).cloned().collect(),
        actual,
        classified,
        duplicates,
    })
}

fn format_string_set(values: &BTreeSet<String>) -> String {
    format!(
        "[{}]",
        values.iter().cloned().collect::<Vec<_>>().join(", ")
    )
}

fn document_registration_pair_matches(
    repository_root: &Path,
    expected_document_id: &str,
    document_path: &str,
    promotion_path: &str,
) -> bool {
    let Ok(document_source) = fs::read_to_string(repository_root.join(document_path)) else {
        return false;
    };
    let Ok(promotion_source) = fs::read_to_string(repository_root.join(promotion_path)) else {
        return false;
    };
    let Ok(document): Result<serde_json::Value, _> = serde_json::from_str(&document_source) else {
        return false;
    };
    let Ok(promotion): Result<serde_json::Value, _> = serde_json::from_str(&promotion_source)
    else {
        return false;
    };
    document
        .get("document_id")
        .and_then(serde_json::Value::as_str)
        == Some(expected_document_id)
        && promotion
            .get("document_id")
            .and_then(serde_json::Value::as_str)
            == Some(expected_document_id)
        && promotion.get("kind").and_then(serde_json::Value::as_str)
            == Some("ui_document_promotion_registration")
        && matches!(
            promotion.get("panel").and_then(serde_json::Value::as_str),
            Some("page" | "hud")
        )
}

fn collect_rust_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), TaskFailure> {
    for entry in fs::read_dir(directory).map_err(|error| {
        boundary_failure(format!("cannot scan {}: {error}", directory.display()))
    })? {
        let entry =
            entry.map_err(|error| boundary_failure(format!("screen scan failed: {error}")))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, output)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            output.push(path);
        }
    }
    Ok(())
}

fn contains_direct_ui_view(source: &str) -> Result<bool, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut visitor = DirectUiViewVisitor::default();
    visitor.visit_file(&syntax);
    Ok(visitor.known_view_helper
        || visitor.children_macro
        || (visitor.entity_tree_builder && visitor.ui_primitive_construction))
}

#[derive(Default)]
struct DirectUiViewVisitor {
    entity_tree_builder: bool,
    ui_primitive_construction: bool,
    known_view_helper: bool,
    children_macro: bool,
}

impl DirectUiViewVisitor {
    fn production_attribute(attrs: &[syn::Attribute]) -> bool {
        !attrs.iter().any(|attribute| {
            attribute.path().is_ident("test")
                || (attribute.path().is_ident("cfg")
                    && matches!(
                        &attribute.meta,
                        syn::Meta::List(list) if list.tokens.to_string() == "test"
                    ))
        })
    }
}

impl<'ast> Visit<'ast> for DirectUiViewVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if Self::production_attribute(&node.attrs) {
            syn::visit::visit_item_mod(self, node);
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if Self::production_attribute(&node.attrs) {
            syn::visit::visit_item_fn(self, node);
        }
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if Self::production_attribute(&node.attrs) {
            syn::visit::visit_impl_item_fn(self, node);
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if matches!(
            node.method.to_string().as_str(),
            "spawn" | "spawn_batch" | "with_child" | "with_children"
        ) {
            self.entity_tree_builder = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            if let Some(name) = path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
            {
                if matches!(
                    name.as_str(),
                    "game_panel_root" | "screen_title_key" | "secondary_action_button"
                ) {
                    self.known_view_helper = true;
                }
            }
            if path_contains_ui_primitive(&path.path) {
                self.ui_primitive_construction = true;
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if path_contains_ui_primitive(&node.path) {
            self.ui_primitive_construction = true;
        }
        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if path_contains_ui_primitive(&node.path) {
            self.ui_primitive_construction = true;
        }
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if node
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "children")
        {
            self.children_macro = true;
        }
        syn::visit::visit_expr_macro(self, node);
    }
}

fn path_contains_ui_primitive(path: &syn::Path) -> bool {
    path.segments.iter().any(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "Node" | "Text" | "Button" | "ImageNode"
        )
    })
}

fn repository_relative(repository_root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(repository_root)
        .ok()
        .and_then(Path::to_str)
        .map(|path| path.replace('\\', "/"))
}

/// Validates a reviewable list of files produced by a UI-only task or promotion dry-run.
/// It rejects source, Cargo, Android, protocol, and unapproved asset destinations by default.
pub fn verify_ui_only_change_manifest(
    manifest: &UiOnlyChangeManifest,
) -> Result<UiOnlyChangeBoundaryReport, TaskFailure> {
    if manifest.paths.is_empty() || manifest.paths.len() > 512 {
        return Err(boundary_failure(
            "UI-only change manifest must contain 1-512 changed paths",
        ));
    }
    let mut paths = BTreeSet::new();
    let mut allowed_paths = Vec::new();
    let mut blocked_paths = Vec::new();
    for path in &manifest.paths {
        if !safe_repository_relative_path(path) || !paths.insert(path.as_str()) {
            return Err(boundary_failure(
                "UI-only change manifest contains an unsafe or duplicate path",
            ));
        }
        if is_ui_only_generation_path(path) {
            allowed_paths.push(path.clone());
        } else {
            blocked_paths.push(path.clone());
        }
    }
    if !blocked_paths.is_empty() {
        return Err(boundary_failure(format!(
            "UI-only generation may not modify protected files: {}",
            blocked_paths.join(", ")
        )));
    }
    Ok(UiOnlyChangeBoundaryReport {
        allowed_paths,
        blocked_paths,
    })
}

fn manifest_graph_reaches(
    root_manifest: &Path,
    target_manifest: &Path,
) -> Result<bool, TaskFailure> {
    let root_manifest = canonical_manifest(root_manifest)?;
    let target_manifest = canonical_manifest(target_manifest)?;
    let mut pending = vec![root_manifest];
    let mut visited = BTreeSet::new();
    while let Some(manifest) = pending.pop() {
        if manifest == target_manifest {
            return Ok(true);
        }
        if !visited.insert(manifest.clone()) {
            continue;
        }
        let document = parse_toml_file(&manifest)?;
        let workspace_manifest = enclosing_workspace_manifest(&manifest)?;
        let workspace_document = workspace_manifest
            .as_ref()
            .map(|workspace| parse_toml_file(workspace))
            .transpose()?;
        let mut dependencies = dependency_manifest_paths(
            &document,
            &manifest,
            workspace_manifest.as_deref(),
            workspace_document.as_ref(),
        )?;
        if let (Some(workspace_manifest), Some(workspace_document)) =
            (workspace_manifest.as_ref(), workspace_document.as_ref())
        {
            dependencies.extend(patch_manifest_paths(
                workspace_document,
                workspace_manifest,
            )?);
        }
        pending.extend(dependencies);
    }
    Ok(false)
}

fn dependency_manifest_paths(
    document: &Value,
    manifest: &Path,
    workspace_manifest: Option<&Path>,
    workspace_document: Option<&Value>,
) -> Result<Vec<PathBuf>, TaskFailure> {
    let mut dependencies = Vec::new();
    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        collect_dependency_table(
            document.get(table_name),
            manifest,
            workspace_manifest,
            workspace_document,
            &mut dependencies,
        )?;
    }
    if let Some(targets) = document.get("target").and_then(Value::as_table) {
        for target in targets.values() {
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                collect_dependency_table(
                    target.get(table_name),
                    manifest,
                    workspace_manifest,
                    workspace_document,
                    &mut dependencies,
                )?;
            }
        }
    }
    dependencies.extend(patch_manifest_paths(document, manifest)?);
    Ok(dependencies)
}

fn collect_dependency_table(
    dependency_table: Option<&Value>,
    manifest: &Path,
    workspace_manifest: Option<&Path>,
    workspace_document: Option<&Value>,
    output: &mut Vec<PathBuf>,
) -> Result<(), TaskFailure> {
    let Some(dependencies) = dependency_table.and_then(Value::as_table) else {
        return Ok(());
    };
    for (name, specification) in dependencies {
        if let Some(path) = specification.get("path").and_then(Value::as_str) {
            output.push(resolve_dependency_manifest(manifest, path)?);
            continue;
        }
        if specification
            .get("workspace")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let workspace_manifest = workspace_manifest.ok_or_else(|| {
                boundary_failure(format!(
                    "dependency `{name}` in {} inherits a workspace dependency without an enclosing workspace",
                    manifest.display()
                ))
            })?;
            let workspace_specification = workspace_document
                .and_then(|document| document.get("workspace"))
                .and_then(|workspace| workspace.get("dependencies"))
                .and_then(|dependencies| dependencies.get(name))
                .ok_or_else(|| {
                    boundary_failure(format!(
                        "workspace dependency `{name}` inherited by {} is not declared",
                        manifest.display()
                    ))
                })?;
            if let Some(path) = workspace_specification.get("path").and_then(Value::as_str) {
                output.push(resolve_dependency_manifest(workspace_manifest, path)?);
            }
        }
    }
    Ok(())
}

fn patch_manifest_paths(document: &Value, manifest: &Path) -> Result<Vec<PathBuf>, TaskFailure> {
    let mut paths = Vec::new();
    if let Some(registries) = document.get("patch").and_then(Value::as_table) {
        for registry in registries.values().filter_map(Value::as_table) {
            for specification in registry.values() {
                if let Some(path) = specification.get("path").and_then(Value::as_str) {
                    paths.push(resolve_dependency_manifest(manifest, path)?);
                }
            }
        }
    }
    if let Some(replacements) = document.get("replace").and_then(Value::as_table) {
        for specification in replacements.values() {
            if let Some(path) = specification.get("path").and_then(Value::as_str) {
                paths.push(resolve_dependency_manifest(manifest, path)?);
            }
        }
    }
    Ok(paths)
}

fn resolve_dependency_manifest(
    owner_manifest: &Path,
    dependency_path: &str,
) -> Result<PathBuf, TaskFailure> {
    let directory = owner_manifest.parent().ok_or_else(|| {
        boundary_failure(format!(
            "manifest has no parent directory: {}",
            owner_manifest.display()
        ))
    })?;
    canonical_manifest(&directory.join(dependency_path).join("Cargo.toml"))
}

fn canonical_manifest(manifest: &Path) -> Result<PathBuf, TaskFailure> {
    let manifest = fs::canonicalize(manifest).map_err(|error| {
        boundary_failure(format!(
            "Cargo manifest cannot be resolved at {}: {error}",
            manifest.display()
        ))
    })?;
    if !manifest.is_file() {
        return Err(boundary_failure(format!(
            "Cargo manifest is not a file: {}",
            manifest.display()
        )));
    }
    Ok(manifest)
}

fn parse_toml_file(path: &Path) -> Result<Value, TaskFailure> {
    let source = fs::read_to_string(path).map_err(|error| {
        boundary_failure(format!("cannot read TOML file {}: {error}", path.display()))
    })?;
    toml::from_str(&source).map_err(|error| {
        boundary_failure(format!(
            "cannot parse TOML file {}: {error}",
            path.display()
        ))
    })
}

fn lock_contains_local_package(lock_path: &Path, package_name: &str) -> Result<bool, TaskFailure> {
    let document = parse_toml_file(lock_path)?;
    let packages = document
        .get("package")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            boundary_failure(format!(
                "Cargo lockfile has no package array: {}",
                lock_path.display()
            ))
        })?;
    Ok(packages.iter().any(|package| {
        package.get("name").and_then(Value::as_str) == Some(package_name)
            && package.get("source").is_none()
    }))
}

fn enclosing_workspace_root(manifest: &Path) -> Result<PathBuf, TaskFailure> {
    Ok(enclosing_workspace_manifest(manifest)?
        .and_then(|workspace| workspace.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| manifest.parent().unwrap_or(manifest).to_path_buf()))
}

fn enclosing_workspace_manifest(manifest: &Path) -> Result<Option<PathBuf>, TaskFailure> {
    let Some(mut directory) = manifest.parent() else {
        return Ok(None);
    };
    loop {
        let candidate = directory.join("Cargo.toml");
        if candidate.exists() {
            let document = parse_toml_file(&candidate)?;
            if document.get("workspace").is_some() {
                return canonical_manifest(&candidate).map(Some);
            }
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent;
    }
    Ok(None)
}

fn validate_boundary_flags(
    project_dependency_graph_excludes_tool: bool,
    tool_dependency_graph_excludes_project: bool,
    project_lock_excludes_tool_package: bool,
    tool_lock_excludes_project_package: bool,
    crates_are_independent_workspaces: bool,
    standalone_preview_target_is_feature_gated: bool,
    ui_only_generation_write_scope_is_closed: bool,
    formal_business_routes_have_approved_documents: bool,
    all_routable_screens_are_classified: bool,
    direct_rust_ui_views_match_controlled_exceptions: bool,
) -> Result<(), TaskFailure> {
    if project_dependency_graph_excludes_tool
        && tool_dependency_graph_excludes_project
        && project_lock_excludes_tool_package
        && tool_lock_excludes_project_package
        && crates_are_independent_workspaces
        && standalone_preview_target_is_feature_gated
        && ui_only_generation_write_scope_is_closed
        && formal_business_routes_have_approved_documents
        && all_routable_screens_are_classified
        && direct_rust_ui_views_match_controlled_exceptions
    {
        Ok(())
    } else {
        Err(boundary_failure(format!(
            "dependency direction and formal UI boundaries must remain closed (project_graph_excludes_tool={project_dependency_graph_excludes_tool}, tool_graph_excludes_project={tool_dependency_graph_excludes_project}, project_lock_excludes_tool={project_lock_excludes_tool_package}, tool_lock_excludes_project={tool_lock_excludes_project_package}, independent={crates_are_independent_workspaces}, preview_feature_gated={standalone_preview_target_is_feature_gated}, ui_only_write_scope={ui_only_generation_write_scope_is_closed}, approved_business_documents={formal_business_routes_have_approved_documents}, classified_routes={all_routable_screens_are_classified}, controlled_rust_views={direct_rust_ui_views_match_controlled_exceptions})"
        )))
    }
}

fn ui_only_generation_write_scope_is_closed() -> bool {
    [
        "project/assets/ui/documents/approved/example/document.v1.json",
        "project/assets/ui/documents/approved/example/promotion.v1.json",
        "project/assets/ui/documents/approved/example/catalog.v1.json",
        "project/assets/ui/documents/approved/example/LICENSES.md",
        "project/assets/ui/documents/approved/example/assets/generated.png",
        "tools/ui-generation/fixtures/stage9/reflow.valid.json",
    ]
    .into_iter()
    .all(is_ui_only_generation_path)
        && [
            "project/src/game/screens/auth/login.rs",
            "project/Cargo.toml",
            "android/app/build.gradle.kts",
            "project/src/game/myserver/protocol.rs",
        ]
        .into_iter()
        .all(|path| !is_ui_only_generation_path(path))
}

fn is_ui_only_generation_path(path: &str) -> bool {
    path.starts_with("tools/ui-generation/fixtures/stage9/") || is_approved_promotion_path(path)
}

fn is_approved_promotion_path(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("project/assets/ui/documents/approved/") else {
        return false;
    };
    let mut parts = relative.split('/');
    let Some(folder) = parts.next() else {
        return false;
    };
    if !safe_label(folder) {
        return false;
    }
    let remainder = parts.collect::<Vec<_>>();
    match remainder.as_slice() {
        [file] => {
            *file == "LICENSES.md" || (safe_resource_file_name(file) && file.ends_with(".v1.json"))
        }
        ["assets", file] => safe_resource_file_name(file),
        _ => false,
    }
}

fn safe_repository_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.is_ascii()
        && !path.contains(['\\', ':', '\0'])
        && !path.contains("//")
        && Path::new(path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn safe_resource_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_uppercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
        })
        && !value.starts_with('.')
}

fn preview_target_is_feature_gated(document: &Value) -> bool {
    let feature = "ui-document-preview-tool";
    let feature_declared = document
        .get("features")
        .and_then(Value::as_table)
        .is_some_and(|features| features.contains_key(feature));
    let excluded_from_default = document
        .get("features")
        .and_then(|features| features.get("default"))
        .and_then(Value::as_array)
        .is_none_or(|defaults| !defaults.iter().any(|value| value.as_str() == Some(feature)));
    let target_gated = document
        .get("bin")
        .and_then(Value::as_array)
        .is_some_and(|bins| {
            bins.iter().any(|bin| {
                bin.get("name").and_then(Value::as_str) == Some("ui-document-preview")
                    && bin
                        .get("required-features")
                        .and_then(Value::as_array)
                        .is_some_and(|features| {
                            features.len() == 1 && features[0].as_str() == Some(feature)
                        })
            })
        });
    feature_declared && excluded_from_default && target_gated
}

fn boundary_failure(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::DependencyBoundaryViolation, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn write_manifest(directory: &Path, source: &str) -> PathBuf {
        fs::create_dir_all(directory).unwrap();
        let manifest = directory.join("Cargo.toml");
        fs::write(&manifest, source).unwrap();
        manifest
    }

    #[test]
    fn manifest_graph_keeps_project_and_tool_graphs_disconnected() {
        let root = tempfile::tempdir().unwrap();
        let project = write_manifest(
            &root.path().join("project"),
            "[package]\nname='project'\nversion='0.1.0'\n",
        );
        write_manifest(
            &root.path().join("core"),
            "[package]\nname='ui-document-core'\nversion='0.1.0'\n",
        );
        let tool = write_manifest(
            &root.path().join("tool"),
            "[package]\nname='ui-generation'\nversion='0.1.0'\n[dependencies]\nui-document-core={path='../core'}\n",
        );
        assert!(!manifest_graph_reaches(&tool, &project).unwrap());
        assert!(!manifest_graph_reaches(&project, &tool).unwrap());
    }

    #[test]
    fn indirect_project_dependency_on_tool_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let project = write_manifest(
            &root.path().join("project"),
            "[package]\nname='project'\nversion='0.1.0'\n[dependencies]\nmiddle={path='../middle'}\n",
        );
        write_manifest(
            &root.path().join("middle"),
            "[package]\nname='middle'\nversion='0.1.0'\n[dependencies]\nui-generation={path='../tool'}\n",
        );
        let tool = write_manifest(
            &root.path().join("tool"),
            "[package]\nname='ui-generation'\nversion='0.1.0'\n",
        );
        let project_dependency_graph_excludes_tool =
            !manifest_graph_reaches(&project, &tool).unwrap();
        assert!(!project_dependency_graph_excludes_tool);
        let failure = validate_boundary_flags(
            project_dependency_graph_excludes_tool,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::DependencyBoundaryViolation);
        assert!(
            failure
                .message()
                .contains("project_graph_excludes_tool=false")
        );
    }

    #[test]
    fn lockfile_check_distinguishes_local_and_registry_packages() {
        let root = tempfile::tempdir().unwrap();
        let lock = root.path().join("Cargo.lock");
        fs::write(
            &lock,
            "version = 4\n[[package]]\nname='project'\nversion='0.1.0'\n[[package]]\nname='ui-generation'\nversion='9.0.0'\nsource='registry+https://example.invalid/index'\n",
        )
        .unwrap();
        assert!(lock_contains_local_package(&lock, "project").unwrap());
        assert!(!lock_contains_local_package(&lock, "ui-generation").unwrap());
    }

    #[test]
    fn standalone_preview_target_requires_non_default_feature() {
        let gated: Value = toml::from_str(
            r#"
            [features]
            default = []
            ui-document-preview-tool = []

            [[bin]]
            name = "ui-document-preview"
            required-features = ["ui-document-preview-tool"]
            "#,
        )
        .unwrap();
        assert!(preview_target_is_feature_gated(&gated));

        let mut ungated = gated;
        ungated["bin"][0]
            .as_table_mut()
            .unwrap()
            .remove("required-features");
        assert!(!preview_target_is_feature_gated(&ungated));
    }

    #[test]
    fn summary_generation_outputs_are_git_ignored() {
        let root = repository_root();
        let output = std::process::Command::new("git")
            .args([
                "check-ignore",
                "--quiet",
                "summary/ui-generation/test/input/reference.png",
            ])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(output.success());
    }

    #[test]
    fn ui_only_change_manifest_allows_only_promotable_resources_and_fixture_evidence() {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/stage9");
        let valid: UiOnlyChangeManifest = serde_json::from_slice(
            &fs::read(fixture_root.join("reflow.ui_only_changes.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            verify_ui_only_change_manifest(&valid)
                .unwrap()
                .allowed_paths,
            valid.paths
        );

        let protected: UiOnlyChangeManifest = serde_json::from_slice(
            &fs::read(fixture_root.join("failure.protected_paths.json")).unwrap(),
        )
        .unwrap();
        assert!(verify_ui_only_change_manifest(&protected).is_err());

        assert!(
            verify_ui_only_change_manifest(&UiOnlyChangeManifest {
                paths: vec!["tools/ui-generation/fixtures/stage9//reflow.valid.json".to_owned()],
            })
            .is_err()
        );

        assert!(
            verify_ui_only_change_manifest(&UiOnlyChangeManifest {
                paths: vec![
                    "project/assets/ui/documents/approved/gameplay/robot_sync_hud.v1.json"
                        .to_owned(),
                    "project/assets/ui/documents/approved/gameplay/robot_sync_hud.promotion.v1.json"
                        .to_owned(),
                ],
            })
            .is_ok()
        );
        assert!(
            verify_ui_only_change_manifest(&UiOnlyChangeManifest {
                paths: vec!["project/src/game/screens/gameplay/robot_sync_scene.rs".to_owned()],
            })
            .is_err()
        );
    }

    #[test]
    fn formal_screen_boundary_covers_business_documents_and_closed_rust_view_exceptions() {
        let report = verify_formal_screen_boundary(&repository_root()).unwrap();
        assert!(report.formal_business_routes_have_approved_documents);
        assert!(report.all_routable_screens_are_classified);
        assert!(report.direct_rust_ui_views_match_controlled_exceptions);
        assert!(
            FORMAL_BUSINESS_DOCUMENTS
                .iter()
                .any(|(variant, document, _, _)| {
                    *variant == "MainWorld" && *document == "game.main_world_hud"
                })
        );
        assert!(
            CONTROLLED_RUST_VIEW_EXCEPTIONS
                .iter()
                .all(|(variant, _)| *variant != "MainWorld")
        );

        assert!(
            contains_direct_ui_view(
                "fn setup(mut commands: Commands) { commands.spawn((game_panel_root(id, kind, owner), Node::default())); }"
            )
            .unwrap()
        );
        assert!(
            !contains_direct_ui_view(
                "fn setup(mut commands: Commands) { commands.spawn((Camera3d::default(), Transform::default())); }"
            )
            .unwrap()
        );
    }

    #[test]
    fn app_ui_mode_added_variant_requires_an_explicit_classification() {
        let navigation_path = repository_root().join("project/src/game/navigation/mod.rs");
        let mut fixture = fs::read_to_string(navigation_path).unwrap();
        let enum_start = fixture.find("enum AppUiMode").unwrap();
        let enum_end = enum_start + fixture[enum_start..].find('}').unwrap();
        fixture.insert_str(enum_end, "    UnclassifiedFixture,\n");

        let check = route_classification_check(&fixture).unwrap();
        assert!(!check.exact());
        assert_eq!(
            check.missing,
            BTreeSet::from(["UnclassifiedFixture".to_owned()])
        );
        assert!(check.stale.is_empty());
        assert!(check.duplicates.is_empty());
    }

    #[test]
    fn direct_bevy_ui_tree_without_legacy_sentinels_is_detected() {
        let source = r#"
            fn setup(mut commands: Commands) {
                commands
                    .spawn((Node { width: Val::Percent(100.0), ..default() }, BackgroundColor::default()))
                    .with_children(|parent| {
                        parent.spawn((Text::new("Title"), TextFont::default()));
                        parent.spawn((Button, Node::default()));
                    });
            }
        "#;
        assert!(!source.contains("game_panel_root("));
        assert!(!source.contains("children!["));
        assert!(!source.contains("screen_title_key("));
        assert!(!source.contains("secondary_action_button"));
        assert!(contains_direct_ui_view(source).unwrap());
    }

    #[test]
    fn gameplay_and_3d_entity_trees_are_not_direct_ui_views() {
        let source = r#"
            fn setup(mut commands: Commands, mesh: Handle<Mesh>, material: Handle<StandardMaterial>) {
                commands.spawn((Camera3d::default(), Transform::default()));
                commands
                    .spawn((Mesh3d(mesh), MeshMaterial3d(material), Transform::default()))
                    .with_children(|parent| {
                        parent.spawn((PointLight::default(), Transform::default()));
                    });
            }
        "#;
        assert!(!contains_direct_ui_view(source).unwrap());
    }
}
