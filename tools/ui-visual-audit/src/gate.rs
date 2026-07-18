use crate::{
    AI_ANALYSIS_ALGORITHM_VERSION, AI_ANALYSIS_REPORT_SCHEMA_VERSION, AiAnalysisReport,
    AiAnalysisStatus, AiProviderIssue, AiSeverity, ArtifactReport, ComparisonError,
    ComparisonErrorCode, DIFF_METRICS_ALGORITHM_VERSION, DIFF_METRICS_REPORT_SCHEMA_VERSION,
    DiffAnalysisReport, DiffAnalysisStatus, PixelSize, REGION_AUDIT_ALGORITHM_VERSION,
    REGION_AUDIT_REPORT_SCHEMA_VERSION, ReferenceBinding, RegionAuditReport, RegionLevel,
    RegionLocalStatus, SEMANTIC_AUDIT_ALGORITHM_VERSION, SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION,
    SemanticAuditReport, SemanticAuditStatus, SemanticFinding,
    comparison::{
        create_output_directory, resolve_allowed_input_roots, resolve_allowed_root,
        resolve_input_file,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const VISUAL_GATE_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const VISUAL_GATE_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const VISUAL_GATE_REPORT_SCHEMA_VERSION: u32 = 1;
pub const VISUAL_GATE_ALGORITHM_VERSION: &str = "ui_visual_gate_v1";
pub const VISUAL_GATE_REPORT_FILENAME: &str = "visual-gate-report.json";
pub const VISUAL_GATE_PEAK_MEMORY_BUDGET_BYTES: u64 = 320 * 1024 * 1024;

const MAX_GATE_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_GATE_BUNDLE_BYTES: u64 = 256 * 1024;
const MAX_GATE_REPORT_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_GATE_TOTAL_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GATE_CAPTURES: usize = 32;
const MAX_GATE_PROFILES: usize = 128;
const MAX_GATE_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Debug)]
pub struct GateRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub bundle: PathBuf,
    pub config: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateState {
    Passed,
    NeedsReview,
    Failed,
    Invalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateFailureType {
    None,
    InvalidEvidence,
    DimensionMismatch,
    SemanticHardFailure,
    CriticalRegionFailure,
    AiSevereIssue,
    NormalRegionFailure,
    AiMediumIssue,
    DecorativeRegionReview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum GateExitCode {
    Passed = 0,
    Invalid = 2,
    NeedsReview = 3,
    Failed = 4,
}

impl GateExitCode {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoundGateReport {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateCaptureInput {
    pub capture_id: String,
    pub screen: String,
    pub device: String,
    pub state: String,
    pub reference_profile: String,
    pub reference_binding: ReferenceBinding,
    pub diff_report: BoundGateReport,
    pub region_report: Option<BoundGateReport>,
    pub semantic_report: BoundGateReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateBundle {
    pub schema_version: u32,
    pub run_id: String,
    pub captures: Vec<GateCaptureInput>,
    pub ai_report: Option<BoundGateReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateRegionThreshold {
    pub max_raw_changed_ratio_millionths: u32,
    pub max_alpha_changed_ratio_millionths: u32,
    pub max_tolerated_changed_ratio_millionths: u32,
    pub minimum_ssim_millionths: i32,
    pub max_geometry_changed_ratio_millionths: u32,
    pub max_large_area_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateThresholdProfiles {
    pub critical: GateRegionThreshold,
    pub normal: GateRegionThreshold,
    pub decorative: GateRegionThreshold,
}

impl GateThresholdProfiles {
    fn for_level(&self, level: RegionLevel) -> &GateRegionThreshold {
        match level {
            RegionLevel::Critical => &self.critical,
            RegionLevel::Normal => &self.normal,
            RegionLevel::Decorative => &self.decorative,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceGateProfile {
    pub profile_id: String,
    pub reference_binding: ReferenceBinding,
    pub thresholds: GateThresholdProfiles,
    pub calibration_fixture_id: String,
    pub adjustment_rationale: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateConfig {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub conservative_default: GateThresholdProfiles,
    pub reference_profiles: Vec<ReferenceGateProfile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateInputIdentity {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateEvidenceError {
    pub capture_id: Option<String>,
    pub evidence_kind: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateMergeStep {
    pub priority: u8,
    pub failure_type: GateFailureType,
    pub rule: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateDimensionBreakdown {
    pub reference: PixelSize,
    pub actual: PixelSize,
    pub dimensions_match: bool,
    pub hard_failure: bool,
    pub blocking: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateSemanticBreakdown {
    pub status: SemanticAuditStatus,
    pub hard_failure_count: usize,
    pub blocking: bool,
    pub findings: Vec<SemanticFinding>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateThresholdViolation {
    pub metric: String,
    pub observed_millionths: i64,
    pub threshold_millionths: i64,
    pub comparison: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateRegionMetricsBreakdown {
    pub raw_changed_ratio_millionths: u32,
    pub alpha_changed_ratio_millionths: u32,
    pub tolerated_changed_ratio_millionths: u32,
    pub ssim_millionths: i32,
    pub geometry_changed_ratio_millionths: u32,
    pub large_area_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateRegionResult {
    pub region_id: String,
    pub level: RegionLevel,
    pub upstream_local_status: RegionLocalStatus,
    pub metrics: GateRegionMetricsBreakdown,
    pub applied_threshold: GateRegionThreshold,
    pub diagnostic_quality_floor_millionths: i32,
    pub upstream_threshold_violations: Vec<crate::ThresholdViolation>,
    pub profile_threshold_violations: Vec<GateThresholdViolation>,
    pub gate_state: GateState,
    pub blocking: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateRegionLevelSummary {
    pub level: RegionLevel,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub action_on_failure: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateRegionsBreakdown {
    pub threshold_source: String,
    pub critical: GateRegionLevelSummary,
    pub normal: GateRegionLevelSummary,
    pub decorative: GateRegionLevelSummary,
    pub results: Vec<GateRegionResult>,
    pub averaging_used_for_gate: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateAiIssueResult {
    pub issue: AiProviderIssue,
    pub blocking: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateAiBreakdown {
    pub ran: bool,
    pub severe_count: usize,
    pub medium_count: usize,
    pub minor_count: usize,
    pub issues: Vec<GateAiIssueResult>,
    pub minor_issues_are_report_only: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateReason {
    pub failure_type: GateFailureType,
    pub source_id: String,
    pub blocking: bool,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureGateResult {
    pub capture_id: String,
    pub screen: String,
    pub device: String,
    pub state_name: String,
    pub reference_profile: String,
    pub reference_binding: ReferenceBinding,
    pub threshold_source: String,
    pub state: GateState,
    pub primary_failure_type: GateFailureType,
    pub reasons: Vec<GateReason>,
    pub dimensions: GateDimensionBreakdown,
    pub semantic: GateSemanticBreakdown,
    pub regions: Option<GateRegionsBreakdown>,
    pub ai: GateAiBreakdown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateSummary {
    pub capture_count: usize,
    pub passed: usize,
    pub failed: usize,
    pub needs_review: usize,
    pub invalid: usize,
    pub global_numeric_score_emitted: bool,
    pub failed_regions_remain_individually_visible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GatePerformance {
    pub bounded_input_bytes: u64,
    pub estimated_peak_memory_bytes: u64,
    pub budget_bytes: u64,
    pub memory_basis: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisualGateReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: GateState,
    pub primary_failure_type: GateFailureType,
    pub run_id: String,
    pub config: GateInputIdentity,
    pub bundle: GateInputIdentity,
    pub merge_order: Vec<GateMergeStep>,
    pub score_policy: String,
    pub validation_errors: Vec<GateEvidenceError>,
    pub captures: Vec<CaptureGateResult>,
    pub summary: GateSummary,
    pub performance: GatePerformance,
    pub artifacts: Vec<ArtifactReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateOutcome {
    pub report: VisualGateReport,
    pub exit_code: GateExitCode,
}

struct LoadedCapture {
    input: GateCaptureInput,
    diff: DiffAnalysisReport,
    region: Option<RegionAuditReport>,
    semantic: SemanticAuditReport,
}

struct LoadedJson<T> {
    value: T,
    identity: GateInputIdentity,
}

pub fn evaluate_visual_gate(request: &GateRequest) -> Result<GateOutcome, ComparisonError> {
    let repository_root = fs::canonicalize(&request.repository_root).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            format!("repository root cannot be resolved: {error}"),
        )
        .at_path(&request.repository_root)
    })?;
    if !repository_root.is_dir() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            "repository root is not a directory",
        ));
    }
    let input_roots = resolve_allowed_input_roots(&repository_root, &request.allowed_input_roots)?;
    let output_root = resolve_allowed_root(
        &repository_root,
        &request.allowed_output_root,
        ComparisonErrorCode::AllowedOutputRootInvalid,
        "allowed output root",
    )?;
    let config_path = resolve_input_file(&repository_root, &input_roots, &request.config)?;
    let bundle_path = resolve_input_file(&repository_root, &input_roots, &request.bundle)?;
    let loaded_config: LoadedJson<GateConfig> = load_json_file(
        &config_path,
        MAX_GATE_CONFIG_BYTES,
        ComparisonErrorCode::GateConfigInvalid,
        "visual gate config",
    )?;
    let loaded_bundle: LoadedJson<GateBundle> = load_json_file(
        &bundle_path,
        MAX_GATE_BUNDLE_BYTES,
        ComparisonErrorCode::GateInputInvalid,
        "visual gate bundle",
    )?;
    validate_config(&loaded_config.value)?;
    validate_bundle(&loaded_bundle.value)?;

    let mut total_report_bytes = 0_u64;
    let mut validation_errors = Vec::new();
    let mut captures = Vec::with_capacity(loaded_bundle.value.captures.len());
    for capture in &loaded_bundle.value.captures {
        let diff = load_evidence_report::<DiffAnalysisReport>(
            &repository_root,
            &input_roots,
            &capture.diff_report,
            &capture.capture_id,
            "diff_report",
            &mut total_report_bytes,
        )?;
        let semantic = load_evidence_report::<SemanticAuditReport>(
            &repository_root,
            &input_roots,
            &capture.semantic_report,
            &capture.capture_id,
            "semantic_report",
            &mut total_report_bytes,
        )?;
        let region = match &capture.region_report {
            Some(reference) => load_evidence_report::<RegionAuditReport>(
                &repository_root,
                &input_roots,
                reference,
                &capture.capture_id,
                "region_report",
                &mut total_report_bytes,
            )?
            .map(Some),
            None => Ok(None),
        };
        match (diff, semantic, region) {
            (Ok(diff), Ok(semantic), Ok(region)) => {
                let loaded = LoadedCapture {
                    input: capture.clone(),
                    diff,
                    region,
                    semantic,
                };
                let errors = validate_capture_evidence(&loaded, &loaded_config.value);
                if errors.is_empty() {
                    captures.push(loaded);
                } else {
                    validation_errors.extend(errors);
                }
            }
            (diff, semantic, region) => {
                if let Err(error) = diff {
                    validation_errors.push(error);
                }
                if let Err(error) = semantic {
                    validation_errors.push(error);
                }
                if let Err(error) = region {
                    validation_errors.push(error);
                }
            }
        }
    }

    let ai_report = match &loaded_bundle.value.ai_report {
        Some(reference) => match load_evidence_report::<AiAnalysisReport>(
            &repository_root,
            &input_roots,
            reference,
            "<run>",
            "ai_report",
            &mut total_report_bytes,
        )? {
            Ok(report) => Some(report),
            Err(error) => {
                validation_errors.push(error);
                None
            }
        },
        None => None,
    };
    if let Some(ai) = &ai_report {
        validation_errors.extend(validate_ai_evidence(ai, &captures, &loaded_bundle.value));
    }
    let bounded_input_bytes = total_report_bytes
        .checked_add(loaded_config.identity.byte_length)
        .and_then(|bytes| bytes.checked_add(loaded_bundle.identity.byte_length))
        .ok_or_else(|| gate_input_too_large("visual gate input byte accounting overflowed"))?;
    let estimated_peak_memory_bytes = bounded_input_bytes
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(8 * 1024 * 1024))
        .ok_or_else(|| gate_input_too_large("visual gate memory estimate overflowed"))?;
    if estimated_peak_memory_bytes > VISUAL_GATE_PEAK_MEMORY_BUDGET_BYTES {
        return Err(gate_input_too_large(format!(
            "visual gate estimated peak {estimated_peak_memory_bytes} exceeds the {}-byte budget",
            VISUAL_GATE_PEAK_MEMORY_BUDGET_BYTES
        )));
    }

    let capture_results = if validation_errors.is_empty() {
        captures
            .iter()
            .map(|capture| evaluate_capture(capture, ai_report.as_ref(), &loaded_config.value))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let (status, primary_failure_type) = if validation_errors.is_empty() {
        summarize_capture_states(&capture_results)
    } else {
        (GateState::Invalid, GateFailureType::InvalidEvidence)
    };
    let summary = build_summary(
        loaded_bundle.value.captures.len(),
        &capture_results,
        validation_errors.len(),
    );
    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    let report_path = output_directory.join(VISUAL_GATE_REPORT_FILENAME);
    if report_path == config_path || report_path == bundle_path {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ArtifactNameConflict,
            "visual gate report would overwrite an input file",
        )
        .at_path(&report_path));
    }
    let report = VisualGateReport {
        schema_version: VISUAL_GATE_REPORT_SCHEMA_VERSION,
        algorithm_version: VISUAL_GATE_ALGORITHM_VERSION.to_owned(),
        status,
        primary_failure_type,
        run_id: loaded_bundle.value.run_id,
        config: loaded_config.identity,
        bundle: loaded_bundle.identity,
        merge_order: merge_order(),
        score_policy: "each_region_is_evaluated_independently_against_profile_thresholds; diagnostic_quality_floor_is_never_averaged_or_used_to_override_hard_failures"
            .to_owned(),
        validation_errors,
        captures: capture_results,
        summary,
        performance: GatePerformance {
            bounded_input_bytes,
            estimated_peak_memory_bytes,
            budget_bytes: VISUAL_GATE_PEAK_MEMORY_BUDGET_BYTES,
            memory_basis: "serialized_config_bundle_and_bound_reports_x4_plus_8_mib_fixed_workspace; estimated_not_os_measured"
                .to_owned(),
        },
        artifacts: vec![ArtifactReport {
            artifact_type: "visual_gate_report".to_owned(),
            path: VISUAL_GATE_REPORT_FILENAME.to_owned(),
        }],
    };
    persist_report(&report_path, &report)?;
    Ok(GateOutcome {
        exit_code: exit_code_for_state(status),
        report,
    })
}

fn validate_config(config: &GateConfig) -> Result<(), ComparisonError> {
    if config.schema_version != VISUAL_GATE_CONFIG_SCHEMA_VERSION
        || config.algorithm_version != VISUAL_GATE_ALGORITHM_VERSION
    {
        return Err(gate_config_error(format!(
            "visual gate config must use schema {} and algorithm {}",
            VISUAL_GATE_CONFIG_SCHEMA_VERSION, VISUAL_GATE_ALGORITHM_VERSION
        )));
    }
    validate_threshold_profiles(&config.conservative_default, "conservative_default")?;
    if config.reference_profiles.len() > MAX_GATE_PROFILES {
        return Err(gate_config_error(format!(
            "reference profile count exceeds {MAX_GATE_PROFILES}"
        )));
    }
    let mut ids = BTreeSet::new();
    for profile in &config.reference_profiles {
        validate_identifier(&profile.profile_id, "reference profile id")?;
        if !ids.insert(profile.profile_id.as_str()) {
            return Err(gate_config_error(format!(
                "duplicate reference profile {}",
                profile.profile_id
            )));
        }
        if !valid_sha256(&profile.reference_binding.sha256)
            || profile.reference_binding.revision == 0
        {
            return Err(gate_config_error(
                "profile reference binding requires lowercase SHA-256 and a positive revision",
            ));
        }
        validate_threshold_profiles(&profile.thresholds, &profile.profile_id)?;
        validate_nonempty_limited(
            &profile.calibration_fixture_id,
            "calibration_fixture_id",
            256,
        )?;
        validate_nonempty_limited(&profile.adjustment_rationale, "adjustment_rationale", 2048)?;
    }
    Ok(())
}

fn validate_threshold_profiles(
    profiles: &GateThresholdProfiles,
    label: &str,
) -> Result<(), ComparisonError> {
    for (level, threshold) in [
        ("critical", &profiles.critical),
        ("normal", &profiles.normal),
        ("decorative", &profiles.decorative),
    ] {
        for (name, value) in [
            (
                "max_raw_changed_ratio_millionths",
                threshold.max_raw_changed_ratio_millionths,
            ),
            (
                "max_alpha_changed_ratio_millionths",
                threshold.max_alpha_changed_ratio_millionths,
            ),
            (
                "max_tolerated_changed_ratio_millionths",
                threshold.max_tolerated_changed_ratio_millionths,
            ),
            (
                "max_geometry_changed_ratio_millionths",
                threshold.max_geometry_changed_ratio_millionths,
            ),
            (
                "max_large_area_ratio_millionths",
                threshold.max_large_area_ratio_millionths,
            ),
        ] {
            if value > 1_000_000 {
                return Err(gate_config_error(format!(
                    "{label}.{level}.{name} exceeds 1000000"
                )));
            }
        }
        if !(-1_000_000..=1_000_000).contains(&threshold.minimum_ssim_millionths) {
            return Err(gate_config_error(format!(
                "{label}.{level}.minimum_ssim_millionths is outside [-1000000, 1000000]"
            )));
        }
    }
    validate_ordering(
        profiles.critical.max_raw_changed_ratio_millionths,
        profiles.normal.max_raw_changed_ratio_millionths,
        profiles.decorative.max_raw_changed_ratio_millionths,
        label,
        "max_raw_changed_ratio_millionths",
    )?;
    validate_ordering(
        profiles.critical.max_alpha_changed_ratio_millionths,
        profiles.normal.max_alpha_changed_ratio_millionths,
        profiles.decorative.max_alpha_changed_ratio_millionths,
        label,
        "max_alpha_changed_ratio_millionths",
    )?;
    validate_ordering(
        profiles.critical.max_tolerated_changed_ratio_millionths,
        profiles.normal.max_tolerated_changed_ratio_millionths,
        profiles.decorative.max_tolerated_changed_ratio_millionths,
        label,
        "max_tolerated_changed_ratio_millionths",
    )?;
    validate_ordering(
        profiles.critical.max_geometry_changed_ratio_millionths,
        profiles.normal.max_geometry_changed_ratio_millionths,
        profiles.decorative.max_geometry_changed_ratio_millionths,
        label,
        "max_geometry_changed_ratio_millionths",
    )?;
    validate_ordering(
        profiles.critical.max_large_area_ratio_millionths,
        profiles.normal.max_large_area_ratio_millionths,
        profiles.decorative.max_large_area_ratio_millionths,
        label,
        "max_large_area_ratio_millionths",
    )?;
    if profiles.critical.minimum_ssim_millionths < profiles.normal.minimum_ssim_millionths
        || profiles.normal.minimum_ssim_millionths < profiles.decorative.minimum_ssim_millionths
    {
        return Err(gate_config_error(format!(
            "{label}.minimum_ssim_millionths must be critical >= normal >= decorative"
        )));
    }
    Ok(())
}

fn validate_ordering(
    critical: u32,
    normal: u32,
    decorative: u32,
    label: &str,
    metric: &str,
) -> Result<(), ComparisonError> {
    if critical > normal || normal > decorative {
        return Err(gate_config_error(format!(
            "{label}.{metric} must be critical <= normal <= decorative"
        )));
    }
    Ok(())
}

fn validate_bundle(bundle: &GateBundle) -> Result<(), ComparisonError> {
    if bundle.schema_version != VISUAL_GATE_BUNDLE_SCHEMA_VERSION {
        return Err(gate_input_error(format!(
            "visual gate bundle must use schema {VISUAL_GATE_BUNDLE_SCHEMA_VERSION}"
        )));
    }
    validate_identifier(&bundle.run_id, "run id")?;
    if bundle.captures.is_empty() || bundle.captures.len() > MAX_GATE_CAPTURES {
        return Err(gate_input_error(format!(
            "capture count must be between 1 and {MAX_GATE_CAPTURES}"
        )));
    }
    let mut ids = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for capture in &bundle.captures {
        validate_identifier(&capture.capture_id, "capture id")?;
        validate_identifier(&capture.screen, "screen")?;
        validate_identifier(&capture.device, "device")?;
        validate_identifier(&capture.state, "state")?;
        validate_identifier(&capture.reference_profile, "reference profile")?;
        validate_binding(&capture.reference_binding, "capture reference binding")?;
        validate_bound_report(&capture.diff_report, "diff report")?;
        if let Some(region) = &capture.region_report {
            validate_bound_report(region, "region report")?;
        }
        validate_bound_report(&capture.semantic_report, "semantic report")?;
        let expected_capture_id =
            format!("{}.{}.{}", capture.screen, capture.device, capture.state);
        if capture.capture_id != expected_capture_id {
            return Err(gate_input_error(format!(
                "capture id {} must equal screen.device.state {}",
                capture.capture_id, expected_capture_id
            )));
        }
        if !identities.insert((
            capture.screen.as_str(),
            capture.device.as_str(),
            capture.state.as_str(),
        )) {
            return Err(gate_input_error(format!(
                "duplicate capture identity {}.{}.{}",
                capture.screen, capture.device, capture.state
            )));
        }
        if !ids.insert(capture.capture_id.as_str()) {
            return Err(gate_input_error(format!(
                "duplicate capture id {}",
                capture.capture_id
            )));
        }
    }
    if let Some(ai) = &bundle.ai_report {
        validate_bound_report(ai, "AI report")?;
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), ComparisonError> {
    if value.is_empty()
        || value.len() > MAX_GATE_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(gate_input_error(format!(
            "{label} must be 1..={MAX_GATE_IDENTIFIER_BYTES} ASCII [A-Za-z0-9_.-] bytes"
        )));
    }
    Ok(())
}

fn validate_nonempty_limited(
    value: &str,
    label: &str,
    maximum: usize,
) -> Result<(), ComparisonError> {
    if value.trim().is_empty() || value.len() > maximum {
        return Err(gate_config_error(format!(
            "{label} must be non-empty and at most {maximum} bytes"
        )));
    }
    Ok(())
}

fn validate_binding(binding: &ReferenceBinding, label: &str) -> Result<(), ComparisonError> {
    if !valid_sha256(&binding.sha256) || binding.revision == 0 {
        return Err(gate_input_error(format!(
            "{label} requires lowercase SHA-256 and a positive revision"
        )));
    }
    Ok(())
}

fn validate_bound_report(reference: &BoundGateReport, label: &str) -> Result<(), ComparisonError> {
    if reference.path.trim().is_empty() || !valid_sha256(&reference.sha256) {
        return Err(gate_input_error(format!(
            "{label} requires a non-empty path and lowercase SHA-256"
        )));
    }
    Ok(())
}

fn validate_capture_evidence(
    capture: &LoadedCapture,
    config: &GateConfig,
) -> Vec<GateEvidenceError> {
    let mut errors = Vec::new();
    let id = &capture.input.capture_id;
    if capture.diff.schema_version != DIFF_METRICS_REPORT_SCHEMA_VERSION
        || capture.diff.algorithm_version != DIFF_METRICS_ALGORITHM_VERSION
    {
        errors.push(evidence_error(
            Some(id),
            "diff_report",
            "protocol_mismatch",
            "diff report schema or algorithm does not match ui_diff_metrics_v1",
        ));
    }
    let dimension_failure = capture.diff.status == DiffAnalysisStatus::ComparisonFailed
        && capture.diff.failure.as_ref().is_some_and(|failure| {
            failure.code == ComparisonErrorCode::DimensionsMismatch
                && failure.failure_type == crate::FailureType::Comparison
        })
        && capture.diff.dimensions.reference != capture.diff.dimensions.actual
        && capture.diff.metrics.is_none()
        && capture.diff.performance.is_none();
    let analyzed = capture.diff.status == DiffAnalysisStatus::Analyzed
        && capture.diff.dimensions.reference == capture.diff.dimensions.actual
        && capture.diff.metrics.is_some()
        && capture.diff.performance.is_some()
        && capture.diff.failure.is_none();
    if !dimension_failure && !analyzed {
        errors.push(evidence_error(
            Some(id),
            "diff_report",
            "invalid_diff_terminal",
            "diff report must be a complete analyzed result or an explicit dimension mismatch",
        ));
    }
    if dimension_failure && capture.region.is_some() {
        errors.push(evidence_error(
            Some(id),
            "region_report",
            "unexpected_downstream_report",
            "dimension mismatch captures must not claim a downstream region report",
        ));
    }
    if analyzed && capture.region.is_none() {
        errors.push(evidence_error(
            Some(id),
            "region_report",
            "missing_required_report",
            "analyzed captures require a region report",
        ));
    }
    validate_semantic_report(&capture.semantic, id, &mut errors);
    if let Some(region) = &capture.region {
        validate_region_report(region, &capture.diff, id, &mut errors);
        if region.reference_binding != capture.input.reference_binding {
            errors.push(evidence_error(
                Some(id),
                "region_report",
                "reference_binding_mismatch",
                "capture and region report reference bindings differ",
            ));
        }
    }
    if let Some(profile) = config
        .reference_profiles
        .iter()
        .find(|profile| profile.profile_id == capture.input.reference_profile)
        && profile.reference_binding != capture.input.reference_binding
    {
        errors.push(evidence_error(
            Some(id),
            "gate_config",
            "reference_profile_binding_mismatch",
            "configured reference profile is bound to a different baseline",
        ));
    }
    errors
}

fn validate_semantic_report(
    report: &SemanticAuditReport,
    capture_id: &str,
    errors: &mut Vec<GateEvidenceError>,
) {
    let failed = !report.findings.is_empty();
    let status_consistent = matches!(report.status, SemanticAuditStatus::SemanticFailed) == failed;
    let mut expected_by_code = BTreeMap::new();
    for finding in &report.findings {
        *expected_by_code.entry(finding.code).or_insert(0_usize) += 1;
    }
    if report.schema_version != SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION
        || report.algorithm_version != SEMANTIC_AUDIT_ALGORITHM_VERSION
        || !status_consistent
        || report.rules.hard_failure_count != report.findings.len()
        || report.rules.findings_by_code != expected_by_code
        || report.separation.semantic_hard_failure != failed
        || report.separation.visual_similarity_consumed
        || report.separation.local_visual_scores_consumed
        || report.separation.can_visual_score_offset_hard_failure
    {
        errors.push(evidence_error(
            Some(capture_id),
            "semantic_report",
            "semantic_contract_invalid",
            "semantic status, finding counts, or hard-failure separation contract is inconsistent",
        ));
    }
}

fn validate_region_report(
    report: &RegionAuditReport,
    diff: &DiffAnalysisReport,
    capture_id: &str,
    errors: &mut Vec<GateEvidenceError>,
) {
    if report.schema_version != REGION_AUDIT_REPORT_SCHEMA_VERSION
        || report.algorithm_version != REGION_AUDIT_ALGORITHM_VERSION
        || report.status != "analyzed"
        || report.scope_boundary
            != "region_local_rules_only_no_global_pass_failed_needs_review_or_invalid_gate"
        || report.dimensions != diff.dimensions.reference
        || report.inputs.aligned_reference_sha256 != diff.inputs.reference_sha256
        || report.inputs.aligned_actual_sha256 != diff.inputs.actual_sha256
    {
        errors.push(evidence_error(
            Some(capture_id),
            "region_report",
            "region_provenance_invalid",
            "region protocol, dimensions, or aligned-image hashes do not match the diff report",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut passed_weight = 0_u64;
    let mut failed_weight = 0_u64;
    let mut counts = [0_u32; 3];
    for region in &report.region_results {
        let metrics_valid = region.metrics.raw.changed_pixel_ratio_millionths <= 1_000_000
            && region.metrics.alpha.changed_pixel_ratio_millionths <= 1_000_000
            && region.metrics.tolerated.changed_pixel_ratio_millionths <= 1_000_000
            && (-1_000_000..=1_000_000).contains(&region.metrics.perceptual.score_millionths)
            && region
                .metrics
                .categories
                .geometry_edges
                .mismatched_edge_ratio_millionths
                <= 1_000_000
            && region
                .metrics
                .categories
                .large_area_content
                .covered_pixel_ratio_millionths
                <= 1_000_000;
        let status_failed = region.local_status == RegionLocalStatus::Failed;
        if !ids.insert(region.region_id.as_str())
            || region.evaluated_pixels == 0
            || region.weight != region.threshold.weight
            || status_failed == region.threshold_violations.is_empty()
            || !metrics_valid
        {
            errors.push(evidence_error(
                Some(capture_id),
                "region_report",
                "region_result_invalid",
                "region IDs, evaluated pixels, weights, metrics, or local status are inconsistent",
            ));
            break;
        }
        counts[level_index(region.level)] += 1;
        if status_failed {
            failed_weight += u64::from(region.weight);
        } else {
            passed_weight += u64::from(region.weight);
        }
    }
    let summary = &report.weight_summary;
    if summary.merge_policy != "independent_region_weights_sum_without_pixel_average_or_global_gate"
        || summary.total_declared_weight != passed_weight + failed_weight
        || summary.passed_weight != passed_weight
        || summary.failed_weight != failed_weight
        || summary.critical_regions != counts[0]
        || summary.normal_regions != counts[1]
        || summary.decorative_regions != counts[2]
    {
        errors.push(evidence_error(
            Some(capture_id),
            "region_report",
            "region_summary_invalid",
            "region weight summary does not equal the independent region results",
        ));
    }
}

fn validate_ai_evidence(
    report: &AiAnalysisReport,
    captures: &[LoadedCapture],
    bundle: &GateBundle,
) -> Vec<GateEvidenceError> {
    let mut errors = Vec::new();
    if report.schema_version != AI_ANALYSIS_REPORT_SCHEMA_VERSION
        || report.algorithm_version != AI_ANALYSIS_ALGORITHM_VERSION
        || report.status != AiAnalysisStatus::Completed
        || !report.deterministic_hard_failures_preserved
        || report.visual_similarity_is_sole_conclusion
        || report.provider.self_review_is_sole_conclusion
    {
        errors.push(evidence_error(
            None,
            "ai_report",
            "ai_contract_invalid",
            "AI report protocol or deterministic separation flags are invalid",
        ));
        return errors;
    }
    let capture_by_id = captures
        .iter()
        .map(|capture| (capture.input.capture_id.as_str(), capture))
        .collect::<BTreeMap<_, _>>();
    let known_bundle_ids = bundle
        .captures
        .iter()
        .map(|capture| capture.capture_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut ai_capture_ids = BTreeSet::new();
    let mut provider_image_ids = BTreeSet::new();
    for image in &report.input.provider_images {
        if !provider_image_ids.insert(image.image_id.as_str()) {
            errors.push(evidence_error(
                None,
                "ai_report",
                "duplicate_provider_image",
                "AI provider image IDs must be unique",
            ));
            continue;
        }
        let matched = bundle.captures.iter().find_map(|capture| {
            ["reference", "actual", "overlay", "heatmap"]
                .iter()
                .find(|role| image.image_id == format!("{}.{}", capture.capture_id, role))
                .map(|role| (&capture.capture_id, *role))
        });
        let Some((capture_id, role)) = matched else {
            errors.push(evidence_error(
                None,
                "ai_report",
                "unknown_provider_image",
                "AI provider image does not map to a gate capture and known image role",
            ));
            continue;
        };
        ai_capture_ids.insert(capture_id.as_str());
        if let Some(capture) = capture_by_id.get(capture_id.as_str()) {
            let expected = match role {
                "reference" => Some(capture.diff.inputs.reference_sha256.as_str()),
                "actual" => Some(capture.diff.inputs.actual_sha256.as_str()),
                "overlay" | "heatmap" => capture
                    .diff
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.artifact_type == role)
                    .and_then(|artifact| artifact.sha256.as_deref()),
                _ => unreachable!(),
            };
            if expected.is_none_or(|hash| hash != image.source_sha256) {
                errors.push(evidence_error(
                    Some(capture_id),
                    "ai_report",
                    "ai_image_provenance_mismatch",
                    "AI source image hash does not match the deterministic diff input",
                ));
            }
        }
    }
    if report.input.image_count != report.input.provider_images.len()
        || report.input.capture_count != ai_capture_ids.len()
    {
        errors.push(evidence_error(
            None,
            "ai_report",
            "ai_input_summary_invalid",
            "AI input capture/image counts do not match provider image evidence",
        ));
    }
    if ai_capture_ids.is_empty() {
        errors.push(evidence_error(
            None,
            "ai_report",
            "ai_capture_set_empty",
            "a supplied AI report must contain at least one complete capture",
        ));
    }
    let expected_region_metric_count = ai_capture_ids
        .iter()
        .filter_map(|capture_id| {
            capture_by_id
                .get(capture_id)
                .and_then(|capture| capture.region.as_ref())
        })
        .map(|region| region.region_results.len())
        .sum::<usize>();
    let expected_semantic_node_count = ai_capture_ids
        .iter()
        .filter_map(|capture_id| capture_by_id.get(capture_id))
        .map(|capture| capture.semantic.input.node_count)
        .sum::<usize>();
    if report.input.region_metric_count != expected_region_metric_count
        || report.input.semantic_node_count != expected_semantic_node_count
    {
        errors.push(evidence_error(
            None,
            "ai_report",
            "ai_analysis_count_mismatch",
            "AI region metric or semantic node counts do not match upstream reports",
        ));
    }
    for capture_id in &ai_capture_ids {
        for role in ["reference", "actual", "overlay", "heatmap"] {
            let expected = format!("{capture_id}.{role}");
            if !provider_image_ids.contains(expected.as_str()) {
                errors.push(evidence_error(
                    Some(capture_id),
                    "ai_report",
                    "ai_image_set_incomplete",
                    "AI-analyzed captures require reference, actual, overlay, and heatmap evidence",
                ));
            }
        }
    }
    for issue in &report.issues {
        if !ai_capture_ids.contains(issue.capture_id.as_str()) {
            errors.push(evidence_error(
                Some(&issue.capture_id),
                "ai_report",
                "ai_issue_capture_invalid",
                "AI issue does not belong to an AI-analyzed gate capture",
            ));
        }
    }
    for hard_failure in &report.deterministic_hard_failures {
        if !known_bundle_ids.contains(hard_failure.capture_id.as_str()) {
            errors.push(evidence_error(
                Some(&hard_failure.capture_id),
                "ai_report",
                "ai_hard_failure_capture_invalid",
                "AI hard-failure copy references an unknown gate capture",
            ));
        }
    }
    for capture_id in ai_capture_ids {
        if let Some(capture) = capture_by_id.get(capture_id) {
            let actual = report
                .deterministic_hard_failures
                .iter()
                .filter(|failure| failure.capture_id == capture_id)
                .map(|failure| &failure.finding)
                .collect::<Vec<_>>();
            let expected = capture.semantic.findings.iter().collect::<Vec<_>>();
            if actual != expected {
                errors.push(evidence_error(
                    Some(capture_id),
                    "ai_report",
                    "deterministic_hard_failures_not_preserved",
                    "AI report did not preserve the exact semantic hard failures for this capture",
                ));
            }
        }
    }
    errors
}

fn evaluate_capture(
    capture: &LoadedCapture,
    ai_report: Option<&AiAnalysisReport>,
    config: &GateConfig,
) -> CaptureGateResult {
    let profile = config
        .reference_profiles
        .iter()
        .find(|profile| profile.profile_id == capture.input.reference_profile);
    let (thresholds, threshold_source) = match profile {
        Some(profile) => (
            &profile.thresholds,
            format!(
                "reference_profile:{} calibrated_by:{}",
                profile.profile_id, profile.calibration_fixture_id
            ),
        ),
        None => (
            &config.conservative_default,
            "conservative_default:no_reference_profile_override".to_owned(),
        ),
    };
    let dimension_hard_failure = capture.diff.status == DiffAnalysisStatus::ComparisonFailed;
    let dimensions = GateDimensionBreakdown {
        reference: capture.diff.dimensions.reference,
        actual: capture.diff.dimensions.actual,
        dimensions_match: capture.diff.dimensions.reference == capture.diff.dimensions.actual,
        hard_failure: dimension_hard_failure,
        blocking: dimension_hard_failure,
    };
    let semantic_blocking = !capture.semantic.findings.is_empty();
    let semantic = GateSemanticBreakdown {
        status: capture.semantic.status,
        hard_failure_count: capture.semantic.findings.len(),
        blocking: semantic_blocking,
        findings: capture.semantic.findings.clone(),
    };
    let regions = capture
        .region
        .as_ref()
        .map(|report| evaluate_regions(report, thresholds, threshold_source.clone()));
    let ai = evaluate_ai(&capture.input.capture_id, ai_report);
    let mut reasons = Vec::new();
    if dimension_hard_failure {
        reasons.push(reason(
            GateFailureType::DimensionMismatch,
            "dimensions",
            true,
            "reference and actual dimensions differ; no visual score may offset this failure",
        ));
    }
    if semantic_blocking {
        reasons.push(reason(
            GateFailureType::SemanticHardFailure,
            "semantic_report",
            true,
            "one or more deterministic semantic hard failures are present",
        ));
    }
    if let Some(regions) = &regions {
        for region in &regions.results {
            if region.gate_state == GateState::Passed {
                continue;
            }
            let (failure_type, blocking) = region_gate_rule(region.level);
            reasons.push(reason(
                failure_type,
                &region.region_id,
                blocking,
                "region failed independently; no weighted average is applied",
            ));
        }
    }
    for issue in &ai.issues {
        let Some((failure_type, blocking)) = ai_gate_rule(issue.issue.severity) else {
            continue;
        };
        reasons.push(reason(
            failure_type,
            &format!(
                "ai:{:?}:{}",
                issue.issue.problem_type, issue.issue.capture_id
            )
            .to_ascii_lowercase(),
            blocking,
            "AI severe and medium issues block; minor issues remain report-only",
        ));
    }
    reasons.sort_by_key(|item| failure_priority(item.failure_type));
    let (state, primary_failure_type) = classify_reasons(&reasons);
    CaptureGateResult {
        capture_id: capture.input.capture_id.clone(),
        screen: capture.input.screen.clone(),
        device: capture.input.device.clone(),
        state_name: capture.input.state.clone(),
        reference_profile: capture.input.reference_profile.clone(),
        reference_binding: capture.input.reference_binding.clone(),
        threshold_source,
        state,
        primary_failure_type,
        reasons,
        dimensions,
        semantic,
        regions,
        ai,
    }
}

fn evaluate_regions(
    report: &RegionAuditReport,
    thresholds: &GateThresholdProfiles,
    threshold_source: String,
) -> GateRegionsBreakdown {
    let mut results = Vec::with_capacity(report.region_results.len());
    for region in &report.region_results {
        let metrics = GateRegionMetricsBreakdown {
            raw_changed_ratio_millionths: region.metrics.raw.changed_pixel_ratio_millionths,
            alpha_changed_ratio_millionths: region.metrics.alpha.changed_pixel_ratio_millionths,
            tolerated_changed_ratio_millionths: region
                .metrics
                .tolerated
                .changed_pixel_ratio_millionths,
            ssim_millionths: region.metrics.perceptual.score_millionths,
            geometry_changed_ratio_millionths: region
                .metrics
                .categories
                .geometry_edges
                .mismatched_edge_ratio_millionths,
            large_area_ratio_millionths: region
                .metrics
                .categories
                .large_area_content
                .covered_pixel_ratio_millionths,
        };
        let applied = thresholds.for_level(region.level).clone();
        let profile_violations = threshold_violations(&metrics, &applied);
        let failed = !profile_violations.is_empty();
        let gate_state = if failed {
            match region.level {
                RegionLevel::Critical | RegionLevel::Normal => GateState::Failed,
                RegionLevel::Decorative => GateState::NeedsReview,
            }
        } else {
            GateState::Passed
        };
        results.push(GateRegionResult {
            region_id: region.region_id.clone(),
            level: region.level,
            upstream_local_status: region.local_status,
            diagnostic_quality_floor_millionths: diagnostic_quality_floor(&metrics),
            metrics,
            applied_threshold: applied,
            upstream_threshold_violations: region.threshold_violations.clone(),
            profile_threshold_violations: profile_violations,
            gate_state,
            blocking: failed && region.level != RegionLevel::Decorative,
        });
    }
    GateRegionsBreakdown {
        threshold_source,
        critical: summarize_level(&results, RegionLevel::Critical, "failed"),
        normal: summarize_level(&results, RegionLevel::Normal, "failed"),
        decorative: summarize_level(&results, RegionLevel::Decorative, "needs_review"),
        results,
        averaging_used_for_gate: false,
    }
}

fn threshold_violations(
    metrics: &GateRegionMetricsBreakdown,
    threshold: &GateRegionThreshold,
) -> Vec<GateThresholdViolation> {
    let mut violations = Vec::new();
    check_max(
        &mut violations,
        "raw_changed_ratio_millionths",
        metrics.raw_changed_ratio_millionths,
        threshold.max_raw_changed_ratio_millionths,
    );
    check_max(
        &mut violations,
        "alpha_changed_ratio_millionths",
        metrics.alpha_changed_ratio_millionths,
        threshold.max_alpha_changed_ratio_millionths,
    );
    check_max(
        &mut violations,
        "tolerated_changed_ratio_millionths",
        metrics.tolerated_changed_ratio_millionths,
        threshold.max_tolerated_changed_ratio_millionths,
    );
    if metrics.ssim_millionths < threshold.minimum_ssim_millionths {
        violations.push(GateThresholdViolation {
            metric: "ssim_millionths".to_owned(),
            observed_millionths: i64::from(metrics.ssim_millionths),
            threshold_millionths: i64::from(threshold.minimum_ssim_millionths),
            comparison: "minimum_inclusive".to_owned(),
            source: "reference_profile".to_owned(),
        });
    }
    check_max(
        &mut violations,
        "geometry_changed_ratio_millionths",
        metrics.geometry_changed_ratio_millionths,
        threshold.max_geometry_changed_ratio_millionths,
    );
    check_max(
        &mut violations,
        "large_area_ratio_millionths",
        metrics.large_area_ratio_millionths,
        threshold.max_large_area_ratio_millionths,
    );
    violations
}

fn check_max(
    violations: &mut Vec<GateThresholdViolation>,
    metric: &str,
    observed: u32,
    threshold: u32,
) {
    if observed > threshold {
        violations.push(GateThresholdViolation {
            metric: metric.to_owned(),
            observed_millionths: i64::from(observed),
            threshold_millionths: i64::from(threshold),
            comparison: "maximum_inclusive".to_owned(),
            source: "reference_profile".to_owned(),
        });
    }
}

fn diagnostic_quality_floor(metrics: &GateRegionMetricsBreakdown) -> i32 {
    let complement = |ratio: u32| 1_000_000_i32.saturating_sub(ratio as i32);
    [
        complement(metrics.raw_changed_ratio_millionths),
        complement(metrics.alpha_changed_ratio_millionths),
        complement(metrics.tolerated_changed_ratio_millionths),
        metrics.ssim_millionths,
        complement(metrics.geometry_changed_ratio_millionths),
        complement(metrics.large_area_ratio_millionths),
    ]
    .into_iter()
    .min()
    .unwrap_or(-1_000_000)
}

fn summarize_level(
    results: &[GateRegionResult],
    level: RegionLevel,
    action: &str,
) -> GateRegionLevelSummary {
    let matching = results.iter().filter(|region| region.level == level);
    let total = matching.clone().count();
    let failed = matching
        .filter(|region| region.gate_state != GateState::Passed)
        .count();
    GateRegionLevelSummary {
        level,
        total,
        passed: total - failed,
        failed,
        action_on_failure: action.to_owned(),
    }
}

fn evaluate_ai(capture_id: &str, report: Option<&AiAnalysisReport>) -> GateAiBreakdown {
    let issues = report
        .into_iter()
        .flat_map(|report| &report.issues)
        .filter(|issue| issue.capture_id == capture_id)
        .cloned()
        .map(|issue| GateAiIssueResult {
            blocking: matches!(issue.severity, AiSeverity::Severe | AiSeverity::Medium),
            issue,
        })
        .collect::<Vec<_>>();
    GateAiBreakdown {
        ran: report.is_some_and(|report| {
            report
                .input
                .provider_images
                .iter()
                .any(|image| image.image_id == format!("{capture_id}.reference"))
        }),
        severe_count: issues
            .iter()
            .filter(|issue| issue.issue.severity == AiSeverity::Severe)
            .count(),
        medium_count: issues
            .iter()
            .filter(|issue| issue.issue.severity == AiSeverity::Medium)
            .count(),
        minor_count: issues
            .iter()
            .filter(|issue| issue.issue.severity == AiSeverity::Minor)
            .count(),
        issues,
        minor_issues_are_report_only: true,
    }
}

fn classify_reasons(reasons: &[GateReason]) -> (GateState, GateFailureType) {
    let primary = reasons
        .iter()
        .min_by_key(|reason| failure_priority(reason.failure_type));
    if let Some(primary) = primary {
        let state = if reasons.iter().any(|reason| reason.blocking) {
            GateState::Failed
        } else {
            GateState::NeedsReview
        };
        (state, primary.failure_type)
    } else {
        (GateState::Passed, GateFailureType::None)
    }
}

fn region_gate_rule(level: RegionLevel) -> (GateFailureType, bool) {
    match level {
        RegionLevel::Critical => (GateFailureType::CriticalRegionFailure, true),
        RegionLevel::Normal => (GateFailureType::NormalRegionFailure, true),
        RegionLevel::Decorative => (GateFailureType::DecorativeRegionReview, false),
    }
}

fn ai_gate_rule(severity: AiSeverity) -> Option<(GateFailureType, bool)> {
    match severity {
        AiSeverity::Severe => Some((GateFailureType::AiSevereIssue, true)),
        AiSeverity::Medium => Some((GateFailureType::AiMediumIssue, true)),
        AiSeverity::Minor => None,
    }
}

fn summarize_capture_states(results: &[CaptureGateResult]) -> (GateState, GateFailureType) {
    let status = if results
        .iter()
        .any(|result| result.state == GateState::Invalid)
    {
        GateState::Invalid
    } else if results
        .iter()
        .any(|result| result.state == GateState::Failed)
    {
        GateState::Failed
    } else if results
        .iter()
        .any(|result| result.state == GateState::NeedsReview)
    {
        GateState::NeedsReview
    } else {
        GateState::Passed
    };
    let primary = results
        .iter()
        .filter(|result| result.primary_failure_type != GateFailureType::None)
        .min_by_key(|result| failure_priority(result.primary_failure_type))
        .map_or(GateFailureType::None, |result| result.primary_failure_type);
    (status, primary)
}

fn build_summary(
    expected_captures: usize,
    results: &[CaptureGateResult],
    invalid_errors: usize,
) -> GateSummary {
    GateSummary {
        capture_count: expected_captures,
        passed: results
            .iter()
            .filter(|result| result.state == GateState::Passed)
            .count(),
        failed: results
            .iter()
            .filter(|result| result.state == GateState::Failed)
            .count(),
        needs_review: results
            .iter()
            .filter(|result| result.state == GateState::NeedsReview)
            .count(),
        invalid: if invalid_errors > 0 {
            expected_captures
        } else {
            0
        },
        global_numeric_score_emitted: false,
        failed_regions_remain_individually_visible: true,
    }
}

fn merge_order() -> Vec<GateMergeStep> {
    [
        (
            GateFailureType::InvalidEvidence,
            "invalid input, schema, hash, provenance, or separation evidence makes the gate invalid",
        ),
        (
            GateFailureType::DimensionMismatch,
            "dimension mismatch is a deterministic hard failure",
        ),
        (
            GateFailureType::SemanticHardFailure,
            "semantic hard failures cannot be offset by visual metrics or AI",
        ),
        (
            GateFailureType::CriticalRegionFailure,
            "any failed critical region blocks independently",
        ),
        (
            GateFailureType::AiSevereIssue,
            "any severe AI issue blocks after deterministic critical evidence",
        ),
        (
            GateFailureType::NormalRegionFailure,
            "any failed normal region blocks independently",
        ),
        (
            GateFailureType::AiMediumIssue,
            "any medium AI issue blocks",
        ),
        (
            GateFailureType::DecorativeRegionReview,
            "a decorative-only failure requires review without automatic failure",
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (failure_type, rule))| GateMergeStep {
        priority: (index + 1) as u8,
        failure_type,
        rule: rule.to_owned(),
    })
    .collect()
}

fn failure_priority(failure_type: GateFailureType) -> u8 {
    match failure_type {
        GateFailureType::InvalidEvidence => 0,
        GateFailureType::DimensionMismatch => 1,
        GateFailureType::SemanticHardFailure => 2,
        GateFailureType::CriticalRegionFailure => 3,
        GateFailureType::AiSevereIssue => 4,
        GateFailureType::NormalRegionFailure => 5,
        GateFailureType::AiMediumIssue => 6,
        GateFailureType::DecorativeRegionReview => 7,
        GateFailureType::None => u8::MAX,
    }
}

fn reason(
    failure_type: GateFailureType,
    source_id: &str,
    blocking: bool,
    message: &str,
) -> GateReason {
    GateReason {
        failure_type,
        source_id: source_id.to_owned(),
        blocking,
        message: message.to_owned(),
    }
}

fn exit_code_for_state(state: GateState) -> GateExitCode {
    match state {
        GateState::Passed => GateExitCode::Passed,
        GateState::NeedsReview => GateExitCode::NeedsReview,
        GateState::Failed => GateExitCode::Failed,
        GateState::Invalid => GateExitCode::Invalid,
    }
}

fn load_json_file<T: DeserializeOwned>(
    path: &Path,
    maximum: u64,
    code: ComparisonErrorCode,
    label: &str,
) -> Result<LoadedJson<T>, ComparisonError> {
    let bytes = read_limited(path, maximum, code)?;
    let value = serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(code, format!("{label} is invalid: {error}")).at_path(path)
    })?;
    Ok(LoadedJson {
        value,
        identity: GateInputIdentity {
            path: path.display().to_string(),
            sha256: sha256(&bytes),
            byte_length: bytes.len() as u64,
        },
    })
}

fn load_evidence_report<T: DeserializeOwned>(
    repository_root: &Path,
    input_roots: &[PathBuf],
    reference: &BoundGateReport,
    capture_id: &str,
    evidence_kind: &str,
    total_bytes: &mut u64,
) -> Result<Result<T, GateEvidenceError>, ComparisonError> {
    let path = resolve_input_file(repository_root, input_roots, Path::new(&reference.path))?;
    let bytes = read_limited(
        &path,
        MAX_GATE_REPORT_INPUT_BYTES,
        ComparisonErrorCode::GateInputTooLarge,
    )?;
    *total_bytes = total_bytes
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| gate_input_too_large("visual gate total evidence byte count overflowed"))?;
    if *total_bytes > MAX_GATE_TOTAL_INPUT_BYTES {
        return Err(gate_input_too_large(format!(
            "visual gate evidence exceeds {MAX_GATE_TOTAL_INPUT_BYTES} bytes"
        )));
    }
    let actual_sha256 = sha256(&bytes);
    if actual_sha256 != reference.sha256 {
        return Ok(Err(evidence_error(
            capture_option(capture_id),
            evidence_kind,
            "report_hash_mismatch",
            "report bytes do not match the SHA-256 bound by the gate bundle",
        )));
    }
    match serde_json::from_slice(&bytes) {
        Ok(report) => Ok(Ok(report)),
        Err(error) => Ok(Err(evidence_error(
            capture_option(capture_id),
            evidence_kind,
            "report_parse_failed",
            &format!("strict report JSON cannot be parsed: {error}"),
        ))),
    }
}

fn capture_option(capture_id: &str) -> Option<&str> {
    (capture_id != "<run>").then_some(capture_id)
}

fn read_limited(
    path: &Path,
    maximum: u64,
    code: ComparisonErrorCode,
) -> Result<Vec<u8>, ComparisonError> {
    let file = fs::File::open(path).map_err(|error| {
        ComparisonError::input(code, format!("visual gate input cannot be opened: {error}"))
            .at_path(path)
    })?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ComparisonError::input(code, format!("visual gate input cannot be read: {error}"))
                .at_path(path)
        })?;
    if bytes.len() as u64 > maximum {
        return Err(ComparisonError::input(
            ComparisonErrorCode::GateInputTooLarge,
            format!("visual gate input exceeds {maximum} bytes"),
        )
        .at_path(path));
    }
    Ok(bytes)
}

fn persist_report(path: &Path, report: &VisualGateReport) -> Result<(), ComparisonError> {
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        ComparisonError::internal_failure(format!(
            "visual gate report cannot be serialized: {error}"
        ))
    })?;
    let temporary = path.with_file_name(format!(
        ".{}.tmp-{}",
        VISUAL_GATE_REPORT_FILENAME,
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            ComparisonError::input(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("temporary visual gate report cannot be created: {error}"),
            )
            .at_path(&temporary)
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::input(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("temporary visual gate report cannot be written: {error}"),
        )
        .at_path(&temporary));
    }
    drop(file);
    if let Err(error) = fs::hard_link(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::input(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("visual gate report cannot be finalized without clobbering: {error}"),
        )
        .at_path(path));
    }
    fs::remove_file(&temporary).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("visual gate temporary report cannot be removed: {error}"),
        )
        .at_path(&temporary)
    })?;
    Ok(())
}

fn evidence_error(
    capture_id: Option<&str>,
    evidence_kind: &str,
    code: &str,
    message: &str,
) -> GateEvidenceError {
    GateEvidenceError {
        capture_id: capture_id.map(str::to_owned),
        evidence_kind: evidence_kind.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn gate_config_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::GateConfigInvalid, message)
}

fn gate_input_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::GateInputInvalid, message)
}

fn gate_input_too_large(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::GateInputTooLarge, message)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn level_index(level: RegionLevel) -> usize {
    match level {
        RegionLevel::Critical => 0,
        RegionLevel::Normal => 1,
        RegionLevel::Decorative => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CalibrationFixture {
        schema_version: u32,
        fixture_id: String,
        scope: String,
        profile: String,
        false_positive_count: usize,
        false_negative_count: usize,
        misclassification_count: usize,
        false_positive_definition: String,
        false_negative_definition: String,
        adjustment_rationale: String,
        cases: Vec<CalibrationCase>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CalibrationCase {
        case_id: String,
        dimension_hard_failure: bool,
        semantic_hard_failure: bool,
        region_level: RegionLevel,
        metric: String,
        observed_millionths: u32,
        expected_profile_threshold_millionths: i64,
        ai_severity: Option<AiSeverity>,
        expected_state: GateState,
    }

    fn thresholds(value: u32) -> GateRegionThreshold {
        GateRegionThreshold {
            max_raw_changed_ratio_millionths: value,
            max_alpha_changed_ratio_millionths: value,
            max_tolerated_changed_ratio_millionths: value,
            minimum_ssim_millionths: 1_000_000 - value as i32,
            max_geometry_changed_ratio_millionths: value,
            max_large_area_ratio_millionths: value,
        }
    }

    fn metrics(value: u32) -> GateRegionMetricsBreakdown {
        GateRegionMetricsBreakdown {
            raw_changed_ratio_millionths: value,
            alpha_changed_ratio_millionths: value,
            tolerated_changed_ratio_millionths: value,
            ssim_millionths: 1_000_000 - value as i32,
            geometry_changed_ratio_millionths: value,
            large_area_ratio_millionths: value,
        }
    }

    #[test]
    fn inclusive_threshold_accepts_equal_and_rejects_one_above_or_below() {
        assert!(threshold_violations(&metrics(50_000), &thresholds(50_000)).is_empty());
        assert_eq!(
            threshold_violations(&metrics(50_001), &thresholds(50_000)).len(),
            6
        );
        let mut below_ssim = metrics(50_000);
        below_ssim.ssim_millionths -= 1;
        assert_eq!(
            threshold_violations(&below_ssim, &thresholds(50_000))
                .iter()
                .filter(|item| item.metric == "ssim_millionths")
                .count(),
            1
        );
    }

    #[test]
    fn priority_keeps_hard_failures_and_critical_regions_above_ai() {
        let reasons = vec![
            reason(GateFailureType::AiSevereIssue, "ai", true, "ai"),
            reason(
                GateFailureType::CriticalRegionFailure,
                "critical",
                true,
                "critical",
            ),
            reason(
                GateFailureType::SemanticHardFailure,
                "semantic",
                true,
                "semantic",
            ),
            reason(
                GateFailureType::DimensionMismatch,
                "dimension",
                true,
                "dimension",
            ),
        ];
        assert_eq!(
            classify_reasons(&reasons),
            (GateState::Failed, GateFailureType::DimensionMismatch)
        );
    }

    #[test]
    fn decorative_failure_is_review_while_minor_ai_is_report_only() {
        let review = vec![reason(
            GateFailureType::DecorativeRegionReview,
            "frame",
            false,
            "review",
        )];
        assert_eq!(
            classify_reasons(&review),
            (
                GateState::NeedsReview,
                GateFailureType::DecorativeRegionReview
            )
        );
        assert_eq!(
            classify_reasons(&[]),
            (GateState::Passed, GateFailureType::None)
        );
        assert_eq!(
            ai_gate_rule(AiSeverity::Severe),
            Some((GateFailureType::AiSevereIssue, true))
        );
        assert_eq!(
            ai_gate_rule(AiSeverity::Medium),
            Some((GateFailureType::AiMediumIssue, true))
        );
        assert_eq!(ai_gate_rule(AiSeverity::Minor), None);
        assert_eq!(
            region_gate_rule(RegionLevel::Critical),
            (GateFailureType::CriticalRegionFailure, true)
        );
        assert_eq!(
            region_gate_rule(RegionLevel::Normal),
            (GateFailureType::NormalRegionFailure, true)
        );
        assert_eq!(
            region_gate_rule(RegionLevel::Decorative),
            (GateFailureType::DecorativeRegionReview, false)
        );
    }

    #[test]
    fn independent_failures_are_never_hidden_by_a_quality_average() {
        let failed = metrics(50_001);
        let passed = metrics(0);
        assert!(!threshold_violations(&failed, &thresholds(50_000)).is_empty());
        assert!(threshold_violations(&passed, &thresholds(50_000)).is_empty());
        assert!(diagnostic_quality_floor(&failed) < diagnostic_quality_floor(&passed));
    }

    #[test]
    fn config_rejects_profiles_that_make_critical_looser_than_normal() {
        let config = GateConfig {
            schema_version: VISUAL_GATE_CONFIG_SCHEMA_VERSION,
            algorithm_version: VISUAL_GATE_ALGORITHM_VERSION.to_owned(),
            conservative_default: GateThresholdProfiles {
                critical: thresholds(20),
                normal: thresholds(10),
                decorative: thresholds(30),
            },
            reference_profiles: Vec::new(),
        };
        assert_eq!(
            validate_config(&config).unwrap_err().failure.code,
            ComparisonErrorCode::GateConfigInvalid
        );
    }

    #[test]
    fn committed_conservative_config_is_strict_and_valid() {
        let config: GateConfig =
            serde_json::from_str(include_str!("../fixtures/gate/conservative.config.json"))
                .unwrap();
        validate_config(&config).unwrap();
        assert_eq!(config.reference_profiles.len(), 1);
        assert_eq!(
            config.reference_profiles[0].calibration_fixture_id,
            "stage09-human-labeled-boundaries-v1"
        );
    }

    #[test]
    fn bundle_requires_stage_eight_capture_identity_and_rejects_duplicates() {
        let report = BoundGateReport {
            path: "inputs/report.json".to_owned(),
            sha256: "0".repeat(64),
        };
        let capture = GateCaptureInput {
            capture_id: "login.compact.initial".to_owned(),
            screen: "login".to_owned(),
            device: "compact".to_owned(),
            state: "initial".to_owned(),
            reference_profile: "fixture".to_owned(),
            reference_binding: ReferenceBinding {
                sha256: "0".repeat(64),
                revision: 1,
            },
            diff_report: report.clone(),
            region_report: Some(report.clone()),
            semantic_report: report,
        };
        let mut inconsistent = GateBundle {
            schema_version: VISUAL_GATE_BUNDLE_SCHEMA_VERSION,
            run_id: "identity-fixture".to_owned(),
            captures: vec![capture.clone()],
            ai_report: None,
        };
        inconsistent.captures[0].capture_id = "other.compact.initial".to_owned();
        assert!(
            validate_bundle(&inconsistent)
                .unwrap_err()
                .failure
                .message
                .contains("must equal screen.device.state")
        );
        let duplicate = GateBundle {
            schema_version: VISUAL_GATE_BUNDLE_SCHEMA_VERSION,
            run_id: "identity-fixture".to_owned(),
            captures: vec![capture.clone(), capture],
            ai_report: None,
        };
        assert!(
            validate_bundle(&duplicate)
                .unwrap_err()
                .failure
                .message
                .contains("duplicate capture identity")
        );
    }

    #[test]
    fn human_labeled_boundary_fixture_has_no_recorded_false_positive_or_negative() {
        let fixture: CalibrationFixture =
            serde_json::from_str(include_str!("../fixtures/gate/human-labeled-cases.json"))
                .unwrap();
        let config: GateConfig =
            serde_json::from_str(include_str!("../fixtures/gate/conservative.config.json"))
                .unwrap();
        assert_eq!(fixture.schema_version, 1);
        assert_eq!(fixture.fixture_id, "stage09-human-labeled-boundaries-v1");
        assert_eq!(fixture.profile, "repository-fixture-balanced");
        assert!(fixture.scope.contains("not a user study"));
        assert!(!fixture.adjustment_rationale.is_empty());
        assert!(
            fixture
                .false_positive_definition
                .contains("more restrictive")
        );
        assert!(
            fixture
                .false_negative_definition
                .contains("less restrictive")
        );
        let matching_profiles = config
            .reference_profiles
            .iter()
            .filter(|profile| profile.profile_id == fixture.profile)
            .collect::<Vec<_>>();
        assert_eq!(matching_profiles.len(), 1);
        let profile = matching_profiles[0];
        assert_eq!(profile.calibration_fixture_id, fixture.fixture_id);
        assert_eq!(
            config
                .reference_profiles
                .iter()
                .filter(|candidate| candidate.calibration_fixture_id == fixture.fixture_id)
                .count(),
            1
        );
        let mut false_positives = 0;
        let mut false_negatives = 0;
        let mut misclassifications = 0;
        let mut classified = Vec::new();
        for case in &fixture.cases {
            assert!(!case.case_id.is_empty());
            let (configured_threshold, minimum) = calibration_metric_threshold(
                profile.thresholds.for_level(case.region_level),
                &case.metric,
            )
            .unwrap_or_else(|| panic!("unsupported calibration metric in {}", case.case_id));
            assert_eq!(
                configured_threshold, case.expected_profile_threshold_millionths,
                "formal profile threshold drifted for {}",
                case.case_id
            );
            let mut reasons = Vec::new();
            if case.dimension_hard_failure {
                reasons.push(reason(
                    GateFailureType::DimensionMismatch,
                    "dimensions",
                    true,
                    "fixture",
                ));
            }
            if case.semantic_hard_failure {
                reasons.push(reason(
                    GateFailureType::SemanticHardFailure,
                    "semantic",
                    true,
                    "fixture",
                ));
            }
            let observed = i64::from(case.observed_millionths);
            if (minimum && observed < configured_threshold)
                || (!minimum && observed > configured_threshold)
            {
                let (failure_type, blocking) = region_gate_rule(case.region_level);
                reasons.push(reason(failure_type, "region", blocking, "fixture"));
            }
            if let Some((failure_type, blocking)) = case.ai_severity.and_then(ai_gate_rule) {
                reasons.push(reason(failure_type, "ai", blocking, "fixture"));
            }
            let actual = classify_reasons(&reasons).0;
            if actual != case.expected_state {
                misclassifications += 1;
                if gate_state_rank(actual) > gate_state_rank(case.expected_state) {
                    false_positives += 1;
                } else {
                    false_negatives += 1;
                }
            }
            classified.push((case.case_id.as_str(), actual, case.expected_state));
        }
        for (case_id, actual, expected) in classified {
            assert_eq!(
                actual, expected,
                "four-state calibration mismatch: {case_id}"
            );
        }
        assert_eq!(false_positives, fixture.false_positive_count);
        assert_eq!(false_negatives, fixture.false_negative_count);
        assert_eq!(misclassifications, fixture.misclassification_count);
        assert_eq!(
            (false_positives, false_negatives, misclassifications),
            (0, 0, 0)
        );
    }

    fn calibration_metric_threshold(
        threshold: &GateRegionThreshold,
        metric: &str,
    ) -> Option<(i64, bool)> {
        match metric {
            "raw_changed_ratio_millionths" => {
                Some((i64::from(threshold.max_raw_changed_ratio_millionths), false))
            }
            "alpha_changed_ratio_millionths" => Some((
                i64::from(threshold.max_alpha_changed_ratio_millionths),
                false,
            )),
            "tolerated_changed_ratio_millionths" => Some((
                i64::from(threshold.max_tolerated_changed_ratio_millionths),
                false,
            )),
            "ssim_millionths" => Some((i64::from(threshold.minimum_ssim_millionths), true)),
            "geometry_changed_ratio_millionths" => Some((
                i64::from(threshold.max_geometry_changed_ratio_millionths),
                false,
            )),
            "large_area_ratio_millionths" => {
                Some((i64::from(threshold.max_large_area_ratio_millionths), false))
            }
            _ => None,
        }
    }

    fn gate_state_rank(state: GateState) -> u8 {
        match state {
            GateState::Passed => 0,
            GateState::NeedsReview => 1,
            GateState::Failed => 2,
            GateState::Invalid => 3,
        }
    }
}
