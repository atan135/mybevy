use serde::{Deserialize, Deserializer, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs, io,
    path::PathBuf,
};

use super::{
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT, FANGYUAN_BLUEPRINT_VERSION, FangyuanAssetPathError,
    FangyuanBlueprintBounds, FangyuanBlueprintValidationError, FangyuanPrefabDefinition,
    FangyuanPrefabIdInvalidReason, FangyuanPrefabPalette, FangyuanPrefabTagInvalidReason,
    FangyuanPrefabValidationError, FangyuanPrimitiveBlueprint, FangyuanPrimitiveSet,
    FangyuanPrimitiveSetStats, compile_blueprint_primitive_to_runtime,
    first_package_fangyuan_asset_fs_path, validate_blueprint_primitive,
    validate_fangyuan_asset_path, validate_prefab_id, validate_prefab_tag,
};

pub const FANGYUAN_SCENE_LAYOUT_VERSION: &str = FANGYUAN_BLUEPRINT_VERSION;
pub const FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT: usize =
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT;
pub const FANGYUAN_HOME_SCENE_LAYOUT_PATH: &str = "fangyuan/layouts/home_layout.ron";
pub const FANGYUAN_SCENE_LAYOUT_INSTANCE_ID_MAX_LEN: usize = 64;
pub const FANGYUAN_SCENE_LAYOUT_MAX_INSTANCE_TAGS: usize = 16;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSceneLayout {
    pub version: String,
    pub name: String,
    pub description: String,
    pub bounds: FangyuanBlueprintBounds,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub palette: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palettes: Vec<String>,
    pub max_primitives: usize,
    pub instances: Vec<FangyuanSceneLayoutInstance>,
}

impl FangyuanSceneLayout {
    pub fn from_ron_str(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str::<Self>(source)
    }

    pub fn load_first_package_ron(
        layout_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanSceneLayoutLoadError> {
        let layout_path = layout_path.as_ref().trim();
        validate_fangyuan_scene_layout_asset_path(layout_path)
            .map_err(FangyuanSceneLayoutLoadError::InvalidPath)?;

        let fs_path = first_package_fangyuan_asset_fs_path(layout_path).ok_or_else(|| {
            FangyuanSceneLayoutLoadError::SceneLayoutNotFound(layout_path.to_string())
        })?;

        let source = fs::read_to_string(&fs_path).map_err(|source| {
            FangyuanSceneLayoutLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            }
        })?;

        Self::from_ron_str(&source).map_err(|source| FangyuanSceneLayoutLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }

    pub fn load_validated_first_package_ron(
        layout_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanSceneLayoutLoadError> {
        let layout = Self::load_first_package_ron(layout_path)?;
        layout
            .validate()
            .map_err(FangyuanSceneLayoutLoadError::ValidationFailed)?;
        Ok(layout)
    }

    pub fn validate(&self) -> Result<(), FangyuanSceneLayoutValidationError> {
        self.validate_top_level()?;
        self.validate_instances(None)
    }

    pub fn validate_against_palette(
        &self,
        palette: &FangyuanPrefabPalette,
    ) -> Result<(), FangyuanSceneLayoutValidationError> {
        self.validate_top_level()?;
        let prefab_ids = palette
            .prefabs
            .iter()
            .map(|prefab| (prefab.id.as_str(), prefab.primitives.len()))
            .collect::<Vec<_>>();
        self.validate_instances(Some(&prefab_ids))
    }

    pub fn validate_against_prefab_ids<'a>(
        &self,
        prefab_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), FangyuanSceneLayoutValidationError> {
        self.validate_top_level()?;
        let prefab_ids = prefab_ids
            .into_iter()
            .map(|prefab_id| (prefab_id, 0usize))
            .collect::<Vec<_>>();
        self.validate_instances(Some(&prefab_ids))
    }

    pub fn palette_paths(&self) -> impl Iterator<Item = &str> {
        self.palette
            .iter()
            .map(String::as_str)
            .chain(self.palettes.iter().map(String::as_str))
    }

    pub fn compile_with_palette(
        &self,
        palette: &FangyuanPrefabPalette,
    ) -> Result<FangyuanSceneLayoutCompileReport, FangyuanSceneLayoutCompileError> {
        palette
            .validate()
            .map_err(FangyuanSceneLayoutCompileError::PaletteValidationFailed)?;
        self.validate_against_palette(palette)
            .map_err(FangyuanSceneLayoutCompileError::LayoutValidationFailed)?;

        let prefab_by_id = palette
            .prefabs
            .iter()
            .map(|prefab| (prefab.id.as_str(), prefab))
            .collect::<HashMap<_, _>>();
        let authored_prefab_primitives = palette
            .prefabs
            .iter()
            .map(|prefab| prefab.primitives.len())
            .sum();
        let expanded_primitive_count =
            self.instances.iter().try_fold(0usize, |count, instance| {
                prefab_by_id
                    .get(instance.prefab.as_str())
                    .map(|prefab| count.saturating_add(prefab.primitives.len()))
                    .ok_or_else(|| {
                        FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                            FangyuanSceneLayoutValidationError::MissingPrefab {
                                instance_index: 0,
                                prefab: instance.prefab.clone(),
                            },
                        )
                    })
            })?;
        let effective_limit = self
            .max_primitives
            .min(FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT);
        if expanded_primitive_count > effective_limit {
            return Err(
                FangyuanSceneLayoutCompileError::ExpandedPrimitiveBudgetExceeded {
                    count: expanded_primitive_count,
                    limit: effective_limit,
                    layout_limit: self.max_primitives,
                    hard_limit: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
                },
            );
        }

        let mut used_prefabs = HashSet::with_capacity(self.instances.len());
        let mut primitives = Vec::new();
        let mut warnings = Vec::new();

        for (instance_index, instance) in self.instances.iter().enumerate() {
            let prefab = prefab_by_id.get(instance.prefab.as_str()).ok_or_else(|| {
                FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                    FangyuanSceneLayoutValidationError::MissingPrefab {
                        instance_index,
                        prefab: instance.prefab.clone(),
                    },
                )
            })?;
            used_prefabs.insert(prefab.id.as_str());

            for (prefab_primitive_index, primitive) in prefab.primitives.iter().enumerate() {
                let transformed = transform_prefab_primitive(instance, prefab, primitive);
                match validate_blueprint_primitive(
                    prefab_primitive_index,
                    &transformed,
                    &self.bounds,
                ) {
                    Ok(()) => primitives.push(compile_blueprint_primitive_to_runtime(&transformed)),
                    Err(source) => warnings.push(FangyuanSceneLayoutCompileWarning {
                        instance_index,
                        instance_id: instance.id.clone(),
                        prefab_id: prefab.id.clone(),
                        prefab_primitive_index,
                        source,
                    }),
                }
            }
        }

        let primitive_set = FangyuanPrimitiveSet::from_primitives(primitives);
        let primitive_stats = primitive_set.stats();

        Ok(FangyuanSceneLayoutCompileReport {
            primitive_set,
            primitive_stats,
            palette_count: self.palette_paths().count(),
            prefab_count: palette.prefabs.len(),
            authored_prefab_primitives,
            instance_count: self.instances.len(),
            generated_primitives: expanded_primitive_count - warnings.len(),
            skipped_primitives: warnings.len(),
            used_prefab_count: used_prefabs.len(),
            top_level_validated: true,
            layout_validated: true,
            palette_validated: true,
            warnings,
        })
    }

    fn validate_top_level(&self) -> Result<(), FangyuanSceneLayoutValidationError> {
        if self.version != FANGYUAN_SCENE_LAYOUT_VERSION {
            return Err(FangyuanSceneLayoutValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_SCENE_LAYOUT_VERSION,
            });
        }

        self.bounds
            .validate()
            .map_err(|source| FangyuanSceneLayoutValidationError::InvalidLayoutBounds { source })?;

        if self.max_primitives > FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT {
            return Err(
                FangyuanSceneLayoutValidationError::LayoutPrimitiveBudgetExceeded {
                    max_primitives: self.max_primitives,
                    hard_limit: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
                },
            );
        }

        let palette_count = self.palette_paths().count();
        if palette_count == 0 {
            return Err(FangyuanSceneLayoutValidationError::MissingPalettePath);
        }

        let mut paths = HashSet::with_capacity(palette_count);
        for (palette_index, path) in self.palette_paths().enumerate() {
            validate_palette_path(path).map_err(|reason| {
                FangyuanSceneLayoutValidationError::InvalidPalettePath {
                    palette_index,
                    path: path.to_string(),
                    reason,
                }
            })?;

            if !paths.insert(path) {
                return Err(FangyuanSceneLayoutValidationError::DuplicatePalettePath {
                    palette_index,
                    path: path.to_string(),
                });
            }
        }

        Ok(())
    }

    fn validate_instances(
        &self,
        prefab_primitives: Option<&[(&str, usize)]>,
    ) -> Result<(), FangyuanSceneLayoutValidationError> {
        let mut instance_ids = HashSet::with_capacity(self.instances.len());
        let mut expanded_primitive_count = 0usize;

        for (instance_index, instance) in self.instances.iter().enumerate() {
            validate_prefab_id(&instance.prefab).map_err(|reason| {
                FangyuanSceneLayoutValidationError::InvalidInstancePrefabId {
                    instance_index,
                    prefab: instance.prefab.clone(),
                    reason,
                }
            })?;

            let prefab_primitive_count = match prefab_primitives {
                Some(prefab_primitives) => prefab_primitives
                    .iter()
                    .find_map(|(id, count)| (*id == instance.prefab).then_some(*count))
                    .ok_or_else(|| FangyuanSceneLayoutValidationError::MissingPrefab {
                        instance_index,
                        prefab: instance.prefab.clone(),
                    })?,
                None => 0,
            };

            if let Some(id) = instance.id.as_deref() {
                validate_instance_id(id).map_err(|reason| {
                    FangyuanSceneLayoutValidationError::InvalidInstanceId {
                        instance_index,
                        id: id.to_string(),
                        reason,
                    }
                })?;

                if !instance_ids.insert(id) {
                    return Err(FangyuanSceneLayoutValidationError::DuplicateInstanceId {
                        instance_index,
                        id: id.to_string(),
                    });
                }
            }

            validate_instance_tags(instance_index, &instance.tags)?;
            validate_instance_position(instance_index, instance.position, &self.bounds)?;
            validate_instance_scale(instance_index, instance.scale)?;

            expanded_primitive_count =
                expanded_primitive_count.saturating_add(prefab_primitive_count);
            if prefab_primitives.is_some() && expanded_primitive_count > self.max_primitives {
                return Err(
                    FangyuanSceneLayoutValidationError::ExpandedPrimitiveBudgetExceeded {
                        count: expanded_primitive_count,
                        limit: self.max_primitives,
                    },
                );
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSceneLayoutCompileReport {
    pub primitive_set: FangyuanPrimitiveSet,
    pub primitive_stats: FangyuanPrimitiveSetStats,
    pub palette_count: usize,
    pub prefab_count: usize,
    pub authored_prefab_primitives: usize,
    pub instance_count: usize,
    pub generated_primitives: usize,
    pub skipped_primitives: usize,
    pub used_prefab_count: usize,
    pub top_level_validated: bool,
    pub layout_validated: bool,
    pub palette_validated: bool,
    pub warnings: Vec<FangyuanSceneLayoutCompileWarning>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSceneLayoutCompileWarning {
    pub instance_index: usize,
    pub instance_id: Option<String>,
    pub prefab_id: String,
    pub prefab_primitive_index: usize,
    pub source: FangyuanBlueprintValidationError,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanSceneLayoutCompileError {
    LayoutValidationFailed(FangyuanSceneLayoutValidationError),
    PaletteValidationFailed(FangyuanPrefabValidationError),
    ExpandedPrimitiveBudgetExceeded {
        count: usize,
        limit: usize,
        layout_limit: usize,
        hard_limit: usize,
    },
}

impl FangyuanSceneLayoutCompileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::LayoutValidationFailed(error) => error.code(),
            Self::PaletteValidationFailed(error) => error.code(),
            Self::ExpandedPrimitiveBudgetExceeded { .. } => "expanded_primitive_budget_exceeded",
        }
    }

    pub fn field_path(&self) -> Cow<'static, str> {
        match self {
            Self::LayoutValidationFailed(error) => error.field_path(),
            Self::PaletteValidationFailed(error) => error.field_path(),
            Self::ExpandedPrimitiveBudgetExceeded { .. } => Cow::Borrowed("instances"),
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::LayoutValidationFailed(error) => error.reason(),
            Self::PaletteValidationFailed(error) => error.reason(),
            Self::ExpandedPrimitiveBudgetExceeded {
                count,
                limit,
                layout_limit,
                hard_limit,
            } => format!(
                "layout expands to {count} primitives, exceeding effective limit {limit} from min(max_primitives={layout_limit}, hard_limit={hard_limit})"
            ),
        }
    }
}

impl fmt::Display for FangyuanSceneLayoutCompileWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.instance_id.as_deref() {
            Some(instance_id) => write!(
                formatter,
                "fangyuan scene layout compile warning at instance {}/`{}` prefab `{}` primitive {}: {}",
                self.instance_index,
                instance_id,
                self.prefab_id,
                self.prefab_primitive_index,
                self.source
            ),
            None => write!(
                formatter,
                "fangyuan scene layout compile warning at instance {} prefab `{}` primitive {}: {}",
                self.instance_index, self.prefab_id, self.prefab_primitive_index, self.source
            ),
        }
    }
}

impl fmt::Display for FangyuanSceneLayoutCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LayoutValidationFailed(error) => {
                write!(formatter, "fangyuan scene layout compile failed: {error}")
            }
            Self::PaletteValidationFailed(error) => {
                write!(
                    formatter,
                    "fangyuan prefab palette validation failed: {error}"
                )
            }
            Self::ExpandedPrimitiveBudgetExceeded {
                count,
                limit,
                layout_limit,
                hard_limit,
            } => write!(
                formatter,
                "fangyuan scene layout compile failed: expanded primitive count {count} exceeds effective limit {limit} (layout {layout_limit}, hard {hard_limit})"
            ),
        }
    }
}

impl Error for FangyuanSceneLayoutCompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LayoutValidationFailed(error) => Some(error),
            Self::PaletteValidationFailed(error) => Some(error),
            Self::ExpandedPrimitiveBudgetExceeded { .. } => None,
        }
    }
}

pub fn load_fangyuan_home_scene_layout() -> Result<FangyuanSceneLayout, FangyuanSceneLayoutLoadError>
{
    FangyuanSceneLayout::load_validated_first_package_ron(FANGYUAN_HOME_SCENE_LAYOUT_PATH)
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSceneLayoutInstance {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    pub prefab: String,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub position: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub scale: [f32; 3],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn transform_prefab_primitive(
    instance: &FangyuanSceneLayoutInstance,
    prefab: &FangyuanPrefabDefinition,
    primitive: &FangyuanPrimitiveBlueprint,
) -> FangyuanPrimitiveBlueprint {
    let pivot = prefab.pivot.unwrap_or([0.0, 0.0, 0.0]);
    let mut transformed = primitive.clone();

    transformed.position = [
        instance.position[0] + (primitive.position[0] - pivot[0]) * instance.scale[0],
        instance.position[1] + (primitive.position[1] - pivot[1]) * instance.scale[1],
        instance.position[2] + (primitive.position[2] - pivot[2]) * instance.scale[2],
    ];
    transformed.size = [
        primitive.size[0] * instance.scale[0],
        primitive.size[1] * instance.scale[1],
        primitive.size[2] * instance.scale[2],
    ];

    transformed
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanSceneLayoutValidationError {
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    InvalidLayoutBounds {
        source: FangyuanBlueprintValidationError,
    },
    LayoutPrimitiveBudgetExceeded {
        max_primitives: usize,
        hard_limit: usize,
    },
    MissingPalettePath,
    InvalidPalettePath {
        palette_index: usize,
        path: String,
        reason: FangyuanSceneLayoutPathInvalidReason,
    },
    DuplicatePalettePath {
        palette_index: usize,
        path: String,
    },
    InvalidInstancePrefabId {
        instance_index: usize,
        prefab: String,
        reason: FangyuanPrefabIdInvalidReason,
    },
    MissingPrefab {
        instance_index: usize,
        prefab: String,
    },
    InvalidInstanceId {
        instance_index: usize,
        id: String,
        reason: FangyuanSceneLayoutInstanceIdInvalidReason,
    },
    DuplicateInstanceId {
        instance_index: usize,
        id: String,
    },
    TooManyInstanceTags {
        instance_index: usize,
        count: usize,
        limit: usize,
    },
    InvalidInstanceTag {
        instance_index: usize,
        tag_index: usize,
        tag: String,
        reason: FangyuanPrefabTagInvalidReason,
    },
    InvalidInstancePosition {
        instance_index: usize,
        axis: usize,
        value: f32,
        min: f32,
        max: f32,
    },
    InvalidInstanceScale {
        instance_index: usize,
        axis: usize,
        value: f32,
    },
    ExpandedPrimitiveBudgetExceeded {
        count: usize,
        limit: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanSceneLayoutPathInvalidReason {
    Empty,
    Backslash,
    Absolute,
    WindowsDrive,
    ParentOrEmptySegment,
    OutsideFangyuanRoot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanSceneLayoutInstanceIdInvalidReason {
    Empty,
    TooLong { max_len: usize },
    MustStartWithLowercaseAscii,
    InvalidCharacter,
    PathLike,
}

impl FangyuanSceneLayoutValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::InvalidLayoutBounds { .. } => "invalid_layout_bounds",
            Self::LayoutPrimitiveBudgetExceeded { .. } => "layout_primitive_budget_exceeded",
            Self::MissingPalettePath => "missing_palette_path",
            Self::InvalidPalettePath { .. } => "invalid_palette_path",
            Self::DuplicatePalettePath { .. } => "duplicate_palette_path",
            Self::InvalidInstancePrefabId { .. } => "invalid_instance_prefab_id",
            Self::MissingPrefab { .. } => "missing_prefab",
            Self::InvalidInstanceId { .. } => "invalid_instance_id",
            Self::DuplicateInstanceId { .. } => "duplicate_instance_id",
            Self::TooManyInstanceTags { .. } => "too_many_instance_tags",
            Self::InvalidInstanceTag { .. } => "invalid_instance_tag",
            Self::InvalidInstancePosition { .. } => "invalid_instance_position",
            Self::InvalidInstanceScale { .. } => "invalid_instance_scale",
            Self::ExpandedPrimitiveBudgetExceeded { .. } => "expanded_primitive_budget_exceeded",
        }
    }

    pub fn field_path(&self) -> Cow<'static, str> {
        match self {
            Self::UnsupportedVersion { .. } => Cow::Borrowed("version"),
            Self::InvalidLayoutBounds { source } => Cow::Owned(format!(
                "bounds.{}",
                strip_bounds_prefix(source.field_path())
            )),
            Self::LayoutPrimitiveBudgetExceeded { .. } => Cow::Borrowed("max_primitives"),
            Self::MissingPalettePath => Cow::Borrowed("palette"),
            Self::InvalidPalettePath { palette_index, .. }
            | Self::DuplicatePalettePath { palette_index, .. } => {
                palette_field_path(*palette_index)
            }
            Self::InvalidInstancePrefabId { instance_index, .. }
            | Self::MissingPrefab { instance_index, .. } => {
                Cow::Owned(format!("instances[{instance_index}].prefab"))
            }
            Self::InvalidInstanceId { instance_index, .. }
            | Self::DuplicateInstanceId { instance_index, .. } => {
                Cow::Owned(format!("instances[{instance_index}].id"))
            }
            Self::TooManyInstanceTags { instance_index, .. } => {
                Cow::Owned(format!("instances[{instance_index}].tags"))
            }
            Self::InvalidInstanceTag {
                instance_index,
                tag_index,
                ..
            } => Cow::Owned(format!("instances[{instance_index}].tags[{tag_index}]")),
            Self::InvalidInstancePosition {
                instance_index,
                axis,
                ..
            } => Cow::Owned(format!("instances[{instance_index}].position[{axis}]")),
            Self::InvalidInstanceScale {
                instance_index,
                axis,
                ..
            } => Cow::Owned(format!("instances[{instance_index}].scale[{axis}]")),
            Self::ExpandedPrimitiveBudgetExceeded { .. } => Cow::Borrowed("instances"),
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                format!("version `{found}` is unsupported; expected `{expected}`")
            }
            Self::InvalidLayoutBounds { source } => source.reason(),
            Self::LayoutPrimitiveBudgetExceeded {
                max_primitives,
                hard_limit,
            } => {
                format!("max_primitives {max_primitives} exceeds hard limit {hard_limit}")
            }
            Self::MissingPalettePath => "at least one palette path is required".to_string(),
            Self::InvalidPalettePath { path, reason, .. } => match reason {
                FangyuanSceneLayoutPathInvalidReason::Empty => {
                    "palette path must not be empty".to_string()
                }
                FangyuanSceneLayoutPathInvalidReason::Backslash => {
                    format!("palette path `{path}` must use forward slashes")
                }
                FangyuanSceneLayoutPathInvalidReason::Absolute => {
                    format!("palette path `{path}` must be relative")
                }
                FangyuanSceneLayoutPathInvalidReason::WindowsDrive => {
                    format!("palette path `{path}` must not include a Windows drive prefix")
                }
                FangyuanSceneLayoutPathInvalidReason::ParentOrEmptySegment => {
                    format!("palette path `{path}` must not contain empty or parent segments")
                }
                FangyuanSceneLayoutPathInvalidReason::OutsideFangyuanRoot => {
                    format!("palette path `{path}` must stay inside assets/fangyuan")
                }
            },
            Self::DuplicatePalettePath { path, .. } => {
                format!("palette path `{path}` is already listed")
            }
            Self::InvalidInstancePrefabId { prefab, reason, .. } => match reason {
                FangyuanPrefabIdInvalidReason::Empty => "prefab must not be empty".to_string(),
                FangyuanPrefabIdInvalidReason::TooLong { max_len } => {
                    format!("prefab `{prefab}` must contain at most {max_len} characters")
                }
                FangyuanPrefabIdInvalidReason::MustStartWithLowercaseAscii => {
                    format!("prefab `{prefab}` must start with a lowercase ASCII letter")
                }
                FangyuanPrefabIdInvalidReason::InvalidCharacter => format!(
                    "prefab `{prefab}` may only contain lowercase ASCII letters, digits, and `_`"
                ),
                FangyuanPrefabIdInvalidReason::PathLike => {
                    format!("prefab `{prefab}` must not contain path-like separators or segments")
                }
            },
            Self::MissingPrefab { prefab, .. } => {
                format!("prefab `{prefab}` is not present in the referenced palette")
            }
            Self::InvalidInstanceId { id, reason, .. } => match reason {
                FangyuanSceneLayoutInstanceIdInvalidReason::Empty => {
                    "id must not be empty".to_string()
                }
                FangyuanSceneLayoutInstanceIdInvalidReason::TooLong { max_len } => {
                    format!("id `{id}` must contain at most {max_len} characters")
                }
                FangyuanSceneLayoutInstanceIdInvalidReason::MustStartWithLowercaseAscii => {
                    format!("id `{id}` must start with a lowercase ASCII letter")
                }
                FangyuanSceneLayoutInstanceIdInvalidReason::InvalidCharacter => format!(
                    "id `{id}` may only contain lowercase ASCII letters, digits, `_`, and `-`"
                ),
                FangyuanSceneLayoutInstanceIdInvalidReason::PathLike => {
                    format!("id `{id}` must not contain path-like separators or segments")
                }
            },
            Self::DuplicateInstanceId { id, .. } => {
                format!("id `{id}` is already used by an earlier instance")
            }
            Self::TooManyInstanceTags { count, limit, .. } => {
                format!("contains {count} tags, exceeding limit {limit}")
            }
            Self::InvalidInstanceTag { tag, reason, .. } => match reason {
                FangyuanPrefabTagInvalidReason::Empty => "tag must not be empty".to_string(),
                FangyuanPrefabTagInvalidReason::TooLong { max_len } => {
                    format!("tag `{tag}` must contain at most {max_len} characters")
                }
                FangyuanPrefabTagInvalidReason::InvalidCharacter => format!(
                    "tag `{tag}` may only contain lowercase ASCII letters, digits, `_`, and `-`"
                ),
            },
            Self::InvalidInstancePosition {
                value, min, max, ..
            } => {
                format!("value {value} must be finite and inside {min}..={max}")
            }
            Self::InvalidInstanceScale { value, .. } => {
                format!("value {value} must be finite and greater than 0")
            }
            Self::ExpandedPrimitiveBudgetExceeded { count, limit } => format!(
                "layout expands to {count} primitives, exceeding max_primitives limit {limit}"
            ),
        }
    }
}

impl fmt::Display for FangyuanSceneLayoutValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fangyuan scene layout validation error [{}] at {}: {}",
            self.code(),
            self.field_path(),
            self.reason()
        )
    }
}

impl Error for FangyuanSceneLayoutValidationError {}

#[derive(Debug)]
pub enum FangyuanSceneLayoutLoadError {
    InvalidPath(FangyuanSceneLayoutPathError),
    SceneLayoutNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ValidationFailed(FangyuanSceneLayoutValidationError),
}

pub type FangyuanSceneLayoutPathError = FangyuanAssetPathError;

impl fmt::Display for FangyuanSceneLayoutLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(error) => write!(formatter, "{error}"),
            Self::SceneLayoutNotFound(path) => write!(
                formatter,
                "fangyuan scene layout was not found under first package assets: {path}"
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read fangyuan scene layout at {}: {source}",
                path.display()
            ),
            Self::ParseFailed { path, source } => write!(
                formatter,
                "failed to parse fangyuan scene layout RON at {}: {source}",
                path.display()
            ),
            Self::ValidationFailed(error) => {
                write!(
                    formatter,
                    "fangyuan scene layout validation failed: {error}"
                )
            }
        }
    }
}

impl Error for FangyuanSceneLayoutLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidPath(error) => Some(error),
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::ValidationFailed(error) => Some(error),
            Self::SceneLayoutNotFound(_) => None,
        }
    }
}

fn validate_palette_path(path: &str) -> Result<(), FangyuanSceneLayoutPathInvalidReason> {
    validate_fangyuan_asset_path(path).map_err(scene_layout_path_invalid_reason)
}

pub fn validate_fangyuan_scene_layout_asset_path(
    path: &str,
) -> Result<(), FangyuanSceneLayoutPathError> {
    validate_fangyuan_asset_path(path)
}

fn scene_layout_path_invalid_reason(
    error: FangyuanAssetPathError,
) -> FangyuanSceneLayoutPathInvalidReason {
    match error {
        FangyuanAssetPathError::Empty => FangyuanSceneLayoutPathInvalidReason::Empty,
        FangyuanAssetPathError::Absolute(_) => FangyuanSceneLayoutPathInvalidReason::Absolute,
        FangyuanAssetPathError::Backslash(_) => FangyuanSceneLayoutPathInvalidReason::Backslash,
        FangyuanAssetPathError::WindowsDrive(_) => {
            FangyuanSceneLayoutPathInvalidReason::WindowsDrive
        }
        FangyuanAssetPathError::ParentOrEmptySegment(_) => {
            FangyuanSceneLayoutPathInvalidReason::ParentOrEmptySegment
        }
        FangyuanAssetPathError::OutsideFangyuanRoot(_) => {
            FangyuanSceneLayoutPathInvalidReason::OutsideFangyuanRoot
        }
    }
}

fn validate_instance_id(id: &str) -> Result<(), FangyuanSceneLayoutInstanceIdInvalidReason> {
    if id.is_empty() {
        return Err(FangyuanSceneLayoutInstanceIdInvalidReason::Empty);
    }

    if id.len() > FANGYUAN_SCENE_LAYOUT_INSTANCE_ID_MAX_LEN {
        return Err(FangyuanSceneLayoutInstanceIdInvalidReason::TooLong {
            max_len: FANGYUAN_SCENE_LAYOUT_INSTANCE_ID_MAX_LEN,
        });
    }

    if id.contains('/')
        || id.contains('\\')
        || id.contains('.')
        || id.contains(':')
        || id.contains("..")
    {
        return Err(FangyuanSceneLayoutInstanceIdInvalidReason::PathLike);
    }

    let mut chars = id.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return Err(FangyuanSceneLayoutInstanceIdInvalidReason::MustStartWithLowercaseAscii);
    }

    if chars.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '_'
            || character == '-'
    }) {
        Ok(())
    } else {
        Err(FangyuanSceneLayoutInstanceIdInvalidReason::InvalidCharacter)
    }
}

fn validate_instance_tags(
    instance_index: usize,
    tags: &[String],
) -> Result<(), FangyuanSceneLayoutValidationError> {
    if tags.len() > FANGYUAN_SCENE_LAYOUT_MAX_INSTANCE_TAGS {
        return Err(FangyuanSceneLayoutValidationError::TooManyInstanceTags {
            instance_index,
            count: tags.len(),
            limit: FANGYUAN_SCENE_LAYOUT_MAX_INSTANCE_TAGS,
        });
    }

    for (tag_index, tag) in tags.iter().enumerate() {
        validate_prefab_tag(tag).map_err(|reason| {
            FangyuanSceneLayoutValidationError::InvalidInstanceTag {
                instance_index,
                tag_index,
                tag: tag.clone(),
                reason,
            }
        })?;
    }

    Ok(())
}

fn validate_instance_position(
    instance_index: usize,
    position: [f32; 3],
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanSceneLayoutValidationError> {
    let ranges = [
        (-bounds.width * 0.5, bounds.width * 0.5),
        (0.0, bounds.height),
        (-bounds.depth * 0.5, bounds.depth * 0.5),
    ];

    for (axis, value) in position.into_iter().enumerate() {
        let (min, max) = ranges[axis];
        if !value.is_finite() || value < min || value > max {
            return Err(
                FangyuanSceneLayoutValidationError::InvalidInstancePosition {
                    instance_index,
                    axis,
                    value,
                    min,
                    max,
                },
            );
        }
    }

    Ok(())
}

fn validate_instance_scale(
    instance_index: usize,
    scale: [f32; 3],
) -> Result<(), FangyuanSceneLayoutValidationError> {
    for (axis, value) in scale.into_iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            return Err(FangyuanSceneLayoutValidationError::InvalidInstanceScale {
                instance_index,
                axis,
                value,
            });
        }
    }

    Ok(())
}

fn strip_bounds_prefix(field_path: Cow<'_, str>) -> String {
    field_path
        .strip_prefix("bounds.")
        .unwrap_or(field_path.as_ref())
        .to_string()
}

fn palette_field_path(palette_index: usize) -> Cow<'static, str> {
    if palette_index == 0 {
        Cow::Borrowed("palette")
    } else {
        Cow::Owned(format!("palettes[{}]", palette_index - 1))
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

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptionalString {
        Value(String),
        Optional(Option<String>),
    }

    match OptionalString::deserialize(deserializer)? {
        OptionalString::Value(value) => Ok(Some(value)),
        OptionalString::Optional(value) => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FangyuanPrefabDefinition, FangyuanPrimitiveBlueprint, FangyuanPrimitiveKind,
        FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
    };
    use bevy::prelude::Vec3;

    #[test]
    fn valid_layout_accepts_palette_paths_and_instances() {
        let layout = FangyuanSceneLayout::from_ron_str(
            r#"
(
    version: "1",
    name: "home_layout",
    description: "Small authored scene layout.",
    bounds: (width: 12.0, depth: 10.0, height: 6.0),
    palette: "fangyuan/prefabs/home.ron",
    palettes: ["fangyuan/prefabs/decor.ron"],
    max_primitives: 16,
    instances: [
        (
            id: "stone_a",
            name: "Stone A",
            prefab: "stone_block",
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            tags: ["structure", "starter"],
        ),
        (
            prefab: "glow_orb",
            position: [2.0, 1.0, -1.0],
            scale: [0.5, 0.5, 0.5],
        ),
    ],
)
"#,
        )
        .unwrap();
        let palette = valid_palette(vec![
            valid_prefab("stone_block", vec![valid_primitive(), valid_primitive()]),
            valid_prefab("glow_orb", vec![valid_primitive()]),
        ]);

        layout.validate_against_palette(&palette).unwrap();

        assert_eq!(layout.version, FANGYUAN_SCENE_LAYOUT_VERSION);
        assert_eq!(layout.palette.as_deref(), Some("fangyuan/prefabs/home.ron"));
        assert_eq!(
            layout.palette_paths().collect::<Vec<_>>(),
            vec!["fangyuan/prefabs/home.ron", "fangyuan/prefabs/decor.ron"]
        );
        assert_eq!(layout.instances.len(), 2);
        assert_eq!(layout.instances[0].id.as_deref(), Some("stone_a"));
    }

    #[test]
    fn layout_rejects_missing_prefab_against_palette() {
        let layout = valid_layout(vec![valid_instance("missing_prefab")]);
        let palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);

        let error = layout.validate_against_palette(&palette).unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::MissingPrefab {
                instance_index: 0,
                prefab: "missing_prefab".to_string(),
            }
        );
        assert_validation_report(
            &error,
            "missing_prefab",
            "instances[0].prefab",
            &["missing_prefab", "not present"],
        );
    }

    #[test]
    fn layout_rejects_duplicate_instance_id() {
        let mut first = valid_instance("stone_block");
        first.id = Some("stone_a".to_string());
        let mut second = valid_instance("stone_block");
        second.id = Some("stone_a".to_string());
        let layout = valid_layout(vec![first, second]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::DuplicateInstanceId {
                instance_index: 1,
                id: "stone_a".to_string(),
            }
        );
        assert_validation_report(
            &error,
            "duplicate_instance_id",
            "instances[1].id",
            &["already used"],
        );
    }

    #[test]
    fn layout_rejects_illegal_instance_prefab_id() {
        let layout = valid_layout(vec![valid_instance("Stone/Block")]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidInstancePrefabId {
                instance_index: 0,
                prefab: "Stone/Block".to_string(),
                reason: FangyuanPrefabIdInvalidReason::PathLike,
            }
        );
        assert_validation_report(
            &error,
            "invalid_instance_prefab_id",
            "instances[0].prefab",
            &["path-like"],
        );
    }

    #[test]
    fn layout_rejects_invalid_instance_id() {
        let mut instance = valid_instance("stone_block");
        instance.id = Some("1stone".to_string());
        let layout = valid_layout(vec![instance]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidInstanceId {
                instance_index: 0,
                id: "1stone".to_string(),
                reason: FangyuanSceneLayoutInstanceIdInvalidReason::MustStartWithLowercaseAscii,
            }
        );
        assert_validation_report(
            &error,
            "invalid_instance_id",
            "instances[0].id",
            &["start with a lowercase ASCII letter"],
        );
    }

    #[test]
    fn layout_rejects_non_finite_position() {
        let mut instance = valid_instance("stone_block");
        instance.position = [0.0, f32::INFINITY, 0.0];
        let layout = valid_layout(vec![instance]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidInstancePosition {
                instance_index: 0,
                axis: 1,
                value: f32::INFINITY,
                min: 0.0,
                max: 8.0,
            }
        );
        assert_validation_report(
            &error,
            "invalid_instance_position",
            "instances[0].position[1]",
            &["finite", "0..=8"],
        );
    }

    #[test]
    fn layout_rejects_position_outside_bounds() {
        let mut instance = valid_instance("stone_block");
        instance.position = [5.1, 0.0, 0.0];
        let layout = valid_layout(vec![instance]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidInstancePosition {
                instance_index: 0,
                axis: 0,
                value: 5.1,
                min: -5.0,
                max: 5.0,
            }
        );
        assert_validation_report(
            &error,
            "invalid_instance_position",
            "instances[0].position[0]",
            &["inside -5..=5"],
        );
    }

    #[test]
    fn layout_rejects_non_positive_scale() {
        let mut instance = valid_instance("stone_block");
        instance.scale = [1.0, 0.0, 1.0];
        let layout = valid_layout(vec![instance]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidInstanceScale {
                instance_index: 0,
                axis: 1,
                value: 0.0,
            }
        );
        assert_validation_report(
            &error,
            "invalid_instance_scale",
            "instances[0].scale[1]",
            &["finite", "greater than 0"],
        );
    }

    #[test]
    fn layout_rejects_non_finite_scale() {
        let mut instance = valid_instance("stone_block");
        instance.scale = [1.0, f32::INFINITY, 1.0];
        let layout = valid_layout(vec![instance]);

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidInstanceScale {
                instance_index: 0,
                axis: 1,
                value: f32::INFINITY,
            }
        );
        assert_validation_report(
            &error,
            "invalid_instance_scale",
            "instances[0].scale[1]",
            &["finite", "greater than 0"],
        );
    }

    #[test]
    fn layout_rejects_primitive_budget_above_hard_limit() {
        let mut layout = valid_layout(Vec::new());
        layout.max_primitives = FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT + 1;

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::LayoutPrimitiveBudgetExceeded {
                max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT + 1,
                hard_limit: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            }
        );
        assert_validation_report(
            &error,
            "layout_primitive_budget_exceeded",
            "max_primitives",
            &["hard limit 1000"],
        );
    }

    #[test]
    fn layout_rejects_expanded_primitive_budget_against_palette() {
        let mut layout = valid_layout(vec![
            valid_instance("stone_block"),
            valid_instance("stone_block"),
        ]);
        layout.max_primitives = 3;
        let palette = valid_palette(vec![valid_prefab(
            "stone_block",
            vec![valid_primitive(), valid_primitive()],
        )]);

        let error = layout.validate_against_palette(&palette).unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::ExpandedPrimitiveBudgetExceeded {
                count: 4,
                limit: 3,
            }
        );
        assert_validation_report(
            &error,
            "expanded_primitive_budget_exceeded",
            "instances",
            &["expands to 4 primitives", "limit 3"],
        );
    }

    #[test]
    fn layout_rejects_missing_palette_path() {
        let mut layout = valid_layout(Vec::new());
        layout.palette = None;
        layout.palettes = Vec::new();

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::MissingPalettePath
        );
        assert_validation_report(&error, "missing_palette_path", "palette", &["at least one"]);
    }

    #[test]
    fn layout_rejects_unsafe_palette_path() {
        let mut layout = valid_layout(Vec::new());
        layout.palette = Some("../prefabs/home.ron".to_string());

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidPalettePath {
                palette_index: 0,
                path: "../prefabs/home.ron".to_string(),
                reason: FangyuanSceneLayoutPathInvalidReason::ParentOrEmptySegment,
            }
        );
        assert_validation_report(
            &error,
            "invalid_palette_path",
            "palette",
            &["parent segments"],
        );
    }

    #[test]
    fn layout_rejects_palette_path_outside_fangyuan_root() {
        let mut layout = valid_layout(Vec::new());
        layout.palette = Some("scenes/fangyuan_home/layout.ron".to_string());

        let error = layout.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutValidationError::InvalidPalettePath {
                palette_index: 0,
                path: "scenes/fangyuan_home/layout.ron".to_string(),
                reason: FangyuanSceneLayoutPathInvalidReason::OutsideFangyuanRoot,
            }
        );
        assert_validation_report(
            &error,
            "invalid_palette_path",
            "palette",
            &["assets/fangyuan"],
        );
    }

    #[test]
    fn home_scene_layout_asset_loads_and_validates_against_home_palette() {
        let layout =
            FangyuanSceneLayout::load_first_package_ron(FANGYUAN_HOME_SCENE_LAYOUT_PATH).unwrap();
        let palette = FangyuanPrefabPalette::load_validated_first_package_ron(
            crate::framework::fangyuan::FANGYUAN_HOME_PREFAB_PALETTE_PATH,
        )
        .unwrap();

        layout.validate_against_palette(&palette).unwrap();

        assert_eq!(layout.name, "home_layout");
        assert_eq!(
            layout.palette_paths().collect::<Vec<_>>(),
            vec![crate::framework::fangyuan::FANGYUAN_HOME_PREFAB_PALETTE_PATH]
        );
        assert!(
            layout
                .instances
                .iter()
                .filter(|instance| instance.prefab == "fence_segment")
                .count()
                >= 2
        );
        assert!(
            layout
                .instances
                .iter()
                .filter(|instance| instance.prefab == "dragon_body_segment")
                .count()
                >= 2
        );

        let generated_primitives = estimate_generated_primitives(&layout, &palette);
        assert!(generated_primitives <= 1000);
        assert!(generated_primitives <= layout.max_primitives);
    }

    #[test]
    fn layout_compile_expands_multiple_instances_with_position_scale_and_pivot() {
        let mut primitive = valid_primitive();
        primitive.position = [1.0, 0.5, 2.0];
        primitive.size = [0.5, 0.5, 1.0];
        primitive.color = [0.1, 0.2, 0.3, 0.4];
        let mut prefab = valid_prefab("stone_block", vec![primitive]);
        prefab.pivot = Some([0.5, 0.0, 1.0]);
        let palette = valid_palette(vec![prefab]);
        let mut first = valid_instance("stone_block");
        first.id = Some("stone_a".to_string());
        first.position = [2.0, 0.0, -1.0];
        first.scale = [2.0, 3.0, 0.5];
        let mut second = valid_instance("stone_block");
        second.id = Some("stone_b".to_string());
        second.position = [-2.0, 0.0, 1.0];
        second.scale = [1.0, 1.0, 1.0];
        let layout = valid_layout(vec![first, second]);

        let report = layout.compile_with_palette(&palette).unwrap();

        assert_eq!(report.palette_count, 1);
        assert_eq!(report.prefab_count, 1);
        assert_eq!(report.authored_prefab_primitives, 1);
        assert_eq!(report.instance_count, 2);
        assert_eq!(report.generated_primitives, 2);
        assert_eq!(report.skipped_primitives, 0);
        assert_eq!(report.used_prefab_count, 1);
        assert_eq!(report.primitive_stats.total, 2);
        assert_eq!(report.primitive_stats.cube_count, 2);
        assert_eq!(report.primitive_stats.material_profile_count, 0);
        assert!(report.top_level_validated);
        assert!(report.layout_validated);
        assert!(report.palette_validated);
        assert!(report.warnings.is_empty());
        assert_eq!(report.primitive_set.len(), 2);
        assert_eq!(
            report.primitive_set.primitives()[0].local_position,
            Vec3::new(3.0, 1.5, -0.5)
        );
        assert_eq!(
            report.primitive_set.primitives()[0].scale,
            Vec3::new(1.0, 1.5, 0.5)
        );
        assert_eq!(
            report.primitive_set.primitives()[1].local_position,
            Vec3::new(-1.5, 0.5, 2.0)
        );
    }

    #[test]
    fn layout_compile_preserves_runtime_primitive_fields() {
        let mut primitive = valid_primitive();
        primitive.kind = FangyuanPrimitiveKind::Sphere;
        primitive.role = Some(FangyuanPrimitiveRole::Trail);
        primitive.color = [0.2, 0.3, 0.4, 0.5];
        primitive.alpha = Some(0.6);
        primitive.emissive = Some(2.5);
        primitive.material_profile_id = Some("fx/trail:soft".to_string());
        primitive.lifecycle = Some(FangyuanPrimitiveLifecycle::new(Some(10), Some(2), Some(20)));
        let palette = valid_palette(vec![valid_prefab("glow_orb", vec![primitive])]);
        let layout = valid_layout(vec![valid_instance("glow_orb")]);

        let report = layout.compile_with_palette(&palette).unwrap();
        let generated = &report.primitive_set.primitives()[0];
        let color = generated.color.to_srgba();

        assert_eq!(report.primitive_stats.total, 1);
        assert_eq!(report.primitive_stats.sphere_count, 1);
        assert_eq!(report.primitive_stats.alpha_count, 1);
        assert_eq!(report.primitive_stats.emissive_count, 1);
        assert_eq!(report.primitive_stats.material_profile_count, 1);
        assert_eq!(generated.kind, FangyuanPrimitiveKind::Sphere);
        assert_eq!(generated.role, FangyuanPrimitiveRole::Trail);
        assert_eq!(
            (color.red, color.green, color.blue, color.alpha),
            (0.2, 0.3, 0.4, 0.5)
        );
        assert_eq!(generated.alpha, 0.6);
        assert_eq!(generated.emissive, 2.5);
        assert_eq!(
            generated.material_profile_id.as_deref(),
            Some("fx/trail:soft")
        );
        assert_eq!(
            generated.lifecycle,
            FangyuanPrimitiveLifecycle::new(Some(10), Some(2), Some(20))
        );
    }

    #[test]
    fn layout_compile_skips_expanded_primitives_rejected_by_unified_validator() {
        let invalid_after_instance_transform = FangyuanPrimitiveBlueprint::new(
            FangyuanPrimitiveKind::Cube,
            [2.1, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.4, 0.6, 1.0],
        );
        let palette = valid_palette(vec![valid_prefab(
            "stone_block",
            vec![valid_primitive(), invalid_after_instance_transform],
        )]);
        let mut instance = valid_instance("stone_block");
        instance.id = Some("stone_a".to_string());
        instance.position = [4.0, 0.0, 0.0];
        let layout = valid_layout(vec![instance]);

        let report = layout.compile_with_palette(&palette).unwrap();

        assert_eq!(report.primitive_set.len(), 1);
        assert_eq!(report.generated_primitives, 1);
        assert_eq!(report.skipped_primitives, 1);
        assert_eq!(report.primitive_stats.total, 1);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].instance_index, 0);
        assert_eq!(report.warnings[0].instance_id.as_deref(), Some("stone_a"));
        assert_eq!(report.warnings[0].prefab_id, "stone_block");
        assert_eq!(report.warnings[0].prefab_primitive_index, 1);
        assert!(matches!(
            report.warnings[0].source,
            FangyuanBlueprintValidationError::InvalidPrimitivePosition { .. }
        ));
    }

    #[test]
    fn layout_compile_rejects_missing_prefab_as_structured_error() {
        let layout = valid_layout(vec![valid_instance("missing_prefab")]);
        let palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);

        let error = layout.compile_with_palette(&palette).unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                FangyuanSceneLayoutValidationError::MissingPrefab {
                    instance_index: 0,
                    prefab: "missing_prefab".to_string(),
                }
            )
        );
        assert_compile_error_report(
            &error,
            "missing_prefab",
            "instances[0].prefab",
            &["missing_prefab", "not present"],
        );
    }

    #[test]
    fn layout_compile_rejects_illegal_instance_as_structured_error() {
        let layout = valid_layout(vec![valid_instance("Stone/Block")]);
        let palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);

        let error = layout.compile_with_palette(&palette).unwrap_err();

        assert!(matches!(
            error,
            FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                FangyuanSceneLayoutValidationError::InvalidInstancePrefabId { .. }
            )
        ));
    }

    #[test]
    fn layout_compile_rejects_invalid_palette_as_structured_error() {
        let layout = valid_layout(vec![valid_instance("stone_block")]);
        let mut palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);
        palette.version = "2".to_string();

        let error = layout.compile_with_palette(&palette).unwrap_err();

        assert!(matches!(
            error,
            FangyuanSceneLayoutCompileError::PaletteValidationFailed(
                FangyuanPrefabValidationError::UnsupportedVersion { .. }
            )
        ));
        assert_compile_error_report(
            &error,
            "unsupported_version",
            "version",
            &["unsupported", "expected"],
        );
    }

    #[test]
    fn layout_compile_rejects_budget_above_layout_limit() {
        let mut layout = valid_layout(vec![
            valid_instance("stone_block"),
            valid_instance("stone_block"),
        ]);
        layout.max_primitives = 3;
        let palette = valid_palette(vec![valid_prefab(
            "stone_block",
            vec![valid_primitive(), valid_primitive()],
        )]);

        let error = layout.compile_with_palette(&palette).unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                FangyuanSceneLayoutValidationError::ExpandedPrimitiveBudgetExceeded {
                    count: 4,
                    limit: 3,
                }
            )
        );
    }

    #[test]
    fn layout_compile_rejects_many_small_instances_above_hard_limit() {
        let mut layout = valid_layout(
            (0..=FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT)
                .map(|index| {
                    let mut instance = valid_instance("stone_block");
                    instance.id = Some(format!("stone_{index}"));
                    instance
                })
                .collect(),
        );
        layout.max_primitives = FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT;
        let palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);

        let error = layout.compile_with_palette(&palette).unwrap_err();

        assert_eq!(
            error,
            FangyuanSceneLayoutCompileError::LayoutValidationFailed(
                FangyuanSceneLayoutValidationError::ExpandedPrimitiveBudgetExceeded {
                    count: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT + 1,
                    limit: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
                }
            )
        );
        assert_compile_error_report(
            &error,
            "expanded_primitive_budget_exceeded",
            "instances",
            &[
                "layout expands to 1001 primitives",
                "max_primitives limit 1000",
            ],
        );
    }

    #[test]
    fn home_scene_layout_asset_compiles_with_home_palette() {
        let layout =
            FangyuanSceneLayout::load_first_package_ron(FANGYUAN_HOME_SCENE_LAYOUT_PATH).unwrap();
        let palette = FangyuanPrefabPalette::load_validated_first_package_ron(
            crate::framework::fangyuan::FANGYUAN_HOME_PREFAB_PALETTE_PATH,
        )
        .unwrap();

        let report = layout.compile_with_palette(&palette).unwrap();

        assert_eq!(report.palette_count, layout.palette_paths().count());
        assert_eq!(report.prefab_count, palette.prefabs.len());
        assert_eq!(report.instance_count, layout.instances.len());
        assert_eq!(report.skipped_primitives, 0);
        assert!(report.generated_primitives > 0);
        assert_eq!(report.generated_primitives, report.primitive_set.len());
        assert_eq!(report.primitive_stats, report.primitive_set.stats());
        assert!(report.used_prefab_count >= 4);
        assert!(report.authored_prefab_primitives <= palette.max_primitives);
        assert!(report.generated_primitives <= layout.max_primitives);
    }

    #[test]
    fn scene_layout_path_policy_reuses_fangyuan_first_package_rules() {
        assert_eq!(
            validate_fangyuan_scene_layout_asset_path(FANGYUAN_HOME_SCENE_LAYOUT_PATH),
            Ok(())
        );

        assert_eq!(
            validate_fangyuan_scene_layout_asset_path("scenes/fangyuan_home/layout.ron"),
            Err(FangyuanSceneLayoutPathError::OutsideFangyuanRoot(
                "scenes/fangyuan_home/layout.ron".to_string()
            ))
        );
        assert_eq!(
            validate_fangyuan_scene_layout_asset_path("../fangyuan/layouts/home_layout.ron"),
            Err(FangyuanSceneLayoutPathError::ParentOrEmptySegment(
                "../fangyuan/layouts/home_layout.ron".to_string()
            ))
        );
        assert_eq!(
            validate_fangyuan_scene_layout_asset_path("fangyuan\\layouts\\home_layout.ron"),
            Err(FangyuanSceneLayoutPathError::Backslash(
                "fangyuan\\layouts\\home_layout.ron".to_string()
            ))
        );
        assert!(matches!(
            validate_fangyuan_scene_layout_asset_path(
                "C:/project/assets/fangyuan/layouts/home_layout.ron"
            ),
            Err(FangyuanSceneLayoutPathError::WindowsDrive(_))
        ));
        assert!(matches!(
            validate_fangyuan_scene_layout_asset_path("/fangyuan/layouts/home_layout.ron"),
            Err(FangyuanSceneLayoutPathError::Absolute(_))
        ));
    }

    #[test]
    fn layout_rejects_forbidden_top_level_fields_by_parse() {
        for field in [
            "rotation",
            "quaternion",
            "euler",
            "angular_velocity",
            "rotate",
            "spin",
        ] {
            let source = valid_layout_ron_with_extra_top_level_field(field);

            assert_parse_error_contains(
                FangyuanSceneLayout::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    #[test]
    fn layout_rejects_forbidden_instance_fields_by_parse() {
        for field in [
            "rotation",
            "quaternion",
            "euler",
            "angular_velocity",
            "rotate",
            "spin",
            "material_override",
        ] {
            let source = valid_layout_ron_with_extra_instance_field(field);

            assert_parse_error_contains(
                FangyuanSceneLayout::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    #[test]
    fn layout_rejects_nested_prefab_or_layout_fields_by_parse() {
        for field in ["prefabs", "children", "prefab", "layouts", "layout"] {
            let source = valid_layout_ron_with_extra_top_level_field(field);

            assert_parse_error_contains(
                FangyuanSceneLayout::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    #[test]
    fn palette_rejects_nested_prefab_fields_by_parse() {
        for field in ["prefabs", "children", "prefab", "instances"] {
            let source = valid_palette_ron_with_extra_prefab_field(field);

            assert_parse_error_contains(
                FangyuanPrefabPalette::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    fn assert_validation_report(
        error: &FangyuanSceneLayoutValidationError,
        code: &'static str,
        field_path: &str,
        reason_parts: &[&str],
    ) {
        assert_eq!(error.code(), code);
        assert_eq!(error.field_path().as_ref(), field_path);

        let reason = error.reason();
        for part in reason_parts {
            assert!(
                reason.contains(part),
                "reason `{reason}` should contain `{part}`"
            );
        }

        let message = error.to_string();
        assert!(
            message.contains(code),
            "message `{message}` should contain code `{code}`"
        );
        assert!(
            message.contains(field_path),
            "message `{message}` should contain field path `{field_path}`"
        );
        for part in reason_parts {
            assert!(
                message.contains(part),
                "message `{message}` should contain `{part}`"
            );
        }
    }

    fn assert_compile_error_report(
        error: &FangyuanSceneLayoutCompileError,
        code: &'static str,
        field_path: &str,
        reason_parts: &[&str],
    ) {
        assert_eq!(error.code(), code);
        assert_eq!(error.field_path().as_ref(), field_path);

        let reason = error.reason();
        for part in reason_parts {
            assert!(
                reason.contains(part),
                "reason `{reason}` should contain `{part}`"
            );
        }
    }

    fn assert_parse_error_contains<T, E>(result: Result<T, E>, field: &str, expected: &str)
    where
        E: fmt::Display,
    {
        let error = match result {
            Ok(_) => panic!("expected parse error for field `{field}`"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(
            message.contains(field),
            "parse error `{message}` should contain field `{field}`"
        );
        assert!(
            message.contains(expected),
            "parse error `{message}` should contain `{expected}`"
        );
    }

    fn valid_layout(instances: Vec<FangyuanSceneLayoutInstance>) -> FangyuanSceneLayout {
        FangyuanSceneLayout {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "home_layout".to_string(),
            description: String::new(),
            bounds: FangyuanBlueprintBounds::new(10.0, 10.0, 8.0),
            palette: Some("fangyuan/prefabs/home.ron".to_string()),
            palettes: Vec::new(),
            max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            instances,
        }
    }

    fn valid_instance(prefab: &str) -> FangyuanSceneLayoutInstance {
        FangyuanSceneLayoutInstance {
            id: None,
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
            name: "starter_palette".to_string(),
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

    fn valid_primitive() -> FangyuanPrimitiveBlueprint {
        FangyuanPrimitiveBlueprint::new(
            FangyuanPrimitiveKind::Cube,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.4, 0.6, 1.0],
        )
    }

    fn estimate_generated_primitives(
        layout: &FangyuanSceneLayout,
        palette: &FangyuanPrefabPalette,
    ) -> usize {
        layout
            .instances
            .iter()
            .map(|instance| {
                palette
                    .prefabs
                    .iter()
                    .find(|prefab| prefab.id == instance.prefab)
                    .map(|prefab| prefab.primitives.len())
                    .unwrap_or(0)
            })
            .sum()
    }

    fn valid_layout_ron_with_extra_top_level_field(field: &str) -> String {
        format!(
            r#"
(
    version: "1",
    name: "home_layout",
    description: "",
    bounds: (width: 10.0, depth: 10.0, height: 8.0),
    palette: "fangyuan/prefabs/home.ron",
    max_primitives: 8,
    instances: [],
    {field}: "forbidden",
)
"#
        )
    }

    fn valid_layout_ron_with_extra_instance_field(field: &str) -> String {
        format!(
            r#"
(
    version: "1",
    name: "home_layout",
    description: "",
    bounds: (width: 10.0, depth: 10.0, height: 8.0),
    palette: "fangyuan/prefabs/home.ron",
    max_primitives: 8,
    instances: [
        (
            prefab: "stone_block",
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            {field}: "forbidden",
        ),
    ],
)
"#
        )
    }

    fn valid_palette_ron_with_extra_prefab_field(field: &str) -> String {
        format!(
            r#"
(
    version: "1",
    name: "starter_palette",
    description: "",
    max_primitives: 8,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    prefabs: [
        (
            id: "stone_block",
            name: "Stone Block",
            description: "",
            primitives: [
                (
                    kind: "cube",
                    position: [0.0, 1.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    color: [0.2, 0.4, 0.6, 1.0],
                ),
            ],
            {field}: [],
        ),
    ],
)
"#
        )
    }
}
