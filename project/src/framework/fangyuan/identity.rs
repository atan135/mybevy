use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, error::Error, fmt};

use super::{
    FANGYUAN_BAKE_SCHEMA_VERSION, FANGYUAN_BLUEPRINT_VERSION, FANGYUAN_CHUNK_VERSION,
    FANGYUAN_MATERIAL_PROFILE_VERSION, FANGYUAN_PREFAB_PALETTE_VERSION,
    FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION, FangyuanAuditReport, FangyuanAuditStatus,
    FangyuanBakeCompiledArtifact, FangyuanBakePayload, FangyuanBlueprint, FangyuanChunkSource,
    FangyuanMaterialProfile, FangyuanPrefabDefinition, FangyuanSkillVisualBlueprint,
    fangyuan_bake_hash_bytes,
};

pub const FANGYUAN_BLUEPRINT_IDENTITY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanIdentityResourceKind {
    Blueprint,
    Prefab,
    Chunk,
    MaterialProfile,
    SkillVisual,
    BakeArtifact,
}

impl FangyuanIdentityResourceKind {
    pub const ALL: [Self; 6] = [
        Self::Blueprint,
        Self::Prefab,
        Self::Chunk,
        Self::MaterialProfile,
        Self::SkillVisual,
        Self::BakeArtifact,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blueprint => "blueprint",
            Self::Prefab => "prefab",
            Self::Chunk => "chunk",
            Self::MaterialProfile => "material_profile",
            Self::SkillVisual => "skill_visual",
            Self::BakeArtifact => "bake_artifact",
        }
    }

    pub fn default_schema_hash(self) -> u64 {
        let schema = match self {
            Self::Blueprint => format!("blueprint:ron:v{FANGYUAN_BLUEPRINT_VERSION}"),
            Self::Prefab => format!("prefab:ron:v{FANGYUAN_PREFAB_PALETTE_VERSION}"),
            Self::Chunk => format!("chunk:ron:v{FANGYUAN_CHUNK_VERSION}"),
            Self::MaterialProfile => {
                format!("material_profile:runtime:v{FANGYUAN_MATERIAL_PROFILE_VERSION}")
            }
            Self::SkillVisual => {
                format!("skill_visual:template_schema:v{FANGYUAN_SKILL_TEMPLATE_SCHEMA_VERSION}")
            }
            Self::BakeArtifact => format!("bake_artifact:bin:v{FANGYUAN_BAKE_SCHEMA_VERSION}"),
        };
        fangyuan_identity_hash_text(&schema)
    }
}

impl fmt::Display for FangyuanIdentityResourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanIdentitySourceKind {
    FirstPackage,
    RuntimeCache,
    Downloaded,
    Generated,
    RemoteAuthority,
    #[default]
    Unknown,
}

impl FangyuanIdentitySourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstPackage => "first_package",
            Self::RuntimeCache => "runtime_cache",
            Self::Downloaded => "downloaded",
            Self::Generated => "generated",
            Self::RemoteAuthority => "remote_authority",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBlueprintIdentity {
    pub kind: FangyuanIdentityResourceKind,
    pub id: String,
    pub version: String,
    pub content_hash: u64,
    pub schema_hash: u64,
    pub source_hash: u64,
    pub dependency_hash: u64,
    pub source_kind: FangyuanIdentitySourceKind,
}

impl FangyuanBlueprintIdentity {
    pub fn new(
        kind: FangyuanIdentityResourceKind,
        id: impl Into<String>,
        version: impl Into<String>,
        hashes: FangyuanIdentityHashes,
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        let id = id.into();
        validate_identity_text("id", &id)?;
        let version = version.into();
        validate_identity_text("version", &version)?;

        Ok(Self {
            kind,
            id,
            version,
            content_hash: hashes.content_hash,
            schema_hash: hashes.schema_hash,
            source_hash: hashes.source_hash,
            dependency_hash: hashes.dependency_hash,
            source_kind,
        })
    }

    pub fn blueprint_from_source(
        blueprint: &FangyuanBlueprint,
        source: &[u8],
        content: &[u8],
        dependencies: &[FangyuanIdentityDependency],
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        Self::from_bytes(
            FangyuanIdentityResourceKind::Blueprint,
            &blueprint.name,
            &blueprint.version,
            source,
            content,
            dependencies,
            source_kind,
        )
    }

    pub fn prefab_from_definition(
        prefab: &FangyuanPrefabDefinition,
        palette_version: impl AsRef<str>,
        source: &[u8],
        dependencies: &[FangyuanIdentityDependency],
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        let content =
            ron::ser::to_string(prefab).map_err(|source| FangyuanIdentityError::Serialize {
                kind: FangyuanIdentityResourceKind::Prefab,
                message: source.to_string(),
            })?;
        Self::from_bytes(
            FangyuanIdentityResourceKind::Prefab,
            &prefab.id,
            palette_version.as_ref(),
            source,
            content.as_bytes(),
            dependencies,
            source_kind,
        )
    }

    pub fn chunk_from_source(
        chunk: &FangyuanChunkSource,
        source: &[u8],
        content: &[u8],
        dependencies: &[FangyuanIdentityDependency],
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        Self::from_bytes(
            FangyuanIdentityResourceKind::Chunk,
            &chunk.id,
            &chunk.version,
            source,
            content,
            dependencies,
            source_kind,
        )
    }

    pub fn material_profile_from_profile(
        profile: &FangyuanMaterialProfile,
        source: &[u8],
        content: &[u8],
        dependencies: &[FangyuanIdentityDependency],
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        Self::from_bytes(
            FangyuanIdentityResourceKind::MaterialProfile,
            &profile.stable_id,
            &profile.version,
            source,
            content,
            dependencies,
            source_kind,
        )
    }

    pub fn skill_visual_from_blueprint(
        visual: &FangyuanSkillVisualBlueprint,
        source: &[u8],
        content: &[u8],
        dependencies: &[FangyuanIdentityDependency],
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        Self::from_bytes(
            FangyuanIdentityResourceKind::SkillVisual,
            &visual.id,
            visual.template_version.to_string(),
            source,
            content,
            dependencies,
            source_kind,
        )
    }

    pub fn bake_artifact(
        id: impl Into<String>,
        version: impl Into<String>,
        compiled: &FangyuanBakeCompiledArtifact,
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        Self::new(
            FangyuanIdentityResourceKind::BakeArtifact,
            id,
            version,
            FangyuanIdentityHashes {
                content_hash: compiled.content_hash,
                schema_hash: FangyuanIdentityResourceKind::BakeArtifact.default_schema_hash(),
                source_hash: compiled.source_hash,
                dependency_hash: fangyuan_identity_dependency_hash(
                    &fangyuan_identity_dependencies_from_bake_table(&compiled.dependency_table),
                ),
            },
            source_kind,
        )
    }

    pub fn from_audited_bake_artifact(
        id: impl Into<String>,
        version: impl Into<String>,
        compiled: &FangyuanBakeCompiledArtifact,
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        ensure_audit_passed(&compiled.audit)?;
        Self::bake_artifact(id, version, compiled, source_kind)
    }

    pub fn cache_key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.id)
    }

    pub fn matches_version_and_hash(&self, version: &str, content_hash: u64) -> bool {
        self.version == version && self.content_hash == content_hash
    }

    fn from_bytes(
        kind: FangyuanIdentityResourceKind,
        id: impl Into<String>,
        version: impl Into<String>,
        source: &[u8],
        content: &[u8],
        dependencies: &[FangyuanIdentityDependency],
        source_kind: FangyuanIdentitySourceKind,
    ) -> Result<Self, FangyuanIdentityError> {
        Self::new(
            kind,
            id,
            version,
            FangyuanIdentityHashes {
                content_hash: fangyuan_bake_hash_bytes(content),
                schema_hash: kind.default_schema_hash(),
                source_hash: fangyuan_bake_hash_bytes(source),
                dependency_hash: fangyuan_identity_dependency_hash(dependencies),
            },
            source_kind,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanIdentityHashes {
    pub content_hash: u64,
    pub schema_hash: u64,
    pub source_hash: u64,
    pub dependency_hash: u64,
}

impl FangyuanIdentityHashes {
    pub fn from_bytes(
        kind: FangyuanIdentityResourceKind,
        source: &[u8],
        content: &[u8],
        dependencies: &[FangyuanIdentityDependency],
    ) -> Self {
        Self {
            content_hash: fangyuan_bake_hash_bytes(content),
            schema_hash: kind.default_schema_hash(),
            source_hash: fangyuan_bake_hash_bytes(source),
            dependency_hash: fangyuan_identity_dependency_hash(dependencies),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanIdentityDependency {
    pub kind: FangyuanIdentityResourceKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<u64>,
}

impl FangyuanIdentityDependency {
    pub fn new(kind: FangyuanIdentityResourceKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
            version: None,
            content_hash: None,
        }
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub const fn with_content_hash(mut self, content_hash: u64) -> Self {
        self.content_hash = Some(content_hash);
        self
    }

    pub fn cache_key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.id)
    }

    fn canonical_record(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.kind.as_str(),
            self.id,
            self.version.as_deref().unwrap_or(""),
            self.content_hash
                .map(|hash| format!("{hash:016x}"))
                .unwrap_or_default()
        )
    }
}

pub fn fangyuan_identity_dependency_hash(dependencies: &[FangyuanIdentityDependency]) -> u64 {
    let canonical = dependencies
        .iter()
        .map(FangyuanIdentityDependency::canonical_record)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    fangyuan_identity_hash_text(&canonical)
}

pub fn fangyuan_identity_hash_text(text: &str) -> u64 {
    fangyuan_bake_hash_bytes(text.as_bytes())
}

pub fn record_fangyuan_identity_after_audit(
    kind: FangyuanIdentityResourceKind,
    id: impl Into<String>,
    version: impl Into<String>,
    source: &[u8],
    content: &[u8],
    dependencies: &[FangyuanIdentityDependency],
    source_kind: FangyuanIdentitySourceKind,
    audit: &FangyuanAuditReport,
) -> Result<FangyuanBlueprintIdentity, FangyuanIdentityError> {
    ensure_audit_passed(audit)?;
    FangyuanBlueprintIdentity::from_bytes(
        kind,
        id,
        version,
        source,
        content,
        dependencies,
        source_kind,
    )
}

pub fn fangyuan_identity_dependencies_from_bake_payload(
    payload: &FangyuanBakePayload,
) -> Vec<FangyuanIdentityDependency> {
    match payload {
        FangyuanBakePayload::Blueprint { blueprint, .. } => blueprint
            .primitives
            .iter()
            .filter_map(|primitive| primitive.material_profile_id.as_deref())
            .map(|id| {
                FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::MaterialProfile, id)
            })
            .collect(),
        FangyuanBakePayload::PrefabPalette { palette, .. } => palette
            .prefabs
            .iter()
            .flat_map(|prefab| {
                prefab
                    .primitives
                    .iter()
                    .filter_map(|primitive| primitive.material_profile_id.as_deref())
                    .map(|id| {
                        FangyuanIdentityDependency::new(
                            FangyuanIdentityResourceKind::MaterialProfile,
                            id,
                        )
                    })
            })
            .collect(),
        FangyuanBakePayload::SceneLayout { layout, .. } => layout
            .palette_paths()
            .map(|path| FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Prefab, path))
            .chain(layout.instances.iter().map(|instance| {
                FangyuanIdentityDependency::new(
                    FangyuanIdentityResourceKind::Prefab,
                    &instance.prefab,
                )
            }))
            .collect(),
        FangyuanBakePayload::ChunkSource { chunk, .. } => chunk
            .prefab_instances
            .iter()
            .map(|reference| {
                FangyuanIdentityDependency::new(
                    FangyuanIdentityResourceKind::Prefab,
                    &reference.prefab,
                )
            })
            .chain(chunk.static_decorations.iter().filter_map(
                |decoration| match &decoration.source {
                    super::FangyuanChunkStaticDecorationSourceRef::Prefab { prefab } => {
                        Some(FangyuanIdentityDependency::new(
                            FangyuanIdentityResourceKind::Prefab,
                            prefab,
                        ))
                    }
                    super::FangyuanChunkStaticDecorationSourceRef::Blueprint { blueprint } => {
                        Some(FangyuanIdentityDependency::new(
                            FangyuanIdentityResourceKind::Blueprint,
                            blueprint,
                        ))
                    }
                    super::FangyuanChunkStaticDecorationSourceRef::Bake { bake } => {
                        Some(FangyuanIdentityDependency::new(
                            FangyuanIdentityResourceKind::BakeArtifact,
                            bake,
                        ))
                    }
                },
            ))
            .collect(),
        FangyuanBakePayload::ChunkManifest { manifest, .. } => manifest
            .chunks
            .iter()
            .map(|entry| {
                FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Chunk, &entry.id)
            })
            .collect(),
        FangyuanBakePayload::MaterialProfile { .. } => Vec::new(),
        FangyuanBakePayload::SkillTemplate { template, .. } => {
            vec![
                FangyuanIdentityDependency::new(
                    FangyuanIdentityResourceKind::SkillVisual,
                    &template.id,
                )
                .with_version(template.version.to_string()),
            ]
        }
        FangyuanBakePayload::SkillVisual { visual, .. } => {
            let mut dependencies = vec![
                FangyuanIdentityDependency::new(
                    FangyuanIdentityResourceKind::SkillVisual,
                    &visual.template_id,
                )
                .with_version(visual.template_version.to_string()),
            ];
            if let Some(profile_ref) = visual.profile_ref.as_deref() {
                dependencies.push(FangyuanIdentityDependency::new(
                    FangyuanIdentityResourceKind::MaterialProfile,
                    profile_ref,
                ));
            }
            dependencies
        }
        FangyuanBakePayload::VfxRecipe { recipe, .. } => vec![FangyuanIdentityDependency::new(
            FangyuanIdentityResourceKind::SkillVisual,
            &recipe.id,
        )],
    }
}

pub fn fangyuan_identity_dependencies_from_bake_table(
    table: &super::FangyuanBakeDependencyTable,
) -> Vec<FangyuanIdentityDependency> {
    let mut dependencies = Vec::new();
    dependencies.extend(
        table
            .prefab_ids
            .iter()
            .map(|id| FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Prefab, id)),
    );
    dependencies.extend(
        table
            .blueprint_paths
            .iter()
            .map(|id| FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Blueprint, id)),
    );
    dependencies.extend(
        table
            .chunk_ids
            .iter()
            .map(|id| FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Chunk, id)),
    );
    dependencies.extend(table.material_profile_ids.iter().map(|id| {
        FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::MaterialProfile, id)
    }));
    dependencies.extend(
        table.skill_ids.iter().map(|id| {
            FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::SkillVisual, id)
        }),
    );
    dependencies
}

fn ensure_audit_passed(audit: &FangyuanAuditReport) -> Result<(), FangyuanIdentityError> {
    if audit.status == FangyuanAuditStatus::Failed || audit.summary.error_count > 0 {
        Err(FangyuanIdentityError::AuditFailed {
            error_count: audit.summary.error_count,
        })
    } else {
        Ok(())
    }
}

fn validate_identity_text(field: &'static str, value: &str) -> Result<(), FangyuanIdentityError> {
    if value.trim().is_empty() {
        Err(FangyuanIdentityError::EmptyField { field })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanIdentityError {
    EmptyField {
        field: &'static str,
    },
    AuditFailed {
        error_count: usize,
    },
    Serialize {
        kind: FangyuanIdentityResourceKind,
        message: String,
    },
}

impl fmt::Display for FangyuanIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "fangyuan identity {field} is empty"),
            Self::AuditFailed { error_count } => write!(
                formatter,
                "fangyuan identity can only be recorded after audit passes; error_count={error_count}"
            ),
            Self::Serialize { kind, message } => {
                write!(
                    formatter,
                    "failed to serialize {kind} identity content: {message}"
                )
            }
        }
    }
}

impl Error for FangyuanIdentityError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FangyuanAuditFinding, FangyuanAuditSeverity, FangyuanAuditSourceKind,
    };

    #[test]
    fn fangyuan_blueprint_identity_records_hashes_after_audit_passes() {
        let audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::Blueprint, None);
        let source = b"(version:\"1\",name:\"identity\")";
        let content = b"compiled-identity";
        let dependencies = vec![
            FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Prefab, "stone_block")
                .with_version("1"),
            FangyuanIdentityDependency::new(
                FangyuanIdentityResourceKind::MaterialProfile,
                "fx/test",
            )
            .with_content_hash(0x1234),
        ];

        let identity = record_fangyuan_identity_after_audit(
            FangyuanIdentityResourceKind::Blueprint,
            "fangyuan/avatars/minimal_player.ron",
            "1",
            source,
            content,
            &dependencies,
            FangyuanIdentitySourceKind::FirstPackage,
            &audit,
        )
        .unwrap();

        assert_eq!(identity.kind, FangyuanIdentityResourceKind::Blueprint);
        assert_eq!(identity.id, "fangyuan/avatars/minimal_player.ron");
        assert_eq!(identity.version, "1");
        assert_eq!(identity.content_hash, fangyuan_bake_hash_bytes(content));
        assert_eq!(identity.source_hash, fangyuan_bake_hash_bytes(source));
        assert_eq!(
            identity.schema_hash,
            FangyuanIdentityResourceKind::Blueprint.default_schema_hash()
        );
        assert_eq!(
            identity.dependency_hash,
            fangyuan_identity_dependency_hash(&dependencies)
        );
        assert_eq!(
            identity.source_kind,
            FangyuanIdentitySourceKind::FirstPackage
        );
    }

    #[test]
    fn fangyuan_blueprint_identity_dependency_hash_is_stable_and_order_independent() {
        let first = vec![
            FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Chunk, "chunk_b")
                .with_version("2"),
            FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Prefab, "stone_block")
                .with_content_hash(0x11),
            FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Chunk, "chunk_a"),
        ];
        let second = vec![first[2].clone(), first[1].clone(), first[0].clone()];

        assert_eq!(
            fangyuan_identity_dependency_hash(&first),
            fangyuan_identity_dependency_hash(&second)
        );

        let changed = vec![
            FangyuanIdentityDependency::new(FangyuanIdentityResourceKind::Chunk, "chunk_b")
                .with_version("3"),
            first[1].clone(),
            first[2].clone(),
        ];
        assert_ne!(
            fangyuan_identity_dependency_hash(&first),
            fangyuan_identity_dependency_hash(&changed)
        );
    }

    #[test]
    fn fangyuan_blueprint_identity_covers_required_resource_kinds() {
        let source = b"source";
        let content = b"content";
        let dependencies = Vec::new();

        for kind in FangyuanIdentityResourceKind::ALL {
            let identity = FangyuanBlueprintIdentity::new(
                kind,
                format!("{}.sample", kind.as_str()),
                "1",
                FangyuanIdentityHashes::from_bytes(kind, source, content, &dependencies),
                FangyuanIdentitySourceKind::Generated,
            )
            .unwrap();

            assert_eq!(identity.kind, kind);
            assert_ne!(identity.schema_hash, 0);
            assert_ne!(identity.content_hash, 0);
            assert_ne!(identity.source_hash, 0);
        }
    }

    #[test]
    fn fangyuan_blueprint_identity_rejects_failed_audit() {
        let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::Blueprint, None);
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            "identity_test_error",
            "identity test error",
            FangyuanAuditSourceKind::Blueprint,
        ));

        let error = record_fangyuan_identity_after_audit(
            FangyuanIdentityResourceKind::Blueprint,
            "bad",
            "1",
            b"source",
            b"content",
            &[],
            FangyuanIdentitySourceKind::FirstPackage,
            &audit,
        )
        .unwrap_err();

        assert_eq!(error, FangyuanIdentityError::AuditFailed { error_count: 1 });
    }
}
