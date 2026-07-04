use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::blueprint::{compile_blueprint_primitive_to_runtime, validate_blueprint_primitive};
use super::{
    FANGYUAN_PRIMITIVE_MAX_EMISSIVE, FangyuanAuditBudgetProfile, FangyuanAuditFinding,
    FangyuanAuditReport, FangyuanAuditSeverity, FangyuanAuditSourceKind, FangyuanBlueprintBounds,
    FangyuanBlueprintValidationError, FangyuanPrimitiveBlueprint, FangyuanPrimitiveBudgetStats,
    FangyuanPrimitiveSet, audit_fangyuan_primitive_budget,
};

pub const FANGYUAN_EQUIPMENT_BLUEPRINT_VERSION: &str = "1";
pub const FANGYUAN_EQUIPMENT_BLUEPRINT_HARD_PRIMITIVE_LIMIT: usize = 256;
pub const FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_PRIMITIVE_LIMIT: usize = 64;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_PRIMITIVE_COUNT: usize = 128;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_BOUNDS_WIDTH: f32 = 8.0;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_BOUNDS_DEPTH: f32 = 8.0;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_BOUNDS_HEIGHT: f32 = 8.0;
pub const FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_TRANSPARENT_COUNT: usize = 8;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_TRANSPARENT_COUNT: usize = 16;
pub const FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_EMISSIVE_COUNT: usize = 6;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_EMISSIVE_COUNT: usize = 12;
pub const FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_MATERIAL_PROFILE_COUNT: usize = 4;
pub const FANGYUAN_EQUIPMENT_DEFAULT_MAX_MATERIAL_PROFILE_COUNT: usize = 8;

const REQUIRED_SOCKET_SEMANTICS: [FangyuanEquipmentSocketSemantic; 5] = [
    FangyuanEquipmentSocketSemantic::Grip,
    FangyuanEquipmentSocketSemantic::Tip,
    FangyuanEquipmentSocketSemantic::Core,
    FangyuanEquipmentSocketSemantic::Guard,
    FangyuanEquipmentSocketSemantic::Aura,
];

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEquipmentBlueprint {
    pub version: String,
    pub id: String,
    pub max_primitives: usize,
    pub bounds: FangyuanBlueprintBounds,
    #[serde(default)]
    pub primitives: Vec<FangyuanPrimitiveBlueprint>,
    #[serde(default)]
    pub sockets: Vec<FangyuanEquipmentSocket>,
}

impl FangyuanEquipmentBlueprint {
    pub fn validate(&self) -> Result<(), FangyuanEquipmentValidationError> {
        self.validate_top_level()?;

        for (index, primitive) in self.primitives.iter().enumerate() {
            validate_blueprint_primitive(index, primitive, &self.bounds).map_err(|source| {
                FangyuanEquipmentValidationError::InvalidPrimitive { index, source }
            })?;
        }

        validate_equipment_sockets(&self.sockets, &self.bounds)?;
        Ok(())
    }

    fn validate_top_level(&self) -> Result<(), FangyuanEquipmentValidationError> {
        if self.version != FANGYUAN_EQUIPMENT_BLUEPRINT_VERSION {
            return Err(FangyuanEquipmentValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_EQUIPMENT_BLUEPRINT_VERSION,
            });
        }
        if self.id.trim().is_empty() {
            return Err(FangyuanEquipmentValidationError::EmptyEquipmentId);
        }

        self.bounds
            .validate()
            .map_err(FangyuanEquipmentValidationError::InvalidBounds)?;

        let primitive_limit = self
            .max_primitives
            .min(FANGYUAN_EQUIPMENT_BLUEPRINT_HARD_PRIMITIVE_LIMIT);
        if self.primitives.len() > primitive_limit {
            return Err(FangyuanEquipmentValidationError::PrimitiveCountExceeded {
                count: self.primitives.len(),
                limit: primitive_limit,
                max_primitives: self.max_primitives,
                hard_limit: FANGYUAN_EQUIPMENT_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            });
        }

        Ok(())
    }

    pub fn compile(&self) -> Result<FangyuanPrimitiveSet, FangyuanEquipmentValidationError> {
        self.validate()?;
        Ok(FangyuanPrimitiveSet::from_primitives(
            self.primitives
                .iter()
                .map(compile_blueprint_primitive_to_runtime)
                .collect(),
        ))
    }

    pub fn compile_sockets(
        &self,
    ) -> Result<FangyuanEquipmentSocketSet, FangyuanEquipmentValidationError> {
        self.validate()?;
        Ok(FangyuanEquipmentSocketSet::from_sockets(
            self.sockets.iter().cloned(),
        ))
    }

    pub fn compile_runtime(
        &self,
    ) -> Result<FangyuanEquipmentRuntime, FangyuanEquipmentValidationError> {
        self.validate()?;
        Ok(FangyuanEquipmentRuntime {
            primitive_set: FangyuanPrimitiveSet::from_primitives(
                self.primitives
                    .iter()
                    .map(compile_blueprint_primitive_to_runtime)
                    .collect(),
            ),
            sockets: FangyuanEquipmentSocketSet::from_sockets(self.sockets.iter().cloned()),
        })
    }

    pub fn audit(&self, profile: &FangyuanAuditBudgetProfile) -> FangyuanAuditReport {
        let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::Blueprint, None);

        if let Err(error) = self.validate_top_level() {
            report.add_finding(equipment_validation_error_to_audit_finding(
                &error,
                FangyuanAuditSeverity::Error,
            ));
            report.summary.authored_primitives = self.primitives.len();
            report.sort_findings();
            return report;
        }

        let mut primitives = Vec::with_capacity(self.primitives.len());
        let mut skipped_primitives = 0usize;
        for (index, primitive) in self.primitives.iter().enumerate() {
            match validate_blueprint_primitive(index, primitive, &self.bounds) {
                Ok(()) => primitives.push(compile_blueprint_primitive_to_runtime(primitive)),
                Err(source) => {
                    skipped_primitives += 1;
                    let error =
                        FangyuanEquipmentValidationError::InvalidPrimitive { index, source };
                    report.add_finding(equipment_validation_error_to_audit_finding(
                        &error,
                        FangyuanAuditSeverity::Warning,
                    ));
                }
            }
        }

        audit_equipment_socket_records(&mut report, &self.sockets, &self.bounds);

        let primitive_set = FangyuanPrimitiveSet::from_primitives(primitives);
        let mut stats = FangyuanPrimitiveBudgetStats::from_primitive_set(&primitive_set);
        stats.authored_primitives = self.primitives.len();
        stats.generated_primitives = primitive_set.len();
        stats.skipped_primitives = skipped_primitives;

        let budget_report = audit_fangyuan_primitive_budget(&stats, profile);
        for mut finding in budget_report.findings {
            finding.source_kind = FangyuanAuditSourceKind::Blueprint;
            report.add_finding(finding);
        }
        for suggestion in budget_report.suggestions {
            report.add_suggestion(suggestion);
        }

        report.refresh_summary_and_status();
        report.apply_primitive_budget_stats(&stats);
        report.sort_findings();
        report
    }

    pub fn audit_with_default_budget(&self) -> FangyuanAuditReport {
        self.audit(&fangyuan_equipment_audit_budget_profile())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanEquipmentRuntime {
    pub primitive_set: FangyuanPrimitiveSet,
    pub sockets: FangyuanEquipmentSocketSet,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanEquipmentSocketSet {
    sockets: BTreeMap<FangyuanEquipmentSocketSemantic, FangyuanEquipmentRuntimeSocket>,
}

impl FangyuanEquipmentSocketSet {
    pub fn new() -> Self {
        Self {
            sockets: BTreeMap::new(),
        }
    }

    pub fn from_sockets(
        sockets: impl IntoIterator<Item = FangyuanEquipmentSocket>,
    ) -> FangyuanEquipmentSocketSet {
        let mut set = Self::new();
        for socket in sockets {
            set.sockets.insert(
                socket.semantic,
                FangyuanEquipmentRuntimeSocket::from_socket(socket),
            );
        }
        set
    }

    pub fn len(&self) -> usize {
        self.sockets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sockets.is_empty()
    }

    pub fn get(
        &self,
        semantic: FangyuanEquipmentSocketSemantic,
    ) -> Option<&FangyuanEquipmentRuntimeSocket> {
        self.sockets.get(&semantic)
    }

    pub fn resolve(
        &self,
        semantic: FangyuanEquipmentSocketSemantic,
        reference_kind: FangyuanEquipmentSocketReferenceKind,
    ) -> FangyuanEquipmentSocketResolution {
        self.resolve_with_fallback(semantic, reference_kind, None)
    }

    pub fn resolve_with_fallback(
        &self,
        semantic: FangyuanEquipmentSocketSemantic,
        reference_kind: FangyuanEquipmentSocketReferenceKind,
        explicit_fallback: Option<&FangyuanEquipmentSocketFallback>,
    ) -> FangyuanEquipmentSocketResolution {
        match self.sockets.get(&semantic) {
            Some(socket) if socket.allows_reference(reference_kind) => {
                FangyuanEquipmentSocketResolution {
                    requested_semantic: semantic,
                    resolved_semantic: Some(socket.semantic),
                    reference_kind,
                    position: socket.position,
                    fallback: None,
                }
            }
            Some(socket) => {
                let fallback = explicit_fallback.cloned().unwrap_or(socket.fallback);
                self.resolve_fallback(
                    semantic,
                    reference_kind,
                    FangyuanEquipmentSocketFallbackReason::ReferenceNotAllowed,
                    fallback,
                )
            }
            None => {
                let fallback = explicit_fallback
                    .cloned()
                    .unwrap_or_else(|| semantic.default_fallback());
                self.resolve_fallback(
                    semantic,
                    reference_kind,
                    FangyuanEquipmentSocketFallbackReason::MissingSocket,
                    fallback,
                )
            }
        }
    }

    fn resolve_fallback(
        &self,
        requested_semantic: FangyuanEquipmentSocketSemantic,
        reference_kind: FangyuanEquipmentSocketReferenceKind,
        reason: FangyuanEquipmentSocketFallbackReason,
        fallback: FangyuanEquipmentSocketFallback,
    ) -> FangyuanEquipmentSocketResolution {
        let (position, resolved_semantic, applied_fallback) =
            match fallback.clone().resolve_position(self, reference_kind) {
                Some((position, resolved_semantic)) => (position, resolved_semantic, fallback),
                None => (Vec3::ZERO, None, FangyuanEquipmentSocketFallback::Origin),
            };
        FangyuanEquipmentSocketResolution {
            requested_semantic,
            resolved_semantic,
            reference_kind,
            position,
            fallback: Some(FangyuanEquipmentSocketFallbackDiagnostic {
                reason,
                applied_fallback,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanEquipmentRuntimeSocket {
    pub semantic: FangyuanEquipmentSocketSemantic,
    pub position: Vec3,
    pub allowed_references: BTreeSet<FangyuanEquipmentSocketReferenceKind>,
    pub fallback: FangyuanEquipmentSocketFallback,
}

impl FangyuanEquipmentRuntimeSocket {
    fn from_socket(socket: FangyuanEquipmentSocket) -> Self {
        let allowed_references = socket.effective_allowed_references();
        let fallback = socket.effective_fallback();
        Self {
            semantic: socket.semantic,
            position: Vec3::from_array(socket.position),
            allowed_references,
            fallback,
        }
    }

    pub fn allows_reference(&self, reference_kind: FangyuanEquipmentSocketReferenceKind) -> bool {
        self.allowed_references.contains(&reference_kind)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanEquipmentSocketResolution {
    pub requested_semantic: FangyuanEquipmentSocketSemantic,
    pub resolved_semantic: Option<FangyuanEquipmentSocketSemantic>,
    pub reference_kind: FangyuanEquipmentSocketReferenceKind,
    pub position: Vec3,
    pub fallback: Option<FangyuanEquipmentSocketFallbackDiagnostic>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanEquipmentSocketFallbackDiagnostic {
    pub reason: FangyuanEquipmentSocketFallbackReason,
    pub applied_fallback: FangyuanEquipmentSocketFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanEquipmentSocketFallbackReason {
    MissingSocket,
    ReferenceNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEquipmentSocket {
    pub semantic: FangyuanEquipmentSocketSemantic,
    pub position: [f32; 3],
    #[serde(default)]
    pub allowed_references: BTreeSet<FangyuanEquipmentSocketReferenceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<FangyuanEquipmentSocketFallback>,
}

impl FangyuanEquipmentSocket {
    pub fn new(semantic: FangyuanEquipmentSocketSemantic, position: [f32; 3]) -> Self {
        Self {
            semantic,
            position,
            allowed_references: semantic.default_allowed_references(),
            fallback: Some(semantic.default_fallback()),
        }
    }

    pub fn with_allowed_references(
        mut self,
        allowed_references: impl IntoIterator<Item = FangyuanEquipmentSocketReferenceKind>,
    ) -> Self {
        self.allowed_references = allowed_references.into_iter().collect();
        self
    }

    pub fn with_fallback(mut self, fallback: FangyuanEquipmentSocketFallback) -> Self {
        self.fallback = Some(fallback);
        self
    }

    pub fn effective_allowed_references(&self) -> BTreeSet<FangyuanEquipmentSocketReferenceKind> {
        if self.allowed_references.is_empty() {
            self.semantic.default_allowed_references()
        } else {
            self.allowed_references.clone()
        }
    }

    pub fn effective_fallback(&self) -> FangyuanEquipmentSocketFallback {
        self.fallback
            .unwrap_or_else(|| self.semantic.default_fallback())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanEquipmentSocketSemantic {
    Grip,
    Tip,
    Core,
    Guard,
    Aura,
}

impl FangyuanEquipmentSocketSemantic {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Grip => "grip",
            Self::Tip => "tip",
            Self::Core => "core",
            Self::Guard => "guard",
            Self::Aura => "aura",
        }
    }

    pub fn default_allowed_references(self) -> BTreeSet<FangyuanEquipmentSocketReferenceKind> {
        use FangyuanEquipmentSocketReferenceKind as ReferenceKind;

        match self {
            Self::Grip => [
                ReferenceKind::Emit,
                ReferenceKind::Trajectory,
                ReferenceKind::Decor,
            ]
            .into_iter()
            .collect(),
            Self::Tip => [
                ReferenceKind::Emit,
                ReferenceKind::Trajectory,
                ReferenceKind::Decor,
            ]
            .into_iter()
            .collect(),
            Self::Core => [
                ReferenceKind::Emit,
                ReferenceKind::Trajectory,
                ReferenceKind::Decor,
            ]
            .into_iter()
            .collect(),
            Self::Guard => [ReferenceKind::Decor].into_iter().collect(),
            Self::Aura => [ReferenceKind::Decor].into_iter().collect(),
        }
    }

    pub const fn default_fallback(self) -> FangyuanEquipmentSocketFallback {
        match self {
            Self::Core => FangyuanEquipmentSocketFallback::Origin,
            Self::Grip => FangyuanEquipmentSocketFallback::Socket {
                semantic: Self::Core,
            },
            Self::Tip => FangyuanEquipmentSocketFallback::Socket {
                semantic: Self::Grip,
            },
            Self::Guard => FangyuanEquipmentSocketFallback::Socket {
                semantic: Self::Core,
            },
            Self::Aura => FangyuanEquipmentSocketFallback::Socket {
                semantic: Self::Core,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanEquipmentSocketReferenceKind {
    Emit,
    Trajectory,
    Decor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanEquipmentSocketFallback {
    Socket {
        semantic: FangyuanEquipmentSocketSemantic,
    },
    Position {
        position: [f32; 3],
    },
    #[default]
    Origin,
}

impl FangyuanEquipmentSocketFallback {
    fn resolve_position(
        self,
        sockets: &FangyuanEquipmentSocketSet,
        reference_kind: FangyuanEquipmentSocketReferenceKind,
    ) -> Option<(Vec3, Option<FangyuanEquipmentSocketSemantic>)> {
        match self {
            Self::Socket { semantic } => {
                let socket = sockets.get(semantic)?;
                if socket.allows_reference(reference_kind) {
                    Some((socket.position, Some(semantic)))
                } else {
                    None
                }
            }
            Self::Position { position } => Some((Vec3::from_array(position), None)),
            Self::Origin => Some((Vec3::ZERO, None)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanEquipmentValidationError {
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    EmptyEquipmentId,
    PrimitiveCountExceeded {
        count: usize,
        limit: usize,
        max_primitives: usize,
        hard_limit: usize,
    },
    InvalidBounds(FangyuanBlueprintValidationError),
    InvalidPrimitive {
        index: usize,
        source: FangyuanBlueprintValidationError,
    },
    DuplicateSocket {
        semantic: FangyuanEquipmentSocketSemantic,
        first_index: usize,
        duplicate_index: usize,
    },
    InvalidSocketPosition {
        index: usize,
        semantic: FangyuanEquipmentSocketSemantic,
        axis: usize,
        value: f32,
        min: f32,
        max: f32,
    },
    InvalidSocketFallback {
        index: usize,
        semantic: FangyuanEquipmentSocketSemantic,
        reason: FangyuanEquipmentSocketFallbackInvalidReason,
    },
    InvalidSocketReferenceRule {
        index: usize,
        semantic: FangyuanEquipmentSocketSemantic,
    },
    MissingSocket {
        semantic: FangyuanEquipmentSocketSemantic,
    },
}

impl FangyuanEquipmentValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "equipment_unsupported_version",
            Self::EmptyEquipmentId => "equipment_empty_id",
            Self::PrimitiveCountExceeded { .. } => "equipment_primitive_count_exceeded",
            Self::InvalidBounds(_) => "equipment_invalid_bounds",
            Self::InvalidPrimitive { source, .. } => source.code(),
            Self::DuplicateSocket { .. } => "equipment_duplicate_socket",
            Self::InvalidSocketPosition { .. } => "equipment_invalid_socket_position",
            Self::InvalidSocketFallback { .. } => "equipment_invalid_socket_fallback",
            Self::InvalidSocketReferenceRule { .. } => "equipment_invalid_socket_reference_rule",
            Self::MissingSocket { .. } => "equipment_missing_socket",
        }
    }

    pub fn field_path(&self) -> String {
        match self {
            Self::UnsupportedVersion { .. } => "version".to_string(),
            Self::EmptyEquipmentId => "id".to_string(),
            Self::PrimitiveCountExceeded { .. } => "primitives".to_string(),
            Self::InvalidBounds(error) => error.field_path().into_owned(),
            Self::InvalidPrimitive { source, .. } => source.field_path().into_owned(),
            Self::DuplicateSocket {
                duplicate_index, ..
            } => format!("sockets[{duplicate_index}].semantic"),
            Self::InvalidSocketPosition { index, axis, .. } => {
                format!("sockets[{index}].position[{axis}]")
            }
            Self::InvalidSocketFallback { index, .. } => {
                format!("sockets[{index}].fallback")
            }
            Self::InvalidSocketReferenceRule { index, .. } => {
                format!("sockets[{index}].allowed_references")
            }
            Self::MissingSocket { semantic } => {
                format!("sockets.{}", semantic.as_str())
            }
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                format!("version `{found}` is unsupported; expected `{expected}`")
            }
            Self::EmptyEquipmentId => "equipment id must not be empty".to_string(),
            Self::PrimitiveCountExceeded {
                count,
                limit,
                max_primitives,
                hard_limit,
            } => format!(
                "contains {count} primitives, exceeding limit {limit} from min(max_primitives={max_primitives}, hard_limit={hard_limit})"
            ),
            Self::InvalidBounds(error) => error.reason(),
            Self::InvalidPrimitive { source, .. } => source.reason(),
            Self::DuplicateSocket {
                semantic,
                first_index,
                duplicate_index,
            } => format!(
                "socket `{}` is declared more than once at indexes {first_index} and {duplicate_index}",
                semantic.as_str()
            ),
            Self::InvalidSocketPosition {
                semantic,
                value,
                min,
                max,
                ..
            } => format!(
                "socket `{}` position value {value} must be finite and inside {min}..={max}",
                semantic.as_str()
            ),
            Self::InvalidSocketFallback {
                semantic, reason, ..
            } => match reason {
                FangyuanEquipmentSocketFallbackInvalidReason::SelfReference => format!(
                    "socket `{}` fallback must not point to itself",
                    semantic.as_str()
                ),
                FangyuanEquipmentSocketFallbackInvalidReason::InvalidPosition => format!(
                    "socket `{}` fallback position must be finite and inside equipment bounds",
                    semantic.as_str()
                ),
            },
            Self::InvalidSocketReferenceRule { semantic, .. } => format!(
                "socket `{}` must allow at least one reference kind",
                semantic.as_str()
            ),
            Self::MissingSocket { semantic } => {
                format!(
                    "equipment is missing semantic socket `{}`",
                    semantic.as_str()
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanEquipmentSocketFallbackInvalidReason {
    SelfReference,
    InvalidPosition,
}

pub fn fangyuan_equipment_audit_budget_profile() -> FangyuanAuditBudgetProfile {
    let mut profile = FangyuanAuditBudgetProfile::default();
    profile.hard_primitive_limit = FANGYUAN_EQUIPMENT_DEFAULT_MAX_PRIMITIVE_COUNT;
    profile.recommended_primitive_limit = FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_PRIMITIVE_LIMIT;
    profile.max_bounds = Vec3::new(
        FANGYUAN_EQUIPMENT_DEFAULT_MAX_BOUNDS_WIDTH,
        FANGYUAN_EQUIPMENT_DEFAULT_MAX_BOUNDS_HEIGHT,
        FANGYUAN_EQUIPMENT_DEFAULT_MAX_BOUNDS_DEPTH,
    );
    profile.recommended_transparent_count =
        FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_TRANSPARENT_COUNT;
    profile.max_transparent_count = FANGYUAN_EQUIPMENT_DEFAULT_MAX_TRANSPARENT_COUNT;
    profile.recommended_alpha_count = FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_TRANSPARENT_COUNT;
    profile.max_alpha_count = FANGYUAN_EQUIPMENT_DEFAULT_MAX_TRANSPARENT_COUNT;
    profile.recommended_emissive_count = FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_EMISSIVE_COUNT;
    profile.max_emissive_count = FANGYUAN_EQUIPMENT_DEFAULT_MAX_EMISSIVE_COUNT;
    profile.recommended_material_profile_count =
        FANGYUAN_EQUIPMENT_DEFAULT_RECOMMENDED_MATERIAL_PROFILE_COUNT;
    profile.max_material_profile_count = FANGYUAN_EQUIPMENT_DEFAULT_MAX_MATERIAL_PROFILE_COUNT;
    profile.max_emissive_intensity = FANGYUAN_PRIMITIVE_MAX_EMISSIVE;
    profile
}

pub fn fangyuan_default_equipment_blueprint() -> FangyuanEquipmentBlueprint {
    FangyuanEquipmentBlueprint {
        version: FANGYUAN_EQUIPMENT_BLUEPRINT_VERSION.to_string(),
        id: "equipment.default_practice_blade".to_string(),
        max_primitives: 16,
        bounds: FangyuanBlueprintBounds::new(2.4, 0.8, 1.4),
        primitives: vec![
            FangyuanPrimitiveBlueprint {
                material_profile_id: Some("equipment/handle".to_string()),
                ..FangyuanPrimitiveBlueprint::new(
                    super::FangyuanPrimitiveKind::Cube,
                    [-0.55, 0.35, 0.0],
                    [0.4, 0.18, 0.18],
                    [0.25, 0.18, 0.12, 1.0],
                )
            },
            FangyuanPrimitiveBlueprint {
                material_profile_id: Some("equipment/blade".to_string()),
                ..FangyuanPrimitiveBlueprint::new(
                    super::FangyuanPrimitiveKind::Cube,
                    [0.25, 0.45, 0.0],
                    [1.25, 0.14, 0.1],
                    [0.82, 0.88, 0.95, 1.0],
                )
            },
            FangyuanPrimitiveBlueprint {
                role: Some(super::FangyuanPrimitiveRole::Core),
                material_profile_id: Some("equipment/core".to_string()),
                emissive: Some(0.4),
                ..FangyuanPrimitiveBlueprint::new(
                    super::FangyuanPrimitiveKind::Sphere,
                    [0.0, 0.45, 0.0],
                    [0.18, 0.18, 0.18],
                    [0.35, 0.75, 1.0, 1.0],
                )
            },
        ],
        sockets: vec![
            FangyuanEquipmentSocket::new(FangyuanEquipmentSocketSemantic::Grip, [-0.75, 0.35, 0.0]),
            FangyuanEquipmentSocket::new(FangyuanEquipmentSocketSemantic::Tip, [0.95, 0.45, 0.0]),
            FangyuanEquipmentSocket::new(FangyuanEquipmentSocketSemantic::Core, [0.0, 0.45, 0.0]),
            FangyuanEquipmentSocket::new(
                FangyuanEquipmentSocketSemantic::Guard,
                [-0.32, 0.42, 0.0],
            ),
            FangyuanEquipmentSocket::new(FangyuanEquipmentSocketSemantic::Aura, [0.0, 0.82, 0.0]),
        ],
    }
}

fn validate_equipment_sockets(
    sockets: &[FangyuanEquipmentSocket],
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanEquipmentValidationError> {
    let mut seen = BTreeMap::<FangyuanEquipmentSocketSemantic, usize>::new();
    for (index, socket) in sockets.iter().enumerate() {
        if let Some(first_index) = seen.insert(socket.semantic, index) {
            return Err(FangyuanEquipmentValidationError::DuplicateSocket {
                semantic: socket.semantic,
                first_index,
                duplicate_index: index,
            });
        }
        validate_socket_record(index, socket, bounds)?;
    }

    Ok(())
}

fn audit_equipment_socket_records(
    report: &mut FangyuanAuditReport,
    sockets: &[FangyuanEquipmentSocket],
    bounds: &FangyuanBlueprintBounds,
) {
    let mut seen = BTreeMap::<FangyuanEquipmentSocketSemantic, usize>::new();
    for (index, socket) in sockets.iter().enumerate() {
        if let Some(first_index) = seen.insert(socket.semantic, index) {
            report.add_finding(equipment_validation_error_to_audit_finding(
                &FangyuanEquipmentValidationError::DuplicateSocket {
                    semantic: socket.semantic,
                    first_index,
                    duplicate_index: index,
                },
                FangyuanAuditSeverity::Error,
            ));
        }

        if let Err(error) = validate_socket_record(index, socket, bounds) {
            report.add_finding(equipment_validation_error_to_audit_finding(
                &error,
                FangyuanAuditSeverity::Error,
            ));
        }
    }

    for semantic in REQUIRED_SOCKET_SEMANTICS {
        if !seen.contains_key(&semantic) {
            report.add_finding(equipment_validation_error_to_audit_finding(
                &FangyuanEquipmentValidationError::MissingSocket { semantic },
                FangyuanAuditSeverity::Warning,
            ));
        }
    }
}

fn validate_socket_record(
    index: usize,
    socket: &FangyuanEquipmentSocket,
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanEquipmentValidationError> {
    if socket.effective_allowed_references().is_empty() {
        return Err(
            FangyuanEquipmentValidationError::InvalidSocketReferenceRule {
                index,
                semantic: socket.semantic,
            },
        );
    }

    validate_socket_position(index, socket.semantic, socket.position, bounds)?;
    validate_socket_fallback(index, socket, bounds)?;
    Ok(())
}

fn validate_socket_fallback(
    index: usize,
    socket: &FangyuanEquipmentSocket,
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanEquipmentValidationError> {
    match socket.effective_fallback() {
        FangyuanEquipmentSocketFallback::Socket { semantic } if semantic == socket.semantic => {
            Err(FangyuanEquipmentValidationError::InvalidSocketFallback {
                index,
                semantic: socket.semantic,
                reason: FangyuanEquipmentSocketFallbackInvalidReason::SelfReference,
            })
        }
        FangyuanEquipmentSocketFallback::Socket { .. }
        | FangyuanEquipmentSocketFallback::Origin => Ok(()),
        FangyuanEquipmentSocketFallback::Position { position } => {
            validate_socket_position(index, socket.semantic, position, bounds).map_err(|_| {
                FangyuanEquipmentValidationError::InvalidSocketFallback {
                    index,
                    semantic: socket.semantic,
                    reason: FangyuanEquipmentSocketFallbackInvalidReason::InvalidPosition,
                }
            })
        }
    }
}

fn validate_socket_position(
    index: usize,
    semantic: FangyuanEquipmentSocketSemantic,
    position: [f32; 3],
    bounds: &FangyuanBlueprintBounds,
) -> Result<(), FangyuanEquipmentValidationError> {
    let ranges = [
        (-bounds.width * 0.5, bounds.width * 0.5),
        (0.0, bounds.height),
        (-bounds.depth * 0.5, bounds.depth * 0.5),
    ];

    for (axis, value) in position.into_iter().enumerate() {
        let (min, max) = ranges[axis];
        if !value.is_finite() || value < min || value > max {
            return Err(FangyuanEquipmentValidationError::InvalidSocketPosition {
                index,
                semantic,
                axis,
                value,
                min,
                max,
            });
        }
    }

    Ok(())
}

fn equipment_validation_error_to_audit_finding(
    error: &FangyuanEquipmentValidationError,
    severity: FangyuanAuditSeverity,
) -> FangyuanAuditFinding {
    let mut finding = FangyuanAuditFinding::new(
        severity,
        error.code(),
        error.reason(),
        FangyuanAuditSourceKind::Blueprint,
    );
    finding.field_path = Some(error.field_path());
    if let FangyuanEquipmentValidationError::InvalidPrimitive { index, .. } = error {
        finding.primitive_index = Some(*index);
    }
    finding
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use crate::framework::fangyuan::FangyuanAuditStatus;

    use super::*;

    #[test]
    fn fangyuan_equipment_default_blueprint_validates_and_compiles_runtime_primitive_set() {
        let blueprint = fangyuan_default_equipment_blueprint();

        blueprint.validate().unwrap();
        let runtime = blueprint.compile_runtime().unwrap();

        assert_eq!(runtime.primitive_set.len(), 3);
        assert_eq!(runtime.sockets.len(), 5);
        assert_eq!(
            runtime
                .sockets
                .get(FangyuanEquipmentSocketSemantic::Tip)
                .unwrap()
                .position,
            Vec3::new(0.95, 0.45, 0.0)
        );
    }

    #[test]
    fn fangyuan_equipment_missing_socket_is_audited_without_blocking_compile() {
        let mut blueprint = fangyuan_default_equipment_blueprint();
        blueprint
            .sockets
            .retain(|socket| socket.semantic != FangyuanEquipmentSocketSemantic::Tip);

        blueprint.validate().unwrap();
        let report = blueprint.audit_with_default_budget();

        assert_eq!(blueprint.compile().unwrap().len(), 3);
        assert!(has_equipment_finding(&report, "equipment_missing_socket"));
        assert_eq!(report.status, FangyuanAuditStatus::PassedWithWarnings);
    }

    #[test]
    fn fangyuan_equipment_duplicate_socket_is_rejected_and_audited() {
        let mut blueprint = fangyuan_default_equipment_blueprint();
        blueprint.sockets.push(FangyuanEquipmentSocket::new(
            FangyuanEquipmentSocketSemantic::Tip,
            [0.9, 0.45, 0.0],
        ));

        let error = blueprint.validate().unwrap_err();
        let report = blueprint.audit_with_default_budget();

        assert_eq!(error.code(), "equipment_duplicate_socket");
        assert!(has_equipment_finding(&report, "equipment_duplicate_socket"));
        assert_eq!(report.status, FangyuanAuditStatus::Failed);
    }

    #[test]
    fn fangyuan_equipment_illegal_socket_position_is_rejected_and_audited() {
        let mut blueprint = fangyuan_default_equipment_blueprint();
        blueprint
            .sockets
            .iter_mut()
            .find(|socket| socket.semantic == FangyuanEquipmentSocketSemantic::Tip)
            .unwrap()
            .position = [9.0, 0.45, 0.0];

        let error = blueprint.validate().unwrap_err();
        let report = blueprint.audit_with_default_budget();

        assert_eq!(error.code(), "equipment_invalid_socket_position");
        assert!(has_equipment_finding(
            &report,
            "equipment_invalid_socket_position"
        ));
        assert_eq!(report.status, FangyuanAuditStatus::Failed);
    }

    #[test]
    fn fangyuan_equipment_audit_reports_primitive_bounds_material_transparency_and_emissive() {
        let mut blueprint = fangyuan_default_equipment_blueprint();
        blueprint.primitives = vec![
            FangyuanPrimitiveBlueprint {
                alpha: Some(0.35),
                material_profile_id: Some("equipment/glass_a".to_string()),
                ..FangyuanPrimitiveBlueprint::new(
                    super::super::FangyuanPrimitiveKind::Cube,
                    [-3.0, 2.5, 0.0],
                    [5.0, 5.0, 0.2],
                    [0.8, 0.9, 1.0, 0.35],
                )
            },
            FangyuanPrimitiveBlueprint {
                emissive: Some(8.0),
                material_profile_id: Some("equipment/glow_b".to_string()),
                ..FangyuanPrimitiveBlueprint::new(
                    super::super::FangyuanPrimitiveKind::Sphere,
                    [3.0, 2.5, 0.0],
                    [5.0, 5.0, 0.2],
                    [0.2, 0.6, 1.0, 1.0],
                )
            },
            FangyuanPrimitiveBlueprint {
                alpha: Some(0.5),
                material_profile_id: Some("equipment/glass_c".to_string()),
                ..FangyuanPrimitiveBlueprint::new(
                    super::super::FangyuanPrimitiveKind::Cube,
                    [0.0, 2.5, 0.0],
                    [5.0, 5.0, 0.2],
                    [0.8, 0.8, 1.0, 0.5],
                )
            },
        ];
        blueprint.bounds = FangyuanBlueprintBounds::new(12.0, 2.0, 6.0);
        let mut profile = fangyuan_equipment_audit_budget_profile();
        profile.recommended_transparent_count = 1;
        profile.max_transparent_count = 1;
        profile.recommended_alpha_count = 1;
        profile.max_alpha_count = 1;
        profile.recommended_primitive_limit = 1;
        profile.hard_primitive_limit = 2;
        profile.recommended_emissive_count = 0;
        profile.max_emissive_count = 0;
        profile.recommended_material_profile_count = 1;
        profile.max_material_profile_count = 2;

        let report = blueprint.audit(&profile);

        assert!(has_equipment_finding(
            &report,
            "primitive_count_above_hard_limit"
        ));
        assert!(has_equipment_finding(&report, "bounds_above_limit"));
        assert!(has_equipment_finding(
            &report,
            "transparent_count_above_hard_limit"
        ));
        assert!(has_equipment_finding(
            &report,
            "alpha_count_above_hard_limit"
        ));
        assert!(has_equipment_finding(
            &report,
            "emissive_count_above_hard_limit"
        ));
        assert!(has_equipment_finding(
            &report,
            "material_profile_count_above_hard_limit"
        ));
    }

    #[test]
    fn fangyuan_equipment_socket_resolution_uses_explicit_fallback_for_missing_socket() {
        let mut blueprint = fangyuan_default_equipment_blueprint();
        blueprint
            .sockets
            .retain(|socket| socket.semantic != FangyuanEquipmentSocketSemantic::Tip);
        let sockets = blueprint.compile_sockets().unwrap();

        let resolution = sockets.resolve_with_fallback(
            FangyuanEquipmentSocketSemantic::Tip,
            FangyuanEquipmentSocketReferenceKind::Emit,
            Some(&FangyuanEquipmentSocketFallback::Socket {
                semantic: FangyuanEquipmentSocketSemantic::Core,
            }),
        );

        assert_eq!(
            resolution.position,
            Vec3::new(0.0, 0.45, 0.0),
            "missing tip must resolve to the explicit core fallback"
        );
        assert_eq!(
            resolution.fallback.unwrap().reason,
            FangyuanEquipmentSocketFallbackReason::MissingSocket
        );
        assert_eq!(
            resolution.resolved_semantic,
            Some(FangyuanEquipmentSocketSemantic::Core)
        );
    }

    fn has_equipment_finding(report: &FangyuanAuditReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }
}
