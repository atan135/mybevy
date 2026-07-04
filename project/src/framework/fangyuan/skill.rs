use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    FangyuanEquipmentSocketFallback, FangyuanEquipmentSocketFallbackDiagnostic,
    FangyuanEquipmentSocketReferenceKind, FangyuanEquipmentSocketResolution,
    FangyuanEquipmentSocketSemantic, FangyuanEquipmentSocketSet, FangyuanPrimitiveKind,
    FangyuanPrimitiveRole, FangyuanVfxBudgetPressure, FangyuanVfxClock, FangyuanVfxCurve,
    FangyuanVfxDiagnostic, FangyuanVfxDynamicPrimitiveState, FangyuanVfxEmitter,
    FangyuanVfxEmitterJitter, FangyuanVfxOperator, FangyuanVfxRecipe, FangyuanVfxReplayContext,
    evaluate_fangyuan_vfx_recipe_with_budget_pressure, fangyuan_vfx_impact_expand_recipe,
    fangyuan_vfx_primitive_state_hash, fangyuan_vfx_projectile_recipe,
    fangyuan_vfx_range_marker_recipe, fangyuan_vfx_shield_recipe,
};

pub const FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION: u32 = 1;
pub const FANGYUAN_SKILL_DEFAULT_TEMPLATE_VERSION: u32 = 1;
pub const FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID: &str = "skill.template.projectile";
pub const FANGYUAN_SKILL_CIRCLE_AREA_TEMPLATE_ID: &str = "skill.template.circle_area";
pub const FANGYUAN_SKILL_CONE_TEMPLATE_ID: &str = "skill.template.cone";
pub const FANGYUAN_SKILL_SHIELD_TEMPLATE_ID: &str = "skill.template.shield";
const FANGYUAN_SKILL_RULE_ALPHA_MIN: f32 = 0.35;
const FANGYUAN_SKILL_RULE_RANGE_TOLERANCE: f32 = 0.05;
const FANGYUAN_SKILL_DECOR_BOUNDS_TOLERANCE: f32 = 0.15;
const FANGYUAN_SKILL_MAX_RULE_OCCLUSION: f32 = 0.25;
const FANGYUAN_SKILL_MAX_TRANSPARENT_PRIMITIVES: u16 = 12;
const FANGYUAN_SKILL_MAX_EMISSIVE_INTENSITY: f32 = 4.0;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillTemplate {
    pub id: String,
    pub version: u32,
    #[serde(default = "default_skill_schema_version")]
    pub schema_version: u32,
    pub rule_layer: FangyuanSkillRuleLayer,
    pub range_shape: FangyuanSkillRangeShape,
    #[serde(default)]
    pub direction: FangyuanSkillDirection,
    pub danger_boundary: FangyuanSkillDangerBoundary,
    pub timing: FangyuanSkillTiming,
    #[serde(default)]
    pub required_visible_elements: BTreeSet<FangyuanSkillVisibleElement>,
    #[serde(default)]
    pub authority_behavior: FangyuanSkillAuthorityBehavior,
    #[serde(default)]
    pub field_policy: FangyuanSkillFieldPolicy,
}

impl FangyuanSkillTemplate {
    pub fn validate(&self) -> Result<(), FangyuanSkillTemplateDiagnostic> {
        if self.id.trim().is_empty() {
            return Err(FangyuanSkillTemplateDiagnostic::new(
                FangyuanSkillTemplateDiagnosticCode::EmptyTemplateId,
                "skill template id must not be empty",
            ));
        }
        if self.version == 0 || self.schema_version != FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION {
            return Err(FangyuanSkillTemplateDiagnostic::new(
                FangyuanSkillTemplateDiagnosticCode::UnsupportedTemplateVersion,
                "skill template version and schema_version must match the supported schema",
            ));
        }
        self.range_shape.validate()?;
        self.danger_boundary.validate()?;
        self.timing.validate()?;
        if self.required_visible_elements.is_empty() {
            return Err(FangyuanSkillTemplateDiagnostic::new(
                FangyuanSkillTemplateDiagnosticCode::MissingRequiredVisibleElement,
                "skill template must declare at least one required visible element",
            ));
        }
        if !self
            .required_visible_elements
            .contains(&FangyuanSkillVisibleElement::DangerBoundary)
        {
            return Err(FangyuanSkillTemplateDiagnostic::new(
                FangyuanSkillTemplateDiagnosticCode::MissingRequiredVisibleElement,
                "danger boundary must be a required visible element",
            ));
        }
        Ok(())
    }

    pub fn permission_for_field(
        &self,
        field: FangyuanSkillTemplateField,
    ) -> FangyuanSkillFieldPermission {
        self.field_policy.permission_for_template_field(field)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillRuleLayer {
    Damage,
    Control,
    Defense,
    Movement,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanSkillRangeShape {
    Projectile {
        length: f32,
        radius: f32,
        speed: f32,
    },
    CircleArea {
        radius: f32,
    },
    Cone {
        radius: f32,
        angle_degrees: f32,
    },
    Shield {
        radius: f32,
        arc_degrees: f32,
    },
}

impl FangyuanSkillRangeShape {
    fn validate(&self) -> Result<(), FangyuanSkillTemplateDiagnostic> {
        match self {
            Self::Projectile {
                length,
                radius,
                speed,
            } => {
                validate_positive_finite(*length, "range_shape.length")?;
                validate_positive_finite(*radius, "range_shape.radius")?;
                validate_positive_finite(*speed, "range_shape.speed")
            }
            Self::CircleArea { radius } => validate_positive_finite(*radius, "range_shape.radius"),
            Self::Cone {
                radius,
                angle_degrees,
            } => {
                validate_positive_finite(*radius, "range_shape.radius")?;
                validate_angle(*angle_degrees, "range_shape.angle_degrees")
            }
            Self::Shield {
                radius,
                arc_degrees,
            } => {
                validate_positive_finite(*radius, "range_shape.radius")?;
                validate_angle(*arc_degrees, "range_shape.arc_degrees")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillDirection {
    #[default]
    CasterForward,
    TargetPoint,
    TargetEntity,
    None,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillDangerBoundary {
    pub warning_lead_ticks: u64,
    pub linger_ticks: u64,
    pub hard_edge: bool,
}

impl FangyuanSkillDangerBoundary {
    fn validate(&self) -> Result<(), FangyuanSkillTemplateDiagnostic> {
        if self.warning_lead_ticks == 0 {
            return Err(FangyuanSkillTemplateDiagnostic::with_field(
                FangyuanSkillTemplateDiagnosticCode::InvalidDangerBoundary,
                "danger boundary warning_lead_ticks must be greater than zero",
                "danger_boundary.warning_lead_ticks",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillTiming {
    pub cast_start_tick_offset: u64,
    pub impact_tick_offset: u64,
    pub recovery_ticks: u64,
}

impl FangyuanSkillTiming {
    fn validate(&self) -> Result<(), FangyuanSkillTemplateDiagnostic> {
        if self.impact_tick_offset <= self.cast_start_tick_offset {
            return Err(FangyuanSkillTemplateDiagnostic::with_field(
                FangyuanSkillTemplateDiagnosticCode::InvalidTiming,
                "impact_tick_offset must be after cast_start_tick_offset",
                "timing.impact_tick_offset",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillVisibleElement {
    DangerBoundary,
    CastDirection,
    CastOrigin,
    ImpactMarker,
    TravelPath,
    ShieldSurface,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillAuthorityBehavior {
    #[default]
    AuthorityConfirmed,
    LocalPredictionWithRollback,
    ServerOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillFieldPolicy {
    #[serde(default = "default_template_locked_fields")]
    pub system_locked_template_fields: BTreeSet<FangyuanSkillTemplateField>,
    #[serde(default = "default_visual_player_fields")]
    pub player_editable_visual_fields: BTreeSet<FangyuanSkillVisualField>,
    #[serde(default = "default_visual_degrade_fields")]
    pub audit_degrade_only_visual_fields: BTreeSet<FangyuanSkillVisualField>,
}

impl FangyuanSkillFieldPolicy {
    pub fn permission_for_template_field(
        &self,
        field: FangyuanSkillTemplateField,
    ) -> FangyuanSkillFieldPermission {
        if self.system_locked_template_fields.contains(&field) {
            FangyuanSkillFieldPermission::SystemLocked
        } else {
            FangyuanSkillFieldPermission::AuditDegradeOnly
        }
    }

    pub fn permission_for_visual_field(
        &self,
        field: FangyuanSkillVisualField,
    ) -> FangyuanSkillFieldPermission {
        if self.player_editable_visual_fields.contains(&field) {
            FangyuanSkillFieldPermission::PlayerEditable
        } else if self.audit_degrade_only_visual_fields.contains(&field) {
            FangyuanSkillFieldPermission::AuditDegradeOnly
        } else {
            FangyuanSkillFieldPermission::SystemLocked
        }
    }
}

impl Default for FangyuanSkillFieldPolicy {
    fn default() -> Self {
        Self {
            system_locked_template_fields: default_template_locked_fields(),
            player_editable_visual_fields: default_visual_player_fields(),
            audit_degrade_only_visual_fields: default_visual_degrade_fields(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillTemplateField {
    Id,
    Version,
    RuleLayer,
    RangeShape,
    Direction,
    DangerBoundary,
    Timing,
    RequiredVisibleElements,
    AuthorityBehavior,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillVisualField {
    Color,
    ProfileRef,
    Trail,
    Decor,
    ImpactResidue,
    Emissive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillFieldPermission {
    SystemLocked,
    PlayerEditable,
    AuditDegradeOnly,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillVisualBlueprint {
    pub id: String,
    pub template_id: String,
    pub template_version: u32,
    pub color: [f32; 4],
    #[serde(default)]
    pub readability: FangyuanSkillReadabilityMetadata,
    #[serde(default)]
    pub visual_range_hint: Option<FangyuanSkillVisualRangeHint>,
    #[serde(default)]
    pub profile_ref: Option<String>,
    #[serde(default)]
    pub vfx_recipe: Option<FangyuanVfxRecipe>,
    #[serde(default)]
    pub trail: FangyuanSkillTrailVisual,
    #[serde(default)]
    pub decor: FangyuanSkillDecorVisual,
    #[serde(default)]
    pub impact_residue: FangyuanSkillImpactResidueVisual,
    #[serde(default)]
    pub emissive: FangyuanSkillEmissiveVisual,
    #[serde(default)]
    pub equipment_socket_bindings: Vec<FangyuanSkillEquipmentSocketBinding>,
    #[serde(default)]
    pub attempted_rule_overrides: Vec<FangyuanSkillTemplateField>,
}

impl FangyuanSkillVisualBlueprint {
    pub fn validate(
        &self,
        templates: &FangyuanSkillTemplateRegistry,
    ) -> Result<(), FangyuanSkillVisualDiagnostic> {
        if self.id.trim().is_empty() {
            return Err(FangyuanSkillVisualDiagnostic::new(
                FangyuanSkillVisualDiagnosticCode::EmptyBlueprintId,
                "skill visual blueprint id must not be empty",
            ));
        }
        let template = templates
            .get(&self.template_id, self.template_version)
            .ok_or_else(|| {
                FangyuanSkillVisualDiagnostic::new(
                    FangyuanSkillVisualDiagnosticCode::InvalidTemplateReference,
                    "skill visual blueprint references an unknown template id/version",
                )
            })?;
        for field in &self.attempted_rule_overrides {
            if template.permission_for_field(*field) == FangyuanSkillFieldPermission::SystemLocked {
                return Err(FangyuanSkillVisualDiagnostic::with_field(
                    FangyuanSkillVisualDiagnosticCode::UnauthorizedRuleOverride,
                    "visual blueprint cannot override system locked template rule fields",
                    format!("attempted_rule_overrides.{field:?}"),
                ));
            }
        }
        validate_color(self.color, FangyuanSkillVisualField::Color)?;
        self.readability.validate()?;
        if let Some(range_hint) = &self.visual_range_hint {
            range_hint.validate()?;
        }
        self.trail.validate(&template.field_policy)?;
        self.decor.validate(&template.field_policy)?;
        self.impact_residue.validate(&template.field_policy)?;
        self.emissive.validate(&template.field_policy)?;
        for (index, binding) in self.equipment_socket_bindings.iter().enumerate() {
            binding.validate(index)?;
        }
        if let Some(recipe) = &self.vfx_recipe {
            recipe.validate().map_err(|error| {
                FangyuanSkillVisualDiagnostic::with_field(
                    FangyuanSkillVisualDiagnosticCode::InvalidVfxRecipe,
                    format!("visual blueprint VFX recipe is invalid: {}", error.message),
                    "vfx_recipe",
                )
            })?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillEquipmentSocketBinding {
    pub emitter_id: String,
    pub target: FangyuanSkillEquipmentSocketBindingTarget,
    pub socket: FangyuanEquipmentSocketSemantic,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<FangyuanEquipmentSocketFallback>,
}

impl FangyuanSkillEquipmentSocketBinding {
    pub fn emitter_origin(
        emitter_id: impl Into<String>,
        socket: FangyuanEquipmentSocketSemantic,
    ) -> Self {
        Self {
            emitter_id: emitter_id.into(),
            target: FangyuanSkillEquipmentSocketBindingTarget::EmitterOrigin,
            socket,
            fallback: Some(socket.default_fallback()),
        }
    }

    pub fn move_from(
        emitter_id: impl Into<String>,
        socket: FangyuanEquipmentSocketSemantic,
    ) -> Self {
        Self {
            emitter_id: emitter_id.into(),
            target: FangyuanSkillEquipmentSocketBindingTarget::MoveFrom,
            socket,
            fallback: Some(socket.default_fallback()),
        }
    }

    pub fn move_to(emitter_id: impl Into<String>, socket: FangyuanEquipmentSocketSemantic) -> Self {
        Self {
            emitter_id: emitter_id.into(),
            target: FangyuanSkillEquipmentSocketBindingTarget::MoveTo,
            socket,
            fallback: Some(socket.default_fallback()),
        }
    }

    pub fn decor_anchor(
        emitter_id: impl Into<String>,
        socket: FangyuanEquipmentSocketSemantic,
    ) -> Self {
        Self {
            emitter_id: emitter_id.into(),
            target: FangyuanSkillEquipmentSocketBindingTarget::DecorAnchor,
            socket,
            fallback: Some(socket.default_fallback()),
        }
    }

    pub fn with_fallback(mut self, fallback: FangyuanEquipmentSocketFallback) -> Self {
        self.fallback = Some(fallback);
        self
    }

    fn validate(&self, index: usize) -> Result<(), FangyuanSkillVisualDiagnostic> {
        if self.emitter_id.trim().is_empty() {
            return Err(FangyuanSkillVisualDiagnostic::with_field(
                FangyuanSkillVisualDiagnosticCode::InvalidEquipmentSocketBinding,
                "equipment socket binding emitter_id must not be empty",
                format!("equipment_socket_bindings[{index}].emitter_id"),
            ));
        }
        Ok(())
    }

    fn reference_kind(&self) -> FangyuanEquipmentSocketReferenceKind {
        self.target.reference_kind()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillEquipmentSocketBindingTarget {
    EmitterOrigin,
    MoveFrom,
    MoveTo,
    DecorAnchor,
}

impl FangyuanSkillEquipmentSocketBindingTarget {
    pub const fn reference_kind(self) -> FangyuanEquipmentSocketReferenceKind {
        match self {
            Self::EmitterOrigin => FangyuanEquipmentSocketReferenceKind::Emit,
            Self::MoveFrom | Self::MoveTo => FangyuanEquipmentSocketReferenceKind::Trajectory,
            Self::DecorAnchor => FangyuanEquipmentSocketReferenceKind::Decor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillReadabilityMetadata {
    pub rule_alpha: f32,
    pub rule_edge_width: f32,
    pub personality_occlusion: f32,
    pub decor_bounds_radius: f32,
    pub transparent_primitive_budget: u16,
}

impl FangyuanSkillReadabilityMetadata {
    fn validate(&self) -> Result<(), FangyuanSkillVisualDiagnostic> {
        validate_normalized(
            self.rule_alpha,
            "readability.rule_alpha",
            FangyuanSkillVisualDiagnosticCode::InvalidVisualValue,
        )?;
        validate_positive_visual(self.rule_edge_width, "readability.rule_edge_width")?;
        validate_normalized(
            self.personality_occlusion,
            "readability.personality_occlusion",
            FangyuanSkillVisualDiagnosticCode::InvalidVisualValue,
        )?;
        validate_positive_visual(self.decor_bounds_radius, "readability.decor_bounds_radius")?;
        Ok(())
    }
}

impl Default for FangyuanSkillReadabilityMetadata {
    fn default() -> Self {
        Self {
            rule_alpha: 0.75,
            rule_edge_width: 0.08,
            personality_occlusion: 0.1,
            decor_bounds_radius: 4.0,
            transparent_primitive_budget: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanSkillVisualRangeHint {
    Projectile { length: f32, radius: f32 },
    CircleArea { radius: f32 },
    Cone { radius: f32, angle_degrees: f32 },
    Shield { radius: f32, arc_degrees: f32 },
}

impl FangyuanSkillVisualRangeHint {
    fn validate(&self) -> Result<(), FangyuanSkillVisualDiagnostic> {
        match self {
            Self::Projectile { length, radius } => {
                validate_positive_visual(*length, "visual_range_hint.length")?;
                validate_positive_visual(*radius, "visual_range_hint.radius")
            }
            Self::CircleArea { radius } => {
                validate_positive_visual(*radius, "visual_range_hint.radius")
            }
            Self::Cone {
                radius,
                angle_degrees,
            } => {
                validate_positive_visual(*radius, "visual_range_hint.radius")?;
                validate_visual_angle(*angle_degrees, "visual_range_hint.angle_degrees")
            }
            Self::Shield {
                radius,
                arc_degrees,
            } => {
                validate_positive_visual(*radius, "visual_range_hint.radius")?;
                validate_visual_angle(*arc_degrees, "visual_range_hint.arc_degrees")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillTrailVisual {
    pub enabled: bool,
    pub segment_count: u16,
}

impl FangyuanSkillTrailVisual {
    fn validate(
        &self,
        policy: &FangyuanSkillFieldPolicy,
    ) -> Result<(), FangyuanSkillVisualDiagnostic> {
        if policy.permission_for_visual_field(FangyuanSkillVisualField::Trail)
            == FangyuanSkillFieldPermission::AuditDegradeOnly
            && self.segment_count > 48
        {
            return Err(FangyuanSkillVisualDiagnostic::with_field(
                FangyuanSkillVisualDiagnosticCode::AuditDegradeOnlyFieldExceeded,
                "trail segment count can only be raised beyond the budget by audit suggestion",
                "trail.segment_count",
            ));
        }
        Ok(())
    }
}

impl Default for FangyuanSkillTrailVisual {
    fn default() -> Self {
        Self {
            enabled: true,
            segment_count: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillDecorVisual {
    pub enabled: bool,
    pub max_pieces: u16,
}

impl FangyuanSkillDecorVisual {
    fn validate(
        &self,
        _policy: &FangyuanSkillFieldPolicy,
    ) -> Result<(), FangyuanSkillVisualDiagnostic> {
        Ok(())
    }
}

impl Default for FangyuanSkillDecorVisual {
    fn default() -> Self {
        Self {
            enabled: true,
            max_pieces: 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillImpactResidueVisual {
    pub enabled: bool,
    pub duration_ticks: u64,
}

impl FangyuanSkillImpactResidueVisual {
    fn validate(
        &self,
        _policy: &FangyuanSkillFieldPolicy,
    ) -> Result<(), FangyuanSkillVisualDiagnostic> {
        Ok(())
    }
}

impl Default for FangyuanSkillImpactResidueVisual {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ticks: 18,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSkillEmissiveVisual {
    pub intensity: f32,
}

impl FangyuanSkillEmissiveVisual {
    fn validate(
        &self,
        _policy: &FangyuanSkillFieldPolicy,
    ) -> Result<(), FangyuanSkillVisualDiagnostic> {
        if !self.intensity.is_finite() || self.intensity < 0.0 {
            return Err(FangyuanSkillVisualDiagnostic::with_field(
                FangyuanSkillVisualDiagnosticCode::InvalidVisualValue,
                "emissive intensity must be finite and non-negative",
                "emissive.intensity",
            ));
        }
        Ok(())
    }
}

impl Default for FangyuanSkillEmissiveVisual {
    fn default() -> Self {
        Self { intensity: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanSkillDegradeLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl FangyuanSkillDegradeLevel {
    fn personality_pressure(self) -> FangyuanVfxBudgetPressure {
        match self {
            Self::None => FangyuanVfxBudgetPressure::none(),
            Self::Low => FangyuanVfxBudgetPressure {
                max_primitives: Some(16),
                max_trail_segments: Some(8),
                skip_decoration: false,
            },
            Self::Medium => FangyuanVfxBudgetPressure::constrained(10, 4),
            Self::High => FangyuanVfxBudgetPressure::constrained(6, 1),
            Self::Critical => FangyuanVfxBudgetPressure::constrained(2, 0),
        }
    }

    const fn removes_residue(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanSkillAuditReport {
    pub diagnostics: Vec<FangyuanSkillAuditDiagnostic>,
}

impl FangyuanSkillAuditReport {
    pub fn passed(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn has_code(&self, code: FangyuanSkillAuditDiagnosticCode) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanSkillAuditDiagnostic {
    pub code: FangyuanSkillAuditDiagnosticCode,
    pub message: String,
    pub field_path: Option<String>,
}

impl FangyuanSkillAuditDiagnostic {
    fn new(code: FangyuanSkillAuditDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: None,
        }
    }

    fn with_field(
        code: FangyuanSkillAuditDiagnosticCode,
        message: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: Some(field_path.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanSkillAuditDiagnosticCode {
    VisualRangeMissing,
    VisualRangeTooSmall,
    VisualRangeMismatch,
    DecorBoundsExceeded,
    RuleLayerOccluded,
    ColorConflict,
    TransparentBudgetExceeded,
    EmissiveBudgetExceeded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSkillRuntimeContext {
    pub start_tick: u64,
    pub current_tick: u64,
    pub ticks_per_second: u32,
    pub caster_id: String,
    pub event_id: String,
    pub external_seed: Option<u64>,
    pub degrade_level: FangyuanSkillDegradeLevel,
    pub equipment_sockets: Option<FangyuanEquipmentSocketSet>,
}

impl FangyuanSkillRuntimeContext {
    pub fn local(
        start_tick: u64,
        current_tick: u64,
        ticks_per_second: u32,
        caster_id: impl Into<String>,
        event_id: impl Into<String>,
    ) -> Self {
        Self {
            start_tick,
            current_tick,
            ticks_per_second,
            caster_id: caster_id.into(),
            event_id: event_id.into(),
            external_seed: None,
            degrade_level: FangyuanSkillDegradeLevel::None,
            equipment_sockets: None,
        }
    }

    pub fn with_degrade_level(mut self, degrade_level: FangyuanSkillDegradeLevel) -> Self {
        self.degrade_level = degrade_level;
        self
    }

    pub fn with_equipment_sockets(mut self, sockets: FangyuanEquipmentSocketSet) -> Self {
        self.equipment_sockets = Some(sockets);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSkillRuntimePresentation {
    pub rule_layer_states: Vec<FangyuanVfxDynamicPrimitiveState>,
    pub personality_layer_states: Vec<FangyuanVfxDynamicPrimitiveState>,
    pub degrade_level: FangyuanSkillDegradeLevel,
    pub equipment_socket_bindings: Vec<FangyuanSkillEquipmentSocketRuntimeBinding>,
}

impl FangyuanSkillRuntimePresentation {
    pub fn playback_states(&self) -> Vec<FangyuanVfxDynamicPrimitiveState> {
        let mut states =
            Vec::with_capacity(self.rule_layer_states.len() + self.personality_layer_states.len());
        states.extend(self.rule_layer_states.iter().cloned());
        states.extend(self.personality_layer_states.iter().cloned());
        states
    }

    pub fn rule_layer_hash(&self) -> u64 {
        fangyuan_vfx_primitive_state_hash(&self.rule_layer_states)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSkillEquipmentSocketRuntimeBinding {
    pub emitter_id: String,
    pub target: FangyuanSkillEquipmentSocketBindingTarget,
    pub requested_socket: FangyuanEquipmentSocketSemantic,
    pub reference_kind: FangyuanEquipmentSocketReferenceKind,
    pub position: Vec3,
    pub fallback: Option<FangyuanEquipmentSocketFallbackDiagnostic>,
    pub applied_to_emitter: bool,
}

pub fn audit_fangyuan_skill_visual_readability(
    template: &FangyuanSkillTemplate,
    blueprint: &FangyuanSkillVisualBlueprint,
) -> FangyuanSkillAuditReport {
    let mut diagnostics = Vec::new();

    match &blueprint.visual_range_hint {
        Some(range_hint) => audit_visual_range_hint(template, range_hint, &mut diagnostics),
        None => diagnostics.push(FangyuanSkillAuditDiagnostic::new(
            FangyuanSkillAuditDiagnosticCode::VisualRangeMissing,
            "visual blueprint must declare a rule range hint for readability audit",
        )),
    }

    let rule_extent = skill_rule_extent(&template.range_shape);
    let decor_limit = rule_extent + FANGYUAN_SKILL_DECOR_BOUNDS_TOLERANCE;
    if blueprint.decor.enabled && blueprint.readability.decor_bounds_radius > decor_limit {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::DecorBoundsExceeded,
            "decor bounds exceed the authoritative skill range",
            "readability.decor_bounds_radius",
        ));
    }

    if blueprint.readability.rule_alpha < FANGYUAN_SKILL_RULE_ALPHA_MIN {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::RuleLayerOccluded,
            "rule layer alpha is too low to remain readable",
            "readability.rule_alpha",
        ));
    }

    if blueprint.readability.personality_occlusion > FANGYUAN_SKILL_MAX_RULE_OCCLUSION {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::RuleLayerOccluded,
            "personality layer occludes too much of the mandatory rule layer",
            "readability.personality_occlusion",
        ));
    }

    if color_conflicts_with_rule_layer(template.rule_layer, blueprint.color) {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::ColorConflict,
            "personality color conflicts with the skill danger/readability convention",
            "color",
        ));
    }

    if blueprint.readability.transparent_primitive_budget
        > FANGYUAN_SKILL_MAX_TRANSPARENT_PRIMITIVES
    {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::TransparentBudgetExceeded,
            "transparent primitive budget exceeds the skill readability budget",
            "readability.transparent_primitive_budget",
        ));
    }

    if blueprint.emissive.intensity > FANGYUAN_SKILL_MAX_EMISSIVE_INTENSITY {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::EmissiveBudgetExceeded,
            "emissive intensity exceeds the skill readability budget",
            "emissive.intensity",
        ));
    }

    FangyuanSkillAuditReport { diagnostics }
}

pub fn compile_fangyuan_skill_runtime_presentation(
    template: &FangyuanSkillTemplate,
    blueprint: &FangyuanSkillVisualBlueprint,
    context: &FangyuanSkillRuntimeContext,
) -> Result<FangyuanSkillRuntimePresentation, FangyuanVfxDiagnostic> {
    let clock = FangyuanVfxClock::new(
        context.start_tick,
        context.current_tick,
        context.ticks_per_second,
    );
    let replay_context = FangyuanVfxReplayContext {
        caster_id: context.caster_id.clone(),
        event_id: context.event_id.clone(),
        external_seed: context.external_seed,
    };

    let rule_recipe = compile_fangyuan_skill_rule_layer_recipe(template, blueprint);
    let rule_layer_states = evaluate_fangyuan_vfx_recipe_with_budget_pressure(
        &rule_recipe,
        clock,
        &replay_context,
        FangyuanVfxBudgetPressure::none(),
    )?;

    let mut personality_recipe =
        compile_fangyuan_skill_personality_layer_recipe(blueprint, context.degrade_level);
    let equipment_socket_bindings = apply_skill_equipment_socket_bindings(
        &mut personality_recipe,
        &blueprint.equipment_socket_bindings,
        context.equipment_sockets.as_ref(),
    );
    let personality_layer_states = evaluate_fangyuan_vfx_recipe_with_budget_pressure(
        &personality_recipe,
        clock,
        &replay_context,
        context.degrade_level.personality_pressure(),
    )?;

    Ok(FangyuanSkillRuntimePresentation {
        rule_layer_states: sort_skill_rule_layer_states(rule_layer_states),
        personality_layer_states,
        degrade_level: context.degrade_level,
        equipment_socket_bindings,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanSkillTemplateRegistry {
    templates: BTreeMap<(String, u32), FangyuanSkillTemplate>,
    fallback_template: Option<(String, u32)>,
}

impl FangyuanSkillTemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        for template in fangyuan_default_skill_templates() {
            registry
                .register(template)
                .expect("default Fangyuan skill templates must validate");
        }
        registry.set_fallback(
            FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID,
            FANGYUAN_SKILL_DEFAULT_TEMPLATE_VERSION,
        );
        registry
    }

    pub fn register(
        &mut self,
        template: FangyuanSkillTemplate,
    ) -> Result<(), FangyuanSkillTemplateRegistryError> {
        template
            .validate()
            .map_err(FangyuanSkillTemplateRegistryError::ValidationFailed)?;
        let key = (template.id.clone(), template.version);
        if self.templates.contains_key(&key) {
            return Err(FangyuanSkillTemplateRegistryError::DuplicateTemplate {
                id: key.0,
                version: key.1,
            });
        }
        self.templates.insert(key, template);
        Ok(())
    }

    pub fn get(&self, id: &str, version: u32) -> Option<&FangyuanSkillTemplate> {
        self.templates.get(&(id.to_string(), version))
    }

    pub fn resolve_or_fallback(
        &self,
        id: Option<&str>,
        version: Option<u32>,
    ) -> FangyuanSkillTemplateResolution<'_> {
        let requested = id.and_then(|id| version.map(|version| (id, version)));
        if let Some((id, version)) = requested {
            if let Some(template) = self.get(id, version) {
                return FangyuanSkillTemplateResolution {
                    template,
                    fallback_reason: None,
                };
            }
        }

        let (fallback_id, fallback_version) = self
            .fallback_template
            .as_ref()
            .expect("Fangyuan skill registry fallback must be configured");
        let template = self
            .get(fallback_id, *fallback_version)
            .expect("Fangyuan skill registry fallback must point to a registered template");
        FangyuanSkillTemplateResolution {
            template,
            fallback_reason: Some(match requested {
                Some((id, version)) => FangyuanSkillFallbackReason::UnknownTemplate {
                    id: id.to_string(),
                    version,
                },
                None => FangyuanSkillFallbackReason::MissingTemplateReference,
            }),
        }
    }

    pub fn set_fallback(&mut self, id: impl Into<String>, version: u32) {
        self.fallback_template = Some((id.into(), version));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSkillTemplateResolution<'a> {
    pub template: &'a FangyuanSkillTemplate,
    pub fallback_reason: Option<FangyuanSkillFallbackReason>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanSkillFallbackReason {
    MissingTemplateReference,
    UnknownTemplate { id: String, version: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanSkillTemplateRegistryError {
    ValidationFailed(FangyuanSkillTemplateDiagnostic),
    DuplicateTemplate { id: String, version: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanSkillTemplateDiagnostic {
    pub code: FangyuanSkillTemplateDiagnosticCode,
    pub message: String,
    pub field_path: Option<String>,
}

impl FangyuanSkillTemplateDiagnostic {
    pub fn new(code: FangyuanSkillTemplateDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: None,
        }
    }

    pub fn with_field(
        code: FangyuanSkillTemplateDiagnosticCode,
        message: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: Some(field_path.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanSkillTemplateDiagnosticCode {
    EmptyTemplateId,
    UnsupportedTemplateVersion,
    InvalidRange,
    InvalidDangerBoundary,
    InvalidTiming,
    MissingRequiredVisibleElement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSkillVisualDiagnostic {
    pub code: FangyuanSkillVisualDiagnosticCode,
    pub message: String,
    pub field_path: Option<String>,
}

impl FangyuanSkillVisualDiagnostic {
    pub fn new(code: FangyuanSkillVisualDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: None,
        }
    }

    pub fn with_field(
        code: FangyuanSkillVisualDiagnosticCode,
        message: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            field_path: Some(field_path.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanSkillVisualDiagnosticCode {
    EmptyBlueprintId,
    InvalidTemplateReference,
    UnauthorizedRuleOverride,
    InvalidVisualValue,
    AuditDegradeOnlyFieldExceeded,
    InvalidVfxRecipe,
    InvalidEquipmentSocketBinding,
}

pub fn fangyuan_default_skill_templates() -> Vec<FangyuanSkillTemplate> {
    vec![
        default_projectile_template(),
        default_circle_area_template(),
        default_cone_template(),
        default_shield_template(),
    ]
}

pub fn fangyuan_default_skill_visual_blueprints() -> Vec<FangyuanSkillVisualBlueprint> {
    vec![
        default_projectile_visual_blueprint(),
        default_circle_area_visual_blueprint(),
        default_cone_visual_blueprint(),
        default_shield_visual_blueprint(),
    ]
}

fn default_projectile_template() -> FangyuanSkillTemplate {
    default_template(
        FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID,
        FangyuanSkillRuleLayer::Damage,
        FangyuanSkillRangeShape::Projectile {
            length: 8.0,
            radius: 0.35,
            speed: 12.0,
        },
        FangyuanSkillDirection::TargetPoint,
        vec![
            FangyuanSkillVisibleElement::DangerBoundary,
            FangyuanSkillVisibleElement::CastDirection,
            FangyuanSkillVisibleElement::TravelPath,
            FangyuanSkillVisibleElement::ImpactMarker,
        ],
    )
}

fn default_circle_area_template() -> FangyuanSkillTemplate {
    default_template(
        FANGYUAN_SKILL_CIRCLE_AREA_TEMPLATE_ID,
        FangyuanSkillRuleLayer::Damage,
        FangyuanSkillRangeShape::CircleArea { radius: 3.5 },
        FangyuanSkillDirection::TargetPoint,
        vec![
            FangyuanSkillVisibleElement::DangerBoundary,
            FangyuanSkillVisibleElement::CastOrigin,
            FangyuanSkillVisibleElement::ImpactMarker,
        ],
    )
}

fn default_cone_template() -> FangyuanSkillTemplate {
    default_template(
        FANGYUAN_SKILL_CONE_TEMPLATE_ID,
        FangyuanSkillRuleLayer::Control,
        FangyuanSkillRangeShape::Cone {
            radius: 5.0,
            angle_degrees: 80.0,
        },
        FangyuanSkillDirection::CasterForward,
        vec![
            FangyuanSkillVisibleElement::DangerBoundary,
            FangyuanSkillVisibleElement::CastDirection,
            FangyuanSkillVisibleElement::ImpactMarker,
        ],
    )
}

fn default_shield_template() -> FangyuanSkillTemplate {
    default_template(
        FANGYUAN_SKILL_SHIELD_TEMPLATE_ID,
        FangyuanSkillRuleLayer::Defense,
        FangyuanSkillRangeShape::Shield {
            radius: 2.0,
            arc_degrees: 180.0,
        },
        FangyuanSkillDirection::CasterForward,
        vec![
            FangyuanSkillVisibleElement::DangerBoundary,
            FangyuanSkillVisibleElement::ShieldSurface,
        ],
    )
}

fn default_template(
    id: &str,
    rule_layer: FangyuanSkillRuleLayer,
    range_shape: FangyuanSkillRangeShape,
    direction: FangyuanSkillDirection,
    required_visible_elements: Vec<FangyuanSkillVisibleElement>,
) -> FangyuanSkillTemplate {
    FangyuanSkillTemplate {
        id: id.to_string(),
        version: FANGYUAN_SKILL_DEFAULT_TEMPLATE_VERSION,
        schema_version: FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION,
        rule_layer,
        range_shape,
        direction,
        danger_boundary: FangyuanSkillDangerBoundary {
            warning_lead_ticks: 8,
            linger_ticks: 6,
            hard_edge: true,
        },
        timing: FangyuanSkillTiming {
            cast_start_tick_offset: 0,
            impact_tick_offset: 18,
            recovery_ticks: 12,
        },
        required_visible_elements: required_visible_elements.into_iter().collect(),
        authority_behavior: FangyuanSkillAuthorityBehavior::AuthorityConfirmed,
        field_policy: FangyuanSkillFieldPolicy::default(),
    }
}

fn default_projectile_visual_blueprint() -> FangyuanSkillVisualBlueprint {
    default_visual_blueprint(
        "skill.visual.projectile",
        FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID,
        [0.95, 0.32, 0.18, 1.0],
        Some(fangyuan_vfx_projectile_recipe()),
    )
}

fn default_circle_area_visual_blueprint() -> FangyuanSkillVisualBlueprint {
    default_visual_blueprint(
        "skill.visual.circle_area",
        FANGYUAN_SKILL_CIRCLE_AREA_TEMPLATE_ID,
        [0.25, 0.75, 1.0, 0.75],
        Some(fangyuan_vfx_range_marker_recipe()),
    )
}

fn default_cone_visual_blueprint() -> FangyuanSkillVisualBlueprint {
    default_visual_blueprint(
        "skill.visual.cone",
        FANGYUAN_SKILL_CONE_TEMPLATE_ID,
        [0.9, 0.75, 0.2, 0.85],
        Some(fangyuan_vfx_impact_expand_recipe()),
    )
}

fn default_shield_visual_blueprint() -> FangyuanSkillVisualBlueprint {
    default_visual_blueprint(
        "skill.visual.shield",
        FANGYUAN_SKILL_SHIELD_TEMPLATE_ID,
        [0.3, 0.7, 1.0, 0.5],
        Some(fangyuan_vfx_shield_recipe()),
    )
}

fn default_visual_blueprint(
    id: &str,
    template_id: &str,
    color: [f32; 4],
    vfx_recipe: Option<FangyuanVfxRecipe>,
) -> FangyuanSkillVisualBlueprint {
    FangyuanSkillVisualBlueprint {
        id: id.to_string(),
        template_id: template_id.to_string(),
        template_version: FANGYUAN_SKILL_DEFAULT_TEMPLATE_VERSION,
        color,
        readability: FangyuanSkillReadabilityMetadata {
            decor_bounds_radius: default_visual_range_hint(template_id)
                .as_ref()
                .map(visual_range_hint_extent)
                .unwrap_or(4.0),
            ..Default::default()
        },
        visual_range_hint: default_visual_range_hint(template_id),
        profile_ref: Some("vfx/default".to_string()),
        vfx_recipe,
        trail: FangyuanSkillTrailVisual::default(),
        decor: FangyuanSkillDecorVisual::default(),
        impact_residue: FangyuanSkillImpactResidueVisual::default(),
        emissive: FangyuanSkillEmissiveVisual::default(),
        equipment_socket_bindings: Vec::new(),
        attempted_rule_overrides: Vec::new(),
    }
}

fn default_skill_schema_version() -> u32 {
    FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION
}

fn default_template_locked_fields() -> BTreeSet<FangyuanSkillTemplateField> {
    [
        FangyuanSkillTemplateField::Id,
        FangyuanSkillTemplateField::Version,
        FangyuanSkillTemplateField::RuleLayer,
        FangyuanSkillTemplateField::RangeShape,
        FangyuanSkillTemplateField::Direction,
        FangyuanSkillTemplateField::DangerBoundary,
        FangyuanSkillTemplateField::Timing,
        FangyuanSkillTemplateField::RequiredVisibleElements,
        FangyuanSkillTemplateField::AuthorityBehavior,
    ]
    .into_iter()
    .collect()
}

fn default_visual_player_fields() -> BTreeSet<FangyuanSkillVisualField> {
    [
        FangyuanSkillVisualField::Color,
        FangyuanSkillVisualField::ProfileRef,
        FangyuanSkillVisualField::Decor,
        FangyuanSkillVisualField::Emissive,
    ]
    .into_iter()
    .collect()
}

fn default_visual_degrade_fields() -> BTreeSet<FangyuanSkillVisualField> {
    [
        FangyuanSkillVisualField::Trail,
        FangyuanSkillVisualField::ImpactResidue,
    ]
    .into_iter()
    .collect()
}

fn validate_positive_finite(
    value: f32,
    field_path: &'static str,
) -> Result<(), FangyuanSkillTemplateDiagnostic> {
    if !value.is_finite() || value <= 0.0 {
        return Err(FangyuanSkillTemplateDiagnostic::with_field(
            FangyuanSkillTemplateDiagnosticCode::InvalidRange,
            "range value must be finite and greater than zero",
            field_path,
        ));
    }
    Ok(())
}

fn validate_angle(
    value: f32,
    field_path: &'static str,
) -> Result<(), FangyuanSkillTemplateDiagnostic> {
    if !value.is_finite() || value <= 0.0 || value > 360.0 {
        return Err(FangyuanSkillTemplateDiagnostic::with_field(
            FangyuanSkillTemplateDiagnosticCode::InvalidRange,
            "angle must be finite and in the range (0, 360]",
            field_path,
        ));
    }
    Ok(())
}

fn validate_color(
    color: [f32; 4],
    field: FangyuanSkillVisualField,
) -> Result<(), FangyuanSkillVisualDiagnostic> {
    for (channel, value) in color.into_iter().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(FangyuanSkillVisualDiagnostic::with_field(
                FangyuanSkillVisualDiagnosticCode::InvalidVisualValue,
                "color channels must be finite and normalized",
                format!("{field:?}.{channel}"),
            ));
        }
    }
    Ok(())
}

fn validate_normalized(
    value: f32,
    field_path: &'static str,
    code: FangyuanSkillVisualDiagnosticCode,
) -> Result<(), FangyuanSkillVisualDiagnostic> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(FangyuanSkillVisualDiagnostic::with_field(
            code,
            "value must be finite and normalized",
            field_path,
        ));
    }
    Ok(())
}

fn validate_positive_visual(
    value: f32,
    field_path: &'static str,
) -> Result<(), FangyuanSkillVisualDiagnostic> {
    if !value.is_finite() || value <= 0.0 {
        return Err(FangyuanSkillVisualDiagnostic::with_field(
            FangyuanSkillVisualDiagnosticCode::InvalidVisualValue,
            "visual value must be finite and greater than zero",
            field_path,
        ));
    }
    Ok(())
}

fn validate_visual_angle(
    value: f32,
    field_path: &'static str,
) -> Result<(), FangyuanSkillVisualDiagnostic> {
    if !value.is_finite() || value <= 0.0 || value > 360.0 {
        return Err(FangyuanSkillVisualDiagnostic::with_field(
            FangyuanSkillVisualDiagnosticCode::InvalidVisualValue,
            "visual angle must be finite and in the range (0, 360]",
            field_path,
        ));
    }
    Ok(())
}

fn default_visual_range_hint(template_id: &str) -> Option<FangyuanSkillVisualRangeHint> {
    match template_id {
        FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID => Some(FangyuanSkillVisualRangeHint::Projectile {
            length: 8.0,
            radius: 0.35,
        }),
        FANGYUAN_SKILL_CIRCLE_AREA_TEMPLATE_ID => {
            Some(FangyuanSkillVisualRangeHint::CircleArea { radius: 3.5 })
        }
        FANGYUAN_SKILL_CONE_TEMPLATE_ID => Some(FangyuanSkillVisualRangeHint::Cone {
            radius: 5.0,
            angle_degrees: 80.0,
        }),
        FANGYUAN_SKILL_SHIELD_TEMPLATE_ID => Some(FangyuanSkillVisualRangeHint::Shield {
            radius: 2.0,
            arc_degrees: 180.0,
        }),
        _ => None,
    }
}

fn audit_visual_range_hint(
    template: &FangyuanSkillTemplate,
    range_hint: &FangyuanSkillVisualRangeHint,
    diagnostics: &mut Vec<FangyuanSkillAuditDiagnostic>,
) {
    let mismatch = || {
        FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::VisualRangeMismatch,
            "visual range hint shape does not match the authoritative skill rule shape",
            "visual_range_hint",
        )
    };

    match (&template.range_shape, range_hint) {
        (
            FangyuanSkillRangeShape::Projectile {
                length,
                radius,
                speed: _,
            },
            FangyuanSkillVisualRangeHint::Projectile {
                length: visual_length,
                radius: visual_radius,
            },
        ) => {
            audit_range_floor(
                *visual_length,
                *length,
                "visual_range_hint.length",
                diagnostics,
            );
            audit_range_floor(
                *visual_radius,
                *radius,
                "visual_range_hint.radius",
                diagnostics,
            );
        }
        (
            FangyuanSkillRangeShape::CircleArea { radius },
            FangyuanSkillVisualRangeHint::CircleArea {
                radius: visual_radius,
            },
        ) => audit_range_floor(
            *visual_radius,
            *radius,
            "visual_range_hint.radius",
            diagnostics,
        ),
        (
            FangyuanSkillRangeShape::Cone {
                radius,
                angle_degrees,
            },
            FangyuanSkillVisualRangeHint::Cone {
                radius: visual_radius,
                angle_degrees: visual_angle,
            },
        ) => {
            audit_range_floor(
                *visual_radius,
                *radius,
                "visual_range_hint.radius",
                diagnostics,
            );
            audit_range_floor(
                *visual_angle,
                *angle_degrees,
                "visual_range_hint.angle_degrees",
                diagnostics,
            );
        }
        (
            FangyuanSkillRangeShape::Shield {
                radius,
                arc_degrees,
            },
            FangyuanSkillVisualRangeHint::Shield {
                radius: visual_radius,
                arc_degrees: visual_arc,
            },
        ) => {
            audit_range_floor(
                *visual_radius,
                *radius,
                "visual_range_hint.radius",
                diagnostics,
            );
            audit_range_floor(
                *visual_arc,
                *arc_degrees,
                "visual_range_hint.arc_degrees",
                diagnostics,
            );
        }
        _ => diagnostics.push(mismatch()),
    }
}

fn audit_range_floor(
    visual_value: f32,
    rule_value: f32,
    field_path: &'static str,
    diagnostics: &mut Vec<FangyuanSkillAuditDiagnostic>,
) {
    if visual_value + FANGYUAN_SKILL_RULE_RANGE_TOLERANCE < rule_value {
        diagnostics.push(FangyuanSkillAuditDiagnostic::with_field(
            FangyuanSkillAuditDiagnosticCode::VisualRangeTooSmall,
            "visual range hint is smaller than the authoritative rule range",
            field_path,
        ));
    }
}

fn skill_rule_extent(range_shape: &FangyuanSkillRangeShape) -> f32 {
    match range_shape {
        FangyuanSkillRangeShape::Projectile { length, radius, .. } => *length + *radius,
        FangyuanSkillRangeShape::CircleArea { radius }
        | FangyuanSkillRangeShape::Cone { radius, .. }
        | FangyuanSkillRangeShape::Shield { radius, .. } => *radius,
    }
}

fn visual_range_hint_extent(range_hint: &FangyuanSkillVisualRangeHint) -> f32 {
    match range_hint {
        FangyuanSkillVisualRangeHint::Projectile { length, radius } => *length + *radius,
        FangyuanSkillVisualRangeHint::CircleArea { radius }
        | FangyuanSkillVisualRangeHint::Cone { radius, .. }
        | FangyuanSkillVisualRangeHint::Shield { radius, .. } => *radius,
    }
}

fn color_conflicts_with_rule_layer(rule_layer: FangyuanSkillRuleLayer, color: [f32; 4]) -> bool {
    let [red, green, blue, alpha] = color;
    match rule_layer {
        FangyuanSkillRuleLayer::Damage => blue > red && blue > 0.55 && alpha > 0.5,
        FangyuanSkillRuleLayer::Control => red > 0.8 && green < 0.45 && alpha > 0.5,
        FangyuanSkillRuleLayer::Defense => red > 0.85 && green < 0.5 && blue < 0.5,
        FangyuanSkillRuleLayer::Movement => red > 0.85 && blue < 0.45,
    }
}

fn compile_fangyuan_skill_rule_layer_recipe(
    template: &FangyuanSkillTemplate,
    blueprint: &FangyuanSkillVisualBlueprint,
) -> FangyuanVfxRecipe {
    let duration_ticks = template
        .timing
        .impact_tick_offset
        .saturating_add(template.danger_boundary.linger_ticks)
        .max(1);
    let mut emitters = Vec::new();
    emitters.push(rule_emitter(
        "rule_core",
        FangyuanPrimitiveKind::Sphere,
        FangyuanPrimitiveRole::Core,
        [0.18, 0.18, 0.18],
        [1.0, 1.0, 1.0, 1.0],
        0,
        duration_ticks,
    ));
    emitters.push(rule_emitter(
        "rule_boundary",
        FangyuanPrimitiveKind::Cube,
        FangyuanPrimitiveRole::Boundary,
        boundary_scale(&template.range_shape, blueprint.readability.rule_edge_width),
        rule_color(template.rule_layer, blueprint.readability.rule_alpha),
        0,
        duration_ticks,
    ));
    emitters.push(rule_emitter(
        "rule_warning",
        FangyuanPrimitiveKind::Cube,
        FangyuanPrimitiveRole::Warning,
        warning_scale(&template.range_shape),
        warning_color(template.rule_layer),
        0,
        template.timing.impact_tick_offset,
    ));
    emitters.push(rule_emitter(
        "rule_impact",
        FangyuanPrimitiveKind::Sphere,
        FangyuanPrimitiveRole::Impact,
        impact_scale(&template.range_shape),
        [1.0, 0.95, 0.55, 0.9],
        template.timing.impact_tick_offset,
        template.danger_boundary.linger_ticks.max(1),
    ));

    FangyuanVfxRecipe {
        id: format!("{}.rule_layer", template.id),
        version: template.version,
        duration_ticks,
        seed_policy: Default::default(),
        emitters,
        curves: Vec::new(),
        budget_hints: Default::default(),
    }
}

fn compile_fangyuan_skill_personality_layer_recipe(
    blueprint: &FangyuanSkillVisualBlueprint,
    degrade_level: FangyuanSkillDegradeLevel,
) -> FangyuanVfxRecipe {
    let mut recipe = blueprint
        .vfx_recipe
        .clone()
        .unwrap_or_else(fangyuan_vfx_impact_expand_recipe);
    recipe.id = format!("{}.personality_layer", blueprint.id);
    for emitter in &mut recipe.emitters {
        emitter.color = blueprint.color;
        emitter.emissive = match degrade_level {
            FangyuanSkillDegradeLevel::None | FangyuanSkillDegradeLevel::Low => {
                blueprint.emissive.intensity
            }
            FangyuanSkillDegradeLevel::Medium => blueprint.emissive.intensity.min(1.0),
            FangyuanSkillDegradeLevel::High | FangyuanSkillDegradeLevel::Critical => 0.0,
        };
        if degrade_level >= FangyuanSkillDegradeLevel::Medium {
            for operator in &mut emitter.operators {
                if let FangyuanVfxOperator::Fade { from, to, .. } = operator {
                    *from = from.max(0.75);
                    *to = to.max(0.75);
                }
            }
        }
    }

    if blueprint.decor.enabled && degrade_level < FangyuanSkillDegradeLevel::Medium {
        recipe.emitters.push(personality_emitter(
            "decor",
            FangyuanPrimitiveRole::Decoration,
            [0.18, 0.18, 0.18],
            blueprint.color,
            blueprint.emissive.intensity * 0.5,
        ));
    }
    if blueprint.impact_residue.enabled && !degrade_level.removes_residue() {
        let mut residue = personality_emitter(
            "impact_residue",
            FangyuanPrimitiveRole::Decoration,
            [0.35, 0.04, 0.35],
            [
                blueprint.color[0],
                blueprint.color[1],
                blueprint.color[2],
                blueprint.color[3].min(0.45),
            ],
            0.0,
        );
        residue.delay_ticks = 1;
        residue.duration_ticks = Some(blueprint.impact_residue.duration_ticks.max(1));
        recipe.emitters.push(residue);
    }
    if blueprint.trail.enabled && degrade_level < FangyuanSkillDegradeLevel::Critical {
        let max_segments = match degrade_level {
            FangyuanSkillDegradeLevel::None | FangyuanSkillDegradeLevel::Low => {
                blueprint.trail.segment_count
            }
            FangyuanSkillDegradeLevel::Medium => blueprint.trail.segment_count.min(4),
            FangyuanSkillDegradeLevel::High => 1,
            FangyuanSkillDegradeLevel::Critical => 0,
        };
        if max_segments > 0 {
            for emitter in &mut recipe.emitters {
                if !emitter
                    .operators
                    .iter()
                    .any(|operator| matches!(operator, FangyuanVfxOperator::Trail { .. }))
                {
                    emitter.operators.push(FangyuanVfxOperator::Trail {
                        segments: max_segments,
                        spacing_ticks: 2,
                        fade: 0.55,
                    });
                    break;
                }
            }
        }
    }

    recipe.budget_hints.max_primitives = recipe.budget_hints.max_primitives.max(32);
    recipe.budget_hints.max_trail_segments = recipe.budget_hints.max_trail_segments.max(16);
    recipe
}

fn apply_skill_equipment_socket_bindings(
    recipe: &mut FangyuanVfxRecipe,
    bindings: &[FangyuanSkillEquipmentSocketBinding],
    equipment_sockets: Option<&FangyuanEquipmentSocketSet>,
) -> Vec<FangyuanSkillEquipmentSocketRuntimeBinding> {
    bindings
        .iter()
        .map(|binding| {
            let resolution = resolve_skill_equipment_socket_binding(binding, equipment_sockets);
            let applied_to_emitter =
                apply_skill_equipment_socket_resolution(recipe, binding, resolution.position);
            FangyuanSkillEquipmentSocketRuntimeBinding {
                emitter_id: binding.emitter_id.clone(),
                target: binding.target,
                requested_socket: binding.socket,
                reference_kind: binding.reference_kind(),
                position: resolution.position,
                fallback: resolution.fallback,
                applied_to_emitter,
            }
        })
        .collect()
}

fn resolve_skill_equipment_socket_binding(
    binding: &FangyuanSkillEquipmentSocketBinding,
    equipment_sockets: Option<&FangyuanEquipmentSocketSet>,
) -> FangyuanEquipmentSocketResolution {
    match equipment_sockets {
        Some(sockets) => sockets.resolve_with_fallback(
            binding.socket,
            binding.reference_kind(),
            binding.fallback.as_ref(),
        ),
        None => {
            let empty = FangyuanEquipmentSocketSet::new();
            empty.resolve_with_fallback(
                binding.socket,
                binding.reference_kind(),
                binding.fallback.as_ref(),
            )
        }
    }
}

fn apply_skill_equipment_socket_resolution(
    recipe: &mut FangyuanVfxRecipe,
    binding: &FangyuanSkillEquipmentSocketBinding,
    position: Vec3,
) -> bool {
    let Some(emitter) = recipe
        .emitters
        .iter_mut()
        .find(|emitter| emitter.id == binding.emitter_id)
    else {
        return false;
    };

    let position = position.to_array();
    match binding.target {
        FangyuanSkillEquipmentSocketBindingTarget::EmitterOrigin
        | FangyuanSkillEquipmentSocketBindingTarget::DecorAnchor => {
            emitter.position = position;
            true
        }
        FangyuanSkillEquipmentSocketBindingTarget::MoveFrom
        | FangyuanSkillEquipmentSocketBindingTarget::MoveTo => {
            let mut applied = false;
            for operator in &mut emitter.operators {
                if let FangyuanVfxOperator::Move { from, to, .. } = operator {
                    match binding.target {
                        FangyuanSkillEquipmentSocketBindingTarget::MoveFrom => *from = position,
                        FangyuanSkillEquipmentSocketBindingTarget::MoveTo => *to = position,
                        FangyuanSkillEquipmentSocketBindingTarget::EmitterOrigin
                        | FangyuanSkillEquipmentSocketBindingTarget::DecorAnchor => {}
                    }
                    applied = true;
                }
            }
            applied
        }
    }
}

fn rule_emitter(
    id: &str,
    primitive_kind: FangyuanPrimitiveKind,
    role: FangyuanPrimitiveRole,
    scale: [f32; 3],
    color: [f32; 4],
    delay_ticks: u64,
    duration_ticks: u64,
) -> FangyuanVfxEmitter {
    FangyuanVfxEmitter {
        id: id.to_string(),
        primitive_kind,
        role,
        delay_ticks,
        duration_ticks: Some(duration_ticks),
        position: [0.0, 0.0, 0.0],
        scale,
        color,
        emissive: if matches!(
            role,
            FangyuanPrimitiveRole::Warning | FangyuanPrimitiveRole::Impact
        ) {
            0.2
        } else {
            0.0
        },
        material_profile_id: Some("skill/rule".to_string()),
        jitter: FangyuanVfxEmitterJitter::default(),
        operators: vec![FangyuanVfxOperator::Spawn {
            curve: FangyuanVfxCurve::Linear,
        }],
    }
}

fn personality_emitter(
    id: &str,
    role: FangyuanPrimitiveRole,
    scale: [f32; 3],
    color: [f32; 4],
    emissive: f32,
) -> FangyuanVfxEmitter {
    FangyuanVfxEmitter {
        id: id.to_string(),
        primitive_kind: FangyuanPrimitiveKind::Sphere,
        role,
        delay_ticks: 0,
        duration_ticks: Some(24),
        position: [0.0, 0.0, 0.0],
        scale,
        color,
        emissive,
        material_profile_id: Some("skill/personality".to_string()),
        jitter: FangyuanVfxEmitterJitter::default(),
        operators: vec![FangyuanVfxOperator::Spawn {
            curve: FangyuanVfxCurve::EaseOut,
        }],
    }
}

fn sort_skill_rule_layer_states(
    mut states: Vec<FangyuanVfxDynamicPrimitiveState>,
) -> Vec<FangyuanVfxDynamicPrimitiveState> {
    states.sort_by_key(|state| {
        (
            skill_rule_role_order(state.role),
            state.emitter_index,
            state.primitive_index,
            state.source_tick,
        )
    });
    states
}

fn skill_rule_role_order(role: FangyuanPrimitiveRole) -> u8 {
    match role {
        FangyuanPrimitiveRole::Core => 0,
        FangyuanPrimitiveRole::Boundary => 1,
        FangyuanPrimitiveRole::Warning => 2,
        FangyuanPrimitiveRole::Impact => 3,
        _ => 4,
    }
}

fn boundary_scale(range_shape: &FangyuanSkillRangeShape, edge_width: f32) -> [f32; 3] {
    match range_shape {
        FangyuanSkillRangeShape::Projectile { length, radius, .. } => {
            [*length, edge_width, (*radius * 2.0).max(edge_width)]
        }
        FangyuanSkillRangeShape::CircleArea { radius }
        | FangyuanSkillRangeShape::Cone { radius, .. }
        | FangyuanSkillRangeShape::Shield { radius, .. } => {
            [*radius * 2.0, edge_width, *radius * 2.0]
        }
    }
}

fn warning_scale(range_shape: &FangyuanSkillRangeShape) -> [f32; 3] {
    let extent = skill_rule_extent(range_shape);
    [extent.max(0.1), 0.03, extent.max(0.1)]
}

fn impact_scale(range_shape: &FangyuanSkillRangeShape) -> [f32; 3] {
    let extent = (skill_rule_extent(range_shape) * 0.2).clamp(0.25, 1.5);
    [extent, extent, extent]
}

fn rule_color(rule_layer: FangyuanSkillRuleLayer, alpha: f32) -> [f32; 4] {
    match rule_layer {
        FangyuanSkillRuleLayer::Damage => [1.0, 0.18, 0.1, alpha],
        FangyuanSkillRuleLayer::Control => [0.95, 0.75, 0.15, alpha],
        FangyuanSkillRuleLayer::Defense => [0.2, 0.75, 1.0, alpha],
        FangyuanSkillRuleLayer::Movement => [0.35, 1.0, 0.45, alpha],
    }
}

fn warning_color(rule_layer: FangyuanSkillRuleLayer) -> [f32; 4] {
    let mut color = rule_color(rule_layer, 0.55);
    color[0] = (color[0] + 1.0) * 0.5;
    color[1] = (color[1] + 1.0) * 0.5;
    color[2] = (color[2] + 1.0) * 0.5;
    color
}

#[cfg(test)]
mod tests {
    use crate::framework::fangyuan::{
        FangyuanEquipmentSocketFallbackReason, fangyuan_default_equipment_blueprint,
    };

    use super::*;

    fn valid_projectile_template() -> FangyuanSkillTemplate {
        fangyuan_default_skill_templates()
            .into_iter()
            .find(|template| template.id == FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID)
            .unwrap()
    }

    #[test]
    fn fangyuan_skill_template_defaults_cover_projectile_circle_cone_and_shield() {
        let templates = fangyuan_default_skill_templates();
        assert_eq!(templates.len(), 4);
        for template in &templates {
            template.validate().unwrap();
        }
        assert!(templates.iter().any(|template| matches!(
            template.range_shape,
            FangyuanSkillRangeShape::Projectile { .. }
        )));
        assert!(templates.iter().any(|template| matches!(
            template.range_shape,
            FangyuanSkillRangeShape::CircleArea { .. }
        )));
        assert!(
            templates.iter().any(|template| matches!(
                template.range_shape,
                FangyuanSkillRangeShape::Cone { .. }
            ))
        );
        assert!(templates.iter().any(|template| matches!(
            template.range_shape,
            FangyuanSkillRangeShape::Shield { .. }
        )));
    }

    #[test]
    fn fangyuan_skill_template_rejects_unsupported_version() {
        let mut template = valid_projectile_template();
        template.schema_version = 99;

        let error = template.validate().unwrap_err();

        assert_eq!(
            error.code,
            FangyuanSkillTemplateDiagnosticCode::UnsupportedTemplateVersion
        );
    }

    #[test]
    fn fangyuan_skill_template_rejects_invalid_range() {
        let mut template = valid_projectile_template();
        template.range_shape = FangyuanSkillRangeShape::Cone {
            radius: 4.0,
            angle_degrees: 720.0,
        };

        let error = template.validate().unwrap_err();

        assert_eq!(
            error.code,
            FangyuanSkillTemplateDiagnosticCode::InvalidRange
        );
        assert_eq!(
            error.field_path.as_deref(),
            Some("range_shape.angle_degrees")
        );
    }

    #[test]
    fn fangyuan_skill_template_rejects_missing_required_visible_elements() {
        let mut template = valid_projectile_template();
        template
            .required_visible_elements
            .remove(&FangyuanSkillVisibleElement::DangerBoundary);

        let error = template.validate().unwrap_err();

        assert_eq!(
            error.code,
            FangyuanSkillTemplateDiagnosticCode::MissingRequiredVisibleElement
        );
    }

    #[test]
    fn fangyuan_skill_template_registry_resolves_missing_reference_to_fallback() {
        let registry = FangyuanSkillTemplateRegistry::with_defaults();

        let resolution = registry.resolve_or_fallback(None, None);

        assert_eq!(
            resolution.template.id,
            FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID
        );
        assert_eq!(
            resolution.fallback_reason,
            Some(FangyuanSkillFallbackReason::MissingTemplateReference)
        );
    }

    #[test]
    fn fangyuan_skill_visual_defaults_validate_against_default_templates() {
        let registry = FangyuanSkillTemplateRegistry::with_defaults();
        let blueprints = fangyuan_default_skill_visual_blueprints();

        assert_eq!(blueprints.len(), 4);
        for blueprint in &blueprints {
            blueprint.validate(&registry).unwrap();
            assert!(blueprint.vfx_recipe.is_some());
        }
    }

    #[test]
    fn fangyuan_skill_visual_rejects_invalid_template_reference() {
        let registry = FangyuanSkillTemplateRegistry::with_defaults();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.template_id = "missing.template".to_string();

        let error = blueprint.validate(&registry).unwrap_err();

        assert_eq!(
            error.code,
            FangyuanSkillVisualDiagnosticCode::InvalidTemplateReference
        );
    }

    #[test]
    fn fangyuan_skill_visual_rejects_unauthorized_rule_override() {
        let registry = FangyuanSkillTemplateRegistry::with_defaults();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint
            .attempted_rule_overrides
            .push(FangyuanSkillTemplateField::RangeShape);

        let error = blueprint.validate(&registry).unwrap_err();

        assert_eq!(
            error.code,
            FangyuanSkillVisualDiagnosticCode::UnauthorizedRuleOverride
        );
    }

    #[test]
    fn fangyuan_skill_visual_reports_audit_degrade_only_field_exceeded() {
        let registry = FangyuanSkillTemplateRegistry::with_defaults();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.trail.segment_count = 64;

        let error = blueprint.validate(&registry).unwrap_err();

        assert_eq!(
            error.code,
            FangyuanSkillVisualDiagnosticCode::AuditDegradeOnlyFieldExceeded
        );
    }

    #[test]
    fn fangyuan_skill_visual_can_resolve_unknown_template_to_registry_fallback() {
        let registry = FangyuanSkillTemplateRegistry::with_defaults();

        let resolution = registry.resolve_or_fallback(Some("missing"), Some(7));

        assert_eq!(
            resolution.template.id,
            FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID
        );
        assert_eq!(
            resolution.fallback_reason,
            Some(FangyuanSkillFallbackReason::UnknownTemplate {
                id: "missing".to_string(),
                version: 7
            })
        );
    }

    #[test]
    fn fangyuan_skill_audit_reports_danger_boundary_that_is_too_small() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.visual_range_hint = Some(FangyuanSkillVisualRangeHint::Projectile {
            length: 4.0,
            radius: 0.2,
        });

        let report = audit_fangyuan_skill_visual_readability(&template, &blueprint);

        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::VisualRangeTooSmall));
        assert!(!report.passed());
    }

    #[test]
    fn fangyuan_skill_audit_reports_misleading_range_shape_and_decor_overflow() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.visual_range_hint =
            Some(FangyuanSkillVisualRangeHint::CircleArea { radius: 2.0 });
        blueprint.readability.decor_bounds_radius = 12.0;

        let report = audit_fangyuan_skill_visual_readability(&template, &blueprint);

        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::VisualRangeMismatch));
        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::DecorBoundsExceeded));
    }

    #[test]
    fn fangyuan_skill_audit_reports_occlusion_color_transparency_and_emissive_conflicts() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.color = [0.1, 0.2, 0.95, 0.9];
        blueprint.readability.rule_alpha = 0.2;
        blueprint.readability.personality_occlusion = 0.8;
        blueprint.readability.transparent_primitive_budget = 20;
        blueprint.emissive.intensity = 8.0;

        let report = audit_fangyuan_skill_visual_readability(&template, &blueprint);

        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::RuleLayerOccluded));
        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::ColorConflict));
        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::TransparentBudgetExceeded));
        assert!(report.has_code(FangyuanSkillAuditDiagnosticCode::EmissiveBudgetExceeded));
    }

    #[test]
    fn fangyuan_skill_runtime_compiles_rule_and_personality_layers_in_playback_order() {
        let template = valid_projectile_template();
        let blueprint = default_projectile_visual_blueprint();
        let context = FangyuanSkillRuntimeContext::local(0, 2, 30, "caster-a", "event-a");

        let presentation =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &context).unwrap();
        let playback = presentation.playback_states();

        assert!(!presentation.rule_layer_states.is_empty());
        assert!(!presentation.personality_layer_states.is_empty());
        assert_eq!(
            playback.len(),
            presentation.rule_layer_states.len() + presentation.personality_layer_states.len()
        );
        assert!(presentation.rule_layer_states.iter().all(|state| matches!(
            state.role,
            FangyuanPrimitiveRole::Core
                | FangyuanPrimitiveRole::Boundary
                | FangyuanPrimitiveRole::Warning
                | FangyuanPrimitiveRole::Impact
        )));
        assert_eq!(playback[0].role, FangyuanPrimitiveRole::Core);
        assert_eq!(
            playback[presentation.rule_layer_states.len()].recipe_id,
            "skill.visual.projectile.personality_layer"
        );
    }

    #[test]
    fn fangyuan_skill_runtime_rule_layer_lifecycle_is_not_overridden_by_personality() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.impact_residue.duration_ticks = 120;
        let context = FangyuanSkillRuntimeContext::local(10, 28, 30, "caster-a", "event-a");

        let presentation =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &context).unwrap();
        let expected_rule_duration = template
            .timing
            .impact_tick_offset
            .saturating_add(template.danger_boundary.linger_ticks);

        assert!(presentation.rule_layer_states.iter().any(|state| {
            state.role == FangyuanPrimitiveRole::Impact
                && state.lifecycle.spawn_tick == Some(context.start_tick)
                && state.lifecycle.lifetime == Some(expected_rule_duration)
        }));
        assert!(
            presentation
                .personality_layer_states
                .iter()
                .all(|state| state.recipe_id.ends_with(".personality_layer"))
        );
    }

    #[test]
    fn fangyuan_skill_degrade_preserves_rule_layer_hash_and_removes_personality_costs() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.trail.segment_count = 12;
        blueprint.emissive.intensity = 3.5;
        let base_context = FangyuanSkillRuntimeContext::local(0, 10, 30, "caster-a", "event-a");
        let critical_context = base_context
            .clone()
            .with_degrade_level(FangyuanSkillDegradeLevel::Critical);

        let full =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &base_context)
                .unwrap();
        let degraded =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &critical_context)
                .unwrap();

        assert_eq!(full.rule_layer_hash(), degraded.rule_layer_hash());
        assert!(full.personality_layer_states.len() > degraded.personality_layer_states.len());
        assert!(
            !degraded
                .personality_layer_states
                .iter()
                .any(|state| state.role == FangyuanPrimitiveRole::Decoration)
        );
        assert!(
            degraded
                .personality_layer_states
                .iter()
                .all(|state| state.emissive == 0.0)
        );
    }

    #[test]
    fn fangyuan_skill_degrade_keeps_rule_layer_visible_under_high_pressure() {
        let template = valid_projectile_template();
        let blueprint = default_projectile_visual_blueprint();
        let context = FangyuanSkillRuntimeContext::local(0, 18, 30, "caster-a", "event-a")
            .with_degrade_level(FangyuanSkillDegradeLevel::High);

        let presentation =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &context).unwrap();

        assert!(
            presentation
                .rule_layer_states
                .iter()
                .any(|state| state.role == FangyuanPrimitiveRole::Boundary && state.alpha >= 0.35)
        );
        assert!(
            presentation
                .rule_layer_states
                .iter()
                .any(|state| state.role == FangyuanPrimitiveRole::Impact)
        );
    }

    #[test]
    fn fangyuan_skill_projectile_uses_equipment_tip_as_trajectory_start() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint
            .equipment_socket_bindings
            .push(FangyuanSkillEquipmentSocketBinding::move_from(
                "projectile",
                FangyuanEquipmentSocketSemantic::Tip,
            ));
        let sockets = fangyuan_default_equipment_blueprint()
            .compile_sockets()
            .unwrap();
        let context = FangyuanSkillRuntimeContext::local(0, 0, 30, "caster-a", "event-tip")
            .with_equipment_sockets(sockets);

        let presentation =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &context).unwrap();
        let projectile = presentation
            .personality_layer_states
            .iter()
            .find(|state| state.emitter_id == "projectile")
            .unwrap();

        assert_eq!(
            projectile.local_position,
            Vec3::new(0.95, 0.45, 0.0),
            "projectile should start from the equipment tip socket at spawn"
        );
        assert_eq!(presentation.equipment_socket_bindings.len(), 1);
        assert_eq!(
            presentation.equipment_socket_bindings[0].fallback, None,
            "existing tip socket should not report fallback"
        );
        assert!(presentation.equipment_socket_bindings[0].applied_to_emitter);
    }

    #[test]
    fn fangyuan_skill_shield_uses_equipment_core_as_emitter_origin() {
        let template = fangyuan_default_skill_templates()
            .into_iter()
            .find(|template| template.id == FANGYUAN_SKILL_SHIELD_TEMPLATE_ID)
            .unwrap();
        let mut blueprint = default_shield_visual_blueprint();
        blueprint.equipment_socket_bindings.push(
            FangyuanSkillEquipmentSocketBinding::emitter_origin(
                "shield",
                FangyuanEquipmentSocketSemantic::Core,
            ),
        );
        let sockets = fangyuan_default_equipment_blueprint()
            .compile_sockets()
            .unwrap();
        let context = FangyuanSkillRuntimeContext::local(0, 0, 30, "caster-a", "event-core")
            .with_equipment_sockets(sockets);

        let presentation =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &context).unwrap();
        let shield = presentation
            .personality_layer_states
            .iter()
            .find(|state| state.emitter_id == "shield")
            .unwrap();

        assert_eq!(shield.local_position, Vec3::new(0.0, 0.45, 0.0));
        assert_eq!(
            presentation.equipment_socket_bindings[0].requested_socket,
            FangyuanEquipmentSocketSemantic::Core
        );
        assert!(presentation.equipment_socket_bindings[0].fallback.is_none());
    }

    #[test]
    fn fangyuan_skill_missing_socket_uses_explicit_fallback_and_reports_it() {
        let template = valid_projectile_template();
        let mut blueprint = default_projectile_visual_blueprint();
        blueprint.equipment_socket_bindings.push(
            FangyuanSkillEquipmentSocketBinding::move_from(
                "projectile",
                FangyuanEquipmentSocketSemantic::Tip,
            )
            .with_fallback(FangyuanEquipmentSocketFallback::Socket {
                semantic: FangyuanEquipmentSocketSemantic::Core,
            }),
        );
        let mut equipment = fangyuan_default_equipment_blueprint();
        equipment
            .sockets
            .retain(|socket| socket.semantic != FangyuanEquipmentSocketSemantic::Tip);
        let context = FangyuanSkillRuntimeContext::local(0, 0, 30, "caster-a", "event-fallback")
            .with_equipment_sockets(equipment.compile_sockets().unwrap());

        let presentation =
            compile_fangyuan_skill_runtime_presentation(&template, &blueprint, &context).unwrap();
        let projectile = presentation
            .personality_layer_states
            .iter()
            .find(|state| state.emitter_id == "projectile")
            .unwrap();
        let binding = &presentation.equipment_socket_bindings[0];
        let fallback = binding.fallback.unwrap();

        assert_eq!(projectile.local_position, Vec3::new(0.0, 0.45, 0.0));
        assert_eq!(
            fallback.reason,
            FangyuanEquipmentSocketFallbackReason::MissingSocket
        );
        assert_eq!(
            fallback.applied_fallback,
            FangyuanEquipmentSocketFallback::Socket {
                semantic: FangyuanEquipmentSocketSemantic::Core
            }
        );
        assert!(binding.applied_to_emitter);
    }
}
