use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, error::Error, fmt};

use super::{
    FANGYUAN_BAKE_SCHEMA_VERSION, FangyuanBlueprintIdentity, FangyuanIdentityDependency,
    FangyuanIdentityResourceKind, fangyuan_bake_hash_bytes, fangyuan_identity_dependency_hash,
};

pub const FANGYUAN_STREAMING_UPDATE_MANIFEST_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanStreamingUpdateManifest {
    pub manifest_version: u16,
    pub update_id: String,
    pub world_id: String,
    pub from_epoch: u64,
    pub to_epoch: u64,
    pub package_version: u64,
    pub entries: Vec<FangyuanStreamingUpdateEntry>,
    pub budget_summary: FangyuanStreamingUpdateBudgetSummary,
    pub signature: FangyuanStreamingUpdateSignature,
    pub permissions: FangyuanStreamingUpdatePermissions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audit_log: Vec<String>,
}

impl FangyuanStreamingUpdateManifest {
    pub fn new(
        update_id: impl Into<String>,
        world_id: impl Into<String>,
        from_epoch: u64,
        to_epoch: u64,
        package_version: u64,
        entries: Vec<FangyuanStreamingUpdateEntry>,
        budget_summary: FangyuanStreamingUpdateBudgetSummary,
    ) -> Self {
        Self {
            manifest_version: FANGYUAN_STREAMING_UPDATE_MANIFEST_VERSION,
            update_id: update_id.into(),
            world_id: world_id.into(),
            from_epoch,
            to_epoch,
            package_version,
            entries,
            budget_summary,
            signature: FangyuanStreamingUpdateSignature::placeholder("authority-placeholder"),
            permissions: FangyuanStreamingUpdatePermissions::authority_placeholder(
                "fangyuan.patch.install",
            ),
            audit_log: Vec::new(),
        }
    }

    pub fn to_ron_string(&self) -> Result<String, FangyuanStreamingUpdateValidationError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|source| {
            FangyuanStreamingUpdateValidationError::ManifestSerialize {
                message: source.to_string(),
            }
        })
    }

    pub fn from_ron_str(source: &str) -> Result<Self, FangyuanStreamingUpdateValidationError> {
        ron::from_str::<Self>(source).map_err(|source| {
            FangyuanStreamingUpdateValidationError::ManifestParse {
                message: source.to_string(),
            }
        })
    }

    pub fn validate(
        &self,
        installed: &FangyuanInstalledResourceIndex,
    ) -> Result<(), FangyuanStreamingUpdateValidationError> {
        if self.manifest_version != FANGYUAN_STREAMING_UPDATE_MANIFEST_VERSION {
            return Err(
                FangyuanStreamingUpdateValidationError::ManifestVersionMismatch {
                    expected: FANGYUAN_STREAMING_UPDATE_MANIFEST_VERSION,
                    actual: self.manifest_version,
                },
            );
        }
        validate_non_empty("update_id", &self.update_id)?;
        validate_non_empty("world_id", &self.world_id)?;
        if self.to_epoch < self.from_epoch {
            return Err(FangyuanStreamingUpdateValidationError::EpochRollback {
                from_epoch: self.from_epoch,
                to_epoch: self.to_epoch,
            });
        }
        if self.package_version <= installed.package_version {
            return Err(
                FangyuanStreamingUpdateValidationError::PackageVersionRollback {
                    installed: installed.package_version,
                    update: self.package_version,
                },
            );
        }
        self.signature.validate_placeholder()?;
        self.permissions.validate_placeholder()?;
        self.budget_summary.validate()?;

        let mut keys = BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !keys.insert(entry.identity.cache_key()) {
                return Err(FangyuanStreamingUpdateValidationError::DuplicateEntry {
                    key: entry.identity.cache_key(),
                });
            }
            if let Some(installed_entry) = installed.resource(&entry.identity.cache_key()) {
                if version_is_rollback(&entry.identity.version, &installed_entry.version) {
                    return Err(
                        FangyuanStreamingUpdateValidationError::ResourceVersionRollback {
                            key: entry.identity.cache_key(),
                            installed: installed_entry.version.clone(),
                            update: entry.identity.version.clone(),
                        },
                    );
                }
            }
            let payload_hash = fangyuan_bake_hash_bytes(&entry.payload);
            if payload_hash != entry.identity.content_hash {
                return Err(FangyuanStreamingUpdateValidationError::HashMismatch {
                    key: entry.identity.cache_key(),
                    expected: entry.identity.content_hash,
                    actual: payload_hash,
                });
            }
            if entry.dependency_hash != fangyuan_identity_dependency_hash(&entry.dependencies) {
                return Err(
                    FangyuanStreamingUpdateValidationError::DependencyHashMismatch {
                        key: entry.identity.cache_key(),
                        expected: entry.dependency_hash,
                        actual: fangyuan_identity_dependency_hash(&entry.dependencies),
                    },
                );
            }
        }

        let available_after_install = installed.keys_after_applying(&self.entries);
        for entry in &self.entries {
            for dependency in &entry.dependencies {
                let key = dependency.cache_key();
                if !available_after_install.contains(&key) {
                    return Err(FangyuanStreamingUpdateValidationError::MissingDependency {
                        owner: entry.identity.cache_key(),
                        dependency: key,
                    });
                }
                if let Some(expected_version) = dependency.version.as_deref() {
                    let actual = self
                        .entries
                        .iter()
                        .find(|candidate| candidate.identity.cache_key() == key)
                        .map(|candidate| candidate.identity.version.as_str())
                        .or_else(|| {
                            installed
                                .resource(&key)
                                .map(|resource| resource.version.as_str())
                        });
                    if actual != Some(expected_version) {
                        return Err(
                            FangyuanStreamingUpdateValidationError::DependencyVersionMismatch {
                                owner: entry.identity.cache_key(),
                                dependency: key,
                                expected: expected_version.to_string(),
                                actual: actual.unwrap_or("<missing>").to_string(),
                            },
                        );
                    }
                }
                if let Some(expected_hash) = dependency.content_hash {
                    let actual = self
                        .entries
                        .iter()
                        .find(|candidate| candidate.identity.cache_key() == key)
                        .map(|candidate| candidate.identity.content_hash)
                        .or_else(|| {
                            installed
                                .resource(&key)
                                .map(|resource| resource.content_hash)
                        });
                    if actual != Some(expected_hash) {
                        return Err(
                            FangyuanStreamingUpdateValidationError::DependencyHashMismatch {
                                key: format!("{} -> {key}", entry.identity.cache_key()),
                                expected: expected_hash,
                                actual: actual.unwrap_or_default(),
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn install_plan(
        &self,
        installed: &FangyuanInstalledResourceIndex,
    ) -> Result<FangyuanStreamingUpdateInstallPlan, FangyuanStreamingUpdateValidationError> {
        self.validate(installed)?;

        let mut plan = FangyuanStreamingUpdateInstallPlan {
            update_id: self.update_id.clone(),
            world_id: self.world_id.clone(),
            from_epoch: self.from_epoch,
            to_epoch: self.to_epoch,
            package_version: self.package_version,
            actions: Vec::new(),
            rollback_keys: Vec::new(),
            audit_log: vec![format!(
                "validated update={} package_version={} authority_placeholder_signature={}",
                self.update_id, self.package_version, self.signature.key_id
            )],
        };

        for entry in &self.entries {
            let key = entry.identity.cache_key();
            let existing = installed.resource(&key);
            if existing
                .map(|existing| existing.content_hash == entry.identity.content_hash)
                .unwrap_or(false)
            {
                plan.actions.push(FangyuanStreamingUpdateAction {
                    key,
                    kind: entry.identity.kind,
                    id: entry.identity.id.clone(),
                    operation: FangyuanStreamingUpdateOperation::Keep,
                    affected_chunks: Vec::new(),
                    affected_prefabs: Vec::new(),
                    affected_blueprints: Vec::new(),
                });
                continue;
            }

            if existing.is_some() {
                plan.rollback_keys.push(key.clone());
            }

            plan.actions.push(FangyuanStreamingUpdateAction {
                key,
                kind: entry.identity.kind,
                id: entry.identity.id.clone(),
                operation: if existing.is_some() {
                    FangyuanStreamingUpdateOperation::Replace
                } else {
                    FangyuanStreamingUpdateOperation::Install
                },
                affected_chunks: entry.affected_chunks.clone(),
                affected_prefabs: entry.affected_prefabs.clone(),
                affected_blueprints: entry.affected_blueprints.clone(),
            });
        }

        Ok(plan)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanStreamingUpdateEntry {
    pub identity: FangyuanBlueprintIdentity,
    pub artifact_schema_version: u16,
    pub payload: Vec<u8>,
    pub dependencies: Vec<FangyuanIdentityDependency>,
    pub dependency_hash: u64,
    pub budget_summary: FangyuanStreamingUpdateBudgetSummary,
    pub audit_status: FangyuanStreamingUpdateAuditStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_chunks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_prefabs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_blueprints: Vec<String>,
}

impl FangyuanStreamingUpdateEntry {
    pub fn new(
        identity: FangyuanBlueprintIdentity,
        payload: Vec<u8>,
        dependencies: Vec<FangyuanIdentityDependency>,
        budget_summary: FangyuanStreamingUpdateBudgetSummary,
    ) -> Self {
        Self {
            dependency_hash: fangyuan_identity_dependency_hash(&dependencies),
            identity,
            artifact_schema_version: FANGYUAN_BAKE_SCHEMA_VERSION,
            payload,
            dependencies,
            budget_summary,
            audit_status: FangyuanStreamingUpdateAuditStatus::Passed,
            affected_chunks: Vec::new(),
            affected_prefabs: Vec::new(),
            affected_blueprints: Vec::new(),
        }
    }

    pub fn with_impacted_chunks(
        mut self,
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.affected_chunks = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_impacted_prefabs(
        mut self,
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.affected_prefabs = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_impacted_blueprints(
        mut self,
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.affected_blueprints = ids.into_iter().map(Into::into).collect();
        self
    }

    fn validate(&self) -> Result<(), FangyuanStreamingUpdateValidationError> {
        if self.artifact_schema_version != FANGYUAN_BAKE_SCHEMA_VERSION {
            return Err(
                FangyuanStreamingUpdateValidationError::ArtifactSchemaVersionMismatch {
                    key: self.identity.cache_key(),
                    expected: FANGYUAN_BAKE_SCHEMA_VERSION,
                    actual: self.artifact_schema_version,
                },
            );
        }
        if self.audit_status == FangyuanStreamingUpdateAuditStatus::Failed {
            return Err(FangyuanStreamingUpdateValidationError::AuditFailed {
                key: self.identity.cache_key(),
            });
        }
        if self.payload.is_empty() {
            return Err(FangyuanStreamingUpdateValidationError::EmptyPayload {
                key: self.identity.cache_key(),
            });
        }
        if self.affected_chunks.is_empty()
            && self.affected_prefabs.is_empty()
            && self.affected_blueprints.is_empty()
        {
            return Err(
                FangyuanStreamingUpdateValidationError::MissingPartialImpactScope {
                    key: self.identity.cache_key(),
                },
            );
        }
        self.budget_summary.validate()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanStreamingUpdateAuditStatus {
    #[default]
    Passed,
    PassedWithWarnings,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanStreamingUpdateBudgetSummary {
    pub primitive_count: usize,
    pub prefab_count: usize,
    pub chunk_count: usize,
    pub material_profile_count: usize,
    pub skill_visual_count: usize,
    pub estimated_bytes: u64,
}

impl FangyuanStreamingUpdateBudgetSummary {
    pub const fn new(
        primitive_count: usize,
        prefab_count: usize,
        chunk_count: usize,
        material_profile_count: usize,
        skill_visual_count: usize,
        estimated_bytes: u64,
    ) -> Self {
        Self {
            primitive_count,
            prefab_count,
            chunk_count,
            material_profile_count,
            skill_visual_count,
            estimated_bytes,
        }
    }

    fn validate(&self) -> Result<(), FangyuanStreamingUpdateValidationError> {
        if self.estimated_bytes == 0 {
            return Err(FangyuanStreamingUpdateValidationError::EmptyBudgetSummary);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanStreamingUpdateSignature {
    pub scheme: String,
    pub key_id: String,
    pub digest: String,
}

impl FangyuanStreamingUpdateSignature {
    pub fn placeholder(key_id: impl Into<String>) -> Self {
        Self {
            scheme: "placeholder-audit-only".to_string(),
            key_id: key_id.into(),
            digest: "pending-real-signature".to_string(),
        }
    }

    fn validate_placeholder(&self) -> Result<(), FangyuanStreamingUpdateValidationError> {
        if self.scheme != "placeholder-audit-only"
            || self.key_id.trim().is_empty()
            || self.digest.trim().is_empty()
        {
            return Err(FangyuanStreamingUpdateValidationError::InvalidSignaturePlaceholder);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanStreamingUpdatePermissions {
    pub authority: String,
    pub scopes: Vec<String>,
}

impl FangyuanStreamingUpdatePermissions {
    pub fn authority_placeholder(scope: impl Into<String>) -> Self {
        Self {
            authority: "server-authority-placeholder".to_string(),
            scopes: vec![scope.into()],
        }
    }

    fn validate_placeholder(&self) -> Result<(), FangyuanStreamingUpdateValidationError> {
        if self.authority.trim().is_empty()
            || !self
                .scopes
                .iter()
                .any(|scope| scope == "fangyuan.patch.install")
        {
            return Err(FangyuanStreamingUpdateValidationError::MissingPermissionPlaceholder);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanInstalledResourceIndex {
    pub package_version: u64,
    resources: Vec<FangyuanInstalledResource>,
}

impl FangyuanInstalledResourceIndex {
    pub fn new(package_version: u64) -> Self {
        Self {
            package_version,
            resources: Vec::new(),
        }
    }

    pub fn with_resource(mut self, identity: FangyuanBlueprintIdentity) -> Self {
        self.resources.push(FangyuanInstalledResource {
            key: identity.cache_key(),
            version: identity.version,
            content_hash: identity.content_hash,
        });
        self
    }

    pub fn resource(&self, key: &str) -> Option<&FangyuanInstalledResource> {
        self.resources.iter().find(|resource| resource.key == key)
    }

    fn keys_after_applying(&self, entries: &[FangyuanStreamingUpdateEntry]) -> BTreeSet<String> {
        self.resources
            .iter()
            .map(|resource| resource.key.clone())
            .chain(entries.iter().map(|entry| entry.identity.cache_key()))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanInstalledResource {
    pub key: String,
    pub version: String,
    pub content_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStreamingUpdateInstallPlan {
    pub update_id: String,
    pub world_id: String,
    pub from_epoch: u64,
    pub to_epoch: u64,
    pub package_version: u64,
    pub actions: Vec<FangyuanStreamingUpdateAction>,
    pub rollback_keys: Vec<String>,
    pub audit_log: Vec<String>,
}

impl FangyuanStreamingUpdateInstallPlan {
    pub fn changed_keys(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter(|action| action.operation != FangyuanStreamingUpdateOperation::Keep)
            .map(|action| action.key.clone())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStreamingUpdateAction {
    pub key: String,
    pub kind: FangyuanIdentityResourceKind,
    pub id: String,
    pub operation: FangyuanStreamingUpdateOperation,
    pub affected_chunks: Vec<String>,
    pub affected_prefabs: Vec<String>,
    pub affected_blueprints: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanStreamingUpdateOperation {
    Install,
    Replace,
    Keep,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanStreamingUpdateValidationError {
    ManifestVersionMismatch {
        expected: u16,
        actual: u16,
    },
    ArtifactSchemaVersionMismatch {
        key: String,
        expected: u16,
        actual: u16,
    },
    EpochRollback {
        from_epoch: u64,
        to_epoch: u64,
    },
    PackageVersionRollback {
        installed: u64,
        update: u64,
    },
    ResourceVersionRollback {
        key: String,
        installed: String,
        update: String,
    },
    EmptyField {
        field: &'static str,
    },
    EmptyPayload {
        key: String,
    },
    EmptyBudgetSummary,
    AuditFailed {
        key: String,
    },
    HashMismatch {
        key: String,
        expected: u64,
        actual: u64,
    },
    DependencyHashMismatch {
        key: String,
        expected: u64,
        actual: u64,
    },
    MissingDependency {
        owner: String,
        dependency: String,
    },
    DependencyVersionMismatch {
        owner: String,
        dependency: String,
        expected: String,
        actual: String,
    },
    DuplicateEntry {
        key: String,
    },
    MissingPartialImpactScope {
        key: String,
    },
    InvalidSignaturePlaceholder,
    MissingPermissionPlaceholder,
    ManifestSerialize {
        message: String,
    },
    ManifestParse {
        message: String,
    },
}

impl fmt::Display for FangyuanStreamingUpdateValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestVersionMismatch { expected, actual } => write!(
                formatter,
                "fangyuan streaming update manifest version mismatch: expected {expected}, actual {actual}"
            ),
            Self::ArtifactSchemaVersionMismatch {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "fangyuan streaming update artifact schema mismatch for {key}: expected {expected}, actual {actual}"
            ),
            Self::EpochRollback {
                from_epoch,
                to_epoch,
            } => write!(
                formatter,
                "fangyuan streaming update epoch rollback is not allowed: from={from_epoch}, to={to_epoch}"
            ),
            Self::PackageVersionRollback { installed, update } => write!(
                formatter,
                "fangyuan streaming update package version rollback is not allowed: installed={installed}, update={update}"
            ),
            Self::ResourceVersionRollback {
                key,
                installed,
                update,
            } => write!(
                formatter,
                "fangyuan streaming update resource version rollback is not allowed for {key}: installed={installed}, update={update}"
            ),
            Self::EmptyField { field } => {
                write!(
                    formatter,
                    "fangyuan streaming update field {field} is empty"
                )
            }
            Self::EmptyPayload { key } => {
                write!(
                    formatter,
                    "fangyuan streaming update payload for {key} is empty"
                )
            }
            Self::EmptyBudgetSummary => write!(
                formatter,
                "fangyuan streaming update budget summary must carry a non-zero byte estimate"
            ),
            Self::AuditFailed { key } => write!(
                formatter,
                "fangyuan streaming update entry {key} cannot be installed because audit failed"
            ),
            Self::HashMismatch {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "fangyuan streaming update hash mismatch for {key}: expected {expected:016x}, actual {actual:016x}"
            ),
            Self::DependencyHashMismatch {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "fangyuan streaming update dependency hash mismatch for {key}: expected {expected:016x}, actual {actual:016x}"
            ),
            Self::MissingDependency { owner, dependency } => write!(
                formatter,
                "fangyuan streaming update dependency missing for {owner}: {dependency}"
            ),
            Self::DependencyVersionMismatch {
                owner,
                dependency,
                expected,
                actual,
            } => write!(
                formatter,
                "fangyuan streaming update dependency version mismatch for {owner}: {dependency} expected {expected}, actual {actual}"
            ),
            Self::DuplicateEntry { key } => {
                write!(
                    formatter,
                    "fangyuan streaming update has duplicate entry {key}"
                )
            }
            Self::MissingPartialImpactScope { key } => write!(
                formatter,
                "fangyuan streaming update entry {key} must name affected chunk, prefab, or blueprint ids"
            ),
            Self::InvalidSignaturePlaceholder => write!(
                formatter,
                "fangyuan streaming update signature placeholder is invalid"
            ),
            Self::MissingPermissionPlaceholder => write!(
                formatter,
                "fangyuan streaming update permission placeholder is missing install scope"
            ),
            Self::ManifestSerialize { message } => write!(
                formatter,
                "fangyuan streaming update manifest serialize failed: {message}"
            ),
            Self::ManifestParse { message } => write!(
                formatter,
                "fangyuan streaming update manifest parse failed: {message}"
            ),
        }
    }
}

impl Error for FangyuanStreamingUpdateValidationError {}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), FangyuanStreamingUpdateValidationError> {
    if value.trim().is_empty() {
        Err(FangyuanStreamingUpdateValidationError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn version_is_rollback(update: &str, installed: &str) -> bool {
    match (
        parse_numeric_version(update),
        parse_numeric_version(installed),
    ) {
        (Some(update), Some(installed)) => update < installed,
        _ => update < installed,
    }
}

fn parse_numeric_version(value: &str) -> Option<Vec<u64>> {
    value
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_BLUEPRINT_VERSION, FangyuanIdentityHashes, FangyuanIdentitySourceKind,
    };

    #[test]
    fn fangyuan_streaming_update_manifest_covers_all_online_package_kinds_and_partial_plan() {
        let installed_prefab = identity(
            FangyuanIdentityResourceKind::Prefab,
            "prefab/tree",
            "1",
            b"old-prefab",
        );
        let installed = FangyuanInstalledResourceIndex::new(7).with_resource(installed_prefab);

        let material_payload = b"material-v2".to_vec();
        let skill_payload = b"skill-visual-v2".to_vec();
        let blueprint_payload = b"blueprint-v2".to_vec();
        let prefab_payload = b"prefab-v2".to_vec();
        let chunk_payload = b"chunk-v2".to_vec();

        let material_identity = identity(
            FangyuanIdentityResourceKind::MaterialProfile,
            "mat/stone",
            "2",
            &material_payload,
        );
        let skill_identity = identity(
            FangyuanIdentityResourceKind::SkillVisual,
            "skill/flame.visual",
            "2",
            &skill_payload,
        );
        let blueprint_identity = identity(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/tower",
            "2",
            &blueprint_payload,
        );
        let prefab_identity = identity(
            FangyuanIdentityResourceKind::Prefab,
            "prefab/tree",
            "2",
            &prefab_payload,
        );
        let chunk_identity = identity(
            FangyuanIdentityResourceKind::Chunk,
            "chunk/aoi_0_0",
            "2",
            &chunk_payload,
        );

        let summary = FangyuanStreamingUpdateBudgetSummary::new(3, 1, 1, 1, 1, 128);
        let entries = vec![
            FangyuanStreamingUpdateEntry::new(
                material_identity.clone(),
                material_payload,
                Vec::new(),
                summary.clone(),
            )
            .with_impacted_blueprints(["bp/tower"]),
            FangyuanStreamingUpdateEntry::new(
                skill_identity,
                skill_payload,
                vec![
                    FangyuanIdentityDependency::new(
                        FangyuanIdentityResourceKind::MaterialProfile,
                        "mat/stone",
                    )
                    .with_version("2")
                    .with_content_hash(material_identity.content_hash),
                ],
                summary.clone(),
            )
            .with_impacted_blueprints(["bp/tower"]),
            FangyuanStreamingUpdateEntry::new(
                blueprint_identity.clone(),
                blueprint_payload,
                vec![
                    FangyuanIdentityDependency::new(
                        FangyuanIdentityResourceKind::MaterialProfile,
                        "mat/stone",
                    )
                    .with_version("2")
                    .with_content_hash(material_identity.content_hash),
                ],
                summary.clone(),
            )
            .with_impacted_blueprints(["bp/tower"]),
            FangyuanStreamingUpdateEntry::new(
                prefab_identity,
                prefab_payload,
                vec![
                    FangyuanIdentityDependency::new(
                        FangyuanIdentityResourceKind::Blueprint,
                        "bp/tower",
                    )
                    .with_version("2")
                    .with_content_hash(blueprint_identity.content_hash),
                ],
                summary.clone(),
            )
            .with_impacted_prefabs(["prefab/tree"]),
            FangyuanStreamingUpdateEntry::new(
                chunk_identity,
                chunk_payload,
                vec![
                    FangyuanIdentityDependency::new(
                        FangyuanIdentityResourceKind::Prefab,
                        "prefab/tree",
                    )
                    .with_version("2"),
                ],
                summary.clone(),
            )
            .with_impacted_chunks(["chunk/aoi_0_0"]),
        ];

        let manifest = FangyuanStreamingUpdateManifest::new(
            "update-2026-07-05",
            "world-main",
            12,
            13,
            8,
            entries,
            summary,
        );

        let plan = manifest.install_plan(&installed).unwrap();

        assert_eq!(
            plan.actions
                .iter()
                .map(|action| action.kind)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                FangyuanIdentityResourceKind::Chunk,
                FangyuanIdentityResourceKind::Prefab,
                FangyuanIdentityResourceKind::Blueprint,
                FangyuanIdentityResourceKind::MaterialProfile,
                FangyuanIdentityResourceKind::SkillVisual,
            ])
        );
        assert_eq!(
            plan.actions
                .iter()
                .find(|action| action.kind == FangyuanIdentityResourceKind::Chunk)
                .unwrap()
                .affected_chunks,
            vec!["chunk/aoi_0_0"]
        );
        assert!(
            plan.rollback_keys
                .contains(&"prefab:prefab/tree".to_string())
        );
        assert!(!plan.changed_keys().contains(&"chunk:unrelated".to_string()));
    }

    #[test]
    fn fangyuan_streaming_update_rejects_missing_dependency_version_rollback_and_bad_hash() {
        let installed = FangyuanInstalledResourceIndex::new(4).with_resource(identity(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/old",
            "3",
            b"old",
        ));
        let summary = FangyuanStreamingUpdateBudgetSummary::new(1, 0, 0, 0, 0, 16);
        let payload = b"new".to_vec();
        let mut entry = FangyuanStreamingUpdateEntry::new(
            identity(
                FangyuanIdentityResourceKind::Blueprint,
                "bp/old",
                "2",
                &payload,
            ),
            payload,
            Vec::new(),
            summary.clone(),
        )
        .with_impacted_blueprints(["bp/old"]);
        let manifest = FangyuanStreamingUpdateManifest::new(
            "rollback",
            "world",
            1,
            2,
            5,
            vec![entry.clone()],
            summary.clone(),
        );

        assert!(matches!(
            manifest.validate(&installed),
            Err(FangyuanStreamingUpdateValidationError::ResourceVersionRollback { .. })
        ));

        entry.identity = identity(
            FangyuanIdentityResourceKind::Blueprint,
            "bp/new",
            "5",
            b"different",
        );
        let bad_hash_manifest = FangyuanStreamingUpdateManifest::new(
            "bad-hash",
            "world",
            1,
            2,
            5,
            vec![entry.clone()],
            summary.clone(),
        );
        assert!(matches!(
            bad_hash_manifest.validate(&FangyuanInstalledResourceIndex::new(4)),
            Err(FangyuanStreamingUpdateValidationError::HashMismatch { .. })
        ));

        let missing_dependency = FangyuanStreamingUpdateEntry::new(
            identity(
                FangyuanIdentityResourceKind::Chunk,
                "chunk/needs-prefab",
                "5",
                b"chunk",
            ),
            b"chunk".to_vec(),
            vec![FangyuanIdentityDependency::new(
                FangyuanIdentityResourceKind::Prefab,
                "prefab/missing",
            )],
            summary.clone(),
        )
        .with_impacted_chunks(["chunk/needs-prefab"]);
        let missing_manifest = FangyuanStreamingUpdateManifest::new(
            "missing",
            "world",
            1,
            2,
            5,
            vec![missing_dependency],
            summary,
        );
        assert!(matches!(
            missing_manifest.validate(&FangyuanInstalledResourceIndex::new(4)),
            Err(FangyuanStreamingUpdateValidationError::MissingDependency { .. })
        ));
    }

    #[test]
    fn fangyuan_streaming_update_rejects_missing_signature_permission_budget_and_rollback() {
        let payload = b"blueprint".to_vec();
        let summary = FangyuanStreamingUpdateBudgetSummary::new(1, 0, 0, 0, 0, 16);
        let entry = FangyuanStreamingUpdateEntry::new(
            identity(
                FangyuanIdentityResourceKind::Blueprint,
                "bp/one",
                FANGYUAN_BLUEPRINT_VERSION,
                &payload,
            ),
            payload,
            Vec::new(),
            summary.clone(),
        )
        .with_impacted_blueprints(["bp/one"]);
        let installed = FangyuanInstalledResourceIndex::new(9);
        let mut manifest = FangyuanStreamingUpdateManifest::new(
            "bad",
            "world",
            3,
            2,
            10,
            vec![entry.clone()],
            summary.clone(),
        );

        assert!(matches!(
            manifest.validate(&installed),
            Err(FangyuanStreamingUpdateValidationError::EpochRollback { .. })
        ));

        manifest.to_epoch = 4;
        manifest.package_version = 9;
        assert!(matches!(
            manifest.validate(&installed),
            Err(FangyuanStreamingUpdateValidationError::PackageVersionRollback { .. })
        ));

        manifest.package_version = 10;
        manifest.budget_summary.estimated_bytes = 0;
        assert!(matches!(
            manifest.validate(&installed),
            Err(FangyuanStreamingUpdateValidationError::EmptyBudgetSummary)
        ));

        manifest.budget_summary = summary;
        manifest.signature.scheme = "real-but-not-implemented".to_string();
        assert!(matches!(
            manifest.validate(&installed),
            Err(FangyuanStreamingUpdateValidationError::InvalidSignaturePlaceholder)
        ));

        manifest.signature = FangyuanStreamingUpdateSignature::placeholder("authority");
        manifest.permissions.scopes.clear();
        assert!(matches!(
            manifest.validate(&installed),
            Err(FangyuanStreamingUpdateValidationError::MissingPermissionPlaceholder)
        ));
    }

    fn identity(
        kind: FangyuanIdentityResourceKind,
        id: &str,
        version: &str,
        payload: &[u8],
    ) -> FangyuanBlueprintIdentity {
        FangyuanBlueprintIdentity::new(
            kind,
            id,
            version,
            FangyuanIdentityHashes::from_bytes(kind, payload, payload, &[]),
            FangyuanIdentitySourceKind::RemoteAuthority,
        )
        .unwrap()
    }
}
