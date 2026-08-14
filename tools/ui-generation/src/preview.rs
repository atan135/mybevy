use crate::{
    host_contract::ResolvedGenerationHostContract,
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
};
use image::{ColorType, ImageDecoder, Limits, codecs::png::PngDecoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    fs::OpenOptions,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use ui_document_core::UiPageState;
use ui_document_core::{
    UI_DOCUMENT_MAX_BYTES, UiAssetSource, canonicalize_json, validate_json_bytes,
};

const MAX_PREVIEW_RESULT_BYTES: u64 = 64 * 1024;
const MAX_PREVIEW_LOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PREVIEW_SCREENSHOT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PREVIEW_DECODE_ALLOC: u64 = 64 * 1024 * 1024;
const MAX_PREVIEW_PIXELS: u64 = 4096 * 4096;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
pub const DEFAULT_PREVIEW_PREWARM_TIMEOUT_MS: u64 = 900_000;
pub const DEFAULT_PREVIEW_PROCESS_TIMEOUT_MS: u64 = 600_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewFailureKind {
    ConfigurationInvalid,
    ResourceMissing,
    ProcessUnavailable,
    ProcessTimeout,
    Cancelled,
    ProcessFailed,
    ResultMissing,
    ResultMalformed,
    ScreenshotMissing,
    EvidenceMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewFailure {
    pub kind: PreviewFailureKind,
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewCommandPlan {
    pub program: String,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub document_path: PathBuf,
    pub screenshot_path: PathBuf,
    pub result_path: PathBuf,
    pub log_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_registration_path: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub page_state: String,
    pub timeout_frames: u32,
    pub stable_frames: u32,
    pub prewarm_timeout_ms: u64,
    pub process_timeout_ms: u64,
    pub canonical_document_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_contract_version: Option<u32>,
    #[serde(skip_serializing)]
    pub approved_registration_json: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewExecutionOptions {
    pub prewarm_timeout_ms: u64,
    pub process_timeout_ms: u64,
}

impl Default for PreviewExecutionOptions {
    fn default() -> Self {
        Self {
            prewarm_timeout_ms: DEFAULT_PREVIEW_PREWARM_TIMEOUT_MS,
            process_timeout_ms: DEFAULT_PREVIEW_PROCESS_TIMEOUT_MS,
        }
    }
}

impl PreviewExecutionOptions {
    pub fn from_seconds(prewarm_seconds: u64, process_seconds: u64) -> Result<Self, TaskFailure> {
        let prewarm_timeout_ms = prewarm_seconds.checked_mul(1_000).ok_or_else(|| {
            TaskFailure::invalid("preview prewarm timeout seconds exceed the supported range")
        })?;
        let process_timeout_ms = process_seconds.checked_mul(1_000).ok_or_else(|| {
            TaskFailure::invalid("preview timeout seconds exceed the supported range")
        })?;
        if prewarm_timeout_ms == 0 || process_timeout_ms == 0 {
            return Err(TaskFailure::invalid(
                "preview prewarm and process timeouts must be greater than zero",
            ));
        }
        Ok(Self {
            prewarm_timeout_ms,
            process_timeout_ms,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewProcessRecord {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewRunStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRunResult {
    pub status: PreviewRunStatus,
    pub command: PreviewCommandPlan,
    pub process: PreviewProcessRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<PreviewFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreviewScreenshotEvidence {
    byte_length: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandalonePreviewResult {
    protocol_version: u32,
    status: StandalonePreviewStatus,
    document_id: String,
    canonical_document_sha256: String,
    width: u32,
    height: u32,
    page_state: String,
    elapsed_frames: u32,
    stable_frames: u32,
    screenshot_path: String,
    host_contract_version: Option<u32>,
    captured_size: Option<(u32, u32)>,
    failure: Option<StandalonePreviewFailure>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum StandalonePreviewStatus {
    Passed,
    Failed,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandalonePreviewFailure {
    kind: String,
    code: String,
    detail: String,
}

pub trait PreviewExecutor {
    fn execute(
        &self,
        command: &PreviewCommandPlan,
        cancellation: &CancellationToken,
    ) -> Result<PreviewProcessRecord, PreviewFailure>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CommandPreviewExecutor;

impl PreviewExecutor for CommandPreviewExecutor {
    fn execute(
        &self,
        plan: &PreviewCommandPlan,
        cancellation: &CancellationToken,
    ) -> Result<PreviewProcessRecord, PreviewFailure> {
        let prewarm_log_path = plan.log_path.with_file_name("preview-prewarm.log");
        let prewarm_arguments = prewarm_arguments(plan)?;
        let prewarm = run_bounded_command(
            &plan.program,
            &prewarm_arguments,
            &plan.working_directory,
            &prewarm_log_path,
            plan.prewarm_timeout_ms,
            cancellation,
            "prewarm",
        )?;
        if prewarm.cancelled {
            return Err(stage_failure(
                PreviewFailureKind::Cancelled,
                "UI_GENERATION_PREVIEW_PREWARM_CANCELLED",
                "standalone preview prewarm was cancelled",
                "prewarm",
                &prewarm_log_path,
            ));
        }
        if prewarm.timed_out {
            return Err(stage_failure(
                PreviewFailureKind::ProcessTimeout,
                "UI_GENERATION_PREVIEW_PREWARM_TIMEOUT",
                &format!(
                    "standalone preview prewarm exceeded {} ms",
                    plan.prewarm_timeout_ms
                ),
                "prewarm",
                &prewarm_log_path,
            ));
        }
        if prewarm.exit_code != Some(0) {
            return Err(stage_failure(
                PreviewFailureKind::ProcessFailed,
                "UI_GENERATION_PREVIEW_PREWARM_FAILED",
                &format!(
                    "standalone preview prewarm exited with code {:?}",
                    prewarm.exit_code
                ),
                "prewarm",
                &prewarm_log_path,
            ));
        }
        run_bounded_command(
            &plan.program,
            &plan.arguments,
            &plan.working_directory,
            &plan.log_path,
            plan.process_timeout_ms,
            cancellation,
            "preview",
        )
    }
}

fn prewarm_arguments(plan: &PreviewCommandPlan) -> Result<Vec<String>, PreviewFailure> {
    let separator = plan
        .arguments
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| {
            stage_failure(
                PreviewFailureKind::ConfigurationInvalid,
                "UI_GENERATION_PREVIEW_PREWARM_ARGUMENTS_INVALID",
                "standalone preview command has no cargo argument separator",
                "prewarm",
                &plan.log_path.with_file_name("preview-prewarm.log"),
            )
        })?;
    let mut arguments = plan.arguments[..separator].to_vec();
    let command = arguments
        .iter_mut()
        .find(|argument| argument.as_str() == "run")
        .ok_or_else(|| {
            stage_failure(
                PreviewFailureKind::ConfigurationInvalid,
                "UI_GENERATION_PREVIEW_PREWARM_ARGUMENTS_INVALID",
                "standalone preview command is not cargo run",
                "prewarm",
                &plan.log_path.with_file_name("preview-prewarm.log"),
            )
        })?;
    *command = "check".to_owned();
    if !arguments.iter().any(|argument| argument == "--locked") {
        arguments.insert(1, "--locked".to_owned());
    }
    Ok(arguments)
}

fn run_bounded_command(
    program: &str,
    arguments: &[String],
    working_directory: &Path,
    log_path: &Path,
    timeout_ms: u64,
    cancellation: &CancellationToken,
    stage: &str,
) -> Result<PreviewProcessRecord, PreviewFailure> {
    let parent = log_path.parent().ok_or_else(|| {
        stage_failure(
            PreviewFailureKind::ConfigurationInvalid,
            "UI_GENERATION_PREVIEW_LOG_PATH_INVALID",
            "preview log path has no parent",
            stage,
            log_path,
        )
    })?;
    fs::create_dir_all(parent).map_err(|_| {
        stage_failure(
            PreviewFailureKind::ConfigurationInvalid,
            "UI_GENERATION_PREVIEW_LOG_DIRECTORY_FAILED",
            "preview log directory could not be created",
            stage,
            log_path,
        )
    })?;
    let mut log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(log_path)
        .map_err(|_| {
            stage_failure(
                PreviewFailureKind::ConfigurationInvalid,
                "UI_GENERATION_PREVIEW_LOG_CONFLICT",
                "preview log must not overwrite existing evidence",
                stage,
                log_path,
            )
        })?;
    writeln!(log, "ui-generation standalone preview {stage} process")
        .and_then(|_| log.flush())
        .map_err(|_| {
            stage_failure(
                PreviewFailureKind::ProcessUnavailable,
                "UI_GENERATION_PREVIEW_LOG_WRITE_FAILED",
                "preview process log preamble could not be written",
                stage,
                log_path,
            )
        })?;
    let stderr = log.try_clone().map_err(|_| {
        stage_failure(
            PreviewFailureKind::ProcessUnavailable,
            "UI_GENERATION_PREVIEW_LOG_CLONE_FAILED",
            "preview process log handle could not be cloned",
            stage,
            log_path,
        )
    })?;
    let started = Instant::now();
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|_| {
            stage_failure(
                PreviewFailureKind::ProcessUnavailable,
                if stage == "prewarm" {
                    "UI_GENERATION_PREVIEW_PREWARM_PROCESS_UNAVAILABLE"
                } else {
                    "UI_GENERATION_PREVIEW_PROCESS_UNAVAILABLE"
                },
                "standalone preview process could not be started",
                stage,
                log_path,
            )
        })?;
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        if cancellation.is_requested() {
            terminate_process_tree(&mut child);
            return Ok(PreviewProcessRecord {
                exit_code: None,
                timed_out: false,
                cancelled: true,
                elapsed_ms: duration_ms(started.elapsed()),
            });
        }
        if started.elapsed() >= timeout {
            terminate_process_tree(&mut child);
            return Ok(PreviewProcessRecord {
                exit_code: None,
                timed_out: true,
                cancelled: false,
                elapsed_ms: duration_ms(started.elapsed()),
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(PreviewProcessRecord {
                    exit_code: status.code(),
                    timed_out: false,
                    cancelled: false,
                    elapsed_ms: duration_ms(started.elapsed()),
                });
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                terminate_process_tree(&mut child);
                return Err(stage_failure(
                    PreviewFailureKind::ProcessFailed,
                    if stage == "prewarm" {
                        "UI_GENERATION_PREVIEW_PREWARM_PROCESS_WAIT_FAILED"
                    } else {
                        "UI_GENERATION_PREVIEW_PROCESS_WAIT_FAILED"
                    },
                    "standalone preview process status could not be read",
                    stage,
                    log_path,
                ));
            }
        }
    }
}

/// `cargo run` can parent another cargo process and the preview executable. Killing only the
/// direct process leaves the renderer alive on Windows and can hold the inherited log handles
/// open indefinitely. Terminate the complete tree before returning timeout/cancellation evidence.
fn terminate_process_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_err() {
            let _ = child.kill();
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

pub fn prepare_preview_command(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    width: u32,
    height: u32,
) -> Result<PreviewCommandPlan, TaskFailure> {
    prepare_preview_command_for_state_with_host_contract(
        repository_root,
        document_path,
        output_directory,
        width,
        height,
        &UiPageState::initial(),
        None,
    )
}

pub fn prepare_preview_command_with_host_contract(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    width: u32,
    height: u32,
    host_contract: Option<&ResolvedGenerationHostContract>,
) -> Result<PreviewCommandPlan, TaskFailure> {
    prepare_preview_command_with_host_contract_and_options(
        repository_root,
        document_path,
        output_directory,
        width,
        height,
        host_contract,
        PreviewExecutionOptions::default(),
    )
}

pub fn prepare_preview_command_with_host_contract_and_options(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    width: u32,
    height: u32,
    host_contract: Option<&ResolvedGenerationHostContract>,
    options: PreviewExecutionOptions,
) -> Result<PreviewCommandPlan, TaskFailure> {
    prepare_preview_command_for_state_with_host_contract_and_options(
        repository_root,
        document_path,
        output_directory,
        width,
        height,
        &UiPageState::initial(),
        host_contract,
        options,
    )
}

/// Prepares the feature-gated standalone preview for one explicitly declared visible page state.
/// The state is parsed before the subprocess starts; the formal runtime remains the authority for
/// verifying that the state exists and can be merged with the requested target profile.
pub fn prepare_preview_command_for_state(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    width: u32,
    height: u32,
    page_state: &UiPageState,
) -> Result<PreviewCommandPlan, TaskFailure> {
    prepare_preview_command_for_state_with_host_contract_and_options(
        repository_root,
        document_path,
        output_directory,
        width,
        height,
        page_state,
        None,
        PreviewExecutionOptions::default(),
    )
}

pub fn prepare_preview_command_for_state_with_host_contract(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    width: u32,
    height: u32,
    page_state: &UiPageState,
    host_contract: Option<&ResolvedGenerationHostContract>,
) -> Result<PreviewCommandPlan, TaskFailure> {
    prepare_preview_command_for_state_with_host_contract_and_options(
        repository_root,
        document_path,
        output_directory,
        width,
        height,
        page_state,
        host_contract,
        PreviewExecutionOptions::default(),
    )
}

pub fn prepare_preview_command_for_state_with_host_contract_and_options(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    width: u32,
    height: u32,
    page_state: &UiPageState,
    host_contract: Option<&ResolvedGenerationHostContract>,
    options: PreviewExecutionOptions,
) -> Result<PreviewCommandPlan, TaskFailure> {
    if output_directory.exists() || output_directory.as_os_str().is_empty() {
        return Err(TaskFailure::new(
            TaskFailureKind::OutputDirectoryConflict,
            "preview output directory must not already exist",
            Some(output_directory.display().to_string()),
        ));
    }
    if !(64..=4096).contains(&width) || !(64..=4096).contains(&height) {
        return Err(TaskFailure::invalid(
            "preview dimensions must be in 64..=4096",
        ));
    }
    let repository_root = fs::canonicalize(repository_root)
        .map_err(|_| TaskFailure::invalid("preview repository root cannot be resolved"))?;
    let project_manifest = repository_root.join("project/Cargo.toml");
    if !project_manifest.is_file() {
        return Err(TaskFailure::invalid(
            "preview repository lacks project/Cargo.toml",
        ));
    }
    let document_path = fs::canonicalize(document_path)
        .map_err(|_| TaskFailure::invalid("preview document path cannot be resolved"))?;
    let canonical_document_sha256 = validate_preview_document(&repository_root, &document_path)?;
    if let Some(host_contract) = host_contract {
        let source = fs::read_to_string(&document_path)
            .map_err(|_| TaskFailure::invalid("preview host document cannot be read"))?;
        host_contract.validate_document_source(&source)?;
    }
    let output_directory = absolute_new_path(output_directory)?;
    validate_preview_output_location(&repository_root, &output_directory)?;
    let screenshot_path = output_directory.join("preview.png");
    let result_path = output_directory.join("preview-result.json");
    let log_path = output_directory.join("preview.log");
    let approved_registration_path = host_contract
        .as_ref()
        .map(|_| output_directory.join("approved-registration.v1.json"));
    let process_manifest = process_path(&project_manifest);
    let process_document = process_path(&document_path);
    let process_working_directory = process_path(&repository_root);
    let mut arguments = vec![
        "run".to_owned(),
        "--locked".to_owned(),
        "--quiet".to_owned(),
        "--manifest-path".to_owned(),
        process_manifest.to_string_lossy().into_owned(),
        "--features".to_owned(),
        "ui-document-preview-tool".to_owned(),
        "--bin".to_owned(),
        "ui-document-preview".to_owned(),
        "--".to_owned(),
        "--document".to_owned(),
        process_document.to_string_lossy().into_owned(),
        "--screenshot".to_owned(),
        screenshot_path.to_string_lossy().into_owned(),
        "--result".to_owned(),
        result_path.to_string_lossy().into_owned(),
        "--width".to_owned(),
        width.to_string(),
        "--height".to_owned(),
        height.to_string(),
        "--page-state".to_owned(),
        page_state.to_string(),
        "--timeout-frames".to_owned(),
        "1200".to_owned(),
        "--stable-frames".to_owned(),
        "30".to_owned(),
    ];
    if let Some(path) = &approved_registration_path {
        arguments.extend([
            "--approved-registration".to_owned(),
            path.to_string_lossy().into_owned(),
        ]);
    }
    Ok(PreviewCommandPlan {
        program: "cargo".to_owned(),
        arguments,
        working_directory: process_working_directory,
        document_path: process_document,
        screenshot_path,
        result_path,
        log_path,
        approved_registration_path,
        width,
        height,
        page_state: page_state.to_string(),
        timeout_frames: 1200,
        stable_frames: 30,
        prewarm_timeout_ms: options.prewarm_timeout_ms,
        process_timeout_ms: options.process_timeout_ms,
        canonical_document_sha256,
        host_contract_version: host_contract.map(|contract| contract.version),
        approved_registration_json: host_contract
            .map(ResolvedGenerationHostContract::preview_registration_json)
            .transpose()?,
    })
}

pub fn run_preview(
    plan: PreviewCommandPlan,
    executor: &dyn PreviewExecutor,
    cancellation: &CancellationToken,
) -> PreviewRunResult {
    if let Err(error) = fs::create_dir(
        &plan
            .screenshot_path
            .parent()
            .expect("planned parent exists"),
    ) {
        return failed_preview(
            plan,
            PreviewProcessRecord {
                exit_code: None,
                timed_out: false,
                cancelled: false,
                elapsed_ms: 0,
            },
            preview_failure(
                PreviewFailureKind::ConfigurationInvalid,
                "UI_GENERATION_PREVIEW_OUTPUT_CREATE_FAILED",
                &format!("preview output directory could not be created: {error}"),
            ),
        );
    }
    match (
        plan.approved_registration_path.as_ref(),
        plan.approved_registration_json.as_ref(),
    ) {
        (Some(path), Some(source)) => {
            if let Err(error) = write_new_preview_registration(path, source.as_bytes()) {
                return failed_preview(
                    plan,
                    PreviewProcessRecord {
                        exit_code: None,
                        timed_out: false,
                        cancelled: false,
                        elapsed_ms: 0,
                    },
                    preview_failure(
                        PreviewFailureKind::ConfigurationInvalid,
                        "UI_GENERATION_PREVIEW_APPROVED_REGISTRATION_WRITE_FAILED",
                        &format!("approved preview registration could not be written: {error}"),
                    ),
                );
            }
        }
        (None, None) => {}
        _ => {
            return failed_preview(
                plan,
                PreviewProcessRecord {
                    exit_code: None,
                    timed_out: false,
                    cancelled: false,
                    elapsed_ms: 0,
                },
                preview_failure(
                    PreviewFailureKind::ConfigurationInvalid,
                    "UI_GENERATION_PREVIEW_APPROVED_REGISTRATION_INCOMPLETE",
                    "approved preview registration path and source must be supplied together",
                ),
            );
        }
    }
    let process = match executor.execute(&plan, cancellation) {
        Ok(process) => process,
        Err(failure) => {
            return failed_preview(
                plan,
                PreviewProcessRecord {
                    exit_code: None,
                    timed_out: false,
                    cancelled: false,
                    elapsed_ms: 0,
                },
                failure,
            );
        }
    };
    let screenshot = match validate_preview_evidence(&plan, &process, None) {
        Ok(evidence) => evidence,
        Err(failure) => {
            let failure = with_preview_log_path(failure, &plan.log_path);
            return failed_preview(plan, process, failure);
        }
    };
    PreviewRunResult {
        status: PreviewRunStatus::Passed,
        command: plan,
        process,
        screenshot_sha256: Some(screenshot.sha256),
        screenshot_bytes: Some(screenshot.byte_length),
        failure: None,
    }
}

fn validate_preview_document(
    repository_root: &Path,
    document_path: &Path,
) -> Result<String, TaskFailure> {
    let metadata = fs::metadata(document_path)
        .map_err(|_| TaskFailure::invalid("preview document metadata is unavailable"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > UI_DOCUMENT_MAX_BYTES as u64 {
        return Err(TaskFailure::invalid(
            "preview document must be a bounded nonempty regular file",
        ));
    }
    let bytes = fs::read(document_path)
        .map_err(|_| TaskFailure::invalid("preview document cannot be read"))?;
    let validation = validate_json_bytes(&bytes);
    let validated = validation.validated().ok_or_else(|| {
        let detail = validation
            .report
            .diagnostics
            .first()
            .map(|diagnostic| format!("{} at {}", diagnostic.code, diagnostic.document_path))
            .unwrap_or_else(|| "diagnostic unavailable".to_owned());
        TaskFailure::invalid(format!(
            "preview document failed formal validation: {detail}"
        ))
    })?;
    let assets_root = fs::canonicalize(repository_root.join("project/assets"))
        .map_err(|_| TaskFailure::invalid("project assets root cannot be resolved"))?;
    for entry in validated.document().assets.values() {
        match &entry.source {
            UiAssetSource::Packaged { path } => {
                let candidate = fs::canonicalize(assets_root.join(path)).map_err(|_| {
                    TaskFailure::new(
                        TaskFailureKind::ImageUnreadable,
                        "declared packaged preview resource is missing",
                        Some(path.clone()),
                    )
                })?;
                if candidate == assets_root
                    || !candidate.starts_with(&assets_root)
                    || !candidate.is_file()
                {
                    return Err(TaskFailure::new(
                        TaskFailureKind::UnsafeOutputPath,
                        "declared preview resource is outside project assets",
                        Some(path.clone()),
                    ));
                }
            }
            UiAssetSource::ContentCache { logical_id } => {
                return Err(TaskFailure::new(
                    TaskFailureKind::ImageUnreadable,
                    "standalone preview cannot resolve content-cache assets",
                    Some(logical_id.clone()),
                ));
            }
            UiAssetSource::BuiltInMaterial { .. } => {}
        }
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| TaskFailure::invalid("preview document is not UTF-8"))?;
    let canonical = canonicalize_json(source)
        .map_err(|_| TaskFailure::invalid("preview document cannot be canonicalized"))?;
    Ok(format!("{:x}", Sha256::digest(canonical.as_bytes())))
}

pub(crate) fn validate_passed_preview_evidence(
    preview: &PreviewRunResult,
) -> Result<(), PreviewFailure> {
    validate_passed_preview_evidence_at(
        preview,
        &preview.command.result_path,
        &preview.command.screenshot_path,
        &preview.command.log_path,
    )
}

pub(crate) fn validate_passed_preview_evidence_at(
    preview: &PreviewRunResult,
    result_path: &Path,
    screenshot_path: &Path,
    log_path: &Path,
) -> Result<(), PreviewFailure> {
    let declared = match (
        preview.screenshot_sha256.as_deref(),
        preview.screenshot_bytes,
    ) {
        (Some(sha256), Some(byte_length))
            if is_sha256(sha256) && byte_length > 0 && preview.failure.is_none() =>
        {
            (sha256, byte_length)
        }
        _ => {
            return Err(preview_failure(
                PreviewFailureKind::EvidenceMismatch,
                "UI_GENERATION_PREVIEW_PASSED_EVIDENCE_INVALID",
                "passed preview has incomplete screenshot or failure evidence",
            ));
        }
    };
    if preview.status != PreviewRunStatus::Passed {
        return Err(preview_failure(
            PreviewFailureKind::EvidenceMismatch,
            "UI_GENERATION_PREVIEW_STATUS_INVALID",
            "only a passed preview can be revalidated as passed evidence",
        ));
    }
    validate_preview_evidence_at(
        &preview.command,
        &preview.process,
        Some(declared),
        result_path,
        screenshot_path,
        log_path,
    )
    .map(|_| ())
}

fn validate_preview_evidence(
    plan: &PreviewCommandPlan,
    process: &PreviewProcessRecord,
    declared_screenshot: Option<(&str, u64)>,
) -> Result<PreviewScreenshotEvidence, PreviewFailure> {
    validate_preview_evidence_at(
        plan,
        process,
        declared_screenshot,
        &plan.result_path,
        &plan.screenshot_path,
        &plan.log_path,
    )
}

fn validate_preview_evidence_at(
    plan: &PreviewCommandPlan,
    process: &PreviewProcessRecord,
    declared_screenshot: Option<(&str, u64)>,
    result_path: &Path,
    screenshot_path: &Path,
    log_path: &Path,
) -> Result<PreviewScreenshotEvidence, PreviewFailure> {
    if process.cancelled {
        return Err(preview_failure(
            PreviewFailureKind::Cancelled,
            "UI_GENERATION_PREVIEW_CANCELLED",
            "standalone preview was cancelled",
        ));
    }
    if process.timed_out {
        return Err(preview_failure(
            PreviewFailureKind::ProcessTimeout,
            "UI_GENERATION_PREVIEW_PROCESS_TIMEOUT",
            "standalone preview exceeded the process timeout",
        ));
    }
    if !(64..=4096).contains(&plan.width)
        || !(64..=4096).contains(&plan.height)
        || u64::from(plan.width) * u64::from(plan.height) > MAX_PREVIEW_PIXELS
    {
        return Err(preview_failure(
            PreviewFailureKind::ConfigurationInvalid,
            "UI_GENERATION_PREVIEW_DIMENSIONS_UNSAFE",
            "preview dimensions exceed the closed pixel budget",
        ));
    }
    validate_log(log_path)?;
    let result =
        read_bounded_json::<StandalonePreviewResult>(result_path, MAX_PREVIEW_RESULT_BYTES)?;
    if result.protocol_version != 1
        || result.document_id.trim().is_empty()
        || result.width != plan.width
        || result.height != plan.height
        || result.page_state != plan.page_state
        || result.canonical_document_sha256 != plan.canonical_document_sha256
        || result.host_contract_version != plan.host_contract_version
        || result.screenshot_path != plan.screenshot_path.to_string_lossy()
        || result.elapsed_frames > plan.timeout_frames
        || result.stable_frames > result.elapsed_frames
    {
        return Err(preview_failure(
            PreviewFailureKind::EvidenceMismatch,
            "UI_GENERATION_PREVIEW_RESULT_MISMATCH",
            "standalone preview result does not match its command evidence",
        ));
    }
    if result.status == StandalonePreviewStatus::Failed {
        let (code, detail) = result.failure.map_or_else(
            || {
                (
                    "UI_GENERATION_PREVIEW_PROCESS_FAILED".to_owned(),
                    "standalone preview reported failure without detail".to_owned(),
                )
            },
            |failure| {
                let _ = failure.kind;
                (failure.code, failure.detail)
            },
        );
        return Err(preview_failure(
            PreviewFailureKind::ProcessFailed,
            &code,
            &detail,
        ));
    }
    if result.failure.is_some()
        || process.exit_code != Some(0)
        || result.captured_size != Some((plan.width, plan.height))
    {
        return Err(preview_failure(
            PreviewFailureKind::ProcessFailed,
            "UI_GENERATION_PREVIEW_PROCESS_FAILED",
            "standalone preview process, result, or captured size indicates failure",
        ));
    }
    let screenshot = read_screenshot(screenshot_path, plan.width, plan.height)?;
    if let Some((sha256, byte_length)) = declared_screenshot
        && (screenshot.sha256 != sha256 || screenshot.byte_length != byte_length)
    {
        return Err(preview_failure(
            PreviewFailureKind::EvidenceMismatch,
            "UI_GENERATION_PREVIEW_SCREENSHOT_EVIDENCE_MISMATCH",
            "passed preview screenshot bytes differ from the declared hash or length",
        ));
    }
    Ok(screenshot)
}

fn write_new_preview_registration(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

fn read_screenshot(
    path: &Path,
    expected_width: u32,
    expected_height: u32,
) -> Result<PreviewScreenshotEvidence, PreviewFailure> {
    let metadata = fs::metadata(path).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_MISSING",
            "standalone preview screenshot is missing",
        )
    })?;
    if !metadata.is_file()
        || metadata.len() < PNG_SIGNATURE.len() as u64
        || metadata.len() > MAX_PREVIEW_SCREENSHOT_BYTES
    {
        return Err(preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_SIZE_INVALID",
            "standalone preview screenshot size is invalid",
        ));
    }
    let bytes = fs::read(path).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_UNREADABLE",
            "standalone preview screenshot cannot be read",
        )
    })?;
    if bytes.len() as u64 != metadata.len() || !bytes.starts_with(PNG_SIGNATURE) {
        return Err(preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_FORMAT_INVALID",
            "standalone preview screenshot is not PNG",
        ));
    }
    decode_static_png(&bytes, expected_width, expected_height)?;
    Ok(PreviewScreenshotEvidence {
        byte_length: metadata.len(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

fn decode_static_png(
    bytes: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> Result<(), PreviewFailure> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(MAX_PREVIEW_DECODE_ALLOC);
    let decoder = PngDecoder::with_limits(Cursor::new(bytes), limits).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_DECODE_INVALID",
            "standalone preview screenshot PNG header is invalid or over budget",
        )
    })?;
    if decoder.is_apng().map_err(|_| {
        preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_DECODE_INVALID",
            "standalone preview screenshot animation metadata is invalid",
        )
    })? {
        return Err(preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_ANIMATION_UNSUPPORTED",
            "standalone preview screenshot must be a static PNG",
        ));
    }
    let (width, height) = decoder.dimensions();
    if width != expected_width || height != expected_height {
        return Err(preview_failure(
            PreviewFailureKind::EvidenceMismatch,
            "UI_GENERATION_PREVIEW_SCREENSHOT_DIMENSION_MISMATCH",
            "decoded preview screenshot dimensions differ from the requested viewport",
        ));
    }
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count == 0 || pixel_count > MAX_PREVIEW_PIXELS {
        return Err(preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_PIXELS_UNSAFE",
            "preview screenshot exceeds the closed pixel budget",
        ));
    }
    if !matches!(
        decoder.color_type(),
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8
    ) {
        return Err(preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_COLOR_UNSUPPORTED",
            "preview screenshot must use a supported 8-bit PNG color type",
        ));
    }
    let decoded_bytes = decoder.total_bytes();
    if decoded_bytes == 0 || decoded_bytes > MAX_PREVIEW_DECODE_ALLOC {
        return Err(preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_ALLOCATION_UNSAFE",
            "preview screenshot decoded allocation exceeds its budget",
        ));
    }
    let decoded_length = usize::try_from(decoded_bytes).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_ALLOCATION_UNSAFE",
            "preview screenshot decoded allocation cannot be represented",
        )
    })?;
    let mut decoded = vec![0; decoded_length];
    decoder.read_image(&mut decoded).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_DECODE_INVALID",
            "standalone preview screenshot PNG pixels are truncated or corrupt",
        )
    })
}

fn validate_log(path: &Path) -> Result<(), PreviewFailure> {
    let metadata = fs::metadata(path).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ResultMissing,
            "UI_GENERATION_PREVIEW_LOG_MISSING",
            "standalone preview log is missing",
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_PREVIEW_LOG_BYTES {
        return Err(preview_failure(
            PreviewFailureKind::ResultMalformed,
            "UI_GENERATION_PREVIEW_LOG_SIZE_INVALID",
            "standalone preview log size is outside the evidence budget",
        ));
    }
    Ok(())
}

fn read_bounded_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    maximum_bytes: u64,
) -> Result<T, PreviewFailure> {
    let metadata = fs::metadata(path).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ResultMissing,
            "UI_GENERATION_PREVIEW_RESULT_MISSING",
            "standalone preview result is missing",
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(preview_failure(
            PreviewFailureKind::ResultMalformed,
            "UI_GENERATION_PREVIEW_RESULT_SIZE_INVALID",
            "standalone preview result size is invalid",
        ));
    }
    let bytes = fs::read(path).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ResultMissing,
            "UI_GENERATION_PREVIEW_RESULT_UNREADABLE",
            "standalone preview result cannot be read",
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|_| {
        preview_failure(
            PreviewFailureKind::ResultMalformed,
            "UI_GENERATION_PREVIEW_RESULT_MALFORMED",
            "standalone preview result is not the strict result contract",
        )
    })
}

fn failed_preview(
    plan: PreviewCommandPlan,
    process: PreviewProcessRecord,
    failure: PreviewFailure,
) -> PreviewRunResult {
    PreviewRunResult {
        status: PreviewRunStatus::Failed,
        command: plan,
        process,
        screenshot_sha256: None,
        screenshot_bytes: None,
        failure: Some(failure),
    }
}

fn preview_failure(
    kind: PreviewFailureKind,
    code: impl Into<String>,
    detail: impl Into<String>,
) -> PreviewFailure {
    PreviewFailure {
        kind,
        code: code.into(),
        detail: detail.into(),
    }
}

fn stage_failure(
    kind: PreviewFailureKind,
    code: impl Into<String>,
    detail: &str,
    stage: &str,
    log_path: &Path,
) -> PreviewFailure {
    preview_failure(
        kind,
        code,
        format!("{detail} during {stage} stage; see {}", log_path.display()),
    )
}

fn with_preview_log_path(mut failure: PreviewFailure, log_path: &Path) -> PreviewFailure {
    let log_path = log_path.display().to_string();
    if !failure.detail.contains(&log_path) {
        failure.detail = format!("{} during preview stage; see {log_path}", failure.detail);
    }
    failure
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn absolute_new_path(path: &Path) -> Result<PathBuf, TaskFailure> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|_| TaskFailure::invalid("current directory cannot be resolved"))
    }
}

fn validate_preview_output_location(
    repository_root: &Path,
    output_directory: &Path,
) -> Result<(), TaskFailure> {
    if output_directory.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "preview output path must be lexically normalized",
            Some(output_directory.display().to_string()),
        ));
    }
    let repository_root = process_path(repository_root);
    let output_directory = process_path(output_directory);
    let allowed_repository_root = repository_root.join("summary/ui-generation");
    let lexical_inside_repository = output_directory.starts_with(&repository_root);
    if lexical_inside_repository && !output_directory.starts_with(&allowed_repository_root) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "preview output inside the repository is restricted to summary/ui-generation",
            Some(output_directory.display().to_string()),
        ));
    }
    let mut ancestor = output_directory.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "preview output has no resolvable ancestor",
                Some(output_directory.display().to_string()),
            )
        })?;
    }
    let ancestor = process_path(&fs::canonicalize(ancestor).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "preview output ancestor cannot be resolved",
            Some(output_directory.display().to_string()),
        )
    })?);
    if ancestor.starts_with(&repository_root)
        && !output_directory.starts_with(&allowed_repository_root)
    {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "preview output ancestor enters a protected repository directory",
            Some(output_directory.display().to_string()),
        ));
    }
    Ok(())
}

fn process_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(value) = value.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{value}"));
        }
        if let Some(value) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(value);
        }
    }
    path.to_path_buf()
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    use std::sync::Mutex;

    struct FixtureExecutor {
        mode: FixtureMode,
        structured_inputs_seen: Mutex<bool>,
    }

    enum FixtureMode {
        Success,
        MissingScreenshot,
        TruncatedScreenshot,
        WrongSizeScreenshot,
        Timeout,
    }

    impl PreviewExecutor for FixtureExecutor {
        fn execute(
            &self,
            plan: &PreviewCommandPlan,
            _cancellation: &CancellationToken,
        ) -> Result<PreviewProcessRecord, PreviewFailure> {
            *self.structured_inputs_seen.lock().unwrap() = plan
                .arguments
                .iter()
                .any(|argument| argument == "ui-document-preview");
            if matches!(self.mode, FixtureMode::Timeout) {
                return Ok(PreviewProcessRecord {
                    exit_code: None,
                    timed_out: true,
                    cancelled: false,
                    elapsed_ms: plan.process_timeout_ms,
                });
            }
            fs::write(&plan.log_path, b"fixture standalone process log").unwrap();
            match self.mode {
                FixtureMode::MissingScreenshot => {}
                FixtureMode::TruncatedScreenshot => {
                    fs::write(&plan.screenshot_path, [PNG_SIGNATURE, b"fixture"].concat()).unwrap();
                }
                FixtureMode::WrongSizeScreenshot => {
                    write_fixture_png(&plan.screenshot_path, plan.width + 1, plan.height);
                }
                FixtureMode::Success => {
                    write_fixture_png(&plan.screenshot_path, plan.width, plan.height);
                }
                FixtureMode::Timeout => unreachable!("timeout returns before writing evidence"),
            }
            let result = serde_json::json!({
                "protocol_version": 1,
                "status": "passed",
                "document_id": "generated.minimal_fixture",
                "canonical_document_sha256": plan.canonical_document_sha256,
                "width": plan.width,
                "height": plan.height,
                "page_state": plan.page_state,
                "elapsed_frames": 60,
                "stable_frames": 30,
                "screenshot_path": plan.screenshot_path.to_string_lossy(),
                "host_contract_version": plan.host_contract_version,
                "captured_size": [plan.width, plan.height]
            });
            fs::write(&plan.result_path, serde_json::to_vec(&result).unwrap()).unwrap();
            Ok(PreviewProcessRecord {
                exit_code: Some(0),
                timed_out: false,
                cancelled: false,
                elapsed_ms: 10,
            })
        }
    }

    fn write_fixture_png(path: &Path, width: u32, height: u32) {
        fs::write(path, fixture_png_bytes(width, height)).unwrap();
    }

    fn fixture_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let pixels = vec![0x7f_u8; (u64::from(width) * u64::from(height) * 4) as usize];
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    fn animated_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = fixture_png_bytes(width, height);
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&8_u32.to_be_bytes());
        chunk.extend_from_slice(b"acTL");
        chunk.extend_from_slice(&1_u32.to_be_bytes());
        chunk.extend_from_slice(&0_u32.to_be_bytes());
        let crc = png_crc32(&chunk[4..]);
        chunk.extend_from_slice(&crc.to_be_bytes());
        bytes.splice(33..33, chunk);
        bytes
    }

    fn png_crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn fixture_document() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/generation/minimal.valid.json")
    }

    fn bare_document(directory: &Path) -> PathBuf {
        let envelope: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture_document()).unwrap()).unwrap();
        let path = directory.join("document.json");
        fs::write(&path, serde_json::to_vec(&envelope["document"]).unwrap()).unwrap();
        path
    }

    #[test]
    fn command_uses_explicit_feature_gated_standalone_process() {
        let directory = tempfile::tempdir().unwrap();
        let document = bare_document(directory.path());
        let plan = prepare_preview_command(
            &repository_root(),
            &document,
            &directory.path().join("preview"),
            390,
            844,
        )
        .unwrap();
        assert!(
            plan.arguments
                .windows(2)
                .any(|pair| { pair == ["--features", "ui-document-preview-tool"] })
        );
        assert!(
            plan.arguments
                .windows(2)
                .any(|pair| { pair == ["--bin", "ui-document-preview"] })
        );
        assert!(plan.arguments.iter().any(|argument| argument == "--locked"));
        assert_eq!(plan.prewarm_timeout_ms, DEFAULT_PREVIEW_PREWARM_TIMEOUT_MS);
        assert_eq!(plan.process_timeout_ms, DEFAULT_PREVIEW_PROCESS_TIMEOUT_MS);
        assert!(
            !plan
                .arguments
                .iter()
                .any(|argument| argument.starts_with(r"\\?\"))
        );
        assert!(plan.result_path.ends_with("preview-result.json"));
    }

    #[test]
    fn preview_execution_options_parameterize_both_process_timeouts() {
        let options = PreviewExecutionOptions::from_seconds(17, 29).unwrap();
        assert_eq!(options.prewarm_timeout_ms, 17_000);
        assert_eq!(options.process_timeout_ms, 29_000);
        assert!(PreviewExecutionOptions::from_seconds(0, 1).is_err());
        assert!(PreviewExecutionOptions::from_seconds(u64::MAX, 1).is_err());
    }

    #[test]
    fn fixture_process_produces_bounded_linked_screenshot_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let document = bare_document(directory.path());
        let plan = prepare_preview_command(
            &repository_root(),
            &document,
            &directory.path().join("preview"),
            390,
            844,
        )
        .unwrap();
        let executor = FixtureExecutor {
            mode: FixtureMode::Success,
            structured_inputs_seen: Mutex::new(false),
        };
        let result = run_preview(plan, &executor, &CancellationToken::default());
        assert_eq!(result.status, PreviewRunStatus::Passed);
        assert!(result.screenshot_sha256.is_some());
        assert!(*executor.structured_inputs_seen.lock().unwrap());
    }

    #[test]
    fn host_aware_preview_writes_production_registration_before_process() {
        use crate::{
            contract::{
                GenerationForbiddenCapability, GenerationHostBindingRequest,
                GenerationHostContractRequest,
            },
            host_contract::{
                REQUIRED_AUDIT_PROFILES, TRUSTED_RESOURCE_CATALOG_PATH,
                resolve_repository_host_contract,
            },
        };

        let host = resolve_repository_host_contract(
            &repository_root(),
            &GenerationHostContractRequest {
                document_id: "approved.business_acceptance".to_owned(),
                owner: "approved_business_acceptance".to_owned(),
                route: "ui_approved_business_acceptance".to_owned(),
                version: 1,
                allowed_actions: vec!["approved.acceptance_continue".to_owned()],
                allowed_bindings: vec![GenerationHostBindingRequest {
                    scope: "owner".to_owned(),
                    path: "acceptance.status".to_owned(),
                    value_type: serde_json::json!({"kind": "string"}),
                }],
                target_profiles: REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec(),
                resource_catalog: TRUSTED_RESOURCE_CATALOG_PATH.to_owned(),
                forbidden_capabilities: vec![
                    GenerationForbiddenCapability::RustCode,
                    GenerationForbiddenCapability::Scripts,
                    GenerationForbiddenCapability::ArbitraryActions,
                    GenerationForbiddenCapability::ArbitraryBindings,
                    GenerationForbiddenCapability::ArbitraryResources,
                    GenerationForbiddenCapability::BusinessProtocol,
                    GenerationForbiddenCapability::CargoConfiguration,
                    GenerationForbiddenCapability::AndroidConfiguration,
                ],
            },
        )
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let document = repository_root().join(
            "project/assets/ui/documents/approved/business_acceptance_fixture/document.v1.json",
        );
        let plan = prepare_preview_command_with_host_contract(
            &repository_root(),
            &document,
            &directory.path().join("preview"),
            390,
            844,
            Some(&host),
        )
        .unwrap();
        assert_eq!(plan.host_contract_version, Some(1));
        assert!(
            plan.arguments
                .windows(2)
                .any(|pair| pair[0] == "--approved-registration")
        );

        let result = run_preview(
            plan,
            &FixtureExecutor {
                mode: FixtureMode::Success,
                structured_inputs_seen: Mutex::new(false),
            },
            &CancellationToken::default(),
        );
        assert_eq!(result.status, PreviewRunStatus::Passed);
        let registration_path = result.command.approved_registration_path.unwrap();
        let registration = fs::read_to_string(registration_path).unwrap();
        let parsed =
            ui_document_core::parse_approved_document_registration(registration.trim()).unwrap();
        assert_eq!(
            parsed.document_id().as_str(),
            "approved.business_acceptance"
        );
        assert_eq!(parsed.host_contract().unwrap().version(), 1);
    }

    #[test]
    fn resource_missing_screenshot_and_timeout_are_stable() {
        let directory = tempfile::tempdir().unwrap();
        let document = bare_document(directory.path());
        let missing_plan = prepare_preview_command(
            &repository_root(),
            &document,
            &directory.path().join("missing"),
            390,
            844,
        )
        .unwrap();
        let missing = run_preview(
            missing_plan,
            &FixtureExecutor {
                mode: FixtureMode::MissingScreenshot,
                structured_inputs_seen: Mutex::new(false),
            },
            &CancellationToken::default(),
        );
        assert_eq!(
            missing.failure.unwrap().kind,
            PreviewFailureKind::ScreenshotMissing
        );

        let timeout_plan = prepare_preview_command(
            &repository_root(),
            &document,
            &directory.path().join("timeout"),
            390,
            844,
        )
        .unwrap();
        let timeout = run_preview(
            timeout_plan,
            &FixtureExecutor {
                mode: FixtureMode::Timeout,
                structured_inputs_seen: Mutex::new(false),
            },
            &CancellationToken::default(),
        );
        assert_eq!(
            timeout.failure.unwrap().kind,
            PreviewFailureKind::ProcessTimeout
        );
    }

    #[test]
    fn command_prewarm_failure_reports_stage_and_log_path() {
        let directory = tempfile::tempdir().unwrap();
        let document = bare_document(directory.path());
        let mut plan = prepare_preview_command(
            &repository_root(),
            &document,
            &directory.path().join("prewarm-failure"),
            390,
            844,
        )
        .unwrap();
        plan.program = "ui-generation-preview-command-does-not-exist".to_owned();
        let result = run_preview(plan, &CommandPreviewExecutor, &CancellationToken::default());
        let failure = result.failure.unwrap();
        assert_eq!(
            failure.code,
            "UI_GENERATION_PREVIEW_PREWARM_PROCESS_UNAVAILABLE"
        );
        assert!(failure.detail.contains("prewarm"));
        assert!(failure.detail.contains("preview-prewarm.log"));
    }

    #[test]
    fn truncated_and_wrong_size_png_are_rejected_after_pixel_decode() {
        let directory = tempfile::tempdir().unwrap();
        let document = bare_document(directory.path());
        for (name, mode, expected_code) in [
            (
                "truncated",
                FixtureMode::TruncatedScreenshot,
                "UI_GENERATION_PREVIEW_SCREENSHOT_DECODE_INVALID",
            ),
            (
                "wrong-size",
                FixtureMode::WrongSizeScreenshot,
                "UI_GENERATION_PREVIEW_SCREENSHOT_DIMENSION_MISMATCH",
            ),
        ] {
            let plan = prepare_preview_command(
                &repository_root(),
                &document,
                &directory.path().join(name),
                390,
                844,
            )
            .unwrap();
            let result = run_preview(
                plan,
                &FixtureExecutor {
                    mode,
                    structured_inputs_seen: Mutex::new(false),
                },
                &CancellationToken::default(),
            );
            assert_eq!(result.status, PreviewRunStatus::Failed);
            assert_eq!(result.failure.unwrap().code, expected_code);
        }
    }

    #[test]
    fn animated_and_unsupported_16_bit_png_are_rejected() {
        let animated = animated_png_bytes(64, 64);
        assert_eq!(
            decode_static_png(&animated, 64, 64).unwrap_err().code,
            "UI_GENERATION_PREVIEW_SCREENSHOT_ANIMATION_UNSUPPORTED"
        );

        let pixels = vec![0_u8; 64 * 64 * 8];
        let mut sixteen_bit = Vec::new();
        PngEncoder::new(&mut sixteen_bit)
            .write_image(&pixels, 64, 64, ExtendedColorType::Rgba16)
            .unwrap();
        assert_eq!(
            decode_static_png(&sixteen_bit, 64, 64).unwrap_err().code,
            "UI_GENERATION_PREVIEW_SCREENSHOT_COLOR_UNSUPPORTED"
        );
    }

    #[test]
    fn declared_missing_resource_stops_before_process() {
        let directory = tempfile::tempdir().unwrap();
        let source = serde_json::json!({
            "schema_version": 1,
            "document_id": "generated.missing_resource",
            "assets": {
                "missing": {
                    "kind": "image",
                    "source": {"kind": "packaged", "path": "ui/missing-stage8.png"}
                }
            },
            "root": {
                "type": "image",
                "id": "page.image",
                "asset": "missing",
                "layout": {"width": {"px": 64}, "height": {"px": 64}}
            }
        });
        let path = directory.path().join("missing.json");
        fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();
        let failure = prepare_preview_command(
            &repository_root(),
            &path,
            &directory.path().join("preview"),
            390,
            844,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageUnreadable);
    }

    #[test]
    fn preview_output_cannot_enter_project_sources_or_assets() {
        let directory = tempfile::tempdir().unwrap();
        let document = bare_document(directory.path());
        for protected in [
            "project/src/stage8-preview",
            "project/assets/stage8-preview",
        ] {
            let failure = prepare_preview_command(
                &repository_root(),
                &document,
                &repository_root().join(protected),
                390,
                844,
            )
            .unwrap_err();
            assert_eq!(failure.kind(), TaskFailureKind::UnsafeOutputPath);
        }
    }

    #[test]
    fn evidence_reader_rejects_oversized_logs_contractually() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("preview.log");
        let mut file = fs::File::create(&log).unwrap();
        file.write_all(&vec![b'x'; MAX_PREVIEW_LOG_BYTES as usize + 1])
            .unwrap();
        let metadata = fs::metadata(log).unwrap();
        assert!(metadata.len() > MAX_PREVIEW_LOG_BYTES);
    }
}
