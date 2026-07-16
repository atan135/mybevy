//! Bounded task accounting and safe, reusable analysis evidence.
//!
//! This module deliberately records counts and stable identifiers only. Provider requests,
//! reference bytes, prompt text, raw provider output, account copy, and credentials are not
//! serializable observability artifacts.

use crate::{
    analysis::UiReferenceAnalysis,
    lifecycle::{TaskFailure, TaskFailureKind},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

pub const OBSERVABILITY_PROTOCOL_VERSION: u32 = 1;
const MAX_LIMIT_DURATION_MS: u64 = 60 * 60 * 1000;
const MAX_CACHE_PARAMETER_BYTES: usize = 16 * 1024;
const REDACTED: &str = "[REDACTED]";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskExecutionLimits {
    pub max_provider_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_images: usize,
    pub max_input_units: u64,
    pub max_output_units: u64,
    pub max_estimated_cost_microunits: u64,
    pub input_cost_microunits_per_1k: u64,
    pub output_cost_microunits_per_1k: u64,
}

impl TaskExecutionLimits {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.max_provider_calls == 0
            || self.max_elapsed_ms == 0
            || self.max_elapsed_ms > MAX_LIMIT_DURATION_MS
            || self.max_images == 0
            || self.max_input_units == 0
            || self.max_output_units == 0
            || self.max_estimated_cost_microunits == 0
        {
            return Err(TaskFailure::invalid(
                "task execution limits must use positive, bounded hard-stop values",
            ));
        }
        Ok(())
    }
}

impl Default for TaskExecutionLimits {
    fn default() -> Self {
        Self {
            max_provider_calls: 6,
            max_elapsed_ms: 5 * 60 * 1000,
            max_images: 12,
            max_input_units: 1_000_000,
            max_output_units: 250_000,
            max_estimated_cost_microunits: 10_000_000,
            input_cost_microunits_per_1k: 1_000,
            output_cost_microunits_per_1k: 2_000,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskUsageSnapshot {
    pub provider_calls: u32,
    pub images: usize,
    pub input_units: u64,
    pub output_units: u64,
    pub estimated_cost_microunits: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone)]
pub struct TaskBudget {
    limits: TaskExecutionLimits,
    started: Instant,
    usage: Arc<Mutex<TaskUsageSnapshot>>,
}

impl TaskBudget {
    pub fn new(limits: TaskExecutionLimits) -> Result<Self, TaskFailure> {
        limits.validate()?;
        Ok(Self {
            limits,
            started: Instant::now(),
            usage: Arc::new(Mutex::new(TaskUsageSnapshot::default())),
        })
    }

    pub fn limits(&self) -> &TaskExecutionLimits {
        &self.limits
    }

    pub fn snapshot(&self) -> TaskUsageSnapshot {
        let mut snapshot = self
            .usage
            .lock()
            .expect("task budget mutex poisoned")
            .clone();
        snapshot.elapsed_ms = elapsed_ms(self.started);
        snapshot
    }

    pub fn reserve_provider_attempt(&self, image_count: usize) -> Result<(), TaskFailure> {
        let elapsed = elapsed_ms(self.started);
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        usage.elapsed_ms = elapsed;
        if elapsed > self.limits.max_elapsed_ms {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_ELAPSED",
                "task elapsed-time limit reached",
            ));
        }
        if usage.provider_calls >= self.limits.max_provider_calls {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_PROVIDER_CALLS",
                "task provider-call limit reached",
            ));
        }
        let next_images = usage.images.checked_add(image_count).ok_or_else(|| {
            limit_failure("UI_GENERATION_LIMIT_IMAGES", "task image limit overflowed")
        })?;
        if next_images > self.limits.max_images {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_IMAGES",
                "task image limit reached",
            ));
        }
        usage.provider_calls += 1;
        usage.images = next_images;
        Ok(())
    }

    pub fn record_provider_usage(
        &self,
        input_units: Option<u64>,
        output_units: Option<u64>,
    ) -> Result<(), TaskFailure> {
        let input_units = input_units.unwrap_or(0);
        let output_units = output_units.unwrap_or(0);
        let input_cost = units_cost(input_units, self.limits.input_cost_microunits_per_1k)?;
        let output_cost = units_cost(output_units, self.limits.output_cost_microunits_per_1k)?;
        let cost = input_cost.checked_add(output_cost).ok_or_else(|| {
            limit_failure("UI_GENERATION_LIMIT_COST", "task estimated cost overflowed")
        })?;

        let elapsed = elapsed_ms(self.started);
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        usage.elapsed_ms = elapsed;
        usage.input_units = usage.input_units.checked_add(input_units).ok_or_else(|| {
            limit_failure(
                "UI_GENERATION_LIMIT_INPUT_UNITS",
                "task input-unit limit overflowed",
            )
        })?;
        usage.output_units = usage
            .output_units
            .checked_add(output_units)
            .ok_or_else(|| {
                limit_failure(
                    "UI_GENERATION_LIMIT_OUTPUT_UNITS",
                    "task output-unit limit overflowed",
                )
            })?;
        usage.estimated_cost_microunits = usage
            .estimated_cost_microunits
            .checked_add(cost)
            .ok_or_else(|| {
                limit_failure("UI_GENERATION_LIMIT_COST", "task estimated cost overflowed")
            })?;
        if elapsed > self.limits.max_elapsed_ms {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_ELAPSED",
                "task elapsed-time limit reached",
            ));
        }
        if usage.input_units > self.limits.max_input_units {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_INPUT_UNITS",
                "task input-unit limit reached",
            ));
        }
        if usage.output_units > self.limits.max_output_units {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_OUTPUT_UNITS",
                "task output-unit limit reached",
            ));
        }
        if usage.estimated_cost_microunits > self.limits.max_estimated_cost_microunits {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_COST",
                "task estimated-cost limit reached",
            ));
        }
        Ok(())
    }
}

fn units_cost(units: u64, rate_per_1k: u64) -> Result<u64, TaskFailure> {
    units
        .checked_mul(rate_per_1k)
        .and_then(|value| value.checked_add(999))
        .map(|value| value / 1000)
        .ok_or_else(|| limit_failure("UI_GENERATION_LIMIT_COST", "task estimated cost overflowed"))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn limit_failure(code: &str, message: &str) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::InvalidInput,
        message,
        Some(code.to_owned()),
    )
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisCacheIdentity {
    pub input_sha256: String,
    pub model_id: String,
    pub prompt_version: String,
    pub schema_id: String,
    pub schema_version: u32,
    pub parameters: Value,
}

impl AnalysisCacheIdentity {
    pub fn new(
        input_sha256: impl Into<String>,
        model_id: impl Into<String>,
        prompt_version: impl Into<String>,
        schema_id: impl Into<String>,
        schema_version: u32,
        parameters: Value,
    ) -> Result<Self, TaskFailure> {
        let identity = Self {
            input_sha256: input_sha256.into(),
            model_id: model_id.into(),
            prompt_version: prompt_version.into(),
            schema_id: schema_id.into(),
            schema_version,
            parameters,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn cache_key(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("validated cache identity serializes");
        hex_sha256(&bytes)
    }

    fn validate(&self) -> Result<(), TaskFailure> {
        if !is_sha256(&self.input_sha256)
            || !is_safe_label(&self.model_id, 128)
            || !is_safe_label(&self.prompt_version, 128)
            || !is_safe_label(&self.schema_id, 128)
            || self.schema_version == 0
        {
            return Err(TaskFailure::invalid(
                "analysis cache identity requires an input hash and safe model/prompt/schema labels",
            ));
        }
        let parameter_bytes = serde_json::to_vec(&self.parameters)
            .map_err(|_| TaskFailure::invalid("analysis cache parameters cannot be serialized"))?;
        if parameter_bytes.len() > MAX_CACHE_PARAMETER_BYTES
            || !safe_cache_parameters(&self.parameters, 0)
        {
            return Err(TaskFailure::invalid(
                "analysis cache parameters must be bounded, non-sensitive configuration values",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisCacheEntry {
    pub protocol_version: u32,
    pub cache_key: String,
    pub identity: AnalysisCacheIdentity,
    /// This is a validated structured analysis, never a provider response envelope or prompt.
    pub analysis: UiReferenceAnalysis,
}

pub struct AnalysisCache {
    root: PathBuf,
}

impl AnalysisCache {
    /// Creates the ignored local cache only under the repository generation work root.
    pub fn open(repository_root: &Path) -> Result<Self, TaskFailure> {
        let root = repository_root
            .join("summary")
            .join("ui-generation")
            .join(".cache")
            .join("analysis");
        fs::create_dir_all(&root)
            .map_err(|_| TaskFailure::invalid("analysis cache directory could not be created"))?;
        if fs::symlink_metadata(&root)
            .map_err(|_| TaskFailure::invalid("analysis cache directory cannot be inspected"))?
            .file_type()
            .is_symlink()
        {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "analysis cache root cannot be a symbolic link",
                None,
            ));
        }
        Ok(Self { root })
    }

    pub fn load(
        &self,
        identity: &AnalysisCacheIdentity,
    ) -> Result<Option<AnalysisCacheEntry>, TaskFailure> {
        let path = self.entry_path(&identity.cache_key())?;
        if !path.exists() {
            return Ok(None);
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| TaskFailure::invalid("analysis cache entry cannot be inspected"))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(TaskFailure::new(
                TaskFailureKind::PreprocessCacheCorrupt,
                "analysis cache entry is not a regular file",
                None,
            ));
        }
        let entry: AnalysisCacheEntry = serde_json::from_slice(
            &fs::read(&path)
                .map_err(|_| TaskFailure::invalid("analysis cache entry cannot be read"))?,
        )
        .map_err(|_| {
            TaskFailure::new(
                TaskFailureKind::PreprocessCacheCorrupt,
                "analysis cache entry is malformed",
                None,
            )
        })?;
        if entry.protocol_version != OBSERVABILITY_PROTOCOL_VERSION
            || entry.identity != *identity
            || entry.cache_key != identity.cache_key()
        {
            return Err(TaskFailure::new(
                TaskFailureKind::PreprocessCacheCorrupt,
                "analysis cache entry does not match its exact identity",
                None,
            ));
        }
        Ok(Some(entry))
    }

    pub fn store(
        &self,
        identity: AnalysisCacheIdentity,
        analysis: UiReferenceAnalysis,
    ) -> Result<AnalysisCacheEntry, TaskFailure> {
        identity.validate()?;
        let analysis_bytes = serde_json::to_vec(&analysis)
            .map_err(|_| TaskFailure::invalid("analysis cache value cannot be serialized"))?;
        let report = crate::analysis::validate_analysis_json(&analysis_bytes);
        if !report.valid {
            return Err(TaskFailure::invalid(
                "analysis cache only accepts a formally valid structured analysis",
            ));
        }
        let cache_key = identity.cache_key();
        let entry = AnalysisCacheEntry {
            protocol_version: OBSERVABILITY_PROTOCOL_VERSION,
            cache_key: cache_key.clone(),
            identity,
            analysis,
        };
        let path = self.entry_path(&cache_key)?;
        let bytes = serde_json::to_vec_pretty(&entry)
            .map_err(|_| TaskFailure::invalid("analysis cache entry cannot be serialized"))?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(&bytes)
                    .and_then(|_| file.write_all(b"\n"))
                    .and_then(|_| file.sync_all())
                    .map_err(|_| TaskFailure::invalid("analysis cache entry cannot be written"))?;
                Ok(entry)
            }
            Err(_) if path.exists() => self.load(&entry.identity)?.ok_or_else(|| {
                TaskFailure::new(
                    TaskFailureKind::PreprocessCacheCorrupt,
                    "analysis cache entry disappeared while resolving a write race",
                    None,
                )
            }),
            Err(_) => Err(TaskFailure::invalid(
                "analysis cache entry cannot be created",
            )),
        }
    }

    /// Explicitly removes one exact identity. Cache entries are never silently invalidated.
    pub fn invalidate(&self, identity: &AnalysisCacheIdentity) -> Result<bool, TaskFailure> {
        let path = self.entry_path(&identity.cache_key())?;
        if !path.exists() {
            return Ok(false);
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| TaskFailure::invalid("analysis cache entry cannot be inspected"))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "analysis cache invalidation refuses a non-regular entry",
                None,
            ));
        }
        fs::remove_file(path)
            .map_err(|_| TaskFailure::invalid("analysis cache entry cannot be invalidated"))?;
        Ok(true)
    }

    fn entry_path(&self, key: &str) -> Result<PathBuf, TaskFailure> {
        if !is_sha256(key) {
            return Err(TaskFailure::invalid(
                "analysis cache key must be a SHA-256 hex value",
            ));
        }
        let path = self.root.join(format!("{key}.json"));
        if path.parent() != Some(self.root.as_path()) {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "analysis cache entry escaped its root",
                None,
            ));
        }
        Ok(path)
    }
}

/// Redacts structured logs and reports before serialization. It is intentionally conservative:
/// sensitive field names hide their complete value instead of attempting to preserve fragments.
pub fn redact_report_value(value: &Value) -> Value {
    redact_value(value, None)
}

fn redact_value(value: &Value, field_name: Option<&str>) -> Value {
    if field_name.is_some_and(is_sensitive_field_name) {
        return Value::String(REDACTED.to_owned());
    }
    match value {
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| redact_value(value, field_name))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), redact_value(value, Some(key))))
                .collect(),
        ),
        Value::String(text) if looks_like_personal_text(text) => Value::String(REDACTED.to_owned()),
        _ => value.clone(),
    }
}

fn is_sensitive_field_name(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "credential",
        "secret",
        "token",
        "password",
        "authorization",
        "api_key",
        "apikey",
        "prompt",
        "instruction",
        "raw_response",
        "raw_output",
        "model_response",
        "account",
        "player_id",
        "email",
        "phone",
        "visible_text",
        "content",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn looks_like_personal_text(value: &str) -> bool {
    let at_count = value.bytes().filter(|byte| *byte == b'@').count();
    at_count == 1
        || value.bytes().filter(|byte| byte.is_ascii_digit()).count() >= 8
        || value.starts_with("sk-")
        || value.starts_with("Bearer ")
}

fn safe_cache_parameters(value: &Value, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
        Value::String(value) => value.len() <= 256 && !looks_like_personal_text(value),
        Value::Array(values) => {
            values.len() <= 64
                && values
                    .iter()
                    .all(|value| safe_cache_parameters(value, depth + 1))
        }
        Value::Object(values) => {
            values.len() <= 64
                && values.iter().all(|(key, value)| {
                    is_safe_label(key, 64)
                        && !is_sensitive_field_name(key)
                        && safe_cache_parameters(value, depth + 1)
                })
        }
    }
}

fn is_safe_label(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_budget_hard_stops_calls_images_and_accounted_cost() {
        let budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 1,
            max_elapsed_ms: 60_000,
            max_images: 2,
            max_input_units: 4,
            max_output_units: 4,
            max_estimated_cost_microunits: 3,
            input_cost_microunits_per_1k: 1_000,
            output_cost_microunits_per_1k: 1_000,
        })
        .unwrap();
        budget.reserve_provider_attempt(2).unwrap();
        assert_eq!(
            budget.reserve_provider_attempt(1).unwrap_err().subject(),
            Some("UI_GENERATION_LIMIT_PROVIDER_CALLS")
        );
        assert_eq!(
            budget
                .record_provider_usage(Some(3), Some(1))
                .unwrap_err()
                .subject(),
            Some("UI_GENERATION_LIMIT_COST")
        );
        let snapshot = budget.snapshot();
        assert_eq!(snapshot.provider_calls, 1);
        assert_eq!(snapshot.images, 2);
    }

    #[test]
    fn cache_identity_binds_all_reuse_inputs_and_rejects_sensitive_parameters() {
        let hash = "a".repeat(64);
        let first = AnalysisCacheIdentity::new(
            hash.clone(),
            "fixture-model",
            "analysis-v1",
            "ui-reference-analysis",
            1,
            serde_json::json!({"temperature": 0}),
        )
        .unwrap();
        let changed = AnalysisCacheIdentity::new(
            hash,
            "fixture-model",
            "analysis-v2",
            "ui-reference-analysis",
            1,
            serde_json::json!({"temperature": 0}),
        )
        .unwrap();
        assert_ne!(first.cache_key(), changed.cache_key());
        assert!(
            AnalysisCacheIdentity::new(
                "b".repeat(64),
                "fixture-model",
                "analysis-v1",
                "ui-reference-analysis",
                1,
                serde_json::json!({"api_key": "do-not-store"}),
            )
            .is_err()
        );
    }

    #[test]
    fn analysis_cache_reuses_only_the_exact_identity_and_needs_explicit_invalidation() {
        let repository = tempfile::tempdir().unwrap();
        let cache = AnalysisCache::open(repository.path()).unwrap();
        let identity = AnalysisCacheIdentity::new(
            "a".repeat(64),
            "fixture-model",
            "analysis-v1",
            "ui-reference-analysis",
            1,
            serde_json::json!({"temperature": 0}),
        )
        .unwrap();
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/analysis/hud.json");
        let analysis =
            crate::analysis::parse_analysis_json(&fs::read(fixture_path).unwrap()).unwrap();
        let stored = cache.store(identity.clone(), analysis).unwrap();
        assert_eq!(stored.cache_key, identity.cache_key());
        assert_eq!(
            cache.load(&identity).unwrap().unwrap().cache_key,
            stored.cache_key
        );
        let changed = AnalysisCacheIdentity::new(
            "a".repeat(64),
            "fixture-model",
            "analysis-v1",
            "ui-reference-analysis",
            1,
            serde_json::json!({"temperature": 1}),
        )
        .unwrap();
        assert!(cache.load(&changed).unwrap().is_none());
        assert!(cache.invalidate(&identity).unwrap());
        assert!(cache.load(&identity).unwrap().is_none());
        assert!(!cache.invalidate(&identity).unwrap());
    }

    #[test]
    fn reports_redact_credentials_accounts_personal_text_and_raw_model_content() {
        let value = serde_json::json!({
            "api_token": "secret-value",
            "player_id": "player-123",
            "raw_response": {"content": "model private output"},
            "email_note": "person@example.test",
            "safe_count": 3
        });
        let redacted = redact_report_value(&value);
        assert_eq!(redacted["api_token"], REDACTED);
        assert_eq!(redacted["player_id"], REDACTED);
        assert_eq!(redacted["raw_response"], REDACTED);
        assert_eq!(redacted["email_note"], REDACTED);
        assert_eq!(redacted["safe_count"], 3);
    }
}
