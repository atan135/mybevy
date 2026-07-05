use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

use super::{
    FangyuanBlueprintCacheEntry, FangyuanBlueprintIdentity, FangyuanIdentityResourceKind,
    FangyuanIdentitySourceKind,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthorityManifest {
    pub world_id: String,
    pub epoch: u64,
    pub manifest_version: u64,
    pub resources: Vec<FangyuanAuthorityResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audit_log: Vec<String>,
}

impl FangyuanAuthorityManifest {
    pub fn new(
        world_id: impl Into<String>,
        epoch: u64,
        manifest_version: u64,
        resources: Vec<FangyuanAuthorityResource>,
    ) -> Self {
        Self {
            world_id: world_id.into(),
            epoch,
            manifest_version,
            resources,
            audit_log: Vec::new(),
        }
    }

    pub fn resource(
        &self,
        kind: FangyuanIdentityResourceKind,
        id: &str,
    ) -> Option<&FangyuanAuthorityResource> {
        self.resources
            .iter()
            .find(|resource| resource.identity.kind == kind && resource.identity.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthorityResource {
    pub identity: FangyuanBlueprintIdentity,
    pub audit_result: FangyuanAuthorityAuditResult,
    pub can_override_client_cache: bool,
    pub reason: String,
}

impl FangyuanAuthorityResource {
    pub fn approved(identity: FangyuanBlueprintIdentity, reason: impl Into<String>) -> Self {
        Self {
            identity,
            audit_result: FangyuanAuthorityAuditResult::Approved,
            can_override_client_cache: true,
            reason: reason.into(),
        }
    }

    pub fn rejected(identity: FangyuanBlueprintIdentity, reason: impl Into<String>) -> Self {
        Self {
            identity,
            audit_result: FangyuanAuthorityAuditResult::Rejected,
            can_override_client_cache: true,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanAuthorityAuditResult {
    Approved,
    ApprovedWithWarnings,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanCacheAuthorityDecision {
    pub kind: FangyuanIdentityResourceKind,
    pub id: String,
    pub selected_identity: FangyuanBlueprintIdentity,
    pub source: FangyuanCacheAuthoritySource,
    pub cache_may_be_used_for_bytes: bool,
    pub cache_is_authoritative_audit: bool,
    pub audit_log: Vec<FangyuanCacheAuthorityAuditEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanCacheAuthoritySource {
    ServerManifest,
    ClientCacheBytesOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanCacheAuthorityAuditEvent {
    pub code: String,
    pub message: String,
}

impl FangyuanCacheAuthorityAuditEvent {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub fn resolve_fangyuan_cache_authority(
    kind: FangyuanIdentityResourceKind,
    id: &str,
    cache_entry: Option<&FangyuanBlueprintCacheEntry>,
    server_manifest: Option<&FangyuanAuthorityManifest>,
) -> Result<FangyuanCacheAuthorityDecision, FangyuanCacheAuthorityError> {
    let authority = server_manifest.and_then(|manifest| manifest.resource(kind, id));
    let mut audit_log = Vec::new();

    if let Some(authority) = authority {
        audit_log.push(FangyuanCacheAuthorityAuditEvent::new(
            "server_manifest_selected",
            format!(
                "server manifest selected {} and overrides client cache audit state",
                authority.identity.cache_key()
            ),
        ));
        if !authority.can_override_client_cache {
            return Err(FangyuanCacheAuthorityError::AuthorityOverrideDisabled {
                key: authority.identity.cache_key(),
            });
        }
        if authority.audit_result == FangyuanAuthorityAuditResult::Rejected {
            audit_log.push(FangyuanCacheAuthorityAuditEvent::new(
                "server_manifest_rejected",
                authority.reason.clone(),
            ));
            return Err(FangyuanCacheAuthorityError::AuthorityRejected {
                key: authority.identity.cache_key(),
                reason: authority.reason.clone(),
                audit_log,
            });
        }

        let cache_may_be_used_for_bytes = cache_entry
            .map(|entry| {
                entry.identity.matches_version_and_hash(
                    &authority.identity.version,
                    authority.identity.content_hash,
                )
            })
            .unwrap_or(false);
        if cache_entry.is_some() && !cache_may_be_used_for_bytes {
            audit_log.push(FangyuanCacheAuthorityAuditEvent::new(
                "cache_overridden",
                format!(
                    "cache identity for {} did not match server manifest version/hash",
                    authority.identity.cache_key()
                ),
            ));
        }

        return Ok(FangyuanCacheAuthorityDecision {
            kind,
            id: id.to_string(),
            selected_identity: authority.identity.clone(),
            source: FangyuanCacheAuthoritySource::ServerManifest,
            cache_may_be_used_for_bytes,
            cache_is_authoritative_audit: false,
            audit_log,
        });
    }

    let cache_entry = cache_entry.ok_or_else(|| FangyuanCacheAuthorityError::MissingResource {
        key: format!("{}:{id}", kind.as_str()),
    })?;
    audit_log.push(FangyuanCacheAuthorityAuditEvent::new(
        "cache_bytes_only",
        format!(
            "client cache can provide bytes for {}, but cannot provide authoritative audit approval",
            cache_entry.identity.cache_key()
        ),
    ));

    Ok(FangyuanCacheAuthorityDecision {
        kind,
        id: id.to_string(),
        selected_identity: cache_entry.identity.clone(),
        source: FangyuanCacheAuthoritySource::ClientCacheBytesOnly,
        cache_may_be_used_for_bytes: true,
        cache_is_authoritative_audit: false,
        audit_log,
    })
}

pub fn authority_identity_from_cache_entry(
    entry: &FangyuanBlueprintCacheEntry,
) -> FangyuanBlueprintIdentity {
    let mut identity = entry.identity.clone();
    identity.source_kind = FangyuanIdentitySourceKind::RemoteAuthority;
    identity
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanCacheAuthorityError {
    MissingResource {
        key: String,
    },
    AuthorityOverrideDisabled {
        key: String,
    },
    AuthorityRejected {
        key: String,
        reason: String,
        audit_log: Vec<FangyuanCacheAuthorityAuditEvent>,
    },
}

impl fmt::Display for FangyuanCacheAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingResource { key } => {
                write!(formatter, "fangyuan authority resource {key} is missing")
            }
            Self::AuthorityOverrideDisabled { key } => write!(
                formatter,
                "fangyuan authority manifest entry {key} cannot override client cache"
            ),
            Self::AuthorityRejected { key, reason, .. } => write!(
                formatter,
                "fangyuan authority manifest rejected {key}: {reason}"
            ),
        }
    }
}

impl Error for FangyuanCacheAuthorityError {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::framework::fangyuan::{FangyuanIdentityDependency, FangyuanIdentityHashes};

    #[test]
    fn fangyuan_cache_authority_treats_client_cache_as_bytes_not_audit_authority() {
        let cache_entry = cache_entry(identity(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/cache-only",
            "1",
            b"cache",
            FangyuanIdentitySourceKind::RuntimeCache,
        ));

        let decision = resolve_fangyuan_cache_authority(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/cache-only",
            Some(&cache_entry),
            None,
        )
        .unwrap();

        assert_eq!(
            decision.source,
            FangyuanCacheAuthoritySource::ClientCacheBytesOnly
        );
        assert!(decision.cache_may_be_used_for_bytes);
        assert!(!decision.cache_is_authoritative_audit);
        assert_eq!(decision.audit_log[0].code, "cache_bytes_only");
    }

    #[test]
    fn fangyuan_cache_authority_server_manifest_overrides_stale_cache_and_logs_audit() {
        let stale_cache = cache_entry(identity(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/home",
            "1",
            b"old",
            FangyuanIdentitySourceKind::RuntimeCache,
        ));
        let authority_identity = identity(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/home",
            "2",
            b"new",
            FangyuanIdentitySourceKind::RemoteAuthority,
        );
        let manifest = FangyuanAuthorityManifest::new(
            "world-main",
            8,
            12,
            vec![FangyuanAuthorityResource::approved(
                authority_identity.clone(),
                "server audit passed",
            )],
        );

        let decision = resolve_fangyuan_cache_authority(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/home",
            Some(&stale_cache),
            Some(&manifest),
        )
        .unwrap();

        assert_eq!(
            decision.source,
            FangyuanCacheAuthoritySource::ServerManifest
        );
        assert_eq!(decision.selected_identity, authority_identity);
        assert!(!decision.cache_may_be_used_for_bytes);
        assert!(
            decision
                .audit_log
                .iter()
                .any(|event| event.code == "cache_overridden")
        );
        assert!(!decision.cache_is_authoritative_audit);
    }

    #[test]
    fn fangyuan_cache_authority_rejects_authority_denial_with_audit_log() {
        let authority_identity = identity(
            FangyuanIdentityResourceKind::SkillVisual,
            "skill/fire.visual",
            "2",
            b"blocked",
            FangyuanIdentitySourceKind::RemoteAuthority,
        );
        let manifest = FangyuanAuthorityManifest::new(
            "world-main",
            8,
            13,
            vec![FangyuanAuthorityResource::rejected(
                authority_identity,
                "server audit found budget violation",
            )],
        );

        let error = resolve_fangyuan_cache_authority(
            FangyuanIdentityResourceKind::SkillVisual,
            "skill/fire.visual",
            None,
            Some(&manifest),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FangyuanCacheAuthorityError::AuthorityRejected { .. }
        ));
        if let FangyuanCacheAuthorityError::AuthorityRejected { audit_log, .. } = error {
            assert!(
                audit_log
                    .iter()
                    .any(|event| event.code == "server_manifest_rejected")
            );
        }
    }

    fn identity(
        kind: FangyuanIdentityResourceKind,
        id: &str,
        version: &str,
        payload: &[u8],
        source_kind: FangyuanIdentitySourceKind,
    ) -> FangyuanBlueprintIdentity {
        FangyuanBlueprintIdentity::new(
            kind,
            id,
            version,
            FangyuanIdentityHashes::from_bytes(kind, payload, payload, &[]),
            source_kind,
        )
        .unwrap()
    }

    fn cache_entry(identity: FangyuanBlueprintIdentity) -> FangyuanBlueprintCacheEntry {
        FangyuanBlueprintCacheEntry {
            relative_path: PathBuf::from("cache.bin"),
            content_hash: identity.content_hash,
            version: identity.version.clone(),
            size: 16,
            last_used: 1,
            use_count: 1,
            dependencies: Vec::<FangyuanIdentityDependency>::new(),
            source_kind: identity.source_kind,
            identity,
        }
    }
}
