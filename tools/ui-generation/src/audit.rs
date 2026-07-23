//! Deterministic audit matrix for a declared `UiDocument` page series.
//!
//! This is intentionally a wrapper around the feature-gated standalone preview process. The
//! process uses the project's normal declarative runtime and screenshot path, while this module
//! records the `screen × device × state` matrix without registering any generation capability in
//! the game plugin or Android library build.

use crate::{
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    preview::{
        CommandPreviewExecutor, PreviewExecutor, PreviewFailureKind, PreviewRunResult,
        PreviewRunStatus, prepare_preview_command_for_state, run_preview,
    },
};
use project::framework::ui::document::tooling::validate_json_bytes;
use project::framework::ui::document::{
    UiDocumentInputMode, UiDocumentPlatform, UiPageState, UiSafeAreaClass, UiTargetProfile,
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

pub const UI_DOCUMENT_AUDIT_SCREEN: &str = "ui_document_preview";
/// A standalone Bevy preview can occasionally lose its swap-chain before it writes any formal
/// evidence. Keep the retry bound small and record every attempt so an audit stays reproducible.
pub const MAX_AUDIT_CAPTURE_ATTEMPTS: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditDevice {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Matches the Stage 11 desktop Runner matrix in logical pixels. It does not imply Android or
/// physical-density validation; those require a real-device metadata contract.
pub const DEFAULT_AUDIT_DEVICES: [AuditDevice; 4] = [
    AuditDevice {
        name: "phone-small",
        width: 360,
        height: 800,
    },
    AuditDevice {
        name: "phone-portrait",
        width: 394,
        height: 853,
    },
    AuditDevice {
        name: "tablet-portrait",
        width: 800,
        height: 1280,
    },
    AuditDevice {
        name: "tablet-landscape",
        width: 1280,
        height: 800,
    },
];

/// An opt-in assertion for fixtures whose visible states are deliberately expected to render
/// differently. Production audits may legitimately contain visually identical states, so this is
/// never inferred from a page-state ID.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditVisualExpectation {
    #[serde(default)]
    pub distinct_from_initial: Vec<UiPageState>,
}

impl AuditVisualExpectation {
    pub fn distinct_from_initial(states: Vec<UiPageState>) -> Result<Self, TaskFailure> {
        let expectation = Self {
            distinct_from_initial: states,
        };
        expectation.validate_definition()?;
        Ok(expectation)
    }

    fn validate_definition(&self) -> Result<(), TaskFailure> {
        let expected = self
            .distinct_from_initial
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        if expected.len() != self.distinct_from_initial.len()
            || expected.contains(UiPageState::initial().as_str())
        {
            return Err(invalid(
                "visual audit expectations must name unique non-initial states",
            ));
        }
        Ok(())
    }

    fn validate(&self, states: &[UiPageState]) -> Result<(), TaskFailure> {
        self.validate_definition()?;
        if self
            .distinct_from_initial
            .iter()
            .any(|state| !states.contains(state))
            || (!self.distinct_from_initial.is_empty() && !states.contains(&UiPageState::initial()))
        {
            return Err(invalid(
                "visual audit expectations must be included with an initial baseline capture",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditVisualExpectationFailure {
    pub code: String,
    pub device: String,
    pub state: String,
    pub initial_sha256: String,
    pub state_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditPreviewAttempt {
    pub number: u8,
    pub output_directory: PathBuf,
    pub preview: PreviewRunResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCapture {
    pub screen: String,
    pub device: String,
    pub state: String,
    pub output_directory: PathBuf,
    /// The successful attempt used as this capture's evidence. Failed captures deliberately
    /// leave this unset rather than presenting their final failed attempt as selected evidence.
    pub selected_attempt: Option<u8>,
    pub attempts: Vec<AuditPreviewAttempt>,
}

impl AuditCapture {
    fn selected_preview(&self) -> Option<&PreviewRunResult> {
        self.selected_attempt.and_then(|number| {
            self.attempts
                .iter()
                .find(|attempt| attempt.number == number)
                .map(|attempt| &attempt.preview)
        })
    }

    fn passed(&self) -> bool {
        self.selected_preview()
            .is_some_and(|preview| preview.status == PreviewRunStatus::Passed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMatrixStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditMatrixResult {
    pub status: AuditMatrixStatus,
    pub screen: String,
    pub output_directory: PathBuf,
    pub manifest_path: PathBuf,
    pub visual_expectation: AuditVisualExpectation,
    pub visual_expectation_failures: Vec<AuditVisualExpectationFailure>,
    pub captures: Vec<AuditCapture>,
}

pub fn parse_page_states(input: Option<&str>) -> Result<Vec<UiPageState>, TaskFailure> {
    let values = input
        .map(|value| value.split(',').map(str::trim).collect::<Vec<_>>())
        .unwrap_or_else(|| vec!["initial"]);
    let mut states = Vec::new();
    let mut seen = BTreeSet::new();
    for value in values {
        let state = UiPageState::from_str(value)
            .map_err(|_| invalid("audit states must be closed UiDocument page-state IDs"))?;
        if !seen.insert(state.to_string()) {
            return Err(invalid("audit states must not contain duplicates"));
        }
        states.push(state);
    }
    if states.is_empty() {
        return Err(invalid("audit needs at least one page state"));
    }
    Ok(states)
}

pub fn run_document_audit(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    states: &[UiPageState],
    executor: &dyn PreviewExecutor,
    cancellation: &CancellationToken,
) -> Result<AuditMatrixResult, TaskFailure> {
    run_document_audit_with_expectation(
        repository_root,
        document_path,
        output_directory,
        states,
        &AuditVisualExpectation::default(),
        executor,
        cancellation,
    )
}

pub fn run_document_audit_with_expectation(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    states: &[UiPageState],
    visual_expectation: &AuditVisualExpectation,
    executor: &dyn PreviewExecutor,
    cancellation: &CancellationToken,
) -> Result<AuditMatrixResult, TaskFailure> {
    if states.is_empty() || output_directory.exists() || output_directory.as_os_str().is_empty() {
        return Err(invalid(
            "audit output directory must be new and audit states must be nonempty",
        ));
    }
    visual_expectation.validate(states)?;
    validate_audit_states(document_path, states)?;
    fs::create_dir(output_directory)
        .map_err(|_| invalid("audit output directory could not be created"))?;

    let mut captures = Vec::new();
    for device in DEFAULT_AUDIT_DEVICES {
        for state in states {
            cancellation.checkpoint()?;
            let capture_name = format!("{}--{}", device.name, state.as_str().replace('.', "_"));
            let capture_directory = output_directory.join(capture_name);
            fs::create_dir(&capture_directory)
                .map_err(|_| invalid("audit capture directory could not be created"))?;
            let mut attempts = Vec::new();
            let mut selected_attempt = None;
            for number in 1..=MAX_AUDIT_CAPTURE_ATTEMPTS {
                cancellation.checkpoint()?;
                let attempt_directory = capture_directory.join(format!("attempt-{number:02}"));
                let plan = prepare_preview_command_for_state(
                    repository_root,
                    document_path,
                    &attempt_directory,
                    device.width,
                    device.height,
                    state,
                )?;
                let preview = run_preview(plan, executor, cancellation);
                let cancelled = preview.process.cancelled
                    || preview
                        .failure
                        .as_ref()
                        .is_some_and(|failure| failure.kind == PreviewFailureKind::Cancelled);
                let retryable = retryable_missing_preview_evidence(&preview);
                let passed = preview.status == PreviewRunStatus::Passed;
                attempts.push(AuditPreviewAttempt {
                    number,
                    output_directory: attempt_directory,
                    preview,
                });
                if cancelled {
                    return Err(TaskFailure::new(
                        TaskFailureKind::Cancelled,
                        "standalone audit preview was cancelled",
                        None,
                    ));
                }
                cancellation.checkpoint()?;
                if passed {
                    selected_attempt = Some(number);
                    break;
                }
                if !retryable {
                    break;
                }
            }
            captures.push(AuditCapture {
                screen: UI_DOCUMENT_AUDIT_SCREEN.to_owned(),
                device: device.name.to_owned(),
                state: state.to_string(),
                output_directory: capture_directory,
                selected_attempt,
                attempts,
            });
        }
    }
    cancellation.checkpoint()?;
    let visual_expectation_failures = validate_visual_expectation(&captures, visual_expectation);
    let status =
        if captures.iter().all(AuditCapture::passed) && visual_expectation_failures.is_empty() {
            AuditMatrixStatus::Passed
        } else {
            AuditMatrixStatus::Failed
        };
    let manifest_path = output_directory.join("audit-manifest.json");
    let result = AuditMatrixResult {
        status,
        screen: UI_DOCUMENT_AUDIT_SCREEN.to_owned(),
        output_directory: output_directory.to_path_buf(),
        manifest_path: manifest_path.clone(),
        visual_expectation: visual_expectation.clone(),
        visual_expectation_failures,
        captures,
    };
    write_new_json(&manifest_path, &result)?;
    Ok(result)
}

fn retryable_missing_preview_evidence(preview: &PreviewRunResult) -> bool {
    if preview.status != PreviewRunStatus::Failed || preview.process.cancelled {
        return false;
    }
    let Some(failure) = preview.failure.as_ref() else {
        return false;
    };
    matches!(
        (failure.kind, failure.code.as_str()),
        (
            PreviewFailureKind::ResultMissing,
            "UI_GENERATION_PREVIEW_LOG_MISSING" | "UI_GENERATION_PREVIEW_RESULT_MISSING"
        ) | (
            PreviewFailureKind::ScreenshotMissing,
            "UI_GENERATION_PREVIEW_SCREENSHOT_MISSING"
        )
    )
}

/// CLI helper kept separate so tests can run with a fixture executor instead of spawning Bevy.
pub fn run_document_audit_command(
    repository_root: &Path,
    document_path: &Path,
    output_directory: &Path,
    states: &[UiPageState],
    visual_expectation: &AuditVisualExpectation,
) -> Result<AuditMatrixResult, TaskFailure> {
    run_document_audit_with_expectation(
        repository_root,
        document_path,
        output_directory,
        states,
        visual_expectation,
        &CommandPreviewExecutor,
        &CancellationToken::default(),
    )
}

fn validate_visual_expectation(
    captures: &[AuditCapture],
    visual_expectation: &AuditVisualExpectation,
) -> Vec<AuditVisualExpectationFailure> {
    let mut failures = Vec::new();
    for device in DEFAULT_AUDIT_DEVICES {
        let initial = captures.iter().find(|capture| {
            capture.device == device.name
                && capture.state == UiPageState::initial().as_str()
                && capture.passed()
        });
        let Some(initial) = initial else {
            continue;
        };
        let Some(initial_sha256) = initial
            .selected_preview()
            .and_then(|preview| preview.screenshot_sha256.as_ref())
        else {
            continue;
        };
        for state in &visual_expectation.distinct_from_initial {
            let capture = captures.iter().find(|capture| {
                capture.device == device.name && capture.state == state.as_str() && capture.passed()
            });
            let Some(capture) = capture else {
                continue;
            };
            let Some(state_sha256) = capture
                .selected_preview()
                .and_then(|preview| preview.screenshot_sha256.as_ref())
            else {
                continue;
            };
            if state_sha256 == initial_sha256 {
                failures.push(AuditVisualExpectationFailure {
                    code: "UI_AUDIT_EXPECTED_VISUAL_DIFFERENCE_MISSING".to_owned(),
                    device: device.name.to_owned(),
                    state: state.to_string(),
                    initial_sha256: initial_sha256.clone(),
                    state_sha256: state_sha256.clone(),
                });
            }
        }
    }
    failures
}

fn validate_audit_states(document_path: &Path, states: &[UiPageState]) -> Result<(), TaskFailure> {
    let bytes = fs::read(document_path).map_err(|_| invalid("audit document cannot be read"))?;
    let validated = validate_json_bytes(&bytes)
        .validated()
        .cloned()
        .ok_or_else(|| invalid("audit document failed formal UiDocument validation"))?;
    for device in DEFAULT_AUDIT_DEVICES {
        let profile = UiTargetProfile::new(
            device.width as f32,
            device.height as f32,
            UiSafeAreaClass::None,
            UiDocumentInputMode::MouseKeyboard,
            UiDocumentPlatform::Windows,
        )
        .map_err(|_| invalid("audit device profile is invalid"))?;
        for state in states {
            validated
                .effective_document(&profile, state)
                .map_err(|error| {
                    invalid(format!(
                        "audit state `{state}` is not available for {}: {}",
                        device.name,
                        error.code()
                    ))
                })?;
        }
    }
    Ok(())
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), TaskFailure> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| invalid("audit manifest could not be serialized"))?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| invalid("audit manifest must not overwrite existing evidence"))?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| invalid("audit manifest could not be written"))
}

fn invalid(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::InvalidInput, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{PreviewCommandPlan, PreviewFailure, PreviewProcessRecord};
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    use std::sync::Mutex;

    struct FixtureExecutor {
        captures: Mutex<Vec<(String, u32, u32)>>,
        state_specific_pixels: bool,
    }

    impl FixtureExecutor {
        fn state_specific() -> Self {
            Self {
                captures: Mutex::new(Vec::new()),
                state_specific_pixels: true,
            }
        }

        fn identical() -> Self {
            Self {
                captures: Mutex::new(Vec::new()),
                state_specific_pixels: false,
            }
        }
    }

    impl PreviewExecutor for FixtureExecutor {
        fn execute(
            &self,
            plan: &PreviewCommandPlan,
            _cancellation: &CancellationToken,
        ) -> Result<PreviewProcessRecord, PreviewFailure> {
            self.captures
                .lock()
                .unwrap()
                .push((plan.page_state.clone(), plan.width, plan.height));
            let color = if self.state_specific_pixels {
                match plan.page_state.as_str() {
                    "initial" => [12, 20, 28, 255],
                    "loading" => [37, 99, 235, 255],
                    "empty" => [71, 85, 105, 255],
                    "error" => [180, 35, 24, 255],
                    "fixture.selected" => [22, 163, 74, 255],
                    "fixture.disabled" => [100, 116, 139, 255],
                    "fixture.modal" => [23, 42, 69, 255],
                    _ => [0, 0, 0, 255],
                }
            } else {
                [0, 0, 0, 255]
            };
            let mut pixels = vec![0_u8; (plan.width * plan.height * 4) as usize];
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&color);
            }
            let mut png = Vec::new();
            PngEncoder::new(&mut png)
                .write_image(&pixels, plan.width, plan.height, ExtendedColorType::Rgba8)
                .unwrap();
            fs::write(&plan.screenshot_path, png).unwrap();
            fs::write(&plan.log_path, b"fixture").unwrap();
            let result = serde_json::json!({
                "protocol_version": 1, "status": "passed", "document_id": "audit.fixture",
                "canonical_document_sha256": plan.canonical_document_sha256, "width": plan.width, "height": plan.height,
                "page_state": plan.page_state, "elapsed_frames": 60, "stable_frames": 30,
                "screenshot_path": plan.screenshot_path.to_string_lossy(), "captured_size": [plan.width, plan.height], "failure": null
            });
            fs::write(&plan.result_path, serde_json::to_vec(&result).unwrap()).unwrap();
            Ok(PreviewProcessRecord {
                exit_code: Some(0),
                timed_out: false,
                cancelled: false,
                elapsed_ms: 1,
            })
        }
    }

    #[derive(Clone, Copy)]
    enum RetryFixtureMode {
        MissingResultThenSuccess,
        MissingScreenshotThenSuccess,
        AlwaysMissingResult,
        SemanticMismatch,
    }

    struct RetryFixtureExecutor {
        mode: RetryFixtureMode,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl RetryFixtureExecutor {
        fn new(mode: RetryFixtureMode) -> Self {
            Self {
                mode,
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl PreviewExecutor for RetryFixtureExecutor {
        fn execute(
            &self,
            plan: &PreviewCommandPlan,
            _cancellation: &CancellationToken,
        ) -> Result<PreviewProcessRecord, PreviewFailure> {
            self.calls
                .lock()
                .unwrap()
                .push(plan.screenshot_path.parent().unwrap().to_path_buf());
            fs::write(&plan.log_path, b"fixture").unwrap();
            let is_first_attempt = plan
                .screenshot_path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "attempt-01");
            if matches!(self.mode, RetryFixtureMode::AlwaysMissingResult)
                || (matches!(self.mode, RetryFixtureMode::MissingResultThenSuccess)
                    && is_first_attempt)
            {
                return Ok(fixture_process_record());
            }

            let missing_screenshot =
                matches!(self.mode, RetryFixtureMode::MissingScreenshotThenSuccess)
                    && is_first_attempt;
            if !missing_screenshot {
                let mut pixels = vec![0_u8; (plan.width * plan.height * 4) as usize];
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.copy_from_slice(&[12, 20, 28, 255]);
                }
                let mut png = Vec::new();
                PngEncoder::new(&mut png)
                    .write_image(&pixels, plan.width, plan.height, ExtendedColorType::Rgba8)
                    .unwrap();
                fs::write(&plan.screenshot_path, png).unwrap();
            }
            let width = if matches!(self.mode, RetryFixtureMode::SemanticMismatch) {
                plan.width + 1
            } else {
                plan.width
            };
            let result = serde_json::json!({
                "protocol_version": 1, "status": "passed", "document_id": "audit.fixture",
                "canonical_document_sha256": plan.canonical_document_sha256, "width": width, "height": plan.height,
                "page_state": plan.page_state, "elapsed_frames": 60, "stable_frames": 30,
                "screenshot_path": plan.screenshot_path.to_string_lossy(), "captured_size": [plan.width, plan.height], "failure": null
            });
            fs::write(&plan.result_path, serde_json::to_vec(&result).unwrap()).unwrap();
            Ok(fixture_process_record())
        }
    }

    struct CancellingExecutor {
        calls: Mutex<u8>,
    }

    impl PreviewExecutor for CancellingExecutor {
        fn execute(
            &self,
            plan: &PreviewCommandPlan,
            cancellation: &CancellationToken,
        ) -> Result<PreviewProcessRecord, PreviewFailure> {
            *self.calls.lock().unwrap() += 1;
            cancellation.request();
            fs::write(&plan.log_path, b"fixture").unwrap();
            Ok(fixture_process_record())
        }
    }

    fn fixture_process_record() -> PreviewProcessRecord {
        PreviewProcessRecord {
            exit_code: Some(0),
            timed_out: false,
            cancelled: false,
            elapsed_ms: 1,
        }
    }

    fn fixture_document() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/audit/phone_tablet_multi_state.valid.json")
    }

    fn all_fixture_states() -> Vec<UiPageState> {
        parse_page_states(Some(
            "initial,loading,empty,error,fixture.selected,fixture.disabled,fixture.modal",
        ))
        .unwrap()
    }

    fn fixture_visual_expectation() -> AuditVisualExpectation {
        AuditVisualExpectation::distinct_from_initial(
            all_fixture_states()
                .into_iter()
                .filter(|state| state != &UiPageState::initial())
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn audit_matrix_captures_every_device_and_visible_state_with_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let document = fixture_document();
        let states = all_fixture_states();
        let visual_expectation = fixture_visual_expectation();
        let executor = FixtureExecutor::state_specific();
        let result = run_document_audit_with_expectation(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &document,
            &directory.path().join("audit"),
            &states,
            &visual_expectation,
            &executor,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(result.status, AuditMatrixStatus::Passed);
        assert_eq!(result.captures.len(), 28);
        assert_eq!(result.visual_expectation, visual_expectation);
        assert!(result.visual_expectation_failures.is_empty());
        assert!(result.captures.iter().all(|capture| {
            capture.screen == UI_DOCUMENT_AUDIT_SCREEN
                && capture.selected_attempt == Some(1)
                && capture.attempts.len() == 1
                && capture.selected_preview().is_some_and(|preview| {
                    preview.command.screenshot_path.is_file()
                        && preview.command.result_path.is_file()
                        && preview.screenshot_sha256.is_some()
                        && preview.screenshot_bytes.is_some()
                })
        }));
        for capture in &result.captures {
            let preview = capture.selected_preview().unwrap();
            assert_eq!(
                image::image_dimensions(&preview.command.screenshot_path).unwrap(),
                (preview.command.width, preview.command.height)
            );
            let metadata: serde_json::Value =
                serde_json::from_slice(&fs::read(&preview.command.result_path).unwrap()).unwrap();
            assert_eq!(
                metadata["page_state"].as_str(),
                Some(capture.state.as_str())
            );
        }
        assert!(result.manifest_path.is_file());
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&result.manifest_path).unwrap()).unwrap();
        assert_eq!(manifest["captures"].as_array().unwrap().len(), 28);
        assert!(
            manifest["visual_expectation_failures"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        for device in DEFAULT_AUDIT_DEVICES {
            let initial_hash = result
                .captures
                .iter()
                .find(|capture| {
                    capture.device == device.name
                        && capture.state == UiPageState::initial().as_str()
                })
                .and_then(|capture| capture.selected_preview())
                .and_then(|preview| preview.screenshot_sha256.as_deref())
                .unwrap();
            for state in &visual_expectation.distinct_from_initial {
                let state_hash = result
                    .captures
                    .iter()
                    .find(|capture| {
                        capture.device == device.name && capture.state == state.as_str()
                    })
                    .and_then(|capture| capture.selected_preview())
                    .and_then(|preview| preview.screenshot_sha256.as_deref())
                    .unwrap();
                assert_ne!(state_hash, initial_hash, "{} / {state}", device.name);
            }
        }
        assert_eq!(executor.captures.lock().unwrap().len(), 28);
    }

    #[test]
    fn explicit_visual_expectation_rejects_identical_claimed_fixture_states() {
        let directory = tempfile::tempdir().unwrap();
        let states = parse_page_states(Some("initial,loading")).unwrap();
        let visual_expectation =
            AuditVisualExpectation::distinct_from_initial(vec![UiPageState::loading()]).unwrap();
        let result = run_document_audit_with_expectation(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &fixture_document(),
            &directory.path().join("audit"),
            &states,
            &visual_expectation,
            &FixtureExecutor::identical(),
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(result.status, AuditMatrixStatus::Failed);
        assert_eq!(
            result.visual_expectation_failures.len(),
            DEFAULT_AUDIT_DEVICES.len()
        );
        assert!(
            result
                .visual_expectation_failures
                .iter()
                .all(|failure| failure.code == "UI_AUDIT_EXPECTED_VISUAL_DIFFERENCE_MISSING")
        );
    }

    #[test]
    fn missing_result_evidence_retries_once_in_a_fresh_attempt_directory() {
        let directory = tempfile::tempdir().unwrap();
        let executor = RetryFixtureExecutor::new(RetryFixtureMode::MissingResultThenSuccess);
        let result = run_document_audit(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &fixture_document(),
            &directory.path().join("audit"),
            &parse_page_states(Some("initial")).unwrap(),
            &executor,
            &CancellationToken::default(),
        )
        .unwrap();

        assert_eq!(result.status, AuditMatrixStatus::Passed);
        assert!(result.captures.iter().all(|capture| {
            capture.selected_attempt == Some(2)
                && capture.attempts.len() == 2
                && capture.attempts[0]
                    .preview
                    .failure
                    .as_ref()
                    .is_some_and(|failure| {
                        failure.kind == PreviewFailureKind::ResultMissing
                            && failure.code == "UI_GENERATION_PREVIEW_RESULT_MISSING"
                    })
                && capture.attempts[1].preview.status == PreviewRunStatus::Passed
                && capture.attempts[0].output_directory != capture.attempts[1].output_directory
                && capture.attempts[0].output_directory.is_dir()
                && capture.attempts[1].output_directory.is_dir()
                && capture
                    .selected_preview()
                    .is_some_and(|preview| preview.command.screenshot_path.is_file())
        }));
        assert_eq!(
            executor.calls.lock().unwrap().len(),
            DEFAULT_AUDIT_DEVICES.len() * 2
        );
    }

    #[test]
    fn missing_screenshot_evidence_retries_once_in_a_fresh_attempt_directory() {
        let directory = tempfile::tempdir().unwrap();
        let executor = RetryFixtureExecutor::new(RetryFixtureMode::MissingScreenshotThenSuccess);
        let result = run_document_audit(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &fixture_document(),
            &directory.path().join("audit"),
            &parse_page_states(Some("initial")).unwrap(),
            &executor,
            &CancellationToken::default(),
        )
        .unwrap();

        assert_eq!(result.status, AuditMatrixStatus::Passed);
        assert!(result.captures.iter().all(|capture| {
            capture.selected_attempt == Some(2)
                && capture.attempts.len() == 2
                && capture.attempts[0]
                    .preview
                    .failure
                    .as_ref()
                    .is_some_and(|failure| {
                        failure.kind == PreviewFailureKind::ScreenshotMissing
                            && failure.code == "UI_GENERATION_PREVIEW_SCREENSHOT_MISSING"
                    })
                && capture.attempts[1].preview.status == PreviewRunStatus::Passed
                && capture.attempts[0].output_directory != capture.attempts[1].output_directory
        }));
        assert_eq!(
            executor.calls.lock().unwrap().len(),
            DEFAULT_AUDIT_DEVICES.len() * 2
        );
    }

    #[test]
    fn exhausted_missing_evidence_keeps_both_failed_attempts_in_the_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let executor = RetryFixtureExecutor::new(RetryFixtureMode::AlwaysMissingResult);
        let result = run_document_audit(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &fixture_document(),
            &directory.path().join("audit"),
            &parse_page_states(Some("initial")).unwrap(),
            &executor,
            &CancellationToken::default(),
        )
        .unwrap();

        assert_eq!(result.status, AuditMatrixStatus::Failed);
        assert!(result.manifest_path.is_file());
        assert!(result.captures.iter().all(|capture| {
            capture.selected_attempt.is_none()
                && capture.attempts.len() == 2
                && capture.attempts.iter().all(|attempt| {
                    attempt.preview.failure.as_ref().is_some_and(|failure| {
                        failure.kind == PreviewFailureKind::ResultMissing
                            && failure.code == "UI_GENERATION_PREVIEW_RESULT_MISSING"
                    })
                })
        }));
    }

    #[test]
    fn semantic_preview_evidence_mismatch_does_not_retry() {
        let directory = tempfile::tempdir().unwrap();
        let executor = RetryFixtureExecutor::new(RetryFixtureMode::SemanticMismatch);
        let result = run_document_audit(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &fixture_document(),
            &directory.path().join("audit"),
            &parse_page_states(Some("initial")).unwrap(),
            &executor,
            &CancellationToken::default(),
        )
        .unwrap();

        assert_eq!(result.status, AuditMatrixStatus::Failed);
        assert!(result.captures.iter().all(|capture| {
            capture.selected_attempt.is_none()
                && capture.attempts.len() == 1
                && capture.attempts[0]
                    .preview
                    .failure
                    .as_ref()
                    .is_some_and(|failure| {
                        failure.kind == PreviewFailureKind::EvidenceMismatch
                            && failure.code == "UI_GENERATION_PREVIEW_RESULT_MISMATCH"
                    })
        }));
        assert_eq!(
            executor.calls.lock().unwrap().len(),
            DEFAULT_AUDIT_DEVICES.len()
        );
    }

    #[test]
    fn cancellation_stops_the_audit_before_a_retry_or_next_capture() {
        let directory = tempfile::tempdir().unwrap();
        let executor = CancellingExecutor {
            calls: Mutex::new(0),
        };
        let failure = run_document_audit(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap(),
            &fixture_document(),
            &directory.path().join("audit"),
            &parse_page_states(Some("initial")).unwrap(),
            &executor,
            &CancellationToken::default(),
        )
        .unwrap_err();

        assert_eq!(failure.kind(), TaskFailureKind::Cancelled);
        assert_eq!(*executor.calls.lock().unwrap(), 1);
    }

    #[test]
    fn audit_state_parser_and_preflight_reject_unknown_state() {
        assert!(parse_page_states(Some("loading,loading")).is_err());
        let directory = tempfile::tempdir().unwrap();
        let document = fixture_document();
        let states = parse_page_states(Some("fixture.unknown")).unwrap();
        let executor = FixtureExecutor::state_specific();
        assert!(
            run_document_audit(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .canonicalize()
                    .unwrap(),
                &document,
                &directory.path().join("audit"),
                &states,
                &executor,
                &CancellationToken::default(),
            )
            .is_err()
        );
    }
}
