use bevy::prelude::*;

use super::{
    FANGYUAN_RENDER_UNIT_SPHERE_SECTORS, FANGYUAN_RENDER_UNIT_SPHERE_STACKS, FangyuanPrimitiveKind,
    FangyuanPrimitiveSet, FangyuanStaticInstanceBatch, FangyuanStaticInstanceBatchKey,
    FangyuanStaticInstanceBounds, FangyuanStaticInstanceBufferSource,
    FangyuanStaticInstanceBuildStats, FangyuanStaticInstanceCacheKey, FangyuanStaticMergeSourceRef,
    fangyuan_static_instance_batches_from_primitive_set_with_source,
};

pub const FANGYUAN_STATIC_INSTANCE_RENDER_POSITION_BYTES: usize = 3 * size_of::<f32>();
pub const FANGYUAN_STATIC_INSTANCE_RENDER_SCALE_BYTES: usize = 3 * size_of::<f32>();
pub const FANGYUAN_STATIC_INSTANCE_RENDER_COLOR_BYTES: usize = 4 * size_of::<f32>();
pub const FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES: usize =
    FANGYUAN_STATIC_INSTANCE_RENDER_POSITION_BYTES
        + FANGYUAN_STATIC_INSTANCE_RENDER_SCALE_BYTES
        + FANGYUAN_STATIC_INSTANCE_RENDER_COLOR_BYTES;
pub const FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_BATCHES: usize = 256;
pub const FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_INSTANCES: usize = 20_000;
pub const FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_SPHERE_INSTANCES: usize = 4_096;
pub const FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_BUFFER_BYTES: usize =
    FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_INSTANCES
        * FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStaticInstanceRenderOptions {
    pub allow_cube: bool,
    pub allow_sphere: bool,
    pub max_batches: usize,
    pub max_instances: usize,
    pub max_sphere_instances: usize,
    pub max_buffer_bytes: usize,
}

impl Default for FangyuanStaticInstanceRenderOptions {
    fn default() -> Self {
        Self {
            allow_cube: true,
            allow_sphere: true,
            max_batches: FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_BATCHES,
            max_instances: FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_INSTANCES,
            max_sphere_instances: FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_SPHERE_INSTANCES,
            max_buffer_bytes: FANGYUAN_STATIC_INSTANCE_RENDER_DEFAULT_MAX_BUFFER_BYTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstanceRenderReport {
    pub batches: Vec<FangyuanStaticInstanceRenderBatch>,
    pub stats: FangyuanStaticInstanceRenderStats,
    pub source_stats: FangyuanStaticInstanceBuildStats,
    pub cache_key: FangyuanStaticInstanceCacheKey,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstanceRenderBatch {
    pub key: FangyuanStaticInstanceBatchKey,
    pub buffer_source: FangyuanStaticInstanceBufferSource,
    pub instances: Vec<FangyuanStaticInstanceRenderInstance>,
    pub batch_index: usize,
    pub buffer_bytes: usize,
    pub debug_name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticInstanceRenderInstance {
    pub position: Vec3,
    pub scale: Vec3,
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
    pub material_profile_id: Option<String>,
    pub source: FangyuanStaticMergeSourceRef,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanStaticInstanceRenderStats {
    pub batch_count: usize,
    pub instance_count: usize,
    pub buffer_bytes: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub material_profile_count: usize,
    pub sphere_sectors: u32,
    pub sphere_stacks: u32,
    pub content_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanStaticInstanceRenderError {
    InvalidOptions {
        reason: String,
    },
    UnsupportedKind {
        kind: FangyuanPrimitiveKind,
    },
    BudgetExceeded {
        budget: &'static str,
        actual: usize,
        max: usize,
    },
}

impl std::fmt::Display for FangyuanStaticInstanceRenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions { reason } => {
                write!(
                    formatter,
                    "invalid fangyuan static instance render options: {reason}"
                )
            }
            Self::UnsupportedKind { kind } => {
                write!(
                    formatter,
                    "fangyuan static instance render unsupported kind: {}",
                    kind.as_str()
                )
            }
            Self::BudgetExceeded {
                budget,
                actual,
                max,
            } => write!(
                formatter,
                "fangyuan static instance render budget exceeded: {budget}={actual}/{max}"
            ),
        }
    }
}

impl std::error::Error for FangyuanStaticInstanceRenderError {}

pub fn fangyuan_static_instance_render_report_from_primitive_set_with_source(
    primitive_set: &FangyuanPrimitiveSet,
    source_path: impl Into<Option<String>>,
    options: &FangyuanStaticInstanceRenderOptions,
) -> Result<FangyuanStaticInstanceRenderReport, FangyuanStaticInstanceRenderError> {
    let build_report =
        fangyuan_static_instance_batches_from_primitive_set_with_source(primitive_set, source_path);
    fangyuan_static_instance_render_report_from_batches(
        build_report.batches,
        build_report.stats,
        build_report.cache_key,
        options,
    )
}

pub fn fangyuan_static_instance_render_report_from_batches(
    batches: Vec<FangyuanStaticInstanceBatch>,
    source_stats: FangyuanStaticInstanceBuildStats,
    cache_key: FangyuanStaticInstanceCacheKey,
    options: &FangyuanStaticInstanceRenderOptions,
) -> Result<FangyuanStaticInstanceRenderReport, FangyuanStaticInstanceRenderError> {
    validate_static_instance_render_options(options)?;

    for batch in &batches {
        if !is_kind_supported(batch.key.primitive_kind, options) {
            return Err(FangyuanStaticInstanceRenderError::UnsupportedKind {
                kind: batch.key.primitive_kind,
            });
        }
    }

    let batch_count = batches.len();
    let instance_count = batches
        .iter()
        .map(|batch| batch.instances.len())
        .sum::<usize>();
    let sphere_count = batches
        .iter()
        .filter(|batch| batch.key.primitive_kind == FangyuanPrimitiveKind::Sphere)
        .map(|batch| batch.instances.len())
        .sum::<usize>();
    let buffer_bytes = instance_count * FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES;
    enforce_budget("batches", batch_count, options.max_batches)?;
    enforce_budget("instances", instance_count, options.max_instances)?;
    enforce_budget(
        "sphere_instances",
        sphere_count,
        options.max_sphere_instances,
    )?;
    enforce_budget("buffer_bytes", buffer_bytes, options.max_buffer_bytes)?;

    let render_batches = batches
        .into_iter()
        .enumerate()
        .map(|(batch_index, batch)| render_batch_from_static_batch(batch_index, batch))
        .collect::<Vec<_>>();
    let stats = FangyuanStaticInstanceRenderStats {
        batch_count,
        instance_count,
        buffer_bytes,
        cube_count: source_stats.cube_count,
        sphere_count: source_stats.sphere_count,
        material_profile_count: source_stats.material_profile_count,
        sphere_sectors: FANGYUAN_RENDER_UNIT_SPHERE_SECTORS,
        sphere_stacks: FANGYUAN_RENDER_UNIT_SPHERE_STACKS,
        content_hash: cache_key.hash,
    };

    Ok(FangyuanStaticInstanceRenderReport {
        batches: render_batches,
        stats,
        source_stats,
        cache_key,
    })
}

fn validate_static_instance_render_options(
    options: &FangyuanStaticInstanceRenderOptions,
) -> Result<(), FangyuanStaticInstanceRenderError> {
    if !options.allow_cube && !options.allow_sphere {
        return Err(FangyuanStaticInstanceRenderError::InvalidOptions {
            reason: "at least one primitive kind must be supported".to_string(),
        });
    }
    if options.max_buffer_bytes < FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES
        && options.max_instances > 0
    {
        return Err(FangyuanStaticInstanceRenderError::InvalidOptions {
            reason: format!(
                "max_buffer_bytes must fit at least one instance stride ({})",
                FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES
            ),
        });
    }

    Ok(())
}

fn is_kind_supported(
    kind: FangyuanPrimitiveKind,
    options: &FangyuanStaticInstanceRenderOptions,
) -> bool {
    match kind {
        FangyuanPrimitiveKind::Cube => options.allow_cube,
        FangyuanPrimitiveKind::Sphere => options.allow_sphere,
    }
}

fn enforce_budget(
    budget: &'static str,
    actual: usize,
    max: usize,
) -> Result<(), FangyuanStaticInstanceRenderError> {
    if actual > max {
        return Err(FangyuanStaticInstanceRenderError::BudgetExceeded {
            budget,
            actual,
            max,
        });
    }

    Ok(())
}

fn render_batch_from_static_batch(
    batch_index: usize,
    batch: FangyuanStaticInstanceBatch,
) -> FangyuanStaticInstanceRenderBatch {
    let buffer_bytes = batch.instances.len() * FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES;
    let debug_name = static_instance_render_batch_debug_name(batch_index, &batch);
    let instances = batch
        .instances
        .into_iter()
        .map(|instance| FangyuanStaticInstanceRenderInstance {
            position: instance.position,
            scale: instance.scale,
            color: instance.color.with_alpha(instance.alpha),
            alpha: instance.alpha,
            emissive: instance.emissive,
            material_profile_id: instance.material_profile_id,
            source: instance.source,
        })
        .collect();

    FangyuanStaticInstanceRenderBatch {
        key: batch.key,
        buffer_source: batch.buffer_source,
        instances,
        batch_index,
        buffer_bytes,
        debug_name,
    }
}

fn static_instance_render_batch_debug_name(
    batch_index: usize,
    batch: &FangyuanStaticInstanceBatch,
) -> String {
    format!(
        "fangyuan_static_instance:{}:{}:{}:{}",
        batch_index,
        batch.key.primitive_kind.as_str(),
        batch.key.material_profile,
        batch.buffer_source.hash
    )
}

#[allow(dead_code)]
pub fn fangyuan_static_instance_bounds_are_empty(bounds: FangyuanStaticInstanceBounds) -> bool {
    bounds.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FangyuanPrimitive, FangyuanPrimitiveRole,
        FangyuanStaticMergeTransparentPath,
    };

    #[test]
    fn fangyuan_static_instance_render_report_consumes_position_scale_color_and_buffer_bytes() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![
            primitive(
                FangyuanPrimitiveKind::Cube,
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::new(0.5, 0.75, 1.25),
                Color::srgba(0.1, 0.2, 0.3, 1.0),
                0.6,
            ),
            primitive(
                FangyuanPrimitiveKind::Cube,
                Vec3::new(4.0, 5.0, 6.0),
                Vec3::splat(2.0),
                Color::srgba(0.7, 0.8, 0.9, 1.0),
                1.0,
            ),
        ]);

        let report = fangyuan_static_instance_render_report_from_primitive_set_with_source(
            &primitive_set,
            Some("test.ron".to_string()),
            &FangyuanStaticInstanceRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(report.stats.batch_count, 2);
        assert_eq!(report.stats.instance_count, 2);
        assert_eq!(
            report.stats.buffer_bytes,
            2 * FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES
        );
        let transparent_batch = report
            .batches
            .iter()
            .find(|batch| {
                batch.key.transparent_path == FangyuanStaticMergeTransparentPath::Transparent
            })
            .unwrap();
        assert_eq!(
            transparent_batch.instances[0].position,
            Vec3::new(1.0, 2.0, 3.0)
        );
        assert_eq!(
            transparent_batch.instances[0].scale,
            Vec3::new(0.5, 0.75, 1.25)
        );
        assert_eq!(
            transparent_batch.instances[0].color,
            Color::srgba(0.1, 0.2, 0.3, 0.6)
        );
        assert_eq!(transparent_batch.instances[0].alpha, 0.6);
        assert_eq!(
            transparent_batch.instances[0].emissive,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE
        );
        assert_eq!(transparent_batch.instances[0].material_profile_id, None);
        assert_eq!(
            transparent_batch.buffer_bytes,
            FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES
        );
        assert!(transparent_batch.debug_name.contains("cube"));
    }

    #[test]
    fn fangyuan_static_instance_render_report_records_low_cost_sphere_base_mesh_budget() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![primitive(
            FangyuanPrimitiveKind::Sphere,
            Vec3::ZERO,
            Vec3::ONE,
            Color::srgb(0.2, 0.4, 0.6),
            1.0,
        )]);

        let report = fangyuan_static_instance_render_report_from_primitive_set_with_source(
            &primitive_set,
            None::<String>,
            &FangyuanStaticInstanceRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(report.stats.sphere_count, 1);
        assert_eq!(
            report.stats.sphere_sectors,
            FANGYUAN_RENDER_UNIT_SPHERE_SECTORS
        );
        assert_eq!(
            report.stats.sphere_stacks,
            FANGYUAN_RENDER_UNIT_SPHERE_STACKS
        );
        assert_eq!(
            report.batches[0].key.primitive_kind,
            FangyuanPrimitiveKind::Sphere
        );
    }

    #[test]
    fn fangyuan_static_instance_render_report_rejects_budget_and_unsupported_kind() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![primitive(
            FangyuanPrimitiveKind::Sphere,
            Vec3::ZERO,
            Vec3::ONE,
            Color::WHITE,
            1.0,
        )]);

        let budget_error = fangyuan_static_instance_render_report_from_primitive_set_with_source(
            &primitive_set,
            None::<String>,
            &FangyuanStaticInstanceRenderOptions {
                max_sphere_instances: 0,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            budget_error,
            FangyuanStaticInstanceRenderError::BudgetExceeded {
                budget: "sphere_instances",
                actual: 1,
                max: 0
            }
        ));

        let unsupported_error =
            fangyuan_static_instance_render_report_from_primitive_set_with_source(
                &primitive_set,
                None::<String>,
                &FangyuanStaticInstanceRenderOptions {
                    allow_sphere: false,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert_eq!(
            unsupported_error,
            FangyuanStaticInstanceRenderError::UnsupportedKind {
                kind: FangyuanPrimitiveKind::Sphere
            }
        );
    }

    #[test]
    fn fangyuan_static_instance_render_report_rejects_initialization_options() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![primitive(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::WHITE,
            1.0,
        )]);

        let error = fangyuan_static_instance_render_report_from_primitive_set_with_source(
            &primitive_set,
            None::<String>,
            &FangyuanStaticInstanceRenderOptions {
                allow_cube: false,
                allow_sphere: false,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FangyuanStaticInstanceRenderError::InvalidOptions { .. }
        ));
    }

    fn primitive(
        kind: FangyuanPrimitiveKind,
        position: Vec3,
        scale: Vec3,
        color: Color,
        alpha: f32,
    ) -> FangyuanPrimitive {
        FangyuanPrimitive::with_runtime_metadata(
            kind,
            position,
            scale,
            color,
            FangyuanPrimitiveRole::default_for_kind(kind),
            alpha,
            0.0,
            None,
            Default::default(),
        )
    }
}
