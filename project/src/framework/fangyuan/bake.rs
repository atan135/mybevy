use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::{
    FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity, FangyuanAuditSourceKind,
    FangyuanBlueprint, FangyuanChunkManifest, FangyuanChunkSource, FangyuanMaterialProfile,
    FangyuanPrefabPalette, FangyuanSceneLayout, FangyuanSkillTemplate,
    FangyuanSkillVisualBlueprint, FangyuanVfxRecipe,
};

pub const FANGYUAN_BAKE_ARTIFACT_MAGIC: [u8; 8] = *b"FYBAKE\0\x01";
pub const FANGYUAN_BAKE_SCHEMA_VERSION: u16 = 1;
pub const FANGYUAN_BAKE_HASH_BYTES: usize = 8;
pub const FANGYUAN_BAKE_FORMAT_NAME: &str = "fangyuan-custom-header-v1";
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
            reason: "阶段 5 只需要稳定 artifact header、hash 校验和 dry-run 校验入口；payload 暂存规范化源字节，避免提前承诺 runtime loader wire format。",
            limitations: &[
                "payload 仍是源 RON 字节，不是阶段 6 的 runtime 二进制加载格式",
                "FNV-1a hash 用于本地内容一致性检查，不用于安全校验",
            ],
            dependency_impact: "不新增依赖；复用 std、serde、ron 和现有 validator/audit。",
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
    Validate {
        target_kind: FangyuanBakeArtifactKind,
        source: String,
    },
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
            Self::UnsupportedMaterialProfileRon => formatter.write_str(
                "material profile RON dry-run is reserved until material profile serde schema is authored",
            ),
        }
    }
}

impl Error for FangyuanBakeValidationError {}

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
            report,
        })
    }
}

pub fn fangyuan_bake_cli_usage() -> String {
    "usage: fangyuan_bake --input <dir> --output <dir> [--dry-run] [--report <path>]".to_string()
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanBakeRunEntry {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub target_kind: FangyuanBakeArtifactKind,
    pub source_hash: u64,
    pub content_hash: u64,
    pub upgraded_from: Option<u16>,
    pub passed: bool,
    pub error_count: usize,
    pub warning_count: usize,
}

pub fn run_fangyuan_bake_cli(
    options: &FangyuanBakeCliOptions,
) -> Result<FangyuanBakeRunReport, FangyuanBakeCliError> {
    let mut entries = Vec::new();
    let files = collect_ron_files(&options.input)?;
    if !options.dry_run {
        fs::create_dir_all(&options.output).map_err(|source| FangyuanBakeCliError::Io {
            path: options.output.clone(),
            source,
        })?;
    }

    for source_path in files {
        let source =
            fs::read_to_string(&source_path).map_err(|source| FangyuanBakeCliError::Io {
                path: source_path.clone(),
                source,
            })?;
        let target_kind =
            infer_fangyuan_bake_artifact_kind(&source_path, &source).ok_or_else(|| {
                FangyuanBakeCliError::InvalidArgs(format!(
                    "cannot infer Fangyuan bake artifact kind for {}",
                    source_path.display()
                ))
            })?;
        let validation =
            validate_fangyuan_bake_source(target_kind, &source, Some(source_path.clone()))?;
        let output_path =
            output_path_for(&options.input, &options.output, &source_path, target_kind);

        if !options.dry_run {
            let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
            fs::create_dir_all(parent).map_err(|source| FangyuanBakeCliError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
            let payload = validation.content.as_bytes();
            let header = FangyuanBakeArtifactHeader {
                schema_version: FANGYUAN_BAKE_SCHEMA_VERSION,
                source_hash: validation.source_hash,
                content_hash: validation.content_hash,
                created_by: "fangyuan_bake".to_string(),
                target_kind,
            };
            let bytes = encode_fangyuan_bake_artifact(&header, payload)?;
            fs::write(&output_path, bytes).map_err(|source| FangyuanBakeCliError::Io {
                path: output_path.clone(),
                source,
            })?;
        }

        entries.push(FangyuanBakeRunEntry {
            source_path,
            output_path,
            target_kind,
            source_hash: validation.source_hash,
            content_hash: validation.content_hash,
            upgraded_from: validation.upgraded_from,
            passed: validation.passed(),
            error_count: validation.audit.summary.error_count,
            warning_count: validation.audit.summary.warning_count,
        });
    }

    let report = FangyuanBakeRunReport {
        dry_run: options.dry_run,
        input: options.input.clone(),
        output: options.output.clone(),
        entries,
    };

    if let Some(report_path) = &options.report {
        write_fangyuan_bake_report(report_path, &report)?;
    }

    Ok(report)
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
        "dry_run={}; input={}; output={}; entries={}; failed={}",
        report.dry_run,
        report.input.display(),
        report.output.display(),
        report.entries.len(),
        report.failed_count()
    )];
    for entry in &report.entries {
        lines.push(format!(
            "{} kind={} output={} passed={} errors={} warnings={} source_hash={:016x} content_hash={:016x} upgraded_from={}",
            entry.source_path.display(),
            entry.target_kind,
            entry.output_path.display(),
            entry.passed,
            entry.error_count,
            entry.warning_count,
            entry.source_hash,
            entry.content_hash,
            entry
                .upgraded_from
                .map(|version| version.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
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
        assert_eq!(decisions[0].name, FANGYUAN_BAKE_FORMAT_NAME);
        assert!(decisions[0].dependency_impact.contains("不新增依赖"));
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
            "--report",
            "reports/bake.txt",
        ])
        .unwrap();

        assert_eq!(options.input, PathBuf::from("assets/fangyuan"));
        assert_eq!(options.output, PathBuf::from("artifacts/fangyuan"));
        assert!(options.dry_run);
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
            report: Some(report_path.clone()),
        };

        let report = run_fangyuan_bake_cli(&options).unwrap();

        assert!(report.passed());
        assert_eq!(report.entries.len(), 1);
        assert!(report_path.is_file());
        assert!(
            !output.exists(),
            "dry-run must not create output artifacts or output directory"
        );

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
            report: None,
        };

        let report = run_fangyuan_bake_cli(&options).unwrap();

        assert!(report.passed());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].upgraded_from, Some(0));
        let bytes = fs::read(&report.entries[0].output_path).unwrap();
        let artifact = decode_fangyuan_bake_artifact(&bytes).unwrap();
        let payload = String::from_utf8(artifact.payload.clone()).unwrap();

        assert!(payload.contains(r#"version: "1""#));
        assert!(!payload.contains(r#"version: "0""#));
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
}
