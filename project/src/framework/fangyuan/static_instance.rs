use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{
    FANGYUAN_STATIC_MERGE_DEFAULT_MATERIAL_PROFILE, FangyuanBlueprintValidationError,
    FangyuanPrefabPalette, FangyuanPrimitive, FangyuanPrimitiveKind, FangyuanPrimitiveSet,
    FangyuanSceneLayout, FangyuanSceneLayoutCompileError, FangyuanSceneLayoutValidationError,
    FangyuanStaticMergeInput, FangyuanStaticMergeLayoutInstanceRef, FangyuanStaticMergeSkipReason,
    FangyuanStaticMergeSkippedPrimitive, FangyuanStaticMergeSourceKind,
    FangyuanStaticMergeSourceRef, FangyuanStaticMergeTransparentPath,
    compile_blueprint_primitive_to_runtime, fangyuan_static_merge_skip_reason,
    transform_prefab_primitive, validate_blueprint_primitive,
};

const FANGYUAN_STATIC_INSTANCE_HASH_OFFSET: u64 = 0xcbf29ce484222325;
const FANGYUAN_STATIC_INSTANCE_HASH_PRIME: u64 = 0x100000001b3;

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstance {
    pub position: Vec3,
    pub scale: Vec3,
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
    pub material_profile_id: Option<String>,
    pub source: FangyuanStaticMergeSourceRef,
}

impl FangyuanStaticInstance {
    pub fn from_primitive(
        primitive: &FangyuanPrimitive,
        source: FangyuanStaticMergeSourceRef,
    ) -> Self {
        Self {
            position: primitive.local_position(),
            scale: primitive.scale(),
            color: primitive.color(),
            alpha: primitive.alpha(),
            emissive: primitive.emissive(),
            material_profile_id: primitive.material_profile_id().map(str::to_string),
            source,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstanceBatch {
    pub key: FangyuanStaticInstanceBatchKey,
    pub instances: Vec<FangyuanStaticInstance>,
    pub buffer_source: FangyuanStaticInstanceBufferSource,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstanceBufferSource {
    pub kind: FangyuanPrimitiveKind,
    pub material_profile: String,
    pub transparent_path: FangyuanStaticMergeTransparentPath,
    pub bounds: FangyuanStaticInstanceBounds,
    pub instance_count: usize,
    pub hash: u64,
    pub source_refs: Vec<FangyuanStaticMergeSourceRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanStaticInstanceBatchKey {
    pub primitive_kind: FangyuanPrimitiveKind,
    pub material_profile: String,
    pub transparent_path: FangyuanStaticMergeTransparentPath,
}

impl FangyuanStaticInstanceBatchKey {
    /// Color and emissive are per-instance attributes. They are intentionally
    /// excluded from the batch key, but included in the buffer hash.
    pub fn from_primitive(primitive: &FangyuanPrimitive) -> Self {
        Self {
            primitive_kind: primitive.kind(),
            material_profile: primitive
                .material_profile_id()
                .map(str::to_string)
                .unwrap_or_else(|| FANGYUAN_STATIC_MERGE_DEFAULT_MATERIAL_PROFILE.to_string()),
            transparent_path: FangyuanStaticMergeTransparentPath::from_alpha(primitive.alpha()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanStaticInstanceBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl FangyuanStaticInstanceBounds {
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn include_instance(&mut self, instance: &FangyuanStaticInstance) {
        let half = instance.scale * 0.5;
        self.include_point(instance.position - half);
        self.include_point(instance.position + half);
    }

    pub fn include_point(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    pub fn size(&self) -> Vec3 {
        if self.is_empty() {
            Vec3::ZERO
        } else {
            self.max - self.min
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }
}

impl Default for FangyuanStaticInstanceBounds {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstanceBuildReport {
    pub batches: Vec<FangyuanStaticInstanceBatch>,
    pub skipped: Vec<FangyuanStaticMergeSkippedPrimitive>,
    pub stats: FangyuanStaticInstanceBuildStats,
    pub cache_key: FangyuanStaticInstanceCacheKey,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanStaticInstanceBuildStats {
    pub authored_primitives: usize,
    pub expanded_primitives: usize,
    pub batch_count: usize,
    pub instance_count: usize,
    pub skipped_primitives: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub material_profile_count: usize,
    pub content_hash: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FangyuanStaticInstanceCacheKey {
    pub hash: u64,
    pub batch_count: usize,
    pub instance_count: usize,
}

impl FangyuanStaticInstanceCacheKey {
    pub fn from_report(report: &FangyuanStaticInstanceBuildReport) -> Self {
        report.cache_key
    }

    pub fn is_dirty_against(&self, previous: Option<&Self>) -> bool {
        fangyuan_static_instance_cache_is_dirty(previous, self)
    }
}

pub fn fangyuan_static_instance_cache_is_dirty(
    previous: Option<&FangyuanStaticInstanceCacheKey>,
    next: &FangyuanStaticInstanceCacheKey,
) -> bool {
    previous.is_none_or(|previous| previous != next)
}

pub fn fangyuan_static_instance_batches_from_inputs(
    inputs: impl IntoIterator<Item = FangyuanStaticMergeInput>,
) -> FangyuanStaticInstanceBuildReport {
    let inputs = inputs.into_iter().collect::<Vec<_>>();
    let mut stats = FangyuanStaticInstanceBuildStats {
        authored_primitives: inputs.len(),
        expanded_primitives: inputs.len(),
        ..Default::default()
    };
    let mut batches_by_key =
        BTreeMap::<FangyuanStaticInstanceBatchKey, Vec<FangyuanStaticInstance>>::new();
    let mut skipped = Vec::new();
    let mut material_profiles = BTreeSet::new();

    for input in inputs {
        let FangyuanStaticMergeInput { primitive, source } = input;
        if let Some(reason) = fangyuan_static_merge_skip_reason(&primitive) {
            skipped.push(FangyuanStaticMergeSkippedPrimitive { source, reason });
            continue;
        }

        let key = FangyuanStaticInstanceBatchKey::from_primitive(&primitive);
        material_profiles.insert(key.material_profile.clone());
        let instance = FangyuanStaticInstance::from_primitive(&primitive, source);
        batches_by_key.entry(key).or_default().push(instance);
    }

    let batches = batches_by_key
        .into_iter()
        .map(|(key, mut instances)| {
            instances.sort_by(|left, right| left.source.cmp(&right.source));
            let mut bounds = FangyuanStaticInstanceBounds::empty();
            for instance in &instances {
                bounds.include_instance(instance);
            }
            let source_refs = instances
                .iter()
                .map(|instance| instance.source.clone())
                .collect::<Vec<_>>();
            let hash = hash_static_instance_batch(&key, &instances);
            let instance_count = instances.len();

            match key.primitive_kind {
                FangyuanPrimitiveKind::Cube => stats.cube_count += instance_count,
                FangyuanPrimitiveKind::Sphere => stats.sphere_count += instance_count,
            }
            stats.instance_count += instance_count;

            let buffer_source = FangyuanStaticInstanceBufferSource {
                kind: key.primitive_kind,
                material_profile: key.material_profile.clone(),
                transparent_path: key.transparent_path,
                bounds,
                instance_count,
                hash,
                source_refs,
            };

            FangyuanStaticInstanceBatch {
                key,
                instances,
                buffer_source,
            }
        })
        .collect::<Vec<_>>();

    stats.batch_count = batches.len();
    stats.skipped_primitives = skipped.len();
    stats.material_profile_count = material_profiles.len();
    let cache_key = cache_key_from_batches(&batches);
    stats.content_hash = cache_key.hash;

    FangyuanStaticInstanceBuildReport {
        batches,
        skipped,
        stats,
        cache_key,
    }
}

pub fn fangyuan_static_instance_batches_from_primitive_set(
    primitive_set: &FangyuanPrimitiveSet,
) -> FangyuanStaticInstanceBuildReport {
    fangyuan_static_instance_batches_from_primitive_set_with_source(primitive_set, None::<String>)
}

pub fn fangyuan_static_instance_batches_from_primitive_set_with_source(
    primitive_set: &FangyuanPrimitiveSet,
    source_path: impl Into<Option<String>>,
) -> FangyuanStaticInstanceBuildReport {
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

    fangyuan_static_instance_batches_from_inputs(inputs)
}

pub fn fangyuan_static_instance_batches_from_layout(
    layout: &FangyuanSceneLayout,
    palette: &FangyuanPrefabPalette,
    layout_source_path: impl Into<Option<String>>,
) -> Result<FangyuanStaticInstanceBuildReport, FangyuanSceneLayoutCompileError> {
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
                    source: source_with_audit_context(
                        source,
                        &error,
                        layout_primitive_error_field_path(instance_index, &error),
                    ),
                    reason: FangyuanStaticMergeSkipReason::InvalidScenePrimitive,
                }),
            }
        }
    }

    let mut report = fangyuan_static_instance_batches_from_inputs(inputs);
    report.skipped.extend(skipped);
    report.skipped.sort_by(|left, right| {
        (left.source.clone(), left.reason).cmp(&(right.source.clone(), right.reason))
    });
    report.stats.authored_primitives = authored_primitives;
    report.stats.expanded_primitives = expanded_primitives;
    report.stats.skipped_primitives = report.skipped.len();

    Ok(report)
}

fn cache_key_from_batches(
    batches: &[FangyuanStaticInstanceBatch],
) -> FangyuanStaticInstanceCacheKey {
    let mut hash = hash_start();
    hash_bytes(&mut hash, b"fangyuan_static_instance_report_v1");
    hash_usize(&mut hash, batches.len());
    let mut instance_count = 0usize;
    for batch in batches {
        hash_u64(&mut hash, batch.buffer_source.hash);
        hash_usize(&mut hash, batch.buffer_source.instance_count);
        instance_count += batch.buffer_source.instance_count;
    }

    FangyuanStaticInstanceCacheKey {
        hash,
        batch_count: batches.len(),
        instance_count,
    }
}

fn hash_static_instance_batch(
    key: &FangyuanStaticInstanceBatchKey,
    instances: &[FangyuanStaticInstance],
) -> u64 {
    let mut hash = hash_start();
    hash_bytes(&mut hash, b"fangyuan_static_instance_batch_v1");
    hash_batch_key(&mut hash, key);
    hash_usize(&mut hash, instances.len());
    for instance in instances {
        hash_static_instance(&mut hash, instance);
    }
    hash
}

fn hash_static_instance(hash: &mut u64, instance: &FangyuanStaticInstance) {
    hash_vec3(hash, instance.position);
    hash_vec3(hash, instance.scale);
    let color = instance.color.to_srgba();
    hash_f32(hash, color.red);
    hash_f32(hash, color.green);
    hash_f32(hash, color.blue);
    hash_f32(hash, color.alpha);
    hash_f32(hash, instance.alpha);
    hash_f32(hash, instance.emissive);
    hash_option_str(hash, instance.material_profile_id.as_deref());
    hash_source_ref(hash, &instance.source);
}

fn hash_batch_key(hash: &mut u64, key: &FangyuanStaticInstanceBatchKey) {
    hash_primitive_kind(hash, key.primitive_kind);
    hash_str(hash, &key.material_profile);
    hash_transparent_path(hash, key.transparent_path);
}

fn hash_source_ref(hash: &mut u64, source: &FangyuanStaticMergeSourceRef) {
    hash_source_kind(hash, source.source_kind);
    hash_option_str(hash, source.source_path.as_deref());
    hash_layout_instance_ref(hash, source.layout_instance.as_ref());
    hash_option_str(hash, source.prefab_id.as_deref());
    hash_usize(hash, source.primitive_index);
    hash_option_str(hash, source.field_path.as_deref());
    hash_option_str(hash, source.audit_code.as_deref());
    hash_option_str(hash, source.audit_reason.as_deref());
}

fn hash_layout_instance_ref(
    hash: &mut u64,
    layout_instance: Option<&FangyuanStaticMergeLayoutInstanceRef>,
) {
    match layout_instance {
        Some(layout_instance) => {
            hash_u8(hash, 1);
            hash_usize(hash, layout_instance.index);
            hash_option_str(hash, layout_instance.id.as_deref());
        }
        None => hash_u8(hash, 0),
    }
}

fn hash_source_kind(hash: &mut u64, source_kind: FangyuanStaticMergeSourceKind) {
    hash_u8(
        hash,
        match source_kind {
            FangyuanStaticMergeSourceKind::SceneLayout => 1,
            FangyuanStaticMergeSourceKind::RuntimePrimitiveSet => 2,
            FangyuanStaticMergeSourceKind::Unknown => 0,
        },
    );
}

fn hash_primitive_kind(hash: &mut u64, kind: FangyuanPrimitiveKind) {
    hash_u8(
        hash,
        match kind {
            FangyuanPrimitiveKind::Cube => 1,
            FangyuanPrimitiveKind::Sphere => 2,
        },
    );
}

fn hash_transparent_path(hash: &mut u64, transparent_path: FangyuanStaticMergeTransparentPath) {
    hash_u8(
        hash,
        match transparent_path {
            FangyuanStaticMergeTransparentPath::Opaque => 1,
            FangyuanStaticMergeTransparentPath::Transparent => 2,
        },
    );
}

fn hash_vec3(hash: &mut u64, value: Vec3) {
    hash_f32(hash, value.x);
    hash_f32(hash, value.y);
    hash_f32(hash, value.z);
}

fn hash_option_str(hash: &mut u64, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_u8(hash, 1);
            hash_str(hash, value);
        }
        None => hash_u8(hash, 0),
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_usize(hash, value.len());
    hash_bytes(hash, value.as_bytes());
}

fn hash_usize(hash: &mut u64, value: usize) {
    hash_u64(hash, value as u64);
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_u32(hash: &mut u64, value: u32) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_u8(hash: &mut u64, value: u8) {
    hash_bytes(hash, &[value]);
}

fn hash_f32(hash: &mut u64, value: f32) {
    hash_u32(hash, canonical_f32_bits(value));
}

fn hash_start() -> u64 {
    FANGYUAN_STATIC_INSTANCE_HASH_OFFSET
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FANGYUAN_STATIC_INSTANCE_HASH_PRIME);
    }
}

fn canonical_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

fn source_with_audit_context(
    mut source: FangyuanStaticMergeSourceRef,
    error: &FangyuanBlueprintValidationError,
    field_path: String,
) -> FangyuanStaticMergeSourceRef {
    source.audit_code = Some(error.code().to_string());
    source.audit_reason = Some(error.reason());
    source.field_path = Some(field_path);
    source
}

fn layout_primitive_error_field_path(
    instance_index: usize,
    error: &FangyuanBlueprintValidationError,
) -> String {
    let prefab_field_path = error.field_path().into_owned();
    format!("instances[{instance_index}].prefab.{prefab_field_path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
        FANGYUAN_SCENE_LAYOUT_VERSION, FangyuanBlueprintBounds, FangyuanPrefabDefinition,
        FangyuanPrimitiveBlueprint, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
        FangyuanSceneLayoutInstance,
    };

    #[test]
    fn fangyuan_static_instance_preserves_runtime_fields_bounds_and_source_refs() {
        let first = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [2.0, 3.0, -1.0],
            [2.0, 4.0, 6.0],
            [0.2, 0.4, 0.6, 1.0],
            0.75,
            1.5,
            Some("stone/wall"),
        );
        let second = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Archive,
            [-2.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.8, 0.1, 0.2, 1.0],
            0.75,
            3.0,
            Some("stone/wall"),
        );
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![second, first]);

        let report = fangyuan_static_instance_batches_from_primitive_set_with_source(
            &primitive_set,
            Some("runtime/home.ron".to_string()),
        );

        assert_eq!(report.batches.len(), 1);
        assert_eq!(report.stats.instance_count, 2);
        assert_eq!(report.stats.cube_count, 2);
        assert_eq!(report.stats.material_profile_count, 1);

        let batch = &report.batches[0];
        assert_eq!(batch.key.primitive_kind, FangyuanPrimitiveKind::Cube);
        assert_eq!(batch.key.material_profile, "stone/wall");
        assert_eq!(
            batch.key.transparent_path,
            FangyuanStaticMergeTransparentPath::Transparent
        );
        assert_eq!(batch.buffer_source.kind, FangyuanPrimitiveKind::Cube);
        assert_eq!(batch.buffer_source.material_profile, "stone/wall");
        assert_eq!(
            batch.buffer_source.transparent_path,
            FangyuanStaticMergeTransparentPath::Transparent
        );
        assert_eq!(batch.buffer_source.instance_count, 2);
        assert_eq!(batch.buffer_source.bounds.min, Vec3::new(-2.5, 0.5, -4.0));
        assert_eq!(batch.buffer_source.bounds.max, Vec3::new(3.0, 5.0, 2.0));
        assert_eq!(batch.buffer_source.bounds.size(), Vec3::new(5.5, 4.5, 6.0));

        let instance = &batch.instances[1];
        assert_eq!(instance.position, Vec3::new(2.0, 3.0, -1.0));
        assert_eq!(instance.scale, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(instance.color.to_srgba().red, 0.2);
        assert_eq!(instance.alpha, 0.75);
        assert_eq!(instance.emissive, 1.5);
        assert_eq!(instance.material_profile_id.as_deref(), Some("stone/wall"));
        assert_eq!(
            instance.source.source_kind,
            FangyuanStaticMergeSourceKind::RuntimePrimitiveSet
        );
        assert_eq!(
            instance.source.source_path.as_deref(),
            Some("runtime/home.ron")
        );
        assert_eq!(instance.source.primitive_index, 1);
        assert_eq!(
            batch
                .buffer_source
                .source_refs
                .iter()
                .map(|source| source.primitive_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn fangyuan_static_instance_batch_key_separates_kind_transparency_and_profile() {
        let shared_opaque_red = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            1.0,
            0.0,
            Some("shared"),
        );
        let shared_opaque_blue_emissive = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            1.0,
            2.5,
            Some("shared"),
        );
        let shared_transparent = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [4.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            0.5,
            0.0,
            Some("shared"),
        );
        let other_profile = static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [6.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0, 1.0],
            1.0,
            0.0,
            Some("other"),
        );
        let sphere = static_primitive(
            FangyuanPrimitiveKind::Sphere,
            FangyuanPrimitiveRole::Decoration,
            [8.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0, 1.0],
            1.0,
            0.0,
            Some("shared"),
        );

        let report = fangyuan_static_instance_batches_from_inputs(vec![
            input(shared_transparent, 2),
            input(sphere, 4),
            input(shared_opaque_blue_emissive, 1),
            input(other_profile, 3),
            input(shared_opaque_red, 0),
        ]);

        assert_eq!(report.batches.len(), 4);
        let shared_opaque = report
            .batches
            .iter()
            .find(|batch| {
                batch.key.primitive_kind == FangyuanPrimitiveKind::Cube
                    && batch.key.material_profile == "shared"
                    && batch.key.transparent_path == FangyuanStaticMergeTransparentPath::Opaque
            })
            .unwrap();
        assert_eq!(shared_opaque.instances.len(), 2);
        assert_eq!(shared_opaque.instances[0].color.to_srgba().red, 1.0);
        assert_eq!(shared_opaque.instances[1].emissive, 2.5);
        assert_eq!(report.stats.material_profile_count, 2);
    }

    #[test]
    fn fangyuan_static_instance_sorts_instances_stably_by_source() {
        let inputs = vec![
            input(static_cube_at([3.0, 1.0, 0.0]), 3),
            input(static_cube_at([1.0, 1.0, 0.0]), 1),
            input(static_cube_at([2.0, 1.0, 0.0]), 2),
        ];

        let report = fangyuan_static_instance_batches_from_inputs(inputs);

        assert_eq!(report.batches.len(), 1);
        assert_eq!(
            report.batches[0]
                .instances
                .iter()
                .map(|instance| instance.source.primitive_index)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn fangyuan_static_instance_from_layout_records_source_location() {
        let layout = valid_layout(vec![valid_instance("wall", Some("wall_a"))]);
        let mut cube = valid_primitive_blueprint(FangyuanPrimitiveKind::Cube);
        cube.material_profile_id = Some("stone".to_string());
        let mut sphere = valid_primitive_blueprint(FangyuanPrimitiveKind::Sphere);
        sphere.role = Some(FangyuanPrimitiveRole::Warning);
        sphere.material_profile_id = Some("warn".to_string());
        let palette = valid_palette(vec![valid_prefab("wall", vec![cube, sphere])]);

        let report = fangyuan_static_instance_batches_from_layout(
            &layout,
            &palette,
            Some("fangyuan/layouts/test_layout.ron".to_string()),
        )
        .unwrap();

        assert_eq!(report.batches.len(), 1);
        assert_eq!(report.batches[0].instances.len(), 1);
        let source = &report.batches[0].instances[0].source;
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
    }

    #[test]
    fn fangyuan_static_instance_filter_matches_static_merge_boundary() {
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
        let warning = static_role(FangyuanPrimitiveRole::Warning);
        let trail = static_role(FangyuanPrimitiveRole::Trail);
        let impact = static_role(FangyuanPrimitiveRole::Impact);
        let socket = static_role(FangyuanPrimitiveRole::Socket);
        let invalid_position = FangyuanPrimitive::with_runtime_metadata(
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
        let valid = static_cube_at([0.0, 1.0, 0.0]);

        let report = fangyuan_static_instance_batches_from_inputs(vec![
            input(dynamic, 0),
            input(warning, 1),
            input(trail, 2),
            input(impact, 3),
            input(socket, 4),
            input(invalid_position, 5),
            input(valid, 6),
        ]);

        assert_eq!(report.batches.len(), 1);
        assert_eq!(report.stats.instance_count, 1);
        assert_eq!(
            report
                .skipped
                .iter()
                .map(|skipped| skipped.reason)
                .collect::<Vec<_>>(),
            vec![
                FangyuanStaticMergeSkipReason::DynamicLifecycle,
                FangyuanStaticMergeSkipReason::DynamicRole,
                FangyuanStaticMergeSkipReason::DynamicRole,
                FangyuanStaticMergeSkipReason::DynamicRole,
                FangyuanStaticMergeSkipReason::SemanticSocket,
                FangyuanStaticMergeSkipReason::NonFiniteTransform,
            ]
        );
    }

    #[test]
    fn fangyuan_static_instance_cache_reuses_same_content_and_rebuilds_on_change() {
        let first = FangyuanPrimitiveSet::from_primitives(vec![static_cube_at([0.0, 1.0, 0.0])]);
        let same = FangyuanPrimitiveSet::from_primitives(vec![static_cube_at([0.0, 1.0, 0.0])]);
        let changed = FangyuanPrimitiveSet::from_primitives(vec![static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            [0.0, 1.0, 0.0],
            [1.5, 1.0, 1.0],
            [0.25, 0.5, 0.75, 1.0],
            1.0,
            0.0,
            None,
        )]);

        let first_report = fangyuan_static_instance_batches_from_primitive_set(&first);
        let same_report = fangyuan_static_instance_batches_from_primitive_set(&same);
        let changed_report = fangyuan_static_instance_batches_from_primitive_set(&changed);

        assert_eq!(first_report.cache_key, same_report.cache_key);
        assert_eq!(
            first_report.batches[0].buffer_source.hash,
            same_report.batches[0].buffer_source.hash
        );
        assert!(!fangyuan_static_instance_cache_is_dirty(
            Some(&first_report.cache_key),
            &same_report.cache_key,
        ));
        assert_ne!(first_report.cache_key, changed_report.cache_key);
        assert_ne!(
            first_report.batches[0].buffer_source.hash,
            changed_report.batches[0].buffer_source.hash
        );
        assert!(
            changed_report
                .cache_key
                .is_dirty_against(Some(&first_report.cache_key))
        );
    }

    fn input(primitive: FangyuanPrimitive, primitive_index: usize) -> FangyuanStaticMergeInput {
        FangyuanStaticMergeInput::new(
            primitive,
            FangyuanStaticMergeSourceRef::runtime_primitive_set(None, primitive_index),
        )
    }

    fn static_cube_at(position: [f32; 3]) -> FangyuanPrimitive {
        static_primitive(
            FangyuanPrimitiveKind::Cube,
            FangyuanPrimitiveRole::Structure,
            position,
            [1.0, 1.0, 1.0],
            [0.25, 0.5, 0.75, 1.0],
            1.0,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
        )
    }

    fn static_role(role: FangyuanPrimitiveRole) -> FangyuanPrimitive {
        static_primitive(
            FangyuanPrimitiveKind::Cube,
            role,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
        )
    }

    fn static_primitive(
        kind: FangyuanPrimitiveKind,
        role: FangyuanPrimitiveRole,
        position: [f32; 3],
        scale: [f32; 3],
        color: [f32; 4],
        alpha: f32,
        emissive: f32,
        material_profile_id: Option<&str>,
    ) -> FangyuanPrimitive {
        FangyuanPrimitive::with_runtime_metadata(
            kind,
            Vec3::from_array(position),
            Vec3::from_array(scale),
            Color::srgba(color[0], color[1], color[2], color[3]),
            role,
            alpha,
            emissive,
            material_profile_id.map(str::to_string),
            FangyuanPrimitiveLifecycle::empty(),
        )
    }

    fn valid_layout(instances: Vec<FangyuanSceneLayoutInstance>) -> FangyuanSceneLayout {
        FangyuanSceneLayout {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "static_instance_layout".to_string(),
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
            name: "static_instance_palette".to_string(),
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
