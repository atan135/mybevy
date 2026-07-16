//! Offline, repository-owned evaluation corpus and aggregate generation metrics.
//!
//! The corpus deliberately contains only synthetic text fixtures. It exercises the same formal
//! analysis/document contracts without committing user reference images, copied product art,
//! prompt transcripts, or model response transcripts.

use crate::{
    analysis::validate_analysis_json,
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    observability::{TaskBudget, TaskExecutionLimits, TaskUsageSnapshot},
    provider::{
        FixtureProvider, Provider, ProviderRegistry, ProviderRequest, ProviderRunner,
        StructuredOutputContract,
    },
};
use project::framework::ui::document::tooling::validate_json_bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};

pub const EVALUATION_CATALOG_VERSION: u32 = 1;
const MAX_EVALUATION_CASES: usize = 16;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCatalog {
    pub catalog_version: u32,
    pub source: EvaluationDatasetSource,
    pub cases: Vec<EvaluationCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationDatasetSource {
    pub kind: String,
    pub statement: String,
    pub license: String,
    pub contains_user_data: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCase {
    pub id: String,
    pub category: EvaluationCategory,
    pub fixture: EvaluationFixture,
    pub expected_components: Vec<String>,
    pub key_regions: Vec<String>,
    pub allowed_differences: Vec<String>,
    pub unsupported_capabilities: Vec<String>,
    pub viewports: Vec<EvaluationViewport>,
    pub states: Vec<String>,
    pub human_acceptance: HumanAcceptance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationCategory {
    Login,
    List,
    Hud,
    Modal,
    ArtPanel,
    ResponsiveState,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationFixture {
    /// Relative to `tools/ui-generation/fixtures/`, never to a user-provided directory.
    pub path: PathBuf,
    pub artifact_kind: EvaluationArtifactKind,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationArtifactKind {
    Analysis,
    GenerationEnvelope,
    UiDocument,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationViewport {
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanAcceptance {
    pub status: HumanAcceptanceStatus,
    /// A role, not a personal name or account identifier.
    pub reviewer_role: String,
    pub basis: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HumanAcceptanceStatus {
    Accepted,
    AcceptedWithLimitations,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationCaseStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCaseResult {
    pub case_id: String,
    pub status: EvaluationCaseStatus,
    pub first_validation_passed: bool,
    pub repair_rounds: u8,
    pub elapsed_ms: u64,
    /// Stable failure classification only; fixture contents and user-visible strings are omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationMetrics {
    pub case_count: usize,
    pub passed_count: usize,
    pub success_rate_percent: u32,
    pub first_validation_pass_rate_percent: u32,
    pub repair_rounds: u32,
    pub elapsed_ms: u64,
    pub provider_calls: u32,
    pub input_units: u64,
    pub output_units: u64,
    pub estimated_cost_microunits: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationReport {
    pub protocol_version: u32,
    pub dataset_kind: String,
    pub offline_fixture_provider: bool,
    pub online_provider_required: bool,
    pub metrics: EvaluationMetrics,
    pub cases: Vec<EvaluationCaseResult>,
}

impl EvaluationCatalog {
    pub fn load(path: &Path) -> Result<Self, TaskFailure> {
        let catalog: Self = serde_json::from_slice(
            &fs::read(path).map_err(|_| invalid("evaluation catalog cannot be read"))?,
        )
        .map_err(|_| invalid("evaluation catalog is malformed"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.catalog_version != EVALUATION_CATALOG_VERSION
            || self.source.kind != "repository_authored_synthetic_text_fixture"
            || self.source.statement.trim().is_empty()
            || self.source.license != "CC0-1.0"
            || self.source.contains_user_data
            || self.cases.is_empty()
            || self.cases.len() > MAX_EVALUATION_CASES
        {
            return Err(invalid("evaluation catalog provenance or size is invalid"));
        }
        let mut ids = BTreeSet::new();
        let categories = self
            .cases
            .iter()
            .map(|case| case.category.clone())
            .collect::<BTreeSet<_>>();
        let required = BTreeSet::from([
            EvaluationCategory::Login,
            EvaluationCategory::List,
            EvaluationCategory::Hud,
            EvaluationCategory::Modal,
            EvaluationCategory::ArtPanel,
            EvaluationCategory::ResponsiveState,
        ]);
        if categories != required {
            return Err(invalid(
                "evaluation catalog must cover login, list, HUD, modal, art panel, and responsive states",
            ));
        }
        for case in &self.cases {
            if !safe_label(&case.id, 96)
                || !ids.insert(case.id.clone())
                || !safe_relative_fixture_path(&case.fixture.path)
                || case.expected_components.is_empty()
                || case.key_regions.is_empty()
                || case.allowed_differences.is_empty()
                || case.unsupported_capabilities.is_empty()
                || case.viewports.is_empty()
                || case.states.is_empty()
                || case.human_acceptance.reviewer_role != "repository-maintainer"
                || case.human_acceptance.basis.trim().is_empty()
            {
                return Err(invalid(
                    "evaluation case coverage or acceptance record is invalid",
                ));
            }
            for component in &case.expected_components {
                if !safe_label(component, 96) {
                    return Err(invalid(
                        "evaluation component labels must be safe identifiers",
                    ));
                }
            }
            for viewport in &case.viewports {
                if !safe_label(&viewport.name, 64)
                    || viewport.width == 0
                    || viewport.height == 0
                    || viewport.width > 16_384
                    || viewport.height > 16_384
                {
                    return Err(invalid("evaluation viewport is invalid"));
                }
            }
            if case.states.iter().any(|state| !safe_label(state, 96)) {
                return Err(invalid("evaluation state IDs must be safe identifiers"));
            }
        }
        Ok(())
    }
}

/// Runs the corpus without network access. A FixtureProvider call verifies the provider boundary,
/// while every repository-owned structured fixture is revalidated by its formal contract.
pub fn run_fixture_evaluation(
    repository_root: &Path,
    catalog_path: &Path,
) -> Result<EvaluationReport, TaskFailure> {
    ensure_repository_root(repository_root)?;
    let expected_catalog = repository_root
        .join("tools")
        .join("ui-generation")
        .join("fixtures")
        .join("evaluation")
        .join("catalog.v1.json");
    if fs::canonicalize(catalog_path).ok() != fs::canonicalize(&expected_catalog).ok() {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "fixture evaluation only accepts the repository-owned catalog",
            None,
        ));
    }
    let catalog = EvaluationCatalog::load(catalog_path)?;
    let fixture_root = catalog_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid("evaluation catalog must live below the fixture root"))?;
    let provider_path = fixture_root.join("providers").join("valid.json");
    let provider = Arc::new(FixtureProvider::load(&provider_path)?);
    let provider_id = provider.descriptor().id.clone();
    let mut registry = ProviderRegistry::default();
    registry.register(provider)?;
    let runner = ProviderRunner::new(
        registry,
        crate::provider::ProviderExecutionPolicy {
            task_limits: TaskExecutionLimits {
                max_provider_calls: u32::try_from(catalog.cases.len())
                    .map_err(|_| invalid("evaluation catalog is too large"))?,
                max_images: catalog.cases.len(),
                ..TaskExecutionLimits::default()
            },
            ..crate::provider::ProviderExecutionPolicy::default()
        },
    )?;
    let budget = TaskBudget::new(runner.task_limits())?;
    let started = Instant::now();
    let mut results = Vec::with_capacity(catalog.cases.len());
    for case in &catalog.cases {
        let case_started = Instant::now();
        let validation = validate_case_fixture(fixture_root, case);
        let first_validation_passed = validation.is_ok();
        let provider_result = runner.execute_with_budget(
            &provider_id,
            fixture_request(&case.id)?,
            &CancellationToken::default(),
            &budget,
        );
        let outcome = validation.and(
            provider_result
                .map(|_| ())
                .map_err(|failure| failure.failure),
        );
        results.push(EvaluationCaseResult {
            case_id: case.id.clone(),
            status: if outcome.is_ok() {
                EvaluationCaseStatus::Passed
            } else {
                EvaluationCaseStatus::Failed
            },
            first_validation_passed,
            repair_rounds: 0,
            elapsed_ms: elapsed_ms(case_started),
            failure_code: outcome.err().map(|failure| safe_failure_code(&failure)),
        });
    }
    let usage = budget.snapshot();
    let report = EvaluationReport {
        protocol_version: EVALUATION_CATALOG_VERSION,
        dataset_kind: catalog.source.kind,
        offline_fixture_provider: true,
        online_provider_required: false,
        metrics: aggregate_metrics(&results, usage, elapsed_ms(started)),
        cases: results,
    };
    Ok(report)
}

fn ensure_repository_root(repository_root: &Path) -> Result<(), TaskFailure> {
    if !repository_root.join("project").join("Cargo.toml").is_file()
        || !repository_root
            .join("tools")
            .join("ui-generation")
            .join("Cargo.toml")
            .is_file()
    {
        return Err(invalid(
            "evaluation repository root is not the expected game repository",
        ));
    }
    Ok(())
}

fn fixture_request(case_id: &str) -> Result<ProviderRequest, TaskFailure> {
    ProviderRequest::visual_analysis(
        format!("evaluation-{case_id}"),
        "fixture-analysis-v1",
        "repository-authored fixture request",
        vec![crate::provider::ProviderImage::new(
            "synthetic-fixture",
            "image/png",
            Arc::<[u8]>::from(b"offline-fixture-image".as_slice()),
        )?],
        StructuredOutputContract::new("ui-reference-analysis", 1)?,
    )
}

fn validate_case_fixture(fixture_root: &Path, case: &EvaluationCase) -> Result<(), TaskFailure> {
    let path = fixture_root.join(&case.fixture.path);
    if path.parent().is_none() || !path.is_file() {
        return Err(invalid(
            "evaluation fixture is not a regular repository file",
        ));
    }
    let bytes = fs::read(path).map_err(|_| invalid("evaluation fixture cannot be read"))?;
    match case.fixture.artifact_kind {
        EvaluationArtifactKind::Analysis => {
            if validate_analysis_json(&bytes).valid {
                Ok(())
            } else {
                Err(invalid("evaluation analysis fixture failed validation"))
            }
        }
        EvaluationArtifactKind::GenerationEnvelope => {
            let envelope: Value = serde_json::from_slice(&bytes)
                .map_err(|_| invalid("evaluation generation envelope is malformed"))?;
            let document = envelope
                .get("document")
                .ok_or_else(|| invalid("evaluation generation envelope has no document"))?;
            let source = serde_json::to_vec(document)
                .map_err(|_| invalid("evaluation document cannot be serialized"))?;
            if validate_json_bytes(&source).report.valid {
                Ok(())
            } else {
                Err(invalid(
                    "evaluation generated document failed formal validation",
                ))
            }
        }
        EvaluationArtifactKind::UiDocument => {
            if validate_json_bytes(&bytes).report.valid {
                Ok(())
            } else {
                Err(invalid(
                    "evaluation UiDocument fixture failed formal validation",
                ))
            }
        }
    }
}

fn aggregate_metrics(
    cases: &[EvaluationCaseResult],
    usage: TaskUsageSnapshot,
    elapsed_ms: u64,
) -> EvaluationMetrics {
    let count = cases.len();
    let passed_count = cases
        .iter()
        .filter(|case| case.status == EvaluationCaseStatus::Passed)
        .count();
    let first_passed = cases
        .iter()
        .filter(|case| case.first_validation_passed)
        .count();
    EvaluationMetrics {
        case_count: count,
        passed_count,
        success_rate_percent: percent(passed_count, count),
        first_validation_pass_rate_percent: percent(first_passed, count),
        repair_rounds: cases.iter().map(|case| u32::from(case.repair_rounds)).sum(),
        elapsed_ms,
        provider_calls: usage.provider_calls,
        input_units: usage.input_units,
        output_units: usage.output_units,
        estimated_cost_microunits: usage.estimated_cost_microunits,
    }
}

fn percent(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        0
    } else {
        u32::try_from(numerator.saturating_mul(100) / denominator).unwrap_or(100)
    }
}

fn safe_failure_code(failure: &TaskFailure) -> String {
    // The error subject can contain a filesystem location in other code paths, so reports retain
    // only the stable error code plus explicitly controlled task-limit labels.
    match failure.subject() {
        Some(subject) if subject.starts_with("UI_GENERATION_LIMIT_") => subject.to_owned(),
        _ => failure.code().to_owned(),
    }
}

fn safe_relative_fixture_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn safe_label(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn invalid(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::InvalidInput, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/evaluation/catalog.v1.json")
    }

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn catalog_is_source_labeled_and_covers_required_fixture_cases() {
        let catalog = EvaluationCatalog::load(&catalog_path()).unwrap();
        assert_eq!(catalog.cases.len(), 6);
        assert!(
            catalog
                .cases
                .iter()
                .any(|case| case.category == EvaluationCategory::ResponsiveState
                    && case.viewports.len() >= 2
                    && case.states.len() >= 2)
        );
    }

    #[test]
    fn fixture_evaluation_collects_safe_metrics_without_online_provider() {
        let report = run_fixture_evaluation(&repository_root(), &catalog_path()).unwrap();
        assert_eq!(report.metrics.case_count, 6);
        assert_eq!(report.metrics.passed_count, 6);
        assert_eq!(report.metrics.success_rate_percent, 100);
        assert_eq!(report.metrics.first_validation_pass_rate_percent, 100);
        assert_eq!(report.metrics.provider_calls, 6);
        assert_eq!(report.metrics.input_units, 72);
        assert_eq!(report.metrics.output_units, 48);
        assert!(!report.online_provider_required);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("repository-authored fixture request"));
        assert!(!json.contains("offline-fixture-image"));
    }

    #[test]
    fn catalog_rejects_unlabeled_sources_and_escaping_fixture_paths() {
        let mut catalog = EvaluationCatalog::load(&catalog_path()).unwrap();
        catalog.source.contains_user_data = true;
        assert!(catalog.validate().is_err());
        catalog.source.contains_user_data = false;
        catalog.cases[0].fixture.path = PathBuf::from("../providers/valid.json");
        assert!(catalog.validate().is_err());
    }
}
