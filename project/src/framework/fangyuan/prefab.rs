use serde::{Deserialize, Deserializer, Serialize};
use std::{borrow::Cow, collections::HashSet, error::Error, fmt};

use super::{
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT, FANGYUAN_BLUEPRINT_VERSION, FangyuanBlueprintBounds,
    FangyuanBlueprintValidationError, FangyuanPrimitiveBlueprint, validate_blueprint_primitive,
};

pub const FANGYUAN_PREFAB_PALETTE_VERSION: &str = FANGYUAN_BLUEPRINT_VERSION;
pub const FANGYUAN_PREFAB_PALETTE_HARD_PRIMITIVE_LIMIT: usize =
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT;
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

fn validate_prefab_id(id: &str) -> Result<(), FangyuanPrefabIdInvalidReason> {
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

fn validate_prefab_tag(tag: &str) -> Result<(), FangyuanPrefabTagInvalidReason> {
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
    use crate::framework::fangyuan::FangyuanPrimitiveKind;

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
