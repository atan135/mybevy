use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    FangyuanVfxRecipe, fangyuan_vfx_impact_expand_recipe, fangyuan_vfx_projectile_recipe,
    fangyuan_vfx_range_marker_recipe, fangyuan_vfx_shield_recipe,
};

pub const FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION: u32 = 1;
pub const FANGYUAN_SKILL_DEFAULT_TEMPLATE_VERSION: u32 = 1;
pub const FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID: &str = "skill.template.projectile";
pub const FANGYUAN_SKILL_CIRCLE_AREA_TEMPLATE_ID: &str = "skill.template.circle_area";
pub const FANGYUAN_SKILL_CONE_TEMPLATE_ID: &str = "skill.template.cone";
pub const FANGYUAN_SKILL_SHIELD_TEMPLATE_ID: &str = "skill.template.shield";

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
        self.trail.validate(&template.field_policy)?;
        self.decor.validate(&template.field_policy)?;
        self.impact_residue.validate(&template.field_policy)?;
        self.emissive.validate(&template.field_policy)?;
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
        profile_ref: Some("vfx/default".to_string()),
        vfx_recipe,
        trail: FangyuanSkillTrailVisual::default(),
        decor: FangyuanSkillDecorVisual::default(),
        impact_residue: FangyuanSkillImpactResidueVisual::default(),
        emissive: FangyuanSkillEmissiveVisual::default(),
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

#[cfg(test)]
mod tests {
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
}
