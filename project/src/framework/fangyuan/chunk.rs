use serde::{Deserialize, Deserializer, Serialize};
use std::{borrow::Cow, collections::HashSet, error::Error, fmt};

use super::{
    FANGYUAN_BLUEPRINT_VERSION, FANGYUAN_PREFAB_MAX_TAGS, FangyuanAssetPathError,
    FangyuanPrefabIdInvalidReason, FangyuanPrefabTagInvalidReason, validate_fangyuan_asset_path,
    validate_prefab_id, validate_prefab_tag,
};

pub const FANGYUAN_CHUNK_VERSION: &str = FANGYUAN_BLUEPRINT_VERSION;
pub const FANGYUAN_CHUNK_ID_MAX_LEN: usize = 64;
pub const FANGYUAN_CHUNK_REF_ID_MAX_LEN: usize = 64;
pub const FANGYUAN_CHUNK_REGION_ID_MAX_LEN: usize = 96;
pub const FANGYUAN_CHUNK_ARTIFACT_HASH_MAX_LEN: usize = 128;
pub const FANGYUAN_CHUNK_DATA_VERSION_MAX_LEN: usize = 32;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkManifest {
    pub version: String,
    pub name: String,
    pub description: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub world_id: Option<String>,
    pub chunks: Vec<FangyuanChunkManifestEntry>,
}

impl FangyuanChunkManifest {
    pub fn from_ron_str(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str::<Self>(source)
    }

    pub fn validate(&self) -> Result<(), FangyuanChunkValidationError> {
        if self.version != FANGYUAN_CHUNK_VERSION {
            return Err(FangyuanChunkValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_CHUNK_VERSION,
            });
        }

        if let Some(world_id) = self.world_id.as_deref() {
            validate_namespace_id(world_id, FANGYUAN_CHUNK_REGION_ID_MAX_LEN, true, "world_id")?;
        }

        if self.chunks.is_empty() {
            return Err(FangyuanChunkValidationError::EmptyManifest);
        }

        let mut ids = HashSet::with_capacity(self.chunks.len());
        for (chunk_index, entry) in self.chunks.iter().enumerate() {
            validate_chunk_id(&entry.id).map_err(|reason| {
                FangyuanChunkValidationError::InvalidChunkId {
                    chunk_index: Some(chunk_index),
                    id: entry.id.clone(),
                    reason,
                }
            })?;

            if !ids.insert(entry.id.as_str()) {
                return Err(FangyuanChunkValidationError::DuplicateChunkId {
                    chunk_index,
                    id: entry.id.clone(),
                });
            }

            entry.bounds.validate().map_err(|source| {
                FangyuanChunkValidationError::InvalidChunkBounds {
                    chunk_index: Some(chunk_index),
                    chunk_id: Some(entry.id.clone()),
                    source,
                }
            })?;

            validate_region_metadata(Some(chunk_index), Some(&entry.id), "chunks", &entry.region)?;
            validate_optional_asset_path(
                Some(chunk_index),
                Some(&entry.id),
                "dev_ron",
                entry.dev_ron.as_deref(),
            )?;
            validate_optional_asset_path(
                Some(chunk_index),
                Some(&entry.id),
                "bin",
                entry.bin.as_deref(),
            )?;
            validate_optional_artifact_hash(
                manifest_entry_field_path(chunk_index, "hash"),
                entry.hash.as_deref(),
            )?;
            validate_optional_data_version(
                manifest_entry_field_path(chunk_index, "data_version"),
                entry.data_version.as_deref(),
            )?;
            validate_non_empty_budget_summary(
                Some(chunk_index),
                Some(&entry.id),
                "chunks",
                &entry.budget,
            )?;
            entry
                .budget
                .validate_internal_consistency(manifest_entry_field_path(chunk_index, "budget"))?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkManifestEntry {
    pub id: String,
    pub bounds: FangyuanChunkBounds,
    pub region: FangyuanChunkRegionMetadata,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub dev_ron: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub bin: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub hash: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub data_version: Option<String>,
    pub budget: FangyuanChunkBudgetSummary,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkSource {
    pub version: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub bounds: FangyuanChunkBounds,
    pub region: FangyuanChunkRegionMetadata,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefab_instances: Vec<FangyuanChunkPrefabInstanceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiandao_refs: Vec<FangyuanChunkTiandaoRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub static_decorations: Vec<FangyuanChunkStaticDecorationRef>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub bin: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub hash: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub data_version: Option<String>,
    pub budget: FangyuanChunkBudgetSummary,
}

pub type FangyuanChunkDevRonSource = FangyuanChunkSource;

impl FangyuanChunkSource {
    pub fn from_ron_str(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str::<Self>(source)
    }

    pub fn validate(&self) -> Result<(), FangyuanChunkValidationError> {
        self.validate_internal(None)
    }

    pub fn validate_against_prefab_ids<'a>(
        &self,
        prefab_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), FangyuanChunkValidationError> {
        let prefab_ids = prefab_ids.into_iter().collect::<HashSet<_>>();
        self.validate_internal(Some(&prefab_ids))
    }

    fn validate_internal(
        &self,
        prefab_ids: Option<&HashSet<&str>>,
    ) -> Result<(), FangyuanChunkValidationError> {
        if self.version != FANGYUAN_CHUNK_VERSION {
            return Err(FangyuanChunkValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_CHUNK_VERSION,
            });
        }

        validate_chunk_id(&self.id).map_err(|reason| {
            FangyuanChunkValidationError::InvalidChunkId {
                chunk_index: None,
                id: self.id.clone(),
                reason,
            }
        })?;
        self.bounds.validate().map_err(|source| {
            FangyuanChunkValidationError::InvalidChunkBounds {
                chunk_index: None,
                chunk_id: Some(self.id.clone()),
                source,
            }
        })?;
        validate_region_metadata(None, Some(&self.id), "region", &self.region)?;
        validate_optional_asset_path(None, Some(&self.id), "bin", self.bin.as_deref())?;
        validate_optional_artifact_hash(Cow::Borrowed("hash"), self.hash.as_deref())?;
        validate_optional_data_version(
            Cow::Borrowed("data_version"),
            self.data_version.as_deref(),
        )?;

        let mut local_ids = HashSet::new();
        for (index, prefab_instance) in self.prefab_instances.iter().enumerate() {
            let prefix = format!("prefab_instances[{index}]");
            validate_local_ref_id(
                &prefix,
                &prefab_instance.id,
                &mut local_ids,
                prefab_instance_field_path(index, "id"),
            )?;
            validate_prefab_ref(
                prefab_instance_field_path(index, "prefab"),
                &prefab_instance.prefab,
                prefab_ids,
            )?;
            validate_transform(
                &prefix,
                &prefab_instance.transform,
                &self.bounds,
                FangyuanChunkRefKind::PrefabInstance,
            )?;
            validate_ref_budget_cost(
                prefab_instance_field_path(index, "budget_cost"),
                prefab_instance.budget_cost,
            )?;
        }

        for (index, tiandao_ref) in self.tiandao_refs.iter().enumerate() {
            let prefix = format!("tiandao_refs[{index}]");
            validate_local_ref_id(
                &prefix,
                &tiandao_ref.id,
                &mut local_ids,
                tiandao_ref_field_path(index, "id"),
            )?;
            validate_namespace_id(
                &tiandao_ref.tiandao,
                FANGYUAN_CHUNK_REGION_ID_MAX_LEN,
                true,
                tiandao_ref_field_path(index, "tiandao"),
            )?;
            if let Some(anchor) = tiandao_ref.anchor {
                validate_position_in_bounds(
                    tiandao_ref_field_path(index, "anchor"),
                    anchor,
                    &self.bounds,
                    FangyuanChunkRefKind::Tiandao,
                )?;
            }
            validate_ref_budget_cost(
                tiandao_ref_field_path(index, "budget_cost"),
                tiandao_ref.budget_cost,
            )?;
        }

        for (index, decoration) in self.static_decorations.iter().enumerate() {
            let prefix = format!("static_decorations[{index}]");
            validate_local_ref_id(
                &prefix,
                &decoration.id,
                &mut local_ids,
                static_decoration_field_path(index, "id"),
            )?;
            validate_static_decoration_source(index, &decoration.source, prefab_ids)?;
            validate_transform(
                &prefix,
                &decoration.transform,
                &self.bounds,
                FangyuanChunkRefKind::StaticDecoration,
            )?;
            validate_ref_budget_cost(
                static_decoration_field_path(index, "budget_cost"),
                decoration.budget_cost,
            )?;
        }

        let expected = FangyuanChunkBudgetSummary::from_refs(
            &self.prefab_instances,
            &self.tiandao_refs,
            &self.static_decorations,
        );
        if expected.total_ref_count == 0 {
            return Err(FangyuanChunkValidationError::EmptyChunk {
                field_path: Cow::Borrowed("prefab_instances").into_owned(),
                chunk_id: self.id.clone(),
            });
        }
        self.budget
            .validate_against(expected, Cow::Borrowed("budget"))?;

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkBounds {
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub min: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub max: [f32; 3],
}

impl FangyuanChunkBounds {
    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }

    pub fn validate(&self) -> Result<(), FangyuanChunkBoundsValidationError> {
        for axis in 0..3 {
            let min = self.min[axis];
            let max = self.max[axis];
            if !min.is_finite() {
                return Err(FangyuanChunkBoundsValidationError {
                    axis,
                    min,
                    max,
                    reason: FangyuanChunkBoundsInvalidReason::NonFiniteMin,
                });
            }
            if !max.is_finite() {
                return Err(FangyuanChunkBoundsValidationError {
                    axis,
                    min,
                    max,
                    reason: FangyuanChunkBoundsInvalidReason::NonFiniteMax,
                });
            }
            if min >= max {
                return Err(FangyuanChunkBoundsValidationError {
                    axis,
                    min,
                    max,
                    reason: FangyuanChunkBoundsInvalidReason::MinMustBeLessThanMax,
                });
            }
        }

        Ok(())
    }

    pub fn contains_point(&self, point: [f32; 3]) -> bool {
        point
            .into_iter()
            .enumerate()
            .all(|(axis, value)| value >= self.min[axis] && value <= self.max[axis])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanChunkBoundsValidationError {
    pub axis: usize,
    pub min: f32,
    pub max: f32,
    pub reason: FangyuanChunkBoundsInvalidReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkBoundsInvalidReason {
    NonFiniteMin,
    NonFiniteMax,
    MinMustBeLessThanMax,
}

impl FangyuanChunkBoundsValidationError {
    pub fn field_path(&self) -> Cow<'static, str> {
        match self.reason {
            FangyuanChunkBoundsInvalidReason::NonFiniteMin => {
                Cow::Owned(format!("bounds.min[{}]", self.axis))
            }
            FangyuanChunkBoundsInvalidReason::NonFiniteMax => {
                Cow::Owned(format!("bounds.max[{}]", self.axis))
            }
            FangyuanChunkBoundsInvalidReason::MinMustBeLessThanMax => {
                Cow::Owned(format!("bounds.axis[{}]", self.axis))
            }
        }
    }

    pub fn reason(&self) -> String {
        match self.reason {
            FangyuanChunkBoundsInvalidReason::NonFiniteMin => {
                format!("min[{}] must be finite", self.axis)
            }
            FangyuanChunkBoundsInvalidReason::NonFiniteMax => {
                format!("max[{}] must be finite", self.axis)
            }
            FangyuanChunkBoundsInvalidReason::MinMustBeLessThanMax => format!(
                "min[{}] {} must be less than max[{}] {}",
                self.axis, self.min, self.axis, self.max
            ),
        }
    }
}

impl fmt::Display for FangyuanChunkBoundsValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fangyuan chunk bounds validation error at {}: {}",
            self.field_path(),
            self.reason()
        )
    }
}

impl Error for FangyuanChunkBoundsValidationError {}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkRegionMetadata {
    pub region_id: String,
    pub layer: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkPrefabInstanceRef {
    pub id: String,
    pub prefab: String,
    pub transform: FangyuanChunkTransform,
    pub budget_cost: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkTiandaoRef {
    pub id: String,
    pub tiandao: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f32_array_3",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor: Option<[f32; 3]>,
    pub budget_cost: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkStaticDecorationRef {
    pub id: String,
    pub source: FangyuanChunkStaticDecorationSourceRef,
    pub transform: FangyuanChunkTransform,
    pub budget_cost: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanChunkStaticDecorationSourceRef {
    Prefab { prefab: String },
    Blueprint { blueprint: String },
    Bake { bake: String },
}

impl FangyuanChunkStaticDecorationSourceRef {
    pub fn prefab(prefab: impl Into<String>) -> Self {
        Self::Prefab {
            prefab: prefab.into(),
        }
    }

    pub fn blueprint(blueprint: impl Into<String>) -> Self {
        Self::Blueprint {
            blueprint: blueprint.into(),
        }
    }

    pub fn bake(bake: impl Into<String>) -> Self {
        Self::Bake { bake: bake.into() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkTransform {
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub position: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub scale: [f32; 3],
}

impl FangyuanChunkTransform {
    pub const fn new(position: [f32; 3], scale: [f32; 3]) -> Self {
        Self { position, scale }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanChunkBudgetSummary {
    pub prefab_instance_count: usize,
    pub tiandao_ref_count: usize,
    pub static_decoration_count: usize,
    pub total_ref_count: usize,
    pub prefab_cost: u32,
    pub tiandao_cost: u32,
    pub static_decoration_cost: u32,
    pub total_cost: u32,
}

impl FangyuanChunkBudgetSummary {
    pub fn from_refs(
        prefab_instances: &[FangyuanChunkPrefabInstanceRef],
        tiandao_refs: &[FangyuanChunkTiandaoRef],
        static_decorations: &[FangyuanChunkStaticDecorationRef],
    ) -> Self {
        let prefab_cost = prefab_instances
            .iter()
            .map(|reference| reference.budget_cost)
            .sum();
        let tiandao_cost = tiandao_refs
            .iter()
            .map(|reference| reference.budget_cost)
            .sum();
        let static_decoration_cost = static_decorations
            .iter()
            .map(|reference| reference.budget_cost)
            .sum();

        Self {
            prefab_instance_count: prefab_instances.len(),
            tiandao_ref_count: tiandao_refs.len(),
            static_decoration_count: static_decorations.len(),
            total_ref_count: prefab_instances.len() + tiandao_refs.len() + static_decorations.len(),
            prefab_cost,
            tiandao_cost,
            static_decoration_cost,
            total_cost: prefab_cost + tiandao_cost + static_decoration_cost,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.total_ref_count == 0
    }

    fn validate_internal_consistency(
        &self,
        field_prefix: Cow<'static, str>,
    ) -> Result<(), FangyuanChunkValidationError> {
        let expected_ref_count =
            self.prefab_instance_count + self.tiandao_ref_count + self.static_decoration_count;
        if self.total_ref_count != expected_ref_count {
            return Err(FangyuanChunkValidationError::BudgetSummaryMismatch {
                field_path: owned_field_path(&field_prefix, "total_ref_count"),
                expected: expected_ref_count as u64,
                actual: self.total_ref_count as u64,
            });
        }

        let expected_cost =
            self.prefab_cost as u64 + self.tiandao_cost as u64 + self.static_decoration_cost as u64;
        if self.total_cost as u64 != expected_cost {
            return Err(FangyuanChunkValidationError::BudgetSummaryMismatch {
                field_path: owned_field_path(&field_prefix, "total_cost"),
                expected: expected_cost,
                actual: self.total_cost as u64,
            });
        }

        Ok(())
    }

    fn validate_against(
        &self,
        expected: Self,
        field_prefix: Cow<'static, str>,
    ) -> Result<(), FangyuanChunkValidationError> {
        self.validate_internal_consistency(field_prefix.clone())?;

        macro_rules! check_field {
            ($field:ident) => {
                if self.$field != expected.$field {
                    return Err(FangyuanChunkValidationError::BudgetSummaryMismatch {
                        field_path: owned_field_path(&field_prefix, stringify!($field)),
                        expected: expected.$field as u64,
                        actual: self.$field as u64,
                    });
                }
            };
        }

        check_field!(prefab_instance_count);
        check_field!(tiandao_ref_count);
        check_field!(static_decoration_count);
        check_field!(total_ref_count);
        check_field!(prefab_cost);
        check_field!(tiandao_cost);
        check_field!(static_decoration_cost);
        check_field!(total_cost);

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanChunkValidationError {
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    EmptyManifest,
    InvalidChunkId {
        chunk_index: Option<usize>,
        id: String,
        reason: FangyuanChunkIdInvalidReason,
    },
    DuplicateChunkId {
        chunk_index: usize,
        id: String,
    },
    InvalidChunkBounds {
        chunk_index: Option<usize>,
        chunk_id: Option<String>,
        source: FangyuanChunkBoundsValidationError,
    },
    InvalidNamespaceId {
        field_path: String,
        id: String,
        reason: FangyuanChunkNamespaceIdInvalidReason,
    },
    TooManyRegionTags {
        field_path: String,
        count: usize,
        limit: usize,
    },
    InvalidRegionTag {
        field_path: String,
        tag: String,
        reason: FangyuanPrefabTagInvalidReason,
    },
    InvalidAssetPath {
        field_path: String,
        path: String,
        source: FangyuanAssetPathError,
    },
    InvalidArtifactHash {
        field_path: String,
        hash: String,
        reason: FangyuanChunkArtifactTextInvalidReason,
    },
    InvalidDataVersion {
        field_path: String,
        data_version: String,
        reason: FangyuanChunkArtifactTextInvalidReason,
    },
    InvalidRefId {
        field_path: String,
        id: String,
        reason: FangyuanChunkIdInvalidReason,
    },
    DuplicateRefId {
        field_path: String,
        id: String,
    },
    InvalidPrefabRef {
        field_path: String,
        prefab: String,
        reason: FangyuanPrefabIdInvalidReason,
    },
    MissingPrefabRef {
        field_path: String,
        prefab: String,
    },
    InvalidRefPosition {
        field_path: String,
        kind: FangyuanChunkRefKind,
        axis: usize,
        value: f32,
        min: f32,
        max: f32,
    },
    InvalidRefScale {
        field_path: String,
        kind: FangyuanChunkRefKind,
        axis: usize,
        value: f32,
    },
    InvalidRefBudgetCost {
        field_path: String,
        value: u32,
    },
    EmptyChunk {
        field_path: String,
        chunk_id: String,
    },
    BudgetSummaryMismatch {
        field_path: String,
        expected: u64,
        actual: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkIdInvalidReason {
    Empty,
    TooLong { max_len: usize },
    MustStartWithLowercaseAscii,
    InvalidCharacter,
    PathLike,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkNamespaceIdInvalidReason {
    Empty,
    TooLong { max_len: usize },
    MustStartWithLowercaseAscii,
    InvalidCharacter,
    PathLike,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkArtifactTextInvalidReason {
    Empty,
    TooLong { max_len: usize },
    InvalidCharacter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkRefKind {
    PrefabInstance,
    Tiandao,
    StaticDecoration,
}

impl FangyuanChunkRefKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PrefabInstance => "prefab_instance",
            Self::Tiandao => "tiandao",
            Self::StaticDecoration => "static_decoration",
        }
    }
}

impl FangyuanChunkValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::EmptyManifest => "empty_manifest",
            Self::InvalidChunkId { .. } => "invalid_chunk_id",
            Self::DuplicateChunkId { .. } => "duplicate_chunk_id",
            Self::InvalidChunkBounds { .. } => "invalid_chunk_bounds",
            Self::InvalidNamespaceId { .. } => "invalid_namespace_id",
            Self::TooManyRegionTags { .. } => "too_many_region_tags",
            Self::InvalidRegionTag { .. } => "invalid_region_tag",
            Self::InvalidAssetPath { .. } => "invalid_asset_path",
            Self::InvalidArtifactHash { .. } => "invalid_artifact_hash",
            Self::InvalidDataVersion { .. } => "invalid_data_version",
            Self::InvalidRefId { .. } => "invalid_ref_id",
            Self::DuplicateRefId { .. } => "duplicate_ref_id",
            Self::InvalidPrefabRef { .. } => "invalid_prefab_ref",
            Self::MissingPrefabRef { .. } => "missing_prefab_ref",
            Self::InvalidRefPosition { .. } => "invalid_ref_position",
            Self::InvalidRefScale { .. } => "invalid_ref_scale",
            Self::InvalidRefBudgetCost { .. } => "invalid_ref_budget_cost",
            Self::EmptyChunk { .. } => "empty_chunk",
            Self::BudgetSummaryMismatch { .. } => "budget_summary_mismatch",
        }
    }

    pub fn field_path(&self) -> Cow<'static, str> {
        match self {
            Self::UnsupportedVersion { .. } => Cow::Borrowed("version"),
            Self::EmptyManifest => Cow::Borrowed("chunks"),
            Self::InvalidChunkId {
                chunk_index: Some(chunk_index),
                ..
            }
            | Self::DuplicateChunkId { chunk_index, .. } => {
                Cow::Owned(format!("chunks[{chunk_index}].id"))
            }
            Self::InvalidChunkId {
                chunk_index: None, ..
            } => Cow::Borrowed("id"),
            Self::InvalidChunkBounds {
                chunk_index: Some(chunk_index),
                source,
                ..
            } => Cow::Owned(format!(
                "chunks[{chunk_index}].bounds.{}",
                strip_bounds_prefix(source.field_path())
            )),
            Self::InvalidChunkBounds {
                chunk_index: None,
                source,
                ..
            } => source.field_path(),
            Self::InvalidNamespaceId { field_path, .. }
            | Self::TooManyRegionTags { field_path, .. }
            | Self::InvalidRegionTag { field_path, .. }
            | Self::InvalidAssetPath { field_path, .. }
            | Self::InvalidArtifactHash { field_path, .. }
            | Self::InvalidDataVersion { field_path, .. }
            | Self::InvalidRefId { field_path, .. }
            | Self::DuplicateRefId { field_path, .. }
            | Self::InvalidPrefabRef { field_path, .. }
            | Self::MissingPrefabRef { field_path, .. }
            | Self::InvalidRefPosition { field_path, .. }
            | Self::InvalidRefScale { field_path, .. }
            | Self::InvalidRefBudgetCost { field_path, .. }
            | Self::EmptyChunk { field_path, .. }
            | Self::BudgetSummaryMismatch { field_path, .. } => Cow::Owned(field_path.clone()),
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                format!("version `{found}` is unsupported; expected `{expected}`")
            }
            Self::EmptyManifest => "at least one chunk entry is required".to_string(),
            Self::InvalidChunkId { id, reason, .. } => chunk_id_reason("chunk id", id, *reason),
            Self::DuplicateChunkId { id, .. } => {
                format!("chunk id `{id}` is already used by an earlier chunk")
            }
            Self::InvalidChunkBounds { source, .. } => source.reason(),
            Self::InvalidNamespaceId { id, reason, .. } => namespace_id_reason("id", id, *reason),
            Self::TooManyRegionTags { count, limit, .. } => {
                format!("contains {count} tags, exceeding limit {limit}")
            }
            Self::InvalidRegionTag { tag, reason, .. } => prefab_tag_reason(tag, *reason),
            Self::InvalidAssetPath { path, source, .. } => {
                format!("asset path `{path}` is invalid: {source}")
            }
            Self::InvalidArtifactHash { hash, reason, .. } => {
                artifact_text_reason("hash", hash, *reason)
            }
            Self::InvalidDataVersion {
                data_version,
                reason,
                ..
            } => artifact_text_reason("data_version", data_version, *reason),
            Self::InvalidRefId { id, reason, .. } => chunk_id_reason("ref id", id, *reason),
            Self::DuplicateRefId { id, .. } => {
                format!("ref id `{id}` is already used inside this chunk")
            }
            Self::InvalidPrefabRef { prefab, reason, .. } => prefab_id_reason(prefab, *reason),
            Self::MissingPrefabRef { prefab, .. } => {
                format!("prefab `{prefab}` is not present in the available prefab ids")
            }
            Self::InvalidRefPosition {
                kind,
                axis,
                value,
                min,
                max,
                ..
            } => format!(
                "{} position axis {axis} value {value} must be finite and within {min}..={max}",
                kind.as_str()
            ),
            Self::InvalidRefScale {
                kind, axis, value, ..
            } => format!(
                "{} scale axis {axis} value {value} must be finite and greater than 0",
                kind.as_str()
            ),
            Self::InvalidRefBudgetCost { value, .. } => {
                format!("budget_cost {value} must be greater than 0")
            }
            Self::EmptyChunk { chunk_id, .. } => {
                format!("chunk `{chunk_id}` contains no prefab, tiandao, or static decoration refs")
            }
            Self::BudgetSummaryMismatch {
                expected, actual, ..
            } => {
                format!("budget summary value {actual} does not match expected {expected}")
            }
        }
    }
}

impl fmt::Display for FangyuanChunkValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fangyuan chunk validation error [{}] at {}: {}",
            self.code(),
            self.field_path(),
            self.reason()
        )
    }
}

impl Error for FangyuanChunkValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidAssetPath { source, .. } => Some(source),
            Self::InvalidChunkBounds { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub fn validate_fangyuan_chunk_id(id: &str) -> Result<(), FangyuanChunkIdInvalidReason> {
    validate_chunk_id(id)
}

fn validate_chunk_id(id: &str) -> Result<(), FangyuanChunkIdInvalidReason> {
    validate_ascii_id(id, FANGYUAN_CHUNK_ID_MAX_LEN, false)
}

fn validate_ref_id(id: &str) -> Result<(), FangyuanChunkIdInvalidReason> {
    validate_ascii_id(id, FANGYUAN_CHUNK_REF_ID_MAX_LEN, true)
}

fn validate_ascii_id(
    id: &str,
    max_len: usize,
    allow_hyphen: bool,
) -> Result<(), FangyuanChunkIdInvalidReason> {
    if id.is_empty() {
        return Err(FangyuanChunkIdInvalidReason::Empty);
    }

    if id.len() > max_len {
        return Err(FangyuanChunkIdInvalidReason::TooLong { max_len });
    }

    if id.contains('/') || id.contains('\\') || id.contains('.') || id.contains(':') {
        return Err(FangyuanChunkIdInvalidReason::PathLike);
    }

    let mut chars = id.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return Err(FangyuanChunkIdInvalidReason::MustStartWithLowercaseAscii);
    }

    if chars.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '_'
            || (allow_hyphen && character == '-')
    }) {
        Ok(())
    } else {
        Err(FangyuanChunkIdInvalidReason::InvalidCharacter)
    }
}

fn validate_namespace_id(
    id: &str,
    max_len: usize,
    allow_dot: bool,
    field_path: impl Into<String>,
) -> Result<(), FangyuanChunkValidationError> {
    let field_path = field_path.into();
    let reason = if id.is_empty() {
        Some(FangyuanChunkNamespaceIdInvalidReason::Empty)
    } else if id.len() > max_len {
        Some(FangyuanChunkNamespaceIdInvalidReason::TooLong { max_len })
    } else if id.contains('/') || id.contains('\\') || id.contains(':') || id.contains("..") {
        Some(FangyuanChunkNamespaceIdInvalidReason::PathLike)
    } else {
        let mut chars = id.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_lowercase() {
            Some(FangyuanChunkNamespaceIdInvalidReason::MustStartWithLowercaseAscii)
        } else if chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
                || (allow_dot && character == '.')
        }) {
            None
        } else {
            Some(FangyuanChunkNamespaceIdInvalidReason::InvalidCharacter)
        }
    };

    if let Some(reason) = reason {
        Err(FangyuanChunkValidationError::InvalidNamespaceId {
            field_path,
            id: id.to_string(),
            reason,
        })
    } else {
        Ok(())
    }
}

fn validate_region_metadata(
    chunk_index: Option<usize>,
    chunk_id: Option<&str>,
    root: &str,
    region: &FangyuanChunkRegionMetadata,
) -> Result<(), FangyuanChunkValidationError> {
    let prefix = match chunk_index {
        Some(chunk_index) => format!("{root}[{chunk_index}].region"),
        None => root.to_string(),
    };
    validate_namespace_id(
        &region.region_id,
        FANGYUAN_CHUNK_REGION_ID_MAX_LEN,
        true,
        format!("{prefix}.region_id"),
    )?;
    validate_namespace_id(
        &region.layer,
        FANGYUAN_CHUNK_REGION_ID_MAX_LEN,
        false,
        format!("{prefix}.layer"),
    )?;

    if region.tags.len() > FANGYUAN_PREFAB_MAX_TAGS {
        return Err(FangyuanChunkValidationError::TooManyRegionTags {
            field_path: format!("{prefix}.tags"),
            count: region.tags.len(),
            limit: FANGYUAN_PREFAB_MAX_TAGS,
        });
    }

    for (tag_index, tag) in region.tags.iter().enumerate() {
        validate_prefab_tag(tag).map_err(|reason| {
            FangyuanChunkValidationError::InvalidRegionTag {
                field_path: format!("{prefix}.tags[{tag_index}]"),
                tag: tag.clone(),
                reason,
            }
        })?;
    }

    if let Some(chunk_id) = chunk_id {
        let _ = chunk_id;
    }

    Ok(())
}

fn validate_optional_asset_path(
    chunk_index: Option<usize>,
    _chunk_id: Option<&str>,
    field: &'static str,
    path: Option<&str>,
) -> Result<(), FangyuanChunkValidationError> {
    let Some(path) = path else {
        return Ok(());
    };

    validate_fangyuan_asset_path(path).map_err(|source| {
        FangyuanChunkValidationError::InvalidAssetPath {
            field_path: match chunk_index {
                Some(chunk_index) => manifest_entry_field_path(chunk_index, field).into_owned(),
                None => field.to_string(),
            },
            path: path.to_string(),
            source,
        }
    })
}

fn validate_optional_artifact_hash(
    field_path: Cow<'static, str>,
    hash: Option<&str>,
) -> Result<(), FangyuanChunkValidationError> {
    let Some(hash) = hash else {
        return Ok(());
    };

    validate_artifact_text(hash, FANGYUAN_CHUNK_ARTIFACT_HASH_MAX_LEN, true).map_err(|reason| {
        FangyuanChunkValidationError::InvalidArtifactHash {
            field_path: field_path.into_owned(),
            hash: hash.to_string(),
            reason,
        }
    })
}

fn validate_optional_data_version(
    field_path: Cow<'static, str>,
    data_version: Option<&str>,
) -> Result<(), FangyuanChunkValidationError> {
    let Some(data_version) = data_version else {
        return Ok(());
    };

    validate_artifact_text(data_version, FANGYUAN_CHUNK_DATA_VERSION_MAX_LEN, false).map_err(
        |reason| FangyuanChunkValidationError::InvalidDataVersion {
            field_path: field_path.into_owned(),
            data_version: data_version.to_string(),
            reason,
        },
    )
}

fn validate_artifact_text(
    value: &str,
    max_len: usize,
    allow_hex_prefix: bool,
) -> Result<(), FangyuanChunkArtifactTextInvalidReason> {
    if value.is_empty() {
        return Err(FangyuanChunkArtifactTextInvalidReason::Empty);
    }
    if value.len() > max_len {
        return Err(FangyuanChunkArtifactTextInvalidReason::TooLong { max_len });
    }

    if value.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '_'
            || character == '-'
            || character == '.'
            || (allow_hex_prefix && character == ':')
    }) {
        Ok(())
    } else {
        Err(FangyuanChunkArtifactTextInvalidReason::InvalidCharacter)
    }
}

fn validate_non_empty_budget_summary(
    chunk_index: Option<usize>,
    chunk_id: Option<&str>,
    root: &str,
    budget: &FangyuanChunkBudgetSummary,
) -> Result<(), FangyuanChunkValidationError> {
    if budget.is_empty() {
        return Err(FangyuanChunkValidationError::EmptyChunk {
            field_path: match chunk_index {
                Some(chunk_index) => format!("{root}[{chunk_index}].budget.total_ref_count"),
                None => "budget.total_ref_count".to_string(),
            },
            chunk_id: chunk_id.unwrap_or("").to_string(),
        });
    }

    Ok(())
}

fn validate_local_ref_id(
    prefix: &str,
    id: &str,
    local_ids: &mut HashSet<String>,
    field_path: Cow<'static, str>,
) -> Result<(), FangyuanChunkValidationError> {
    validate_ref_id(id).map_err(|reason| FangyuanChunkValidationError::InvalidRefId {
        field_path: field_path.clone().into_owned(),
        id: id.to_string(),
        reason,
    })?;

    if !local_ids.insert(id.to_string()) {
        return Err(FangyuanChunkValidationError::DuplicateRefId {
            field_path: format!("{prefix}.id"),
            id: id.to_string(),
        });
    }

    Ok(())
}

fn validate_prefab_ref(
    field_path: Cow<'static, str>,
    prefab: &str,
    prefab_ids: Option<&HashSet<&str>>,
) -> Result<(), FangyuanChunkValidationError> {
    validate_prefab_id(prefab).map_err(|reason| {
        FangyuanChunkValidationError::InvalidPrefabRef {
            field_path: field_path.clone().into_owned(),
            prefab: prefab.to_string(),
            reason,
        }
    })?;

    if let Some(prefab_ids) = prefab_ids
        && !prefab_ids.contains(prefab)
    {
        return Err(FangyuanChunkValidationError::MissingPrefabRef {
            field_path: field_path.into_owned(),
            prefab: prefab.to_string(),
        });
    }

    Ok(())
}

fn validate_static_decoration_source(
    decoration_index: usize,
    source: &FangyuanChunkStaticDecorationSourceRef,
    prefab_ids: Option<&HashSet<&str>>,
) -> Result<(), FangyuanChunkValidationError> {
    match source {
        FangyuanChunkStaticDecorationSourceRef::Prefab { prefab } => validate_prefab_ref(
            static_decoration_source_field_path(decoration_index, "prefab"),
            prefab,
            prefab_ids,
        ),
        FangyuanChunkStaticDecorationSourceRef::Blueprint { blueprint } => {
            validate_fangyuan_asset_path(blueprint).map_err(|source| {
                FangyuanChunkValidationError::InvalidAssetPath {
                    field_path: static_decoration_source_field_path(decoration_index, "blueprint")
                        .into_owned(),
                    path: blueprint.clone(),
                    source,
                }
            })
        }
        FangyuanChunkStaticDecorationSourceRef::Bake { bake } => validate_fangyuan_asset_path(bake)
            .map_err(|source| FangyuanChunkValidationError::InvalidAssetPath {
                field_path: static_decoration_source_field_path(decoration_index, "bake")
                    .into_owned(),
                path: bake.clone(),
                source,
            }),
    }
}

fn validate_transform(
    prefix: &str,
    transform: &FangyuanChunkTransform,
    bounds: &FangyuanChunkBounds,
    kind: FangyuanChunkRefKind,
) -> Result<(), FangyuanChunkValidationError> {
    validate_position_in_bounds(
        format!("{prefix}.transform.position"),
        transform.position,
        bounds,
        kind,
    )?;

    for (axis, value) in transform.scale.into_iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            return Err(FangyuanChunkValidationError::InvalidRefScale {
                field_path: format!("{prefix}.transform.scale[{axis}]"),
                kind,
                axis,
                value,
            });
        }
    }

    Ok(())
}

fn validate_position_in_bounds(
    field_path: impl Into<String>,
    position: [f32; 3],
    bounds: &FangyuanChunkBounds,
    kind: FangyuanChunkRefKind,
) -> Result<(), FangyuanChunkValidationError> {
    let field_path = field_path.into();
    for (axis, value) in position.into_iter().enumerate() {
        let min = bounds.min[axis];
        let max = bounds.max[axis];
        if !value.is_finite() || value < min || value > max {
            return Err(FangyuanChunkValidationError::InvalidRefPosition {
                field_path: format!("{field_path}[{axis}]"),
                kind,
                axis,
                value,
                min,
                max,
            });
        }
    }

    Ok(())
}

fn validate_ref_budget_cost(
    field_path: Cow<'static, str>,
    budget_cost: u32,
) -> Result<(), FangyuanChunkValidationError> {
    if budget_cost == 0 {
        Err(FangyuanChunkValidationError::InvalidRefBudgetCost {
            field_path: field_path.into_owned(),
            value: budget_cost,
        })
    } else {
        Ok(())
    }
}

fn manifest_entry_field_path(chunk_index: usize, field: &str) -> Cow<'static, str> {
    Cow::Owned(format!("chunks[{chunk_index}].{field}"))
}

fn prefab_instance_field_path(index: usize, field: &str) -> Cow<'static, str> {
    Cow::Owned(format!("prefab_instances[{index}].{field}"))
}

fn tiandao_ref_field_path(index: usize, field: &str) -> Cow<'static, str> {
    Cow::Owned(format!("tiandao_refs[{index}].{field}"))
}

fn static_decoration_field_path(index: usize, field: &str) -> Cow<'static, str> {
    Cow::Owned(format!("static_decorations[{index}].{field}"))
}

fn static_decoration_source_field_path(index: usize, field: &str) -> Cow<'static, str> {
    Cow::Owned(format!("static_decorations[{index}].source.{field}"))
}

fn owned_field_path(prefix: &Cow<'static, str>, field: &str) -> String {
    format!("{prefix}.{field}")
}

fn strip_bounds_prefix(field_path: Cow<'_, str>) -> String {
    field_path
        .strip_prefix("bounds.")
        .unwrap_or(field_path.as_ref())
        .to_string()
}

fn chunk_id_reason(kind: &str, id: &str, reason: FangyuanChunkIdInvalidReason) -> String {
    match reason {
        FangyuanChunkIdInvalidReason::Empty => format!("{kind} must not be empty"),
        FangyuanChunkIdInvalidReason::TooLong { max_len } => {
            format!("{kind} `{id}` must contain at most {max_len} characters")
        }
        FangyuanChunkIdInvalidReason::MustStartWithLowercaseAscii => {
            format!("{kind} `{id}` must start with a lowercase ASCII letter")
        }
        FangyuanChunkIdInvalidReason::InvalidCharacter => {
            format!(
                "{kind} `{id}` may only contain lowercase ASCII letters, digits, `_`, and allowed `-` separators"
            )
        }
        FangyuanChunkIdInvalidReason::PathLike => {
            format!("{kind} `{id}` must not contain path-like separators or namespace dots")
        }
    }
}

fn namespace_id_reason(
    kind: &str,
    id: &str,
    reason: FangyuanChunkNamespaceIdInvalidReason,
) -> String {
    match reason {
        FangyuanChunkNamespaceIdInvalidReason::Empty => format!("{kind} must not be empty"),
        FangyuanChunkNamespaceIdInvalidReason::TooLong { max_len } => {
            format!("{kind} `{id}` must contain at most {max_len} characters")
        }
        FangyuanChunkNamespaceIdInvalidReason::MustStartWithLowercaseAscii => {
            format!("{kind} `{id}` must start with a lowercase ASCII letter")
        }
        FangyuanChunkNamespaceIdInvalidReason::InvalidCharacter => {
            format!(
                "{kind} `{id}` may only contain lowercase ASCII letters, digits, `_`, `-`, and optional `.` separators"
            )
        }
        FangyuanChunkNamespaceIdInvalidReason::PathLike => {
            format!("{kind} `{id}` must not contain path-like separators")
        }
    }
}

fn prefab_id_reason(prefab: &str, reason: FangyuanPrefabIdInvalidReason) -> String {
    match reason {
        FangyuanPrefabIdInvalidReason::Empty => "prefab must not be empty".to_string(),
        FangyuanPrefabIdInvalidReason::TooLong { max_len } => {
            format!("prefab `{prefab}` must contain at most {max_len} characters")
        }
        FangyuanPrefabIdInvalidReason::MustStartWithLowercaseAscii => {
            format!("prefab `{prefab}` must start with a lowercase ASCII letter")
        }
        FangyuanPrefabIdInvalidReason::InvalidCharacter => {
            format!("prefab `{prefab}` may only contain lowercase ASCII letters, digits, and `_`")
        }
        FangyuanPrefabIdInvalidReason::PathLike => {
            format!("prefab `{prefab}` must not contain path-like separators or segments")
        }
    }
}

fn prefab_tag_reason(tag: &str, reason: FangyuanPrefabTagInvalidReason) -> String {
    match reason {
        FangyuanPrefabTagInvalidReason::Empty => "tag must not be empty".to_string(),
        FangyuanPrefabTagInvalidReason::TooLong { max_len } => {
            format!("tag `{tag}` must contain at most {max_len} characters")
        }
        FangyuanPrefabTagInvalidReason::InvalidCharacter => {
            format!("tag `{tag}` may only contain lowercase ASCII letters, digits, `_`, and `-`")
        }
    }
}

fn artifact_text_reason(
    label: &str,
    value: &str,
    reason: FangyuanChunkArtifactTextInvalidReason,
) -> String {
    match reason {
        FangyuanChunkArtifactTextInvalidReason::Empty => {
            format!("{label} must not be empty")
        }
        FangyuanChunkArtifactTextInvalidReason::TooLong { max_len } => {
            format!("{label} `{value}` must contain at most {max_len} characters")
        }
        FangyuanChunkArtifactTextInvalidReason::InvalidCharacter => {
            format!("{label} `{value}` contains unsupported characters")
        }
    }
}

fn deserialize_f32_array_3<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<f32>::deserialize(deserializer)?;
    values
        .try_into()
        .map_err(|values: Vec<f32>| serde::de::Error::invalid_length(values.len(), &"3 f32 values"))
}

fn deserialize_optional_f32_array_3<'de, D>(deserializer: D) -> Result<Option<[f32; 3]>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_value(deserializer)
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_value(deserializer)
}

fn deserialize_optional_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptionalValue<T> {
        Value(T),
        Optional(Option<T>),
    }

    match OptionalValue::deserialize(deserializer)? {
        OptionalValue::Value(value) => Ok(Some(value)),
        OptionalValue::Optional(value) => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fangyuan_chunk_source_accepts_refs_without_primitive_payload() {
        let source = FangyuanChunkSource::from_ron_str(
            r#"
(
    version: "1",
    id: "home_chunk_0",
    name: "Home Chunk 0",
    description: "Chunk source keeps refs only.",
    bounds: (min: [-8.0, 0.0, -8.0], max: [8.0, 6.0, 8.0]),
    region: (region_id: "home.default", layer: "ground", tags: ["starter"]),
    prefab_instances: [
        (
            id: "stone_a",
            prefab: "stone_block",
            transform: (position: [0.0, 0.0, 0.0], scale: [1.0, 1.0, 1.0]),
            budget_cost: 5,
        ),
    ],
    tiandao_refs: [
        (
            id: "wind_a",
            tiandao: "tiandao.local_wind",
            anchor: [1.0, 1.0, 0.0],
            budget_cost: 8,
        ),
    ],
    static_decorations: [
        (
            id: "tree_a",
            source: (kind: "blueprint", blueprint: "fangyuan/blueprints/tree.ron"),
            transform: (position: [2.0, 0.0, 2.0], scale: [1.0, 1.0, 1.0]),
            budget_cost: 3,
        ),
    ],
    budget: (
        prefab_instance_count: 1,
        tiandao_ref_count: 1,
        static_decoration_count: 1,
        total_ref_count: 3,
        prefab_cost: 5,
        tiandao_cost: 8,
        static_decoration_cost: 3,
        total_cost: 16,
    ),
)
"#,
        )
        .unwrap();

        source.validate_against_prefab_ids(["stone_block"]).unwrap();
        assert_eq!(source.budget.total_ref_count, 3);
        assert_eq!(source.prefab_instances[0].prefab, "stone_block");
        assert!(matches!(
            source.static_decorations[0].source,
            FangyuanChunkStaticDecorationSourceRef::Blueprint { .. }
        ));
    }

    #[test]
    fn fangyuan_chunk_manifest_accepts_dev_ron_and_reserved_artifact_fields() {
        let manifest = FangyuanChunkManifest::from_ron_str(
            r#"
(
    version: "1",
    name: "home_chunks",
    description: "Development chunk manifest.",
    world_id: "home.world",
    chunks: [
        (
            id: "home_chunk_0",
            bounds: (min: [-8.0, 0.0, -8.0], max: [8.0, 6.0, 8.0]),
            region: (region_id: "home.default", layer: "ground", tags: []),
            dev_ron: "fangyuan/chunks/home_chunk_0.ron",
            bin: "fangyuan/chunks/home_chunk_0.bin",
            hash: "sha256:abc123",
            data_version: "chunk_v1",
            budget: (
                prefab_instance_count: 1,
                tiandao_ref_count: 0,
                static_decoration_count: 0,
                total_ref_count: 1,
                prefab_cost: 5,
                tiandao_cost: 0,
                static_decoration_cost: 0,
                total_cost: 5,
            ),
        ),
    ],
)
"#,
        )
        .unwrap();

        manifest.validate().unwrap();
        assert_eq!(
            manifest.chunks[0].dev_ron.as_deref(),
            Some("fangyuan/chunks/home_chunk_0.ron")
        );
        assert_eq!(manifest.chunks[0].data_version.as_deref(), Some("chunk_v1"));
    }

    #[test]
    fn fangyuan_chunk_rejects_invalid_bounds() {
        let mut source = valid_source();
        source.bounds.max[0] = source.bounds.min[0];

        let error = source.validate().unwrap_err();

        assert!(matches!(
            error,
            FangyuanChunkValidationError::InvalidChunkBounds { .. }
        ));
        assert_eq!(error.code(), "invalid_chunk_bounds");
        assert_eq!(error.field_path().as_ref(), "bounds.axis[0]");
    }

    #[test]
    fn fangyuan_chunk_manifest_rejects_duplicate_chunk_id() {
        let mut manifest = valid_manifest();
        manifest.chunks.push(manifest.chunks[0].clone());

        let error = manifest.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanChunkValidationError::DuplicateChunkId {
                chunk_index: 1,
                id: "home_chunk_0".to_string(),
            }
        );
        assert_eq!(error.field_path().as_ref(), "chunks[1].id");
    }

    #[test]
    fn fangyuan_chunk_rejects_missing_prefab_ref() {
        let mut source = valid_source();
        source.prefab_instances[0].prefab = "missing_prefab".to_string();

        let error = source
            .validate_against_prefab_ids(["stone_block"])
            .unwrap_err();

        assert_eq!(
            error,
            FangyuanChunkValidationError::MissingPrefabRef {
                field_path: "prefab_instances[0].prefab".to_string(),
                prefab: "missing_prefab".to_string(),
            }
        );
    }

    #[test]
    fn fangyuan_chunk_rejects_duplicate_content_ref_id() {
        let mut source = valid_source();
        source.tiandao_refs.push(FangyuanChunkTiandaoRef {
            id: "stone_a".to_string(),
            tiandao: "tiandao.local_wind".to_string(),
            anchor: None,
            budget_cost: 1,
        });
        source.budget.tiandao_ref_count = 1;
        source.budget.total_ref_count = 2;
        source.budget.tiandao_cost = 1;
        source.budget.total_cost = 6;

        let error = source.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanChunkValidationError::DuplicateRefId {
                field_path: "tiandao_refs[0].id".to_string(),
                id: "stone_a".to_string(),
            }
        );
    }

    #[test]
    fn fangyuan_chunk_rejects_empty_chunk() {
        let mut source = valid_source();
        source.prefab_instances.clear();
        source.budget = FangyuanChunkBudgetSummary::default();

        let error = source.validate().unwrap_err();

        assert_eq!(error.code(), "empty_chunk");
        assert_eq!(error.field_path().as_ref(), "prefab_instances");
    }

    #[test]
    fn fangyuan_chunk_rejects_budget_summary_mismatch() {
        let mut source = valid_source();
        source.budget.total_cost = 99;

        let error = source.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanChunkValidationError::BudgetSummaryMismatch {
                field_path: "budget.total_cost".to_string(),
                expected: 5,
                actual: 99,
            }
        );
    }

    #[test]
    fn fangyuan_chunk_rejects_primitive_payload_by_parse() {
        let result = FangyuanChunkSource::from_ron_str(
            r#"
(
    version: "1",
    id: "home_chunk_0",
    name: "Home Chunk 0",
    description: "",
    bounds: (min: [-8.0, 0.0, -8.0], max: [8.0, 6.0, 8.0]),
    region: (region_id: "home.default", layer: "ground", tags: []),
    prefab_instances: [],
    tiandao_refs: [],
    static_decorations: [],
    primitives: [],
    budget: (
        prefab_instance_count: 0,
        tiandao_ref_count: 0,
        static_decoration_count: 0,
        total_ref_count: 0,
        prefab_cost: 0,
        tiandao_cost: 0,
        static_decoration_cost: 0,
        total_cost: 0,
    ),
)
"#,
        );

        let error = result.unwrap_err().to_string();
        assert!(error.contains("primitives"));
        assert!(error.contains("Unexpected field"));
    }

    fn valid_manifest() -> FangyuanChunkManifest {
        FangyuanChunkManifest {
            version: FANGYUAN_CHUNK_VERSION.to_string(),
            name: "home_chunks".to_string(),
            description: String::new(),
            world_id: Some("home.world".to_string()),
            chunks: vec![FangyuanChunkManifestEntry {
                id: "home_chunk_0".to_string(),
                bounds: valid_bounds(),
                region: valid_region(),
                dev_ron: Some("fangyuan/chunks/home_chunk_0.ron".to_string()),
                bin: None,
                hash: None,
                data_version: None,
                budget: FangyuanChunkBudgetSummary {
                    prefab_instance_count: 1,
                    tiandao_ref_count: 0,
                    static_decoration_count: 0,
                    total_ref_count: 1,
                    prefab_cost: 5,
                    tiandao_cost: 0,
                    static_decoration_cost: 0,
                    total_cost: 5,
                },
            }],
        }
    }

    fn valid_source() -> FangyuanChunkSource {
        FangyuanChunkSource {
            version: FANGYUAN_CHUNK_VERSION.to_string(),
            id: "home_chunk_0".to_string(),
            name: "Home Chunk 0".to_string(),
            description: String::new(),
            bounds: valid_bounds(),
            region: valid_region(),
            prefab_instances: vec![FangyuanChunkPrefabInstanceRef {
                id: "stone_a".to_string(),
                prefab: "stone_block".to_string(),
                transform: FangyuanChunkTransform::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
                budget_cost: 5,
            }],
            tiandao_refs: Vec::new(),
            static_decorations: Vec::new(),
            bin: None,
            hash: None,
            data_version: None,
            budget: FangyuanChunkBudgetSummary {
                prefab_instance_count: 1,
                tiandao_ref_count: 0,
                static_decoration_count: 0,
                total_ref_count: 1,
                prefab_cost: 5,
                tiandao_cost: 0,
                static_decoration_cost: 0,
                total_cost: 5,
            },
        }
    }

    fn valid_bounds() -> FangyuanChunkBounds {
        FangyuanChunkBounds::new([-8.0, 0.0, -8.0], [8.0, 6.0, 8.0])
    }

    fn valid_region() -> FangyuanChunkRegionMetadata {
        FangyuanChunkRegionMetadata {
            region_id: "home.default".to_string(),
            layer: "ground".to_string(),
            tags: Vec::new(),
        }
    }
}
