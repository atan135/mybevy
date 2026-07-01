use bevy::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::{FangyuanPrimitive, FangyuanPrimitiveKind, FangyuanPrimitiveSet};

pub const FANGYUAN_AVATAR_BLUEPRINT_VERSION: &str = "1";
pub const FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT: usize = 1000;
pub const FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH: &str = "fangyuan/avatars/minimal_player.ron";
pub const FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT: usize = 2;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAvatarBlueprint {
    pub version: String,
    pub name: String,
    pub description: String,
    pub max_primitives: usize,
    pub bounds: FangyuanAvatarBlueprintBounds,
    pub primitives: Vec<FangyuanPrimitiveBlueprint>,
}

impl FangyuanAvatarBlueprint {
    pub fn from_ron_str(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str::<Self>(source)
    }

    pub fn load_first_package_ron(
        blueprint_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanAvatarBlueprintLoadError> {
        let blueprint_path = blueprint_path.as_ref().trim();
        validate_avatar_blueprint_asset_path(blueprint_path)
            .map_err(FangyuanAvatarBlueprintLoadError::InvalidPath)?;

        let fs_path = first_package_avatar_blueprint_fs_path(blueprint_path).ok_or_else(|| {
            FangyuanAvatarBlueprintLoadError::BlueprintNotFound(blueprint_path.to_string())
        })?;

        let source = fs::read_to_string(&fs_path).map_err(|source| {
            FangyuanAvatarBlueprintLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            }
        })?;

        Self::from_ron_str(&source).map_err(|source| {
            FangyuanAvatarBlueprintLoadError::ParseFailed {
                path: fs_path,
                source,
            }
        })
    }

    pub fn validate(&self) -> Result<(), FangyuanAvatarBlueprintValidationError> {
        if self.version != FANGYUAN_AVATAR_BLUEPRINT_VERSION {
            return Err(FangyuanAvatarBlueprintValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_AVATAR_BLUEPRINT_VERSION,
            });
        }

        self.bounds.validate()?;

        let primitive_limit = self
            .max_primitives
            .min(FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT);
        if self.primitives.len() > primitive_limit {
            return Err(
                FangyuanAvatarBlueprintValidationError::PrimitiveCountExceeded {
                    count: self.primitives.len(),
                    limit: primitive_limit,
                    max_primitives: self.max_primitives,
                    hard_limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
                },
            );
        }

        for (index, primitive) in self.primitives.iter().enumerate() {
            validate_avatar_primitive(index, primitive, &self.bounds)?;
        }

        Ok(())
    }

    pub fn compile(&self) -> Result<FangyuanPrimitiveSet, FangyuanAvatarBlueprintValidationError> {
        self.validate()?;

        Ok(FangyuanPrimitiveSet::from_primitives(
            self.primitives
                .iter()
                .map(|primitive| {
                    FangyuanPrimitive::new(
                        primitive.kind,
                        Vec3::from_array(primitive.position),
                        Vec3::from_array(primitive.size),
                        Color::srgba(
                            primitive.color[0],
                            primitive.color[1],
                            primitive.color[2],
                            primitive.color[3],
                        ),
                    )
                })
                .collect(),
        ))
    }

    pub fn load_compiled_first_package_ron(
        blueprint_path: impl AsRef<str>,
    ) -> Result<FangyuanPrimitiveSet, FangyuanAvatarBlueprintLoadError> {
        let blueprint_path = blueprint_path.as_ref();
        let blueprint = Self::load_first_package_ron(blueprint_path)?;
        blueprint
            .compile()
            .map_err(FangyuanAvatarBlueprintLoadError::ValidationFailed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAvatarBlueprintBounds {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
}

impl FangyuanAvatarBlueprintBounds {
    pub const fn new(width: f32, depth: f32, height: f32) -> Self {
        Self {
            width,
            depth,
            height,
        }
    }

    pub fn validate(&self) -> Result<(), FangyuanAvatarBlueprintValidationError> {
        validate_bounds_dimension("width", self.width)?;
        validate_bounds_dimension("depth", self.depth)?;
        validate_bounds_dimension("height", self.height)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPrimitiveBlueprint {
    pub kind: FangyuanPrimitiveKind,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub position: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub size: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_4")]
    pub color: [f32; 4],
}

impl FangyuanPrimitiveBlueprint {
    pub const fn new(
        kind: FangyuanPrimitiveKind,
        position: [f32; 3],
        size: [f32; 3],
        color: [f32; 4],
    ) -> Self {
        Self {
            kind,
            position,
            size,
            color,
        }
    }
}

pub fn load_fangyuan_minimal_player_blueprint()
-> Result<FangyuanAvatarBlueprint, FangyuanAvatarBlueprintLoadError> {
    FangyuanAvatarBlueprint::load_first_package_ron(FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH)
}

pub fn load_fangyuan_minimal_player_primitive_set()
-> Result<FangyuanPrimitiveSet, FangyuanAvatarBlueprintLoadError> {
    FangyuanAvatarBlueprint::load_compiled_first_package_ron(FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH)
}

pub fn load_fangyuan_minimal_player_primitive_set_or_log() -> Option<FangyuanPrimitiveSet> {
    match load_fangyuan_minimal_player_primitive_set() {
        Ok(primitives) => Some(primitives),
        Err(error) => {
            error!("{error}");
            None
        }
    }
}

pub fn load_fangyuan_avatar_primitive_set_from_first_package_ron_or_log(
    blueprint_path: impl AsRef<str>,
) -> Option<FangyuanPrimitiveSet> {
    let blueprint_path = blueprint_path.as_ref();
    match FangyuanAvatarBlueprint::load_compiled_first_package_ron(blueprint_path) {
        Ok(primitives) => Some(primitives),
        Err(error) => {
            error!("{error}");
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanAvatarBlueprintValidationError {
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    PrimitiveCountExceeded {
        count: usize,
        limit: usize,
        max_primitives: usize,
        hard_limit: usize,
    },
    InvalidBoundsDimension {
        field: &'static str,
        value: f32,
    },
    InvalidPrimitivePosition {
        index: usize,
        axis: usize,
        value: f32,
        min: f32,
        max: f32,
    },
    PrimitiveBelowGround {
        index: usize,
        bottom_y: f32,
    },
    InvalidPrimitiveSize {
        index: usize,
        axis: usize,
        value: f32,
    },
    InvalidPrimitiveColor {
        index: usize,
        channel: usize,
        value: f32,
    },
}

impl fmt::Display for FangyuanAvatarBlueprintValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { found, expected } => write!(
                formatter,
                "fangyuan avatar blueprint version `{found}` is unsupported; expected `{expected}`"
            ),
            Self::PrimitiveCountExceeded {
                count,
                limit,
                max_primitives,
                hard_limit,
            } => write!(
                formatter,
                "fangyuan avatar blueprint contains {count} primitives, exceeding limit {limit} from min(max_primitives={max_primitives}, hard_limit={hard_limit})"
            ),
            Self::InvalidBoundsDimension { field, value } => write!(
                formatter,
                "fangyuan avatar blueprint bounds.{field} must be finite and greater than 0, got {value}"
            ),
            Self::InvalidPrimitivePosition {
                index,
                axis,
                value,
                min,
                max,
            } => write!(
                formatter,
                "fangyuan avatar blueprint primitive #{index} position[{axis}]={value} must be inside {min}..={max}"
            ),
            Self::PrimitiveBelowGround { index, bottom_y } => write!(
                formatter,
                "fangyuan avatar blueprint primitive #{index} extends below ground, bottom_y={bottom_y}"
            ),
            Self::InvalidPrimitiveSize { index, axis, value } => write!(
                formatter,
                "fangyuan avatar blueprint primitive #{index} size[{axis}]={value} must be finite and greater than 0"
            ),
            Self::InvalidPrimitiveColor {
                index,
                channel,
                value,
            } => write!(
                formatter,
                "fangyuan avatar blueprint primitive #{index} color[{channel}]={value} must be in 0.0..=1.0"
            ),
        }
    }
}

impl Error for FangyuanAvatarBlueprintValidationError {}

#[derive(Debug)]
pub enum FangyuanAvatarBlueprintLoadError {
    InvalidPath(FangyuanAvatarBlueprintPathError),
    BlueprintNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ValidationFailed(FangyuanAvatarBlueprintValidationError),
}

impl fmt::Display for FangyuanAvatarBlueprintLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(error) => write!(formatter, "{error}"),
            Self::BlueprintNotFound(path) => write!(
                formatter,
                "fangyuan avatar blueprint was not found under first package assets: {path}"
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read fangyuan avatar blueprint at {}: {source}",
                path.display()
            ),
            Self::ParseFailed { path, source } => write!(
                formatter,
                "failed to parse fangyuan avatar blueprint RON at {}: {source}",
                path.display()
            ),
            Self::ValidationFailed(error) => {
                write!(
                    formatter,
                    "fangyuan avatar blueprint validation failed: {error}"
                )
            }
        }
    }
}

impl Error for FangyuanAvatarBlueprintLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidPath(error) => Some(error),
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::ValidationFailed(error) => Some(error),
            Self::BlueprintNotFound(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAvatarBlueprintPathError {
    Empty,
    Absolute(String),
    Backslash(String),
    WindowsDrive(String),
    ParentOrEmptySegment(String),
}

impl fmt::Display for FangyuanAvatarBlueprintPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("fangyuan avatar blueprint path must not be empty"),
            Self::Absolute(path) => write!(
                formatter,
                "fangyuan avatar blueprint path must be relative to assets: {path}"
            ),
            Self::Backslash(path) => write!(
                formatter,
                "fangyuan avatar blueprint path must use forward slashes: {path}"
            ),
            Self::WindowsDrive(path) => write!(
                formatter,
                "fangyuan avatar blueprint path must not include a Windows drive prefix: {path}"
            ),
            Self::ParentOrEmptySegment(path) => write!(
                formatter,
                "fangyuan avatar blueprint path must stay inside assets: {path}"
            ),
        }
    }
}

impl Error for FangyuanAvatarBlueprintPathError {}

fn validate_avatar_primitive(
    index: usize,
    primitive: &FangyuanPrimitiveBlueprint,
    bounds: &FangyuanAvatarBlueprintBounds,
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    validate_primitive_kind(index, primitive.kind)?;
    validate_primitive_position(index, primitive.position, bounds)?;
    validate_primitive_size(index, primitive.size)?;
    validate_primitive_above_ground(index, primitive.position, primitive.size)?;
    validate_primitive_color(index, primitive.color)?;
    Ok(())
}

fn validate_primitive_kind(
    _index: usize,
    kind: FangyuanPrimitiveKind,
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    match kind {
        FangyuanPrimitiveKind::Cube | FangyuanPrimitiveKind::Sphere => Ok(()),
    }
}

fn validate_bounds_dimension(
    field: &'static str,
    value: f32,
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(FangyuanAvatarBlueprintValidationError::InvalidBoundsDimension { field, value })
    }
}

fn validate_primitive_position(
    index: usize,
    position: [f32; 3],
    bounds: &FangyuanAvatarBlueprintBounds,
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    let ranges = [
        (-bounds.width * 0.5, bounds.width * 0.5),
        (0.0, bounds.height),
        (-bounds.depth * 0.5, bounds.depth * 0.5),
    ];

    for (axis, value) in position.into_iter().enumerate() {
        let (min, max) = ranges[axis];
        if !value.is_finite() || value < min || value > max {
            return Err(
                FangyuanAvatarBlueprintValidationError::InvalidPrimitivePosition {
                    index,
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

fn validate_primitive_size(
    index: usize,
    size: [f32; 3],
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    for (axis, value) in size.into_iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            return Err(
                FangyuanAvatarBlueprintValidationError::InvalidPrimitiveSize { index, axis, value },
            );
        }
    }

    Ok(())
}

fn validate_primitive_above_ground(
    index: usize,
    position: [f32; 3],
    size: [f32; 3],
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    let bottom_y = position[1] - size[1] * 0.5;
    if bottom_y >= 0.0 {
        Ok(())
    } else {
        Err(FangyuanAvatarBlueprintValidationError::PrimitiveBelowGround { index, bottom_y })
    }
}

fn validate_primitive_color(
    index: usize,
    color: [f32; 4],
) -> Result<(), FangyuanAvatarBlueprintValidationError> {
    for (channel, value) in color.into_iter().enumerate() {
        if !(0.0..=1.0).contains(&value) {
            return Err(
                FangyuanAvatarBlueprintValidationError::InvalidPrimitiveColor {
                    index,
                    channel,
                    value,
                },
            );
        }
    }

    Ok(())
}

fn validate_avatar_blueprint_asset_path(
    path: &str,
) -> Result<(), FangyuanAvatarBlueprintPathError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(FangyuanAvatarBlueprintPathError::Empty);
    }
    if trimmed.contains('\\') {
        return Err(FangyuanAvatarBlueprintPathError::Backslash(
            trimmed.to_string(),
        ));
    }
    if has_windows_drive_prefix(trimmed) {
        return Err(FangyuanAvatarBlueprintPathError::WindowsDrive(
            trimmed.to_string(),
        ));
    }
    if Path::new(trimmed).is_absolute() || trimmed.starts_with('/') {
        return Err(FangyuanAvatarBlueprintPathError::Absolute(
            trimmed.to_string(),
        ));
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "..")
    {
        return Err(FangyuanAvatarBlueprintPathError::ParentOrEmptySegment(
            trimmed.to_string(),
        ));
    }

    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn first_package_avatar_blueprint_fs_path(blueprint_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(blueprint_path)))
        .find(|candidate| candidate.is_file())
}

fn first_package_asset_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets"));
        candidates.push(current_dir.join("project").join("assets"));
    }
    candidates.push(PathBuf::from("assets"));
    candidates.push(PathBuf::from("project").join("assets"));
    candidates
}

fn deserialize_f32_array_3<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_f32_array::<3, D>(deserializer)
}

fn deserialize_f32_array_4<'de, D>(deserializer: D) -> Result<[f32; 4], D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_f32_array::<4, D>(deserializer)
}

fn deserialize_f32_array<'de, const N: usize, D>(deserializer: D) -> Result<[f32; N], D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<f32>::deserialize(deserializer)?;
    values.try_into().map_err(|values: Vec<f32>| {
        de::Error::invalid_length(values.len(), &format!("exactly {N} f32 values").as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_primitive_accepts_only_expected_fields() {
        let blueprint: FangyuanPrimitiveBlueprint = serde_json::from_str(
            r#"{
                "kind": "cube",
                "position": [0.0, 0.5, 0.0],
                "size": [1.0, 1.0, 1.0],
                "color": [0.8, 0.6, 0.4, 1.0]
            }"#,
        )
        .unwrap();

        assert_eq!(blueprint.kind, FangyuanPrimitiveKind::Cube);
        assert_eq!(blueprint.position, [0.0, 0.5, 0.0]);
        assert_eq!(blueprint.size, [1.0, 1.0, 1.0]);
        assert_eq!(blueprint.color, [0.8, 0.6, 0.4, 1.0]);
    }

    #[test]
    fn blueprint_primitive_rejects_rotation_field() {
        let result = serde_json::from_str::<FangyuanPrimitiveBlueprint>(
            r#"{
                "kind": "sphere",
                "position": [0.0, 1.2, 0.0],
                "size": [0.8, 0.8, 0.8],
                "color": [0.9, 0.8, 0.7, 1.0],
                "rotation": [0.0, 0.0, 0.0]
            }"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn minimal_player_blueprint_loads_from_first_package_assets_and_compiles() {
        let blueprint = load_fangyuan_minimal_player_blueprint().unwrap();
        let primitive_set = blueprint.compile().unwrap();

        assert_eq!(blueprint.version, FANGYUAN_AVATAR_BLUEPRINT_VERSION);
        assert_eq!(blueprint.name, "minimal_player");
        assert_eq!(
            blueprint.primitives.len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
        assert_eq!(primitive_set.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
        assert_eq!(
            primitive_set.primitives()[0].kind,
            FangyuanPrimitiveKind::Cube
        );
        assert_eq!(
            primitive_set.primitives()[1].kind,
            FangyuanPrimitiveKind::Sphere
        );
        assert_eq!(
            primitive_set.primitives()[0].local_position,
            Vec3::new(0.0, 0.75, 0.0)
        );
        assert_eq!(
            primitive_set.primitives()[0].scale,
            Vec3::new(0.9, 1.5, 0.6)
        );
        let color = primitive_set.primitives()[0].color.to_srgba();
        assert_eq!(
            (color.red, color.green, color.blue, color.alpha),
            (0.25, 0.45, 0.95, 1.0)
        );
    }

    #[test]
    fn invalid_ron_returns_parse_error_without_panicking() {
        let error = FangyuanAvatarBlueprint::from_ron_str("not valid ron").unwrap_err();

        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn unknown_primitive_kind_is_rejected_by_blueprint_parse() {
        let result = FangyuanAvatarBlueprint::from_ron_str(
            r#"
(
    version: "1",
    name: "invalid_kind",
    description: "",
    max_primitives: 1,
    bounds: (width: 2.0, depth: 2.0, height: 2.0),
    primitives: [
        (
            kind: "cylinder",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        ),
    ],
)
"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn compile_rejects_unsupported_version() {
        let mut blueprint = valid_avatar_blueprint(vec![valid_primitive()]);
        blueprint.version = "2".to_string();

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::UnsupportedVersion {
                found: "2".to_string(),
                expected: FANGYUAN_AVATAR_BLUEPRINT_VERSION,
            }
        );
    }

    #[test]
    fn compile_rejects_primitive_count_above_effective_limit() {
        let mut blueprint = valid_avatar_blueprint(vec![valid_primitive(), valid_primitive()]);
        blueprint.max_primitives = 1;

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::PrimitiveCountExceeded {
                count: 2,
                limit: 1,
                max_primitives: 1,
                hard_limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            }
        );
    }

    #[test]
    fn compile_rejects_primitive_count_above_hard_limit() {
        let mut blueprint = valid_avatar_blueprint(vec![
            valid_primitive();
            FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT
                + 1
        ]);
        blueprint.max_primitives = FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT + 500;

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::PrimitiveCountExceeded {
                count: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT + 1,
                limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
                max_primitives: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT + 500,
                hard_limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            }
        );
    }

    #[test]
    fn compile_rejects_position_outside_bounds() {
        let mut primitive = valid_primitive();
        primitive.position = [2.1, 1.0, 0.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::InvalidPrimitivePosition {
                index: 0,
                axis: 0,
                value: 2.1,
                min: -2.0,
                max: 2.0,
            }
        );
    }

    #[test]
    fn compile_rejects_primitive_body_below_ground() {
        let mut primitive = valid_primitive();
        primitive.position = [0.0, 0.2, 0.0];
        primitive.size = [1.0, 1.0, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::PrimitiveBelowGround {
                index: 0,
                bottom_y: -0.3,
            }
        );
    }

    #[test]
    fn compile_rejects_non_positive_size_axis() {
        let mut primitive = valid_primitive();
        primitive.size = [1.0, 0.0, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveSize {
                index: 0,
                axis: 1,
                value: 0.0,
            }
        );
    }

    #[test]
    fn compile_rejects_color_channel_outside_unit_range() {
        let mut primitive = valid_primitive();
        primitive.color = [0.2, 0.4, 1.2, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        assert_eq!(
            blueprint.compile().unwrap_err(),
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveColor {
                index: 0,
                channel: 2,
                value: 1.2,
            }
        );
    }

    fn valid_avatar_blueprint(
        primitives: Vec<FangyuanPrimitiveBlueprint>,
    ) -> FangyuanAvatarBlueprint {
        FangyuanAvatarBlueprint {
            version: FANGYUAN_AVATAR_BLUEPRINT_VERSION.to_string(),
            name: "test_avatar".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            bounds: FangyuanAvatarBlueprintBounds::new(4.0, 4.0, 4.0),
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
}
