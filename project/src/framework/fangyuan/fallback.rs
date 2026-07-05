use serde::{Deserialize, Serialize};

use super::FangyuanBlueprintIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanBlueprintFallbackDomain {
    Home,
    Equipment,
    Skill,
    Npc,
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanBlueprintMissingFallbackMode {
    DefaultAppearance,
    Marker,
    RuleOnly,
    Hidden,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanBlueprintFallbackRuleScope {
    VisualOnly,
    AuthorityRuleOnly,
    NoRuleScope,
    PendingAuthorityIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBlueprintFallbackPolicy {
    pub domain: FangyuanBlueprintFallbackDomain,
    pub mode: FangyuanBlueprintMissingFallbackMode,
    pub rule_scope: FangyuanBlueprintFallbackRuleScope,
    pub appearance_id: Option<String>,
    pub marker: Option<String>,
    pub blocks_gameplay_authority: bool,
    pub may_represent_real_rule_bounds: bool,
    pub recovery_key: String,
}

impl FangyuanBlueprintFallbackPolicy {
    pub fn for_missing_blueprint(
        domain: FangyuanBlueprintFallbackDomain,
        requested_id: impl Into<String>,
    ) -> Self {
        let requested_id = requested_id.into();
        match domain {
            FangyuanBlueprintFallbackDomain::Home => Self {
                domain,
                mode: FangyuanBlueprintMissingFallbackMode::Marker,
                rule_scope: FangyuanBlueprintFallbackRuleScope::VisualOnly,
                appearance_id: None,
                marker: Some("home_missing_blueprint_marker".to_string()),
                blocks_gameplay_authority: false,
                may_represent_real_rule_bounds: false,
                recovery_key: requested_id,
            },
            FangyuanBlueprintFallbackDomain::Equipment => Self {
                domain,
                mode: FangyuanBlueprintMissingFallbackMode::DefaultAppearance,
                rule_scope: FangyuanBlueprintFallbackRuleScope::VisualOnly,
                appearance_id: Some("equipment.default_practice_blade".to_string()),
                marker: Some("equipment_missing_visual_badge".to_string()),
                blocks_gameplay_authority: false,
                may_represent_real_rule_bounds: false,
                recovery_key: requested_id,
            },
            FangyuanBlueprintFallbackDomain::Skill => Self {
                domain,
                mode: FangyuanBlueprintMissingFallbackMode::RuleOnly,
                rule_scope: FangyuanBlueprintFallbackRuleScope::AuthorityRuleOnly,
                appearance_id: Some("skill.rule_layer_only".to_string()),
                marker: None,
                blocks_gameplay_authority: false,
                may_represent_real_rule_bounds: true,
                recovery_key: requested_id,
            },
            FangyuanBlueprintFallbackDomain::Npc => Self {
                domain,
                mode: FangyuanBlueprintMissingFallbackMode::Pending,
                rule_scope: FangyuanBlueprintFallbackRuleScope::PendingAuthorityIdentity,
                appearance_id: Some("npc.pending_nameplate_marker".to_string()),
                marker: Some("npc_missing_blueprint_marker".to_string()),
                blocks_gameplay_authority: false,
                may_represent_real_rule_bounds: false,
                recovery_key: requested_id,
            },
            FangyuanBlueprintFallbackDomain::Generic => Self {
                domain,
                mode: FangyuanBlueprintMissingFallbackMode::Hidden,
                rule_scope: FangyuanBlueprintFallbackRuleScope::NoRuleScope,
                appearance_id: None,
                marker: None,
                blocks_gameplay_authority: false,
                may_represent_real_rule_bounds: false,
                recovery_key: requested_id,
            },
        }
    }

    pub fn recover(
        &self,
        loaded_identity: Option<FangyuanBlueprintIdentity>,
    ) -> FangyuanBlueprintFallbackRecovery {
        match loaded_identity {
            Some(identity) if identity.id == self.recovery_key => {
                FangyuanBlueprintFallbackRecovery::Recovered { identity }
            }
            Some(identity) => FangyuanBlueprintFallbackRecovery::IdentityMismatch {
                expected_id: self.recovery_key.clone(),
                actual_id: identity.id,
            },
            None => FangyuanBlueprintFallbackRecovery::StillPending {
                policy: self.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanBlueprintFallbackRecovery {
    StillPending {
        policy: FangyuanBlueprintFallbackPolicy,
    },
    Recovered {
        identity: FangyuanBlueprintIdentity,
    },
    IdentityMismatch {
        expected_id: String,
        actual_id: String,
    },
}

pub fn fangyuan_missing_blueprint_fallback(
    domain: FangyuanBlueprintFallbackDomain,
    requested_id: impl Into<String>,
) -> FangyuanBlueprintFallbackPolicy {
    FangyuanBlueprintFallbackPolicy::for_missing_blueprint(domain, requested_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FangyuanBlueprintIdentity, FangyuanIdentityHashes, FangyuanIdentityResourceKind,
        FangyuanIdentitySourceKind,
    };

    #[test]
    fn fangyuan_blueprint_fallback_defines_distinct_home_equipment_skill_and_npc_policies() {
        let home =
            fangyuan_missing_blueprint_fallback(FangyuanBlueprintFallbackDomain::Home, "home/tree");
        let equipment = fangyuan_missing_blueprint_fallback(
            FangyuanBlueprintFallbackDomain::Equipment,
            "equipment/sword",
        );
        let skill = fangyuan_missing_blueprint_fallback(
            FangyuanBlueprintFallbackDomain::Skill,
            "skill/fireball",
        );
        let npc =
            fangyuan_missing_blueprint_fallback(FangyuanBlueprintFallbackDomain::Npc, "npc/guard");

        assert_eq!(home.mode, FangyuanBlueprintMissingFallbackMode::Marker);
        assert_eq!(
            home.rule_scope,
            FangyuanBlueprintFallbackRuleScope::VisualOnly
        );
        assert!(!home.may_represent_real_rule_bounds);

        assert_eq!(
            equipment.mode,
            FangyuanBlueprintMissingFallbackMode::DefaultAppearance
        );
        assert_eq!(
            equipment.appearance_id.as_deref(),
            Some("equipment.default_practice_blade")
        );
        assert!(!equipment.may_represent_real_rule_bounds);

        assert_eq!(skill.mode, FangyuanBlueprintMissingFallbackMode::RuleOnly);
        assert_eq!(
            skill.rule_scope,
            FangyuanBlueprintFallbackRuleScope::AuthorityRuleOnly
        );
        assert!(skill.may_represent_real_rule_bounds);

        assert_eq!(npc.mode, FangyuanBlueprintMissingFallbackMode::Pending);
        assert_eq!(
            npc.rule_scope,
            FangyuanBlueprintFallbackRuleScope::PendingAuthorityIdentity
        );
        assert!(!npc.may_represent_real_rule_bounds);
    }

    #[test]
    fn fangyuan_blueprint_fallback_hidden_state_is_available_for_unknown_domains() {
        let fallback = fangyuan_missing_blueprint_fallback(
            FangyuanBlueprintFallbackDomain::Generic,
            "unknown/asset",
        );

        assert_eq!(fallback.mode, FangyuanBlueprintMissingFallbackMode::Hidden);
        assert_eq!(
            fallback.rule_scope,
            FangyuanBlueprintFallbackRuleScope::NoRuleScope
        );
        assert_eq!(fallback.appearance_id, None);
        assert_eq!(fallback.marker, None);
    }

    #[test]
    fn fangyuan_blueprint_fallback_recovers_when_expected_identity_arrives() {
        let fallback =
            fangyuan_missing_blueprint_fallback(FangyuanBlueprintFallbackDomain::Home, "home/tree");
        assert!(matches!(
            fallback.recover(None),
            FangyuanBlueprintFallbackRecovery::StillPending { .. }
        ));

        let identity = FangyuanBlueprintIdentity::new(
            FangyuanIdentityResourceKind::Blueprint,
            "home/tree",
            "1",
            FangyuanIdentityHashes::from_bytes(
                FangyuanIdentityResourceKind::Blueprint,
                b"source",
                b"content",
                &[],
            ),
            FangyuanIdentitySourceKind::Downloaded,
        )
        .unwrap();

        assert!(matches!(
            fallback.recover(Some(identity)),
            FangyuanBlueprintFallbackRecovery::Recovered { .. }
        ));
    }

    #[test]
    fn fangyuan_blueprint_fallback_rejects_recovery_with_wrong_identity() {
        let fallback =
            fangyuan_missing_blueprint_fallback(FangyuanBlueprintFallbackDomain::Npc, "npc/guard");
        let identity = FangyuanBlueprintIdentity::new(
            FangyuanIdentityResourceKind::Blueprint,
            "npc/merchant",
            "1",
            FangyuanIdentityHashes::from_bytes(
                FangyuanIdentityResourceKind::Blueprint,
                b"source",
                b"content",
                &[],
            ),
            FangyuanIdentitySourceKind::Downloaded,
        )
        .unwrap();

        assert_eq!(
            fallback.recover(Some(identity)),
            FangyuanBlueprintFallbackRecovery::IdentityMismatch {
                expected_id: "npc/guard".to_string(),
                actual_id: "npc/merchant".to_string(),
            }
        );
    }
}
