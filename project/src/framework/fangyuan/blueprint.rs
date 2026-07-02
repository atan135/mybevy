use bevy::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    borrow::Cow,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::{
    FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FANGYUAN_PRIMITIVE_MAX_EMISSIVE, FangyuanPrimitive,
    FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole, FangyuanPrimitiveSet,
};

pub const FANGYUAN_AVATAR_BLUEPRINT_VERSION: &str = "1";
pub const FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT: usize = 1000;
pub const FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH: &str = "fangyuan/avatars/minimal_player.ron";
pub const FANGYUAN_HOME_PREVIEW_BLUEPRINT_PATH: &str = "fangyuan/home_preview.ron";
pub const FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT: usize = 2;
pub const FANGYUAN_BLUEPRINT_VERSION: &str = FANGYUAN_AVATAR_BLUEPRINT_VERSION;
pub const FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT: usize =
    FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT;

/// Shared Fangyuan RON v1 blueprint.
///
/// Player, home, and static-object previews should vary by caller semantics,
/// default path, and logical root components. They share this top-level data
/// shape and compile into `FangyuanPrimitiveSet`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBlueprint {
    /// RON schema version. The current first-package format is `"1"`.
    pub version: String,
    /// Human-readable blueprint identifier, not a gameplay entity identity.
    pub name: String,
    /// Authoring note for inspectors and documentation.
    pub description: String,
    /// Asset-authored primitive limit, capped by the framework hard limit.
    pub max_primitives: usize,
    /// Local authoring bounds used to reject primitives outside the object.
    pub bounds: FangyuanBlueprintBounds,
    /// Shared primitive authoring records compiled into runtime primitives.
    pub primitives: Vec<FangyuanPrimitiveBlueprint>,
}

pub type FangyuanAvatarBlueprint = FangyuanBlueprint;

impl FangyuanBlueprint {
    pub fn from_ron_str(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str::<Self>(source)
    }

    pub fn load_first_package_ron(
        blueprint_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanBlueprintLoadError> {
        let blueprint_path = blueprint_path.as_ref().trim();
        validate_fangyuan_blueprint_asset_path(blueprint_path)
            .map_err(FangyuanBlueprintLoadError::InvalidPath)?;

        let fs_path =
            first_package_fangyuan_blueprint_fs_path(blueprint_path).ok_or_else(|| {
                FangyuanBlueprintLoadError::BlueprintNotFound(blueprint_path.to_string())
            })?;

        let source = fs::read_to_string(&fs_path).map_err(|source| {
            FangyuanBlueprintLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            }
        })?;

        Self::from_ron_str(&source).map_err(|source| FangyuanBlueprintLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }

    pub fn validate(&self) -> Result<(), FangyuanBlueprintValidationError> {
        if self.version != FANGYUAN_BLUEPRINT_VERSION {
            return Err(FangyuanBlueprintValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_BLUEPRINT_VERSION,
            });
        }

        self.bounds.validate()?;

        let primitive_limit = self
            .max_primitives
            .min(FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT);
        if self.primitives.len() > primitive_limit {
            return Err(FangyuanBlueprintValidationError::PrimitiveCountExceeded {
                count: self.primitives.len(),
                limit: primitive_limit,
                max_primitives: self.max_primitives,
                hard_limit: FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            });
        }

        for (index, primitive) in self.primitives.iter().enumerate() {
            validate_blueprint_primitive(index, primitive, &self.bounds)?;
        }

        Ok(())
    }

    pub fn compile(&self) -> Result<FangyuanPrimitiveSet, FangyuanBlueprintValidationError> {
        self.validate()?;

        Ok(FangyuanPrimitiveSet::from_primitives(
            self.primitives
                .iter()
                .map(|primitive| {
                    let color = Color::srgba(
                        primitive.color[0],
                        primitive.color[1],
                        primitive.color[2],
                        primitive.color[3],
                    );
                    FangyuanPrimitive::with_runtime_metadata(
                        primitive.kind,
                        Vec3::from_array(primitive.position),
                        Vec3::from_array(primitive.size),
                        color,
                        primitive.role(),
                        primitive.alpha(),
                        primitive.emissive(),
                        primitive.material_profile_id.clone(),
                        primitive.lifecycle(),
                    )
                })
                .collect(),
        ))
    }

    pub fn load_compiled_first_package_ron(
        blueprint_path: impl AsRef<str>,
    ) -> Result<FangyuanPrimitiveSet, FangyuanBlueprintLoadError> {
        let blueprint_path = blueprint_path.as_ref();
        let blueprint = Self::load_first_package_ron(blueprint_path)?;
        blueprint
            .compile()
            .map_err(FangyuanBlueprintLoadError::ValidationFailed)
    }
}

/// Local authoring bounds for a shared Fangyuan blueprint.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBlueprintBounds {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
}

pub type FangyuanAvatarBlueprintBounds = FangyuanBlueprintBounds;

impl FangyuanBlueprintBounds {
    pub const fn new(width: f32, depth: f32, height: f32) -> Self {
        Self {
            width,
            depth,
            height,
        }
    }

    pub fn validate(&self) -> Result<(), FangyuanBlueprintValidationError> {
        validate_bounds_dimension("width", self.width)?;
        validate_bounds_dimension("depth", self.depth)?;
        validate_bounds_dimension("height", self.height)?;
        Ok(())
    }
}

/// Shared Fangyuan primitive authoring record.
///
/// These fields are intentionally shared by player and home/static-object
/// blueprints; callers should not fork a second primitive data model.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPrimitiveBlueprint {
    /// Geometry kind. RON v1 accepts `cube` and `sphere`.
    #[serde(deserialize_with = "deserialize_primitive_kind")]
    pub kind: FangyuanPrimitiveKind,
    /// Optional semantic role. Defaults from `kind` when omitted by legacy v1.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_primitive_role",
        skip_serializing_if = "Option::is_none"
    )]
    pub role: Option<FangyuanPrimitiveRole>,
    /// Local primitive center inside `bounds`.
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub position: [f32; 3],
    /// Local primitive scale. Rotation is deliberately not part of RON v1.
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    pub size: [f32; 3],
    /// SRGBA color. Alpha is also used as the legacy opacity default.
    #[serde(deserialize_with = "deserialize_f32_array_4")]
    pub color: [f32; 4],
    /// Optional opacity override. Defaults to `color[3]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f32>,
    /// Optional emissive intensity reserved by the runtime model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive: Option<f32>,
    /// Optional material profile id reserved by the runtime model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_profile_id: Option<String>,
    /// Optional lifecycle metadata reserved by the runtime model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<FangyuanPrimitiveLifecycle>,
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
            role: None,
            position,
            size,
            color,
            alpha: None,
            emissive: None,
            material_profile_id: None,
            lifecycle: None,
        }
    }

    pub const fn role(&self) -> FangyuanPrimitiveRole {
        match self.role {
            Some(role) => role,
            None => FangyuanPrimitiveRole::default_for_kind(self.kind),
        }
    }

    pub fn alpha(&self) -> f32 {
        self.alpha.unwrap_or(self.color[3])
    }

    pub fn emissive(&self) -> f32 {
        self.emissive.unwrap_or(FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE)
    }

    pub fn lifecycle(&self) -> FangyuanPrimitiveLifecycle {
        self.lifecycle.unwrap_or_default()
    }
}

pub fn load_fangyuan_minimal_player_blueprint()
-> Result<FangyuanAvatarBlueprint, FangyuanAvatarBlueprintLoadError> {
    FangyuanBlueprint::load_first_package_ron(FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH)
}

pub fn load_fangyuan_minimal_player_primitive_set()
-> Result<FangyuanPrimitiveSet, FangyuanAvatarBlueprintLoadError> {
    FangyuanBlueprint::load_compiled_first_package_ron(FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH)
}

pub fn load_fangyuan_blueprint_from_first_package_ron(
    blueprint_path: impl AsRef<str>,
) -> Result<FangyuanBlueprint, FangyuanBlueprintLoadError> {
    FangyuanBlueprint::load_first_package_ron(blueprint_path)
}

pub fn load_fangyuan_primitive_set_from_first_package_ron(
    blueprint_path: impl AsRef<str>,
) -> Result<FangyuanPrimitiveSet, FangyuanBlueprintLoadError> {
    FangyuanBlueprint::load_compiled_first_package_ron(blueprint_path)
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
    match FangyuanBlueprint::load_compiled_first_package_ron(blueprint_path) {
        Ok(primitives) => Some(primitives),
        Err(error) => {
            error!("{error}");
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanBlueprintValidationError {
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
    InvalidPrimitiveAlpha {
        index: usize,
        value: f32,
    },
    InvalidPrimitiveEmissive {
        index: usize,
        value: f32,
        max: f32,
    },
}

pub type FangyuanAvatarBlueprintValidationError = FangyuanBlueprintValidationError;

impl FangyuanBlueprintValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::PrimitiveCountExceeded { .. } => "primitive_count_exceeded",
            Self::InvalidBoundsDimension { .. } => "invalid_bounds_dimension",
            Self::InvalidPrimitivePosition { .. } => "invalid_primitive_position",
            Self::PrimitiveBelowGround { .. } => "primitive_below_ground",
            Self::InvalidPrimitiveSize { .. } => "invalid_primitive_size",
            Self::InvalidPrimitiveColor { .. } => "invalid_primitive_color",
            Self::InvalidPrimitiveAlpha { .. } => "invalid_primitive_alpha",
            Self::InvalidPrimitiveEmissive { .. } => "invalid_primitive_emissive",
        }
    }

    pub fn primitive_index(&self) -> Option<usize> {
        match self {
            Self::InvalidPrimitivePosition { index, .. }
            | Self::PrimitiveBelowGround { index, .. }
            | Self::InvalidPrimitiveSize { index, .. }
            | Self::InvalidPrimitiveColor { index, .. }
            | Self::InvalidPrimitiveAlpha { index, .. }
            | Self::InvalidPrimitiveEmissive { index, .. } => Some(*index),
            Self::UnsupportedVersion { .. }
            | Self::PrimitiveCountExceeded { .. }
            | Self::InvalidBoundsDimension { .. } => None,
        }
    }

    pub fn field_path(&self) -> Cow<'static, str> {
        match self {
            Self::UnsupportedVersion { .. } => Cow::Borrowed("version"),
            Self::PrimitiveCountExceeded { .. } => Cow::Borrowed("primitives"),
            Self::InvalidBoundsDimension { field, .. } => Cow::Owned(format!("bounds.{field}")),
            Self::InvalidPrimitivePosition { index, axis, .. } => {
                Cow::Owned(format!("primitives[{index}].position[{axis}]"))
            }
            Self::PrimitiveBelowGround { index, .. } => {
                Cow::Owned(format!("primitives[{index}].position[1]"))
            }
            Self::InvalidPrimitiveSize { index, axis, .. } => {
                Cow::Owned(format!("primitives[{index}].size[{axis}]"))
            }
            Self::InvalidPrimitiveColor { index, channel, .. } => {
                Cow::Owned(format!("primitives[{index}].color[{channel}]"))
            }
            Self::InvalidPrimitiveAlpha { index, .. } => {
                Cow::Owned(format!("primitives[{index}].alpha"))
            }
            Self::InvalidPrimitiveEmissive { index, .. } => {
                Cow::Owned(format!("primitives[{index}].emissive"))
            }
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                format!("version `{found}` is unsupported; expected `{expected}`")
            }
            Self::PrimitiveCountExceeded {
                count,
                limit,
                max_primitives,
                hard_limit,
            } => format!(
                "contains {count} primitives, exceeding limit {limit} from min(max_primitives={max_primitives}, hard_limit={hard_limit})"
            ),
            Self::InvalidBoundsDimension { value, .. } => {
                format!("value {value} must be finite and greater than 0")
            }
            Self::InvalidPrimitivePosition {
                value, min, max, ..
            } => {
                format!("value {value} must be finite and inside {min}..={max}")
            }
            Self::PrimitiveBelowGround { bottom_y, .. } => {
                format!("bottom_y {bottom_y} must be greater than or equal to 0")
            }
            Self::InvalidPrimitiveSize { value, .. } => {
                format!("value {value} must be finite and greater than 0")
            }
            Self::InvalidPrimitiveColor { value, .. } => {
                format!("value {value} must be finite and in 0.0..=1.0")
            }
            Self::InvalidPrimitiveAlpha { value, .. } => {
                format!("value {value} must be finite and in 0.0..=1.0")
            }
            Self::InvalidPrimitiveEmissive { value, max, .. } => {
                format!("value {value} must be finite and in 0.0..={max}")
            }
        }
    }
}

impl fmt::Display for FangyuanBlueprintValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fangyuan blueprint validation error [{}] at {}: {}",
            self.code(),
            self.field_path(),
            self.reason()
        )
    }
}

impl Error for FangyuanBlueprintValidationError {}

#[derive(Debug)]
pub enum FangyuanBlueprintLoadError {
    InvalidPath(FangyuanBlueprintPathError),
    BlueprintNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ValidationFailed(FangyuanBlueprintValidationError),
}

pub type FangyuanAvatarBlueprintLoadError = FangyuanBlueprintLoadError;

impl fmt::Display for FangyuanBlueprintLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(error) => write!(formatter, "{error}"),
            Self::BlueprintNotFound(path) => write!(
                formatter,
                "fangyuan blueprint was not found under first package assets: {path}"
            ),
            Self::ReadFailed { path, source } => write!(
                formatter,
                "failed to read fangyuan blueprint at {}: {source}",
                path.display()
            ),
            Self::ParseFailed { path, source } => write!(
                formatter,
                "failed to parse fangyuan blueprint RON at {}: {source}",
                path.display()
            ),
            Self::ValidationFailed(error) => {
                write!(formatter, "fangyuan blueprint validation failed: {error}")
            }
        }
    }
}

impl Error for FangyuanBlueprintLoadError {
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
pub enum FangyuanBlueprintPathError {
    Empty,
    Absolute(String),
    Backslash(String),
    WindowsDrive(String),
    ParentOrEmptySegment(String),
    OutsideFangyuanRoot(String),
}

pub type FangyuanAvatarBlueprintPathError = FangyuanBlueprintPathError;

impl fmt::Display for FangyuanBlueprintPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("fangyuan blueprint path must not be empty"),
            Self::Absolute(path) => write!(
                formatter,
                "fangyuan blueprint path must be relative to assets: {path}"
            ),
            Self::Backslash(path) => write!(
                formatter,
                "fangyuan blueprint path must use forward slashes: {path}"
            ),
            Self::WindowsDrive(path) => write!(
                formatter,
                "fangyuan blueprint path must not include a Windows drive prefix: {path}"
            ),
            Self::ParentOrEmptySegment(path) => write!(
                formatter,
                "fangyuan blueprint path must stay inside assets: {path}"
            ),
            Self::OutsideFangyuanRoot(path) => write!(
                formatter,
                "fangyuan blueprint path must stay inside assets/fangyuan: {path}"
            ),
        }
    }
}

impl Error for FangyuanBlueprintPathError {}

fn validate_blueprint_primitive(
    index: usize,
    primitive: &FangyuanPrimitiveBlueprint,
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanBlueprintValidationError> {
    validate_primitive_kind(index, primitive.kind)?;
    validate_primitive_position(index, primitive.position, bounds)?;
    validate_primitive_size(index, primitive.size)?;
    validate_primitive_above_ground(index, primitive.position, primitive.size)?;
    validate_primitive_color(index, primitive.color)?;
    validate_primitive_alpha(index, primitive.alpha())?;
    validate_primitive_emissive(index, primitive.emissive())?;
    validate_primitive_role(index, primitive.role())?;
    Ok(())
}

fn validate_primitive_kind(
    _index: usize,
    kind: FangyuanPrimitiveKind,
) -> Result<(), FangyuanBlueprintValidationError> {
    match kind {
        FangyuanPrimitiveKind::Cube | FangyuanPrimitiveKind::Sphere => Ok(()),
    }
}

fn validate_primitive_role(
    _index: usize,
    role: FangyuanPrimitiveRole,
) -> Result<(), FangyuanBlueprintValidationError> {
    match role {
        FangyuanPrimitiveRole::Structure
        | FangyuanPrimitiveRole::Core
        | FangyuanPrimitiveRole::Boundary
        | FangyuanPrimitiveRole::Warning
        | FangyuanPrimitiveRole::Trail
        | FangyuanPrimitiveRole::Impact
        | FangyuanPrimitiveRole::Decoration
        | FangyuanPrimitiveRole::Socket
        | FangyuanPrimitiveRole::Archive => Ok(()),
    }
}

fn validate_bounds_dimension(
    field: &'static str,
    value: f32,
) -> Result<(), FangyuanBlueprintValidationError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(FangyuanBlueprintValidationError::InvalidBoundsDimension { field, value })
    }
}

fn validate_primitive_position(
    index: usize,
    position: [f32; 3],
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanBlueprintValidationError> {
    let ranges = [
        (-bounds.width * 0.5, bounds.width * 0.5),
        (0.0, bounds.height),
        (-bounds.depth * 0.5, bounds.depth * 0.5),
    ];

    for (axis, value) in position.into_iter().enumerate() {
        let (min, max) = ranges[axis];
        if !value.is_finite() || value < min || value > max {
            return Err(FangyuanBlueprintValidationError::InvalidPrimitivePosition {
                index,
                axis,
                value,
                min,
                max,
            });
        }
    }

    Ok(())
}

fn validate_primitive_size(
    index: usize,
    size: [f32; 3],
) -> Result<(), FangyuanBlueprintValidationError> {
    for (axis, value) in size.into_iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            return Err(FangyuanBlueprintValidationError::InvalidPrimitiveSize {
                index,
                axis,
                value,
            });
        }
    }

    Ok(())
}

fn validate_primitive_above_ground(
    index: usize,
    position: [f32; 3],
    size: [f32; 3],
) -> Result<(), FangyuanBlueprintValidationError> {
    let bottom_y = position[1] - size[1] * 0.5;
    if bottom_y >= 0.0 {
        Ok(())
    } else {
        Err(FangyuanBlueprintValidationError::PrimitiveBelowGround { index, bottom_y })
    }
}

fn validate_primitive_color(
    index: usize,
    color: [f32; 4],
) -> Result<(), FangyuanBlueprintValidationError> {
    for (channel, value) in color.into_iter().enumerate() {
        if !(0.0..=1.0).contains(&value) {
            return Err(FangyuanBlueprintValidationError::InvalidPrimitiveColor {
                index,
                channel,
                value,
            });
        }
    }

    Ok(())
}

fn validate_primitive_alpha(
    index: usize,
    alpha: f32,
) -> Result<(), FangyuanBlueprintValidationError> {
    if alpha.is_finite() && (0.0..=1.0).contains(&alpha) {
        Ok(())
    } else {
        Err(FangyuanBlueprintValidationError::InvalidPrimitiveAlpha {
            index,
            value: alpha,
        })
    }
}

fn validate_primitive_emissive(
    index: usize,
    emissive: f32,
) -> Result<(), FangyuanBlueprintValidationError> {
    if emissive.is_finite() && (0.0..=FANGYUAN_PRIMITIVE_MAX_EMISSIVE).contains(&emissive) {
        Ok(())
    } else {
        Err(FangyuanBlueprintValidationError::InvalidPrimitiveEmissive {
            index,
            value: emissive,
            max: FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
        })
    }
}

pub fn validate_fangyuan_blueprint_asset_path(
    path: &str,
) -> Result<(), FangyuanBlueprintPathError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(FangyuanBlueprintPathError::Empty);
    }
    if trimmed.contains('\\') {
        return Err(FangyuanBlueprintPathError::Backslash(trimmed.to_string()));
    }
    if has_windows_drive_prefix(trimmed) {
        return Err(FangyuanBlueprintPathError::WindowsDrive(
            trimmed.to_string(),
        ));
    }
    if Path::new(trimmed).is_absolute() || trimmed.starts_with('/') {
        return Err(FangyuanBlueprintPathError::Absolute(trimmed.to_string()));
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "..")
    {
        return Err(FangyuanBlueprintPathError::ParentOrEmptySegment(
            trimmed.to_string(),
        ));
    }
    if trimmed != "fangyuan" && !trimmed.starts_with("fangyuan/") {
        return Err(FangyuanBlueprintPathError::OutsideFangyuanRoot(
            trimmed.to_string(),
        ));
    }

    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn first_package_fangyuan_blueprint_fs_path(blueprint_path: &str) -> Option<PathBuf> {
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

fn deserialize_primitive_kind<'de, D>(deserializer: D) -> Result<FangyuanPrimitiveKind, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    FangyuanPrimitiveKind::parse(&value).ok_or_else(|| {
        de::Error::custom(format!(
            "unknown fangyuan primitive kind at field `kind`: `{value}`; expected `cube` or `sphere`"
        ))
    })
}

fn deserialize_optional_primitive_role<'de, D>(
    deserializer: D,
) -> Result<Option<FangyuanPrimitiveRole>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    FangyuanPrimitiveRole::parse(&value).map(Some).ok_or_else(|| {
        de::Error::custom(format!(
            "unknown fangyuan primitive role at field `role`: `{value}`; expected one of `structure`, `core`, `boundary`, `warning`, `trail`, `impact`, `decoration`, `socket`, `archive`"
        ))
    })
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
        assert_eq!(blueprint.role(), FangyuanPrimitiveRole::Structure);
        assert_eq!(blueprint.position, [0.0, 0.5, 0.0]);
        assert_eq!(blueprint.size, [1.0, 1.0, 1.0]);
        assert_eq!(blueprint.color, [0.8, 0.6, 0.4, 1.0]);
        assert_eq!(blueprint.alpha(), 1.0);
        assert_eq!(blueprint.emissive(), FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
        assert_eq!(blueprint.material_profile_id, None);
        assert_eq!(blueprint.lifecycle(), FangyuanPrimitiveLifecycle::empty());
    }

    #[test]
    fn blueprint_primitive_accepts_explicit_role() {
        let blueprint: FangyuanPrimitiveBlueprint = serde_json::from_str(
            r#"{
                "kind": "sphere",
                "role": "decoration",
                "position": [0.0, 0.5, 0.0],
                "size": [1.0, 1.0, 1.0],
                "color": [0.8, 0.6, 0.4, 1.0]
            }"#,
        )
        .unwrap();

        assert_eq!(blueprint.role, Some(FangyuanPrimitiveRole::Decoration));
        assert_eq!(blueprint.role(), FangyuanPrimitiveRole::Decoration);
    }

    #[test]
    fn blueprint_primitive_rejects_reserved_transform_fields() {
        for field in [
            "rotation",
            "quaternion",
            "euler",
            "angular_velocity",
            "rotate",
            "spin",
        ] {
            let mut value = serde_json::json!({
                "kind": "sphere",
                "position": [0.0, 1.2, 0.0],
                "size": [0.8, 0.8, 0.8],
                "color": [0.9, 0.8, 0.7, 1.0]
            });
            value
                .as_object_mut()
                .unwrap()
                .insert(field.to_string(), serde_json::json!([0.0, 0.0, 0.0]));

            assert_parse_error_contains(
                serde_json::from_value::<FangyuanPrimitiveBlueprint>(value),
                field,
                "unknown field",
            );
        }
    }

    #[test]
    fn minimal_player_blueprint_loads_from_first_package_assets_and_compiles() {
        let blueprint = load_fangyuan_minimal_player_blueprint().unwrap();
        let primitive_set = blueprint.compile().unwrap();

        assert_eq!(blueprint.version, FANGYUAN_BLUEPRINT_VERSION);
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
            primitive_set.primitives()[0].role,
            FangyuanPrimitiveRole::Structure
        );
        assert_eq!(
            primitive_set.primitives()[1].role,
            FangyuanPrimitiveRole::Core
        );
        for primitive in primitive_set.primitives() {
            let color = primitive.color.to_srgba();
            assert_eq!(primitive.alpha, color.alpha);
            assert_eq!(primitive.emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
            assert_eq!(primitive.material_profile_id, None);
            assert_eq!(primitive.lifecycle, FangyuanPrimitiveLifecycle::empty());
        }
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
    fn shared_blueprint_entry_loads_minimal_player_and_home_preview_paths() {
        let player =
            load_fangyuan_blueprint_from_first_package_ron(FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH)
                .unwrap();
        let home =
            load_fangyuan_blueprint_from_first_package_ron(FANGYUAN_HOME_PREVIEW_BLUEPRINT_PATH)
                .unwrap();

        assert_eq!(player.version, FANGYUAN_BLUEPRINT_VERSION);
        assert_eq!(player.name, "minimal_player");
        assert_eq!(player.description, "最小方圆玩家外观");
        assert_eq!(
            player.max_primitives,
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );
        assert_eq!(player.bounds, FangyuanBlueprintBounds::new(2.0, 2.0, 3.0));
        assert_eq!(
            player.primitives.len(),
            FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT
        );

        assert_eq!(home.version, FANGYUAN_BLUEPRINT_VERSION);
        assert_eq!(home.name, "home_preview");
        assert_eq!(home.max_primitives, FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT);
        assert_eq!(home.bounds, FangyuanBlueprintBounds::new(40.0, 40.0, 20.0));
        assert!(!home.description.is_empty());
        assert!(!home.primitives.is_empty());
    }

    #[test]
    fn shared_blueprint_entry_compiles_minimal_player_to_primitive_set() {
        let player = load_fangyuan_primitive_set_from_first_package_ron(
            FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH,
        )
        .unwrap();

        assert_eq!(player.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
        assert!(
            player
                .primitives()
                .iter()
                .any(|primitive| primitive.kind == FangyuanPrimitiveKind::Cube)
        );
        assert!(
            player
                .primitives()
                .iter()
                .any(|primitive| primitive.kind == FangyuanPrimitiveKind::Sphere)
        );
    }

    #[test]
    fn avatar_blueprint_name_remains_compatible_alias_for_shared_entry() {
        let blueprint: FangyuanAvatarBlueprint =
            FangyuanBlueprint::load_first_package_ron(FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH)
                .unwrap();
        let primitive_set = FangyuanAvatarBlueprint::load_compiled_first_package_ron(
            FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH,
        )
        .unwrap();

        assert_eq!(blueprint.name, "minimal_player");
        assert_eq!(primitive_set.len(), FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT);
    }

    #[test]
    fn fangyuan_blueprint_path_policy_allows_only_fangyuan_first_package_paths() {
        for path in [
            FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH,
            FANGYUAN_HOME_PREVIEW_BLUEPRINT_PATH,
        ] {
            assert_eq!(validate_fangyuan_blueprint_asset_path(path), Ok(()));
        }

        assert_eq!(
            validate_fangyuan_blueprint_asset_path("scenes/fangyuan_home/layout.ron"),
            Err(FangyuanBlueprintPathError::OutsideFangyuanRoot(
                "scenes/fangyuan_home/layout.ron".to_string()
            ))
        );
        assert_eq!(
            validate_fangyuan_blueprint_asset_path("../fangyuan/home_preview.ron"),
            Err(FangyuanBlueprintPathError::ParentOrEmptySegment(
                "../fangyuan/home_preview.ron".to_string()
            ))
        );
        assert_eq!(
            validate_fangyuan_blueprint_asset_path("fangyuan\\home_preview.ron"),
            Err(FangyuanBlueprintPathError::Backslash(
                "fangyuan\\home_preview.ron".to_string()
            ))
        );
        assert!(matches!(
            validate_fangyuan_blueprint_asset_path("C:/project/assets/fangyuan/home_preview.ron"),
            Err(FangyuanBlueprintPathError::WindowsDrive(_))
        ));
        assert!(matches!(
            validate_fangyuan_blueprint_asset_path("/fangyuan/home_preview.ron"),
            Err(FangyuanBlueprintPathError::Absolute(_))
        ));
    }

    #[test]
    fn shared_blueprint_shape_documents_top_level_and_primitive_semantics() {
        let mut primitive = FangyuanPrimitiveBlueprint::new(
            FangyuanPrimitiveKind::Sphere,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.3, 0.4, 0.5],
        );
        primitive.role = Some(FangyuanPrimitiveRole::Decoration);
        primitive.alpha = Some(0.4);
        primitive.emissive = Some(1.5);
        primitive.material_profile_id = Some("shared_preview".to_string());
        primitive.lifecycle = Some(FangyuanPrimitiveLifecycle::new(Some(8), Some(2), Some(10)));
        let blueprint = FangyuanBlueprint {
            version: FANGYUAN_BLUEPRINT_VERSION.to_string(),
            name: "shared_entry".to_string(),
            description: "shared player/home/static object primitive schema".to_string(),
            max_primitives: FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            bounds: FangyuanBlueprintBounds::new(4.0, 4.0, 4.0),
            primitives: vec![primitive],
        };

        let primitive_set = blueprint.compile().unwrap();
        let primitive = &primitive_set.primitives()[0];

        assert_eq!(blueprint.version, FANGYUAN_BLUEPRINT_VERSION);
        assert_eq!(blueprint.name, "shared_entry");
        assert!(!blueprint.description.is_empty());
        assert_eq!(
            blueprint.max_primitives,
            FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT
        );
        assert_eq!(
            blueprint.bounds,
            FangyuanBlueprintBounds::new(4.0, 4.0, 4.0)
        );
        assert_eq!(blueprint.primitives.len(), 1);

        assert_eq!(primitive.kind, FangyuanPrimitiveKind::Sphere);
        assert_eq!(primitive.role, FangyuanPrimitiveRole::Decoration);
        assert_eq!(primitive.local_position, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(primitive.scale, Vec3::ONE);
        assert_eq!(
            primitive.color.to_srgba(),
            Color::srgba(0.2, 0.3, 0.4, 0.5).to_srgba()
        );
        assert_eq!(primitive.alpha, 0.4);
        assert_eq!(primitive.emissive, 1.5);
        assert_eq!(
            primitive.material_profile_id.as_deref(),
            Some("shared_preview")
        );
        assert_eq!(
            primitive.lifecycle,
            FangyuanPrimitiveLifecycle::new(Some(8), Some(2), Some(10))
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

        assert_parse_error_contains(result, "kind", "cylinder");
    }

    #[test]
    fn unknown_primitive_role_is_rejected_by_blueprint_parse() {
        let result = FangyuanAvatarBlueprint::from_ron_str(
            r#"
(
    version: "1",
    name: "invalid_role",
    description: "",
    max_primitives: 1,
    bounds: (width: 2.0, depth: 2.0, height: 2.0),
    primitives: [
        (
            kind: "cube",
            role: "weapon_socket",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        ),
    ],
)
"#,
        );

        assert_parse_error_contains(result, "role", "weapon_socket");
    }

    #[test]
    fn compile_uses_explicit_primitive_role_without_changing_entity_boundary() {
        let mut primitive = valid_primitive();
        primitive.role = Some(FangyuanPrimitiveRole::Warning);
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let primitive_set = blueprint.compile().unwrap();

        assert_eq!(primitive_set.len(), 1);
        assert_eq!(
            primitive_set.primitives()[0].role,
            FangyuanPrimitiveRole::Warning
        );
    }

    #[test]
    fn compile_defaults_legacy_v1_required_primitive_fields_to_runtime_defaults() {
        let blueprint = FangyuanAvatarBlueprint::from_ron_str(
            r#"
(
    version: "1",
    name: "legacy_v1_required_fields",
    description: "",
    max_primitives: 2,
    bounds: (width: 4.0, depth: 4.0, height: 4.0),
    primitives: [
        (
            kind: "cube",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [1.0, 0.8, 0.6, 0.35],
        ),
        (
            kind: "sphere",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [0.6, 0.8, 1.0, 0.6],
        ),
    ],
)
"#,
        )
        .unwrap();

        assert_eq!(blueprint.primitives[0].role, None);
        assert_eq!(blueprint.primitives[1].role, None);
        assert_eq!(blueprint.primitives[0].alpha, None);
        assert_eq!(blueprint.primitives[1].alpha, None);
        assert_eq!(blueprint.primitives[0].emissive, None);
        assert_eq!(blueprint.primitives[1].emissive, None);
        assert_eq!(blueprint.primitives[0].material_profile_id, None);
        assert_eq!(blueprint.primitives[1].material_profile_id, None);
        assert_eq!(blueprint.primitives[0].lifecycle, None);
        assert_eq!(blueprint.primitives[1].lifecycle, None);

        let primitive_set = blueprint.compile().unwrap();
        let primitives = primitive_set.primitives();

        assert_eq!(primitives[0].role, FangyuanPrimitiveRole::Structure);
        assert_eq!(primitives[0].alpha, 0.35);
        assert_eq!(primitives[0].emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
        assert_eq!(primitives[0].material_profile_id, None);
        assert_eq!(primitives[0].lifecycle, FangyuanPrimitiveLifecycle::empty());
        assert_eq!(primitives[1].role, FangyuanPrimitiveRole::Core);
        assert_eq!(primitives[1].alpha, 0.6);
        assert_eq!(primitives[1].emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
        assert_eq!(primitives[1].material_profile_id, None);
        assert_eq!(primitives[1].lifecycle, FangyuanPrimitiveLifecycle::empty());
    }

    #[test]
    fn compile_defaults_reserved_material_fields_and_empty_lifecycle() {
        let mut primitive = valid_primitive();
        primitive.color = [0.2, 0.4, 0.6, 0.35];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let primitive_set = blueprint.compile().unwrap();
        let primitive = &primitive_set.primitives()[0];

        assert_eq!(primitive.alpha, 0.35);
        assert_eq!(primitive.emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
        assert_eq!(primitive.material_profile_id, None);
        assert_eq!(primitive.lifecycle, FangyuanPrimitiveLifecycle::empty());
        assert!(primitive.lifecycle.is_empty());
    }

    #[test]
    fn compile_stores_explicit_reserved_material_fields_and_lifecycle() {
        let mut primitive = valid_primitive();
        primitive.alpha = Some(0.25);
        primitive.emissive = Some(3.5);
        primitive.material_profile_id = Some("avatar_glow".to_string());
        primitive.lifecycle = Some(FangyuanPrimitiveLifecycle::new(Some(30), Some(4), Some(34)));
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let primitive_set = blueprint.compile().unwrap();
        let primitive = &primitive_set.primitives()[0];

        assert_eq!(primitive.alpha, 0.25);
        assert_eq!(primitive.emissive, 3.5);
        assert_eq!(
            primitive.material_profile_id.as_deref(),
            Some("avatar_glow")
        );
        assert_eq!(
            primitive.lifecycle,
            FangyuanPrimitiveLifecycle::new(Some(30), Some(4), Some(34))
        );
    }

    #[test]
    fn compile_rejects_unsupported_version() {
        let mut blueprint = valid_avatar_blueprint(vec![valid_primitive()]);
        blueprint.version = "2".to_string();

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::UnsupportedVersion {
                found: "2".to_string(),
                expected: FANGYUAN_AVATAR_BLUEPRINT_VERSION,
            }
        );
        assert_validation_report(
            &error,
            "unsupported_version",
            None,
            "version",
            &["unsupported", FANGYUAN_AVATAR_BLUEPRINT_VERSION],
        );
    }

    #[test]
    fn compile_rejects_invalid_bounds_dimension() {
        let mut blueprint = valid_avatar_blueprint(vec![valid_primitive()]);
        blueprint.bounds.width = f32::INFINITY;

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidBoundsDimension {
                field: "width",
                value: f32::INFINITY,
            }
        );
        assert_validation_report(
            &error,
            "invalid_bounds_dimension",
            None,
            "bounds.width",
            &["finite", "greater than 0"],
        );
    }

    #[test]
    fn compile_rejects_primitive_count_above_effective_limit() {
        let mut blueprint = valid_avatar_blueprint(vec![valid_primitive(), valid_primitive()]);
        blueprint.max_primitives = 1;

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::PrimitiveCountExceeded {
                count: 2,
                limit: 1,
                max_primitives: 1,
                hard_limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            }
        );
        assert_validation_report(
            &error,
            "primitive_count_exceeded",
            None,
            "primitives",
            &["contains 2 primitives", "limit 1"],
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

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::PrimitiveCountExceeded {
                count: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT + 1,
                limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
                max_primitives: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT + 500,
                hard_limit: FANGYUAN_AVATAR_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            }
        );
        assert_validation_report(
            &error,
            "primitive_count_exceeded",
            None,
            "primitives",
            &[
                "exceeding limit 1000",
                "min(max_primitives=1500, hard_limit=1000)",
            ],
        );
    }

    #[test]
    fn compile_rejects_position_outside_bounds() {
        let mut primitive = valid_primitive();
        primitive.position = [2.1, 1.0, 0.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitivePosition {
                index: 0,
                axis: 0,
                value: 2.1,
                min: -2.0,
                max: 2.0,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_position",
            Some(0),
            "primitives[0].position[0]",
            &["inside -2..=2"],
        );
    }

    #[test]
    fn compile_rejects_non_finite_position_axis() {
        let mut primitive = valid_primitive();
        primitive.position = [0.0, f32::INFINITY, 0.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitivePosition {
                index: 0,
                axis: 1,
                value: f32::INFINITY,
                min: 0.0,
                max: 4.0,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_position",
            Some(0),
            "primitives[0].position[1]",
            &["finite", "inside 0..=4"],
        );
    }

    #[test]
    fn compile_rejects_primitive_body_below_ground() {
        let mut primitive = valid_primitive();
        primitive.position = [0.0, 0.2, 0.0];
        primitive.size = [1.0, 1.0, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::PrimitiveBelowGround {
                index: 0,
                bottom_y: -0.3,
            }
        );
        assert_validation_report(
            &error,
            "primitive_below_ground",
            Some(0),
            "primitives[0].position[1]",
            &["bottom_y -0.3", "greater than or equal to 0"],
        );
    }

    #[test]
    fn compile_rejects_non_positive_size_axis() {
        let mut primitive = valid_primitive();
        primitive.size = [1.0, 0.0, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveSize {
                index: 0,
                axis: 1,
                value: 0.0,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_size",
            Some(0),
            "primitives[0].size[1]",
            &["finite", "greater than 0"],
        );
    }

    #[test]
    fn compile_rejects_non_finite_size_axis() {
        let mut primitive = valid_primitive();
        primitive.size = [1.0, f32::INFINITY, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveSize {
                index: 0,
                axis: 1,
                value: f32::INFINITY,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_size",
            Some(0),
            "primitives[0].size[1]",
            &["finite", "greater than 0"],
        );
    }

    #[test]
    fn compile_rejects_color_channel_outside_unit_range() {
        let mut primitive = valid_primitive();
        primitive.color = [0.2, 0.4, 1.2, 1.0];
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveColor {
                index: 0,
                channel: 2,
                value: 1.2,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_color",
            Some(0),
            "primitives[0].color[2]",
            &["0.0..=1.0"],
        );
    }

    #[test]
    fn compile_rejects_explicit_alpha_outside_unit_range() {
        let mut primitive = valid_primitive();
        primitive.alpha = Some(1.2);
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveAlpha {
                index: 0,
                value: 1.2,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_alpha",
            Some(0),
            "primitives[0].alpha",
            &["0.0..=1.0"],
        );
    }

    #[test]
    fn compile_rejects_emissive_outside_allowed_range() {
        let mut primitive = valid_primitive();
        primitive.emissive = Some(FANGYUAN_PRIMITIVE_MAX_EMISSIVE + 0.5);
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveEmissive {
                index: 0,
                value: FANGYUAN_PRIMITIVE_MAX_EMISSIVE + 0.5,
                max: FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_emissive",
            Some(0),
            "primitives[0].emissive",
            &["0.0..=16"],
        );
    }

    #[test]
    fn compile_rejects_negative_emissive() {
        let mut primitive = valid_primitive();
        primitive.emissive = Some(-0.1);
        let blueprint = valid_avatar_blueprint(vec![primitive]);

        let error = blueprint.compile().unwrap_err();
        assert_eq!(
            error,
            FangyuanAvatarBlueprintValidationError::InvalidPrimitiveEmissive {
                index: 0,
                value: -0.1,
                max: FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
            }
        );
        assert_validation_report(
            &error,
            "invalid_primitive_emissive",
            Some(0),
            "primitives[0].emissive",
            &["0.0..=16"],
        );
    }

    fn assert_validation_report(
        error: &FangyuanAvatarBlueprintValidationError,
        code: &'static str,
        primitive_index: Option<usize>,
        field_path: &str,
        reason_parts: &[&str],
    ) {
        assert_eq!(error.code(), code);
        assert_eq!(error.primitive_index(), primitive_index);
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
