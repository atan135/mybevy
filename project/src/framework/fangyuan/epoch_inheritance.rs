use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

use super::{
    FangyuanAuditBudgetProfile, FangyuanAuditReport, FangyuanAuditStatus, FangyuanAuditSuggestion,
    FangyuanAuditSuggestionAction, FangyuanBlueprint, FangyuanBlueprintIdentity,
    FangyuanBlueprintValidationError, FangyuanIdentityResourceKind,
    FangyuanObjectBudgetDegradeSuggestion, FangyuanObjectBudgetProfile,
    FangyuanObjectBudgetSnapshot, audit_fangyuan_object_budget,
    upgrade_fangyuan_bake_source_if_needed,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEpochInheritanceInput {
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub old_world_blueprint_refs: Vec<FangyuanBlueprintIdentity>,
    pub player_homes: Vec<FangyuanEpochPlayerHomeRef>,
    pub equipment: Vec<FangyuanEpochEquipmentRef>,
    pub skill_visuals: Vec<FangyuanEpochSkillVisualRef>,
    pub tiandao_archives: Vec<FangyuanEpochTiandaoArchiveRef>,
}

impl FangyuanEpochInheritanceInput {
    pub fn validate(&self) -> Result<(), FangyuanEpochInheritanceError> {
        if self.new_epoch <= self.old_epoch {
            return Err(FangyuanEpochInheritanceError::EpochNotAdvanced {
                old_epoch: self.old_epoch,
                new_epoch: self.new_epoch,
            });
        }
        if self.old_world_blueprint_refs.is_empty() {
            return Err(FangyuanEpochInheritanceError::MissingBlueprintRefs);
        }
        for home in &self.player_homes {
            validate_non_empty("player_homes[].player_id", &home.player_id)?;
            validate_non_empty("player_homes[].home_id", &home.home_id)?;
            validate_non_empty("player_homes[].blueprint_ref", &home.blueprint_ref)?;
        }
        for equipment in &self.equipment {
            validate_non_empty("equipment[].equipment_id", &equipment.equipment_id)?;
            validate_non_empty("equipment[].blueprint_ref", &equipment.blueprint_ref)?;
        }
        for skill_visual in &self.skill_visuals {
            validate_non_empty("skill_visuals[].skill_id", &skill_visual.skill_id)?;
            validate_non_empty("skill_visuals[].visual_id", &skill_visual.visual_id)?;
        }
        for archive in &self.tiandao_archives {
            validate_non_empty("tiandao_archives[].archive_id", &archive.archive_id)?;
            validate_non_empty("tiandao_archives[].solidified_by", &archive.solidified_by)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEpochPlayerHomeRef {
    pub player_id: String,
    pub home_id: String,
    pub blueprint_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEpochEquipmentRef {
    pub owner_character_id: String,
    pub equipment_id: String,
    pub blueprint_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEpochSkillVisualRef {
    pub skill_id: String,
    pub visual_id: String,
    pub template_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanEpochTiandaoArchiveRef {
    pub archive_id: String,
    pub solidified_by: String,
    pub blueprint_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanEpochMigrationSource {
    pub identity: FangyuanBlueprintIdentity,
    pub ron_source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanEpochMigrationProfile {
    pub audit_budget: FangyuanAuditBudgetProfile,
    pub object_budget: FangyuanObjectBudgetProfile,
    pub object_snapshot: FangyuanObjectBudgetSnapshot,
}

impl Default for FangyuanEpochMigrationProfile {
    fn default() -> Self {
        Self {
            audit_budget: FangyuanAuditBudgetProfile::default(),
            object_budget: FangyuanObjectBudgetProfile::default(),
            object_snapshot: FangyuanObjectBudgetSnapshot::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanEpochInheritanceReport {
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub migrated_blueprints: Vec<FangyuanEpochMigratedBlueprint>,
    pub player_home_count: usize,
    pub equipment_count: usize,
    pub skill_visual_count: usize,
    pub tiandao_archive_count: usize,
    pub authority_reaudit_required: bool,
}

impl FangyuanEpochInheritanceReport {
    pub fn has_budget_pressure(&self) -> bool {
        self.migrated_blueprints.iter().any(|blueprint| {
            blueprint.audit.status != FangyuanAuditStatus::Passed
                || !blueprint.degrade_suggestions.is_empty()
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanEpochMigratedBlueprint {
    pub identity: FangyuanBlueprintIdentity,
    pub old_version: String,
    pub upgraded_version: String,
    pub upgraded_from_legacy: bool,
    pub audit: FangyuanAuditReport,
    pub degrade_suggestions: Vec<FangyuanEpochDegradeSuggestion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanEpochDegradeSuggestion {
    pub target: String,
    pub reason: String,
}

pub fn migrate_fangyuan_epoch_inheritance(
    input: &FangyuanEpochInheritanceInput,
    sources: &[FangyuanEpochMigrationSource],
    profile: &FangyuanEpochMigrationProfile,
) -> Result<FangyuanEpochInheritanceReport, FangyuanEpochInheritanceError> {
    input.validate()?;

    let mut migrated_blueprints = Vec::new();
    for identity in &input.old_world_blueprint_refs {
        if identity.kind != FangyuanIdentityResourceKind::Blueprint {
            return Err(FangyuanEpochInheritanceError::UnexpectedResourceKind {
                id: identity.id.clone(),
                kind: identity.kind,
            });
        }

        let source = sources
            .iter()
            .find(|source| source.identity.cache_key() == identity.cache_key())
            .ok_or_else(|| FangyuanEpochInheritanceError::MissingMigrationSource {
                key: identity.cache_key(),
            })?;

        let old_version = identity.version.clone();
        let (upgraded_source, source_version) =
            upgrade_fangyuan_bake_source_if_needed(&source.ron_source)?;
        let blueprint = FangyuanBlueprint::from_ron_str(&upgraded_source).map_err(|error| {
            FangyuanEpochInheritanceError::BlueprintParse {
                key: identity.cache_key(),
                message: error.to_string(),
            }
        })?;
        let audit = blueprint.audit(&profile.audit_budget);
        let mut degrade_suggestions = audit
            .suggestions
            .iter()
            .map(degrade_suggestion_from_audit)
            .collect::<Vec<_>>();

        let object_audit =
            audit_fangyuan_object_budget(&profile.object_snapshot, &profile.object_budget);
        degrade_suggestions.extend(
            object_audit
                .degrade_suggestions
                .iter()
                .map(degrade_suggestion_from_object_budget),
        );

        migrated_blueprints.push(FangyuanEpochMigratedBlueprint {
            identity: identity.clone(),
            old_version,
            upgraded_version: blueprint.version,
            upgraded_from_legacy: matches!(
                source_version,
                super::FangyuanBakeSourceVersion::LegacyZero
            ),
            audit,
            degrade_suggestions,
        });
    }

    Ok(FangyuanEpochInheritanceReport {
        old_epoch: input.old_epoch,
        new_epoch: input.new_epoch,
        migrated_blueprints,
        player_home_count: input.player_homes.len(),
        equipment_count: input.equipment.len(),
        skill_visual_count: input.skill_visuals.len(),
        tiandao_archive_count: input.tiandao_archives.len(),
        authority_reaudit_required: true,
    })
}

fn degrade_suggestion_from_audit(
    suggestion: &FangyuanAuditSuggestion,
) -> FangyuanEpochDegradeSuggestion {
    FangyuanEpochDegradeSuggestion {
        target: audit_suggestion_target(suggestion.action).to_string(),
        reason: suggestion.reason.clone(),
    }
}

fn degrade_suggestion_from_object_budget(
    suggestion: &FangyuanObjectBudgetDegradeSuggestion,
) -> FangyuanEpochDegradeSuggestion {
    FangyuanEpochDegradeSuggestion {
        target: suggestion.target.as_str().to_string(),
        reason: suggestion.reason.clone(),
    }
}

fn audit_suggestion_target(action: FangyuanAuditSuggestionAction) -> &'static str {
    match action {
        FangyuanAuditSuggestionAction::ReducePrimitives => "blueprint.reduce_primitives",
        FangyuanAuditSuggestionAction::ShrinkBounds => "blueprint.shrink_bounds",
        FangyuanAuditSuggestionAction::RemoveAlpha => "blueprint.remove_alpha",
        FangyuanAuditSuggestionAction::LowerEmissive => "blueprint.lower_emissive",
        FangyuanAuditSuggestionAction::ReplaceMaterialProfile => {
            "blueprint.replace_material_profile"
        }
        FangyuanAuditSuggestionAction::ReduceWarningRole => "blueprint.reduce_warning_role",
    }
}

#[derive(Debug)]
pub enum FangyuanEpochInheritanceError {
    EpochNotAdvanced {
        old_epoch: u64,
        new_epoch: u64,
    },
    MissingBlueprintRefs,
    EmptyField {
        field: &'static str,
    },
    UnexpectedResourceKind {
        id: String,
        kind: FangyuanIdentityResourceKind,
    },
    MissingMigrationSource {
        key: String,
    },
    BlueprintParse {
        key: String,
        message: String,
    },
    BlueprintValidation {
        key: String,
        source: FangyuanBlueprintValidationError,
    },
    SourceUpgrade(super::FangyuanBakeValidationError),
}

impl From<super::FangyuanBakeValidationError> for FangyuanEpochInheritanceError {
    fn from(source: super::FangyuanBakeValidationError) -> Self {
        Self::SourceUpgrade(source)
    }
}

impl fmt::Display for FangyuanEpochInheritanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EpochNotAdvanced {
                old_epoch,
                new_epoch,
            } => write!(
                formatter,
                "fangyuan epoch inheritance requires a newer epoch: old={old_epoch}, new={new_epoch}"
            ),
            Self::MissingBlueprintRefs => {
                write!(
                    formatter,
                    "fangyuan epoch inheritance has no old blueprint refs"
                )
            }
            Self::EmptyField { field } => {
                write!(
                    formatter,
                    "fangyuan epoch inheritance field {field} is empty"
                )
            }
            Self::UnexpectedResourceKind { id, kind } => write!(
                formatter,
                "fangyuan epoch inheritance expected blueprint identity for {id}, found {kind}"
            ),
            Self::MissingMigrationSource { key } => {
                write!(
                    formatter,
                    "fangyuan epoch inheritance missing source for {key}"
                )
            }
            Self::BlueprintParse { key, message } => write!(
                formatter,
                "fangyuan epoch inheritance failed to parse blueprint {key}: {message}"
            ),
            Self::BlueprintValidation { key, source } => write!(
                formatter,
                "fangyuan epoch inheritance failed to validate blueprint {key}: {source}"
            ),
            Self::SourceUpgrade(source) => write!(
                formatter,
                "fangyuan epoch inheritance source upgrade failed: {source}"
            ),
        }
    }
}

impl Error for FangyuanEpochInheritanceError {}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), FangyuanEpochInheritanceError> {
    if value.trim().is_empty() {
        Err(FangyuanEpochInheritanceError::EmptyField { field })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_BLUEPRINT_VERSION, FangyuanBlueprintBounds, FangyuanIdentityHashes,
        FangyuanIdentitySourceKind, FangyuanObjectBudgetEntry, FangyuanObjectClass,
        FangyuanPrimitiveBlueprint, FangyuanPrimitiveKind,
    };

    #[test]
    fn fangyuan_epoch_inheritance_input_covers_homes_equipment_skill_visuals_and_tiandao_archives()
    {
        let input = inheritance_input(identity("bp/home", "1", b"home"));

        input.validate().unwrap();

        assert_eq!(input.player_homes[0].home_id, "home-1");
        assert_eq!(input.equipment[0].equipment_id, "sword-1");
        assert_eq!(input.skill_visuals[0].visual_id, "skill-fire-visual");
        assert_eq!(input.tiandao_archives[0].archive_id, "archive-1");
    }

    #[test]
    fn fangyuan_epoch_inheritance_reaudits_legacy_blueprint_and_reports_budget_degrade() {
        let source = legacy_blueprint_source(3);
        let identity = identity("bp/legacy-home", "0", source.as_bytes());
        let input = inheritance_input(identity.clone());
        let profile = FangyuanEpochMigrationProfile {
            audit_budget: FangyuanAuditBudgetProfile {
                recommended_primitive_limit: 1,
                hard_primitive_limit: 8,
                ..Default::default()
            },
            object_budget: FangyuanObjectBudgetProfile {
                recommended_total_cost: 1,
                hard_total_cost: 16,
                ..Default::default()
            },
            object_snapshot: FangyuanObjectBudgetSnapshot::from_entries(vec![
                FangyuanObjectBudgetEntry::new("npc.decor", FangyuanObjectClass::Npc, 3),
            ]),
        };

        let report = migrate_fangyuan_epoch_inheritance(
            &input,
            &[FangyuanEpochMigrationSource {
                identity,
                ron_source: source,
            }],
            &profile,
        )
        .unwrap();

        let migrated = &report.migrated_blueprints[0];
        assert!(report.authority_reaudit_required);
        assert!(migrated.upgraded_from_legacy);
        assert_eq!(migrated.old_version, "0");
        assert_eq!(migrated.upgraded_version, FANGYUAN_BLUEPRINT_VERSION);
        assert_eq!(
            migrated.audit.status,
            FangyuanAuditStatus::PassedWithWarnings
        );
        assert!(
            migrated
                .degrade_suggestions
                .iter()
                .any(|suggestion| { suggestion.target == "blueprint.reduce_primitives" })
        );
        assert!(
            migrated
                .degrade_suggestions
                .iter()
                .any(|suggestion| { suggestion.target == "npc_decoration" })
        );
        assert!(report.has_budget_pressure());
    }

    #[test]
    fn fangyuan_epoch_inheritance_rejects_missing_source_and_non_advanced_epoch() {
        let identity = identity("bp/home", "1", b"home");
        let mut input = inheritance_input(identity.clone());
        input.new_epoch = input.old_epoch;

        assert!(matches!(
            migrate_fangyuan_epoch_inheritance(
                &input,
                &[],
                &FangyuanEpochMigrationProfile::default()
            ),
            Err(FangyuanEpochInheritanceError::EpochNotAdvanced { .. })
        ));

        input.new_epoch = input.old_epoch + 1;
        assert!(matches!(
            migrate_fangyuan_epoch_inheritance(
                &input,
                &[],
                &FangyuanEpochMigrationProfile::default()
            ),
            Err(FangyuanEpochInheritanceError::MissingMigrationSource { .. })
        ));
    }

    fn inheritance_input(identity: FangyuanBlueprintIdentity) -> FangyuanEpochInheritanceInput {
        FangyuanEpochInheritanceInput {
            old_epoch: 11,
            new_epoch: 12,
            old_world_blueprint_refs: vec![identity],
            player_homes: vec![FangyuanEpochPlayerHomeRef {
                player_id: "player-1".to_string(),
                home_id: "home-1".to_string(),
                blueprint_ref: "bp/home".to_string(),
            }],
            equipment: vec![FangyuanEpochEquipmentRef {
                owner_character_id: "character-1".to_string(),
                equipment_id: "sword-1".to_string(),
                blueprint_ref: "bp/sword".to_string(),
            }],
            skill_visuals: vec![FangyuanEpochSkillVisualRef {
                skill_id: "skill-fire".to_string(),
                visual_id: "skill-fire-visual".to_string(),
                template_version: 1,
            }],
            tiandao_archives: vec![FangyuanEpochTiandaoArchiveRef {
                archive_id: "archive-1".to_string(),
                solidified_by: "tiandao-rule".to_string(),
                blueprint_ref: Some("bp/archive".to_string()),
            }],
        }
    }

    fn legacy_blueprint_source(count: usize) -> String {
        let primitives = (0..count)
            .map(|index| {
                let x = index as f32 * 0.25;
                format!(
                    r#"(
            kind: "cube",
            position: [{x}, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [0.2, 0.4, 0.6, 1.0],
        )"#
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");

        format!(
            r#"(
    version: "0",
    name: "legacy_home",
    description: "legacy home",
    max_primitives: 16,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    primitives: [
{primitives}
    ],
)"#
        )
    }

    fn identity(id: &str, version: &str, source: &[u8]) -> FangyuanBlueprintIdentity {
        let content = FangyuanBlueprint {
            version: version.to_string(),
            name: id.to_string(),
            description: String::new(),
            max_primitives: 16,
            bounds: FangyuanBlueprintBounds::new(8.0, 8.0, 8.0),
            primitives: vec![FangyuanPrimitiveBlueprint::new(
                FangyuanPrimitiveKind::Cube,
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.2, 0.4, 0.6, 1.0],
            )],
        };
        let bytes = ron::ser::to_string(&content).unwrap();
        FangyuanBlueprintIdentity::new(
            FangyuanIdentityResourceKind::Blueprint,
            id,
            version,
            FangyuanIdentityHashes::from_bytes(
                FangyuanIdentityResourceKind::Blueprint,
                source,
                bytes.as_bytes(),
                &[],
            ),
            FangyuanIdentitySourceKind::RemoteAuthority,
        )
        .unwrap()
    }
}
