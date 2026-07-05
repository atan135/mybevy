use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    time::Instant,
};

use super::{
    FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity, FangyuanAuditSourceKind,
    FangyuanBlueprint, FangyuanChunkManifest, FangyuanChunkManifestEntry, FangyuanChunkSource,
    FangyuanMaterialProfile, FangyuanPrefabPalette, FangyuanSceneLayout, FangyuanSkillTemplate,
    FangyuanSkillVisualBlueprint, FangyuanVfxRecipe,
};

pub const FANGYUAN_BAKE_ARTIFACT_MAGIC: [u8; 8] = *b"FYBAKE\0\x01";
pub const FANGYUAN_BAKE_SCHEMA_VERSION: u16 = 1;
pub const FANGYUAN_BAKE_PAYLOAD_VERSION: u16 = 1;
pub const FANGYUAN_BAKE_HASH_BYTES: usize = 8;
pub const FANGYUAN_BAKE_FORMAT_NAME: &str = "fangyuan-custom-header-typed-payload-v1";
const FANGYUAN_BAKE_CREATED_BY_MAX_BYTES: usize = u16::MAX as usize;
const FANGYUAN_BAKE_HEADER_FIXED_BYTES: usize =
    8 + 2 + 1 + FANGYUAN_BAKE_HASH_BYTES + FANGYUAN_BAKE_HASH_BYTES + 2;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanBakeArtifactKind {
    Blueprint,
    PrefabPalette,
    SceneLayout,
    Chunk,
    MaterialProfile,
    SkillRecipe,
}

impl FangyuanBakeArtifactKind {
    pub const ALL: [Self; 6] = [
        Self::Blueprint,
        Self::PrefabPalette,
        Self::SceneLayout,
        Self::Chunk,
        Self::MaterialProfile,
        Self::SkillRecipe,
    ];

    pub const fn wire_id(self) -> u8 {
        match self {
            Self::Blueprint => 1,
            Self::PrefabPalette => 2,
            Self::SceneLayout => 3,
            Self::Chunk => 4,
            Self::MaterialProfile => 5,
            Self::SkillRecipe => 6,
        }
    }

    pub const fn file_stem_suffix(self) -> &'static str {
        match self {
            Self::Blueprint => "blueprint",
            Self::PrefabPalette => "prefab_palette",
            Self::SceneLayout => "scene_layout",
            Self::Chunk => "chunk",
            Self::MaterialProfile => "material_profile",
            Self::SkillRecipe => "skill_recipe",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blueprint => "blueprint",
            Self::PrefabPalette => "prefab_palette",
            Self::SceneLayout => "scene_layout",
            Self::Chunk => "chunk",
            Self::MaterialProfile => "material_profile",
            Self::SkillRecipe => "skill_recipe",
        }
    }

    pub fn from_wire_id(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Blueprint),
            2 => Some(Self::PrefabPalette),
            3 => Some(Self::SceneLayout),
            4 => Some(Self::Chunk),
            5 => Some(Self::MaterialProfile),
            6 => Some(Self::SkillRecipe),
            _ => None,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "blueprint" => Some(Self::Blueprint),
            "prefab_palette" | "palette" => Some(Self::PrefabPalette),
            "scene_layout" | "layout" => Some(Self::SceneLayout),
            "chunk" => Some(Self::Chunk),
            "material_profile" | "material" => Some(Self::MaterialProfile),
            "skill_recipe" | "skill" => Some(Self::SkillRecipe),
            _ => None,
        }
    }
}

impl fmt::Display for FangyuanBakeArtifactKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanBakeArtifactHeader {
    pub schema_version: u16,
    pub source_hash: u64,
    pub content_hash: u64,
    pub created_by: String,
    pub target_kind: FangyuanBakeArtifactKind,
}

impl FangyuanBakeArtifactHeader {
    pub fn new(
        source: &[u8],
        content: &[u8],
        created_by: impl Into<String>,
        target_kind: FangyuanBakeArtifactKind,
    ) -> Self {
        Self {
            schema_version: FANGYUAN_BAKE_SCHEMA_VERSION,
            source_hash: fangyuan_bake_hash_bytes(source),
            content_hash: fangyuan_bake_hash_bytes(content),
            created_by: created_by.into(),
            target_kind,
        }
    }

    pub fn validate_schema_version(&self) -> Result<(), FangyuanBakeFormatError> {
        if self.schema_version == FANGYUAN_BAKE_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(FangyuanBakeFormatError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: FANGYUAN_BAKE_SCHEMA_VERSION,
            })
        }
    }

    pub fn validate_hashes(
        &self,
        source: &[u8],
        content: &[u8],
    ) -> Result<(), FangyuanBakeFormatError> {
        let actual_source_hash = fangyuan_bake_hash_bytes(source);
        if actual_source_hash != self.source_hash {
            return Err(FangyuanBakeFormatError::SourceHashMismatch {
                expected: self.source_hash,
                actual: actual_source_hash,
            });
        }

        let actual_content_hash = fangyuan_bake_hash_bytes(content);
        if actual_content_hash != self.content_hash {
            return Err(FangyuanBakeFormatError::ContentHashMismatch {
                expected: self.content_hash,
                actual: actual_content_hash,
            });
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanBakeArtifact {
    pub header: FangyuanBakeArtifactHeader,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FangyuanBakePayload {
    Blueprint {
        payload_version: u16,
        blueprint: FangyuanBlueprint,
    },
    PrefabPalette {
        payload_version: u16,
        palette: FangyuanPrefabPalette,
    },
    SceneLayout {
        payload_version: u16,
        layout: FangyuanSceneLayout,
    },
    ChunkSource {
        payload_version: u16,
        chunk: FangyuanChunkSource,
    },
    ChunkManifest {
        payload_version: u16,
        manifest: FangyuanChunkManifest,
    },
    MaterialProfile {
        payload_version: u16,
        profile: FangyuanMaterialProfileArtifact,
    },
    SkillTemplate {
        payload_version: u16,
        template: FangyuanSkillTemplate,
    },
    SkillVisual {
        payload_version: u16,
        visual: FangyuanSkillVisualBlueprint,
    },
    VfxRecipe {
        payload_version: u16,
        recipe: FangyuanVfxRecipe,
    },
}

impl FangyuanBakePayload {
    pub const fn artifact_kind(&self) -> FangyuanBakeArtifactKind {
        match self {
            Self::Blueprint { .. } => FangyuanBakeArtifactKind::Blueprint,
            Self::PrefabPalette { .. } => FangyuanBakeArtifactKind::PrefabPalette,
            Self::SceneLayout { .. } => FangyuanBakeArtifactKind::SceneLayout,
            Self::ChunkSource { .. } | Self::ChunkManifest { .. } => {
                FangyuanBakeArtifactKind::Chunk
            }
            Self::MaterialProfile { .. } => FangyuanBakeArtifactKind::MaterialProfile,
            Self::SkillTemplate { .. } | Self::SkillVisual { .. } | Self::VfxRecipe { .. } => {
                FangyuanBakeArtifactKind::SkillRecipe
            }
        }
    }

    pub const fn payload_version(&self) -> u16 {
        match self {
            Self::Blueprint {
                payload_version, ..
            }
            | Self::PrefabPalette {
                payload_version, ..
            }
            | Self::SceneLayout {
                payload_version, ..
            }
            | Self::ChunkSource {
                payload_version, ..
            }
            | Self::ChunkManifest {
                payload_version, ..
            }
            | Self::MaterialProfile {
                payload_version, ..
            }
            | Self::SkillTemplate {
                payload_version, ..
            }
            | Self::SkillVisual {
                payload_version, ..
            }
            | Self::VfxRecipe {
                payload_version, ..
            } => *payload_version,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanMaterialProfileArtifact {
    pub stable_id: String,
    pub version: String,
    pub debug_label: String,
    pub base_color: [f32; 4],
    pub base_alpha: f32,
    pub base_emissive: f32,
    pub alpha_policy: FangyuanMaterialAlphaPolicyArtifact,
    pub emissive_policy: FangyuanMaterialEmissivePolicyArtifact,
}

impl FangyuanMaterialProfileArtifact {
    pub fn from_profile(profile: &FangyuanMaterialProfile) -> Self {
        let color = profile.base.color.to_srgba();
        Self {
            stable_id: profile.stable_id.clone(),
            version: profile.version.clone(),
            debug_label: profile.debug_label.clone(),
            base_color: [color.red, color.green, color.blue, color.alpha],
            base_alpha: profile.base.alpha,
            base_emissive: profile.base.emissive,
            alpha_policy: FangyuanMaterialAlphaPolicyArtifact::from_policy(profile.alpha_policy),
            emissive_policy: FangyuanMaterialEmissivePolicyArtifact::from_policy(
                profile.emissive_policy,
            ),
        }
    }

    pub fn default_from_minimal(
        stable_id: impl Into<String>,
        version: impl Into<String>,
        debug_label: impl Into<String>,
    ) -> Self {
        let profile = FangyuanMaterialProfile {
            stable_id: stable_id.into(),
            version: version.into(),
            debug_label: debug_label.into(),
            ..FangyuanMaterialProfile::default_profile()
        };
        Self::from_profile(&profile)
    }

    pub fn to_profile(&self) -> FangyuanMaterialProfile {
        FangyuanMaterialProfile {
            stable_id: self.stable_id.clone(),
            version: self.version.clone(),
            base: super::FangyuanMaterialBaseParams {
                color: bevy::prelude::Color::srgba(
                    self.base_color[0],
                    self.base_color[1],
                    self.base_color[2],
                    self.base_color[3],
                ),
                alpha: self.base_alpha,
                emissive: self.base_emissive,
            },
            emissive_policy: self.emissive_policy.to_policy(),
            alpha_policy: self.alpha_policy.to_policy(),
            debug_label: self.debug_label.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanMaterialAlphaPolicyArtifact {
    MultiplyClamp { min: f32, max: f32 },
    ForceOpaque,
}

impl FangyuanMaterialAlphaPolicyArtifact {
    const fn from_policy(policy: super::FangyuanMaterialAlphaPolicy) -> Self {
        match policy {
            super::FangyuanMaterialAlphaPolicy::MultiplyClamp { min, max } => {
                Self::MultiplyClamp { min, max }
            }
            super::FangyuanMaterialAlphaPolicy::ForceOpaque => Self::ForceOpaque,
        }
    }

    const fn to_policy(self) -> super::FangyuanMaterialAlphaPolicy {
        match self {
            Self::MultiplyClamp { min, max } => {
                super::FangyuanMaterialAlphaPolicy::MultiplyClamp { min, max }
            }
            Self::ForceOpaque => super::FangyuanMaterialAlphaPolicy::ForceOpaque,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanMaterialEmissivePolicyArtifact {
    AdditiveClamp { max: f32 },
    Disabled,
}

impl FangyuanMaterialEmissivePolicyArtifact {
    const fn from_policy(policy: super::FangyuanMaterialEmissivePolicy) -> Self {
        match policy {
            super::FangyuanMaterialEmissivePolicy::AdditiveClamp { max } => {
                Self::AdditiveClamp { max }
            }
            super::FangyuanMaterialEmissivePolicy::Disabled => Self::Disabled,
        }
    }

    const fn to_policy(self) -> super::FangyuanMaterialEmissivePolicy {
        match self {
            Self::AdditiveClamp { max } => {
                super::FangyuanMaterialEmissivePolicy::AdditiveClamp { max }
            }
            Self::Disabled => super::FangyuanMaterialEmissivePolicy::Disabled,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBakeDependencyTable {
    pub source_path: Option<String>,
    pub layout_paths: Vec<String>,
    pub palette_paths: Vec<String>,
    pub prefab_ids: Vec<String>,
    pub material_profile_ids: Vec<String>,
    pub blueprint_paths: Vec<String>,
    pub chunk_ids: Vec<String>,
    pub chunk_paths: Vec<String>,
    pub skill_ids: Vec<String>,
    pub missing: Vec<FangyuanBakeMissingDependency>,
}

impl FangyuanBakeDependencyTable {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBakeMissingDependency {
    pub owner: String,
    pub kind: FangyuanBakeDependencyKind,
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanBakeDependencyKind {
    Layout,
    Palette,
    Prefab,
    MaterialProfile,
    Blueprint,
    Chunk,
    Skill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBakeArtifactStats {
    pub primitive_count: usize,
    pub prefab_count: usize,
    pub chunk_count: usize,
    pub profile_count: usize,
    pub budget: usize,
    pub warning_count: usize,
    pub artifact_size: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanBakeCompiledArtifact {
    pub target_kind: FangyuanBakeArtifactKind,
    pub source_hash: u64,
    pub content_hash: u64,
    pub normalized_source_hash: u64,
    pub upgraded_from: Option<u16>,
    pub payload: FangyuanBakePayload,
    pub payload_bytes: Vec<u8>,
    pub dependency_table: FangyuanBakeDependencyTable,
    pub stats: FangyuanBakeArtifactStats,
    pub audit: FangyuanAuditReport,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanRuntimeArtifactManifestEntry {
    pub id: String,
    pub kind: FangyuanBakeArtifactKind,
    pub bin: Option<PathBuf>,
    pub ron: Option<PathBuf>,
    pub expected_content_hash: Option<u64>,
    pub expected_source_hash: Option<u64>,
    pub required_dependencies: Vec<String>,
}

impl FangyuanRuntimeArtifactManifestEntry {
    pub fn from_chunk_manifest_entry(
        entry: &FangyuanChunkManifestEntry,
        kind: FangyuanBakeArtifactKind,
    ) -> Self {
        Self {
            id: entry.id.clone(),
            kind,
            bin: entry.bin.as_ref().map(PathBuf::from),
            ron: entry.dev_ron.as_ref().map(PathBuf::from),
            expected_content_hash: entry.hash.as_deref().and_then(parse_fangyuan_bake_hex_hash),
            expected_source_hash: None,
            required_dependencies: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanRuntimeArtifactLoaderOptions {
    pub allow_ron_fallback: bool,
}

impl FangyuanRuntimeArtifactLoaderOptions {
    pub const fn release() -> Self {
        Self {
            allow_ron_fallback: false,
        }
    }

    pub const fn debug_with_ron_fallback() -> Self {
        Self {
            allow_ron_fallback: true,
        }
    }
}

impl Default for FangyuanRuntimeArtifactLoaderOptions {
    fn default() -> Self {
        if cfg!(debug_assertions) {
            Self::debug_with_ron_fallback()
        } else {
            Self::release()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanRuntimeArtifactLoadReport {
    pub id: String,
    pub kind: FangyuanBakeArtifactKind,
    pub status: FangyuanRuntimeArtifactLoadStatus,
    pub source: FangyuanRuntimeArtifactLoadSource,
    pub fallback: FangyuanRuntimeArtifactFallback,
    pub error: Option<FangyuanRuntimeArtifactLoadError>,
    pub header: Option<FangyuanBakeArtifactHeader>,
    pub payload: Option<FangyuanBakePayload>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanRuntimeArtifactLoadStatus {
    Loaded,
    FallbackLoaded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanRuntimeArtifactLoadSource {
    Bin,
    Ron,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanRuntimeArtifactFallback {
    NotNeeded,
    Disabled,
    Attempted,
    Used,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanRuntimeArtifactLoadError {
    LoadFailed {
        path: PathBuf,
        message: String,
    },
    VersionMismatch {
        found: u16,
        expected: u16,
    },
    PayloadVersionMismatch {
        found: u16,
        expected: u16,
    },
    KindMismatch {
        found: FangyuanBakeArtifactKind,
        expected: FangyuanBakeArtifactKind,
    },
    HashMismatch {
        hash_kind: FangyuanRuntimeArtifactHashKind,
        expected: u64,
        actual: u64,
    },
    DependencyMissing {
        id: String,
    },
    ParseFailed {
        path: PathBuf,
        message: String,
    },
    Format(FangyuanBakeFormatError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanRuntimeArtifactHashKind {
    Source,
    Content,
}

impl fmt::Display for FangyuanRuntimeArtifactLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed { path, message } => {
                write!(formatter, "failed to load {}: {message}", path.display())
            }
            Self::VersionMismatch { found, expected } => {
                write!(
                    formatter,
                    "artifact schema version mismatch: found {found}, expected {expected}"
                )
            }
            Self::PayloadVersionMismatch { found, expected } => {
                write!(
                    formatter,
                    "artifact payload version mismatch: found {found}, expected {expected}"
                )
            }
            Self::KindMismatch { found, expected } => {
                write!(
                    formatter,
                    "artifact kind mismatch: found {found}, expected {expected}"
                )
            }
            Self::HashMismatch {
                hash_kind,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "{hash_kind:?} hash mismatch: expected {expected:016x}, actual {actual:016x}"
                )
            }
            Self::DependencyMissing { id } => {
                write!(formatter, "artifact dependency missing: {id}")
            }
            Self::ParseFailed { path, message } => {
                write!(formatter, "failed to parse {}: {message}", path.display())
            }
            Self::Format(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for FangyuanRuntimeArtifactLoadError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanBakeFormatDecision {
    pub name: &'static str,
    pub selected: bool,
    pub reason: &'static str,
    pub limitations: &'static [&'static str],
    pub dependency_impact: &'static str,
}

pub fn fangyuan_bake_format_decisions() -> Vec<FangyuanBakeFormatDecision> {
    vec![
        FangyuanBakeFormatDecision {
            name: FANGYUAN_BAKE_FORMAT_NAME,
            selected: true,
            reason: "使用稳定 custom artifact header 包裹 typed RON payload envelope；payload 由已解析并升级后的 blueprint、prefab、layout、chunk、material profile 和 skill recipe 类型重新序列化生成，保持 deterministic bake，并供 runtime loader 按 schema、kind、hash 和 payload 校验后加载。",
            limitations: &[
                "typed RON payload 是 runtime 可校验、可加载的 artifact 内容，但不是紧凑二进制 codec 或零拷贝格式",
                "FNV-1a hash 用于本地内容一致性检查，不用于安全校验",
            ],
            dependency_impact: "不新增依赖；复用 std、serde、ron 和现有 validator/audit/runtime loader。",
        },
        FangyuanBakeFormatDecision {
            name: "bincode",
            selected: false,
            reason: "需要新增依赖并绑定 serde 二进制编码语义；当前数据模型仍包含 Bevy 类型和后续 loader 未定的 runtime payload。",
            limitations: &["适合后续稳定 payload schema 后再评估"],
            dependency_impact: "会新增外部 crate 和 Cargo.lock 变更。",
        },
        FangyuanBakeFormatDecision {
            name: "postcard",
            selected: false,
            reason: "面向 no_std/紧凑消息很合适，但本阶段重点是工具侧校验和 artifact envelope，不需要引入新编码器。",
            limitations: &["完整零拷贝/紧凑 payload 需等 runtime loader 设计明确"],
            dependency_impact: "会新增外部 crate 和 Cargo.lock 变更。",
        },
    ]
}

pub fn encode_fangyuan_bake_artifact(
    header: &FangyuanBakeArtifactHeader,
    payload: &[u8],
) -> Result<Vec<u8>, FangyuanBakeFormatError> {
    header.validate_schema_version()?;
    let created_by = header.created_by.as_bytes();
    if created_by.len() > FANGYUAN_BAKE_CREATED_BY_MAX_BYTES {
        return Err(FangyuanBakeFormatError::CreatedByTooLong {
            len: created_by.len(),
            max: FANGYUAN_BAKE_CREATED_BY_MAX_BYTES,
        });
    }

    let mut bytes =
        Vec::with_capacity(FANGYUAN_BAKE_HEADER_FIXED_BYTES + created_by.len() + payload.len());
    bytes.extend_from_slice(&FANGYUAN_BAKE_ARTIFACT_MAGIC);
    bytes.extend_from_slice(&header.schema_version.to_le_bytes());
    bytes.push(header.target_kind.wire_id());
    bytes.extend_from_slice(&header.source_hash.to_le_bytes());
    bytes.extend_from_slice(&header.content_hash.to_le_bytes());
    bytes.extend_from_slice(&(created_by.len() as u16).to_le_bytes());
    bytes.extend_from_slice(created_by);
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

pub fn decode_fangyuan_bake_artifact(
    bytes: &[u8],
) -> Result<FangyuanBakeArtifact, FangyuanBakeFormatError> {
    if bytes.len() < FANGYUAN_BAKE_HEADER_FIXED_BYTES {
        return Err(FangyuanBakeFormatError::TruncatedHeader {
            len: bytes.len(),
            min: FANGYUAN_BAKE_HEADER_FIXED_BYTES,
        });
    }

    let mut cursor = 0usize;
    let magic = read_exact::<8>(bytes, &mut cursor)?;
    if magic != FANGYUAN_BAKE_ARTIFACT_MAGIC {
        return Err(FangyuanBakeFormatError::InvalidMagic { found: magic });
    }

    let schema_version = u16::from_le_bytes(read_exact::<2>(bytes, &mut cursor)?);
    let target_kind = FangyuanBakeArtifactKind::from_wire_id(bytes[cursor]).ok_or(
        FangyuanBakeFormatError::InvalidArtifactKind {
            value: bytes[cursor],
        },
    )?;
    cursor += 1;
    let source_hash = u64::from_le_bytes(read_exact::<8>(bytes, &mut cursor)?);
    let content_hash = u64::from_le_bytes(read_exact::<8>(bytes, &mut cursor)?);
    let created_by_len = u16::from_le_bytes(read_exact::<2>(bytes, &mut cursor)?) as usize;
    if bytes.len() < cursor + created_by_len {
        return Err(FangyuanBakeFormatError::TruncatedCreatedBy {
            len: bytes.len().saturating_sub(cursor),
            expected: created_by_len,
        });
    }
    let created_by = std::str::from_utf8(&bytes[cursor..cursor + created_by_len])
        .map_err(|_| FangyuanBakeFormatError::InvalidCreatedByUtf8)?
        .to_string();
    cursor += created_by_len;

    let artifact = FangyuanBakeArtifact {
        header: FangyuanBakeArtifactHeader {
            schema_version,
            source_hash,
            content_hash,
            created_by,
            target_kind,
        },
        payload: bytes[cursor..].to_vec(),
    };
    artifact.header.validate_schema_version()?;
    Ok(artifact)
}

pub fn fangyuan_bake_hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanBakeSourceVersion {
    Current,
    LegacyZero,
}

pub fn upgrade_fangyuan_bake_source_if_needed(
    source: &str,
) -> Result<(String, FangyuanBakeSourceVersion), FangyuanBakeValidationError> {
    let mut upgraded = String::with_capacity(source.len());
    let mut changed = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        let prefix_len = line.len() - trimmed.len();
        let replacement = if trimmed.starts_with("version: \"0\"") {
            Some(format!(
                "{}{}",
                &line[..prefix_len],
                trimmed.replacen("version: \"0\"", "version: \"1\"", 1)
            ))
        } else if trimmed.starts_with("version: 0") {
            Some(format!(
                "{}{}",
                &line[..prefix_len],
                trimmed.replacen("version: 0", "version: 1", 1)
            ))
        } else {
            None
        };

        if let Some(line) = replacement {
            upgraded.push_str(&line);
            changed = true;
        } else {
            upgraded.push_str(line);
        }
        upgraded.push('\n');
    }

    if changed {
        Ok((upgraded, FangyuanBakeSourceVersion::LegacyZero))
    } else {
        Ok((source.to_string(), FangyuanBakeSourceVersion::Current))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanBakeFormatError {
    CreatedByTooLong { len: usize, max: usize },
    TruncatedHeader { len: usize, min: usize },
    InvalidMagic { found: [u8; 8] },
    InvalidArtifactKind { value: u8 },
    TruncatedCreatedBy { len: usize, expected: usize },
    InvalidCreatedByUtf8,
    UnsupportedSchemaVersion { found: u16, expected: u16 },
    SourceHashMismatch { expected: u64, actual: u64 },
    ContentHashMismatch { expected: u64, actual: u64 },
}

impl fmt::Display for FangyuanBakeFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreatedByTooLong { len, max } => {
                write!(formatter, "created_by is {len} bytes, max {max}")
            }
            Self::TruncatedHeader { len, min } => {
                write!(
                    formatter,
                    "bake artifact header is truncated: {len} < {min}"
                )
            }
            Self::InvalidMagic { found } => {
                write!(formatter, "invalid bake artifact magic: {found:?}")
            }
            Self::InvalidArtifactKind { value } => {
                write!(formatter, "invalid bake artifact kind id: {value}")
            }
            Self::TruncatedCreatedBy { len, expected } => {
                write!(formatter, "created_by is truncated: {len} < {expected}")
            }
            Self::InvalidCreatedByUtf8 => formatter.write_str("created_by is not valid UTF-8"),
            Self::UnsupportedSchemaVersion { found, expected } => {
                write!(
                    formatter,
                    "unsupported bake schema version {found}, expected {expected}"
                )
            }
            Self::SourceHashMismatch { expected, actual } => {
                write!(
                    formatter,
                    "source hash mismatch: expected {expected:016x}, actual {actual:016x}"
                )
            }
            Self::ContentHashMismatch { expected, actual } => {
                write!(
                    formatter,
                    "content hash mismatch: expected {expected:016x}, actual {actual:016x}"
                )
            }
        }
    }
}

impl Error for FangyuanBakeFormatError {}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanBakeValidationReport {
    pub target_kind: FangyuanBakeArtifactKind,
    pub source_path: Option<PathBuf>,
    pub content: String,
    pub source_hash: u64,
    pub content_hash: u64,
    pub upgraded_from: Option<u16>,
    pub audit: FangyuanAuditReport,
}

impl FangyuanBakeValidationReport {
    pub fn passed(&self) -> bool {
        self.audit.summary.error_count == 0
    }
}

#[derive(Debug)]
pub enum FangyuanBakeValidationError {
    Parse {
        target_kind: FangyuanBakeArtifactKind,
        source: String,
    },
    Serialize {
        target_kind: FangyuanBakeArtifactKind,
        source: String,
    },
    Validate {
        target_kind: FangyuanBakeArtifactKind,
        source: String,
    },
    KindMismatch {
        expected: FangyuanBakeArtifactKind,
        actual: FangyuanBakeArtifactKind,
    },
    Format(String),
    UnsupportedMaterialProfileRon,
}

impl fmt::Display for FangyuanBakeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse {
                target_kind,
                source,
            } => {
                write!(formatter, "failed to parse {target_kind} RON: {source}")
            }
            Self::Validate {
                target_kind,
                source,
            } => {
                write!(formatter, "failed to validate {target_kind}: {source}")
            }
            Self::Serialize {
                target_kind,
                source,
            } => {
                write!(formatter, "failed to serialize {target_kind} payload: {source}")
            }
            Self::KindMismatch { expected, actual } => {
                write!(
                    formatter,
                    "payload kind mismatch: expected {expected}, actual {actual}"
                )
            }
            Self::Format(source) => write!(formatter, "bake format error: {source}"),
            Self::UnsupportedMaterialProfileRon => formatter.write_str(
                "material profile RON dry-run is reserved until material profile serde schema is authored",
            ),
        }
    }
}

impl Error for FangyuanBakeValidationError {}

pub fn compile_fangyuan_bake_artifact(
    target_kind: FangyuanBakeArtifactKind,
    source: &str,
    source_path: Option<PathBuf>,
) -> Result<FangyuanBakeCompiledArtifact, FangyuanBakeValidationError> {
    let validation = validate_fangyuan_bake_source(target_kind, source, source_path)?;
    let payload = compile_current_fangyuan_bake_payload(target_kind, &validation.content)?;
    let payload_bytes = encode_fangyuan_bake_payload(&payload)?;
    let content_hash = fangyuan_bake_hash_bytes(&payload_bytes);
    let dependency_table =
        build_fangyuan_bake_dependency_table(&payload, validation.source_path.as_ref());
    let mut stats =
        collect_fangyuan_bake_artifact_stats(&payload, &validation.audit, payload_bytes.len());
    stats.artifact_size = encoded_fangyuan_bake_artifact_size(
        validation.source_hash,
        content_hash,
        target_kind,
        payload_bytes.len(),
    )?;

    Ok(FangyuanBakeCompiledArtifact {
        target_kind,
        source_hash: validation.source_hash,
        content_hash,
        normalized_source_hash: validation.content_hash,
        upgraded_from: validation.upgraded_from,
        payload,
        payload_bytes,
        dependency_table,
        stats,
        audit: validation.audit,
    })
}

pub fn encode_fangyuan_bake_payload(
    payload: &FangyuanBakePayload,
) -> Result<Vec<u8>, FangyuanBakeValidationError> {
    ron::ser::to_string(payload)
        .map(|source| source.into_bytes())
        .map_err(|error| FangyuanBakeValidationError::Serialize {
            target_kind: payload.artifact_kind(),
            source: error.to_string(),
        })
}

pub fn decode_fangyuan_bake_payload(
    target_kind: FangyuanBakeArtifactKind,
    bytes: &[u8],
) -> Result<FangyuanBakePayload, FangyuanBakeValidationError> {
    let source =
        std::str::from_utf8(bytes).map_err(|error| FangyuanBakeValidationError::Parse {
            target_kind,
            source: error.to_string(),
        })?;
    let payload = ron::from_str::<FangyuanBakePayload>(source).map_err(|error| {
        FangyuanBakeValidationError::Parse {
            target_kind,
            source: error.to_string(),
        }
    })?;
    if payload.artifact_kind() != target_kind {
        return Err(FangyuanBakeValidationError::KindMismatch {
            expected: target_kind,
            actual: payload.artifact_kind(),
        });
    }
    Ok(payload)
}

fn encoded_fangyuan_bake_artifact_size(
    source_hash: u64,
    content_hash: u64,
    target_kind: FangyuanBakeArtifactKind,
    payload_len: usize,
) -> Result<usize, FangyuanBakeValidationError> {
    let header = FangyuanBakeArtifactHeader {
        schema_version: FANGYUAN_BAKE_SCHEMA_VERSION,
        source_hash,
        content_hash,
        created_by: "fangyuan_bake".to_string(),
        target_kind,
    };
    encode_fangyuan_bake_artifact(&header, &vec![0; payload_len])
        .map(|bytes| bytes.len())
        .map_err(|error| FangyuanBakeValidationError::Format(error.to_string()))
}

fn compile_current_fangyuan_bake_payload(
    target_kind: FangyuanBakeArtifactKind,
    source: &str,
) -> Result<FangyuanBakePayload, FangyuanBakeValidationError> {
    match target_kind {
        FangyuanBakeArtifactKind::Blueprint => {
            let blueprint = parse_ron::<FangyuanBlueprint>(target_kind, source)?;
            Ok(FangyuanBakePayload::Blueprint {
                payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
                blueprint,
            })
        }
        FangyuanBakeArtifactKind::PrefabPalette => {
            let palette = parse_ron::<FangyuanPrefabPalette>(target_kind, source)?;
            Ok(FangyuanBakePayload::PrefabPalette {
                payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
                palette,
            })
        }
        FangyuanBakeArtifactKind::SceneLayout => {
            let layout = parse_ron::<FangyuanSceneLayout>(target_kind, source)?;
            Ok(FangyuanBakePayload::SceneLayout {
                payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
                layout,
            })
        }
        FangyuanBakeArtifactKind::Chunk => compile_chunk_payload(source),
        FangyuanBakeArtifactKind::MaterialProfile => {
            let profile = parse_material_profile_source(source)?;
            Ok(FangyuanBakePayload::MaterialProfile {
                payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
                profile: FangyuanMaterialProfileArtifact::from_profile(&profile),
            })
        }
        FangyuanBakeArtifactKind::SkillRecipe => compile_skill_recipe_payload(source),
    }
}

fn compile_chunk_payload(source: &str) -> Result<FangyuanBakePayload, FangyuanBakeValidationError> {
    if let Ok(chunk) = ron::from_str::<FangyuanChunkSource>(source) {
        return Ok(FangyuanBakePayload::ChunkSource {
            payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
            chunk,
        });
    }

    let manifest = parse_ron::<FangyuanChunkManifest>(FangyuanBakeArtifactKind::Chunk, source)?;
    Ok(FangyuanBakePayload::ChunkManifest {
        payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
        manifest,
    })
}

fn compile_skill_recipe_payload(
    source: &str,
) -> Result<FangyuanBakePayload, FangyuanBakeValidationError> {
    if source.contains("emitters:") {
        let recipe = parse_ron::<FangyuanVfxRecipe>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
        return Ok(FangyuanBakePayload::VfxRecipe {
            payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
            recipe,
        });
    }
    if source.contains("range_shape:") {
        let template =
            parse_ron::<FangyuanSkillTemplate>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
        return Ok(FangyuanBakePayload::SkillTemplate {
            payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
            template,
        });
    }
    if source.contains("template_id:") {
        let visual = parse_ron::<FangyuanSkillVisualBlueprint>(
            FangyuanBakeArtifactKind::SkillRecipe,
            source,
        )?;
        return Ok(FangyuanBakePayload::SkillVisual {
            payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
            visual,
        });
    }

    if let Ok(template) = ron::from_str::<FangyuanSkillTemplate>(source) {
        return Ok(FangyuanBakePayload::SkillTemplate {
            payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
            template,
        });
    }

    if let Ok(recipe) = ron::from_str::<FangyuanVfxRecipe>(source) {
        return Ok(FangyuanBakePayload::VfxRecipe {
            payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
            recipe,
        });
    }

    let visual =
        parse_ron::<FangyuanSkillVisualBlueprint>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
    Ok(FangyuanBakePayload::SkillVisual {
        payload_version: FANGYUAN_BAKE_PAYLOAD_VERSION,
        visual,
    })
}

pub fn build_fangyuan_bake_dependency_table(
    payload: &FangyuanBakePayload,
    source_path: Option<&PathBuf>,
) -> FangyuanBakeDependencyTable {
    let mut table = FangyuanBakeDependencyTable {
        source_path: source_path.map(|path| path.to_string_lossy().into_owned()),
        ..Default::default()
    };

    match payload {
        FangyuanBakePayload::Blueprint { blueprint, .. } => {
            insert_material_profile_ids_from_primitives(
                &mut table.material_profile_ids,
                blueprint.primitives.iter(),
            );
        }
        FangyuanBakePayload::PrefabPalette { palette, .. } => {
            insert_prefab_ids(
                &mut table.prefab_ids,
                palette.prefabs.iter().map(|prefab| &prefab.id),
            );
            for prefab in &palette.prefabs {
                insert_material_profile_ids_from_primitives(
                    &mut table.material_profile_ids,
                    prefab.primitives.iter(),
                );
            }
        }
        FangyuanBakePayload::SceneLayout { layout, .. } => {
            insert_sorted_unique(
                &mut table.layout_paths,
                source_path
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|| layout.name.clone()),
            );
            for palette_path in layout.palette_paths() {
                insert_sorted_unique(&mut table.palette_paths, palette_path.to_string());
            }
            insert_prefab_ids(
                &mut table.prefab_ids,
                layout.instances.iter().map(|instance| &instance.prefab),
            );
            for palette_path in layout.palette_paths() {
                table.missing.push(FangyuanBakeMissingDependency {
                    owner: layout.name.clone(),
                    kind: FangyuanBakeDependencyKind::Palette,
                    id: palette_path.to_string(),
                });
            }
            for instance in &layout.instances {
                table.missing.push(FangyuanBakeMissingDependency {
                    owner: layout.name.clone(),
                    kind: FangyuanBakeDependencyKind::Prefab,
                    id: instance.prefab.clone(),
                });
            }
        }
        FangyuanBakePayload::ChunkSource { chunk, .. } => {
            insert_sorted_unique(&mut table.chunk_ids, chunk.id.clone());
            insert_prefab_ids(
                &mut table.prefab_ids,
                chunk
                    .prefab_instances
                    .iter()
                    .map(|reference| &reference.prefab),
            );
            for reference in &chunk.static_decorations {
                match &reference.source {
                    super::FangyuanChunkStaticDecorationSourceRef::Prefab { prefab } => {
                        insert_sorted_unique(&mut table.prefab_ids, prefab.clone());
                    }
                    super::FangyuanChunkStaticDecorationSourceRef::Blueprint { blueprint } => {
                        insert_sorted_unique(&mut table.blueprint_paths, blueprint.clone());
                    }
                    super::FangyuanChunkStaticDecorationSourceRef::Bake { bake } => {
                        insert_sorted_unique(&mut table.chunk_paths, bake.clone());
                    }
                }
            }
        }
        FangyuanBakePayload::ChunkManifest { manifest, .. } => {
            insert_sorted_unique(
                &mut table.chunk_ids,
                manifest
                    .world_id
                    .clone()
                    .unwrap_or_else(|| manifest.name.clone()),
            );
            for chunk in &manifest.chunks {
                insert_sorted_unique(&mut table.chunk_ids, chunk.id.clone());
                if let Some(path) = &chunk.dev_ron {
                    insert_sorted_unique(&mut table.chunk_paths, path.clone());
                }
                if let Some(path) = &chunk.bin {
                    insert_sorted_unique(&mut table.chunk_paths, path.clone());
                }
            }
        }
        FangyuanBakePayload::MaterialProfile { profile, .. } => {
            insert_sorted_unique(&mut table.material_profile_ids, profile.stable_id.clone());
        }
        FangyuanBakePayload::SkillTemplate { template, .. } => {
            insert_sorted_unique(&mut table.skill_ids, template.id.clone());
        }
        FangyuanBakePayload::SkillVisual { visual, .. } => {
            insert_sorted_unique(&mut table.skill_ids, visual.id.clone());
            table.missing.push(FangyuanBakeMissingDependency {
                owner: visual.id.clone(),
                kind: FangyuanBakeDependencyKind::Skill,
                id: format!("{}@{}", visual.template_id, visual.template_version),
            });
            if let Some(profile_ref) = &visual.profile_ref {
                insert_sorted_unique(&mut table.material_profile_ids, profile_ref.clone());
                table.missing.push(FangyuanBakeMissingDependency {
                    owner: visual.id.clone(),
                    kind: FangyuanBakeDependencyKind::MaterialProfile,
                    id: profile_ref.clone(),
                });
            }
        }
        FangyuanBakePayload::VfxRecipe { recipe, .. } => {
            insert_sorted_unique(&mut table.skill_ids, recipe.id.clone());
        }
    }

    table
}

fn insert_prefab_ids<'a>(target: &mut Vec<String>, ids: impl IntoIterator<Item = &'a String>) {
    for id in ids {
        insert_sorted_unique(target, id.clone());
    }
}

fn insert_material_profile_ids_from_primitives<'a>(
    target: &mut Vec<String>,
    primitives: impl IntoIterator<Item = &'a super::FangyuanPrimitiveBlueprint>,
) {
    for primitive in primitives {
        if let Some(profile_id) = &primitive.material_profile_id {
            insert_sorted_unique(target, profile_id.clone());
        }
    }
}

fn insert_sorted_unique(target: &mut Vec<String>, value: String) {
    if value.trim().is_empty() || target.iter().any(|existing| existing == &value) {
        return;
    }
    target.push(value);
    target.sort();
}

fn collect_fangyuan_bake_artifact_stats(
    payload: &FangyuanBakePayload,
    audit: &FangyuanAuditReport,
    payload_size: usize,
) -> FangyuanBakeArtifactStats {
    let mut stats = FangyuanBakeArtifactStats {
        primitive_count: audit
            .summary
            .generated_primitives
            .max(audit.summary.authored_primitives),
        prefab_count: audit.summary.prefab_count,
        profile_count: audit.summary.material_count,
        budget: audit
            .summary
            .generated_primitives
            .max(audit.summary.authored_primitives),
        warning_count: audit.summary.warning_count,
        artifact_size: payload_size,
        ..Default::default()
    };

    match payload {
        FangyuanBakePayload::Blueprint { blueprint, .. } => {
            stats.primitive_count = blueprint.primitives.len();
            stats.budget = blueprint.max_primitives;
        }
        FangyuanBakePayload::PrefabPalette { palette, .. } => {
            stats.prefab_count = palette.prefabs.len();
            stats.primitive_count = palette
                .prefabs
                .iter()
                .map(|prefab| prefab.primitives.len())
                .sum();
            stats.budget = palette.max_primitives;
            stats.profile_count = count_material_profiles_in_palette(palette);
        }
        FangyuanBakePayload::SceneLayout { layout, .. } => {
            stats.prefab_count = layout
                .instances
                .iter()
                .map(|instance| instance.prefab.as_str())
                .collect::<BTreeSet<_>>()
                .len();
            stats.budget = layout.max_primitives;
        }
        FangyuanBakePayload::ChunkSource { chunk, .. } => {
            stats.chunk_count = 1;
            stats.prefab_count = chunk
                .prefab_instances
                .iter()
                .map(|reference| reference.prefab.as_str())
                .chain(chunk.static_decorations.iter().filter_map(|reference| {
                    if let super::FangyuanChunkStaticDecorationSourceRef::Prefab { prefab } =
                        &reference.source
                    {
                        Some(prefab.as_str())
                    } else {
                        None
                    }
                }))
                .collect::<BTreeSet<_>>()
                .len();
            stats.budget = chunk.budget.total_cost as usize;
        }
        FangyuanBakePayload::ChunkManifest { manifest, .. } => {
            stats.chunk_count = manifest.chunks.len();
            stats.budget = manifest
                .chunks
                .iter()
                .map(|chunk| chunk.budget.total_cost as usize)
                .sum();
        }
        FangyuanBakePayload::MaterialProfile { .. } => {
            stats.profile_count = 1;
        }
        FangyuanBakePayload::SkillTemplate { .. }
        | FangyuanBakePayload::SkillVisual { .. }
        | FangyuanBakePayload::VfxRecipe { .. } => {}
    }

    stats
}

fn count_material_profiles_in_palette(palette: &FangyuanPrefabPalette) -> usize {
    palette
        .prefabs
        .iter()
        .flat_map(|prefab| &prefab.primitives)
        .filter_map(|primitive| primitive.material_profile_id.as_deref())
        .collect::<BTreeSet<_>>()
        .len()
}

fn resolve_fangyuan_bake_dependencies(
    entries: &mut [FangyuanBakeRunEntry],
    catalog: &FangyuanBakeDependencyCatalog,
) {
    for entry in entries {
        catalog.expand_entry_dependency_table(&mut entry.dependency_table);
        let mut missing = Vec::new();
        for dependency in &entry.dependency_table.missing {
            let resolved = match dependency.kind {
                FangyuanBakeDependencyKind::Layout => {
                    catalog.layout_paths.contains(&dependency.id)
                        || catalog.layout_names.contains(&dependency.id)
                }
                FangyuanBakeDependencyKind::Palette => {
                    catalog.palette_paths.contains(&dependency.id)
                }
                FangyuanBakeDependencyKind::Prefab => catalog.prefab_ids.contains(&dependency.id),
                FangyuanBakeDependencyKind::MaterialProfile => {
                    catalog.material_profile_ids.contains(&dependency.id)
                }
                FangyuanBakeDependencyKind::Blueprint => {
                    catalog.blueprint_paths.contains(&dependency.id)
                }
                FangyuanBakeDependencyKind::Chunk => {
                    catalog.chunk_ids.contains(&dependency.id)
                        || catalog.chunk_paths.contains(&dependency.id)
                }
                FangyuanBakeDependencyKind::Skill => catalog.skill_ids.contains(&dependency.id),
            };
            if !resolved {
                missing.push(dependency.clone());
            }
        }
        entry.dependency_table.missing = missing;
        entry.missing_dependency_count = entry.dependency_table.missing.len();
        entry.passed = entry.passed && entry.missing_dependency_count == 0;
        if entry.missing_dependency_count > 0 {
            entry.error_count = entry
                .error_count
                .saturating_add(entry.missing_dependency_count);
        }
    }
}

#[derive(Default)]
struct FangyuanBakeDependencyCatalog {
    layout_paths: BTreeSet<String>,
    layout_names: BTreeSet<String>,
    palette_paths: BTreeSet<String>,
    prefab_ids: BTreeSet<String>,
    prefab_material_profiles: BTreeMap<String, BTreeSet<String>>,
    material_profile_ids: BTreeSet<String>,
    blueprint_paths: BTreeSet<String>,
    chunk_ids: BTreeSet<String>,
    chunk_paths: BTreeSet<String>,
    skill_ids: BTreeSet<String>,
}

impl FangyuanBakeDependencyCatalog {
    fn add_payload(&mut self, payload: &FangyuanBakePayload, source_path: &Path) {
        let normalized_path = source_path.to_string_lossy().replace('\\', "/");
        match payload {
            FangyuanBakePayload::Blueprint { .. } => {
                self.blueprint_paths.insert(normalized_path);
            }
            FangyuanBakePayload::PrefabPalette { palette, .. } => {
                self.palette_paths.insert(normalized_path.clone());
                if let Some(asset_path) = infer_fangyuan_asset_path(&normalized_path) {
                    self.palette_paths.insert(asset_path);
                }
                for prefab in &palette.prefabs {
                    self.prefab_ids.insert(prefab.id.clone());
                    let profiles = self
                        .prefab_material_profiles
                        .entry(prefab.id.clone())
                        .or_default();
                    for primitive in &prefab.primitives {
                        if let Some(profile_id) = &primitive.material_profile_id {
                            profiles.insert(profile_id.clone());
                        }
                    }
                }
            }
            FangyuanBakePayload::SceneLayout { layout, .. } => {
                self.layout_paths.insert(normalized_path.clone());
                self.layout_names.insert(layout.name.clone());
                if let Some(asset_path) = infer_fangyuan_asset_path(&normalized_path) {
                    self.layout_paths.insert(asset_path);
                }
            }
            FangyuanBakePayload::ChunkSource { chunk, .. } => {
                self.chunk_ids.insert(chunk.id.clone());
                self.chunk_paths.insert(normalized_path);
            }
            FangyuanBakePayload::ChunkManifest { manifest, .. } => {
                if let Some(world_id) = &manifest.world_id {
                    self.chunk_ids.insert(world_id.clone());
                }
                for chunk in &manifest.chunks {
                    self.chunk_ids.insert(chunk.id.clone());
                    if let Some(path) = &chunk.dev_ron {
                        self.chunk_paths.insert(path.clone());
                    }
                    if let Some(path) = &chunk.bin {
                        self.chunk_paths.insert(path.clone());
                    }
                }
            }
            FangyuanBakePayload::MaterialProfile { profile, .. } => {
                self.material_profile_ids.insert(profile.stable_id.clone());
            }
            FangyuanBakePayload::SkillTemplate { template, .. } => {
                self.skill_ids
                    .insert(format!("{}@{}", template.id, template.version));
                self.skill_ids.insert(template.id.clone());
            }
            FangyuanBakePayload::SkillVisual { visual, .. } => {
                self.skill_ids.insert(visual.id.clone());
            }
            FangyuanBakePayload::VfxRecipe { recipe, .. } => {
                self.skill_ids.insert(recipe.id.clone());
            }
        }
    }

    fn expand_entry_dependency_table(&self, table: &mut FangyuanBakeDependencyTable) {
        let prefab_ids = table.prefab_ids.clone();
        for prefab_id in prefab_ids {
            if let Some(profile_ids) = self.prefab_material_profiles.get(&prefab_id) {
                for profile_id in profile_ids {
                    insert_sorted_unique(&mut table.material_profile_ids, profile_id.clone());
                    if !self.material_profile_ids.contains(profile_id)
                        && !table.missing.iter().any(|dependency| {
                            dependency.kind == FangyuanBakeDependencyKind::MaterialProfile
                                && dependency.id == *profile_id
                        })
                    {
                        table.missing.push(FangyuanBakeMissingDependency {
                            owner: prefab_id.clone(),
                            kind: FangyuanBakeDependencyKind::MaterialProfile,
                            id: profile_id.clone(),
                        });
                    }
                }
            }
        }
        table
            .missing
            .sort_by(|left, right| (left.kind, &left.id).cmp(&(right.kind, &right.id)));
    }
}

fn infer_fangyuan_asset_path(normalized_path: &str) -> Option<String> {
    normalized_path
        .find("fangyuan/")
        .map(|index| normalized_path[index..].to_string())
}

pub fn load_fangyuan_runtime_artifact(
    entry: &FangyuanRuntimeArtifactManifestEntry,
    options: FangyuanRuntimeArtifactLoaderOptions,
    available_dependencies: impl IntoIterator<Item = impl AsRef<str>>,
) -> FangyuanRuntimeArtifactLoadReport {
    let available_dependencies = available_dependencies
        .into_iter()
        .map(|dependency| dependency.as_ref().to_string())
        .collect::<BTreeSet<_>>();
    for dependency in &entry.required_dependencies {
        if !available_dependencies.contains(dependency) {
            return FangyuanRuntimeArtifactLoadReport {
                id: entry.id.clone(),
                kind: entry.kind,
                status: FangyuanRuntimeArtifactLoadStatus::Failed,
                source: FangyuanRuntimeArtifactLoadSource::None,
                fallback: FangyuanRuntimeArtifactFallback::Unavailable,
                error: Some(FangyuanRuntimeArtifactLoadError::DependencyMissing {
                    id: dependency.clone(),
                }),
                header: None,
                payload: None,
            };
        }
    }

    match load_fangyuan_runtime_artifact_bin(entry) {
        Ok((header, payload)) => FangyuanRuntimeArtifactLoadReport {
            id: entry.id.clone(),
            kind: entry.kind,
            status: FangyuanRuntimeArtifactLoadStatus::Loaded,
            source: FangyuanRuntimeArtifactLoadSource::Bin,
            fallback: FangyuanRuntimeArtifactFallback::NotNeeded,
            error: None,
            header: Some(header),
            payload: Some(payload),
        },
        Err(bin_error) => {
            if !options.allow_ron_fallback {
                return FangyuanRuntimeArtifactLoadReport {
                    id: entry.id.clone(),
                    kind: entry.kind,
                    status: FangyuanRuntimeArtifactLoadStatus::Failed,
                    source: FangyuanRuntimeArtifactLoadSource::Bin,
                    fallback: FangyuanRuntimeArtifactFallback::Disabled,
                    error: Some(bin_error),
                    header: None,
                    payload: None,
                };
            }

            match load_fangyuan_runtime_artifact_ron(entry) {
                Ok(payload) => FangyuanRuntimeArtifactLoadReport {
                    id: entry.id.clone(),
                    kind: entry.kind,
                    status: FangyuanRuntimeArtifactLoadStatus::FallbackLoaded,
                    source: FangyuanRuntimeArtifactLoadSource::Ron,
                    fallback: FangyuanRuntimeArtifactFallback::Used,
                    error: Some(bin_error),
                    header: None,
                    payload: Some(payload),
                },
                Err(ron_error) => FangyuanRuntimeArtifactLoadReport {
                    id: entry.id.clone(),
                    kind: entry.kind,
                    status: FangyuanRuntimeArtifactLoadStatus::Failed,
                    source: FangyuanRuntimeArtifactLoadSource::None,
                    fallback: FangyuanRuntimeArtifactFallback::Attempted,
                    error: Some(ron_error),
                    header: None,
                    payload: None,
                },
            }
        }
    }
}

fn load_fangyuan_runtime_artifact_bin(
    entry: &FangyuanRuntimeArtifactManifestEntry,
) -> Result<(FangyuanBakeArtifactHeader, FangyuanBakePayload), FangyuanRuntimeArtifactLoadError> {
    let path = entry
        .bin
        .as_ref()
        .ok_or_else(|| FangyuanRuntimeArtifactLoadError::LoadFailed {
            path: PathBuf::from("<missing bin>"),
            message: "manifest entry has no bin path".to_string(),
        })?;
    let bytes = fs::read(path).map_err(|source| FangyuanRuntimeArtifactLoadError::LoadFailed {
        path: path.clone(),
        message: source.to_string(),
    })?;
    let artifact = match decode_fangyuan_bake_artifact(&bytes) {
        Ok(artifact) => artifact,
        Err(FangyuanBakeFormatError::UnsupportedSchemaVersion { found, expected }) => {
            return Err(FangyuanRuntimeArtifactLoadError::VersionMismatch { found, expected });
        }
        Err(error) => return Err(FangyuanRuntimeArtifactLoadError::Format(error)),
    };
    if artifact.header.schema_version != FANGYUAN_BAKE_SCHEMA_VERSION {
        return Err(FangyuanRuntimeArtifactLoadError::VersionMismatch {
            found: artifact.header.schema_version,
            expected: FANGYUAN_BAKE_SCHEMA_VERSION,
        });
    }
    if artifact.header.target_kind != entry.kind {
        return Err(FangyuanRuntimeArtifactLoadError::KindMismatch {
            found: artifact.header.target_kind,
            expected: entry.kind,
        });
    }

    let actual_content_hash = fangyuan_bake_hash_bytes(&artifact.payload);
    if actual_content_hash != artifact.header.content_hash {
        return Err(FangyuanRuntimeArtifactLoadError::HashMismatch {
            hash_kind: FangyuanRuntimeArtifactHashKind::Content,
            expected: artifact.header.content_hash,
            actual: actual_content_hash,
        });
    }
    if let Some(expected_content_hash) = entry.expected_content_hash
        && expected_content_hash != artifact.header.content_hash
    {
        return Err(FangyuanRuntimeArtifactLoadError::HashMismatch {
            hash_kind: FangyuanRuntimeArtifactHashKind::Content,
            expected: expected_content_hash,
            actual: artifact.header.content_hash,
        });
    }
    if let Some(expected_source_hash) = entry.expected_source_hash
        && expected_source_hash != artifact.header.source_hash
    {
        return Err(FangyuanRuntimeArtifactLoadError::HashMismatch {
            hash_kind: FangyuanRuntimeArtifactHashKind::Source,
            expected: expected_source_hash,
            actual: artifact.header.source_hash,
        });
    }

    let payload = decode_fangyuan_bake_payload(entry.kind, &artifact.payload).map_err(|error| {
        FangyuanRuntimeArtifactLoadError::ParseFailed {
            path: path.clone(),
            message: error.to_string(),
        }
    })?;
    if payload.payload_version() != FANGYUAN_BAKE_PAYLOAD_VERSION {
        return Err(FangyuanRuntimeArtifactLoadError::PayloadVersionMismatch {
            found: payload.payload_version(),
            expected: FANGYUAN_BAKE_PAYLOAD_VERSION,
        });
    }

    Ok((artifact.header, payload))
}

fn load_fangyuan_runtime_artifact_ron(
    entry: &FangyuanRuntimeArtifactManifestEntry,
) -> Result<FangyuanBakePayload, FangyuanRuntimeArtifactLoadError> {
    let path = entry
        .ron
        .as_ref()
        .ok_or_else(|| FangyuanRuntimeArtifactLoadError::LoadFailed {
            path: PathBuf::from("<missing ron>"),
            message: "manifest entry has no RON fallback path".to_string(),
        })?;
    let source = fs::read_to_string(path).map_err(|source| {
        FangyuanRuntimeArtifactLoadError::LoadFailed {
            path: path.clone(),
            message: source.to_string(),
        }
    })?;
    let (_, upgraded_source) = load_fangyuan_runtime_artifact_ron_source(entry.kind, &source)
        .map_err(|error| FangyuanRuntimeArtifactLoadError::ParseFailed {
            path: path.clone(),
            message: error.to_string(),
        })?;
    compile_current_fangyuan_bake_payload(entry.kind, &upgraded_source).map_err(|error| {
        FangyuanRuntimeArtifactLoadError::ParseFailed {
            path: path.clone(),
            message: error.to_string(),
        }
    })
}

pub fn load_fangyuan_runtime_artifact_ron_source(
    target_kind: FangyuanBakeArtifactKind,
    source: &str,
) -> Result<(u64, String), FangyuanBakeValidationError> {
    let (upgraded_source, _) = upgrade_fangyuan_bake_source_if_needed(source)?;
    validate_current_fangyuan_bake_source(target_kind, &upgraded_source)?;
    Ok((fangyuan_bake_hash_bytes(source.as_bytes()), upgraded_source))
}

fn parse_fangyuan_bake_hex_hash(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    let without_prefix = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u64::from_str_radix(without_prefix, 16).ok()
}

pub fn validate_fangyuan_bake_source(
    target_kind: FangyuanBakeArtifactKind,
    source: &str,
    source_path: Option<PathBuf>,
) -> Result<FangyuanBakeValidationReport, FangyuanBakeValidationError> {
    let original_hash = fangyuan_bake_hash_bytes(source.as_bytes());
    let (upgraded_source, source_version) = upgrade_fangyuan_bake_source_if_needed(source)?;
    let content_hash = fangyuan_bake_hash_bytes(upgraded_source.as_bytes());
    let mut audit = validate_current_fangyuan_bake_source(target_kind, &upgraded_source)?;
    audit.source_path = source_path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    for finding in &mut audit.findings {
        finding.source_path = audit.source_path.clone();
    }

    Ok(FangyuanBakeValidationReport {
        target_kind,
        source_path,
        content: upgraded_source,
        source_hash: original_hash,
        content_hash,
        upgraded_from: match source_version {
            FangyuanBakeSourceVersion::Current => None,
            FangyuanBakeSourceVersion::LegacyZero => Some(0),
        },
        audit,
    })
}

fn validate_current_fangyuan_bake_source(
    target_kind: FangyuanBakeArtifactKind,
    source: &str,
) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    match target_kind {
        FangyuanBakeArtifactKind::Blueprint => {
            let blueprint = parse_ron::<FangyuanBlueprint>(target_kind, source)?;
            Ok(blueprint.audit_with_default_budget())
        }
        FangyuanBakeArtifactKind::PrefabPalette => {
            let palette = parse_ron::<FangyuanPrefabPalette>(target_kind, source)?;
            Ok(palette.audit_with_default_budget())
        }
        FangyuanBakeArtifactKind::SceneLayout => {
            let layout = parse_ron::<FangyuanSceneLayout>(target_kind, source)?;
            let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::SceneLayout, None);
            if let Err(error) = layout.validate() {
                audit.add_finding(FangyuanAuditFinding::new(
                    FangyuanAuditSeverity::Error,
                    error.code(),
                    error.reason(),
                    FangyuanAuditSourceKind::SceneLayout,
                ));
                if let Some(finding) = audit.findings.last_mut() {
                    finding.field_path = Some(error.field_path().into_owned());
                }
            }
            Ok(audit)
        }
        FangyuanBakeArtifactKind::Chunk => validate_chunk_source(source),
        FangyuanBakeArtifactKind::MaterialProfile => validate_material_profile_source(source),
        FangyuanBakeArtifactKind::SkillRecipe => validate_skill_recipe_source(source),
    }
}

fn validate_chunk_source(source: &str) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    if let Ok(chunk) = ron::from_str::<FangyuanChunkSource>(source) {
        let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::Unknown, None);
        if let Err(error) = chunk.validate() {
            audit.add_finding(FangyuanAuditFinding::new(
                FangyuanAuditSeverity::Error,
                error.code(),
                error.reason(),
                FangyuanAuditSourceKind::Unknown,
            ));
            if let Some(finding) = audit.findings.last_mut() {
                finding.field_path = Some(error.field_path().into_owned());
            }
        }
        return Ok(audit);
    }

    let manifest = parse_ron::<FangyuanChunkManifest>(FangyuanBakeArtifactKind::Chunk, source)?;
    let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::Unknown, None);
    if let Err(error) = manifest.validate() {
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            error.code(),
            error.reason(),
            FangyuanAuditSourceKind::Unknown,
        ));
        if let Some(finding) = audit.findings.last_mut() {
            finding.field_path = Some(error.field_path().into_owned());
        }
    }
    Ok(audit)
}

fn validate_material_profile_source(
    source: &str,
) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    let profile = parse_material_profile_source(source)?;
    let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::RuntimePrimitiveSet, None);
    if let Err(error) = profile.validate() {
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            material_profile_error_code(&error),
            format!("{error:?}"),
            FangyuanAuditSourceKind::RuntimePrimitiveSet,
        ));
        if let Some(finding) = audit.findings.last_mut() {
            finding.field_path = Some(material_profile_error_field_path(&error));
        }
    }
    Ok(audit)
}

fn validate_skill_recipe_source(
    source: &str,
) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    if source.contains("emitters:") {
        return validate_vfx_recipe_source(source);
    }
    if source.contains("range_shape:") {
        return validate_skill_template_source(source);
    }
    if source.contains("template_id:") {
        return validate_skill_visual_source(source);
    }

    if let Ok(template) = ron::from_str::<FangyuanSkillTemplate>(source) {
        return Ok(validate_skill_template(template));
    }

    if let Ok(recipe) = ron::from_str::<FangyuanVfxRecipe>(source) {
        return Ok(validate_vfx_recipe(recipe));
    }

    let visual =
        parse_ron::<FangyuanSkillVisualBlueprint>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
    Ok(validate_skill_visual(visual))
}

fn validate_skill_template_source(
    source: &str,
) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    let template =
        parse_ron::<FangyuanSkillTemplate>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
    Ok(validate_skill_template(template))
}

fn validate_skill_template(template: FangyuanSkillTemplate) -> FangyuanAuditReport {
    let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::Unknown, None);
    if let Err(error) = template.validate() {
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            format!("{:?}", error.code).to_ascii_lowercase(),
            error.message,
            FangyuanAuditSourceKind::Unknown,
        ));
        if let Some(finding) = audit.findings.last_mut() {
            finding.field_path = error.field_path;
        }
    }
    audit
}

fn validate_vfx_recipe_source(
    source: &str,
) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    let recipe = parse_ron::<FangyuanVfxRecipe>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
    Ok(validate_vfx_recipe(recipe))
}

fn validate_vfx_recipe(recipe: FangyuanVfxRecipe) -> FangyuanAuditReport {
    let mut audit = super::audit_fangyuan_vfx_recipe(&recipe);
    if let Err(error) = recipe.validate() {
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            format!("{:?}", error.code).to_ascii_lowercase(),
            error.message,
            FangyuanAuditSourceKind::Unknown,
        ));
        if let Some(finding) = audit.findings.last_mut() {
            finding.field_path = error
                .emitter_index
                .map(|index| format!("emitters[{index}]"));
        }
    }
    audit
}

fn validate_skill_visual_source(
    source: &str,
) -> Result<FangyuanAuditReport, FangyuanBakeValidationError> {
    let visual =
        parse_ron::<FangyuanSkillVisualBlueprint>(FangyuanBakeArtifactKind::SkillRecipe, source)?;
    Ok(validate_skill_visual(visual))
}

fn validate_skill_visual(visual: FangyuanSkillVisualBlueprint) -> FangyuanAuditReport {
    let mut audit = FangyuanAuditReport::new(FangyuanAuditSourceKind::Unknown, None);
    let templates = super::FangyuanSkillTemplateRegistry::with_defaults();
    if let Err(error) = visual.validate(&templates) {
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            format!("{:?}", error.code).to_ascii_lowercase(),
            error.message,
            FangyuanAuditSourceKind::Unknown,
        ));
        if let Some(finding) = audit.findings.last_mut() {
            finding.field_path = error.field_path;
        }
    }
    audit
}

fn parse_ron<T: DeserializeOwned>(
    target_kind: FangyuanBakeArtifactKind,
    source: &str,
) -> Result<T, FangyuanBakeValidationError> {
    ron::from_str::<T>(source).map_err(|error| FangyuanBakeValidationError::Parse {
        target_kind,
        source: error.to_string(),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialProfileRon {
    stable_id: String,
    version: String,
    debug_label: String,
}

fn parse_material_profile_source(
    source: &str,
) -> Result<FangyuanMaterialProfile, FangyuanBakeValidationError> {
    let profile = ron::from_str::<MaterialProfileRon>(source).map_err(|error| {
        FangyuanBakeValidationError::Parse {
            target_kind: FangyuanBakeArtifactKind::MaterialProfile,
            source: error.to_string(),
        }
    })?;

    Ok(FangyuanMaterialProfile {
        stable_id: profile.stable_id,
        version: profile.version,
        debug_label: profile.debug_label,
        ..FangyuanMaterialProfile::default_profile()
    })
}

fn material_profile_error_code(
    error: &super::FangyuanMaterialProfileValidationError,
) -> &'static str {
    match error {
        super::FangyuanMaterialProfileValidationError::InvalidStableId { .. } => {
            "invalid_stable_id"
        }
        super::FangyuanMaterialProfileValidationError::UnsupportedVersion { .. } => {
            "unsupported_version"
        }
        super::FangyuanMaterialProfileValidationError::InvalidDebugLabel { .. } => {
            "invalid_debug_label"
        }
        super::FangyuanMaterialProfileValidationError::InvalidBaseParams(_) => {
            "invalid_base_params"
        }
        super::FangyuanMaterialProfileValidationError::InvalidAlphaPolicy(_) => {
            "invalid_alpha_policy"
        }
        super::FangyuanMaterialProfileValidationError::InvalidEmissivePolicy(_) => {
            "invalid_emissive_policy"
        }
    }
}

fn material_profile_error_field_path(
    error: &super::FangyuanMaterialProfileValidationError,
) -> String {
    match error {
        super::FangyuanMaterialProfileValidationError::InvalidStableId { .. } => {
            "stable_id".to_string()
        }
        super::FangyuanMaterialProfileValidationError::UnsupportedVersion { .. } => {
            "version".to_string()
        }
        super::FangyuanMaterialProfileValidationError::InvalidDebugLabel { .. } => {
            "debug_label".to_string()
        }
        super::FangyuanMaterialProfileValidationError::InvalidBaseParams(_) => "base".to_string(),
        super::FangyuanMaterialProfileValidationError::InvalidAlphaPolicy(_) => {
            "alpha_policy".to_string()
        }
        super::FangyuanMaterialProfileValidationError::InvalidEmissivePolicy(_) => {
            "emissive_policy".to_string()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanBakeCliOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub dry_run: bool,
    pub clean_output: bool,
    pub report: Option<PathBuf>,
}

impl FangyuanBakeCliOptions {
    pub fn parse_from<I, S>(args: I) -> Result<Self, FangyuanBakeCliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut input = None;
        let mut output = None;
        let mut dry_run = false;
        let mut clean_output = false;
        let mut report = None;
        let mut args = args.into_iter().map(Into::into);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--input" => {
                    input = Some(PathBuf::from(args.next().ok_or_else(|| {
                        FangyuanBakeCliError::InvalidArgs(
                            "--input requires a directory".to_string(),
                        )
                    })?));
                }
                "--output" => {
                    output = Some(PathBuf::from(args.next().ok_or_else(|| {
                        FangyuanBakeCliError::InvalidArgs(
                            "--output requires a directory".to_string(),
                        )
                    })?));
                }
                "--dry-run" => dry_run = true,
                "--clean-output" => clean_output = true,
                "--report" => {
                    report = Some(PathBuf::from(args.next().ok_or_else(|| {
                        FangyuanBakeCliError::InvalidArgs("--report requires a path".to_string())
                    })?));
                }
                "--help" | "-h" => {
                    return Err(FangyuanBakeCliError::Help(fangyuan_bake_cli_usage()));
                }
                other => {
                    return Err(FangyuanBakeCliError::InvalidArgs(format!(
                        "unknown argument `{other}`"
                    )));
                }
            }
        }

        Ok(Self {
            input: input.ok_or_else(|| {
                FangyuanBakeCliError::InvalidArgs("--input <dir> is required".to_string())
            })?,
            output: output.ok_or_else(|| {
                FangyuanBakeCliError::InvalidArgs("--output <dir> is required".to_string())
            })?,
            dry_run,
            clean_output,
            report,
        })
    }
}

pub fn fangyuan_bake_cli_usage() -> String {
    "usage: fangyuan_bake --input <dir> --output <dir> [--dry-run] [--clean-output] [--report <path>]"
        .to_string()
}

#[derive(Debug)]
pub enum FangyuanBakeCliError {
    Help(String),
    InvalidArgs(String),
    Io { path: PathBuf, source: io::Error },
    Validation(FangyuanBakeValidationError),
    Format(FangyuanBakeFormatError),
}

impl fmt::Display for FangyuanBakeCliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help(usage) => formatter.write_str(usage),
            Self::InvalidArgs(message) => formatter.write_str(message),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Validation(error) => write!(formatter, "{error}"),
            Self::Format(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for FangyuanBakeCliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Validation(source) => Some(source),
            Self::Format(source) => Some(source),
            Self::Help(_) | Self::InvalidArgs(_) => None,
        }
    }
}

impl From<FangyuanBakeValidationError> for FangyuanBakeCliError {
    fn from(error: FangyuanBakeValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<FangyuanBakeFormatError> for FangyuanBakeCliError {
    fn from(error: FangyuanBakeFormatError) -> Self {
        Self::Format(error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanBakeRunReport {
    pub dry_run: bool,
    pub clean_output: bool,
    pub input: PathBuf,
    pub output: PathBuf,
    pub entries: Vec<FangyuanBakeRunEntry>,
}

impl FangyuanBakeRunReport {
    pub fn failed_count(&self) -> usize {
        self.entries.iter().filter(|entry| !entry.passed).count()
    }

    pub fn passed(&self) -> bool {
        self.failed_count() == 0
    }

    pub fn peak_resource_count(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.peak_resource_count)
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanBakeRunEntry {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub target_kind: FangyuanBakeArtifactKind,
    pub source_bytes: usize,
    pub source_hash: u64,
    pub content_hash: u64,
    pub normalized_source_hash: u64,
    pub ron_load_us: u128,
    pub bin_load_us: u128,
    pub peak_resource_count: usize,
    pub upgraded_from: Option<u16>,
    pub passed: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub missing_dependency_count: usize,
    pub dependency_table: FangyuanBakeDependencyTable,
    pub stats: FangyuanBakeArtifactStats,
}

pub fn run_fangyuan_bake_cli(
    options: &FangyuanBakeCliOptions,
) -> Result<FangyuanBakeRunReport, FangyuanBakeCliError> {
    let mut entries = Vec::new();
    let mut catalog = FangyuanBakeDependencyCatalog::default();
    let files = collect_ron_files(&options.input)?;
    if !options.dry_run {
        prepare_fangyuan_bake_output_dir(options)?;
    }

    for source_path in files {
        let source =
            fs::read_to_string(&source_path).map_err(|source| FangyuanBakeCliError::Io {
                path: source_path.clone(),
                source,
            })?;
        let source_bytes = source.len();
        let target_kind =
            infer_fangyuan_bake_artifact_kind(&source_path, &source).ok_or_else(|| {
                FangyuanBakeCliError::InvalidArgs(format!(
                    "cannot infer Fangyuan bake artifact kind for {}",
                    source_path.display()
                ))
            })?;
        let ron_load_start = Instant::now();
        let compiled =
            compile_fangyuan_bake_artifact(target_kind, &source, Some(source_path.clone()))?;
        let ron_load_us = ron_load_start.elapsed().as_micros();
        let bin_load_us = profile_fangyuan_bake_bin_load_us(&compiled)?;
        let peak_resource_count = fangyuan_bake_peak_resource_count(&compiled.stats);
        catalog.add_payload(&compiled.payload, &source_path);
        let output_path =
            output_path_for(&options.input, &options.output, &source_path, target_kind);

        if !options.dry_run {
            let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
            fs::create_dir_all(parent).map_err(|source| FangyuanBakeCliError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
            let bytes = encode_fangyuan_bake_compiled_artifact(&compiled, "fangyuan_bake")?;
            fs::write(&output_path, bytes).map_err(|source| FangyuanBakeCliError::Io {
                path: output_path.clone(),
                source,
            })?;
        }

        entries.push(FangyuanBakeRunEntry {
            source_path,
            output_path,
            target_kind,
            source_bytes,
            source_hash: compiled.source_hash,
            content_hash: compiled.content_hash,
            normalized_source_hash: compiled.normalized_source_hash,
            ron_load_us,
            bin_load_us,
            peak_resource_count,
            upgraded_from: compiled.upgraded_from,
            passed: compiled.audit.summary.error_count == 0,
            error_count: compiled.audit.summary.error_count,
            warning_count: compiled.audit.summary.warning_count,
            missing_dependency_count: compiled.dependency_table.missing.len(),
            dependency_table: compiled.dependency_table,
            stats: compiled.stats,
        });
    }

    resolve_fangyuan_bake_dependencies(&mut entries, &catalog);

    let report = FangyuanBakeRunReport {
        dry_run: options.dry_run,
        clean_output: options.clean_output && !options.dry_run,
        input: options.input.clone(),
        output: options.output.clone(),
        entries,
    };

    if let Some(report_path) = &options.report {
        write_fangyuan_bake_report(report_path, &report)?;
    }

    Ok(report)
}

pub fn fangyuan_bake_cli_exit_code(report: &FangyuanBakeRunReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

fn prepare_fangyuan_bake_output_dir(
    options: &FangyuanBakeCliOptions,
) -> Result<(), FangyuanBakeCliError> {
    if options.clean_output {
        validate_fangyuan_bake_clean_output_paths(&options.input, &options.output)?;
        if options.output.exists() {
            if !options.output.is_dir() {
                return Err(FangyuanBakeCliError::InvalidArgs(format!(
                    "--clean-output requires an output directory, found file {}",
                    options.output.display()
                )));
            }
            fs::remove_dir_all(&options.output).map_err(|source| FangyuanBakeCliError::Io {
                path: options.output.clone(),
                source,
            })?;
        }
    }

    fs::create_dir_all(&options.output).map_err(|source| FangyuanBakeCliError::Io {
        path: options.output.clone(),
        source,
    })
}

fn validate_fangyuan_bake_clean_output_paths(
    input: &Path,
    output: &Path,
) -> Result<(), FangyuanBakeCliError> {
    let input = normalize_fangyuan_bake_path(input)?;
    let output = normalize_fangyuan_bake_path(output)?;
    if input == output || input.starts_with(&output) || output.starts_with(&input) {
        return Err(FangyuanBakeCliError::InvalidArgs(format!(
            "--clean-output requires disjoint input and output directories: input={} output={}",
            input.display(),
            output.display()
        )));
    }
    Ok(())
}

fn normalize_fangyuan_bake_path(path: &Path) -> Result<PathBuf, FangyuanBakeCliError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| FangyuanBakeCliError::Io {
                path: PathBuf::from("."),
                source,
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

fn profile_fangyuan_bake_bin_load_us(
    compiled: &FangyuanBakeCompiledArtifact,
) -> Result<u128, FangyuanBakeCliError> {
    let bytes = encode_fangyuan_bake_compiled_artifact(compiled, "fangyuan_bake_profile")?;
    let bin_load_start = Instant::now();
    let artifact = decode_fangyuan_bake_artifact(&bytes)?;
    decode_fangyuan_bake_payload(compiled.target_kind, &artifact.payload)?;
    Ok(bin_load_start.elapsed().as_micros())
}

fn encode_fangyuan_bake_compiled_artifact(
    compiled: &FangyuanBakeCompiledArtifact,
    created_by: &str,
) -> Result<Vec<u8>, FangyuanBakeCliError> {
    let header = FangyuanBakeArtifactHeader {
        schema_version: FANGYUAN_BAKE_SCHEMA_VERSION,
        source_hash: compiled.source_hash,
        content_hash: compiled.content_hash,
        created_by: created_by.to_string(),
        target_kind: compiled.target_kind,
    };
    Ok(encode_fangyuan_bake_artifact(
        &header,
        &compiled.payload_bytes,
    )?)
}

fn fangyuan_bake_peak_resource_count(stats: &FangyuanBakeArtifactStats) -> usize {
    stats
        .primitive_count
        .saturating_add(stats.prefab_count)
        .saturating_add(stats.chunk_count)
        .saturating_add(stats.profile_count)
}

pub fn infer_fangyuan_bake_artifact_kind(
    path: &Path,
    source: &str,
) -> Option<FangyuanBakeArtifactKind> {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if normalized.contains("/palettes/") || normalized.contains("palette") {
        return Some(FangyuanBakeArtifactKind::PrefabPalette);
    }
    if normalized.contains("/layouts/") || normalized.contains("layout") {
        return Some(FangyuanBakeArtifactKind::SceneLayout);
    }
    if normalized.contains("/chunks/") || normalized.contains("chunk") {
        return Some(FangyuanBakeArtifactKind::Chunk);
    }
    if normalized.contains("material") {
        return Some(FangyuanBakeArtifactKind::MaterialProfile);
    }
    if normalized.contains("skill") || normalized.contains("vfx") {
        return Some(FangyuanBakeArtifactKind::SkillRecipe);
    }
    if source.contains("prefabs:") {
        Some(FangyuanBakeArtifactKind::PrefabPalette)
    } else if source.contains("instances:") && source.contains("palette") {
        Some(FangyuanBakeArtifactKind::SceneLayout)
    } else if source.contains("chunks:")
        || source.contains("prefab_instances:")
        || source.contains("static_decorations:")
    {
        Some(FangyuanBakeArtifactKind::Chunk)
    } else if source.contains("stable_id:") && source.contains("debug_label:") {
        Some(FangyuanBakeArtifactKind::MaterialProfile)
    } else if source.contains("range_shape:")
        || source.contains("template_id:")
        || source.contains("emitters:")
    {
        Some(FangyuanBakeArtifactKind::SkillRecipe)
    } else if source.contains("primitives:") && source.contains("bounds:") {
        Some(FangyuanBakeArtifactKind::Blueprint)
    } else {
        None
    }
}

fn collect_ron_files(input: &Path) -> Result<Vec<PathBuf>, FangyuanBakeCliError> {
    let mut files = Vec::new();
    collect_ron_files_inner(input, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_ron_files_inner(
    input: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), FangyuanBakeCliError> {
    for entry in fs::read_dir(input).map_err(|source| FangyuanBakeCliError::Io {
        path: input.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| FangyuanBakeCliError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| FangyuanBakeCliError::Io {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_ron_files_inner(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("ron") {
            files.push(path);
        }
    }
    Ok(())
}

fn output_path_for(
    input: &Path,
    output: &Path,
    source_path: &Path,
    kind: FangyuanBakeArtifactKind,
) -> PathBuf {
    let relative = source_path.strip_prefix(input).unwrap_or(source_path);
    let mut output_path = output.join(relative);
    let file_name = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| format!("{}.{}.fyb", stem, kind.file_stem_suffix()))
        .unwrap_or_else(|| format!("artifact.{}.fyb", kind.file_stem_suffix()));
    output_path.set_file_name(file_name);
    output_path
}

fn write_fangyuan_bake_report(
    report_path: &Path,
    report: &FangyuanBakeRunReport,
) -> Result<(), FangyuanBakeCliError> {
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| FangyuanBakeCliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(report_path, format_fangyuan_bake_run_report(report)).map_err(|source| {
        FangyuanBakeCliError::Io {
            path: report_path.to_path_buf(),
            source,
        }
    })
}

pub fn format_fangyuan_bake_run_report(report: &FangyuanBakeRunReport) -> String {
    let mut lines = vec![format!(
        "dry_run={}; clean_output={}; input={}; output={}; entries={}; failed={}; peak_resource_count={}; load_error_model=ron(parse+upgrade+validate),bin(header+schema+kind+hash+payload)",
        report.dry_run,
        report.clean_output,
        report.input.display(),
        report.output.display(),
        report.entries.len(),
        report.failed_count(),
        report.peak_resource_count()
    )];
    for entry in &report.entries {
        lines.push(format!(
            "{} kind={} output={} passed={} errors={} warnings={} missing_dependencies={} source_bytes={} primitives={} prefabs={} chunks={} profiles={} budget={} artifact_size={} peak_resource_count={} ron_load_us={} bin_load_us={} source_hash={:016x} normalized_source_hash={:016x} content_hash={:016x} upgraded_from={}",
            entry.source_path.display(),
            entry.target_kind,
            entry.output_path.display(),
            entry.passed,
            entry.error_count,
            entry.warning_count,
            entry.missing_dependency_count,
            entry.source_bytes,
            entry.stats.primitive_count,
            entry.stats.prefab_count,
            entry.stats.chunk_count,
            entry.stats.profile_count,
            entry.stats.budget,
            entry.stats.artifact_size,
            entry.peak_resource_count,
            entry.ron_load_us,
            entry.bin_load_us,
            entry.source_hash,
            entry.normalized_source_hash,
            entry.content_hash,
            entry
                .upgraded_from
                .map(|version| version.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        if !entry.dependency_table.is_complete() {
            let missing = entry
                .dependency_table
                .missing
                .iter()
                .map(|dependency| format!("{:?}:{}", dependency.kind, dependency.id))
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "  dependencies complete=false missing=[{}]",
                missing
            ));
        }
    }
    lines.join("\n")
}

fn read_exact<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], FangyuanBakeFormatError> {
    if bytes.len() < *cursor + N {
        return Err(FangyuanBakeFormatError::TruncatedHeader {
            len: bytes.len(),
            min: *cursor + N,
        });
    }
    let mut output = [0u8; N];
    output.copy_from_slice(&bytes[*cursor..*cursor + N]);
    *cursor += N;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fangyuan_bake_format_header_roundtrips_and_carries_required_fields() {
        let source = br#"(version: "1")"#;
        let payload = b"compiled payload";
        let header = FangyuanBakeArtifactHeader::new(
            source,
            payload,
            "fangyuan_bake_test",
            FangyuanBakeArtifactKind::Blueprint,
        );

        let bytes = encode_fangyuan_bake_artifact(&header, payload).unwrap();
        let artifact = decode_fangyuan_bake_artifact(&bytes).unwrap();

        assert_eq!(artifact.header, header);
        assert_eq!(artifact.payload, payload);
        assert_eq!(
            artifact.header.target_kind,
            FangyuanBakeArtifactKind::Blueprint
        );
        assert_eq!(artifact.header.schema_version, FANGYUAN_BAKE_SCHEMA_VERSION);
        artifact.header.validate_hashes(source, payload).unwrap();
    }

    #[test]
    fn fangyuan_bake_format_rejects_schema_mismatch_and_hash_mismatch() {
        let source = b"source";
        let payload = b"payload";
        let mut header = FangyuanBakeArtifactHeader::new(
            source,
            payload,
            "tester",
            FangyuanBakeArtifactKind::Chunk,
        );
        header.schema_version = FANGYUAN_BAKE_SCHEMA_VERSION + 1;

        let error = encode_fangyuan_bake_artifact(&header, payload).unwrap_err();
        assert!(matches!(
            error,
            FangyuanBakeFormatError::UnsupportedSchemaVersion { .. }
        ));

        header.schema_version = FANGYUAN_BAKE_SCHEMA_VERSION;
        header
            .validate_hashes(b"different", payload)
            .expect_err("source hash should mismatch");
        header
            .validate_hashes(source, b"different")
            .expect_err("content hash should mismatch");
    }

    #[test]
    fn fangyuan_bake_format_records_binary_format_decision_without_new_dependencies() {
        let decisions = fangyuan_bake_format_decisions();
        let selected = decisions
            .iter()
            .filter(|decision| decision.selected)
            .count();

        assert_eq!(selected, 1);
        let selected_decision = &decisions[0];
        assert_eq!(selected_decision.name, FANGYUAN_BAKE_FORMAT_NAME);
        assert!(selected_decision.reason.contains("typed RON payload"));
        assert!(selected_decision.reason.contains("runtime loader"));
        assert!(
            selected_decision
                .limitations
                .iter()
                .any(|limitation| limitation.contains("不是紧凑二进制 codec"))
        );
        assert!(selected_decision.dependency_impact.contains("不新增依赖"));
        assert!(
            selected_decision
                .dependency_impact
                .contains("runtime loader")
        );
        assert!(!selected_decision.reason.contains("阶段 5"));
        assert!(
            !selected_decision
                .limitations
                .iter()
                .any(|limitation| limitation.contains("payload 仍是源"))
        );
        assert!(decisions.iter().any(|decision| decision.name == "bincode"));
        assert!(decisions.iter().any(|decision| decision.name == "postcard"));
    }

    #[test]
    fn fangyuan_bake_validation_reuses_blueprint_audit_and_version_upgrade() {
        let source = r#"
(
    version: "0",
    name: "legacy_avatar",
    description: "",
    max_primitives: 8,
    bounds: (width: 4.0, depth: 4.0, height: 4.0),
    primitives: [
        (
            kind: "cube",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [0.2, 0.4, 0.6, 1.0],
        ),
    ],
)
"#;

        let report = validate_fangyuan_bake_source(
            FangyuanBakeArtifactKind::Blueprint,
            source,
            Some(PathBuf::from("legacy.ron")),
        )
        .unwrap();

        assert!(report.passed());
        assert_eq!(report.upgraded_from, Some(0));
        assert_ne!(report.source_hash, report.content_hash);
        assert_eq!(report.audit.source_kind, FangyuanAuditSourceKind::Blueprint);
        assert_eq!(report.audit.summary.generated_primitives, 1);
    }

    #[test]
    fn fangyuan_bake_validation_reports_scene_layout_errors_without_palette_compile() {
        let source = r#"
(
    version: "1",
    name: "bad_layout",
    description: "",
    bounds: (width: 10.0, depth: 10.0, height: 8.0),
    palette: "fangyuan/palettes/home_prefabs.ron",
    max_primitives: 8,
    instances: [
        (
            prefab: "bad id",
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        ),
    ],
)
"#;

        let report =
            validate_fangyuan_bake_source(FangyuanBakeArtifactKind::SceneLayout, source, None)
                .unwrap();

        assert!(!report.passed());
        assert_eq!(report.audit.summary.error_count, 1);
        assert_eq!(report.audit.findings[0].code, "invalid_instance_prefab_id");
        assert_eq!(
            report.audit.findings[0].field_path.as_deref(),
            Some("instances[0].prefab")
        );
    }

    #[test]
    fn fangyuan_bake_validation_covers_chunk_material_and_skill_targets() {
        let chunk = r#"
(
    version: "1",
    id: "home_chunk_0",
    name: "Home Chunk 0",
    description: "",
    bounds: (min: [-8.0, 0.0, -8.0], max: [8.0, 6.0, 8.0]),
    region: (region_id: "home.default", layer: "ground", tags: []),
    prefab_instances: [
        (
            id: "stone_a",
            prefab: "stone_block",
            transform: (position: [0.0, 0.0, 0.0], scale: [1.0, 1.0, 1.0]),
            budget_cost: 5,
        ),
    ],
    budget: (
        prefab_instance_count: 1,
        tiandao_ref_count: 0,
        static_decoration_count: 0,
        total_ref_count: 1,
        prefab_cost: 5,
        tiandao_cost: 0,
        static_decoration_cost: 0,
        total_cost: 5,
    ),
)
"#;
        let material = r#"
(
    stable_id: "fx/test",
    version: "1",
    debug_label: "FX Test",
)
"#;
        let skill = r#"
(
    id: "skill.recipe.test",
    version: 1,
    duration_ticks: 10,
    emitters: [
        (
            id: "impact",
            operators: [
                (type: "spawn"),
            ],
        ),
    ],
)
"#;

        assert!(
            validate_fangyuan_bake_source(FangyuanBakeArtifactKind::Chunk, chunk, None)
                .unwrap()
                .passed()
        );
        assert!(
            validate_fangyuan_bake_source(
                FangyuanBakeArtifactKind::MaterialProfile,
                material,
                None
            )
            .unwrap()
            .passed()
        );
        assert!(
            validate_fangyuan_bake_source(FangyuanBakeArtifactKind::SkillRecipe, skill, None)
                .unwrap()
                .passed()
        );
    }

    #[test]
    fn fangyuan_bake_cli_parses_required_flags() {
        let options = FangyuanBakeCliOptions::parse_from([
            "--input",
            "assets/fangyuan",
            "--output",
            "artifacts/fangyuan",
            "--dry-run",
            "--clean-output",
            "--report",
            "reports/bake.txt",
        ])
        .unwrap();

        assert_eq!(options.input, PathBuf::from("assets/fangyuan"));
        assert_eq!(options.output, PathBuf::from("artifacts/fangyuan"));
        assert!(options.dry_run);
        assert!(options.clean_output);
        assert_eq!(options.report, Some(PathBuf::from("reports/bake.txt")));
    }

    #[test]
    fn fangyuan_bake_cli_dry_run_writes_only_report() {
        let temp = std::env::temp_dir().join(format!("fangyuan_bake_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let input = temp.join("input");
        let output = temp.join("output");
        let report_path = temp.join("report.txt");
        fs::create_dir_all(&input).unwrap();
        fs::write(
            input.join("avatar_blueprint.ron"),
            r#"
(
    version: "1",
    name: "dry_run_avatar",
    description: "",
    max_primitives: 8,
    bounds: (width: 4.0, depth: 4.0, height: 4.0),
    primitives: [
        (
            kind: "cube",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [0.2, 0.4, 0.6, 1.0],
        ),
    ],
)
"#,
        )
        .unwrap();

        let options = FangyuanBakeCliOptions {
            input: input.clone(),
            output: output.clone(),
            dry_run: true,
            clean_output: false,
            report: Some(report_path.clone()),
        };

        let report = run_fangyuan_bake_cli(&options).unwrap();

        assert!(report.passed());
        assert_eq!(report.entries.len(), 1);
        assert!(report.entries[0].ron_load_us > 0);
        assert!(report.entries[0].bin_load_us > 0);
        assert_eq!(report.entries[0].peak_resource_count, 1);
        assert!(report_path.is_file());
        assert!(
            !output.exists(),
            "dry-run must not create output artifacts or output directory"
        );
        let report_text = fs::read_to_string(&report_path).unwrap();
        assert!(report_text.contains(
            "load_error_model=ron(parse+upgrade+validate),bin(header+schema+kind+hash+payload)"
        ));
        assert!(report_text.contains("source_bytes="));
        assert!(report_text.contains("peak_resource_count="));
        assert!(report_text.contains("ron_load_us="));
        assert!(report_text.contains("bin_load_us="));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_bake_cli_incremental_bake_preserves_unrelated_output_files() {
        let temp = unique_temp_dir("fangyuan_bake_incremental_test");
        let input = temp.join("input");
        let output = temp.join("output");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir_all(&output).unwrap();
        let stale_path = output.join("manual.keep");
        fs::write(&stale_path, "manual output").unwrap();
        fs::write(
            input.join("avatar_blueprint.ron"),
            sample_blueprint_ron("incremental_avatar", [0.2, 0.4, 0.6, 1.0]),
        )
        .unwrap();

        let report = run_fangyuan_bake_cli(&FangyuanBakeCliOptions {
            input,
            output,
            dry_run: false,
            clean_output: false,
            report: None,
        })
        .unwrap();

        assert!(report.passed());
        assert!(!report.clean_output);
        assert!(stale_path.is_file());
        assert!(report.entries[0].output_path.is_file());

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_bake_cli_clean_output_removes_stale_files_before_writing_artifacts() {
        let temp = unique_temp_dir("fangyuan_bake_clean_output_test");
        let input = temp.join("input");
        let output = temp.join("output");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir_all(output.join("nested")).unwrap();
        let stale_path = output.join("nested").join("stale.fyb");
        fs::write(&stale_path, "old artifact").unwrap();
        let source_path = input.join("avatar_blueprint.ron");
        fs::write(
            &source_path,
            sample_blueprint_ron("clean_avatar", [0.2, 0.4, 0.6, 1.0]),
        )
        .unwrap();

        let report = run_fangyuan_bake_cli(&FangyuanBakeCliOptions {
            input: input.clone(),
            output,
            dry_run: false,
            clean_output: true,
            report: None,
        })
        .unwrap();

        assert!(report.passed());
        assert!(report.clean_output);
        assert!(source_path.is_file());
        assert!(!stale_path.exists());
        assert!(report.entries[0].output_path.is_file());
        assert!(matches!(
            decode_fangyuan_bake_artifact(&fs::read(&report.entries[0].output_path).unwrap()),
            Ok(FangyuanBakeArtifact { .. })
        ));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_bake_cli_writes_artifact_when_not_dry_run() {
        let temp =
            std::env::temp_dir().join(format!("fangyuan_bake_write_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let input = temp.join("input");
        let output = temp.join("output");
        fs::create_dir_all(&input).unwrap();
        fs::write(
            input.join("avatar_blueprint.ron"),
            r#"
(
    version: "1",
    name: "write_avatar",
    description: "",
    max_primitives: 8,
    bounds: (width: 4.0, depth: 4.0, height: 4.0),
    primitives: [
        (
            kind: "cube",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [0.2, 0.4, 0.6, 1.0],
        ),
    ],
)
"#,
        )
        .unwrap();

        let options = FangyuanBakeCliOptions {
            input,
            output: output.clone(),
            dry_run: false,
            clean_output: false,
            report: None,
        };

        let report = run_fangyuan_bake_cli(&options).unwrap();

        assert!(report.passed());
        assert!(report.entries[0].output_path.is_file());
        let bytes = fs::read(&report.entries[0].output_path).unwrap();
        let artifact = decode_fangyuan_bake_artifact(&bytes).unwrap();
        assert_eq!(
            artifact.header.target_kind,
            FangyuanBakeArtifactKind::Blueprint
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_bake_cli_writes_upgraded_legacy_payload_matching_header_hash() {
        let temp = std::env::temp_dir().join(format!(
            "fangyuan_bake_legacy_write_test_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        let input = temp.join("input");
        let output = temp.join("output");
        fs::create_dir_all(&input).unwrap();
        let legacy_source = r#"
(
    version: "0",
    name: "legacy_write_avatar",
    description: "",
    max_primitives: 8,
    bounds: (width: 4.0, depth: 4.0, height: 4.0),
    primitives: [
        (
            kind: "cube",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [0.2, 0.4, 0.6, 1.0],
        ),
    ],
)
"#;
        fs::write(input.join("legacy_avatar_blueprint.ron"), legacy_source).unwrap();

        let options = FangyuanBakeCliOptions {
            input,
            output,
            dry_run: false,
            clean_output: false,
            report: None,
        };

        let report = run_fangyuan_bake_cli(&options).unwrap();

        assert!(report.passed());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].upgraded_from, Some(0));
        let bytes = fs::read(&report.entries[0].output_path).unwrap();
        let artifact = decode_fangyuan_bake_artifact(&bytes).unwrap();
        let payload =
            decode_fangyuan_bake_payload(FangyuanBakeArtifactKind::Blueprint, &artifact.payload)
                .unwrap();

        assert!(matches!(
            payload,
            FangyuanBakePayload::Blueprint {
                blueprint: FangyuanBlueprint { ref version, .. },
                ..
            } if version == "1"
        ));
        assert_eq!(
            artifact.header.source_hash,
            fangyuan_bake_hash_bytes(legacy_source.as_bytes())
        );
        assert_eq!(
            artifact.header.content_hash,
            fangyuan_bake_hash_bytes(artifact.payload.as_slice())
        );
        artifact
            .header
            .validate_hashes(legacy_source.as_bytes(), artifact.payload.as_slice())
            .unwrap();

        let _ = fs::remove_dir_all(
            report
                .input
                .parent()
                .expect("test input should have a temp parent"),
        );
    }

    #[test]
    fn fangyuan_bake_artifact_deterministic_bake_and_size_report_are_stable() {
        let first = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::PrefabPalette,
            sample_prefab_palette_ron(),
            Some(PathBuf::from("fangyuan/palettes/home_prefabs.ron")),
        )
        .unwrap();
        let second = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::PrefabPalette,
            sample_prefab_palette_ron(),
            Some(PathBuf::from("fangyuan/palettes/home_prefabs.ron")),
        )
        .unwrap();

        assert_eq!(first.payload_bytes, second.payload_bytes);
        assert_eq!(first.content_hash, second.content_hash);
        assert_eq!(first.stats.artifact_size, second.stats.artifact_size);
        assert!(first.stats.artifact_size > first.payload_bytes.len());
        assert_eq!(first.stats.primitive_count, 1);
        assert_eq!(first.stats.prefab_count, 1);
        assert_eq!(first.stats.profile_count, 1);
        assert_eq!(first.stats.budget, 8);
        assert_eq!(first.stats.warning_count, 0);
    }

    #[test]
    fn fangyuan_bake_payload_roundtrips_optional_role_from_prefab_palette() {
        let source = r#"
(
    version: "1",
    name: "role_prefabs",
    description: "",
    max_primitives: 8,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    prefabs: [
        (
            id: "role_block",
            name: "Role Block",
            description: "",
            max_primitives: 2,
            primitives: [
                (
                    kind: "cube",
                    role: "boundary",
                    position: [0.0, 1.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    color: [0.2, 0.4, 0.6, 1.0],
                ),
            ],
        ),
    ],
)
"#;
        let compiled =
            compile_fangyuan_bake_artifact(FangyuanBakeArtifactKind::PrefabPalette, source, None)
                .unwrap();
        let payload = decode_fangyuan_bake_payload(
            FangyuanBakeArtifactKind::PrefabPalette,
            &compiled.payload_bytes,
        )
        .unwrap();

        let FangyuanBakePayload::PrefabPalette { palette, .. } = payload else {
            panic!("expected prefab palette payload");
        };
        assert_eq!(
            palette.prefabs[0].primitives[0].role,
            Some(super::super::FangyuanPrimitiveRole::Boundary)
        );
    }

    #[test]
    fn fangyuan_bake_artifact_dependency_table_resolves_layout_to_palette_prefab_and_profile() {
        let temp = unique_temp_dir("fangyuan_bake_dependency_test");
        let input = temp.join("input").join("fangyuan");
        let output = temp.join("output");
        fs::create_dir_all(input.join("palettes")).unwrap();
        fs::create_dir_all(input.join("layouts")).unwrap();
        fs::create_dir_all(input.join("materials")).unwrap();
        fs::write(
            input.join("palettes").join("home_prefabs.ron"),
            sample_prefab_palette_ron(),
        )
        .unwrap();
        fs::write(
            input.join("layouts").join("home_layout.ron"),
            sample_layout_ron(),
        )
        .unwrap();
        fs::write(
            input.join("materials").join("fx_test_material.ron"),
            sample_material_profile_ron(),
        )
        .unwrap();

        let report = run_fangyuan_bake_cli(&FangyuanBakeCliOptions {
            input: input.clone(),
            output,
            dry_run: true,
            clean_output: false,
            report: None,
        })
        .unwrap();

        assert!(report.passed());
        let layout_entry = report
            .entries
            .iter()
            .find(|entry| entry.target_kind == FangyuanBakeArtifactKind::SceneLayout)
            .unwrap();
        assert!(layout_entry.dependency_table.is_complete());
        assert_eq!(
            layout_entry.dependency_table.palette_paths,
            vec!["fangyuan/palettes/home_prefabs.ron".to_string()]
        );
        assert_eq!(
            layout_entry.dependency_table.prefab_ids,
            vec!["stone_block".to_string()]
        );
        assert_eq!(
            layout_entry.dependency_table.material_profile_ids,
            vec!["fx/test".to_string()]
        );
        let palette_entry = report
            .entries
            .iter()
            .find(|entry| entry.target_kind == FangyuanBakeArtifactKind::PrefabPalette)
            .unwrap();
        assert_eq!(
            palette_entry.dependency_table.material_profile_ids,
            vec!["fx/test".to_string()]
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_bake_artifact_reports_dependency_missing() {
        let temp = unique_temp_dir("fangyuan_bake_missing_dependency_test");
        let input = temp.join("input").join("fangyuan");
        fs::create_dir_all(input.join("layouts")).unwrap();
        fs::write(
            input.join("layouts").join("home_layout.ron"),
            sample_layout_ron(),
        )
        .unwrap();

        let report = run_fangyuan_bake_cli(&FangyuanBakeCliOptions {
            input: input.clone(),
            output: temp.join("output"),
            dry_run: true,
            clean_output: false,
            report: None,
        })
        .unwrap();

        assert!(!report.passed());
        assert_eq!(fangyuan_bake_cli_exit_code(&report), 1);
        let entry = &report.entries[0];
        assert_eq!(entry.target_kind, FangyuanBakeArtifactKind::SceneLayout);
        assert_eq!(entry.missing_dependency_count, 2);
        assert!(entry.error_count >= 2);
        assert!(
            entry
                .dependency_table
                .missing
                .iter()
                .any(|dependency| dependency.kind == FangyuanBakeDependencyKind::Palette)
        );
        assert!(
            entry
                .dependency_table
                .missing
                .iter()
                .any(|dependency| dependency.kind == FangyuanBakeDependencyKind::Prefab)
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_bake_artifact_hash_changes_when_content_changes() {
        let first = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::Blueprint,
            &sample_blueprint_ron("hash_avatar", [0.2, 0.4, 0.6, 1.0]),
            None,
        )
        .unwrap();
        let second = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::Blueprint,
            &sample_blueprint_ron("hash_avatar", [0.9, 0.4, 0.6, 1.0]),
            None,
        )
        .unwrap();

        assert_ne!(first.source_hash, second.source_hash);
        assert_ne!(first.content_hash, second.content_hash);
        assert_ne!(first.payload_bytes, second.payload_bytes);
    }

    #[test]
    fn fangyuan_runtime_load_prefers_bin_and_matches_ron_payload() {
        let temp = unique_temp_dir("fangyuan_runtime_load_consistency_test");
        fs::create_dir_all(&temp).unwrap();
        let ron_path = temp.join("avatar.ron");
        let bin_path = temp.join("avatar.fyb");
        let source = sample_blueprint_ron("runtime_avatar", [0.2, 0.4, 0.6, 1.0]);
        fs::write(&ron_path, &source).unwrap();
        let compiled = write_sample_artifact(
            FangyuanBakeArtifactKind::Blueprint,
            &source,
            &bin_path,
            Some(ron_path.clone()),
        );

        let report = load_fangyuan_runtime_artifact(
            &FangyuanRuntimeArtifactManifestEntry {
                id: "runtime_avatar".to_string(),
                kind: FangyuanBakeArtifactKind::Blueprint,
                bin: Some(bin_path),
                ron: Some(ron_path),
                expected_content_hash: Some(compiled.content_hash),
                expected_source_hash: Some(compiled.source_hash),
                required_dependencies: Vec::new(),
            },
            FangyuanRuntimeArtifactLoaderOptions::debug_with_ron_fallback(),
            std::iter::empty::<&str>(),
        );

        assert_eq!(report.status, FangyuanRuntimeArtifactLoadStatus::Loaded);
        assert_eq!(report.source, FangyuanRuntimeArtifactLoadSource::Bin);
        let bin_payload = report.payload.unwrap();
        let (_, ron_source) =
            load_fangyuan_runtime_artifact_ron_source(FangyuanBakeArtifactKind::Blueprint, &source)
                .unwrap();
        let ron_payload =
            compile_current_fangyuan_bake_payload(FangyuanBakeArtifactKind::Blueprint, &ron_source)
                .unwrap();
        assert_eq!(bin_payload, ron_payload);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_runtime_load_falls_back_to_ron_when_debug_allows() {
        let temp = unique_temp_dir("fangyuan_runtime_load_fallback_test");
        fs::create_dir_all(&temp).unwrap();
        let ron_path = temp.join("avatar.ron");
        fs::write(
            &ron_path,
            sample_blueprint_ron("fallback_avatar", [0.2, 0.4, 0.6, 1.0]),
        )
        .unwrap();

        let report = load_fangyuan_runtime_artifact(
            &FangyuanRuntimeArtifactManifestEntry {
                id: "fallback_avatar".to_string(),
                kind: FangyuanBakeArtifactKind::Blueprint,
                bin: Some(temp.join("missing.fyb")),
                ron: Some(ron_path),
                expected_content_hash: None,
                expected_source_hash: None,
                required_dependencies: Vec::new(),
            },
            FangyuanRuntimeArtifactLoaderOptions::debug_with_ron_fallback(),
            std::iter::empty::<&str>(),
        );

        assert_eq!(
            report.status,
            FangyuanRuntimeArtifactLoadStatus::FallbackLoaded
        );
        assert_eq!(report.source, FangyuanRuntimeArtifactLoadSource::Ron);
        assert_eq!(report.fallback, FangyuanRuntimeArtifactFallback::Used);
        assert!(matches!(
            report.error,
            Some(FangyuanRuntimeArtifactLoadError::LoadFailed { .. })
        ));
        assert!(report.payload.is_some());

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_runtime_load_reports_version_hash_kind_and_dependency_errors() {
        let temp = unique_temp_dir("fangyuan_runtime_load_error_test");
        let bin_path = temp.join("avatar.fyb");
        let source = sample_blueprint_ron("error_avatar", [0.2, 0.4, 0.6, 1.0]);
        let compiled = write_sample_artifact(
            FangyuanBakeArtifactKind::Blueprint,
            &source,
            &bin_path,
            None,
        );

        let missing_dep = load_fangyuan_runtime_artifact(
            &FangyuanRuntimeArtifactManifestEntry {
                id: "error_avatar".to_string(),
                kind: FangyuanBakeArtifactKind::Blueprint,
                bin: Some(bin_path.clone()),
                ron: None,
                expected_content_hash: Some(compiled.content_hash),
                expected_source_hash: Some(compiled.source_hash),
                required_dependencies: vec!["palette:missing".to_string()],
            },
            FangyuanRuntimeArtifactLoaderOptions::release(),
            std::iter::empty::<&str>(),
        );
        assert!(matches!(
            missing_dep.error,
            Some(FangyuanRuntimeArtifactLoadError::DependencyMissing { .. })
        ));

        let hash_mismatch = load_fangyuan_runtime_artifact(
            &FangyuanRuntimeArtifactManifestEntry {
                id: "error_avatar".to_string(),
                kind: FangyuanBakeArtifactKind::Blueprint,
                bin: Some(bin_path.clone()),
                ron: None,
                expected_content_hash: Some(compiled.content_hash.wrapping_add(1)),
                expected_source_hash: None,
                required_dependencies: Vec::new(),
            },
            FangyuanRuntimeArtifactLoaderOptions::release(),
            std::iter::empty::<&str>(),
        );
        assert!(matches!(
            hash_mismatch.error,
            Some(FangyuanRuntimeArtifactLoadError::HashMismatch {
                hash_kind: FangyuanRuntimeArtifactHashKind::Content,
                ..
            })
        ));

        let mut bytes = fs::read(&bin_path).unwrap();
        bytes[8] = (FANGYUAN_BAKE_SCHEMA_VERSION + 1) as u8;
        bytes[9] = 0;
        let version_path = temp.join("avatar_bad_version.fyb");
        fs::write(&version_path, bytes).unwrap();
        let version_mismatch = load_fangyuan_runtime_artifact(
            &FangyuanRuntimeArtifactManifestEntry {
                id: "error_avatar".to_string(),
                kind: FangyuanBakeArtifactKind::Blueprint,
                bin: Some(version_path),
                ron: None,
                expected_content_hash: None,
                expected_source_hash: None,
                required_dependencies: Vec::new(),
            },
            FangyuanRuntimeArtifactLoaderOptions::release(),
            std::iter::empty::<&str>(),
        );
        assert!(matches!(
            version_mismatch.error,
            Some(FangyuanRuntimeArtifactLoadError::VersionMismatch { .. })
        ));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn fangyuan_runtime_load_covers_chunk_material_profile_and_skill_payloads() {
        let chunk = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::Chunk,
            sample_chunk_ron(),
            None,
        )
        .unwrap();
        let material = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::MaterialProfile,
            sample_material_profile_ron(),
            None,
        )
        .unwrap();
        let skill = compile_fangyuan_bake_artifact(
            FangyuanBakeArtifactKind::SkillRecipe,
            sample_skill_recipe_ron(),
            None,
        )
        .unwrap();

        assert!(matches!(
            chunk.payload,
            FangyuanBakePayload::ChunkSource { .. }
        ));
        assert!(matches!(
            material.payload,
            FangyuanBakePayload::MaterialProfile { .. }
        ));
        assert!(matches!(
            skill.payload,
            FangyuanBakePayload::VfxRecipe { .. }
        ));
    }

    #[test]
    fn fangyuan_home_runtime_loads_home_layout_artifact_from_bin_or_ron() {
        let temp = unique_temp_dir("fangyuan_home_runtime_load_test");
        fs::create_dir_all(&temp).unwrap();
        let ron_path = temp.join("home_layout.ron");
        let bin_path = temp.join("home_layout.fyb");
        fs::write(&ron_path, sample_layout_ron()).unwrap();
        let compiled = write_sample_artifact(
            FangyuanBakeArtifactKind::SceneLayout,
            sample_layout_ron(),
            &bin_path,
            Some(ron_path.clone()),
        );
        let entry = FangyuanRuntimeArtifactManifestEntry {
            id: "home_layout".to_string(),
            kind: FangyuanBakeArtifactKind::SceneLayout,
            bin: Some(bin_path),
            ron: Some(ron_path),
            expected_content_hash: Some(compiled.content_hash),
            expected_source_hash: None,
            required_dependencies: Vec::new(),
        };

        let report = load_fangyuan_runtime_artifact(
            &entry,
            FangyuanRuntimeArtifactLoaderOptions::debug_with_ron_fallback(),
            std::iter::empty::<&str>(),
        );

        assert_eq!(report.status, FangyuanRuntimeArtifactLoadStatus::Loaded);
        assert!(matches!(
            report.payload,
            Some(FangyuanBakePayload::SceneLayout { .. })
        ));

        let _ = fs::remove_dir_all(&temp);
    }

    fn write_sample_artifact(
        kind: FangyuanBakeArtifactKind,
        source: &str,
        output_path: &Path,
        source_path: Option<PathBuf>,
    ) -> FangyuanBakeCompiledArtifact {
        let compiled = compile_fangyuan_bake_artifact(kind, source, source_path).unwrap();
        let header = FangyuanBakeArtifactHeader {
            schema_version: FANGYUAN_BAKE_SCHEMA_VERSION,
            source_hash: compiled.source_hash,
            content_hash: compiled.content_hash,
            created_by: "fangyuan_bake_test".to_string(),
            target_kind: kind,
        };
        let bytes = encode_fangyuan_bake_artifact(&header, &compiled.payload_bytes).unwrap();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(output_path, bytes).unwrap();
        compiled
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let temp = std::env::temp_dir().join(format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&temp);
        temp
    }

    fn sample_blueprint_ron(name: &str, color: [f32; 4]) -> String {
        format!(
            r#"
(
    version: "1",
    name: "{name}",
    description: "",
    max_primitives: 8,
    bounds: (width: 4.0, depth: 4.0, height: 4.0),
    primitives: [
        (
            kind: "cube",
            position: [0.0, 1.0, 0.0],
            size: [1.0, 1.0, 1.0],
            color: [{}, {}, {}, {}],
        ),
    ],
)
"#,
            color[0], color[1], color[2], color[3]
        )
    }

    fn sample_prefab_palette_ron() -> &'static str {
        r#"
(
    version: "1",
    name: "home_prefabs",
    description: "",
    max_primitives: 8,
    bounds: (width: 8.0, depth: 8.0, height: 8.0),
    prefabs: [
        (
            id: "stone_block",
            name: "Stone Block",
            description: "",
            tags: ["stone"],
            max_primitives: Some(2),
            primitives: [
                (
                    kind: "cube",
                    position: [0.0, 1.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                    color: [0.2, 0.4, 0.6, 1.0],
                    material_profile_id: Some("fx/test"),
                ),
            ],
        ),
    ],
)
"#
    }

    fn sample_layout_ron() -> &'static str {
        r#"
(
    version: "1",
    name: "home_layout",
    description: "",
    bounds: (width: 10.0, depth: 10.0, height: 8.0),
    palette: "fangyuan/palettes/home_prefabs.ron",
    max_primitives: 8,
    instances: [
        (
            id: "stone_a",
            prefab: "stone_block",
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        ),
    ],
)
"#
    }

    fn sample_chunk_ron() -> &'static str {
        r#"
(
    version: "1",
    id: "home_chunk_0",
    name: "Home Chunk 0",
    description: "",
    bounds: (min: [-8.0, 0.0, -8.0], max: [8.0, 6.0, 8.0]),
    region: (region_id: "home.default", layer: "ground", tags: []),
    prefab_instances: [
        (
            id: "stone_a",
            prefab: "stone_block",
            transform: (position: [0.0, 0.0, 0.0], scale: [1.0, 1.0, 1.0]),
            budget_cost: 5,
        ),
    ],
    budget: (
        prefab_instance_count: 1,
        tiandao_ref_count: 0,
        static_decoration_count: 0,
        total_ref_count: 1,
        prefab_cost: 5,
        tiandao_cost: 0,
        static_decoration_cost: 0,
        total_cost: 5,
    ),
)
"#
    }

    fn sample_material_profile_ron() -> &'static str {
        r#"
(
    stable_id: "fx/test",
    version: "1",
    debug_label: "FX Test",
)
"#
    }

    fn sample_skill_recipe_ron() -> &'static str {
        r#"
(
    id: "skill.recipe.test",
    version: 1,
    duration_ticks: 10,
    emitters: [
        (
            id: "impact",
            operators: [
                (type: "spawn"),
            ],
        ),
    ],
)
"#
    }
}
