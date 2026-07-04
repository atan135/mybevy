use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
    FangyuanAuditBudgetProfile, FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity,
    FangyuanAuditSourceKind, FangyuanBlueprintBounds, FangyuanBlueprintValidationError,
    FangyuanPrimitive, FangyuanPrimitiveBlueprint, FangyuanPrimitiveBudgetStats,
    FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole, FangyuanPrimitiveSet,
    audit_fangyuan_primitive_budget,
    blueprint::{compile_blueprint_primitive_to_runtime, validate_blueprint_primitive},
};

pub const FANGYUAN_NPC_BLUEPRINT_VERSION: &str = "1";
pub const FANGYUAN_NPC_BLUEPRINT_HARD_PRIMITIVE_LIMIT: usize = 64;
pub const FANGYUAN_NPC_DEFAULT_RECOMMENDED_PRIMITIVE_LIMIT: usize = 12;
pub const FANGYUAN_NPC_DEFAULT_MAX_PRIMITIVE_COUNT: usize = 24;
pub const FANGYUAN_NPC_DEFAULT_MAX_BOUNDS_WIDTH: f32 = 4.0;
pub const FANGYUAN_NPC_DEFAULT_MAX_BOUNDS_DEPTH: f32 = 4.0;
pub const FANGYUAN_NPC_DEFAULT_MAX_BOUNDS_HEIGHT: f32 = 4.0;
pub const FANGYUAN_NPC_DEFAULT_RECOMMENDED_TRANSPARENT_COUNT: usize = 3;
pub const FANGYUAN_NPC_DEFAULT_MAX_TRANSPARENT_COUNT: usize = 6;
pub const FANGYUAN_NPC_DEFAULT_RECOMMENDED_EMISSIVE_COUNT: usize = 2;
pub const FANGYUAN_NPC_DEFAULT_MAX_EMISSIVE_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNpcBlueprint {
    pub version: String,
    pub id: String,
    pub profile: FangyuanNpcProfile,
    pub max_primitives: usize,
    pub bounds: FangyuanBlueprintBounds,
    #[serde(default)]
    pub simple_state: FangyuanNpcSimpleState,
}

impl FangyuanNpcBlueprint {
    pub fn validate(&self) -> Result<(), FangyuanNpcValidationError> {
        self.validate_top_level()?;

        let authored = self.profile.compile_authored_primitives(self.simple_state);
        if authored.len() > self.primitive_limit() {
            return Err(FangyuanNpcValidationError::PrimitiveCountExceeded {
                count: authored.len(),
                limit: self.primitive_limit(),
                max_primitives: self.max_primitives,
                hard_limit: FANGYUAN_NPC_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            });
        }
        for (index, primitive) in authored.iter().enumerate() {
            validate_blueprint_primitive(index, primitive, &self.bounds)
                .map_err(|source| FangyuanNpcValidationError::InvalidPrimitive { index, source })?;
        }

        Ok(())
    }

    fn validate_top_level(&self) -> Result<(), FangyuanNpcValidationError> {
        if self.version != FANGYUAN_NPC_BLUEPRINT_VERSION {
            return Err(FangyuanNpcValidationError::UnsupportedVersion {
                found: self.version.clone(),
                expected: FANGYUAN_NPC_BLUEPRINT_VERSION,
            });
        }
        if self.id.trim().is_empty() {
            return Err(FangyuanNpcValidationError::EmptyNpcId);
        }
        self.bounds
            .validate()
            .map_err(FangyuanNpcValidationError::InvalidBounds)?;
        self.profile.validate()?;
        Ok(())
    }

    fn primitive_limit(&self) -> usize {
        self.max_primitives
            .min(FANGYUAN_NPC_BLUEPRINT_HARD_PRIMITIVE_LIMIT)
    }

    pub fn compile(&self) -> Result<FangyuanPrimitiveSet, FangyuanNpcValidationError> {
        self.compile_for(FangyuanNpcCompileOptions::default())
    }

    pub fn compile_for(
        &self,
        options: FangyuanNpcCompileOptions,
    ) -> Result<FangyuanPrimitiveSet, FangyuanNpcValidationError> {
        self.validate()?;
        let authored = self
            .profile
            .compile_authored_primitives(options.simple_state.unwrap_or(self.simple_state));
        Ok(degrade_npc_primitives(
            authored
                .iter()
                .map(compile_blueprint_primitive_to_runtime)
                .collect(),
            options.degrade_level,
        ))
    }

    pub fn audit(&self, profile: &FangyuanAuditBudgetProfile) -> FangyuanAuditReport {
        let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::Blueprint, None);

        if let Err(error) = self.validate_top_level() {
            report.add_finding(npc_validation_error_to_audit_finding(
                &error,
                FangyuanAuditSeverity::Error,
            ));
            report.sort_findings();
            return report;
        }

        let authored = self.profile.compile_authored_primitives(self.simple_state);
        let mut primitives = Vec::with_capacity(authored.len());
        let mut skipped_primitives = 0usize;
        for (index, primitive) in authored.iter().enumerate() {
            match validate_blueprint_primitive(index, primitive, &self.bounds) {
                Ok(()) => primitives.push(compile_blueprint_primitive_to_runtime(primitive)),
                Err(source) => {
                    skipped_primitives += 1;
                    let error = FangyuanNpcValidationError::InvalidPrimitive { index, source };
                    report.add_finding(npc_validation_error_to_audit_finding(
                        &error,
                        FangyuanAuditSeverity::Warning,
                    ));
                }
            }
        }

        if authored.len() > self.primitive_limit() {
            report.add_finding(npc_validation_error_to_audit_finding(
                &FangyuanNpcValidationError::PrimitiveCountExceeded {
                    count: authored.len(),
                    limit: self.primitive_limit(),
                    max_primitives: self.max_primitives,
                    hard_limit: FANGYUAN_NPC_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
                },
                FangyuanAuditSeverity::Error,
            ));
        }

        let primitive_set = FangyuanPrimitiveSet::from_primitives(primitives);
        let mut stats = FangyuanPrimitiveBudgetStats::from_primitive_set(&primitive_set);
        stats.authored_primitives = authored.len();
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
        self.audit(&fangyuan_npc_audit_budget_profile())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanNpcSimpleState {
    Idle,
    Moving,
    Casting,
    Damaged,
}

impl Default for FangyuanNpcSimpleState {
    fn default() -> Self {
        Self::Idle
    }
}

impl FangyuanNpcSimpleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Moving => "moving",
            Self::Casting => "casting",
            Self::Damaged => "damaged",
        }
    }

    const fn presentation(self) -> FangyuanNpcStatePresentation {
        match self {
            Self::Idle => FangyuanNpcStatePresentation {
                body_scale_factor: 1.0,
                marker_alpha: 0.72,
                aura_alpha: 0.26,
                aura_emissive: 0.35,
                role_emissive: 0.15,
                color_multiplier: [1.0, 1.0, 1.0],
                lifecycle: FangyuanPrimitiveLifecycle::empty(),
            },
            Self::Moving => FangyuanNpcStatePresentation {
                body_scale_factor: 1.04,
                marker_alpha: 0.82,
                aura_alpha: 0.18,
                aura_emissive: 0.2,
                role_emissive: 0.25,
                color_multiplier: [0.9, 1.04, 1.12],
                lifecycle: FangyuanPrimitiveLifecycle::new(Some(18), None, None),
            },
            Self::Casting => FangyuanNpcStatePresentation {
                body_scale_factor: 1.08,
                marker_alpha: 0.95,
                aura_alpha: 0.44,
                aura_emissive: 1.8,
                role_emissive: 1.2,
                color_multiplier: [1.16, 1.08, 0.86],
                lifecycle: FangyuanPrimitiveLifecycle::new(Some(24), None, None),
            },
            Self::Damaged => FangyuanNpcStatePresentation {
                body_scale_factor: 0.96,
                marker_alpha: 0.68,
                aura_alpha: 0.12,
                aura_emissive: 0.0,
                role_emissive: 0.75,
                color_multiplier: [1.18, 0.45, 0.45],
                lifecycle: FangyuanPrimitiveLifecycle::new(Some(10), None, None),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FangyuanNpcStatePresentation {
    body_scale_factor: f32,
    marker_alpha: f32,
    aura_alpha: f32,
    aura_emissive: f32,
    role_emissive: f32,
    color_multiplier: [f32; 3],
    lifecycle: FangyuanPrimitiveLifecycle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanNpcDegradeLevel {
    Full,
    Silhouette,
    Marker,
    Nameplate,
}

impl Default for FangyuanNpcDegradeLevel {
    fn default() -> Self {
        Self::Full
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanNpcCompileOptions {
    pub simple_state: Option<FangyuanNpcSimpleState>,
    pub degrade_level: FangyuanNpcDegradeLevel,
}

impl FangyuanNpcCompileOptions {
    pub const fn for_state(simple_state: FangyuanNpcSimpleState) -> Self {
        Self {
            simple_state: Some(simple_state),
            degrade_level: FangyuanNpcDegradeLevel::Full,
        }
    }

    pub const fn degraded(degrade_level: FangyuanNpcDegradeLevel) -> Self {
        Self {
            simple_state: None,
            degrade_level,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNpcProfile {
    pub body_color: [f32; 4],
    pub marker_color: [f32; 4],
    pub role_color: [f32; 4],
    pub aura_color: [f32; 4],
    #[serde(default)]
    pub body: FangyuanNpcBodyProfile,
    #[serde(default)]
    pub marker: FangyuanNpcMarkerProfile,
    #[serde(default)]
    pub aura: FangyuanNpcAuraProfile,
    #[serde(default)]
    pub nameplate: FangyuanNpcNameplateProfile,
}

impl FangyuanNpcProfile {
    pub fn validate(&self) -> Result<(), FangyuanNpcValidationError> {
        validate_color("profile.body_color", self.body_color)?;
        validate_color("profile.marker_color", self.marker_color)?;
        validate_color("profile.role_color", self.role_color)?;
        validate_color("profile.aura_color", self.aura_color)?;
        self.body.validate()?;
        self.marker.validate()?;
        self.aura.validate()?;
        self.nameplate.validate()?;
        Ok(())
    }

    fn compile_authored_primitives(
        &self,
        simple_state: FangyuanNpcSimpleState,
    ) -> Vec<FangyuanPrimitiveBlueprint> {
        let state = simple_state.presentation();
        let body_height = self.body.height * state.body_scale_factor;
        let body_color = modulate_color(self.body_color, state.color_multiplier);
        let role_color = modulate_color(self.role_color, state.color_multiplier);
        let mut marker_color = modulate_color(self.marker_color, state.color_multiplier);
        marker_color[3] = state.marker_alpha;
        let mut aura_color = modulate_color(self.aura_color, state.color_multiplier);
        aura_color[3] = state.aura_alpha;

        let mut primitives = vec![
            primitive_blueprint(
                FangyuanPrimitiveKind::Cube,
                FangyuanPrimitiveRole::Structure,
                [0.0, body_height * 0.5, 0.0],
                [self.body.width, body_height, self.body.depth],
                body_color,
                Some("npc/body".to_string()),
                None,
                Some(state.lifecycle),
            ),
            primitive_blueprint(
                FangyuanPrimitiveKind::Sphere,
                FangyuanPrimitiveRole::Core,
                [0.0, body_height + self.body.head_radius * 0.45, 0.0],
                [self.body.head_radius; 3],
                role_color,
                Some("npc/role_core".to_string()),
                Some(state.role_emissive),
                Some(state.lifecycle),
            ),
            primitive_blueprint(
                FangyuanPrimitiveKind::Cube,
                FangyuanPrimitiveRole::Boundary,
                [0.0, self.marker.height, 0.0],
                [self.marker.width, self.marker.thickness, self.marker.depth],
                marker_color,
                Some("npc/marker".to_string()),
                Some(FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE),
                Some(state.lifecycle),
            ),
            primitive_blueprint(
                FangyuanPrimitiveKind::Sphere,
                FangyuanPrimitiveRole::Decoration,
                [0.0, self.aura.height, 0.0],
                [self.aura.radius, self.aura.thickness, self.aura.radius],
                aura_color,
                Some("npc/aura".to_string()),
                Some(state.aura_emissive),
                Some(state.lifecycle),
            ),
            primitive_blueprint(
                FangyuanPrimitiveKind::Cube,
                FangyuanPrimitiveRole::Archive,
                [0.0, self.nameplate.height, 0.0],
                [
                    self.nameplate.width,
                    self.nameplate.thickness,
                    self.nameplate.depth,
                ],
                self.nameplate.color,
                Some("npc/nameplate".to_string()),
                Some(FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE),
                Some(state.lifecycle),
            ),
        ];

        if !self.aura.enabled {
            primitives.retain(|primitive| primitive.role() != FangyuanPrimitiveRole::Decoration);
        }

        primitives
    }
}

impl Default for FangyuanNpcProfile {
    fn default() -> Self {
        Self {
            body_color: [0.28, 0.34, 0.38, 1.0],
            marker_color: [0.12, 0.76, 0.95, 0.72],
            role_color: [0.95, 0.78, 0.24, 1.0],
            aura_color: [0.35, 0.8, 1.0, 0.26],
            body: FangyuanNpcBodyProfile::default(),
            marker: FangyuanNpcMarkerProfile::default(),
            aura: FangyuanNpcAuraProfile::default(),
            nameplate: FangyuanNpcNameplateProfile::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNpcBodyProfile {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    pub head_radius: f32,
}

impl Default for FangyuanNpcBodyProfile {
    fn default() -> Self {
        Self {
            width: 0.44,
            depth: 0.28,
            height: 1.15,
            head_radius: 0.34,
        }
    }
}

impl FangyuanNpcBodyProfile {
    fn validate(&self) -> Result<(), FangyuanNpcValidationError> {
        validate_positive_range("profile.body.width", self.width, 0.1, 2.0)?;
        validate_positive_range("profile.body.depth", self.depth, 0.1, 2.0)?;
        validate_positive_range("profile.body.height", self.height, 0.2, 3.0)?;
        validate_positive_range("profile.body.head_radius", self.head_radius, 0.1, 1.5)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNpcMarkerProfile {
    pub width: f32,
    pub depth: f32,
    pub thickness: f32,
    pub height: f32,
}

impl Default for FangyuanNpcMarkerProfile {
    fn default() -> Self {
        Self {
            width: 0.92,
            depth: 0.92,
            thickness: 0.1,
            height: 0.05,
        }
    }
}

impl FangyuanNpcMarkerProfile {
    fn validate(&self) -> Result<(), FangyuanNpcValidationError> {
        validate_positive_range("profile.marker.width", self.width, 0.1, 3.0)?;
        validate_positive_range("profile.marker.depth", self.depth, 0.1, 3.0)?;
        validate_positive_range("profile.marker.thickness", self.thickness, 0.1, 0.5)?;
        validate_positive_range("profile.marker.height", self.height, 0.05, 2.0)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNpcAuraProfile {
    pub enabled: bool,
    pub radius: f32,
    pub thickness: f32,
    pub height: f32,
}

impl Default for FangyuanNpcAuraProfile {
    fn default() -> Self {
        Self {
            enabled: true,
            radius: 1.05,
            thickness: 0.12,
            height: 1.05,
        }
    }
}

impl FangyuanNpcAuraProfile {
    fn validate(&self) -> Result<(), FangyuanNpcValidationError> {
        validate_positive_range("profile.aura.radius", self.radius, 0.1, 3.0)?;
        validate_positive_range("profile.aura.thickness", self.thickness, 0.1, 1.0)?;
        validate_positive_range("profile.aura.height", self.height, 0.1, 3.0)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNpcNameplateProfile {
    pub width: f32,
    pub depth: f32,
    pub thickness: f32,
    pub height: f32,
    pub color: [f32; 4],
}

impl Default for FangyuanNpcNameplateProfile {
    fn default() -> Self {
        Self {
            width: 0.82,
            depth: 0.18,
            thickness: 0.1,
            height: 1.78,
            color: [0.05, 0.08, 0.1, 0.86],
        }
    }
}

impl FangyuanNpcNameplateProfile {
    fn validate(&self) -> Result<(), FangyuanNpcValidationError> {
        validate_positive_range("profile.nameplate.width", self.width, 0.1, 3.0)?;
        validate_positive_range("profile.nameplate.depth", self.depth, 0.1, 1.0)?;
        validate_positive_range("profile.nameplate.thickness", self.thickness, 0.1, 0.5)?;
        validate_positive_range("profile.nameplate.height", self.height, 0.1, 4.0)?;
        validate_color("profile.nameplate.color", self.color)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanNpcValidationError {
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    EmptyNpcId,
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
    InvalidProfileScalar {
        field: &'static str,
        value: f32,
        min: f32,
        max: f32,
    },
    InvalidProfileColor {
        field: &'static str,
        channel: usize,
        value: f32,
    },
}

impl FangyuanNpcValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "npc_unsupported_version",
            Self::EmptyNpcId => "npc_empty_id",
            Self::PrimitiveCountExceeded { .. } => "npc_primitive_count_exceeded",
            Self::InvalidBounds(_) => "npc_invalid_bounds",
            Self::InvalidPrimitive { source, .. } => source.code(),
            Self::InvalidProfileScalar { .. } => "npc_invalid_profile_scalar",
            Self::InvalidProfileColor { .. } => "npc_invalid_profile_color",
        }
    }

    pub fn field_path(&self) -> String {
        match self {
            Self::UnsupportedVersion { .. } => "version".to_string(),
            Self::EmptyNpcId => "id".to_string(),
            Self::PrimitiveCountExceeded { .. } => "primitives".to_string(),
            Self::InvalidBounds(error) => error.field_path().into_owned(),
            Self::InvalidPrimitive { source, .. } => source.field_path().into_owned(),
            Self::InvalidProfileScalar { field, .. } | Self::InvalidProfileColor { field, .. } => {
                (*field).to_string()
            }
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                format!("version `{found}` is unsupported; expected `{expected}`")
            }
            Self::EmptyNpcId => "npc id must not be empty".to_string(),
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
            Self::InvalidProfileScalar {
                field,
                value,
                min,
                max,
            } => format!("{field} value {value} must be finite and inside {min}..={max}"),
            Self::InvalidProfileColor {
                field,
                channel,
                value,
            } => {
                format!("{field}[{channel}] value {value} must be finite and inside 0..=1")
            }
        }
    }
}

pub fn fangyuan_npc_audit_budget_profile() -> FangyuanAuditBudgetProfile {
    let mut profile = FangyuanAuditBudgetProfile::default();
    profile.hard_primitive_limit = FANGYUAN_NPC_DEFAULT_MAX_PRIMITIVE_COUNT;
    profile.recommended_primitive_limit = FANGYUAN_NPC_DEFAULT_RECOMMENDED_PRIMITIVE_LIMIT;
    profile.max_bounds = Vec3::new(
        FANGYUAN_NPC_DEFAULT_MAX_BOUNDS_WIDTH,
        FANGYUAN_NPC_DEFAULT_MAX_BOUNDS_HEIGHT,
        FANGYUAN_NPC_DEFAULT_MAX_BOUNDS_DEPTH,
    );
    profile.recommended_transparent_count = FANGYUAN_NPC_DEFAULT_RECOMMENDED_TRANSPARENT_COUNT;
    profile.max_transparent_count = FANGYUAN_NPC_DEFAULT_MAX_TRANSPARENT_COUNT;
    profile.recommended_alpha_count = FANGYUAN_NPC_DEFAULT_RECOMMENDED_TRANSPARENT_COUNT;
    profile.max_alpha_count = FANGYUAN_NPC_DEFAULT_MAX_TRANSPARENT_COUNT;
    profile.recommended_emissive_count = FANGYUAN_NPC_DEFAULT_RECOMMENDED_EMISSIVE_COUNT;
    profile.max_emissive_count = FANGYUAN_NPC_DEFAULT_MAX_EMISSIVE_COUNT;
    profile.max_emissive_intensity = FANGYUAN_PRIMITIVE_MAX_EMISSIVE;
    profile
}

pub fn fangyuan_default_npc_blueprint() -> FangyuanNpcBlueprint {
    FangyuanNpcBlueprint {
        version: FANGYUAN_NPC_BLUEPRINT_VERSION.to_string(),
        id: "npc.default_wayfarer".to_string(),
        profile: FangyuanNpcProfile::default(),
        max_primitives: 8,
        bounds: FangyuanBlueprintBounds::new(2.4, 2.4, 2.4),
        simple_state: FangyuanNpcSimpleState::Idle,
    }
}

fn degrade_npc_primitives(
    primitives: Vec<FangyuanPrimitive>,
    degrade_level: FangyuanNpcDegradeLevel,
) -> FangyuanPrimitiveSet {
    let retained = primitives
        .into_iter()
        .filter_map(|primitive| degrade_npc_primitive(primitive, degrade_level))
        .collect();
    FangyuanPrimitiveSet::from_primitives(retained)
}

fn degrade_npc_primitive(
    primitive: FangyuanPrimitive,
    degrade_level: FangyuanNpcDegradeLevel,
) -> Option<FangyuanPrimitive> {
    match degrade_level {
        FangyuanNpcDegradeLevel::Full => Some(primitive),
        FangyuanNpcDegradeLevel::Silhouette => match primitive.role() {
            FangyuanPrimitiveRole::Structure | FangyuanPrimitiveRole::Core => {
                Some(strip_high_cost_material(primitive, 0.72))
            }
            FangyuanPrimitiveRole::Boundary => Some(strip_high_cost_material(primitive, 0.56)),
            FangyuanPrimitiveRole::Archive
            | FangyuanPrimitiveRole::Decoration
            | FangyuanPrimitiveRole::Warning
            | FangyuanPrimitiveRole::Trail
            | FangyuanPrimitiveRole::Impact
            | FangyuanPrimitiveRole::Socket => None,
        },
        FangyuanNpcDegradeLevel::Marker => match primitive.role() {
            FangyuanPrimitiveRole::Boundary => Some(strip_high_cost_material(primitive, 0.85)),
            FangyuanPrimitiveRole::Core => Some(strip_high_cost_material(primitive, 1.0)),
            FangyuanPrimitiveRole::Structure
            | FangyuanPrimitiveRole::Archive
            | FangyuanPrimitiveRole::Decoration
            | FangyuanPrimitiveRole::Warning
            | FangyuanPrimitiveRole::Trail
            | FangyuanPrimitiveRole::Impact
            | FangyuanPrimitiveRole::Socket => None,
        },
        FangyuanNpcDegradeLevel::Nameplate => match primitive.role() {
            FangyuanPrimitiveRole::Archive => Some(strip_high_cost_material(primitive, 0.92)),
            FangyuanPrimitiveRole::Structure
            | FangyuanPrimitiveRole::Core
            | FangyuanPrimitiveRole::Boundary
            | FangyuanPrimitiveRole::Decoration
            | FangyuanPrimitiveRole::Warning
            | FangyuanPrimitiveRole::Trail
            | FangyuanPrimitiveRole::Impact
            | FangyuanPrimitiveRole::Socket => None,
        },
    }
}

fn strip_high_cost_material(primitive: FangyuanPrimitive, alpha: f32) -> FangyuanPrimitive {
    let mut color = primitive.color().to_srgba();
    color.alpha = color.alpha.min(alpha);
    FangyuanPrimitive::with_runtime_metadata(
        primitive.kind(),
        primitive.local_position(),
        primitive.scale(),
        Color::srgba(color.red, color.green, color.blue, color.alpha),
        primitive.role(),
        color.alpha,
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
        None,
        FangyuanPrimitiveLifecycle::empty(),
    )
}

fn primitive_blueprint(
    kind: FangyuanPrimitiveKind,
    role: FangyuanPrimitiveRole,
    position: [f32; 3],
    size: [f32; 3],
    color: [f32; 4],
    material_profile_id: Option<String>,
    emissive: Option<f32>,
    lifecycle: Option<FangyuanPrimitiveLifecycle>,
) -> super::FangyuanPrimitiveBlueprint {
    super::FangyuanPrimitiveBlueprint {
        role: Some(role),
        alpha: Some(color[3]),
        emissive,
        material_profile_id,
        lifecycle,
        ..super::FangyuanPrimitiveBlueprint::new(kind, position, size, color)
    }
}

fn validate_positive_range(
    field: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> Result<(), FangyuanNpcValidationError> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(FangyuanNpcValidationError::InvalidProfileScalar {
            field,
            value,
            min,
            max,
        })
    }
}

fn validate_color(field: &'static str, color: [f32; 4]) -> Result<(), FangyuanNpcValidationError> {
    for (channel, value) in color.into_iter().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(FangyuanNpcValidationError::InvalidProfileColor {
                field,
                channel,
                value,
            });
        }
    }
    Ok(())
}

fn modulate_color(color: [f32; 4], multiplier: [f32; 3]) -> [f32; 4] {
    [
        (color[0] * multiplier[0]).clamp(0.0, 1.0),
        (color[1] * multiplier[1]).clamp(0.0, 1.0),
        (color[2] * multiplier[2]).clamp(0.0, 1.0),
        color[3],
    ]
}

fn npc_validation_error_to_audit_finding(
    error: &FangyuanNpcValidationError,
    severity: FangyuanAuditSeverity,
) -> FangyuanAuditFinding {
    let mut finding = FangyuanAuditFinding::new(
        severity,
        error.code(),
        error.reason(),
        FangyuanAuditSourceKind::Blueprint,
    );
    finding.field_path = Some(error.field_path());
    if let FangyuanNpcValidationError::InvalidPrimitive { index, .. } = error {
        finding.primitive_index = Some(*index);
    }
    finding
}

#[cfg(test)]
mod tests {
    use crate::framework::fangyuan::FangyuanAuditStatus;

    use super::*;

    #[test]
    fn fangyuan_npc_default_blueprint_validates_and_compiles_low_cost_body_marker_role_aura() {
        let blueprint = fangyuan_default_npc_blueprint();

        blueprint.validate().unwrap();
        let primitive_set = blueprint.compile().unwrap();

        assert_eq!(primitive_set.len(), 5);
        assert!(has_role(&primitive_set, FangyuanPrimitiveRole::Structure));
        assert!(has_role(&primitive_set, FangyuanPrimitiveRole::Boundary));
        assert!(has_role(&primitive_set, FangyuanPrimitiveRole::Core));
        assert!(has_role(&primitive_set, FangyuanPrimitiveRole::Decoration));
        assert!(has_role(&primitive_set, FangyuanPrimitiveRole::Archive));
        assert!(
            primitive_set
                .primitives()
                .iter()
                .all(|primitive| primitive.scale().max_element() <= 1.8)
        );
    }

    #[test]
    fn fangyuan_npc_state_switches_change_material_alpha_emissive_and_lifecycle() {
        let blueprint = fangyuan_default_npc_blueprint();
        let idle = blueprint
            .compile_for(FangyuanNpcCompileOptions::for_state(
                FangyuanNpcSimpleState::Idle,
            ))
            .unwrap();
        let casting = blueprint
            .compile_for(FangyuanNpcCompileOptions::for_state(
                FangyuanNpcSimpleState::Casting,
            ))
            .unwrap();
        let damaged = blueprint
            .compile_for(FangyuanNpcCompileOptions::for_state(
                FangyuanNpcSimpleState::Damaged,
            ))
            .unwrap();

        let idle_aura = find_role(&idle, FangyuanPrimitiveRole::Decoration);
        let casting_aura = find_role(&casting, FangyuanPrimitiveRole::Decoration);
        let damaged_core = find_role(&damaged, FangyuanPrimitiveRole::Core);

        assert!(casting_aura.emissive() > idle_aura.emissive());
        assert!(casting_aura.alpha() > idle_aura.alpha());
        assert!(damaged_core.emissive() > idle_aura.emissive());
        assert_eq!(casting_aura.lifecycle().lifetime, Some(24));
        assert_eq!(damaged_core.lifecycle().lifetime, Some(10));
    }

    #[test]
    fn fangyuan_npc_moving_state_keeps_identity_but_changes_body_motion_hint() {
        let blueprint = fangyuan_default_npc_blueprint();
        let idle = blueprint
            .compile_for(FangyuanNpcCompileOptions::for_state(
                FangyuanNpcSimpleState::Idle,
            ))
            .unwrap();
        let moving = blueprint
            .compile_for(FangyuanNpcCompileOptions::for_state(
                FangyuanNpcSimpleState::Moving,
            ))
            .unwrap();

        let idle_body = find_role(&idle, FangyuanPrimitiveRole::Structure);
        let moving_body = find_role(&moving, FangyuanPrimitiveRole::Structure);

        assert_eq!(idle_body.role(), moving_body.role());
        assert!(moving_body.scale().y > idle_body.scale().y);
        assert_eq!(moving_body.lifecycle().lifetime, Some(18));
    }

    #[test]
    fn fangyuan_npc_degrade_removes_aura_and_high_cost_decoration_under_pressure() {
        let blueprint = fangyuan_default_npc_blueprint();
        let full = blueprint.compile().unwrap();
        let silhouette = blueprint
            .compile_for(FangyuanNpcCompileOptions::degraded(
                FangyuanNpcDegradeLevel::Silhouette,
            ))
            .unwrap();
        let marker = blueprint
            .compile_for(FangyuanNpcCompileOptions::degraded(
                FangyuanNpcDegradeLevel::Marker,
            ))
            .unwrap();
        let nameplate = blueprint
            .compile_for(FangyuanNpcCompileOptions::degraded(
                FangyuanNpcDegradeLevel::Nameplate,
            ))
            .unwrap();

        assert!(full.len() > silhouette.len());
        assert!(!has_role(&silhouette, FangyuanPrimitiveRole::Decoration));
        assert!(!has_role(&marker, FangyuanPrimitiveRole::Decoration));
        assert!(has_role(&marker, FangyuanPrimitiveRole::Boundary));
        assert!(has_role(&marker, FangyuanPrimitiveRole::Core));
        assert_eq!(nameplate.len(), 1);
        assert!(has_role(&nameplate, FangyuanPrimitiveRole::Archive));
        assert!(
            marker
                .primitives()
                .iter()
                .all(|primitive| primitive.material_profile_id().is_none()
                    && primitive.emissive() == FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE)
        );
    }

    #[test]
    fn fangyuan_npc_audit_reports_budget_pressure_without_blocking_default_compile() {
        let mut blueprint = fangyuan_default_npc_blueprint();
        blueprint.profile.aura.enabled = true;
        let mut profile = fangyuan_npc_audit_budget_profile();
        profile.recommended_primitive_limit = 1;
        profile.hard_primitive_limit = 3;
        profile.max_emissive_count = 0;
        profile.recommended_emissive_count = 0;

        let report = blueprint.audit(&profile);

        assert_eq!(blueprint.compile().unwrap().len(), 5);
        assert!(has_finding(&report, "primitive_count_above_hard_limit"));
        assert!(has_finding(&report, "emissive_count_above_hard_limit"));
        assert_eq!(report.status, FangyuanAuditStatus::Failed);
    }

    #[test]
    fn fangyuan_npc_blueprint_rejects_invalid_profile_color() {
        let mut blueprint = fangyuan_default_npc_blueprint();
        blueprint.profile.role_color[0] = 1.5;

        let error = blueprint.validate().unwrap_err();

        assert_eq!(error.code(), "npc_invalid_profile_color");
        assert_eq!(error.field_path(), "profile.role_color");
    }

    fn has_role(primitive_set: &FangyuanPrimitiveSet, role: FangyuanPrimitiveRole) -> bool {
        primitive_set
            .primitives()
            .iter()
            .any(|primitive| primitive.role() == role)
    }

    fn find_role(
        primitive_set: &FangyuanPrimitiveSet,
        role: FangyuanPrimitiveRole,
    ) -> &FangyuanPrimitive {
        primitive_set
            .primitives()
            .iter()
            .find(|primitive| primitive.role() == role)
            .unwrap()
    }

    fn has_finding(report: &FangyuanAuditReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }
}
