use crate::{
    TASK_CONTRACT_VERSION,
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

const MAX_TASK_JSON_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationTask {
    pub contract_version: u32,
    pub run_id: String,
    #[serde(default)]
    pub page_purpose: Option<PagePurpose>,
    pub primary_reference: ReferenceImage,
    #[serde(default)]
    pub additional_references: Vec<AdditionalReferenceImage>,
    #[serde(default)]
    pub target_viewport: Option<TargetViewport>,
    #[serde(default)]
    pub visible_text: Vec<VisibleText>,
    #[serde(default)]
    pub must_preserve: Vec<PreservedContent>,
    #[serde(default)]
    pub allowed_changes: Option<AllowedModificationScope>,
    #[serde(default)]
    pub visual_preferences: VisualPreferences,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PagePurpose {
    pub summary: String,
    pub audience: String,
    #[serde(default)]
    pub required_actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceImage {
    pub reference_id: String,
    pub path: PathBuf,
    pub metadata: ImageInputMetadata,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdditionalReferenceImage {
    #[serde(flatten)]
    pub image: ReferenceImage,
    /// Lower values are considered first. Zero is reserved for the primary reference.
    pub priority: u16,
    pub role: AdditionalReferenceRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdditionalReferenceRole {
    State {
        state_id: String,
        #[serde(default)]
        transition_evidence: Option<String>,
    },
    Viewport {
        viewport: TargetViewport,
    },
    Detail {
        purpose: String,
        region: NormalizedRegion,
    },
}

impl AdditionalReferenceRole {
    fn tie_break_rank(&self) -> u8 {
        match self {
            Self::State { .. } => 0,
            Self::Viewport { .. } => 1,
            Self::Detail { .. } => 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageInputMetadata {
    pub original_size: PixelSize,
    pub orientation: ImageOrientation,
    pub color_space: ImageColorSpace,
    pub sha256: String,
    pub provenance: ImageProvenance,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageOrientation {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    MirrorHorizontal,
    MirrorVertical,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageColorSpace {
    Srgb,
    DisplayP3,
    AdobeRgb,
    Other(String),
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageProvenance {
    pub source: String,
    #[serde(default)]
    pub source_uri: Option<String>,
    pub authorization: ImageAuthorization,
    #[serde(default)]
    pub license_reference: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageAuthorization {
    AnalysisOnly,
    DerivativesAllowed,
    DistributionAllowed,
    Unknown,
    Denied,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub device_scale: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleText {
    pub text: String,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreservedContent {
    pub description: String,
    #[serde(default)]
    pub reference_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedModificationScope {
    #[serde(default)]
    pub areas: Vec<ModificationArea>,
    pub text_changes_allowed: bool,
    #[serde(default)]
    pub protected_regions: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModificationArea {
    Layout,
    Spacing,
    Typography,
    Color,
    Decoration,
    Imagery,
    Components,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisualPreferences {
    #[serde(default)]
    pub decorative_treatment: Option<LowRiskVisualChoice>,
    #[serde(default)]
    pub surface_detail: Option<LowRiskVisualChoice>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LowRiskVisualChoice {
    FollowReference,
    UseProjectTheme,
    Minimal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedVisualDefaults {
    pub decorative_treatment: LowRiskVisualChoice,
    pub surface_detail: LowRiskVisualChoice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionImpact {
    High,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputQuestion {
    pub code: String,
    pub impact: QuestionImpact,
    pub field_path: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAssessment {
    pub defaults: AppliedVisualDefaults,
    pub questions: Vec<InputQuestion>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceOrderEntry {
    pub reference_id: String,
    pub effective_priority: u32,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedReferenceImage {
    pub reference_id: String,
    pub resolved_path: PathBuf,
    pub byte_length: u64,
    pub sha256: String,
    pub metadata: ImageInputMetadata,
}

impl GenerationTask {
    pub fn load_json(path: &Path) -> Result<Self, TaskFailure> {
        let metadata = fs::metadata(path).map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::InvalidInput,
                format!("task JSON cannot be read: {error}"),
                Some(path.display().to_string()),
            )
        })?;
        if !metadata.is_file() || metadata.len() > MAX_TASK_JSON_BYTES {
            return Err(TaskFailure::new(
                TaskFailureKind::InvalidInput,
                format!("task JSON must be a file no larger than {MAX_TASK_JSON_BYTES} bytes"),
                Some(path.display().to_string()),
            ));
        }
        let bytes = fs::read(path).map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::InvalidInput,
                format!("task JSON cannot be read: {error}"),
                Some(path.display().to_string()),
            )
        })?;
        Self::parse_json(&bytes)
    }

    pub fn parse_json(bytes: &[u8]) -> Result<Self, TaskFailure> {
        let task: Self = serde_json::from_slice(bytes).map_err(|error| {
            TaskFailure::invalid(format!(
                "task JSON does not match the strict contract: {error}"
            ))
        })?;
        task.validate()?;
        Ok(task)
    }

    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.contract_version != TASK_CONTRACT_VERSION {
            return Err(TaskFailure::invalid(format!(
                "unsupported task contract_version {}; expected {TASK_CONTRACT_VERSION}",
                self.contract_version
            )));
        }
        crate::directory::RunId::parse(&self.run_id)?;
        let viewport = self.target_viewport.ok_or_else(|| {
            TaskFailure::new(
                TaskFailureKind::TargetViewportMissing,
                "target_viewport is required because reference pixels do not define game logical size",
                Some("$.target_viewport".to_owned()),
            )
        })?;
        validate_viewport(viewport, "$.target_viewport")?;

        let mut reference_ids = BTreeSet::new();
        validate_reference(&self.primary_reference, "$.primary_reference")?;
        reference_ids.insert(self.primary_reference.reference_id.as_str());
        for (index, reference) in self.additional_references.iter().enumerate() {
            if reference.priority == 0 {
                return Err(TaskFailure::invalid(format!(
                    "$.additional_references[{index}].priority must be greater than zero"
                )));
            }
            validate_reference(
                &reference.image,
                &format!("$.additional_references[{index}]"),
            )?;
            if !reference_ids.insert(reference.image.reference_id.as_str()) {
                return Err(TaskFailure::invalid(format!(
                    "reference_id `{}` is duplicated",
                    reference.image.reference_id
                )));
            }
            validate_reference_role(&reference.role, index)?;
        }
        validate_optional_text(
            &self
                .page_purpose
                .as_ref()
                .map(|value| value.summary.as_str()),
            "$.page_purpose.summary",
        )?;
        if let Some(purpose) = &self.page_purpose {
            validate_nonempty(&purpose.audience, "$.page_purpose.audience")?;
            for (index, action) in purpose.required_actions.iter().enumerate() {
                validate_nonempty(action, &format!("$.page_purpose.required_actions[{index}]"))?;
            }
        }
        for (index, item) in self.visible_text.iter().enumerate() {
            validate_nonempty(&item.text, &format!("$.visible_text[{index}].text"))?;
        }
        for (index, item) in self.must_preserve.iter().enumerate() {
            validate_nonempty(
                &item.description,
                &format!("$.must_preserve[{index}].description"),
            )?;
            if let Some(reference_id) = &item.reference_id
                && !reference_ids.contains(reference_id.as_str())
            {
                return Err(TaskFailure::invalid(format!(
                    "$.must_preserve[{index}].reference_id does not name an input reference"
                )));
            }
        }
        if let Some(scope) = &self.allowed_changes {
            let unique = scope.areas.iter().copied().collect::<BTreeSet<_>>();
            if unique.len() != scope.areas.len() {
                return Err(TaskFailure::invalid(
                    "$.allowed_changes.areas contains duplicates",
                ));
            }
            for (index, region) in scope.protected_regions.iter().enumerate() {
                validate_nonempty(
                    region,
                    &format!("$.allowed_changes.protected_regions[{index}]"),
                )?;
            }
        }
        Ok(())
    }

    pub fn ordered_references(&self) -> Vec<ReferenceOrderEntry> {
        let additional = self.ordered_additional_references();
        let mut ordered = vec![ReferenceOrderEntry {
            reference_id: self.primary_reference.reference_id.clone(),
            effective_priority: 0,
            role: "primary".to_owned(),
        }];
        ordered.extend(additional.into_iter().map(|reference| {
            ReferenceOrderEntry {
                reference_id: reference.image.reference_id.clone(),
                effective_priority: u32::from(reference.priority),
                role: match reference.role {
                    AdditionalReferenceRole::State { .. } => "state",
                    AdditionalReferenceRole::Viewport { .. } => "viewport",
                    AdditionalReferenceRole::Detail { .. } => "detail",
                }
                .to_owned(),
            }
        }));
        ordered
    }

    pub fn assess(&self) -> TaskAssessment {
        let defaults = AppliedVisualDefaults {
            decorative_treatment: self
                .visual_preferences
                .decorative_treatment
                .unwrap_or(LowRiskVisualChoice::UseProjectTheme),
            surface_detail: self
                .visual_preferences
                .surface_detail
                .unwrap_or(LowRiskVisualChoice::UseProjectTheme),
        };
        let mut questions = Vec::new();
        if self.page_purpose.is_none() {
            questions.push(question(
                "PAGE_PURPOSE_REQUIRED",
                "$.page_purpose",
                "What user goal, audience, and required actions does this page serve?",
            ));
        }
        if self.visible_text.is_empty() {
            questions.push(question(
                "VISIBLE_TEXT_CONFIRMATION_REQUIRED",
                "$.visible_text",
                "Confirm the exact visible copy and localization ownership for this page.",
            ));
        }
        if self.must_preserve.is_empty() {
            questions.push(question(
                "PRESERVED_CONTENT_CONFIRMATION_REQUIRED",
                "$.must_preserve",
                "Which content, hierarchy, brand, and interaction elements must remain unchanged?",
            ));
        }
        if self.allowed_changes.is_none() {
            questions.push(question(
                "MODIFICATION_SCOPE_REQUIRED",
                "$.allowed_changes",
                "Which layout, style, component, imagery, and text changes are explicitly allowed?",
            ));
        }
        for (reference, reference_path) in self.references_with_json_paths() {
            if reference.metadata.orientation == ImageOrientation::Unknown {
                questions.push(question(
                    "IMAGE_ORIENTATION_CONFIRMATION_REQUIRED",
                    &format!("{reference_path}.metadata.orientation"),
                    "Confirm the intended display orientation before visual analysis.",
                ));
            }
            if reference.metadata.color_space == ImageColorSpace::Unknown {
                questions.push(question(
                    "IMAGE_COLOR_SPACE_CONFIRMATION_REQUIRED",
                    &format!("{reference_path}.metadata.color_space"),
                    "Confirm the source color space before extracting color tokens.",
                ));
            }
            if reference.metadata.provenance.authorization == ImageAuthorization::Unknown {
                questions.push(question(
                    "IMAGE_AUTHORIZATION_REQUIRED",
                    &format!("{reference_path}.metadata.provenance.authorization"),
                    "Confirm whether this image may be analyzed, transformed, and distributed.",
                ));
            }
        }
        for (index, reference) in self.additional_references.iter().enumerate() {
            let needs_transition_evidence = match &reference.role {
                AdditionalReferenceRole::State {
                    transition_evidence,
                    ..
                } => transition_evidence
                    .as_deref()
                    .map(str::trim)
                    .filter(|evidence| !evidence.is_empty())
                    .is_none(),
                _ => false,
            };
            if needs_transition_evidence {
                questions.push(question(
                    "STATE_TRANSITION_EVIDENCE_REQUIRED",
                    &format!("$.additional_references[{index}].role.transition_evidence"),
                    "Describe how this visible state is entered and which behavior is authoritative.",
                ));
            }
        }
        TaskAssessment {
            defaults,
            questions,
        }
    }

    pub fn verify_reference_files(
        &self,
        task_path: &Path,
        cancellation: &CancellationToken,
    ) -> Result<Vec<VerifiedReferenceImage>, TaskFailure> {
        let task_directory = task_path.parent().unwrap_or_else(|| Path::new("."));
        let mut verified = Vec::new();
        let references = std::iter::once(&self.primary_reference)
            .chain(
                self.ordered_additional_references()
                    .into_iter()
                    .map(|reference| &reference.image),
            )
            .collect::<Vec<_>>();
        for reference in references {
            cancellation.checkpoint()?;
            let path = if reference.path.is_absolute() {
                reference.path.clone()
            } else {
                task_directory.join(&reference.path)
            };
            let (byte_length, actual_hash) =
                hash_reference_file(&path, &reference.reference_id, cancellation)?;
            if actual_hash != reference.metadata.sha256 {
                return Err(TaskFailure::new(
                    TaskFailureKind::ImageHashMismatch,
                    format!(
                        "reference image `{}` does not match its declared SHA-256",
                        reference.reference_id
                    ),
                    Some(path.display().to_string()),
                ));
            }
            verified.push(VerifiedReferenceImage {
                reference_id: reference.reference_id.clone(),
                resolved_path: path,
                byte_length,
                sha256: actual_hash,
                metadata: reference.metadata.clone(),
            });
        }
        Ok(verified)
    }

    fn references_with_json_paths(&self) -> Vec<(&ReferenceImage, String)> {
        let mut references = Vec::with_capacity(1 + self.additional_references.len());
        references.push((&self.primary_reference, "$.primary_reference".to_owned()));
        references.extend(self.additional_references.iter().enumerate().map(
            |(index, reference)| {
                (
                    &reference.image,
                    format!("$.additional_references[{index}]"),
                )
            },
        ));
        references
    }

    fn ordered_additional_references(&self) -> Vec<&AdditionalReferenceImage> {
        let mut additional = self.additional_references.iter().collect::<Vec<_>>();
        additional.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.role.tie_break_rank().cmp(&right.role.tie_break_rank()))
                .then_with(|| left.image.reference_id.cmp(&right.image.reference_id))
        });
        additional
    }
}

fn hash_reference_file(
    path: &Path,
    reference_id: &str,
    cancellation: &CancellationToken,
) -> Result<(u64, String), TaskFailure> {
    let file = File::open(path).map_err(|error| image_read_failure(path, reference_id, error))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut byte_length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        cancellation.checkpoint()?;
        let count = reader
            .read(&mut buffer)
            .map_err(|error| image_read_failure(path, reference_id, error))?;
        if count == 0 {
            break;
        }
        byte_length = byte_length.checked_add(count as u64).ok_or_else(|| {
            TaskFailure::new(
                TaskFailureKind::ImageUnreadable,
                format!("reference image `{reference_id}` byte length overflowed"),
                Some(path.display().to_string()),
            )
        })?;
        digest.update(&buffer[..count]);
    }
    cancellation.checkpoint()?;
    Ok((byte_length, format!("{:x}", digest.finalize())))
}

fn image_read_failure(path: &Path, reference_id: &str, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ImageUnreadable,
        format!("reference image `{reference_id}` cannot be read: {error}"),
        Some(path.display().to_string()),
    )
}

fn validate_reference(reference: &ReferenceImage, path: &str) -> Result<(), TaskFailure> {
    validate_identifier(&reference.reference_id, &format!("{path}.reference_id"))?;
    if reference.path.as_os_str().is_empty() {
        return Err(TaskFailure::invalid(format!(
            "{path}.path must not be empty"
        )));
    }
    if reference.metadata.original_size.width == 0 || reference.metadata.original_size.height == 0 {
        return Err(TaskFailure::invalid(format!(
            "{path}.metadata.original_size dimensions must be non-zero"
        )));
    }
    let hash = &reference.metadata.sha256;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(TaskFailure::invalid(format!(
            "{path}.metadata.sha256 must be 64 lowercase hexadecimal characters"
        )));
    }
    validate_nonempty(
        &reference.metadata.provenance.source,
        &format!("{path}.metadata.provenance.source"),
    )?;
    if reference.metadata.provenance.authorization == ImageAuthorization::Denied {
        return Err(TaskFailure::invalid(format!(
            "{path} has denied authorization and cannot be used as a generation input"
        )));
    }
    Ok(())
}

fn validate_reference_role(
    role: &AdditionalReferenceRole,
    index: usize,
) -> Result<(), TaskFailure> {
    match role {
        AdditionalReferenceRole::State { state_id, .. } => validate_identifier(
            state_id,
            &format!("$.additional_references[{index}].role.state_id"),
        ),
        AdditionalReferenceRole::Viewport { viewport } => validate_viewport(
            *viewport,
            &format!("$.additional_references[{index}].role.viewport"),
        ),
        AdditionalReferenceRole::Detail { purpose, region } => {
            validate_nonempty(
                purpose,
                &format!("$.additional_references[{index}].role.purpose"),
            )?;
            let valid = region.x >= 0.0
                && region.y >= 0.0
                && region.width > 0.0
                && region.height > 0.0
                && region.x + region.width <= 1.0
                && region.y + region.height <= 1.0;
            if !valid {
                return Err(TaskFailure::invalid(format!(
                    "$.additional_references[{index}].role.region must be a non-empty normalized rectangle inside 0..=1"
                )));
            }
            Ok(())
        }
    }
}

fn validate_viewport(viewport: TargetViewport, path: &str) -> Result<(), TaskFailure> {
    if viewport.logical_width <= 0.0
        || viewport.logical_height <= 0.0
        || viewport.device_scale <= 0.0
    {
        Err(TaskFailure::invalid(format!(
            "{path} dimensions and device_scale must be positive"
        )))
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, path: &str) -> Result<(), TaskFailure> {
    let mut chars = value.chars();
    let first = chars.next();
    let valid = value.len() <= 96
        && first.is_some_and(|character| character.is_ascii_alphanumeric())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        });
    if valid {
        Ok(())
    } else {
        Err(TaskFailure::invalid(format!(
            "{path} is not a safe stable identifier"
        )))
    }
}

fn validate_optional_text(value: &Option<&str>, path: &str) -> Result<(), TaskFailure> {
    if let Some(value) = value {
        validate_nonempty(value, path)
    } else {
        Ok(())
    }
}

fn validate_nonempty(value: &str, path: &str) -> Result<(), TaskFailure> {
    if value.trim().is_empty() {
        Err(TaskFailure::invalid(format!("{path} must not be empty")))
    } else {
        Ok(())
    }
}

fn question(code: &str, field_path: &str, prompt: &str) -> InputQuestion {
    InputQuestion {
        code: code.to_owned(),
        impact: QuestionImpact::High,
        field_path: field_path.to_owned(),
        prompt: prompt.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_task_value() -> serde_json::Value {
        json!({
            "contract_version": 1,
            "run_id": "gallery-login",
            "page_purpose": {
                "summary": "Authenticate a player",
                "audience": "returning players",
                "required_actions": ["submit login"]
            },
            "primary_reference": {
                "reference_id": "primary",
                "path": "primary.bin",
                "metadata": {
                    "original_size": { "width": 1080, "height": 2400 },
                    "orientation": "normal",
                    "color_space": "srgb",
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "provenance": {
                        "source": "repository test fixture",
                        "authorization": "analysis_only"
                    }
                }
            },
            "additional_references": [],
            "target_viewport": {
                "logical_width": 360.0,
                "logical_height": 800.0,
                "device_scale": 3.0
            },
            "visible_text": [{ "text": "Sign in" }],
            "must_preserve": [{ "description": "brand title" }],
            "allowed_changes": {
                "areas": ["layout", "spacing", "decoration"],
                "text_changes_allowed": false,
                "protected_regions": ["brand title"]
            }
        })
    }

    fn parse_value(value: &serde_json::Value) -> Result<GenerationTask, TaskFailure> {
        GenerationTask::parse_json(&serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn strict_task_parser_accepts_contract_and_rejects_unknown_fields() {
        assert!(parse_value(&valid_task_value()).is_ok());
        let mut invalid = valid_task_value();
        invalid["unexpected"] = json!(true);
        let failure = parse_value(&invalid).unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::InvalidInput);
        assert!(failure.message().contains("unknown field"));
    }

    #[test]
    fn missing_target_viewport_has_its_own_failure_kind() {
        let mut value = valid_task_value();
        value.as_object_mut().unwrap().remove("target_viewport");
        let failure = parse_value(&value).unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::TargetViewportMissing);
    }

    #[test]
    fn reference_priority_is_primary_then_numeric_role_and_id() {
        let mut value = valid_task_value();
        value["additional_references"] = json!([
            reference_value(
                "z-detail",
                2,
                json!({
                    "kind": "detail", "purpose": "icon", "region": {"x": 0.0, "y": 0.0, "width": 0.2, "height": 0.2}
                })
            ),
            reference_value(
                "b-viewport",
                1,
                json!({
                    "kind": "viewport", "viewport": {"logical_width": 800.0, "logical_height": 600.0, "device_scale": 1.0}
                })
            ),
            reference_value(
                "a-state",
                1,
                json!({
                    "kind": "state", "state_id": "loading", "transition_evidence": "after submit"
                })
            )
        ]);
        let task = parse_value(&value).unwrap();
        let ids = task
            .ordered_references()
            .into_iter()
            .map(|entry| entry.reference_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["primary", "a-state", "b-viewport", "z-detail"]);
    }

    fn reference_value(id: &str, priority: u16, role: serde_json::Value) -> serde_json::Value {
        json!({
            "reference_id": id,
            "path": format!("{id}.bin"),
            "metadata": {
                "original_size": { "width": 100, "height": 100 },
                "orientation": "normal",
                "color_space": "srgb",
                "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "provenance": { "source": "test", "authorization": "analysis_only" }
            },
            "priority": priority,
            "role": role
        })
    }

    #[test]
    fn defaults_are_low_risk_and_high_impact_gaps_become_questions() {
        let mut value = valid_task_value();
        for field in [
            "page_purpose",
            "visible_text",
            "must_preserve",
            "allowed_changes",
        ] {
            value.as_object_mut().unwrap().remove(field);
        }
        value["primary_reference"]["metadata"]["orientation"] = json!("unknown");
        value["primary_reference"]["metadata"]["color_space"] = json!("unknown");
        value["primary_reference"]["metadata"]["provenance"]["authorization"] = json!("unknown");
        let task = parse_value(&value).unwrap();
        let assessment = task.assess();
        assert_eq!(
            assessment.defaults.decorative_treatment,
            LowRiskVisualChoice::UseProjectTheme
        );
        assert_eq!(
            assessment.defaults.surface_detail,
            LowRiskVisualChoice::UseProjectTheme
        );
        let codes = assessment
            .questions
            .iter()
            .map(|question| question.code.as_str())
            .collect::<BTreeSet<_>>();
        for code in [
            "PAGE_PURPOSE_REQUIRED",
            "VISIBLE_TEXT_CONFIRMATION_REQUIRED",
            "PRESERVED_CONTENT_CONFIRMATION_REQUIRED",
            "MODIFICATION_SCOPE_REQUIRED",
            "IMAGE_ORIENTATION_CONFIRMATION_REQUIRED",
            "IMAGE_COLOR_SPACE_CONFIRMATION_REQUIRED",
            "IMAGE_AUTHORIZATION_REQUIRED",
        ] {
            assert!(codes.contains(code), "missing question {code}");
        }
    }

    #[test]
    fn image_questions_use_exact_primary_and_additional_json_paths() {
        let mut value = valid_task_value();
        value["primary_reference"]["metadata"]["orientation"] = json!("unknown");
        value["primary_reference"]["metadata"]["color_space"] = json!("unknown");
        value["primary_reference"]["metadata"]["provenance"]["authorization"] = json!("unknown");
        value["additional_references"] = json!([reference_value(
            "state-loading",
            1,
            json!({
                "kind": "state",
                "state_id": "loading",
                "transition_evidence": "shown after submit"
            })
        )]);
        value["additional_references"][0]["metadata"]["orientation"] = json!("unknown");
        value["additional_references"][0]["metadata"]["color_space"] = json!("unknown");
        value["additional_references"][0]["metadata"]["provenance"]["authorization"] =
            json!("unknown");

        let assessment = parse_value(&value).unwrap().assess();
        let actual = assessment
            .questions
            .iter()
            .map(|question| (question.code.as_str(), question.field_path.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                (
                    "IMAGE_ORIENTATION_CONFIRMATION_REQUIRED",
                    "$.primary_reference.metadata.orientation",
                ),
                (
                    "IMAGE_COLOR_SPACE_CONFIRMATION_REQUIRED",
                    "$.primary_reference.metadata.color_space",
                ),
                (
                    "IMAGE_AUTHORIZATION_REQUIRED",
                    "$.primary_reference.metadata.provenance.authorization",
                ),
                (
                    "IMAGE_ORIENTATION_CONFIRMATION_REQUIRED",
                    "$.additional_references[0].metadata.orientation",
                ),
                (
                    "IMAGE_COLOR_SPACE_CONFIRMATION_REQUIRED",
                    "$.additional_references[0].metadata.color_space",
                ),
                (
                    "IMAGE_AUTHORIZATION_REQUIRED",
                    "$.additional_references[0].metadata.provenance.authorization",
                ),
            ]
        );
    }

    #[test]
    fn blank_state_transition_evidence_remains_a_high_impact_question() {
        for evidence in [None, Some(""), Some("   ")] {
            let mut value = valid_task_value();
            let mut role = json!({
                "kind": "state",
                "state_id": "loading"
            });
            if let Some(evidence) = evidence {
                role["transition_evidence"] = json!(evidence);
            }
            value["additional_references"] = json!([reference_value("state-loading", 1, role)]);
            let task = parse_value(&value).unwrap();
            assert!(
                task.assess()
                    .questions
                    .iter()
                    .any(|question| { question.code == "STATE_TRANSITION_EVIDENCE_REQUIRED" })
            );
        }

        let mut value = valid_task_value();
        value["additional_references"] = json!([reference_value(
            "state-loading",
            1,
            json!({
                "kind": "state",
                "state_id": "loading",
                "transition_evidence": "shown after the player submits the form"
            })
        )]);
        let task = parse_value(&value).unwrap();
        assert!(
            !task
                .assess()
                .questions
                .iter()
                .any(|question| { question.code == "STATE_TRANSITION_EVIDENCE_REQUIRED" })
        );
    }

    #[test]
    fn reference_bytes_are_hashed_and_metadata_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let task_path = directory.path().join("task.json");
        let image_path = directory.path().join("primary.bin");
        let bytes = b"stage-one-reference-bytes";
        fs::write(&image_path, bytes).unwrap();
        let mut value = valid_task_value();
        value["primary_reference"]["metadata"]["sha256"] =
            json!(format!("{:x}", Sha256::digest(bytes)));
        fs::write(&task_path, serde_json::to_vec(&value).unwrap()).unwrap();
        let task = GenerationTask::load_json(&task_path).unwrap();
        let verified = task
            .verify_reference_files(&task_path, &CancellationToken::default())
            .unwrap();
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].byte_length, bytes.len() as u64);
        assert_eq!(
            verified[0].metadata.original_size,
            PixelSize {
                width: 1080,
                height: 2400
            }
        );

        fs::write(&image_path, b"changed").unwrap();
        let failure = task
            .verify_reference_files(&task_path, &CancellationToken::default())
            .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageHashMismatch);
    }

    #[test]
    fn unreadable_image_and_early_cancellation_are_distinct() {
        let task = parse_value(&valid_task_value()).unwrap();
        let task_path = Path::new("missing/task.json");
        let failure = task
            .verify_reference_files(task_path, &CancellationToken::default())
            .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageUnreadable);

        let token = CancellationToken::default();
        token.request();
        let failure = task.verify_reference_files(task_path, &token).unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::Cancelled);
    }

    #[test]
    fn committed_text_fixture_matches_the_strict_contract() {
        let fixture = include_bytes!("../fixtures/task.valid.json");
        let task = GenerationTask::parse_json(fixture).unwrap();
        assert_eq!(task.contract_version, TASK_CONTRACT_VERSION);
        assert_eq!(task.run_id, "fixture-contract-v1");
    }
}
