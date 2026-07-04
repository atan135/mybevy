use bevy::prelude::*;
use std::collections::BTreeMap;

use super::{
    FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FANGYUAN_PRIMITIVE_MAX_EMISSIVE, FangyuanPrimitive,
};

pub const FANGYUAN_MATERIAL_PROFILE_VERSION: &str = "1";
pub const FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID: &str = "material:default";
pub const FANGYUAN_MATERIAL_PROFILE_DEFAULT_DEBUG_LABEL: &str = "default";
pub const FANGYUAN_MATERIAL_PROFILE_ID_MAX_LEN: usize = 64;
pub const FANGYUAN_MATERIAL_PROFILE_DEBUG_LABEL_MAX_LEN: usize = 96;
pub const FANGYUAN_MATERIAL_PROFILE_MAX_COUNT: usize = 256;

/// Stable material defaults and policy for Fangyuan runtime primitives.
///
/// Runtime primitive fields remain per-instance data. The profile supplies the
/// base RGB/alpha/emissive values and policy caps; composition is handled by
/// `compose_primitive` without requiring a rendering backend migration.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanMaterialProfile {
    pub stable_id: String,
    pub version: String,
    pub base: FangyuanMaterialBaseParams,
    pub emissive_policy: FangyuanMaterialEmissivePolicy,
    pub alpha_policy: FangyuanMaterialAlphaPolicy,
    pub debug_label: String,
}

impl FangyuanMaterialProfile {
    pub fn default_profile() -> Self {
        Self {
            stable_id: FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID.to_string(),
            version: FANGYUAN_MATERIAL_PROFILE_VERSION.to_string(),
            base: FangyuanMaterialBaseParams::default(),
            emissive_policy: FangyuanMaterialEmissivePolicy::default(),
            alpha_policy: FangyuanMaterialAlphaPolicy::default(),
            debug_label: FANGYUAN_MATERIAL_PROFILE_DEFAULT_DEBUG_LABEL.to_string(),
        }
    }

    pub fn validate(&self) -> Result<(), FangyuanMaterialProfileValidationError> {
        validate_fangyuan_material_profile_id(&self.stable_id).map_err(|reason| {
            FangyuanMaterialProfileValidationError::InvalidStableId {
                stable_id: self.stable_id.clone(),
                reason,
            }
        })?;

        if self.version != FANGYUAN_MATERIAL_PROFILE_VERSION {
            return Err(FangyuanMaterialProfileValidationError::UnsupportedVersion {
                stable_id: self.stable_id.clone(),
                found: self.version.clone(),
                expected: FANGYUAN_MATERIAL_PROFILE_VERSION,
            });
        }

        if self.debug_label.trim().is_empty() {
            return Err(FangyuanMaterialProfileValidationError::InvalidDebugLabel {
                stable_id: self.stable_id.clone(),
                reason: FangyuanMaterialProfileDebugLabelInvalidReason::Empty,
            });
        }
        if self.debug_label.len() > FANGYUAN_MATERIAL_PROFILE_DEBUG_LABEL_MAX_LEN {
            return Err(FangyuanMaterialProfileValidationError::InvalidDebugLabel {
                stable_id: self.stable_id.clone(),
                reason: FangyuanMaterialProfileDebugLabelInvalidReason::TooLong {
                    max_len: FANGYUAN_MATERIAL_PROFILE_DEBUG_LABEL_MAX_LEN,
                },
            });
        }

        self.base
            .validate(&self.stable_id)
            .map_err(FangyuanMaterialProfileValidationError::InvalidBaseParams)?;
        self.alpha_policy
            .validate(&self.stable_id)
            .map_err(FangyuanMaterialProfileValidationError::InvalidAlphaPolicy)?;
        self.emissive_policy
            .validate(&self.stable_id)
            .map_err(FangyuanMaterialProfileValidationError::InvalidEmissivePolicy)?;

        Ok(())
    }

    /// Composes final instance material params.
    ///
    /// Priority rules:
    /// - RGB: `profile.base.color.rgb * primitive.color.rgb`.
    /// - Alpha: `primitive.alpha` is the instance alpha input. Blueprint color
    ///   alpha remains compatible because it compiles into `primitive.alpha`
    ///   when no explicit alpha override is authored.
    /// - Emissive: `primitive.emissive` is an instance boost. The profile
    ///   chooses whether to add and clamp it or disable emissive output.
    pub fn compose_primitive(
        &self,
        primitive: &FangyuanPrimitive,
    ) -> FangyuanMaterialInstanceParams {
        let alpha = self
            .alpha_policy
            .compose(self.base.alpha, primitive.alpha());
        let emissive = self
            .emissive_policy
            .compose(self.base.emissive, primitive.emissive());
        let color = compose_color(self.base.color, primitive.color(), alpha);

        FangyuanMaterialInstanceParams {
            profile_id: self.stable_id.clone(),
            requested_profile_id: primitive.material_profile_id().map(str::to_string),
            color,
            alpha,
            emissive,
            debug_label: self.debug_label.clone(),
            fallback_reason: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanMaterialBaseParams {
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
}

impl FangyuanMaterialBaseParams {
    pub fn validate(
        &self,
        stable_id: &str,
    ) -> Result<(), FangyuanMaterialBaseParamsValidationError> {
        let color = self.color.to_srgba();
        for (channel, value) in [color.red, color.green, color.blue].into_iter().enumerate() {
            if !is_valid_unit_channel(value) {
                return Err(
                    FangyuanMaterialBaseParamsValidationError::InvalidColorChannel {
                        stable_id: stable_id.to_string(),
                        channel,
                        value,
                    },
                );
            }
        }

        if !is_valid_unit_channel(self.alpha) {
            return Err(FangyuanMaterialBaseParamsValidationError::InvalidAlpha {
                stable_id: stable_id.to_string(),
                value: self.alpha,
            });
        }

        if !self.emissive.is_finite()
            || !(FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE..=FANGYUAN_PRIMITIVE_MAX_EMISSIVE)
                .contains(&self.emissive)
        {
            return Err(FangyuanMaterialBaseParamsValidationError::InvalidEmissive {
                stable_id: stable_id.to_string(),
                value: self.emissive,
                max: FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
            });
        }

        Ok(())
    }
}

impl Default for FangyuanMaterialBaseParams {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            alpha: 1.0,
            emissive: FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FangyuanMaterialAlphaPolicy {
    MultiplyClamp { min: f32, max: f32 },
    ForceOpaque,
}

impl FangyuanMaterialAlphaPolicy {
    pub fn validate(
        self,
        stable_id: &str,
    ) -> Result<(), FangyuanMaterialAlphaPolicyValidationError> {
        match self {
            Self::MultiplyClamp { min, max } => {
                if min.is_finite() && max.is_finite() && 0.0 <= min && min <= max && max <= 1.0 {
                    Ok(())
                } else {
                    Err(FangyuanMaterialAlphaPolicyValidationError::InvalidClamp {
                        stable_id: stable_id.to_string(),
                        min,
                        max,
                    })
                }
            }
            Self::ForceOpaque => Ok(()),
        }
    }

    pub fn compose(self, base_alpha: f32, primitive_alpha: f32) -> f32 {
        match self {
            Self::MultiplyClamp { min, max } => {
                clamp_finite(base_alpha * primitive_alpha, min, max)
            }
            Self::ForceOpaque => 1.0,
        }
    }
}

impl Default for FangyuanMaterialAlphaPolicy {
    fn default() -> Self {
        Self::MultiplyClamp { min: 0.0, max: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FangyuanMaterialEmissivePolicy {
    AdditiveClamp { max: f32 },
    Disabled,
}

impl FangyuanMaterialEmissivePolicy {
    pub fn validate(
        self,
        stable_id: &str,
    ) -> Result<(), FangyuanMaterialEmissivePolicyValidationError> {
        match self {
            Self::AdditiveClamp { max } => {
                if max.is_finite()
                    && (FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE..=FANGYUAN_PRIMITIVE_MAX_EMISSIVE)
                        .contains(&max)
                {
                    Ok(())
                } else {
                    Err(
                        FangyuanMaterialEmissivePolicyValidationError::InvalidClamp {
                            stable_id: stable_id.to_string(),
                            max,
                        },
                    )
                }
            }
            Self::Disabled => Ok(()),
        }
    }

    pub fn compose(self, base_emissive: f32, primitive_emissive: f32) -> f32 {
        match self {
            Self::AdditiveClamp { max } => clamp_finite(
                base_emissive + primitive_emissive,
                FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
                max,
            ),
            Self::Disabled => FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
        }
    }
}

impl Default for FangyuanMaterialEmissivePolicy {
    fn default() -> Self {
        Self::AdditiveClamp {
            max: FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanMaterialInstanceParams {
    pub profile_id: String,
    pub requested_profile_id: Option<String>,
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
    pub debug_label: String,
    pub fallback_reason: Option<FangyuanMaterialProfileFallbackReason>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanMaterialProfileRegistry {
    profiles: BTreeMap<String, FangyuanMaterialProfile>,
    default_profile_id: String,
    max_profiles: usize,
}

pub type FangyuanMaterialProfileTable = FangyuanMaterialProfileRegistry;

impl FangyuanMaterialProfileRegistry {
    pub fn new() -> Self {
        Self::with_max_profiles(FANGYUAN_MATERIAL_PROFILE_MAX_COUNT)
    }

    pub fn with_max_profiles(max_profiles: usize) -> Self {
        let default_profile = FangyuanMaterialProfile::default_profile();
        let default_profile_id = default_profile.stable_id.clone();
        let mut profiles = BTreeMap::new();
        profiles.insert(default_profile.stable_id.clone(), default_profile);

        Self {
            profiles,
            default_profile_id,
            max_profiles: max_profiles.max(1),
        }
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn max_profiles(&self) -> usize {
        self.max_profiles
    }

    pub fn default_profile(&self) -> &FangyuanMaterialProfile {
        self.profiles
            .get(&self.default_profile_id)
            .expect("fangyuan material registry must contain the default profile")
    }

    pub fn get(&self, stable_id: &str) -> Option<&FangyuanMaterialProfile> {
        self.profiles.get(stable_id)
    }

    pub fn insert_profile(
        &mut self,
        profile: FangyuanMaterialProfile,
    ) -> Result<(), FangyuanMaterialProfileRegistryError> {
        profile
            .validate()
            .map_err(FangyuanMaterialProfileRegistryError::ValidationFailed)?;

        if self.profiles.contains_key(&profile.stable_id) {
            return Err(FangyuanMaterialProfileRegistryError::DuplicateProfileId {
                stable_id: profile.stable_id,
            });
        }

        let next_count = self.profiles.len() + 1;
        if next_count > self.max_profiles {
            return Err(
                FangyuanMaterialProfileRegistryError::ProfileCountLimitExceeded {
                    count: next_count,
                    limit: self.max_profiles,
                },
            );
        }

        self.profiles.insert(profile.stable_id.clone(), profile);
        Ok(())
    }

    pub fn resolve(&self, profile_id: Option<&str>) -> FangyuanMaterialProfileResolution<'_> {
        let Some(profile_id) = profile_id else {
            return FangyuanMaterialProfileResolution {
                profile: self.default_profile(),
                requested_profile_id: None,
                fallback_reason: None,
            };
        };

        if let Err(reason) = validate_fangyuan_material_profile_id(profile_id) {
            return FangyuanMaterialProfileResolution {
                profile: self.default_profile(),
                requested_profile_id: Some(profile_id.to_string()),
                fallback_reason: Some(FangyuanMaterialProfileFallbackReason::InvalidProfileId {
                    profile_id: profile_id.to_string(),
                    reason,
                }),
            };
        }

        match self.profiles.get(profile_id) {
            Some(profile) => FangyuanMaterialProfileResolution {
                profile,
                requested_profile_id: Some(profile_id.to_string()),
                fallback_reason: None,
            },
            None => FangyuanMaterialProfileResolution {
                profile: self.default_profile(),
                requested_profile_id: Some(profile_id.to_string()),
                fallback_reason: Some(FangyuanMaterialProfileFallbackReason::UnknownProfileId {
                    profile_id: profile_id.to_string(),
                }),
            },
        }
    }

    pub fn compose_primitive(
        &self,
        primitive: &FangyuanPrimitive,
    ) -> FangyuanMaterialInstanceParams {
        self.compose_runtime_fields(
            primitive.color(),
            primitive.alpha(),
            primitive.emissive(),
            primitive.material_profile_id(),
        )
    }

    pub fn compose_runtime_fields(
        &self,
        color: Color,
        alpha: f32,
        emissive: f32,
        material_profile_id: Option<&str>,
    ) -> FangyuanMaterialInstanceParams {
        let resolution = self.resolve(material_profile_id);
        let composed_alpha = resolution
            .profile
            .alpha_policy
            .compose(resolution.profile.base.alpha, alpha);
        let composed_emissive = resolution
            .profile
            .emissive_policy
            .compose(resolution.profile.base.emissive, emissive);
        let composed_color = compose_color(resolution.profile.base.color, color, composed_alpha);

        FangyuanMaterialInstanceParams {
            profile_id: resolution.profile.stable_id.clone(),
            requested_profile_id: resolution.requested_profile_id,
            color: composed_color,
            alpha: composed_alpha,
            emissive: composed_emissive,
            debug_label: resolution.profile.debug_label.clone(),
            fallback_reason: resolution.fallback_reason,
        }
    }
}

impl Default for FangyuanMaterialProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanMaterialProfileResolution<'a> {
    pub profile: &'a FangyuanMaterialProfile,
    pub requested_profile_id: Option<String>,
    pub fallback_reason: Option<FangyuanMaterialProfileFallbackReason>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanMaterialProfileFallbackReason {
    InvalidProfileId {
        profile_id: String,
        reason: FangyuanMaterialProfileIdInvalidReason,
    },
    UnknownProfileId {
        profile_id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanMaterialProfileRegistryError {
    ValidationFailed(FangyuanMaterialProfileValidationError),
    DuplicateProfileId { stable_id: String },
    ProfileCountLimitExceeded { count: usize, limit: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanMaterialProfileValidationError {
    InvalidStableId {
        stable_id: String,
        reason: FangyuanMaterialProfileIdInvalidReason,
    },
    UnsupportedVersion {
        stable_id: String,
        found: String,
        expected: &'static str,
    },
    InvalidDebugLabel {
        stable_id: String,
        reason: FangyuanMaterialProfileDebugLabelInvalidReason,
    },
    InvalidBaseParams(FangyuanMaterialBaseParamsValidationError),
    InvalidAlphaPolicy(FangyuanMaterialAlphaPolicyValidationError),
    InvalidEmissivePolicy(FangyuanMaterialEmissivePolicyValidationError),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanMaterialBaseParamsValidationError {
    InvalidColorChannel {
        stable_id: String,
        channel: usize,
        value: f32,
    },
    InvalidAlpha {
        stable_id: String,
        value: f32,
    },
    InvalidEmissive {
        stable_id: String,
        value: f32,
        max: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanMaterialAlphaPolicyValidationError {
    InvalidClamp {
        stable_id: String,
        min: f32,
        max: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanMaterialEmissivePolicyValidationError {
    InvalidClamp { stable_id: String, max: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanMaterialProfileIdInvalidReason {
    Empty,
    TooLong { max_len: usize },
    InvalidCharacter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanMaterialProfileDebugLabelInvalidReason {
    Empty,
    TooLong { max_len: usize },
}

pub fn is_valid_fangyuan_material_profile_id(profile_id: &str) -> bool {
    validate_fangyuan_material_profile_id(profile_id).is_ok()
}

pub fn validate_fangyuan_material_profile_id(
    profile_id: &str,
) -> Result<(), FangyuanMaterialProfileIdInvalidReason> {
    if profile_id.is_empty() {
        return Err(FangyuanMaterialProfileIdInvalidReason::Empty);
    }
    if profile_id.len() > FANGYUAN_MATERIAL_PROFILE_ID_MAX_LEN {
        return Err(FangyuanMaterialProfileIdInvalidReason::TooLong {
            max_len: FANGYUAN_MATERIAL_PROFILE_ID_MAX_LEN,
        });
    }
    if !profile_id.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b':')
    }) {
        return Err(FangyuanMaterialProfileIdInvalidReason::InvalidCharacter);
    }

    Ok(())
}

fn compose_color(base_color: Color, primitive_color: Color, alpha: f32) -> Color {
    let base = base_color.to_srgba();
    let primitive = primitive_color.to_srgba();
    Color::srgba(
        sanitize_unit_channel(base.red * primitive.red),
        sanitize_unit_channel(base.green * primitive.green),
        sanitize_unit_channel(base.blue * primitive.blue),
        sanitize_unit_channel(alpha),
    )
}

fn clamp_finite(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    }
}

fn is_valid_unit_channel(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn sanitize_unit_channel(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT, FANGYUAN_SCENE_LAYOUT_VERSION,
        FANGYUAN_STATIC_MERGE_DEFAULT_MATERIAL_PROFILE, FangyuanAuditBudgetProfile,
        FangyuanAuditStatus, FangyuanBlueprint, FangyuanBlueprintBounds, FangyuanPrefabDefinition,
        FangyuanPrefabPalette, FangyuanPrimitiveBlueprint, FangyuanPrimitiveKind,
        FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole, FangyuanPrimitiveSet,
        FangyuanSceneLayout, FangyuanSceneLayoutInstance, FangyuanStaticMergeTransparentPath,
        audit_fangyuan_primitive_set_budget, fangyuan_static_instance_batches_from_layout,
        fangyuan_static_merge_groups_from_layout, fangyuan_static_merge_groups_from_primitive_set,
        fangyuan_static_meshes_from_primitive_set,
    };
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn fangyuan_material_profile_registry_uses_default_and_fallbacks() {
        let registry = FangyuanMaterialProfileRegistry::default();

        let default_resolution = registry.resolve(None);
        assert_eq!(
            default_resolution.profile.stable_id,
            FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID
        );
        assert_eq!(default_resolution.fallback_reason, None);

        let unknown_resolution = registry.resolve(Some("missing/profile"));
        assert_eq!(
            unknown_resolution.profile.stable_id,
            FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID
        );
        assert_eq!(
            unknown_resolution.fallback_reason,
            Some(FangyuanMaterialProfileFallbackReason::UnknownProfileId {
                profile_id: "missing/profile".to_string()
            })
        );

        let invalid_resolution = registry.resolve(Some("bad profile"));
        assert_eq!(
            invalid_resolution.profile.stable_id,
            FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID
        );
        assert_eq!(
            invalid_resolution.fallback_reason,
            Some(FangyuanMaterialProfileFallbackReason::InvalidProfileId {
                profile_id: "bad profile".to_string(),
                reason: FangyuanMaterialProfileIdInvalidReason::InvalidCharacter,
            })
        );
    }

    #[test]
    fn fangyuan_material_profile_registry_enforces_profile_limit_and_validation() {
        let mut registry = FangyuanMaterialProfileRegistry::with_max_profiles(2);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.max_profiles(), 2);

        registry
            .insert_profile(test_profile("stone/matte", "Stone Matte"))
            .unwrap();

        let limit_error = registry
            .insert_profile(test_profile("glass/soft", "Glass Soft"))
            .unwrap_err();
        assert_eq!(
            limit_error,
            FangyuanMaterialProfileRegistryError::ProfileCountLimitExceeded { count: 3, limit: 2 }
        );

        let duplicate_error = registry
            .insert_profile(test_profile("stone/matte", "Stone Matte Duplicate"))
            .unwrap_err();
        assert_eq!(
            duplicate_error,
            FangyuanMaterialProfileRegistryError::DuplicateProfileId {
                stable_id: "stone/matte".to_string()
            }
        );

        let invalid_profile = FangyuanMaterialProfile {
            version: "2".to_string(),
            ..test_profile("fx/glow", "FX Glow")
        };
        let invalid_error = FangyuanMaterialProfileRegistry::default()
            .insert_profile(invalid_profile)
            .unwrap_err();
        assert!(matches!(
            invalid_error,
            FangyuanMaterialProfileRegistryError::ValidationFailed(
                FangyuanMaterialProfileValidationError::UnsupportedVersion { .. }
            )
        ));
    }

    #[test]
    fn fangyuan_material_profile_composes_color_alpha_and_emissive_by_policy() {
        let profile = FangyuanMaterialProfile {
            stable_id: "fx/warm_glow".to_string(),
            version: FANGYUAN_MATERIAL_PROFILE_VERSION.to_string(),
            base: FangyuanMaterialBaseParams {
                color: Color::srgba(0.5, 1.0, 0.25, 1.0),
                alpha: 0.8,
                emissive: 1.0,
            },
            alpha_policy: FangyuanMaterialAlphaPolicy::MultiplyClamp { min: 0.2, max: 0.6 },
            emissive_policy: FangyuanMaterialEmissivePolicy::AdditiveClamp { max: 2.0 },
            debug_label: "Warm Glow".to_string(),
        };
        let primitive = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::srgba(0.4, 0.5, 0.5, 0.1),
            FangyuanPrimitiveRole::Structure,
            0.75,
            1.5,
            Some("fx/warm_glow".to_string()),
            FangyuanPrimitiveLifecycle::empty(),
        );

        profile.validate().unwrap();
        let params = profile.compose_primitive(&primitive);

        assert_eq!(params.profile_id, "fx/warm_glow");
        assert_eq!(params.requested_profile_id.as_deref(), Some("fx/warm_glow"));
        assert_srgba(params.color, [0.2, 0.5, 0.125, 0.6]);
        assert_nearly_eq(params.alpha, 0.6);
        assert_nearly_eq(params.emissive, 2.0);
        assert_eq!(params.debug_label, "Warm Glow");
    }

    #[test]
    fn fangyuan_material_profile_can_force_opaque_and_disable_emissive() {
        let profile = FangyuanMaterialProfile {
            stable_id: "stone/opaque".to_string(),
            version: FANGYUAN_MATERIAL_PROFILE_VERSION.to_string(),
            base: FangyuanMaterialBaseParams {
                color: Color::WHITE,
                alpha: 0.2,
                emissive: 4.0,
            },
            alpha_policy: FangyuanMaterialAlphaPolicy::ForceOpaque,
            emissive_policy: FangyuanMaterialEmissivePolicy::Disabled,
            debug_label: "Opaque Stone".to_string(),
        };
        let primitive = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::srgba(0.2, 0.3, 0.4, 0.5),
            FangyuanPrimitiveRole::Structure,
            0.5,
            3.0,
            Some("stone/opaque".to_string()),
            FangyuanPrimitiveLifecycle::empty(),
        );

        let params = profile.compose_primitive(&primitive);

        assert_srgba(params.color, [0.2, 0.3, 0.4, 1.0]);
        assert_nearly_eq(params.alpha, 1.0);
        assert_nearly_eq(params.emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);
    }

    #[test]
    fn fangyuan_material_profile_legacy_missing_profile_uses_default_profile() {
        let mut primitive_blueprint = valid_primitive_blueprint();
        primitive_blueprint.color = [0.2, 0.4, 0.6, 0.5];
        primitive_blueprint.material_profile_id = None;
        let blueprint = valid_blueprint(vec![primitive_blueprint]);
        let primitive_set = blueprint.compile().unwrap();
        let primitive = &primitive_set.primitives()[0];

        let registry = FangyuanMaterialProfileRegistry::default();
        let params = registry.compose_primitive(primitive);

        assert_eq!(primitive.material_profile_id(), None);
        assert_eq!(params.profile_id, FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID);
        assert_eq!(params.requested_profile_id, None);
        assert_eq!(params.fallback_reason, None);
        assert_srgba(params.color, [0.2, 0.4, 0.6, 0.5]);
        assert_nearly_eq(params.alpha, 0.5);
        assert_nearly_eq(params.emissive, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE);

        let merge_report = fangyuan_static_merge_groups_from_primitive_set(&primitive_set);
        assert_eq!(merge_report.groups.len(), 1);
        assert_eq!(
            merge_report.groups[0].key.material_profile,
            FANGYUAN_STATIC_MERGE_DEFAULT_MATERIAL_PROFILE
        );
        assert_eq!(
            merge_report.groups[0].key.material_profile,
            FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID
        );
    }

    #[test]
    fn fangyuan_material_fields_flow_from_blueprint_prefab_layout_to_static_outputs() {
        let mut primitive_blueprint = valid_primitive_blueprint();
        primitive_blueprint.color = [0.25, 0.5, 0.75, 0.9];
        primitive_blueprint.alpha = Some(0.4);
        primitive_blueprint.emissive = Some(2.25);
        primitive_blueprint.material_profile_id = Some("fx/trail:soft".to_string());
        let blueprint = valid_blueprint(vec![primitive_blueprint.clone()]);

        let blueprint_primitive = blueprint.compile().unwrap().into_primitives().remove(0);
        assert_runtime_material_fields(
            &blueprint_primitive,
            "fx/trail:soft",
            [0.25, 0.5, 0.75, 0.9],
            0.4,
            2.25,
        );

        let palette = valid_palette(vec![valid_prefab("trail_piece", vec![primitive_blueprint])]);
        let layout = valid_layout(vec![FangyuanSceneLayoutInstance {
            id: Some("trail_a".to_string()),
            name: None,
            prefab: "trail_piece".to_string(),
            position: [2.0, 0.0, 0.0],
            scale: [2.0, 1.0, 1.0],
            tags: Vec::new(),
        }]);

        let compile_report = layout.compile_with_palette(&palette).unwrap();
        let generated = &compile_report.primitive_set.primitives()[0];
        assert_eq!(generated.local_position, Vec3::new(2.0, 1.0, 0.0));
        assert_eq!(generated.scale, Vec3::new(2.0, 1.0, 1.0));
        assert_runtime_material_fields(
            generated,
            "fx/trail:soft",
            [0.25, 0.5, 0.75, 0.9],
            0.4,
            2.25,
        );

        let merge_report = fangyuan_static_merge_groups_from_layout(
            &layout,
            &palette,
            Some("fangyuan/layouts/test_material.ron".to_string()),
        )
        .unwrap();
        assert_eq!(merge_report.groups.len(), 1);
        let merge_key = &merge_report.groups[0].key;
        assert_eq!(merge_key.material_profile, "fx/trail:soft");
        assert_eq!(
            merge_key.transparent_path,
            FangyuanStaticMergeTransparentPath::Transparent
        );
        assert_eq!(merge_key.color.channels(), [0.25, 0.5, 0.75, 0.4]);
        assert_nearly_eq(merge_key.emissive.to_f32(), 2.25);

        let mesh_report =
            fangyuan_static_meshes_from_primitive_set(&compile_report.primitive_set).unwrap();
        assert_eq!(mesh_report.meshes.len(), 1);
        assert_eq!(mesh_report.meshes[0].key.material_profile, "fx/trail:soft");
        assert_srgba(mesh_report.meshes[0].material.color, [0.25, 0.5, 0.75, 0.4]);
        assert_nearly_eq(mesh_report.meshes[0].material.alpha, 0.4);
        assert_nearly_eq(mesh_report.meshes[0].material.emissive, 2.25);
        let Some(VertexAttributeValues::Float32x4(colors)) =
            mesh_report.meshes[0].mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("static mesh should carry f32x4 color attributes");
        };
        assert!(colors.iter().all(|color| *color == [0.25, 0.5, 0.75, 0.4]));

        let instance_report = fangyuan_static_instance_batches_from_layout(
            &layout,
            &palette,
            Some("fangyuan/layouts/test_material.ron".to_string()),
        )
        .unwrap();
        assert_eq!(instance_report.batches.len(), 1);
        assert_eq!(
            instance_report.batches[0].key.material_profile,
            "fx/trail:soft"
        );
        let instance = &instance_report.batches[0].instances[0];
        assert_eq!(
            instance.material_profile_id.as_deref(),
            Some("fx/trail:soft")
        );
        assert_srgba(instance.color, [0.25, 0.5, 0.75, 0.9]);
        assert_nearly_eq(instance.alpha, 0.4);
        assert_nearly_eq(instance.emissive, 2.25);
    }

    #[test]
    fn fangyuan_material_fallback_and_transparent_budget_are_reported() {
        let registry = FangyuanMaterialProfileRegistry::default();
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![
            runtime_material_primitive(0.0, 0.35, 1.0, Some("stage8/unsupported_profile")),
            runtime_material_primitive(1.0, 0.45, 0.0, Some("stage8/unsupported_profile")),
            runtime_material_primitive(2.0, 0.55, 0.0, Some("stage8/other_profile")),
        ]);

        let fallback = registry.compose_primitive(&primitive_set.primitives()[0]);
        assert_eq!(
            fallback.profile_id, FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID,
            "unknown but syntactically supported profile ids must fall back to the default profile"
        );
        assert_eq!(
            fallback.fallback_reason,
            Some(FangyuanMaterialProfileFallbackReason::UnknownProfileId {
                profile_id: "stage8/unsupported_profile".to_string(),
            })
        );

        let warning_profile = FangyuanAuditBudgetProfile {
            recommended_transparent_count: 1,
            max_transparent_count: 10,
            recommended_alpha_count: 1,
            max_alpha_count: 10,
            recommended_emissive_count: 0,
            max_emissive_count: 10,
            recommended_material_profile_count: 1,
            max_material_profile_count: 10,
            ..Default::default()
        };
        let warning_report = audit_fangyuan_primitive_set_budget(&primitive_set, &warning_profile);
        println!(
            "fangyuan material warning summary: status={:?}, transparent={}, emissive={}, profiles={}, warnings={}, errors={}",
            warning_report.status,
            warning_report.summary.transparent_count,
            warning_report.summary.emissive_count,
            warning_report.summary.material_count,
            warning_report.summary.warning_count,
            warning_report.summary.error_count,
        );

        assert_eq!(
            warning_report.status,
            FangyuanAuditStatus::PassedWithWarnings
        );
        assert_eq!(warning_report.summary.transparent_count, 3);
        assert_eq!(warning_report.summary.emissive_count, 1);
        assert!(has_finding(
            &warning_report,
            "transparent_count_above_recommended"
        ));
        assert!(has_finding(
            &warning_report,
            "material_profile_count_above_recommended"
        ));

        let failed_profile = FangyuanAuditBudgetProfile {
            max_transparent_count: 2,
            max_alpha_count: 2,
            ..warning_profile
        };
        let failed_report = audit_fangyuan_primitive_set_budget(&primitive_set, &failed_profile);
        println!(
            "fangyuan material failed summary: status={:?}, transparent={}, warnings={}, errors={}",
            failed_report.status,
            failed_report.summary.transparent_count,
            failed_report.summary.warning_count,
            failed_report.summary.error_count,
        );

        assert_eq!(failed_report.status, FangyuanAuditStatus::Failed);
        assert!(has_finding(
            &failed_report,
            "transparent_count_above_hard_limit"
        ));
        assert!(has_finding(&failed_report, "alpha_count_above_hard_limit"));
    }

    fn test_profile(stable_id: &str, debug_label: &str) -> FangyuanMaterialProfile {
        FangyuanMaterialProfile {
            stable_id: stable_id.to_string(),
            version: FANGYUAN_MATERIAL_PROFILE_VERSION.to_string(),
            base: FangyuanMaterialBaseParams::default(),
            emissive_policy: FangyuanMaterialEmissivePolicy::default(),
            alpha_policy: FangyuanMaterialAlphaPolicy::default(),
            debug_label: debug_label.to_string(),
        }
    }

    fn valid_blueprint(primitives: Vec<FangyuanPrimitiveBlueprint>) -> FangyuanBlueprint {
        FangyuanBlueprint {
            version: "1".to_string(),
            name: "material_profile_test".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            bounds: FangyuanBlueprintBounds::new(10.0, 10.0, 8.0),
            primitives,
        }
    }

    fn valid_palette(prefabs: Vec<FangyuanPrefabDefinition>) -> FangyuanPrefabPalette {
        FangyuanPrefabPalette {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "material_profile_palette".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            bounds: FangyuanBlueprintBounds::new(10.0, 10.0, 8.0),
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

    fn valid_layout(instances: Vec<FangyuanSceneLayoutInstance>) -> FangyuanSceneLayout {
        FangyuanSceneLayout {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "material_profile_layout".to_string(),
            description: String::new(),
            bounds: FangyuanBlueprintBounds::new(10.0, 10.0, 8.0),
            palette: Some("fangyuan/palettes/material_profile_test.ron".to_string()),
            palettes: Vec::new(),
            max_primitives: FANGYUAN_SCENE_LAYOUT_HARD_PRIMITIVE_LIMIT,
            instances,
        }
    }

    fn valid_primitive_blueprint() -> FangyuanPrimitiveBlueprint {
        let mut primitive = FangyuanPrimitiveBlueprint::new(
            FangyuanPrimitiveKind::Cube,
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.2, 0.4, 0.6, 1.0],
        );
        primitive.role = Some(FangyuanPrimitiveRole::Structure);
        primitive
    }

    fn runtime_material_primitive(
        x: f32,
        alpha: f32,
        emissive: f32,
        material_profile_id: Option<&str>,
    ) -> FangyuanPrimitive {
        FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::new(x, 1.0, 0.0),
            Vec3::ONE,
            Color::srgba(0.2, 0.4, 0.6, alpha),
            FangyuanPrimitiveRole::Structure,
            alpha,
            emissive,
            material_profile_id.map(str::to_string),
            FangyuanPrimitiveLifecycle::empty(),
        )
    }

    fn has_finding(report: &crate::framework::fangyuan::FangyuanAuditReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }

    fn assert_runtime_material_fields(
        primitive: &FangyuanPrimitive,
        profile_id: &str,
        color: [f32; 4],
        alpha: f32,
        emissive: f32,
    ) {
        assert_eq!(primitive.material_profile_id(), Some(profile_id));
        assert_srgba(primitive.color(), color);
        assert_nearly_eq(primitive.alpha(), alpha);
        assert_nearly_eq(primitive.emissive(), emissive);
    }

    fn assert_srgba(color: Color, expected: [f32; 4]) {
        let color = color.to_srgba();
        assert_nearly_eq(color.red, expected[0]);
        assert_nearly_eq(color.green, expected[1]);
        assert_nearly_eq(color.blue, expected[2]);
        assert_nearly_eq(color.alpha, expected[3]);
    }

    fn assert_nearly_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.0001,
            "expected {actual} to be near {expected}"
        );
    }
}
