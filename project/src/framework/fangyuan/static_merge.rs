use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{
    FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
    FangyuanAuditSourceKind, FangyuanBlueprintValidationError, FangyuanPrefabPalette,
    FangyuanPrimitive, FangyuanPrimitiveKind, FangyuanPrimitiveRole, FangyuanPrimitiveSet,
    FangyuanSceneLayout, FangyuanSceneLayoutCompileError, FangyuanSceneLayoutValidationError,
    compile_blueprint_primitive_to_runtime, is_valid_fangyuan_material_profile_id,
    transform_prefab_primitive, validate_blueprint_primitive,
};

pub const FANGYUAN_STATIC_MERGE_DEFAULT_REGION_PLACEHOLDER: &str = "region:unassigned";
pub const FANGYUAN_STATIC_MERGE_DEFAULT_MATERIAL_PROFILE: &str =
    FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID;
pub const FANGYUAN_STATIC_MERGE_DEFAULT_DEBUG_LABEL: &str = "fangyuan_static";

pub const FANGYUAN_STATIC_MERGE_CUBE_VERTEX_COUNT: usize = 24;
pub const FANGYUAN_STATIC_MERGE_CUBE_INDEX_COUNT: usize = 36;
pub const FANGYUAN_STATIC_MERGE_SPHERE_SECTORS: usize = 24;
pub const FANGYUAN_STATIC_MERGE_SPHERE_STACKS: usize = 12;
pub const FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT: usize =
    (FANGYUAN_STATIC_MERGE_SPHERE_SECTORS + 1) * (FANGYUAN_STATIC_MERGE_SPHERE_STACKS + 1);
pub const FANGYUAN_STATIC_MERGE_SPHERE_INDEX_COUNT: usize =
    FANGYUAN_STATIC_MERGE_SPHERE_SECTORS * FANGYUAN_STATIC_MERGE_SPHERE_STACKS * 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanStaticMergeSourceKind {
    SceneLayout,
    RuntimePrimitiveSet,
    #[default]
    Unknown,
}

impl From<FangyuanAuditSourceKind> for FangyuanStaticMergeSourceKind {
    fn from(value: FangyuanAuditSourceKind) -> Self {
        match value {
            FangyuanAuditSourceKind::SceneLayout => Self::SceneLayout,
            FangyuanAuditSourceKind::RuntimePrimitiveSet => Self::RuntimePrimitiveSet,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanStaticMergeSourceRef {
    pub source_kind: FangyuanStaticMergeSourceKind,
    pub source_path: Option<String>,
    pub layout_instance: Option<FangyuanStaticMergeLayoutInstanceRef>,
    pub prefab_id: Option<String>,
    pub primitive_index: usize,
    pub field_path: Option<String>,
    pub audit_code: Option<String>,
    pub audit_reason: Option<String>,
}

impl FangyuanStaticMergeSourceRef {
    pub fn runtime_primitive_set(
        source_path: impl Into<Option<String>>,
        primitive_index: usize,
    ) -> Self {
        Self {
            source_kind: FangyuanStaticMergeSourceKind::RuntimePrimitiveSet,
            source_path: source_path.into(),
            primitive_index,
            field_path: Some(format!("primitives[{primitive_index}]")),
            ..Default::default()
        }
    }

    pub fn scene_layout(
        source_path: impl Into<Option<String>>,
        instance_index: usize,
        instance_id: Option<&str>,
        prefab_id: &str,
        primitive_index: usize,
    ) -> Self {
        Self {
            source_kind: FangyuanStaticMergeSourceKind::SceneLayout,
            source_path: source_path.into(),
            layout_instance: Some(FangyuanStaticMergeLayoutInstanceRef {
                index: instance_index,
                id: instance_id.map(str::to_string),
            }),
            prefab_id: Some(prefab_id.to_string()),
            primitive_index,
            field_path: Some(format!(
                "instances[{instance_index}].prefab.primitives[{primitive_index}]"
            )),
            ..Default::default()
        }
    }

    fn with_audit_context(
        mut self,
        code: impl Into<String>,
        reason: impl Into<String>,
        field_path: impl Into<Option<String>>,
    ) -> Self {
        self.audit_code = Some(code.into());
        self.audit_reason = Some(reason.into());
        if let Some(field_path) = field_path.into() {
            self.field_path = Some(field_path);
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanStaticMergeLayoutInstanceRef {
    pub index: usize,
    pub id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticMergeInput {
    pub primitive: FangyuanPrimitive,
    pub source: FangyuanStaticMergeSourceRef,
}

impl FangyuanStaticMergeInput {
    pub fn new(primitive: FangyuanPrimitive, source: FangyuanStaticMergeSourceRef) -> Self {
        Self { primitive, source }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticMergeBuildOptions {
    pub region_placeholder: String,
    pub debug_label: String,
}

impl Default for FangyuanStaticMergeBuildOptions {
    fn default() -> Self {
        Self {
            region_placeholder: FANGYUAN_STATIC_MERGE_DEFAULT_REGION_PLACEHOLDER.to_string(),
            debug_label: FANGYUAN_STATIC_MERGE_DEFAULT_DEBUG_LABEL.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStaticMergeBuildReport {
    pub groups: Vec<FangyuanStaticMergeGroup>,
    pub skipped: Vec<FangyuanStaticMergeSkippedPrimitive>,
    pub stats: FangyuanStaticMergeStats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStaticMergeGroup {
    pub key: FangyuanStaticMergeGroupKey,
    pub primitive_count: usize,
    pub source_refs: Vec<FangyuanStaticMergeSourceRef>,
    pub estimated_vertex_count: usize,
    pub estimated_index_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanStaticMergeGroupKey {
    pub region_placeholder: String,
    pub primitive_kind: FangyuanPrimitiveKind,
    pub material_profile: String,
    pub transparent_path: FangyuanStaticMergeTransparentPath,
    pub debug_label: String,
    pub color: FangyuanStaticMergeColorKey,
    pub emissive: FangyuanStaticMergeF32Key,
}

impl FangyuanStaticMergeGroupKey {
    pub fn from_primitive(
        primitive: &FangyuanPrimitive,
        options: &FangyuanStaticMergeBuildOptions,
    ) -> Self {
        Self {
            region_placeholder: options.region_placeholder.clone(),
            primitive_kind: primitive.kind(),
            material_profile: primitive
                .material_profile_id()
                .map(str::to_string)
                .unwrap_or_else(|| FANGYUAN_STATIC_MERGE_DEFAULT_MATERIAL_PROFILE.to_string()),
            transparent_path: FangyuanStaticMergeTransparentPath::from_alpha(primitive.alpha()),
            debug_label: options.debug_label.clone(),
            color: FangyuanStaticMergeColorKey::from_color_and_alpha(
                primitive.color(),
                primitive.alpha(),
            ),
            emissive: FangyuanStaticMergeF32Key::from_f32(primitive.emissive()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanStaticMergeTransparentPath {
    #[default]
    Opaque,
    Transparent,
}

impl FangyuanStaticMergeTransparentPath {
    pub fn from_alpha(alpha: f32) -> Self {
        if alpha < 1.0 {
            Self::Transparent
        } else {
            Self::Opaque
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanStaticMergeColorKey([u32; 4]);

impl FangyuanStaticMergeColorKey {
    pub fn from_color_and_alpha(color: Color, alpha: f32) -> Self {
        let color = color.to_srgba();
        Self([
            canonical_f32_bits(color.red),
            canonical_f32_bits(color.green),
            canonical_f32_bits(color.blue),
            canonical_f32_bits(alpha),
        ])
    }

    pub fn channels(self) -> [f32; 4] {
        [
            f32::from_bits(self.0[0]),
            f32::from_bits(self.0[1]),
            f32::from_bits(self.0[2]),
            f32::from_bits(self.0[3]),
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanStaticMergeF32Key(u32);

impl FangyuanStaticMergeF32Key {
    pub fn from_f32(value: f32) -> Self {
        Self(canonical_f32_bits(value))
    }

    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStaticMergeSkippedPrimitive {
    pub source: FangyuanStaticMergeSourceRef,
    pub reason: FangyuanStaticMergeSkipReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanStaticMergeSkipReason {
    DynamicLifecycle,
    DynamicRole,
    SemanticSocket,
    NonFiniteTransform,
    NonPositiveScale,
    InvalidColor,
    InvalidAlpha,
    InvalidEmissive,
    InvalidMaterialProfile,
    InvalidScenePrimitive,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanStaticMergeStats {
    pub authored_primitives: usize,
    pub expanded_primitives: usize,
    pub merged_group_count: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub skipped_primitives: usize,
    pub material_profile_count: usize,
    pub estimated_vertex_count: usize,
    pub estimated_index_count: usize,
}

pub fn fangyuan_static_merge_groups_from_inputs(
    inputs: impl IntoIterator<Item = FangyuanStaticMergeInput>,
) -> FangyuanStaticMergeBuildReport {
    fangyuan_static_merge_groups_from_inputs_with_options(
        inputs,
        &FangyuanStaticMergeBuildOptions::default(),
    )
}

pub fn fangyuan_static_merge_groups_from_inputs_with_options(
    inputs: impl IntoIterator<Item = FangyuanStaticMergeInput>,
    options: &FangyuanStaticMergeBuildOptions,
) -> FangyuanStaticMergeBuildReport {
    let inputs = inputs.into_iter().collect::<Vec<_>>();
    let mut stats = FangyuanStaticMergeStats {
        authored_primitives: inputs.len(),
        expanded_primitives: inputs.len(),
        ..Default::default()
    };
    let mut groups_by_key =
        BTreeMap::<FangyuanStaticMergeGroupKey, Vec<FangyuanStaticMergeInput>>::new();
    let mut skipped = Vec::new();
    let mut material_profiles = BTreeSet::new();

    for input in inputs {
        match fangyuan_static_merge_skip_reason(&input.primitive) {
            Some(reason) => {
                skipped.push(FangyuanStaticMergeSkippedPrimitive {
                    source: input.source,
                    reason,
                });
                continue;
            }
            None => {
                let key = FangyuanStaticMergeGroupKey::from_primitive(&input.primitive, options);
                material_profiles.insert(key.material_profile.clone());
                groups_by_key.entry(key).or_default().push(input);
            }
        }
    }

    let groups = groups_by_key
        .into_iter()
        .map(|(key, mut inputs)| {
            inputs.sort_by(|left, right| left.source.cmp(&right.source));
            let primitive_count = inputs.len();
            let estimated_vertex_count =
                primitive_count * estimated_vertex_count_for_kind(key.primitive_kind);
            let estimated_index_count =
                primitive_count * estimated_index_count_for_kind(key.primitive_kind);
            let source_refs = inputs
                .into_iter()
                .map(|input| input.source)
                .collect::<Vec<_>>();

            match key.primitive_kind {
                FangyuanPrimitiveKind::Cube => stats.cube_count += primitive_count,
                FangyuanPrimitiveKind::Sphere => stats.sphere_count += primitive_count,
            }
            stats.estimated_vertex_count += estimated_vertex_count;
            stats.estimated_index_count += estimated_index_count;

            FangyuanStaticMergeGroup {
                key,
                primitive_count,
                source_refs,
                estimated_vertex_count,
                estimated_index_count,
            }
        })
        .collect::<Vec<_>>();

    stats.merged_group_count = groups.len();
    stats.skipped_primitives = skipped.len();
    stats.material_profile_count = material_profiles.len();

    FangyuanStaticMergeBuildReport {
        groups,
        skipped,
        stats,
    }
}

pub fn fangyuan_static_merge_groups_from_primitive_set(
    primitive_set: &FangyuanPrimitiveSet,
) -> FangyuanStaticMergeBuildReport {
    fangyuan_static_merge_groups_from_primitive_set_with_source(
        primitive_set,
        None::<String>,
        &FangyuanStaticMergeBuildOptions::default(),
    )
}

pub fn fangyuan_static_merge_groups_from_primitive_set_with_source(
    primitive_set: &FangyuanPrimitiveSet,
    source_path: impl Into<Option<String>>,
    options: &FangyuanStaticMergeBuildOptions,
) -> FangyuanStaticMergeBuildReport {
    let source_path = source_path.into();
    let inputs = primitive_set.primitives().iter().cloned().enumerate().map(
        |(primitive_index, primitive)| {
            FangyuanStaticMergeInput::new(
                primitive,
                FangyuanStaticMergeSourceRef::runtime_primitive_set(
                    source_path.clone(),
                    primitive_index,
                ),
            )
        },
    );

    fangyuan_static_merge_groups_from_inputs_with_options(inputs, options)
}

pub fn fangyuan_static_merge_groups_from_layout(
    layout: &FangyuanSceneLayout,
    palette: &FangyuanPrefabPalette,
    layout_source_path: impl Into<Option<String>>,
) -> Result<FangyuanStaticMergeBuildReport, FangyuanSceneLayoutCompileError> {
    fangyuan_static_merge_groups_from_layout_with_options(
        layout,
        palette,
        layout_source_path,
        &FangyuanStaticMergeBuildOptions::default(),
    )
}

pub fn fangyuan_static_merge_groups_from_layout_with_options(
    layout: &FangyuanSceneLayout,
    palette: &FangyuanPrefabPalette,
    layout_source_path: impl Into<Option<String>>,
    options: &FangyuanStaticMergeBuildOptions,
) -> Result<FangyuanStaticMergeBuildReport, FangyuanSceneLayoutCompileError> {
    palette
        .validate()
        .map_err(FangyuanSceneLayoutCompileError::PaletteValidationFailed)?;
    layout
        .validate_against_palette(palette)
        .map_err(FangyuanSceneLayoutCompileError::LayoutValidationFailed)?;

    let source_path = layout_source_path.into();
    let prefab_by_id = palette
        .prefabs
        .iter()
        .map(|prefab| (prefab.id.as_str(), prefab))
        .collect::<HashMap<_, _>>();
    let authored_primitives = palette
        .prefabs
        .iter()
        .map(|prefab| prefab.primitives.len())
        .sum::<usize>();
    let expanded_primitives = layout
        .instances
        .iter()
        .filter_map(|instance| prefab_by_id.get(instance.prefab.as_str()))
        .map(|prefab| prefab.primitives.len())
        .sum::<usize>();

    let mut inputs = Vec::with_capacity(expanded_primitives);
    let mut skipped = Vec::new();
    for (instance_index, instance) in layout.instances.iter().enumerate() {
        let Some(prefab) = prefab_by_id.get(instance.prefab.as_str()) else {
            return Err(FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                FangyuanSceneLayoutValidationError::MissingPrefab {
                    instance_index,
                    prefab: instance.prefab.clone(),
                },
            ));
        };

        for (primitive_index, primitive) in prefab.primitives.iter().enumerate() {
            let transformed = transform_prefab_primitive(instance, prefab, primitive);
            let source = FangyuanStaticMergeSourceRef::scene_layout(
                source_path.clone(),
                instance_index,
                instance.id.as_deref(),
                &prefab.id,
                primitive_index,
            );

            match validate_blueprint_primitive(primitive_index, &transformed, &layout.bounds) {
                Ok(()) => inputs.push(FangyuanStaticMergeInput::new(
                    compile_blueprint_primitive_to_runtime(&transformed),
                    source,
                )),
                Err(error) => skipped.push(FangyuanStaticMergeSkippedPrimitive {
                    source: source.with_audit_context(
                        error.code(),
                        error.reason(),
                        Some(layout_primitive_error_field_path(instance_index, &error)),
                    ),
                    reason: FangyuanStaticMergeSkipReason::InvalidScenePrimitive,
                }),
            }
        }
    }

    let mut report = fangyuan_static_merge_groups_from_inputs_with_options(inputs, options);
    report.skipped.extend(skipped);
    report.skipped.sort_by(|left, right| {
        (left.source.clone(), left.reason).cmp(&(right.source.clone(), right.reason))
    });
    report.stats.authored_primitives = authored_primitives;
    report.stats.expanded_primitives = expanded_primitives;
    report.stats.skipped_primitives = report.skipped.len();

    Ok(report)
}

pub fn fangyuan_static_merge_skip_reason(
    primitive: &FangyuanPrimitive,
) -> Option<FangyuanStaticMergeSkipReason> {
    if !primitive.lifecycle().is_empty() {
        return Some(FangyuanStaticMergeSkipReason::DynamicLifecycle);
    }

    match primitive.role() {
        FangyuanPrimitiveRole::Warning
        | FangyuanPrimitiveRole::Trail
        | FangyuanPrimitiveRole::Impact => {
            return Some(FangyuanStaticMergeSkipReason::DynamicRole);
        }
        FangyuanPrimitiveRole::Socket => {
            return Some(FangyuanStaticMergeSkipReason::SemanticSocket);
        }
        FangyuanPrimitiveRole::Structure
        | FangyuanPrimitiveRole::Core
        | FangyuanPrimitiveRole::Boundary
        | FangyuanPrimitiveRole::Decoration
        | FangyuanPrimitiveRole::Archive => {}
    }

    let position = primitive.local_position();
    let scale = primitive.scale();
    if !position.is_finite() || !scale.is_finite() {
        return Some(FangyuanStaticMergeSkipReason::NonFiniteTransform);
    }
    if scale.cmple(Vec3::ZERO).any() {
        return Some(FangyuanStaticMergeSkipReason::NonPositiveScale);
    }
    let color = primitive.color().to_srgba();
    if !is_valid_static_merge_color_channel(color.red)
        || !is_valid_static_merge_color_channel(color.green)
        || !is_valid_static_merge_color_channel(color.blue)
        || !is_valid_static_merge_color_channel(color.alpha)
    {
        return Some(FangyuanStaticMergeSkipReason::InvalidColor);
    }
    if !primitive.alpha().is_finite() || !(0.0..=1.0).contains(&primitive.alpha()) {
        return Some(FangyuanStaticMergeSkipReason::InvalidAlpha);
    }
    if !primitive.emissive().is_finite()
        || primitive.emissive() < FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE
    {
        return Some(FangyuanStaticMergeSkipReason::InvalidEmissive);
    }
    if primitive
        .material_profile_id()
        .is_some_and(|material_profile_id| {
            !is_valid_static_merge_material_profile(material_profile_id)
        })
    {
        return Some(FangyuanStaticMergeSkipReason::InvalidMaterialProfile);
    }

    None
}

fn estimated_vertex_count_for_kind(kind: FangyuanPrimitiveKind) -> usize {
    match kind {
        FangyuanPrimitiveKind::Cube => FANGYUAN_STATIC_MERGE_CUBE_VERTEX_COUNT,
        FangyuanPrimitiveKind::Sphere => FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT,
    }
}

fn estimated_index_count_for_kind(kind: FangyuanPrimitiveKind) -> usize {
    match kind {
        FangyuanPrimitiveKind::Cube => FANGYUAN_STATIC_MERGE_CUBE_INDEX_COUNT,
        FangyuanPrimitiveKind::Sphere => FANGYUAN_STATIC_MERGE_SPHERE_INDEX_COUNT,
    }
}

fn is_valid_static_merge_material_profile(material_profile_id: &str) -> bool {
    is_valid_fangyuan_material_profile_id(material_profile_id)
}

fn is_valid_static_merge_color_channel(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn layout_primitive_error_field_path(
    instance_index: usize,
    error: &FangyuanBlueprintValidationError,
) -> String {
    let prefab_field_path = error.field_path().into_owned();
    format!("instances[{instance_index}].prefab.{prefab_field_path}")
}

fn canonical_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT, FANGYUAN_SCENE_LAYOUT_VERSION,
        FangyuanBlueprintBounds, FangyuanPrefabDefinition, FangyuanPrimitiveBlueprint,
        FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole, FangyuanSceneLayoutInstance,
    };

    #[test]
    fn fangyuan_static_merge_group_key_is_stable_and_sorted_by_key() {
        let options = FangyuanStaticMergeBuildOptions {
            region_placeholder: "region:test".to_string(),
            debug_label: "debug:static_home".to_string(),
        };
        let first = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.3, 0.4, 1.0],
            Some("stone/wall"),
        );
        let second = static_primitive(
            FangyuanPrimitiveKind::Sphere,
            FangyuanPrimitiveRole::Decoration,
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.8, 0.6, 0.4, 1.0],
            Some("clay/trim"),
        );

        let report = fangyuan_static_merge_groups_from_inputs_with_options(
            vec![
                FangyuanStaticMergeInput::new(
                    second.clone(),
                    FangyuanStaticMergeSourceRef::runtime_primitive_set(None, 1),
                ),
                FangyuanStaticMergeInput::new(
                    first.clone(),
                    FangyuanStaticMergeSourceRef::runtime_primitive_set(None, 0),
                ),
            ],
            &options,
        );

        assert_eq!(report.groups.len(), 2);
        assert_eq!(
            report.groups[0].key.primitive_kind,
            FangyuanPrimitiveKind::Cube
        );
        assert_eq!(report.groups[0].key.material_profile, "stone/wall");
        assert_eq!(report.groups[0].key.region_placeholder, "region:test");
        assert_eq!(report.groups[0].key.debug_label, "debug:static_home");
        assert_eq!(
            report.groups[0].key.transparent_path,
            FangyuanStaticMergeTransparentPath::Opaque
        );
        assert_eq!(report.groups[0].estimated_vertex_count, 24);
        assert_eq!(report.groups[0].estimated_index_count, 36);
        assert_eq!(report.stats.merged_group_count, 2);
        assert_eq!(report.stats.cube_count, 1);
        assert_eq!(report.stats.sphere_count, 1);
        assert_eq!(report.stats.material_profile_count, 2);
    }

    #[test]
    fn fangyuan_static_merge_group_skips_dynamic_short_lived_and_socket_roles() {
        let dynamic = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::WHITE,
            FangyuanPrimitiveRole::Structure,
            1.0,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
            FangyuanPrimitiveLifecycle::new(Some(3), Some(1), Some(4)),
        );
        let trail = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Trail,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            None,
        );
        let socket = static_primitive(
            FangyuanPrimitiveKind::Sphere,
            FangyuanPrimitiveRole::Socket,
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            None,
        );
        let static_cube = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Archive,
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            None,
        );

        let report = fangyuan_static_merge_groups_from_inputs(vec![
            input(dynamic, 0),
            input(trail, 1),
            input(socket, 2),
            input(static_cube, 3),
        ]);

        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].primitive_count, 1);
        assert_eq!(
            skipped_reasons(&report),
            vec![
                FangyuanStaticMergeSkipReason::DynamicLifecycle,
                FangyuanStaticMergeSkipReason::DynamicRole,
                FangyuanStaticMergeSkipReason::SemanticSocket,
            ]
        );
        assert_eq!(report.stats.authored_primitives, 4);
        assert_eq!(report.stats.expanded_primitives, 4);
        assert_eq!(report.stats.skipped_primitives, 3);
        assert_eq!(report.stats.cube_count, 1);
    }

    #[test]
    fn fangyuan_static_merge_group_skips_invalid_runtime_content() {
        let non_finite = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::new(f32::NAN, 1.0, 0.0),
            Vec3::ONE,
            Color::WHITE,
            FangyuanPrimitiveRole::Structure,
            1.0,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
            FangyuanPrimitiveLifecycle::empty(),
        );
        let invalid_scale = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::Y,
            Vec3::new(1.0, 0.0, 1.0),
            Color::WHITE,
            FangyuanPrimitiveRole::Structure,
            1.0,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
            FangyuanPrimitiveLifecycle::empty(),
        );
        let invalid_alpha = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::Y,
            Vec3::ONE,
            Color::WHITE,
            FangyuanPrimitiveRole::Structure,
            1.25,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
            FangyuanPrimitiveLifecycle::empty(),
        );
        let invalid_material = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::Y,
            Vec3::ONE,
            Color::WHITE,
            FangyuanPrimitiveRole::Structure,
            1.0,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            Some("bad material".to_string()),
            FangyuanPrimitiveLifecycle::empty(),
        );

        let report = fangyuan_static_merge_groups_from_inputs(vec![
            input(non_finite, 0),
            input(invalid_scale, 1),
            input(invalid_alpha, 2),
            input(invalid_material, 3),
        ]);

        assert!(report.groups.is_empty());
        assert_eq!(
            skipped_reasons(&report),
            vec![
                FangyuanStaticMergeSkipReason::NonFiniteTransform,
                FangyuanStaticMergeSkipReason::NonPositiveScale,
                FangyuanStaticMergeSkipReason::InvalidAlpha,
                FangyuanStaticMergeSkipReason::InvalidMaterialProfile,
            ]
        );
    }

    #[test]
    fn fangyuan_static_merge_group_keeps_transparent_primitives_in_separate_groups() {
        let opaque = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Decoration,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.5, 0.5, 0.5, 1.0],
            Some("shared"),
        );
        let transparent = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::Y,
            Vec3::ONE,
            Color::srgba(0.5, 0.5, 0.5, 1.0),
            FangyuanPrimitiveRole::Decoration,
            0.5,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            Some("shared".to_string()),
            FangyuanPrimitiveLifecycle::empty(),
        );

        let report =
            fangyuan_static_merge_groups_from_inputs(vec![input(transparent, 1), input(opaque, 0)]);

        assert_eq!(report.skipped.len(), 0);
        assert_eq!(report.groups.len(), 2);
        assert_eq!(
            report
                .groups
                .iter()
                .map(|group| group.key.transparent_path)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                FangyuanStaticMergeTransparentPath::Opaque,
                FangyuanStaticMergeTransparentPath::Transparent,
            ])
        );
        assert_eq!(report.stats.cube_count, 2);
        assert_eq!(report.stats.material_profile_count, 1);
    }

    #[test]
    fn fangyuan_static_merge_group_records_layout_source_location() {
        let layout = valid_layout(vec![valid_instance("wall", Some("wall_a"))]);
        let mut cube = valid_primitive_blueprint(FangyuanPrimitiveKind::Cube);
        cube.material_profile_id = Some("stone".to_string());
        let mut sphere = valid_primitive_blueprint(FangyuanPrimitiveKind::Sphere);
        sphere.role = Some(FangyuanPrimitiveRole::Warning);
        sphere.material_profile_id = Some("warn".to_string());
        let palette = valid_palette(vec![valid_prefab("wall", vec![cube, sphere])]);

        let report = fangyuan_static_merge_groups_from_layout(
            &layout,
            &palette,
            Some("fangyuan/layouts/test_layout.ron".to_string()),
        )
        .unwrap();

        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].source_refs.len(), 1);
        let source = &report.groups[0].source_refs[0];
        assert_eq!(
            source.source_kind,
            FangyuanStaticMergeSourceKind::SceneLayout
        );
        assert_eq!(
            source.source_path.as_deref(),
            Some("fangyuan/layouts/test_layout.ron")
        );
        assert_eq!(source.layout_instance.as_ref().unwrap().index, 0);
        assert_eq!(
            source.layout_instance.as_ref().unwrap().id.as_deref(),
            Some("wall_a")
        );
        assert_eq!(source.prefab_id.as_deref(), Some("wall"));
        assert_eq!(source.primitive_index, 0);
        assert_eq!(
            source.field_path.as_deref(),
            Some("instances[0].prefab.primitives[0]")
        );
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(
            report.skipped[0].reason,
            FangyuanStaticMergeSkipReason::DynamicRole
        );
        assert_eq!(report.stats.authored_primitives, 2);
        assert_eq!(report.stats.expanded_primitives, 2);
        assert_eq!(report.stats.skipped_primitives, 1);
        assert_eq!(report.stats.estimated_vertex_count, 24);
        assert_eq!(report.stats.estimated_index_count, 36);
    }

    fn input(primitive: FangyuanPrimitive, primitive_index: usize) -> FangyuanStaticMergeInput {
        FangyuanStaticMergeInput::new(
            primitive,
            FangyuanStaticMergeSourceRef::runtime_primitive_set(None, primitive_index),
        )
    }

    fn skipped_reasons(
        report: &FangyuanStaticMergeBuildReport,
    ) -> Vec<FangyuanStaticMergeSkipReason> {
        report
            .skipped
            .iter()
            .map(|skipped| skipped.reason)
            .collect()
    }

    fn static_primitive(
        kind: FangyuanPrimitiveKind,
        role: FangyuanPrimitiveRole,
        position: [f32; 3],
        scale: [f32; 3],
        color: [f32; 4],
        material_profile_id: Option<&str>,
    ) -> FangyuanPrimitive {
        FangyuanPrimitive::with_runtime_metadata(
            kind,
            Vec3::from_array(position),
            Vec3::from_array(scale),
            Color::srgba(color[0], color[1], color[2], color[3]),
            role,
            color[3],
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            material_profile_id.map(str::to_string),
            FangyuanPrimitiveLifecycle::empty(),
        )
    }

    fn valid_layout(instances: Vec<FangyuanSceneLayoutInstance>) -> FangyuanSceneLayout {
        FangyuanSceneLayout {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "static_merge_layout".to_string(),
            description: String::new(),
            bounds: FangyuanBlueprintBounds::new(10.0, 10.0, 8.0),
            palette: Some("fangyuan/palettes/test.ron".to_string()),
            palettes: Vec::new(),
            max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            instances,
        }
    }

    fn valid_instance(prefab: &str, id: Option<&str>) -> FangyuanSceneLayoutInstance {
        FangyuanSceneLayoutInstance {
            id: id.map(str::to_string),
            name: None,
            prefab: prefab.to_string(),
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            tags: Vec::new(),
        }
    }

    fn valid_palette(prefabs: Vec<FangyuanPrefabDefinition>) -> FangyuanPrefabPalette {
        FangyuanPrefabPalette {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "static_merge_palette".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            bounds: FangyuanBlueprintBounds::new(8.0, 8.0, 8.0),
            prefabs,
        }
    }

    fn valid_prefab(
        id: &str,
        primitives: Vec<FangyuanPrimitiveBlueprint>,
    ) -> FangyuanPrefabDefinition {
        FangyuanPrefabDefinition {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            bounds: None,
            pivot: None,
            tags: Vec::new(),
            max_primitives: None,
            primitives,
        }
    }

    fn valid_primitive_blueprint(kind: FangyuanPrimitiveKind) -> FangyuanPrimitiveBlueprint {
        let role = match kind {
            FangyuanPrimitiveKind::Cube => FangyuanPrimitiveRole::Structure,
            FangyuanPrimitiveKind::Sphere => FangyuanPrimitiveRole::Decoration,
        };
        let mut primitive = FangyuanPrimitiveBlueprint::new(
            kind,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.4, 0.6, 1.0],
        );
        primitive.role = Some(role);
        primitive
    }
}
