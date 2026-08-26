//! Local-only Fangyuan authoring command boundary.
//!
//! This module owns the strict v1 task parser and run-directory boundary used
//! by the development-only `fangyuan_authoring` binary. It deliberately does
//! not invoke an AI provider, touch game runtime state, or implement the
//! later canonical/audit/promotion semantics.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use getrandom::fill as fill_random_bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    FangyuanCatalogAssetPaths, FangyuanCatalogProjection, FangyuanCatalogRenderModel,
    FangyuanPrimitiveKind, FangyuanPrimitiveRole, FangyuanSurfacePresetCacheIdentity,
    FangyuanSurfacePresetCatalogMetadata, FangyuanSurfacePresetRegistry,
};

pub const FANGYUAN_AUTHORING_TASK_SCHEMA_VERSION: &str = "fangyuan-generation-task/v1";
pub const FANGYUAN_AUTHORING_RUN_SCHEMA_VERSION: &str = "fangyuan-generation-run/v1";
pub const FANGYUAN_AUTHORING_PREPARED_TASK_SCHEMA_VERSION: &str =
    "fangyuan-generation-prepared-task/v1";
pub const FANGYUAN_AUTHORING_LOG_SUMMARY_SCHEMA_VERSION: &str =
    "fangyuan-generation-log-summary/v1";
pub const FANGYUAN_AUTHORING_CATALOG_CONTEXT_SCHEMA_VERSION: &str =
    "fangyuan-generation-catalog-context/v1";
pub const FANGYUAN_AUTHORING_RUN_DIRECTORY: &str = "summary/fangyuan-generation";
pub const FANGYUAN_AUTHORING_MAX_TASK_BYTES: usize = 64 * 1024;

const RUN_MANIFEST_FILE: &str = "run-manifest.json";
const TASK_SOURCE_FILE: &str = "input/task.source.json";
const TASK_PREPARED_FILE: &str = "input/task.prepared.json";
const DRAFT_BLUEPRINT_FILE: &str = "draft/blueprint.ron";
const CANONICAL_BLUEPRINT_FILE: &str = "canonical/blueprint.ron";
const AUDIT_REPORT_FILE: &str = "audit/report.json";
const PROMOTION_DECISION_FILE: &str = "promotion/decision.json";
const LOG_SUMMARY_FILE: &str = "logs/summary.json";
const CATALOG_CONTEXT_FILE: &str = "input/catalog-context.json";

const V0_FORBIDDEN_FEATURES: [FangyuanAuthoringForbiddenFeature; 15] = [
    FangyuanAuthoringForbiddenFeature::AbsolutePath,
    FangyuanAuthoringForbiddenFeature::Url,
    FangyuanAuthoringForbiddenFeature::Credential,
    FangyuanAuthoringForbiddenFeature::NetworkCall,
    FangyuanAuthoringForbiddenFeature::Script,
    FangyuanAuthoringForbiddenFeature::Shader,
    FangyuanAuthoringForbiddenFeature::AtlasUv,
    FangyuanAuthoringForbiddenFeature::AtlasTileIndex,
    FangyuanAuthoringForbiddenFeature::NoiseTileId,
    FangyuanAuthoringForbiddenFeature::NoiseStrength,
    FangyuanAuthoringForbiddenFeature::ArbitraryPbrOverride,
    FangyuanAuthoringForbiddenFeature::MaterialBindingTable,
    FangyuanAuthoringForbiddenFeature::Rotation,
    FangyuanAuthoringForbiddenFeature::FreeMesh,
    FangyuanAuthoringForbiddenFeature::GameplayOverride,
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthoringTask {
    pub schema_version: String,
    pub task_id: String,
    pub stable_id: String,
    pub object_purpose: FangyuanAuthoringObjectPurpose,
    pub prompt: String,
    pub budget: FangyuanAuthoringBudget,
    pub allowed: FangyuanAuthoringAllowedValues,
    pub material_budget: FangyuanAuthoringMaterialBudget,
    pub forbidden_features: Vec<FangyuanAuthoringForbiddenFeature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_forbidden_features: Vec<FangyuanAuthoringAdditionalForbiddenFeature>,
}

impl FangyuanAuthoringTask {
    pub fn parse_json(bytes: &[u8]) -> Result<Self, FangyuanAuthoringTaskError> {
        if bytes.len() > FANGYUAN_AUTHORING_MAX_TASK_BYTES {
            return Err(FangyuanAuthoringTaskError::TooLarge {
                actual: bytes.len(),
                maximum: FANGYUAN_AUTHORING_MAX_TASK_BYTES,
            });
        }
        let task = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| FangyuanAuthoringTaskError::Json(error.to_string()))?;
        task.validate()?;
        Ok(task)
    }

    pub fn validate(&self) -> Result<(), FangyuanAuthoringTaskError> {
        if self.schema_version != FANGYUAN_AUTHORING_TASK_SCHEMA_VERSION {
            return Err(FangyuanAuthoringTaskError::UnsupportedSchemaVersion {
                found: self.schema_version.clone(),
                expected: FANGYUAN_AUTHORING_TASK_SCHEMA_VERSION,
            });
        }
        validate_identifier(&self.task_id, IdentifierKind::TaskId)?;
        validate_identifier(&self.stable_id, IdentifierKind::StableId)?;
        let prompt_len = self.prompt.chars().count();
        if prompt_len == 0 || self.prompt.trim().is_empty() || prompt_len > 4096 {
            return Err(FangyuanAuthoringTaskError::InvalidPromptLength { prompt_len });
        }
        self.budget.validate()?;
        self.allowed.validate()?;
        self.material_budget.validate()?;
        if self.forbidden_features != V0_FORBIDDEN_FEATURES {
            return Err(FangyuanAuthoringTaskError::InvalidForbiddenFeatures);
        }
        if !has_unique_values(&self.additional_forbidden_features) {
            return Err(FangyuanAuthoringTaskError::DuplicateAdditionalForbiddenFeature);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanAuthoringObjectPurpose {
    AvatarAppearance,
    HomeDecoration,
    EquipmentAppearance,
    SkillVisual,
    NpcAppearance,
    SceneDecoration,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthoringBudget {
    pub max_primitives: usize,
    pub bounds: FangyuanAuthoringBounds,
    pub max_primitive_volume: f32,
    pub max_total_volume: f32,
}

impl FangyuanAuthoringBudget {
    fn validate(&self) -> Result<(), FangyuanAuthoringTaskError> {
        if !(1..=1000).contains(&self.max_primitives) {
            return Err(FangyuanAuthoringTaskError::InvalidBudget(
                "max_primitives must be in 1..=1000",
            ));
        }
        self.bounds.validate()?;
        validate_budget_number("max_primitive_volume", self.max_primitive_volume, 4096.0)?;
        validate_budget_number("max_total_volume", self.max_total_volume, 32768.0)?;
        if self.max_primitive_volume > self.max_total_volume {
            return Err(FangyuanAuthoringTaskError::InvalidBudget(
                "max_primitive_volume must not exceed max_total_volume",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthoringBounds {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
}

impl FangyuanAuthoringBounds {
    fn validate(&self) -> Result<(), FangyuanAuthoringTaskError> {
        for (name, value) in [
            ("bounds.width", self.width),
            ("bounds.depth", self.depth),
            ("bounds.height", self.height),
        ] {
            validate_budget_number(name, value, 64.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthoringAllowedValues {
    pub primitive_kinds: Vec<FangyuanPrimitiveKind>,
    pub roles: Vec<FangyuanPrimitiveRole>,
}

impl FangyuanAuthoringAllowedValues {
    fn validate(&self) -> Result<(), FangyuanAuthoringTaskError> {
        if self.primitive_kinds.as_slice()
            != &[FangyuanPrimitiveKind::Cube, FangyuanPrimitiveKind::Sphere]
        {
            return Err(FangyuanAuthoringTaskError::InvalidAllowedPrimitiveKinds);
        }
        if self.roles.is_empty() || !has_unique_roles(&self.roles) {
            return Err(FangyuanAuthoringTaskError::InvalidAllowedRoles);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanAuthoringMaterialBudget {
    pub max_surface_presets: usize,
    pub max_material_profiles: usize,
    pub allow_catalog_suggested_noise: bool,
    pub max_catalog_noise_suggestions_per_preset: usize,
    pub allow_arbitrary_overrides: bool,
}

impl FangyuanAuthoringMaterialBudget {
    fn validate(&self) -> Result<(), FangyuanAuthoringTaskError> {
        if !(1..=4).contains(&self.max_surface_presets) {
            return Err(FangyuanAuthoringTaskError::InvalidMaterialBudget(
                "max_surface_presets must be in 1..=4",
            ));
        }
        if !(1..=8).contains(&self.max_material_profiles) {
            return Err(FangyuanAuthoringTaskError::InvalidMaterialBudget(
                "max_material_profiles must be in 1..=8",
            ));
        }
        if self.max_catalog_noise_suggestions_per_preset != 1 {
            return Err(FangyuanAuthoringTaskError::InvalidMaterialBudget(
                "max_catalog_noise_suggestions_per_preset must equal 1",
            ));
        }
        if self.allow_arbitrary_overrides {
            return Err(FangyuanAuthoringTaskError::InvalidMaterialBudget(
                "allow_arbitrary_overrides must be false",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanAuthoringForbiddenFeature {
    AbsolutePath,
    Url,
    Credential,
    NetworkCall,
    Script,
    Shader,
    AtlasUv,
    AtlasTileIndex,
    NoiseTileId,
    NoiseStrength,
    ArbitraryPbrOverride,
    MaterialBindingTable,
    Rotation,
    FreeMesh,
    GameplayOverride,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanAuthoringAdditionalForbiddenFeature {
    Transparent,
    Emissive,
    SpecialSurface,
    WaterSurface,
    SkySurface,
    LavaSurface,
    TriplanarRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAuthoringTaskError {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Json(String),
    UnsupportedSchemaVersion {
        found: String,
        expected: &'static str,
    },
    InvalidIdentifier {
        kind: IdentifierKind,
    },
    InvalidPromptLength {
        prompt_len: usize,
    },
    InvalidBudget(&'static str),
    InvalidMaterialBudget(&'static str),
    InvalidAllowedPrimitiveKinds,
    InvalidAllowedRoles,
    InvalidForbiddenFeatures,
    DuplicateAdditionalForbiddenFeature,
}

impl fmt::Display for FangyuanAuthoringTaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "task has {actual} bytes, exceeding the {maximum}-byte limit"
                )
            }
            Self::Json(message) => write!(
                formatter,
                "task JSON does not match the strict contract: {message}"
            ),
            Self::UnsupportedSchemaVersion { found, expected } => {
                write!(
                    formatter,
                    "unsupported task schema version `{found}`; expected `{expected}`"
                )
            }
            Self::InvalidIdentifier { kind } => write!(formatter, "invalid {}", kind.as_str()),
            Self::InvalidPromptLength { prompt_len } => {
                write!(formatter, "prompt length {prompt_len} must be in 1..=4096")
            }
            Self::InvalidBudget(message) | Self::InvalidMaterialBudget(message) => {
                formatter.write_str(message)
            }
            Self::InvalidAllowedPrimitiveKinds => {
                formatter.write_str("allowed.primitive_kinds must equal [cube, sphere]")
            }
            Self::InvalidAllowedRoles => {
                formatter.write_str("allowed.roles must be non-empty and unique")
            }
            Self::InvalidForbiddenFeatures => {
                formatter.write_str("forbidden_features must equal the fixed v0 forbidden list")
            }
            Self::DuplicateAdditionalForbiddenFeature => {
                formatter.write_str("additional_forbidden_features must be unique")
            }
        }
    }
}

impl Error for FangyuanAuthoringTaskError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierKind {
    TaskId,
    StableId,
}

impl IdentifierKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TaskId => "task_id",
            Self::StableId => "stable_id",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuthoringCliOptions {
    pub command: FangyuanAuthoringCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAuthoringCommand {
    PrepareTask { task: PathBuf },
    ValidateDraft { run_id: String },
    Promote { run_id: String },
    InspectRun { run_id: String },
}

impl FangyuanAuthoringCliOptions {
    pub fn parse_from<I, S>(args: I) -> Result<Self, FangyuanAuthoringCliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        if args
            .iter()
            .any(|argument| argument == "--help" || argument == "-h")
        {
            return Err(FangyuanAuthoringCliError::Help(
                fangyuan_authoring_cli_usage(),
            ));
        }
        let Some((command, rest)) = args.split_first() else {
            return Err(FangyuanAuthoringCliError::InvalidArgs(
                "a subcommand is required".to_string(),
            ));
        };
        let command = match command.as_str() {
            "prepare-task" => FangyuanAuthoringCommand::PrepareTask {
                task: PathBuf::from(parse_exact_option(rest, "--task")?),
            },
            "validate-draft" => FangyuanAuthoringCommand::ValidateDraft {
                run_id: parse_exact_option(rest, "--run")?,
            },
            "promote" => FangyuanAuthoringCommand::Promote {
                run_id: parse_exact_option(rest, "--run")?,
            },
            "inspect-run" => FangyuanAuthoringCommand::InspectRun {
                run_id: parse_exact_option(rest, "--run")?,
            },
            other => {
                return Err(FangyuanAuthoringCliError::InvalidArgs(format!(
                    "unknown subcommand `{other}`"
                )));
            }
        };
        Ok(Self { command })
    }
}

pub fn fangyuan_authoring_cli_usage() -> String {
    [
        "usage: fangyuan_authoring <subcommand> [options]",
        "",
        "subcommands:",
        "  prepare-task --task <repository-relative-task.json>",
        "  validate-draft --run <run-id>",
        "  promote --run <run-id>",
        "  inspect-run --run <run-id>",
    ]
    .join("\n")
}

#[derive(Debug)]
pub enum FangyuanAuthoringCliError {
    Help(String),
    InvalidArgs(String),
    Task(FangyuanAuthoringTaskError),
    Boundary(&'static str),
    InvalidRunId,
    Io {
        operation: &'static str,
        source: io::Error,
    },
    Serialization(String),
    Random(String),
    OperationFailed {
        run_id: String,
        code: &'static str,
    },
}

impl fmt::Display for FangyuanAuthoringCliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help(usage) => formatter.write_str(usage),
            Self::InvalidArgs(message) => formatter.write_str(message),
            Self::Task(error) => write!(formatter, "invalid task: {error}"),
            Self::Boundary(reason) => formatter.write_str(reason),
            Self::InvalidRunId => formatter.write_str("invalid run id"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::Serialization(message) => {
                write!(
                    formatter,
                    "failed to serialize local authoring record: {message}"
                )
            }
            Self::Random(message) => {
                write!(formatter, "failed to generate a local run nonce: {message}")
            }
            Self::OperationFailed { run_id, code } => {
                write!(formatter, "operation failed for run `{run_id}`: {code}")
            }
        }
    }
}

impl Error for FangyuanAuthoringCliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Task(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Help(_)
            | Self::InvalidArgs(_)
            | Self::Boundary(_)
            | Self::InvalidRunId
            | Self::Serialization(_)
            | Self::Random(_)
            | Self::OperationFailed { .. } => None,
        }
    }
}

pub fn fangyuan_authoring_cli_error_exit_code(error: &FangyuanAuthoringCliError) -> i32 {
    match error {
        FangyuanAuthoringCliError::Help(_) => 0,
        FangyuanAuthoringCliError::InvalidArgs(_) => 2,
        FangyuanAuthoringCliError::Task(_)
        | FangyuanAuthoringCliError::Boundary(_)
        | FangyuanAuthoringCliError::InvalidRunId
        | FangyuanAuthoringCliError::Io { .. }
        | FangyuanAuthoringCliError::Serialization(_)
        | FangyuanAuthoringCliError::Random(_)
        | FangyuanAuthoringCliError::OperationFailed { .. } => 1,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuthoringCliReport {
    pub command: FangyuanAuthoringCommandName,
    pub run_id: String,
    pub status: FangyuanAuthoringCommandStatus,
    pub report_relative_path: String,
    pub artifact_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanAuthoringCommandName {
    PrepareTask,
    ValidateDraft,
    Promote,
    InspectRun,
}

impl FangyuanAuthoringCommandName {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PrepareTask => "prepare-task",
            Self::ValidateDraft => "validate-draft",
            Self::Promote => "promote",
            Self::InspectRun => "inspect-run",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanAuthoringCommandStatus {
    Prepared,
    Inspected,
}

impl FangyuanAuthoringCommandStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Inspected => "inspected",
        }
    }
}

pub fn format_fangyuan_authoring_cli_report(report: &FangyuanAuthoringCliReport) -> String {
    format!(
        "fangyuan_authoring: command={} status={} run={} artifacts={} report={}",
        report.command.as_str(),
        report.status.as_str(),
        report.run_id,
        report.artifact_count,
        report.report_relative_path
    )
}

pub fn run_fangyuan_authoring_cli(
    options: &FangyuanAuthoringCliOptions,
    repository_root: &Path,
) -> Result<FangyuanAuthoringCliReport, FangyuanAuthoringCliError> {
    let repository_root = canonical_repository_root(repository_root)?;
    match &options.command {
        FangyuanAuthoringCommand::PrepareTask { task } => prepare_task(&repository_root, task),
        FangyuanAuthoringCommand::ValidateDraft { run_id } => {
            validate_draft_placeholder(&repository_root, run_id)
        }
        FangyuanAuthoringCommand::Promote { run_id } => {
            promote_placeholder(&repository_root, run_id)
        }
        FangyuanAuthoringCommand::InspectRun { run_id } => inspect_run(&repository_root, run_id),
    }
}

pub fn run_fangyuan_authoring_cli_in_repository(
    options: &FangyuanAuthoringCliOptions,
) -> Result<FangyuanAuthoringCliReport, FangyuanAuthoringCliError> {
    let manifest_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root =
        manifest_directory
            .parent()
            .ok_or(FangyuanAuthoringCliError::Boundary(
                "project manifest directory has no repository parent",
            ))?;
    run_fangyuan_authoring_cli(options, repository_root)
}

fn prepare_task(
    repository_root: &Path,
    task_argument: &Path,
) -> Result<FangyuanAuthoringCliReport, FangyuanAuthoringCliError> {
    let task_path = resolve_repository_input_file(repository_root, task_argument)?;
    let task_bytes = fs::read(&task_path).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to read task input",
        source,
    })?;
    if task_bytes.len() > FANGYUAN_AUTHORING_MAX_TASK_BYTES {
        return Err(FangyuanAuthoringCliError::Task(
            FangyuanAuthoringTaskError::TooLarge {
                actual: task_bytes.len(),
                maximum: FANGYUAN_AUTHORING_MAX_TASK_BYTES,
            },
        ));
    }

    let (run_id, run_directory, created_at_utc) = create_run_directory(repository_root)?;
    write_new_run_file(&run_directory, TASK_SOURCE_FILE, &task_bytes)?;

    let task = match FangyuanAuthoringTask::parse_json(&task_bytes) {
        Ok(task) => task,
        Err(error) => {
            let mut manifest = FangyuanAuthoringRunManifest::new(run_id.clone(), created_at_utc);
            append_run_log_event(
                &run_directory,
                &run_id,
                "prepare_task",
                "invalid_task",
                Some(sha256_hex(&task_bytes)),
            )?;
            refresh_run_manifest(&run_directory, &mut manifest)?;
            return Err(FangyuanAuthoringCliError::OperationFailed {
                run_id,
                code: match error {
                    FangyuanAuthoringTaskError::Json(_) => "invalid_task_json",
                    _ => "invalid_task_contract",
                },
            });
        }
    };

    let prepared = FangyuanAuthoringPreparedTask {
        schema_version: FANGYUAN_AUTHORING_PREPARED_TASK_SCHEMA_VERSION.to_string(),
        source_task_sha256: sha256_hex(&task_bytes),
        task,
    };
    let prepared_bytes = canonical_json_bytes(&prepared)?;
    let prepared_task_sha256 = sha256_hex(&prepared_bytes);
    write_new_run_file(&run_directory, TASK_PREPARED_FILE, &prepared_bytes)?;

    let catalog_context =
        match load_catalog_context(repository_root, &prepared, &prepared_task_sha256) {
            Ok(context) => context,
            Err(_) => {
                let mut manifest =
                    FangyuanAuthoringRunManifest::new(run_id.clone(), created_at_utc);
                append_run_log_event(
                    &run_directory,
                    &run_id,
                    "prepare_task",
                    "catalog_context_unavailable",
                    None,
                )?;
                refresh_run_manifest(&run_directory, &mut manifest)?;
                return Err(FangyuanAuthoringCliError::OperationFailed {
                    run_id,
                    code: "catalog_context_unavailable",
                });
            }
        };
    let catalog_context_bytes = canonical_json_bytes(&catalog_context)?;
    let catalog_context_sha256 = sha256_hex(&catalog_context_bytes);
    write_new_run_file(&run_directory, CATALOG_CONTEXT_FILE, &catalog_context_bytes)?;

    let mut manifest = FangyuanAuthoringRunManifest::new(run_id.clone(), created_at_utc);
    append_run_log_event(
        &run_directory,
        &run_id,
        "prepare_task",
        "prepared",
        Some(prepared.source_task_sha256.clone()),
    )?;
    append_run_log_event(
        &run_directory,
        &run_id,
        "prepare_task",
        "catalog_context_prepared",
        Some(catalog_context_sha256),
    )?;
    refresh_run_manifest(&run_directory, &mut manifest)?;
    Ok(report_from_manifest(
        FangyuanAuthoringCommandName::PrepareTask,
        FangyuanAuthoringCommandStatus::Prepared,
        &manifest,
    ))
}

fn validate_draft_placeholder(
    repository_root: &Path,
    run_id: &str,
) -> Result<FangyuanAuthoringCliReport, FangyuanAuthoringCliError> {
    let (run_directory, mut manifest) = open_run(repository_root, run_id)?;
    let code = if run_file_exists(&run_directory, DRAFT_BLUEPRINT_FILE)? {
        "validate_draft_not_implemented"
    } else {
        "draft_missing"
    };
    append_run_log_event(&run_directory, run_id, "validate_draft", code, None)?;
    refresh_run_manifest(&run_directory, &mut manifest)?;
    Err(FangyuanAuthoringCliError::OperationFailed {
        run_id: run_id.to_string(),
        code,
    })
}

fn promote_placeholder(
    repository_root: &Path,
    run_id: &str,
) -> Result<FangyuanAuthoringCliReport, FangyuanAuthoringCliError> {
    let (run_directory, mut manifest) = open_run(repository_root, run_id)?;
    append_run_log_event(
        &run_directory,
        run_id,
        "promote",
        "promote_not_implemented",
        None,
    )?;
    refresh_run_manifest(&run_directory, &mut manifest)?;
    Err(FangyuanAuthoringCliError::OperationFailed {
        run_id: run_id.to_string(),
        code: "promote_not_implemented",
    })
}

fn inspect_run(
    repository_root: &Path,
    run_id: &str,
) -> Result<FangyuanAuthoringCliReport, FangyuanAuthoringCliError> {
    let (_, manifest) = open_run(repository_root, run_id)?;
    Ok(report_from_manifest(
        FangyuanAuthoringCommandName::InspectRun,
        FangyuanAuthoringCommandStatus::Inspected,
        &manifest,
    ))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringPreparedTask {
    schema_version: String,
    source_task_sha256: String,
    task: FangyuanAuthoringTask,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringCatalogContext {
    schema_version: String,
    prepared_task_sha256: String,
    catalog_content_sha256: String,
    catalog_dependency_sha256: String,
    sampling_contract: FangyuanAuthoringCatalogSamplingContract,
    selection: FangyuanAuthoringCatalogSelection,
    presets: Vec<FangyuanAuthoringCatalogPreset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringCatalogSamplingContract {
    surface_color_normal_coordinates: String,
    noise_coordinates: String,
    ambient_occlusion: f32,
    pbr_parameter_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringCatalogSelection {
    strategy: FangyuanAuthoringCatalogSelectionStrategy,
    confidence: FangyuanAuthoringCatalogSelectionConfidence,
    reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FangyuanAuthoringCatalogSelectionStrategy {
    MaterialKeyword,
    RoleAndPurpose,
    FullCatalogFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FangyuanAuthoringCatalogSelectionConfidence {
    High,
    Low,
    FullFallback,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringCatalogPreset {
    preset_id: String,
    name_zh: String,
    description_zh: String,
    material_family: String,
    preferred_roles: Vec<String>,
    primitive_kinds: Vec<String>,
    pbr: FangyuanAuthoringCatalogPbrSummary,
    projection: FangyuanCatalogProjection,
    render_model: FangyuanCatalogRenderModel,
    noise_suggestion: Option<FangyuanAuthoringCatalogNoiseSuggestion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringCatalogPbrSummary {
    roughness: f32,
    metallic: f32,
    reflectance: f32,
    normal_enabled: bool,
    normal_strength: f32,
    ambient_occlusion: f32,
    parameter_source: String,
    physical_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringCatalogNoiseSuggestion {
    targets: Vec<String>,
    parameter_status: String,
}

fn load_catalog_context(
    repository_root: &Path,
    prepared: &FangyuanAuthoringPreparedTask,
    prepared_task_sha256: &str,
) -> Result<FangyuanAuthoringCatalogContext, FangyuanAuthoringCliError> {
    let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
        repository_root.join("project/assets/fangyuan"),
    );
    let registry = FangyuanSurfacePresetRegistry::load_from_paths(&paths).map_err(|_| {
        FangyuanAuthoringCliError::Boundary(
            "validated Fangyuan catalog bundle is unavailable for local authoring",
        )
    })?;
    build_catalog_context(&prepared.task, prepared_task_sha256, &registry)
}

fn build_catalog_context(
    task: &FangyuanAuthoringTask,
    prepared_task_sha256: &str,
    registry: &FangyuanSurfacePresetRegistry,
) -> Result<FangyuanAuthoringCatalogContext, FangyuanAuthoringCliError> {
    build_catalog_context_from_metadata(
        task,
        prepared_task_sha256,
        registry.catalog_metadata(),
        registry.cache_identity(),
    )
}

fn build_catalog_context_from_metadata(
    task: &FangyuanAuthoringTask,
    prepared_task_sha256: &str,
    metadata: &[FangyuanSurfacePresetCatalogMetadata],
    identity: &FangyuanSurfacePresetCacheIdentity,
) -> Result<FangyuanAuthoringCatalogContext, FangyuanAuthoringCliError> {
    if metadata
        .iter()
        .any(|preset| preset.ambient_occlusion != 1.0)
    {
        return Err(FangyuanAuthoringCliError::Boundary(
            "validated Fangyuan catalog must keep ambient occlusion at 1.0",
        ));
    }
    let mut allowed: Vec<_> = metadata
        .iter()
        .filter(|preset| catalog_preset_is_allowed_for_task(preset, task))
        .collect();
    allowed.sort_by(|left, right| left.preset_id.cmp(&right.preset_id));
    if allowed.is_empty() {
        return Err(FangyuanAuthoringCliError::Boundary(
            "task restrictions exclude every validated Fangyuan surface preset",
        ));
    }

    let prompt = task.prompt.to_lowercase();
    let matched_families = matching_material_families(&prompt);
    let (mut selected, selection) = if !matched_families.is_empty() {
        let selected: Vec<_> = allowed
            .iter()
            .copied()
            .filter(|preset| matched_families.contains(preset.material_family.as_str()))
            .collect();
        if selected.is_empty() {
            full_catalog_fallback(allowed)
        } else {
            (
                selected,
                FangyuanAuthoringCatalogSelection {
                    strategy: FangyuanAuthoringCatalogSelectionStrategy::MaterialKeyword,
                    confidence: FangyuanAuthoringCatalogSelectionConfidence::High,
                    reason: "task material keywords match validated catalog families".to_string(),
                },
            )
        }
    } else {
        let role_candidates = collect_role_and_purpose_candidates(task, &allowed);
        if role_candidates.is_empty() {
            full_catalog_fallback(allowed)
        } else {
            (
                role_candidates.into_iter().take(3).collect(),
                FangyuanAuthoringCatalogSelection {
                    strategy: FangyuanAuthoringCatalogSelectionStrategy::RoleAndPurpose,
                    confidence: FangyuanAuthoringCatalogSelectionConfidence::Low,
                    reason:
                        "task roles and object purpose prioritize at most three catalog presets"
                            .to_string(),
                },
            )
        }
    };
    selected.sort_by(|left, right| left.preset_id.cmp(&right.preset_id));

    let presets = selected
        .into_iter()
        .map(|preset| catalog_context_preset(preset, task))
        .collect();

    Ok(FangyuanAuthoringCatalogContext {
        schema_version: FANGYUAN_AUTHORING_CATALOG_CONTEXT_SCHEMA_VERSION.to_string(),
        prepared_task_sha256: prepared_task_sha256.to_string(),
        catalog_content_sha256: catalog_identity_sha256("content", identity.content_hash),
        catalog_dependency_sha256: catalog_identity_sha256("dependency", identity.dependency_hash),
        sampling_contract: FangyuanAuthoringCatalogSamplingContract {
            surface_color_normal_coordinates: "same_surface_preset_coordinates".to_string(),
            noise_coordinates: "independent_registered_noise_suggestion".to_string(),
            ambient_occlusion: 1.0,
            pbr_parameter_status: "semantic_estimate".to_string(),
        },
        selection,
        presets,
    })
}

fn full_catalog_fallback(
    presets: Vec<&FangyuanSurfacePresetCatalogMetadata>,
) -> (
    Vec<&FangyuanSurfacePresetCatalogMetadata>,
    FangyuanAuthoringCatalogSelection,
) {
    (
        presets,
        FangyuanAuthoringCatalogSelection {
            strategy: FangyuanAuthoringCatalogSelectionStrategy::FullCatalogFallback,
            confidence: FangyuanAuthoringCatalogSelectionConfidence::FullFallback,
            reason:
                "task has no reliable catalog material or role match; exposing all allowed presets"
                    .to_string(),
        },
    )
}

fn collect_role_and_purpose_candidates<'a>(
    task: &FangyuanAuthoringTask,
    presets: &[&'a FangyuanSurfacePresetCatalogMetadata],
) -> Vec<&'a FangyuanSurfacePresetCatalogMetadata> {
    let task_roles: BTreeSet<_> = task
        .allowed
        .roles
        .iter()
        .map(|role| role.as_str())
        .collect();
    let purpose_roles = purpose_preferred_catalog_roles(task.object_purpose);
    let mut ranked: Vec<_> = presets
        .iter()
        .copied()
        .filter_map(|preset| {
            let task_role_matches = preset
                .preferred_roles
                .iter()
                .filter(|role| task_roles.contains(role.as_str()))
                .count();
            let purpose_role_matches = preset
                .preferred_roles
                .iter()
                .filter(|role| purpose_roles.contains(&role.as_str()))
                .count();
            // Object purpose only orders explicit task-role matches. Purpose
            // alone is too broad to safely hide the remainder of the catalog.
            (task_role_matches > 0).then_some((
                std::cmp::Reverse(task_role_matches * 2 + purpose_role_matches),
                preset,
            ))
        })
        .collect();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.preset_id.cmp(&right.1.preset_id))
    });
    ranked.into_iter().map(|(_, preset)| preset).collect()
}

fn purpose_preferred_catalog_roles(
    purpose: FangyuanAuthoringObjectPurpose,
) -> &'static [&'static str] {
    match purpose {
        FangyuanAuthoringObjectPurpose::AvatarAppearance => &["clothing", "prop"],
        FangyuanAuthoringObjectPurpose::HomeDecoration => &["structure", "prop", "wall"],
        FangyuanAuthoringObjectPurpose::EquipmentAppearance => &["prop", "structure"],
        FangyuanAuthoringObjectPurpose::SkillVisual => &["effect_surface", "special_surface"],
        FangyuanAuthoringObjectPurpose::NpcAppearance => &["prop", "structure"],
        FangyuanAuthoringObjectPurpose::SceneDecoration => &["structure", "ground", "wall", "prop"],
    }
}

fn matching_material_families(prompt: &str) -> BTreeSet<&'static str> {
    const FAMILY_KEYWORDS: [(&str, &[&str]); 15] = [
        (
            "masonry",
            &["masonry", "stone", "brick", "concrete", "石", "砖", "水泥"],
        ),
        ("terrain", &["terrain", "earth", "soil", "泥", "土", "地表"]),
        (
            "metal",
            &["metal", "steel", "iron", "金属", "钢", "铁", "铜"],
        ),
        ("wood_bark", &["wood", "timber", "bark", "木", "树皮"]),
        ("leather", &["leather", "皮革"]),
        ("textile", &["textile", "fabric", "cloth", "布", "织物"]),
        (
            "foliage",
            &["foliage", "leaf", "plant", "树叶", "植物", "草"],
        ),
        ("ice_snow", &["ice", "snow", "冰", "雪"]),
        ("water", &["water", "水"]),
        ("lava", &["lava", "熔岩", "岩浆"]),
        ("sky", &["sky", "cloud", "天空", "云"]),
        (
            "electronics_scifi",
            &["electronics", "circuit", "sci-fi", "电子", "电路", "科幻"],
        ),
        ("ceramic_tile", &["ceramic", "tile", "陶瓷", "瓷砖"]),
        ("composite", &["composite", "复合"]),
        (
            "coating_polymer",
            &["polymer", "plastic", "coating", "塑料", "涂层"],
        ),
    ];
    FAMILY_KEYWORDS
        .into_iter()
        .filter_map(|(family, keywords)| {
            keywords
                .iter()
                .any(|keyword| prompt.contains(keyword))
                .then_some(family)
        })
        .collect()
}

fn catalog_preset_is_allowed_for_task(
    preset: &FangyuanSurfacePresetCatalogMetadata,
    task: &FangyuanAuthoringTask,
) -> bool {
    let forbidden = &task.additional_forbidden_features;
    if forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::Transparent)
        && preset.alpha < 1.0
    {
        return false;
    }
    if forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::Emissive)
        && (preset.emissive_intensity > 0.0
            || matches!(
                preset.render_model,
                FangyuanCatalogRenderModel::StandardPbrEmissive
            ))
    {
        return false;
    }
    if forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::SpecialSurface)
        && matches!(
            preset.render_model,
            FangyuanCatalogRenderModel::SpecialWater | FangyuanCatalogRenderModel::SpecialUnlitSky
        )
    {
        return false;
    }
    if forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::WaterSurface)
        && preset.material_family == "water"
    {
        return false;
    }
    if forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::SkySurface)
        && preset.material_family == "sky"
    {
        return false;
    }
    if forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::LavaSurface)
        && preset.material_family == "lava"
    {
        return false;
    }
    !(forbidden.contains(&FangyuanAuthoringAdditionalForbiddenFeature::TriplanarRequired)
        && matches!(preset.projection, FangyuanCatalogProjection::Triplanar))
}

fn catalog_context_preset(
    preset: &FangyuanSurfacePresetCatalogMetadata,
    task: &FangyuanAuthoringTask,
) -> FangyuanAuthoringCatalogPreset {
    FangyuanAuthoringCatalogPreset {
        preset_id: preset.preset_id.clone(),
        name_zh: preset.name_zh.clone(),
        description_zh: preset.description_zh.clone(),
        material_family: preset.material_family.clone(),
        preferred_roles: preset.preferred_roles.clone(),
        primitive_kinds: preset.recommended_primitive_kinds.clone(),
        pbr: FangyuanAuthoringCatalogPbrSummary {
            roughness: preset.roughness,
            metallic: preset.metallic,
            reflectance: preset.reflectance,
            normal_enabled: preset.normal_enabled,
            normal_strength: preset.normal_strength,
            ambient_occlusion: 1.0,
            parameter_source: preset.parameter_source.clone(),
            physical_status: preset.physical_status.clone(),
        },
        projection: preset.projection,
        render_model: preset.render_model,
        noise_suggestion: task
            .material_budget
            .allow_catalog_suggested_noise
            .then(|| {
                preset.noise_suggestion.as_ref().map(|noise| {
                    FangyuanAuthoringCatalogNoiseSuggestion {
                        targets: noise.targets.clone(),
                        parameter_status: noise.parameter_status.clone(),
                    }
                })
            })
            .flatten(),
    }
}

fn catalog_identity_sha256(kind: &str, identity: u64) -> String {
    let mut bytes = Vec::with_capacity(kind.len() + 8 + 32);
    bytes.extend_from_slice(b"fangyuan-authoring-catalog-identity/v1:");
    bytes.extend_from_slice(kind.as_bytes());
    bytes.push(b':');
    bytes.extend_from_slice(&identity.to_be_bytes());
    sha256_hex(&bytes)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringRunManifest {
    schema_version: String,
    run_id: String,
    created_at_utc: String,
    time_source: String,
    artifacts: Vec<FangyuanAuthoringArtifactRecord>,
}

impl FangyuanAuthoringRunManifest {
    fn new(run_id: String, created_at_utc: String) -> Self {
        Self {
            schema_version: FANGYUAN_AUTHORING_RUN_SCHEMA_VERSION.to_string(),
            run_id,
            created_at_utc,
            time_source: "local_tool_clock".to_string(),
            artifacts: Vec::new(),
        }
    }

    fn validate(&self, expected_run_id: &str) -> Result<(), FangyuanAuthoringCliError> {
        if self.schema_version != FANGYUAN_AUTHORING_RUN_SCHEMA_VERSION
            || self.time_source != "local_tool_clock"
            || self.run_id != expected_run_id
            || !is_valid_run_id(&self.run_id)
        {
            return Err(FangyuanAuthoringCliError::Boundary(
                "run manifest does not match the local authoring contract",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringArtifactRecord {
    path: String,
    schema_version: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringLogSummary {
    schema_version: String,
    run_id: String,
    events: Vec<FangyuanAuthoringLogEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FangyuanAuthoringLogEvent {
    stage: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_sha256: Option<String>,
}

impl FangyuanAuthoringLogSummary {
    fn new(run_id: &str) -> Self {
        Self {
            schema_version: FANGYUAN_AUTHORING_LOG_SUMMARY_SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            events: Vec::new(),
        }
    }
}

fn append_run_log_event(
    run_directory: &Path,
    run_id: &str,
    stage: &str,
    code: &str,
    artifact_sha256: Option<String>,
) -> Result<(), FangyuanAuthoringCliError> {
    let log_path = run_file_path(run_directory, LOG_SUMMARY_FILE)?;
    let mut summary = if log_path.exists() {
        let bytes = read_safe_run_file(run_directory, LOG_SUMMARY_FILE)?;
        serde_json::from_slice::<FangyuanAuthoringLogSummary>(&bytes).map_err(|_| {
            FangyuanAuthoringCliError::Boundary(
                "run log does not match the local authoring contract",
            )
        })?
    } else {
        FangyuanAuthoringLogSummary::new(run_id)
    };
    if summary.schema_version != FANGYUAN_AUTHORING_LOG_SUMMARY_SCHEMA_VERSION
        || summary.run_id != run_id
    {
        return Err(FangyuanAuthoringCliError::Boundary(
            "run log does not match the current run",
        ));
    }
    summary.events.push(FangyuanAuthoringLogEvent {
        stage: stage.to_string(),
        code: code.to_string(),
        artifact_sha256,
    });
    summary.events.sort_by(|left, right| {
        (&left.stage, &left.code, &left.artifact_sha256).cmp(&(
            &right.stage,
            &right.code,
            &right.artifact_sha256,
        ))
    });
    write_or_replace_run_file(
        run_directory,
        LOG_SUMMARY_FILE,
        &canonical_json_bytes(&summary)?,
    )
}

fn refresh_run_manifest(
    run_directory: &Path,
    manifest: &mut FangyuanAuthoringRunManifest,
) -> Result<(), FangyuanAuthoringCliError> {
    let mut artifacts = Vec::new();
    for relative_path in known_run_artifact_paths() {
        if run_file_exists(run_directory, relative_path)? {
            let bytes = read_safe_run_file(run_directory, relative_path)?;
            artifacts.push(FangyuanAuthoringArtifactRecord {
                path: relative_path.to_string(),
                schema_version: artifact_schema_version(relative_path).to_string(),
                bytes: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
        }
    }
    manifest.artifacts = artifacts;
    write_or_replace_run_file(
        run_directory,
        RUN_MANIFEST_FILE,
        &canonical_json_bytes(manifest)?,
    )
}

fn report_from_manifest(
    command: FangyuanAuthoringCommandName,
    status: FangyuanAuthoringCommandStatus,
    manifest: &FangyuanAuthoringRunManifest,
) -> FangyuanAuthoringCliReport {
    FangyuanAuthoringCliReport {
        command,
        run_id: manifest.run_id.clone(),
        status,
        report_relative_path: format!(
            "{}/{}/{}",
            FANGYUAN_AUTHORING_RUN_DIRECTORY, manifest.run_id, LOG_SUMMARY_FILE
        ),
        artifact_count: manifest.artifacts.len(),
    }
}

fn canonical_repository_root(repository_root: &Path) -> Result<PathBuf, FangyuanAuthoringCliError> {
    let canonical =
        fs::canonicalize(repository_root).map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to resolve repository root",
            source,
        })?;
    if !fs::metadata(&canonical)
        .map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to inspect repository root",
            source,
        })?
        .is_dir()
    {
        return Err(FangyuanAuthoringCliError::Boundary(
            "repository root must be a directory",
        ));
    }
    Ok(canonical)
}

fn authoring_root(repository_root: &Path) -> Result<PathBuf, FangyuanAuthoringCliError> {
    let candidate = repository_root.join(FANGYUAN_AUTHORING_RUN_DIRECTORY);
    fs::create_dir_all(&candidate).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to create local authoring run root",
        source,
    })?;
    let canonical =
        fs::canonicalize(&candidate).map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to resolve local authoring run root",
            source,
        })?;
    if !canonical.starts_with(repository_root) {
        return Err(FangyuanAuthoringCliError::Boundary(
            "local authoring run root escapes the repository",
        ));
    }
    Ok(canonical)
}

fn resolve_repository_input_file(
    repository_root: &Path,
    relative_path: &Path,
) -> Result<PathBuf, FangyuanAuthoringCliError> {
    validate_relative_path(relative_path)?;
    if relative_path_starts_with_authoring_root(relative_path) {
        return Err(FangyuanAuthoringCliError::Boundary(
            "task input must not be read from an authoring run directory",
        ));
    }
    let candidate = repository_root.join(relative_path);
    let canonical =
        fs::canonicalize(candidate).map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to resolve task input",
            source,
        })?;
    if !canonical.starts_with(repository_root) {
        return Err(FangyuanAuthoringCliError::Boundary(
            "task input escapes the repository",
        ));
    }
    if !fs::metadata(&canonical)
        .map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to inspect task input",
            source,
        })?
        .is_file()
    {
        return Err(FangyuanAuthoringCliError::Boundary(
            "task input must be a regular file",
        ));
    }
    Ok(canonical)
}

fn create_run_directory(
    repository_root: &Path,
) -> Result<(String, PathBuf, String), FangyuanAuthoringCliError> {
    let authoring_root = authoring_root(repository_root)?;
    for _ in 0..16 {
        let created_at_utc = current_utc_rfc3339()?;
        let run_id = format!(
            "fygen-v1-{}-{}",
            compact_utc_run_timestamp(&created_at_utc)?,
            local_nonce()?
        );
        let candidate = authoring_root.join(&run_id);
        match fs::create_dir(&candidate) {
            Ok(()) => {
                let canonical = fs::canonicalize(&candidate).map_err(|source| {
                    FangyuanAuthoringCliError::Io {
                        operation: "failed to resolve newly created authoring run",
                        source,
                    }
                })?;
                if canonical.parent() != Some(authoring_root.as_path()) {
                    return Err(FangyuanAuthoringCliError::Boundary(
                        "new authoring run escapes the run root",
                    ));
                }
                return Ok((run_id, canonical, created_at_utc));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(FangyuanAuthoringCliError::Io {
                    operation: "failed to create authoring run",
                    source,
                });
            }
        }
    }
    Err(FangyuanAuthoringCliError::Boundary(
        "could not allocate a unique local authoring run id",
    ))
}

fn open_run(
    repository_root: &Path,
    run_id: &str,
) -> Result<(PathBuf, FangyuanAuthoringRunManifest), FangyuanAuthoringCliError> {
    if !is_valid_run_id(run_id) {
        return Err(FangyuanAuthoringCliError::InvalidRunId);
    }
    let authoring_root = authoring_root(repository_root)?;
    let candidate = authoring_root.join(run_id);
    let run_directory =
        fs::canonicalize(candidate).map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to resolve authoring run",
            source,
        })?;
    if run_directory.parent() != Some(authoring_root.as_path())
        || !fs::metadata(&run_directory)
            .map_err(|source| FangyuanAuthoringCliError::Io {
                operation: "failed to inspect authoring run",
                source,
            })?
            .is_dir()
    {
        return Err(FangyuanAuthoringCliError::Boundary(
            "authoring run must be a direct directory below the run root",
        ));
    }
    let bytes = read_safe_run_file(&run_directory, RUN_MANIFEST_FILE)?;
    let manifest =
        serde_json::from_slice::<FangyuanAuthoringRunManifest>(&bytes).map_err(|_| {
            FangyuanAuthoringCliError::Boundary(
                "run manifest does not match the local authoring contract",
            )
        })?;
    manifest.validate(run_id)?;
    Ok((run_directory, manifest))
}

fn validate_relative_path(path: &Path) -> Result<(), FangyuanAuthoringCliError> {
    if path.as_os_str().is_empty() || path.is_absolute() || path.to_string_lossy().contains("://") {
        return Err(FangyuanAuthoringCliError::Boundary(
            "path must be a repository-relative non-URL path",
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(FangyuanAuthoringCliError::Boundary(
                    "path must not contain a root, drive, or parent traversal",
                ));
            }
        }
    }
    Ok(())
}

fn relative_path_starts_with_authoring_root(path: &Path) -> bool {
    let components: Vec<_> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();
    matches!(
        components.as_slice(),
        ["summary", "fangyuan-generation", ..]
    )
}

fn run_file_path(
    run_directory: &Path,
    relative_path: &str,
) -> Result<PathBuf, FangyuanAuthoringCliError> {
    let path = Path::new(relative_path);
    validate_relative_path(path)?;
    let candidate = run_directory.join(path);
    if !candidate.starts_with(run_directory) {
        return Err(FangyuanAuthoringCliError::Boundary(
            "run artifact path escapes the current run",
        ));
    }
    Ok(candidate)
}

fn run_file_exists(
    run_directory: &Path,
    relative_path: &str,
) -> Result<bool, FangyuanAuthoringCliError> {
    let path = run_file_path(run_directory, relative_path)?;
    if !path.exists() {
        return Ok(false);
    }
    ensure_safe_existing_run_file(run_directory, &path)?;
    Ok(true)
}

fn read_safe_run_file(
    run_directory: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, FangyuanAuthoringCliError> {
    let path = run_file_path(run_directory, relative_path)?;
    ensure_safe_existing_run_file(run_directory, &path)?;
    fs::read(path).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to read authoring run artifact",
        source,
    })
}

fn ensure_safe_existing_run_file(
    run_directory: &Path,
    path: &Path,
) -> Result<(), FangyuanAuthoringCliError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to inspect authoring run artifact",
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FangyuanAuthoringCliError::Boundary(
            "authoring run artifact must be a regular file",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to resolve authoring run artifact",
        source,
    })?;
    if !canonical.starts_with(run_directory) {
        return Err(FangyuanAuthoringCliError::Boundary(
            "authoring run artifact escapes the current run",
        ));
    }
    Ok(())
}

fn write_new_run_file(
    run_directory: &Path,
    relative_path: &str,
    bytes: &[u8],
) -> Result<(), FangyuanAuthoringCliError> {
    let path = run_file_path(run_directory, relative_path)?;
    ensure_safe_run_parent(run_directory, &path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    use std::io::Write;
    let mut file = options
        .open(path)
        .map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to create authoring run artifact",
            source,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to write authoring run artifact",
            source,
        })
}

fn write_or_replace_run_file(
    run_directory: &Path,
    relative_path: &str,
    bytes: &[u8],
) -> Result<(), FangyuanAuthoringCliError> {
    let path = run_file_path(run_directory, relative_path)?;
    ensure_safe_run_parent(run_directory, &path)?;
    if path.exists() {
        ensure_safe_existing_run_file(run_directory, &path)?;
    }
    fs::write(path, bytes).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to write authoring run artifact",
        source,
    })
}

fn ensure_safe_run_parent(
    run_directory: &Path,
    path: &Path,
) -> Result<(), FangyuanAuthoringCliError> {
    let parent = path.parent().ok_or(FangyuanAuthoringCliError::Boundary(
        "authoring run artifact has no parent",
    ))?;
    fs::create_dir_all(parent).map_err(|source| FangyuanAuthoringCliError::Io {
        operation: "failed to create authoring run artifact directory",
        source,
    })?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|source| FangyuanAuthoringCliError::Io {
            operation: "failed to resolve authoring run artifact directory",
            source,
        })?;
    if !canonical_parent.starts_with(run_directory) {
        return Err(FangyuanAuthoringCliError::Boundary(
            "authoring run artifact directory escapes the current run",
        ));
    }
    Ok(())
}

fn known_run_artifact_paths() -> [&'static str; 8] {
    [
        TASK_SOURCE_FILE,
        TASK_PREPARED_FILE,
        CATALOG_CONTEXT_FILE,
        DRAFT_BLUEPRINT_FILE,
        CANONICAL_BLUEPRINT_FILE,
        AUDIT_REPORT_FILE,
        PROMOTION_DECISION_FILE,
        LOG_SUMMARY_FILE,
    ]
}

fn artifact_schema_version(relative_path: &str) -> &'static str {
    match relative_path {
        TASK_SOURCE_FILE => FANGYUAN_AUTHORING_TASK_SCHEMA_VERSION,
        TASK_PREPARED_FILE => FANGYUAN_AUTHORING_PREPARED_TASK_SCHEMA_VERSION,
        CATALOG_CONTEXT_FILE => "fangyuan-generation-catalog-context/v1",
        DRAFT_BLUEPRINT_FILE | CANONICAL_BLUEPRINT_FILE => "fangyuan-blueprint-ron/v1",
        AUDIT_REPORT_FILE => "fangyuan-generation-audit-report/v1",
        PROMOTION_DECISION_FILE => "fangyuan-generation-promotion-decision/v1",
        LOG_SUMMARY_FILE => FANGYUAN_AUTHORING_LOG_SUMMARY_SCHEMA_VERSION,
        _ => "unknown",
    }
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, FangyuanAuthoringCliError> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| FangyuanAuthoringCliError::Serialization(error.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn validate_identifier(
    value: &str,
    kind: IdentifierKind,
) -> Result<(), FangyuanAuthoringTaskError> {
    if value.is_empty() || value.len() > 64 {
        return Err(FangyuanAuthoringTaskError::InvalidIdentifier { kind });
    }
    let bytes = value.as_bytes();
    let valid_tail = match kind {
        IdentifierKind::TaskId => bytes[1..]
            .iter()
            .all(|byte| is_ascii_lowercase(*byte) || byte.is_ascii_digit() || *byte == b'-'),
        IdentifierKind::StableId => bytes[1..]
            .iter()
            .all(|byte| is_ascii_lowercase(*byte) || byte.is_ascii_digit() || *byte == b'_'),
    };
    if !is_ascii_lowercase(bytes[0]) || !valid_tail {
        return Err(FangyuanAuthoringTaskError::InvalidIdentifier { kind });
    }
    Ok(())
}

fn validate_budget_number(
    name: &'static str,
    value: f32,
    maximum: f32,
) -> Result<(), FangyuanAuthoringTaskError> {
    if !value.is_finite() || value <= 0.0 || value > maximum {
        return Err(FangyuanAuthoringTaskError::InvalidBudget(match name {
            "bounds.width" => "bounds.width must be finite and in 0..=64",
            "bounds.depth" => "bounds.depth must be finite and in 0..=64",
            "bounds.height" => "bounds.height must be finite and in 0..=64",
            "max_primitive_volume" => "max_primitive_volume must be finite and in 0..=4096",
            _ => "max_total_volume must be finite and in 0..=32768",
        }));
    }
    Ok(())
}

fn has_unique_roles(roles: &[FangyuanPrimitiveRole]) -> bool {
    let mut values = BTreeSet::new();
    roles.iter().all(|role| values.insert(role.as_str()))
}

fn has_unique_values<T: Ord>(values: &[T]) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().all(|value| seen.insert(value))
}

const fn is_ascii_lowercase(byte: u8) -> bool {
    byte >= b'a' && byte <= b'z'
}

fn is_valid_run_id(run_id: &str) -> bool {
    let Some(suffix) = run_id.strip_prefix("fygen-v1-") else {
        return false;
    };
    let mut parts = suffix.split('-');
    let (Some(timestamp), Some(nonce), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    timestamp.len() == 16
        && timestamp.as_bytes().get(8) == Some(&b'T')
        && timestamp.as_bytes().get(15) == Some(&b'Z')
        && timestamp
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 8 | 15) || byte.is_ascii_digit())
        && nonce.len() == 12
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn local_nonce() -> Result<String, FangyuanAuthoringCliError> {
    let mut bytes = [0_u8; 6];
    fill_random_bytes(&mut bytes)
        .map_err(|error| FangyuanAuthoringCliError::Random(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn current_utc_rfc3339() -> Result<String, FangyuanAuthoringCliError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            FangyuanAuthoringCliError::Boundary("local tool clock predates the Unix epoch")
        })?
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let seconds_in_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_unix_days(days);
    let hour = seconds_in_day / 3_600;
    let minute = (seconds_in_day % 3_600) / 60;
    let second = seconds_in_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn compact_utc_run_timestamp(rfc3339: &str) -> Result<String, FangyuanAuthoringCliError> {
    let bytes = rfc3339.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return Err(FangyuanAuthoringCliError::Boundary(
            "local tool clock did not produce RFC 3339 UTC",
        ));
    }
    Ok(format!(
        "{}{}{}T{}{}{}Z",
        &rfc3339[0..4],
        &rfc3339[5..7],
        &rfc3339[8..10],
        &rfc3339[11..13],
        &rfc3339[14..16],
        &rfc3339[17..19]
    ))
}

fn civil_date_from_unix_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

fn parse_exact_option(
    args: &[String],
    expected_name: &str,
) -> Result<String, FangyuanAuthoringCliError> {
    if args.len() != 2 || args[0] != expected_name || args[1].is_empty() {
        let message = if args.first().is_some_and(|value| value.starts_with("--"))
            && args.first().is_some_and(|value| value != expected_name)
        {
            format!("unknown argument `{}`", args[0])
        } else {
            format!("{expected_name} requires a value")
        };
        return Err(FangyuanAuthoringCliError::InvalidArgs(message));
    }
    Ok(args[1].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture_task_bytes() -> Vec<u8> {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/方圆灵构/规范/本地生成任务.v1.fixture.json"
        ))
        .to_vec()
    }

    fn temporary_repository() -> TempDir {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("tasks")).unwrap();
        directory
    }

    fn copy_catalog_fixture(repository: &TempDir) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan/atlases");
        let target = repository.path().join("project/assets/fangyuan/atlases");
        copy_directory(&source, &target);
    }

    fn copy_directory(source: &Path, target: &Path) {
        fs::create_dir_all(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target_path = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory(&entry.path(), &target_path);
            } else {
                fs::copy(entry.path(), target_path).unwrap();
            }
        }
    }

    fn fixture_registry() -> FangyuanSurfacePresetRegistry {
        let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        );
        FangyuanSurfacePresetRegistry::load_from_paths(&paths).unwrap()
    }

    fn fixture_task() -> FangyuanAuthoringTask {
        FangyuanAuthoringTask::parse_json(&fixture_task_bytes()).unwrap()
    }

    fn prepare_options() -> FangyuanAuthoringCliOptions {
        FangyuanAuthoringCliOptions::parse_from(["prepare-task", "--task", "tasks/task.json"])
            .unwrap()
    }

    fn write_fixture_task(repository: &TempDir) {
        copy_catalog_fixture(repository);
        fs::write(
            repository.path().join("tasks/task.json"),
            fixture_task_bytes(),
        )
        .unwrap();
    }

    fn only_run_directory(repository: &TempDir) -> PathBuf {
        let root = repository.path().join(FANGYUAN_AUTHORING_RUN_DIRECTORY);
        let entries: Vec<_> = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(entries.len(), 1);
        entries.into_iter().next().unwrap()
    }

    #[test]
    fn fixture_task_matches_the_strict_rust_contract() {
        let task = FangyuanAuthoringTask::parse_json(&fixture_task_bytes()).unwrap();
        assert_eq!(task.task_id, "stage1-contract-fixture");
        assert_eq!(task.material_budget.max_surface_presets, 4);
        assert_eq!(task.allowed.primitive_kinds.len(), 2);
    }

    #[test]
    fn task_parser_rejects_unknown_fields_and_relaxed_v0_policy() {
        let mut value: serde_json::Value = serde_json::from_slice(&fixture_task_bytes()).unwrap();
        value["provider"] = serde_json::Value::String("not-allowed".to_string());
        assert!(matches!(
            FangyuanAuthoringTask::parse_json(&serde_json::to_vec(&value).unwrap()),
            Err(FangyuanAuthoringTaskError::Json(_))
        ));

        let mut value: serde_json::Value = serde_json::from_slice(&fixture_task_bytes()).unwrap();
        value["material_budget"]["max_surface_presets"] = serde_json::json!(5);
        assert!(matches!(
            FangyuanAuthoringTask::parse_json(&serde_json::to_vec(&value).unwrap()),
            Err(FangyuanAuthoringTaskError::InvalidMaterialBudget(_))
        ));
    }

    #[test]
    fn cli_parser_has_stable_help_missing_and_unknown_command_results() {
        assert!(matches!(
            FangyuanAuthoringCliOptions::parse_from(["--help"]),
            Err(FangyuanAuthoringCliError::Help(_))
        ));
        assert!(matches!(
            FangyuanAuthoringCliOptions::parse_from(Vec::<String>::new()),
            Err(FangyuanAuthoringCliError::InvalidArgs(_))
        ));
        assert!(matches!(
            FangyuanAuthoringCliOptions::parse_from(["unknown-command"]),
            Err(FangyuanAuthoringCliError::InvalidArgs(_))
        ));
        assert_eq!(
            fangyuan_authoring_cli_error_exit_code(&FangyuanAuthoringCliError::InvalidArgs(
                "missing".to_string()
            )),
            2
        );
    }

    #[test]
    fn prepare_task_creates_an_isolated_bundle_and_machine_report() {
        let repository = temporary_repository();
        write_fixture_task(&repository);
        let report = run_fangyuan_authoring_cli(&prepare_options(), repository.path()).unwrap();
        assert_eq!(report.status, FangyuanAuthoringCommandStatus::Prepared);
        assert!(is_valid_run_id(&report.run_id));
        assert_eq!(
            report.report_relative_path,
            format!(
                "{}/{}/{}",
                FANGYUAN_AUTHORING_RUN_DIRECTORY, report.run_id, LOG_SUMMARY_FILE
            )
        );

        let run_directory = only_run_directory(&repository);
        assert!(run_directory.join(TASK_SOURCE_FILE).is_file());
        assert!(run_directory.join(TASK_PREPARED_FILE).is_file());
        assert!(run_directory.join(CATALOG_CONTEXT_FILE).is_file());
        assert!(run_directory.join(LOG_SUMMARY_FILE).is_file());
        assert!(run_directory.join(RUN_MANIFEST_FILE).is_file());
        let context: FangyuanAuthoringCatalogContext =
            serde_json::from_slice(&fs::read(run_directory.join(CATALOG_CONTEXT_FILE)).unwrap())
                .unwrap();
        assert_eq!(
            context.schema_version,
            FANGYUAN_AUTHORING_CATALOG_CONTEXT_SCHEMA_VERSION
        );
        assert!(!context.presets.is_empty());
        assert!(!run_directory.join(CANONICAL_BLUEPRINT_FILE).exists());
        assert!(!run_directory.join(PROMOTION_DECISION_FILE).exists());

        let inspect = FangyuanAuthoringCliOptions::parse_from([
            "inspect-run",
            "--run",
            report.run_id.as_str(),
        ])
        .unwrap();
        let inspected = run_fangyuan_authoring_cli(&inspect, repository.path()).unwrap();
        assert_eq!(inspected.status, FangyuanAuthoringCommandStatus::Inspected);
    }

    #[test]
    fn invalid_task_preserves_source_and_writes_a_failure_report() {
        let repository = temporary_repository();
        let invalid = br#"{"schema_version":"fangyuan-generation-task/v1"}"#;
        fs::write(repository.path().join("tasks/task.json"), invalid).unwrap();
        let result = run_fangyuan_authoring_cli(&prepare_options(), repository.path());
        assert!(matches!(
            result,
            Err(FangyuanAuthoringCliError::OperationFailed {
                code: "invalid_task_json",
                ..
            })
        ));
        let run_directory = only_run_directory(&repository);
        assert_eq!(
            fs::read(run_directory.join(TASK_SOURCE_FILE)).unwrap(),
            invalid
        );
        let log: FangyuanAuthoringLogSummary =
            serde_json::from_slice(&fs::read(run_directory.join(LOG_SUMMARY_FILE)).unwrap())
                .unwrap();
        assert_eq!(log.events[0].code, "invalid_task");
        assert!(!run_directory.join(TASK_PREPARED_FILE).exists());
        assert!(!run_directory.join(CANONICAL_BLUEPRINT_FILE).exists());
    }

    #[test]
    fn task_paths_reject_escape_url_and_authoring_input_output_overlap() {
        let repository = temporary_repository();
        write_fixture_task(&repository);
        for task in [
            "../outside.json",
            "https://example.invalid/task.json",
            "summary/fangyuan-generation/existing/input/task.source.json",
        ] {
            let options =
                FangyuanAuthoringCliOptions::parse_from(["prepare-task", "--task", task]).unwrap();
            assert!(matches!(
                run_fangyuan_authoring_cli(&options, repository.path()),
                Err(FangyuanAuthoringCliError::Boundary(_))
            ));
        }
    }

    #[test]
    fn deferred_commands_leave_canonical_and_decision_absent() {
        let repository = temporary_repository();
        write_fixture_task(&repository);
        let prepared = run_fangyuan_authoring_cli(&prepare_options(), repository.path()).unwrap();
        let validate = FangyuanAuthoringCliOptions::parse_from([
            "validate-draft",
            "--run",
            prepared.run_id.as_str(),
        ])
        .unwrap();
        assert!(matches!(
            run_fangyuan_authoring_cli(&validate, repository.path()),
            Err(FangyuanAuthoringCliError::OperationFailed {
                code: "draft_missing",
                ..
            })
        ));
        let promote =
            FangyuanAuthoringCliOptions::parse_from(["promote", "--run", prepared.run_id.as_str()])
                .unwrap();
        assert!(matches!(
            run_fangyuan_authoring_cli(&promote, repository.path()),
            Err(FangyuanAuthoringCliError::OperationFailed {
                code: "promote_not_implemented",
                ..
            })
        ));
        let run_directory = only_run_directory(&repository);
        assert!(!run_directory.join(CANONICAL_BLUEPRINT_FILE).exists());
        assert!(!run_directory.join(PROMOTION_DECISION_FILE).exists());
        assert_eq!(
            fangyuan_authoring_cli_error_exit_code(&FangyuanAuthoringCliError::OperationFailed {
                run_id: prepared.run_id,
                code: "draft_missing",
            }),
            1
        );
    }

    #[test]
    fn catalog_context_uses_validated_metadata_and_is_byte_stable() {
        let task = fixture_task();
        let registry = fixture_registry();
        let prepared_task_sha256 = "a".repeat(64);
        let first = build_catalog_context(&task, &prepared_task_sha256, &registry).unwrap();
        let second = build_catalog_context(&task, &prepared_task_sha256, &registry).unwrap();
        let first_bytes = canonical_json_bytes(&first).unwrap();
        assert_eq!(first_bytes, canonical_json_bytes(&second).unwrap());
        assert_eq!(first.catalog_content_sha256.len(), 64);
        assert_eq!(first.catalog_dependency_sha256.len(), 64);
        assert!(
            first
                .presets
                .windows(2)
                .all(|pair| pair[0].preset_id <= pair[1].preset_id)
        );
        assert!(
            first
                .presets
                .iter()
                .all(|preset| preset.pbr.ambient_occlusion == 1.0)
        );

        let encoded = String::from_utf8(first_bytes).unwrap();
        assert!(!encoded.contains("surface_color.png"));
        assert!(!encoded.contains("surface_normal.png"));
        assert!(!encoded.contains("procedural_noise.png"));
        assert!(!encoded.contains("noise_x"));

        let mut changed_identity = registry.cache_identity().clone();
        changed_identity.content_hash ^= 1;
        let changed = build_catalog_context_from_metadata(
            &task,
            &prepared_task_sha256,
            registry.catalog_metadata(),
            &changed_identity,
        )
        .unwrap();
        assert_ne!(first.catalog_content_sha256, changed.catalog_content_sha256);
    }

    #[test]
    fn catalog_context_prefers_material_keyword_then_role_and_full_fallback() {
        let registry = fixture_registry();
        let prepared_task_sha256 = "b".repeat(64);

        let mut keyword_task = fixture_task();
        keyword_task.prompt = "a metal structure with a safe surface".to_string();
        keyword_task.allowed.roles = vec![FangyuanPrimitiveRole::Archive];
        let keyword_context =
            build_catalog_context(&keyword_task, &prepared_task_sha256, &registry).unwrap();
        assert_eq!(
            keyword_context.selection.strategy,
            FangyuanAuthoringCatalogSelectionStrategy::MaterialKeyword
        );
        assert_eq!(
            keyword_context.selection.confidence,
            FangyuanAuthoringCatalogSelectionConfidence::High
        );
        assert!(
            keyword_context
                .presets
                .iter()
                .all(|preset| preset.material_family == "metal")
        );

        let mut role_task = fixture_task();
        role_task.prompt = "neutral object finish".to_string();
        role_task.allowed.roles = vec![FangyuanPrimitiveRole::Structure];
        let role_context =
            build_catalog_context(&role_task, &prepared_task_sha256, &registry).unwrap();
        assert_eq!(
            role_context.selection.strategy,
            FangyuanAuthoringCatalogSelectionStrategy::RoleAndPurpose
        );
        assert_eq!(
            role_context.selection.confidence,
            FangyuanAuthoringCatalogSelectionConfidence::Low
        );
        assert!((1..=3).contains(&role_context.presets.len()));

        let mut fallback_task = fixture_task();
        fallback_task.prompt = "neutral object finish".to_string();
        fallback_task.allowed.roles = vec![FangyuanPrimitiveRole::Archive];
        fallback_task.additional_forbidden_features.clear();
        let fallback_context =
            build_catalog_context(&fallback_task, &prepared_task_sha256, &registry).unwrap();
        assert_eq!(
            fallback_context.selection.strategy,
            FangyuanAuthoringCatalogSelectionStrategy::FullCatalogFallback
        );
        assert_eq!(fallback_context.presets.len(), 256);
    }

    #[test]
    fn catalog_context_excludes_task_forbidden_special_surfaces() {
        let registry = fixture_registry();
        let mut task = fixture_task();
        task.prompt = "neutral object finish".to_string();
        task.allowed.roles = vec![FangyuanPrimitiveRole::Archive];
        task.additional_forbidden_features = vec![
            FangyuanAuthoringAdditionalForbiddenFeature::SpecialSurface,
            FangyuanAuthoringAdditionalForbiddenFeature::WaterSurface,
            FangyuanAuthoringAdditionalForbiddenFeature::SkySurface,
            FangyuanAuthoringAdditionalForbiddenFeature::LavaSurface,
        ];
        let context = build_catalog_context(&task, &"c".repeat(64), &registry).unwrap();
        assert!(context.presets.len() < 256);
        assert!(context.presets.iter().all(|preset| {
            !matches!(
                preset.render_model,
                FangyuanCatalogRenderModel::SpecialWater
                    | FangyuanCatalogRenderModel::SpecialUnlitSky
            ) && !matches!(preset.material_family.as_str(), "water" | "sky" | "lava")
        }));
    }

    #[test]
    fn run_id_and_clock_contract_are_well_formed() {
        let timestamp = current_utc_rfc3339().unwrap();
        assert_eq!(timestamp.len(), 20);
        let compact = compact_utc_run_timestamp(&timestamp).unwrap();
        assert_eq!(compact.len(), 16);
        assert!(is_valid_run_id(&format!("fygen-v1-{compact}-012345abcdef")));
        assert!(!is_valid_run_id("fygen-v1-20260826T010203Z-upperCASE123"));
    }
}
