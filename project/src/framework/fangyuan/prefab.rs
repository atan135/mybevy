use serde::{Deserialize, Deserializer, Serialize};
use std::{borrow::Cow, collections::HashSet, error::Error, fmt, fs, io, path::PathBuf};

use super::{
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT, FANGYUAN_BLUEPRINT_VERSION, FangyuanAssetPathError,
    FangyuanAuditBudgetProfile, FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity,
    FangyuanAuditSourceKind, FangyuanBlueprintBounds, FangyuanBlueprintValidationError,
    FangyuanPrimitiveBlueprint, FangyuanPrimitiveBudgetStats, audit_fangyuan_primitive_budget,
    compile_blueprint_primitive_to_runtime, first_package_fangyuan_asset_fs_path,
    validate_blueprint_primitive, validate_fangyuan_asset_path,
};

pub const FANGYUAN_PREFAB_PALETTE_VERSION: &str = FANGYUAN_BLUEPRINT_VERSION;
pub const FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT: usize =
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT;
pub const FANGYUAN_HOME_PREFAB_PALETTE_PATH: &str = "fangyuan/palettes/home_prefabs.ron";
pub const FANGYUAN_PREFAB_ID_MAX_LEN: usize = 64;
pub const FANGYUAN_PREFAB_TAG_MAX_LEN: usize = 48;
pub const FANGYUAN_PREFAB_MAX_TAGS: usize = 16;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPrefabPalette {
    pub version: String,
    pub name: String,
    pub description: String,
    pub max_primitives: usize,
    pub bounds: FangyuanBlueprintBounds,
    pub prefabs: Vec<FangyuanPrefabDefinition>,
}

impl FangyuanPrefabPalette {
    pub fn from_ron_str(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str::<Self>(source)
    }

    pub fn load_first_package_ron(
        palette_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanPrefabPaletteLoadError> {
        let palette_path = palette_path.as_ref().trim();
        validate_fangyuan_prefab_palette_asset_path(palette_path)
            .map_err(FangyuanPrefabPaletteLoadError::InvalidPath)?;

        let fs_path = first_package_fangyuan_asset_fs_path(palette_path).ok_or_else(|| {
            FangyuanPrefabPaletteLoadError::PrefabPaletteNotFound(palette_path.to_string())
        })?;

        let source = fs::read_to_string(&fs_path).map_err(|source| {
            FangyuanPrefabPaletteLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            }
        })?;

        Self::from_ron_str(&source).map_err(|source| FangyuanPrefabPaletteLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }

    pub fn load_validated_first_package_ron(
        palette_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanPrefabPaletteLoadError> {
        let palette = Self::load_first_package_ron(palette_path)?;
        palette
            .validate()
            .map_err(FangyuanPrefabPaletteLoadError::ValidationFailed)?;
        Ok(palette)
    }

    pub fn validate(&self) -> Result<(), FangyuanPrefabValidationError> {
        if self.version != FANGYUAN_PREFAB_PALETTE_VERSION {
            return Err(FangyuanPrefabValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_PREFAB_PALETTE_VERSION,
            });
        }

        self.bounds
            .validate()
            .map_err(|source| FangyuanPrefabValidationError::InvalidPaletteBounds { source })?;

        if self.max_primitives > FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT {
            return Err(
                FangyuanPrefabValidationError::PalettePrimitiveBudgetExceeded {
                    max_primitives: self.max_primitives,
                    hard_limit: FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT,
                },
            );
        }

        let mut ids = HashSet::with_capacity(self.prefabs.len());
        let mut total_primitives = 0usize;
        for (prefab_index, prefab) in self.prefabs.iter().enumerate() {
            validate_prefab_id(&prefab.id).map_err(|reason| {
                FangyuanPrefabValidationError::InvalidPrefabId {
                    prefab_index,
                    id: prefab.id.clone(),
                    reason,
                }
            })?;

            if !ids.insert(prefab.id.as_str()) {
                return Err(FangyuanPrefabValidationError::DuplicatePrefabId {
                    prefab_index,
                    id: prefab.id.clone(),
                });
            }

            let bounds = prefab.bounds.unwrap_or(self.bounds);
            bounds.validate().map_err(|source| {
                FangyuanPrefabValidationError::InvalidPrefabBounds {
                    prefab_index,
                    id: prefab.id.clone(),
                    source,
                }
            })?;

            validate_prefab_pivot(prefab_index, &prefab.id, prefab.pivot)?;
            validate_prefab_tags(prefab_index, &prefab.id, &prefab.tags)?;
            validate_prefab_primitive_budget(
                prefab_index,
                &prefab.id,
                prefab.primitives.len(),
                prefab.max_primitives,
                self.max_primitives,
            )?;

            for (primitive_index, primitive) in prefab.primitives.iter().enumerate() {
                validate_blueprint_primitive(primitive_index, primitive, &bounds).map_err(
                    |source| FangyuanPrefabValidationError::InvalidPrefabPrimitive {
                        prefab_index,
                        prefab_id: prefab.id.clone(),
                        primitive_index,
                        source,
                    },
                )?;
            }

            total_primitives += prefab.primitives.len();
            if total_primitives > self.max_primitives {
                return Err(
                    FangyuanPrefabValidationError::TotalPrimitiveBudgetExceeded {
                        count: total_primitives,
                        limit: self.max_primitives,
                    },
                );
            }
        }

        Ok(())
    }

    pub fn audit(&self, profile: &FangyuanAuditBudgetProfile) -> FangyuanAuditReport {
        let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::PrefabPalette, None);
        let mut runtime_primitives = Vec::new();
        let mut skipped_primitives = 0usize;
        let mut total_authored_primitives = 0usize;
        let mut reusable_prefab_count = 0usize;

        if self.version != FANGYUAN_PREFAB_PALETTE_VERSION {
            add_prefab_validation_finding(
                &mut report,
                FangyuanPrefabValidationError::UnsupportedVersion {
                    found: self.version.clone(),
                    expected: FANGYUAN_PREFAB_PALETTE_VERSION,
                },
            );
        }

        let palette_bounds_valid = match self.bounds.validate() {
            Ok(()) => true,
            Err(source) => {
                add_prefab_validation_finding(
                    &mut report,
                    FangyuanPrefabValidationError::InvalidPaletteBounds { source },
                );
                false
            }
        };

        if self.max_primitives > FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT {
            add_prefab_validation_finding(
                &mut report,
                FangyuanPrefabValidationError::PalettePrimitiveBudgetExceeded {
                    max_primitives: self.max_primitives,
                    hard_limit: FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT,
                },
            );
        }

        let mut ids = HashSet::with_capacity(self.prefabs.len());
        for (prefab_index, prefab) in self.prefabs.iter().enumerate() {
            if prefab.primitives.len() > 1 {
                reusable_prefab_count += 1;
            }

            total_authored_primitives =
                total_authored_primitives.saturating_add(prefab.primitives.len());

            if let Err(reason) = validate_prefab_id(&prefab.id) {
                add_prefab_validation_finding(
                    &mut report,
                    FangyuanPrefabValidationError::InvalidPrefabId {
                        prefab_index,
                        id: prefab.id.clone(),
                        reason,
                    },
                );
            }

            if !ids.insert(prefab.id.as_str()) {
                add_prefab_validation_finding(
                    &mut report,
                    FangyuanPrefabValidationError::DuplicatePrefabId {
                        prefab_index,
                        id: prefab.id.clone(),
                    },
                );
            }

            let bounds = prefab.bounds.unwrap_or(self.bounds);
            let bounds_valid = if let Some(prefab_bounds) = prefab.bounds {
                match prefab_bounds.validate() {
                    Ok(()) => true,
                    Err(source) => {
                        add_prefab_validation_finding(
                            &mut report,
                            FangyuanPrefabValidationError::InvalidPrefabBounds {
                                prefab_index,
                                id: prefab.id.clone(),
                                source,
                            },
                        );
                        false
                    }
                }
            } else {
                palette_bounds_valid
            };

            if let Err(error) = validate_prefab_pivot(prefab_index, &prefab.id, prefab.pivot) {
                add_prefab_validation_finding(&mut report, error);
            }

            if let Err(error) = validate_prefab_tags(prefab_index, &prefab.id, &prefab.tags) {
                add_prefab_validation_finding(&mut report, error);
            }

            if let Err(error) = validate_prefab_primitive_budget(
                prefab_index,
                &prefab.id,
                prefab.primitives.len(),
                prefab.max_primitives,
                self.max_primitives,
            ) {
                add_prefab_validation_finding(&mut report, error);
            }

            if !bounds_valid {
                skipped_primitives = skipped_primitives.saturating_add(prefab.primitives.len());
                continue;
            }

            for (primitive_index, primitive) in prefab.primitives.iter().enumerate() {
                match validate_blueprint_primitive(primitive_index, primitive, &bounds) {
                    Ok(()) => {
                        runtime_primitives.push(compile_blueprint_primitive_to_runtime(primitive));
                    }
                    Err(source) => {
                        skipped_primitives += 1;
                        add_prefab_validation_finding(
                            &mut report,
                            FangyuanPrefabValidationError::InvalidPrefabPrimitive {
                                prefab_index,
                                prefab_id: prefab.id.clone(),
                                primitive_index,
                                source,
                            },
                        );
                    }
                }
            }
        }

        if total_authored_primitives > self.max_primitives {
            add_prefab_validation_finding(
                &mut report,
                FangyuanPrefabValidationError::TotalPrimitiveBudgetExceeded {
                    count: total_authored_primitives,
                    limit: self.max_primitives,
                },
            );
        }

        let mut stats = FangyuanPrimitiveBudgetStats::from_runtime_primitives(&runtime_primitives);
        stats.authored_primitives = total_authored_primitives;
        stats.generated_primitives = runtime_primitives.len();
        stats.skipped_primitives = skipped_primitives;
        stats.expanded_primitives = total_authored_primitives;

        let budget_report = audit_fangyuan_primitive_budget(&stats, profile);
        for mut finding in budget_report.findings {
            finding.source_kind = FangyuanAuditSourceKind::PrefabPalette;
            finding.field_path = finding.field_path.map(prefab_palette_budget_field_path);
            report.add_finding(finding);
        }
        for mut suggestion in budget_report.suggestions {
            suggestion.field_path = suggestion.field_path.map(prefab_palette_budget_field_path);
            report.add_suggestion(suggestion);
        }

        report.refresh_summary_and_status();
        report.apply_primitive_budget_stats(&stats);
        report.summary.prefab_count = self.prefabs.len();
        report.summary.reusable_prefab_count = reusable_prefab_count;
        report.sort_findings();
        report
    }

    pub fn audit_with_default_budget(&self) -> FangyuanAuditReport {
        self.audit(&FangyuanAuditBudgetProfile::default())
    }
}

pub fn load_fangyuan_home_prefab_palette()
-> Result<FangyuanPrefabPalette, FangyuanPrefabPaletteLoadError> {
    FangyuanPrefabPalette::load_validated_first_package_ron(FANGYUAN_HOME_PREFAB_PALETTE_PATH)
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPrefabDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_bounds",
        skip_serializing_if = "Option::is_none"
    )]
    pub bounds: Option<FangyuanBlueprintBounds>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f32_array_3",
        skip_serializing_if = "Option::is_none"
    )]
    pub pivot: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_usize",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_primitives: Option<usize>,
    pub primitives: Vec<FangyuanPrimitiveBlueprint>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanPrefabValidationError {
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    InvalidPaletteBounds {
        source: FangyuanBlueprintValidationError,
    },
    PalettePrimitiveBudgetExceeded {
        max_primitives: usize,
        hard_limit: usize,
    },
    InvalidPrefabId {
        prefab_index: usize,
        id: String,
        reason: FangyuanPrefabIdInvalidReason,
    },
    DuplicatePrefabId {
        prefab_index: usize,
        id: String,
    },
    InvalidPrefabBounds {
        prefab_index: usize,
        id: String,
        source: FangyuanBlueprintValidationError,
    },
    InvalidPrefabPivot {
        prefab_index: usize,
        id: String,
        axis: usize,
        value: f32,
    },
    TooManyPrefabTags {
        prefab_index: usize,
        id: String,
        count: usize,
        limit: usize,
    },
    InvalidPrefabTag {
        prefab_index: usize,
        id: String,
        tag_index: usize,
        tag: String,
        reason: FangyuanPrefabTagInvalidReason,
    },
    PrefabPrimitiveBudgetExceeded {
        prefab_index: usize,
        id: String,
        count: usize,
        limit: usize,
    },
    TotalPrimitiveBudgetExceeded {
        count: usize,
        limit: usize,
    },
    InvalidPrefabPrimitive {
        prefab_index: usize,
        prefab_id: String,
        primitive_index: usize,
        source: FangyuanBlueprintValidationError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanPrefabIdInvalidReason {
    Empty,
    TooLong { max_len: usize },
    MustStartWithLowercaseAscii,
    InvalidCharacter,
    PathLike,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanPrefabTagInvalidReason {
    Empty,
    TooLong { max_len: usize },
    InvalidCharacter,
}

impl FangyuanPrefabValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::InvalidPaletteBounds { .. } => "invalid_palette_bounds",
            Self::PalettePrimitiveBudgetExceeded { .. } => "palette_primitive_budget_exceeded",
            Self::InvalidPrefabId { .. } => "invalid_prefab_id",
            Self::DuplicatePrefabId { .. } => "duplicate_prefab_id",
            Self::InvalidPrefabBounds { .. } => "invalid_prefab_bounds",
            Self::InvalidPrefabPivot { .. } => "invalid_prefab_pivot",
            Self::TooManyPrefabTags { .. } => "too_many_prefab_tags",
            Self::InvalidPrefabTag { .. } => "invalid_prefab_tag",
            Self::PrefabPrimitiveBudgetExceeded { .. } => "prefab_primitive_budget_exceeded",
            Self::TotalPrimitiveBudgetExceeded { .. } => "total_primitive_budget_exceeded",
            Self::InvalidPrefabPrimitive { .. } => "invalid_prefab_primitive",
        }
    }

    pub fn field_path(&self) -> Cow<'static, str> {
        match self {
            Self::UnsupportedVersion { .. } => Cow::Borrowed("version"),
            Self::InvalidPaletteBounds { source } => Cow::Owned(format!(
                "bounds.{}",
                strip_bounds_prefix(source.field_path())
            )),
            Self::PalettePrimitiveBudgetExceeded { .. } => Cow::Borrowed("max_primitives"),
            Self::InvalidPrefabId { prefab_index, .. }
            | Self::DuplicatePrefabId { prefab_index, .. } => {
                Cow::Owned(format!("prefabs[{prefab_index}].id"))
            }
            Self::InvalidPrefabBounds {
                prefab_index,
                source,
                ..
            } => Cow::Owned(format!(
                "prefabs[{prefab_index}].bounds.{}",
                strip_bounds_prefix(source.field_path())
            )),
            Self::InvalidPrefabPivot {
                prefab_index, axis, ..
            } => Cow::Owned(format!("prefabs[{prefab_index}].pivot[{axis}]")),
            Self::TooManyPrefabTags { prefab_index, .. } => {
                Cow::Owned(format!("prefabs[{prefab_index}].tags"))
            }
            Self::InvalidPrefabTag {
                prefab_index,
                tag_index,
                ..
            } => Cow::Owned(format!("prefabs[{prefab_index}].tags[{tag_index}]")),
            Self::PrefabPrimitiveBudgetExceeded { prefab_index, .. } => {
                Cow::Owned(format!("prefabs[{prefab_index}].primitives"))
            }
            Self::TotalPrimitiveBudgetExceeded { .. } => Cow::Borrowed("prefabs"),
            Self::InvalidPrefabPrimitive {
                prefab_index,
                source,
                ..
            } => Cow::Owned(format!("prefabs[{prefab_index}].{}", source.field_path())),
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                format!("version `{found}` is unsupported; expected `{expected}`")
            }
            Self::InvalidPaletteBounds { source } => source.reason(),
            Self::PalettePrimitiveBudgetExceeded {
                max_primitives,
                hard_limit,
            } => {
                format!("max_primitives {max_primitives} exceeds hard limit {hard_limit}")
            }
            Self::InvalidPrefabId { id, reason, .. } => match reason {
                FangyuanPrefabIdInvalidReason::Empty => "id must not be empty".to_string(),
                FangyuanPrefabIdInvalidReason::TooLong { max_len } => {
                    format!("id `{id}` must contain at most {max_len} characters")
                }
                FangyuanPrefabIdInvalidReason::MustStartWithLowercaseAscii => {
                    format!("id `{id}` must start with a lowercase ASCII letter")
                }
                FangyuanPrefabIdInvalidReason::InvalidCharacter => {
                    format!("id `{id}` may only contain lowercase ASCII letters, digits, and `_`")
                }
                FangyuanPrefabIdInvalidReason::PathLike => {
                    format!("id `{id}` must not contain path-like separators or segments")
                }
            },
            Self::DuplicatePrefabId { id, .. } => {
                format!("id `{id}` is already used by an earlier prefab")
            }
            Self::InvalidPrefabBounds { source, .. } => source.reason(),
            Self::InvalidPrefabPivot { value, .. } => {
                format!("value {value} must be finite")
            }
            Self::TooManyPrefabTags { count, limit, .. } => {
                format!("contains {count} tags, exceeding limit {limit}")
            }
            Self::InvalidPrefabTag { tag, reason, .. } => match reason {
                FangyuanPrefabTagInvalidReason::Empty => "tag must not be empty".to_string(),
                FangyuanPrefabTagInvalidReason::TooLong { max_len } => {
                    format!("tag `{tag}` must contain at most {max_len} characters")
                }
                FangyuanPrefabTagInvalidReason::InvalidCharacter => format!(
                    "tag `{tag}` may only contain lowercase ASCII letters, digits, `_`, and `-`"
                ),
            },
            Self::PrefabPrimitiveBudgetExceeded {
                count, limit, id, ..
            } => {
                format!("prefab `{id}` contains {count} primitives, exceeding limit {limit}")
            }
            Self::TotalPrimitiveBudgetExceeded { count, limit } => {
                format!("palette contains {count} authored primitives, exceeding limit {limit}")
            }
            Self::InvalidPrefabPrimitive {
                prefab_id,
                primitive_index,
                source,
                ..
            } => format!(
                "prefab `{prefab_id}` primitive {primitive_index} failed blueprint primitive validation: {}",
                source.reason()
            ),
        }
    }
}

impl fmt::Display for FangyuanPrefabValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fangyuan prefab palette validation error [{}] at {}: {}",
            self.code(),
            self.field_path(),
            self.reason()
        )
    }
}

impl Error for FangyuanPrefabValidationError {}

#[derive(Debug)]
pub enum FangyuanPrefabPaletteLoadError {
    InvalidPath(FangyuanPrefabPalettePathError),
    PrefabPaletteNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ValidationFailed(FangyuanPrefabValidationError),
}

pub type FangyuanPrefabPalettePathError = FangyuanAssetPathError;

impl fmt::Display for FangyuanPrefabPaletteLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(error) => write!(formatter, "{error}"),
            Self::PrefabPaletteNotFound(path) => write!(
                formatter,
                "fangyuan prefab palette was not found under first package assets: {path}"
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read fangyuan prefab palette at {}: {source}",
                path.display()
            ),
            Self::ParseFailed { path, source } => write!(
                formatter,
                "failed to parse fangyuan prefab palette RON at {}: {source}",
                path.display()
            ),
            Self::ValidationFailed(error) => {
                write!(
                    formatter,
                    "fangyuan prefab palette validation failed: {error}"
                )
            }
        }
    }
}

impl Error for FangyuanPrefabPaletteLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidPath(error) => Some(error),
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::ValidationFailed(error) => Some(error),
            Self::PrefabPaletteNotFound(_) => None,
        }
    }
}

pub fn validate_fangyuan_prefab_palette_asset_path(
    path: &str,
) -> Result<(), FangyuanPrefabPalettePathError> {
    validate_fangyuan_asset_path(path)
}

pub(super) fn validate_prefab_id(id: &str) -> Result<(), FangyuanPrefabIdInvalidReason> {
    if id.is_empty() {
        return Err(FangyuanPrefabIdInvalidReason::Empty);
    }

    if id.len() > FANGYUAN_PREFAB_ID_MAX_LEN {
        return Err(FangyuanPrefabIdInvalidReason::TooLong {
            max_len: FANGYUAN_PREFAB_ID_MAX_LEN,
        });
    }

    if id.contains('/')
        || id.contains('\\')
        || id.contains('.')
        || id.contains(':')
        || id.contains("..")
    {
        return Err(FangyuanPrefabIdInvalidReason::PathLike);
    }

    let mut chars = id.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return Err(FangyuanPrefabIdInvalidReason::MustStartWithLowercaseAscii);
    }

    if !chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        return Err(FangyuanPrefabIdInvalidReason::InvalidCharacter);
    }

    Ok(())
}

fn validate_prefab_pivot(
    prefab_index: usize,
    id: &str,
    pivot: Option<[f32; 3]>,
) -> Result<(), FangyuanPrefabValidationError> {
    let Some(pivot) = pivot else {
        return Ok(());
    };

    for (axis, value) in pivot.into_iter().enumerate() {
        if !value.is_finite() {
            return Err(FangyuanPrefabValidationError::InvalidPrefabPivot {
                prefab_index,
                id: id.to_string(),
                axis,
                value,
            });
        }
    }

    Ok(())
}

fn validate_prefab_tags(
    prefab_index: usize,
    id: &str,
    tags: &[String],
) -> Result<(), FangyuanPrefabValidationError> {
    if tags.len() > FANGYUAN_PREFAB_MAX_TAGS {
        return Err(FangyuanPrefabValidationError::TooManyPrefabTags {
            prefab_index,
            id: id.to_string(),
            count: tags.len(),
            limit: FANGYUAN_PREFAB_MAX_TAGS,
        });
    }

    for (tag_index, tag) in tags.iter().enumerate() {
        validate_prefab_tag(tag).map_err(|reason| {
            FangyuanPrefabValidationError::InvalidPrefabTag {
                prefab_index,
                id: id.to_string(),
                tag_index,
                tag: tag.clone(),
                reason,
            }
        })?;
    }

    Ok(())
}

pub(super) fn validate_prefab_tag(tag: &str) -> Result<(), FangyuanPrefabTagInvalidReason> {
    if tag.is_empty() {
        return Err(FangyuanPrefabTagInvalidReason::Empty);
    }

    if tag.len() > FANGYUAN_PREFAB_TAG_MAX_LEN {
        return Err(FangyuanPrefabTagInvalidReason::TooLong {
            max_len: FANGYUAN_PREFAB_TAG_MAX_LEN,
        });
    }

    if tag.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '_'
            || character == '-'
    }) {
        Ok(())
    } else {
        Err(FangyuanPrefabTagInvalidReason::InvalidCharacter)
    }
}

fn validate_prefab_primitive_budget(
    prefab_index: usize,
    id: &str,
    count: usize,
    prefab_max_primitives: Option<usize>,
    palette_max_primitives: usize,
) -> Result<(), FangyuanPrefabValidationError> {
    let limit = prefab_max_primitives
        .unwrap_or(palette_max_primitives)
        .min(palette_max_primitives);
    if count > limit {
        return Err(
            FangyuanPrefabValidationError::PrefabPrimitiveBudgetExceeded {
                prefab_index,
                id: id.to_string(),
                count,
                limit,
            },
        );
    }

    Ok(())
}

fn add_prefab_validation_finding(
    report: &mut FangyuanAuditReport,
    error: FangyuanPrefabValidationError,
) {
    report.add_finding(prefab_validation_error_to_audit_finding(
        &error,
        FangyuanAuditSeverity::Error,
    ));
}

fn prefab_validation_error_to_audit_finding(
    error: &FangyuanPrefabValidationError,
    severity: FangyuanAuditSeverity,
) -> FangyuanAuditFinding {
    let mut finding = FangyuanAuditFinding::new(
        severity,
        error.code(),
        error.reason(),
        FangyuanAuditSourceKind::PrefabPalette,
    );
    finding.field_path = Some(error.field_path().into_owned());
    finding.prefab_id = match error {
        FangyuanPrefabValidationError::InvalidPrefabId { id, .. }
        | FangyuanPrefabValidationError::DuplicatePrefabId { id, .. }
        | FangyuanPrefabValidationError::InvalidPrefabBounds { id, .. }
        | FangyuanPrefabValidationError::InvalidPrefabPivot { id, .. }
        | FangyuanPrefabValidationError::TooManyPrefabTags { id, .. }
        | FangyuanPrefabValidationError::InvalidPrefabTag { id, .. }
        | FangyuanPrefabValidationError::PrefabPrimitiveBudgetExceeded { id, .. } => {
            Some(id.clone())
        }
        FangyuanPrefabValidationError::InvalidPrefabPrimitive { prefab_id, .. } => {
            Some(prefab_id.clone())
        }
        FangyuanPrefabValidationError::UnsupportedVersion { .. }
        | FangyuanPrefabValidationError::InvalidPaletteBounds { .. }
        | FangyuanPrefabValidationError::PalettePrimitiveBudgetExceeded { .. }
        | FangyuanPrefabValidationError::TotalPrimitiveBudgetExceeded { .. } => None,
    };
    finding.prefab_primitive_index = match error {
        FangyuanPrefabValidationError::InvalidPrefabPrimitive {
            primitive_index, ..
        } => Some(*primitive_index),
        _ => None,
    };
    finding
}

fn prefab_palette_budget_field_path(field_path: String) -> String {
    if field_path == "primitives" {
        "prefabs[].primitives".to_string()
    } else if let Some(suffix) = field_path.strip_prefix("primitives[]") {
        format!("prefabs[].primitives[]{suffix}")
    } else {
        field_path
    }
}

fn strip_bounds_prefix(field_path: Cow<'_, str>) -> String {
    field_path
        .strip_prefix("bounds.")
        .unwrap_or(field_path.as_ref())
        .to_string()
}

fn deserialize_optional_bounds<'de, D>(
    deserializer: D,
) -> Result<Option<FangyuanBlueprintBounds>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_value(deserializer)
}

fn deserialize_optional_f32_array_3<'de, D>(deserializer: D) -> Result<Option<[f32; 3]>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_value(deserializer)
}

fn deserialize_optional_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
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
    use crate::framework::fangyuan::{
        FangyuanAuditBudgetProfile, FangyuanAuditSeverity, FangyuanAuditSourceKind,
        FangyuanAuditStatus, FangyuanPrimitiveKind,
    };

    #[test]
    fn valid_palette_accepts_prefab_metadata_and_blueprint_primitives() {
        let palette = FangyuanPrefabPalette::from_ron_str(
            r#"
(
    version: "1",
    name: "starter_palette",
    description: "Small authored prefab palette.",
    max_primitives: 8,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    prefabs: [
        (
            id: "stone_block",
            name: "Stone Block",
            description: "One cube.",
            bounds: (width: 2.0, depth: 2.0, height: 2.0),
            pivot: [0.0, 0.0, 0.0],
            tags: ["structure", "starter"],
            max_primitives: 2,
            primitives: [
                (
                    kind: "cube",
                    position: [0.0, 0.5, 0.0],
                    size: [1.0, 1.0, 1.0],
                    color: [0.4, 0.5, 0.6, 1.0],
                ),
            ],
        ),
        (
            id: "glow_orb",
            name: "Glow Orb",
            description: "One sphere.",
            primitives: [
                (
                    kind: "sphere",
                    role: "decoration",
                    position: [0.0, 1.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    color: [0.3, 0.7, 1.0, 0.8],
                    alpha: Some(0.75),
                    emissive: Some(2.0),
                    material_profile_id: Some("prefab_glow"),
                ),
            ],
        ),
    ],
)
"#,
        )
        .unwrap();

        palette.validate().unwrap();

        assert_eq!(palette.version, FANGYUAN_PREFAB_PALETTE_VERSION);
        assert_eq!(palette.prefabs.len(), 2);
        assert_eq!(palette.prefabs[0].id, "stone_block");
        assert_eq!(
            palette.prefabs[0].primitives[0].kind,
            FangyuanPrimitiveKind::Cube
        );
    }

    #[test]
    fn palette_rejects_unsupported_version() {
        let mut palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);
        palette.version = "2".to_string();

        let error = palette.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanPrefabValidationError::UnsupportedVersion {
                found: "2".to_string(),
                expected: FANGYUAN_PREFAB_PALETTE_VERSION,
            }
        );
        assert_validation_report(&error, "unsupported_version", "version", &["expected `1`"]);
    }

    #[test]
    fn palette_rejects_duplicate_prefab_id() {
        let palette = valid_palette(vec![
            valid_prefab("stone_block", vec![valid_primitive()]),
            valid_prefab("stone_block", vec![valid_primitive()]),
        ]);

        let error = palette.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanPrefabValidationError::DuplicatePrefabId {
                prefab_index: 1,
                id: "stone_block".to_string(),
            }
        );
        assert_validation_report(
            &error,
            "duplicate_prefab_id",
            "prefabs[1].id",
            &["already used"],
        );
    }

    #[test]
    fn palette_rejects_illegal_prefab_ids() {
        for (id, reason) in [
            ("", FangyuanPrefabIdInvalidReason::Empty),
            (
                "Stone",
                FangyuanPrefabIdInvalidReason::MustStartWithLowercaseAscii,
            ),
            (
                "1stone",
                FangyuanPrefabIdInvalidReason::MustStartWithLowercaseAscii,
            ),
            (
                "stone-block",
                FangyuanPrefabIdInvalidReason::InvalidCharacter,
            ),
            (
                "stone block",
                FangyuanPrefabIdInvalidReason::InvalidCharacter,
            ),
            ("stone:block", FangyuanPrefabIdInvalidReason::PathLike),
            ("stone/block", FangyuanPrefabIdInvalidReason::PathLike),
            ("stone\\block", FangyuanPrefabIdInvalidReason::PathLike),
            ("stone.block", FangyuanPrefabIdInvalidReason::PathLike),
            ("..", FangyuanPrefabIdInvalidReason::PathLike),
        ] {
            let palette = valid_palette(vec![valid_prefab(id, vec![valid_primitive()])]);

            let error = palette.validate().unwrap_err();

            assert_eq!(
                error,
                FangyuanPrefabValidationError::InvalidPrefabId {
                    prefab_index: 0,
                    id: id.to_string(),
                    reason,
                }
            );
            assert_validation_report(&error, "invalid_prefab_id", "prefabs[0].id", &["id"]);
        }
    }

    #[test]
    fn palette_rejects_palette_budget_above_hard_limit() {
        let mut palette = valid_palette(vec![valid_prefab("stone_block", vec![valid_primitive()])]);
        palette.max_primitives = FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT + 1;

        let error = palette.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanPrefabValidationError::PalettePrimitiveBudgetExceeded {
                max_primitives: FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT + 1,
                hard_limit: FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT,
            }
        );
    }

    #[test]
    fn palette_accepts_default_bounds_pivot_tags_and_prefab_budget_metadata() {
        let mut prefab = valid_prefab("stone_block", vec![valid_primitive()]);
        prefab.bounds = Some(FangyuanBlueprintBounds::new(2.0, 2.0, 2.0));
        prefab.pivot = Some([0.0, 0.0, 0.0]);
        prefab.tags = vec!["structure".to_string(), "starter-tag".to_string()];
        prefab.max_primitives = Some(2);
        let palette = valid_palette(vec![prefab]);

        palette.validate().unwrap();
    }

    #[test]
    fn palette_rejects_prefab_primitive_count_above_own_budget() {
        let mut prefab = valid_prefab("stone_block", vec![valid_primitive(), valid_primitive()]);
        prefab.max_primitives = Some(1);
        let palette = valid_palette(vec![prefab]);

        let error = palette.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanPrefabValidationError::PrefabPrimitiveBudgetExceeded {
                prefab_index: 0,
                id: "stone_block".to_string(),
                count: 2,
                limit: 1,
            }
        );
        assert_validation_report(
            &error,
            "prefab_primitive_budget_exceeded",
            "prefabs[0].primitives",
            &["contains 2 primitives", "limit 1"],
        );
    }

    #[test]
    fn palette_rejects_total_authored_primitives_above_palette_budget() {
        let mut palette = valid_palette(vec![
            valid_prefab("stone_block", vec![valid_primitive()]),
            valid_prefab("glow_orb", vec![valid_primitive()]),
        ]);
        palette.max_primitives = 1;

        let error = palette.validate().unwrap_err();

        assert_eq!(
            error,
            FangyuanPrefabValidationError::TotalPrimitiveBudgetExceeded { count: 2, limit: 1 }
        );
        assert_validation_report(
            &error,
            "total_primitive_budget_exceeded",
            "prefabs",
            &["2 authored primitives", "limit 1"],
        );
    }

    #[test]
    fn palette_rejects_invalid_prefab_primitive_with_blueprint_validator() {
        let mut primitive = valid_primitive();
        primitive.size = [0.0, 1.0, 1.0];
        let palette = valid_palette(vec![valid_prefab("stone_block", vec![primitive])]);

        let error = palette.validate().unwrap_err();

        assert!(matches!(
            error,
            FangyuanPrefabValidationError::InvalidPrefabPrimitive {
                prefab_index: 0,
                ref prefab_id,
                primitive_index: 0,
                source: FangyuanBlueprintValidationError::InvalidPrimitiveSize { .. },
            } if prefab_id == "stone_block"
        ));
        assert_validation_report(
            &error,
            "invalid_prefab_primitive",
            "prefabs[0].primitives[0].size[0]",
            &["blueprint primitive validation", "0.1..=5"],
        );
    }

    #[test]
    fn palette_rejects_forbidden_prefab_fields_by_parse() {
        for field in [
            "rotation",
            "quaternion",
            "euler",
            "angular_velocity",
            "rotate",
            "spin",
            "script",
            "shader",
            "server_rule",
            "external_asset",
        ] {
            let source = valid_palette_ron_with_extra_prefab_field(field);

            assert_parse_error_contains(
                FangyuanPrefabPalette::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    #[test]
    fn palette_rejects_forbidden_primitive_fields_by_parse() {
        for field in [
            "rotation",
            "quaternion",
            "euler",
            "angular_velocity",
            "rotate",
            "spin",
        ] {
            let source = valid_palette_ron_with_extra_primitive_field(field);

            assert_parse_error_contains(
                FangyuanPrefabPalette::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    #[test]
    fn palette_rejects_forbidden_top_level_fields_by_parse() {
        for field in ["script", "shader", "server_rule", "external_asset"] {
            let source = format!(
                r#"
(
    version: "1",
    name: "starter_palette",
    description: "",
    max_primitives: 8,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    prefabs: [],
    {field}: "forbidden",
)
"#
            );

            assert_parse_error_contains(
                FangyuanPrefabPalette::from_ron_str(&source),
                field,
                "Unexpected field",
            );
        }
    }

    #[test]
    fn home_prefab_palette_asset_loads_and_validates() {
        let palette =
            FangyuanPrefabPalette::load_first_package_ron(FANGYUAN_HOME_PREFAB_PALETTE_PATH)
                .unwrap();

        palette.validate().unwrap();

        assert_eq!(palette.name, "home_prefabs");
        assert_eq!(
            palette
                .prefabs
                .iter()
                .map(|prefab| prefab.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "fence_segment",
                "gate_piece",
                "dragon_body_segment",
                "cloud_puff",
                "stone_marker",
            ]
        );
        assert!(palette.prefabs.len() >= 3);
        assert!(palette.prefabs.len() <= 5);
        assert!(
            palette
                .prefabs
                .iter()
                .map(|prefab| prefab.primitives.len())
                .sum::<usize>()
                <= 64
        );
    }

    #[test]
    fn fangyuan_prefab_audit_passes_default_home_prefab_palette() {
        let palette =
            FangyuanPrefabPalette::load_first_package_ron(FANGYUAN_HOME_PREFAB_PALETTE_PATH)
                .unwrap();

        let report = palette.audit_with_default_budget();
        let authored_primitives = palette
            .prefabs
            .iter()
            .map(|prefab| prefab.primitives.len())
            .sum::<usize>();
        let material_count = palette
            .prefabs
            .iter()
            .flat_map(|prefab| &prefab.primitives)
            .filter_map(|primitive| primitive.material_profile_id.as_deref())
            .collect::<HashSet<_>>()
            .len();

        assert_eq!(report.source_kind, FangyuanAuditSourceKind::PrefabPalette);
        assert_eq!(report.status, FangyuanAuditStatus::Passed);
        assert!(report.findings.is_empty());
        assert_eq!(report.summary.prefab_count, palette.prefabs.len());
        assert_eq!(report.summary.prefab_count, 5);
        assert_eq!(report.summary.authored_primitives, authored_primitives);
        assert_eq!(report.summary.authored_primitives, 19);
        assert_eq!(report.summary.generated_primitives, authored_primitives);
        assert_eq!(report.summary.skipped_primitives, 0);
        assert_eq!(report.summary.material_count, material_count);
        assert_eq!(report.summary.material_count, 0);
        assert_eq!(report.summary.reusable_prefab_count, 5);
    }

    #[test]
    fn fangyuan_prefab_audit_reports_illegal_id_and_duplicate_without_stopping() {
        let palette = valid_palette(vec![
            valid_prefab("BadId", vec![valid_primitive()]),
            valid_prefab("stone_block", vec![valid_primitive()]),
            valid_prefab("stone_block", vec![valid_primitive()]),
        ]);

        let report = palette.audit_with_default_budget();

        assert_eq!(report.status, FangyuanAuditStatus::Failed);
        assert_audit_finding(
            &report,
            "invalid_prefab_id",
            FangyuanAuditSeverity::Error,
            Some("BadId"),
            None,
            "prefabs[0].id",
        );
        assert_audit_finding(
            &report,
            "duplicate_prefab_id",
            FangyuanAuditSeverity::Error,
            Some("stone_block"),
            None,
            "prefabs[2].id",
        );
        assert_eq!(report.summary.prefab_count, 3);
        assert_eq!(report.summary.authored_primitives, 3);
        assert_eq!(report.summary.generated_primitives, 3);
    }

    #[test]
    fn fangyuan_prefab_audit_reports_prefab_budget_and_total_budget() {
        let mut over_budget_prefab = valid_prefab(
            "stone_block",
            vec![valid_primitive(), valid_primitive(), valid_primitive()],
        );
        over_budget_prefab.max_primitives = Some(1);
        let mut palette = valid_palette(vec![over_budget_prefab, valid_prefab("glow_orb", vec![])]);
        palette.max_primitives = 2;

        let report = palette.audit_with_default_budget();

        assert_eq!(report.status, FangyuanAuditStatus::Failed);
        assert_audit_finding(
            &report,
            "prefab_primitive_budget_exceeded",
            FangyuanAuditSeverity::Error,
            Some("stone_block"),
            None,
            "prefabs[0].primitives",
        );
        assert_audit_finding(
            &report,
            "total_primitive_budget_exceeded",
            FangyuanAuditSeverity::Error,
            None,
            None,
            "prefabs",
        );
        assert_eq!(report.summary.prefab_count, 2);
        assert_eq!(report.summary.authored_primitives, 3);
        assert_eq!(report.summary.generated_primitives, 3);
    }

    #[test]
    fn fangyuan_prefab_audit_reports_palette_bounds_pivot_tags_and_primitive_paths() {
        let mut invalid_pivot = valid_prefab("stone_block", vec![valid_primitive()]);
        invalid_pivot.bounds = Some(FangyuanBlueprintBounds::new(4.0, 4.0, 4.0));
        invalid_pivot.pivot = Some([0.0, f32::NAN, 0.0]);

        let mut invalid_tag = valid_prefab("glow_orb", vec![valid_primitive()]);
        invalid_tag.bounds = Some(FangyuanBlueprintBounds::new(4.0, 4.0, 4.0));
        invalid_tag.tags = vec!["bad tag".to_string()];

        let mut invalid_primitive = valid_primitive();
        invalid_primitive.size = [0.0, 1.0, 1.0];
        let mut primitive_prefab = valid_prefab("bad_primitive", vec![invalid_primitive]);
        primitive_prefab.bounds = Some(FangyuanBlueprintBounds::new(4.0, 4.0, 4.0));

        let mut invalid_bounds = valid_prefab("bad_bounds", vec![valid_primitive()]);
        invalid_bounds.bounds = Some(FangyuanBlueprintBounds::new(f32::INFINITY, 4.0, 4.0));

        let mut palette = valid_palette(vec![
            invalid_pivot,
            invalid_tag,
            primitive_prefab,
            invalid_bounds,
        ]);
        palette.bounds.width = f32::NAN;

        let report = palette.audit_with_default_budget();

        assert_eq!(report.status, FangyuanAuditStatus::Failed);
        assert_audit_finding(
            &report,
            "invalid_palette_bounds",
            FangyuanAuditSeverity::Error,
            None,
            None,
            "bounds.width",
        );
        assert_audit_finding(
            &report,
            "invalid_prefab_pivot",
            FangyuanAuditSeverity::Error,
            Some("stone_block"),
            None,
            "prefabs[0].pivot[1]",
        );
        assert_audit_finding(
            &report,
            "invalid_prefab_tag",
            FangyuanAuditSeverity::Error,
            Some("glow_orb"),
            None,
            "prefabs[1].tags[0]",
        );
        assert_audit_finding(
            &report,
            "invalid_prefab_primitive",
            FangyuanAuditSeverity::Error,
            Some("bad_primitive"),
            Some(0),
            "prefabs[2].primitives[0].size[0]",
        );
        assert_audit_finding(
            &report,
            "invalid_prefab_bounds",
            FangyuanAuditSeverity::Error,
            Some("bad_bounds"),
            None,
            "prefabs[3].bounds.width",
        );
        assert_eq!(report.summary.authored_primitives, 4);
        assert_eq!(report.summary.generated_primitives, 2);
        assert_eq!(report.summary.skipped_primitives, 2);
    }

    #[test]
    fn fangyuan_prefab_audit_reports_runtime_budget_risks_with_palette_paths() {
        let mut alpha = valid_primitive();
        alpha.alpha = Some(0.5);
        alpha.material_profile_id = Some("glass".to_string());

        let mut emissive = valid_primitive();
        emissive.emissive = Some(2.0);
        emissive.material_profile_id = Some("glow".to_string());

        let profile = FangyuanAuditBudgetProfile {
            recommended_primitive_limit: 1,
            hard_primitive_limit: 10,
            recommended_alpha_count: 0,
            max_alpha_count: 10,
            recommended_emissive_count: 0,
            max_emissive_count: 10,
            recommended_material_profile_count: 1,
            max_material_profile_count: 10,
            ..Default::default()
        };
        let palette = valid_palette(vec![valid_prefab("fx_cluster", vec![alpha, emissive])]);

        let report = palette.audit(&profile);

        assert_eq!(report.status, FangyuanAuditStatus::PassedWithWarnings);
        assert_eq!(report.summary.prefab_count, 1);
        assert_eq!(report.summary.reusable_prefab_count, 1);
        assert_eq!(report.summary.authored_primitives, 2);
        assert_eq!(report.summary.material_count, 2);
        assert_eq!(report.summary.alpha_count, 1);
        assert_eq!(report.summary.emissive_count, 1);
        assert_audit_finding(
            &report,
            "primitive_count_above_recommended",
            FangyuanAuditSeverity::Warning,
            None,
            None,
            "prefabs[].primitives",
        );
        assert_audit_finding(
            &report,
            "alpha_count_above_recommended",
            FangyuanAuditSeverity::Warning,
            None,
            None,
            "prefabs[].primitives[].alpha",
        );
        assert_audit_finding(
            &report,
            "emissive_count_above_recommended",
            FangyuanAuditSeverity::Warning,
            None,
            None,
            "prefabs[].primitives[].emissive",
        );
        assert_audit_finding(
            &report,
            "material_profile_count_above_recommended",
            FangyuanAuditSeverity::Warning,
            None,
            None,
            "prefabs[].primitives[].material_profile_id",
        );
    }

    #[test]
    fn fangyuan_prefab_audit_rejects_forbidden_fields_by_parse() {
        for source in [
            valid_palette_ron_with_extra_prefab_field("rotation"),
            valid_palette_ron_with_extra_primitive_field("spin"),
            format!(
                r#"
(
    version: "1",
    name: "starter_palette",
    description: "",
    max_primitives: 8,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    prefabs: [],
    shader: "forbidden",
)
"#
            ),
        ] {
            assert_parse_error_contains(
                FangyuanPrefabPalette::from_ron_str(&source),
                "Unexpected field",
                "Unexpected field",
            );
        }
    }

    #[test]
    fn prefab_palette_path_policy_reuses_fangyuan_first_package_rules() {
        assert_eq!(
            validate_fangyuan_prefab_palette_asset_path(FANGYUAN_HOME_PREFAB_PALETTE_PATH),
            Ok(())
        );

        assert_eq!(
            validate_fangyuan_prefab_palette_asset_path("scenes/fangyuan_home/layout.ron"),
            Err(FangyuanPrefabPalettePathError::OutsideFangyuanRoot(
                "scenes/fangyuan_home/layout.ron".to_string()
            ))
        );
        assert_eq!(
            validate_fangyuan_prefab_palette_asset_path("../fangyuan/palettes/home_prefabs.ron"),
            Err(FangyuanPrefabPalettePathError::ParentOrEmptySegment(
                "../fangyuan/palettes/home_prefabs.ron".to_string()
            ))
        );
        assert_eq!(
            validate_fangyuan_prefab_palette_asset_path("fangyuan\\palettes\\home_prefabs.ron"),
            Err(FangyuanPrefabPalettePathError::Backslash(
                "fangyuan\\palettes\\home_prefabs.ron".to_string()
            ))
        );
        assert!(matches!(
            validate_fangyuan_prefab_palette_asset_path(
                "C:/project/assets/fangyuan/palettes/home_prefabs.ron"
            ),
            Err(FangyuanPrefabPalettePathError::WindowsDrive(_))
        ));
        assert!(matches!(
            validate_fangyuan_prefab_palette_asset_path("/fangyuan/palettes/home_prefabs.ron"),
            Err(FangyuanPrefabPalettePathError::Absolute(_))
        ));
    }

    fn assert_validation_report(
        error: &FangyuanPrefabValidationError,
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

    fn assert_audit_finding(
        report: &FangyuanAuditReport,
        code: &str,
        severity: FangyuanAuditSeverity,
        prefab_id: Option<&str>,
        prefab_primitive_index: Option<usize>,
        field_path: &str,
    ) {
        let finding = report
            .findings
            .iter()
            .find(|finding| {
                finding.code == code
                    && finding.field_path.as_deref() == Some(field_path)
                    && finding.prefab_id.as_deref() == prefab_id
                    && finding.prefab_primitive_index == prefab_primitive_index
            })
            .unwrap_or_else(|| {
                panic!("expected audit finding `{code}` at `{field_path}` in {report:#?}")
            });

        assert_eq!(finding.severity, severity);
        assert_eq!(finding.source_kind, FangyuanAuditSourceKind::PrefabPalette);
        assert!(!finding.reason.is_empty());
    }

    fn valid_palette(prefabs: Vec<FangyuanPrefabDefinition>) -> FangyuanPrefabPalette {
        FangyuanPrefabPalette {
            version: FANGYUAN_PREFAB_PALETTE_VERSION.to_string(),
            name: "starter_palette".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT,
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
            {field}: "forbidden",
        ),
    ],
)
"#
        )
    }

    fn valid_palette_ron_with_extra_primitive_field(field: &str) -> String {
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
                    {field}: [0.0, 0.0, 0.0],
                ),
            ],
        ),
    ],
)
"#
        )
    }
}
