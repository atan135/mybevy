use image::{ImageError, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
};

pub const REFERENCE_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const COMMITTED_REFERENCE_ROOT: &str = "tools/ui-visual-audit/fixtures/references";
pub const TEMPORARY_REFERENCE_ROOT: &str = "summary/ui-visual-audit";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_REFERENCE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    RepositoryRootInvalid,
    ManifestPathUnsafe,
    ManifestReadFailed,
    ManifestTooLarge,
    ManifestParseFailed,
    UnsupportedSchemaVersion,
    ManifestEmpty,
    InvalidField,
    InvalidHash,
    InvalidBaselineRevision,
    UnrecordedBaselineUpdate,
    DuplicateReferenceId,
    DuplicateReferenceKey,
    UnsafeReferencePath,
    ReferenceOutsideAllowedRoot,
    ReferenceMissing,
    ReferenceNotFile,
    ReferenceTooLarge,
    ReferenceUnsupportedFormat,
    ReferenceFormatMismatch,
    ReferenceCorrupt,
    ReferenceHashMismatch,
    ReferenceDimensionsMismatch,
    ReferenceViewportMismatch,
    ReferenceAuthorizationInvalid,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ManifestError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            reference_id: None,
            path: None,
        }
    }

    fn for_reference(mut self, reference_id: &str) -> Self {
        self.reference_id = Some(reference_id.to_owned());
        self
    }

    fn at_path(mut self, path: &Path) -> Self {
        self.path = Some(path.display().to_string());
        self
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl Error for ManifestError {}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceManifest {
    pub schema_version: u32,
    pub references: Vec<ReferenceEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceEntry {
    pub reference_id: String,
    pub key: ReferenceKey,
    pub viewport: Viewport,
    pub image: ReferenceImage,
    pub metadata: ImageMetadata,
    pub provenance: ReferenceProvenance,
    pub baseline: BaselineRevision,
    pub allowed_differences: AllowedDifferences,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceKey {
    pub screen: String,
    pub device: String,
    pub state: String,
    pub locale: String,
    pub theme: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    pub logical_size: LogicalSize,
    pub physical_size: PixelSize,
    pub device_scale: f64,
    pub orientation: Orientation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Portrait,
    Landscape,
    Square,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceImage {
    pub storage: ReferenceStorage,
    pub relative_path: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceStorage {
    CommittedFixture,
    TemporaryLocal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageMetadata {
    pub original_size: PixelSize,
    pub color_space: ColorSpace,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSpace {
    Srgb,
    DisplayP3,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceProvenance {
    pub source: String,
    pub source_uri: Option<String>,
    pub authorization_status: AuthorizationStatus,
    pub license_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationStatus {
    RepositoryOwned,
    LicensedExternal,
    LocalRestricted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineRevision {
    pub version: u32,
    pub update_reason: String,
    pub previous_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedDifferences {
    pub profile: String,
    pub per_channel_tolerance: u8,
    pub max_changed_pixel_ratio: f64,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedReference {
    pub reference_id: String,
    pub path: PathBuf,
    pub decoded_size: PixelSize,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedReferenceManifest {
    pub manifest: ReferenceManifest,
    pub references: Vec<ResolvedReference>,
}

pub fn load_and_validate_manifest(
    repository_root: &Path,
    manifest_path: &Path,
) -> Result<ValidatedReferenceManifest, ManifestError> {
    let repository_root = canonical_repository_root(repository_root)?;
    let candidate = if manifest_path.is_absolute() {
        manifest_path.to_owned()
    } else {
        repository_root.join(manifest_path)
    };
    let canonical_manifest = fs::canonicalize(&candidate).map_err(|error| {
        ManifestError::new(
            ErrorCode::ManifestReadFailed,
            format!("manifest cannot be resolved: {error}"),
        )
        .at_path(&candidate)
    })?;
    if !canonical_manifest.starts_with(&repository_root) {
        return Err(ManifestError::new(
            ErrorCode::ManifestPathUnsafe,
            "manifest resolves outside the repository root",
        )
        .at_path(&canonical_manifest));
    }
    let metadata = fs::metadata(&canonical_manifest).map_err(|error| {
        ManifestError::new(
            ErrorCode::ManifestReadFailed,
            format!("manifest metadata cannot be read: {error}"),
        )
        .at_path(&canonical_manifest)
    })?;
    if !metadata.is_file() {
        return Err(ManifestError::new(
            ErrorCode::ManifestReadFailed,
            "manifest is not a regular file",
        )
        .at_path(&canonical_manifest));
    }
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::new(
            ErrorCode::ManifestTooLarge,
            format!("manifest exceeds the {MAX_MANIFEST_BYTES}-byte limit"),
        )
        .at_path(&canonical_manifest));
    }
    let bytes = fs::read(&canonical_manifest).map_err(|error| {
        ManifestError::new(
            ErrorCode::ManifestReadFailed,
            format!("manifest cannot be read: {error}"),
        )
        .at_path(&canonical_manifest)
    })?;
    parse_and_validate_manifest(&repository_root, &bytes)
}

pub fn parse_and_validate_manifest(
    repository_root: &Path,
    source: &[u8],
) -> Result<ValidatedReferenceManifest, ManifestError> {
    if source.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ManifestError::new(
            ErrorCode::ManifestTooLarge,
            format!("manifest exceeds the {MAX_MANIFEST_BYTES}-byte limit"),
        ));
    }
    let manifest: ReferenceManifest = serde_json::from_slice(source).map_err(|error| {
        ManifestError::new(
            ErrorCode::ManifestParseFailed,
            format!("reference manifest is not valid schema JSON: {error}"),
        )
    })?;
    validate_manifest(repository_root, manifest)
}

pub fn validate_manifest(
    repository_root: &Path,
    manifest: ReferenceManifest,
) -> Result<ValidatedReferenceManifest, ManifestError> {
    let repository_root = canonical_repository_root(repository_root)?;
    if manifest.schema_version != REFERENCE_MANIFEST_SCHEMA_VERSION {
        return Err(ManifestError::new(
            ErrorCode::UnsupportedSchemaVersion,
            format!(
                "schema_version must be {REFERENCE_MANIFEST_SCHEMA_VERSION}, got {}",
                manifest.schema_version
            ),
        ));
    }
    if manifest.references.is_empty() {
        return Err(ManifestError::new(
            ErrorCode::ManifestEmpty,
            "reference manifest must contain at least one entry",
        ));
    }

    let mut ids = BTreeMap::<String, usize>::new();
    let mut keys = BTreeMap::<CompositeKey, usize>::new();
    let mut resolved = Vec::with_capacity(manifest.references.len());
    for (index, reference) in manifest.references.iter().enumerate() {
        validate_entry_fields(reference)?;
        if let Some(first) = ids.insert(reference.reference_id.clone(), index) {
            return Err(ManifestError::new(
                ErrorCode::DuplicateReferenceId,
                format!(
                    "reference_id duplicates entries {first} and {index}: {}",
                    reference.reference_id
                ),
            )
            .for_reference(&reference.reference_id));
        }
        let key = CompositeKey::from(reference);
        if let Some(first) = keys.insert(key, index) {
            return Err(ManifestError::new(
                ErrorCode::DuplicateReferenceKey,
                format!("reference key duplicates entries {first} and {index}"),
            )
            .for_reference(&reference.reference_id));
        }
        resolved.push(validate_image(&repository_root, reference)?);
    }

    Ok(ValidatedReferenceManifest {
        manifest,
        references: resolved,
    })
}

pub fn validate_baseline_update(
    previous: &ReferenceEntry,
    candidate: &ReferenceEntry,
) -> Result<(), ManifestError> {
    if previous.reference_id != candidate.reference_id
        || CompositeKey::from(previous) != CompositeKey::from(candidate)
    {
        return Err(ManifestError::new(
            ErrorCode::UnrecordedBaselineUpdate,
            "baseline update must retain reference_id and the complete reference key",
        )
        .for_reference(&candidate.reference_id));
    }
    let next_version = previous.baseline.version.checked_add(1);
    if Some(candidate.baseline.version) != next_version
        || candidate.baseline.previous_sha256.as_deref() != Some(previous.image.sha256.as_str())
        || candidate.image.sha256 == previous.image.sha256
        || candidate.baseline.update_reason.trim().is_empty()
    {
        return Err(ManifestError::new(
            ErrorCode::UnrecordedBaselineUpdate,
            "baseline replacement requires the next version, a non-empty reason, the prior hash, and a changed image hash",
        )
        .for_reference(&candidate.reference_id));
    }
    Ok(())
}

fn canonical_repository_root(repository_root: &Path) -> Result<PathBuf, ManifestError> {
    let canonical = fs::canonicalize(repository_root).map_err(|error| {
        ManifestError::new(
            ErrorCode::RepositoryRootInvalid,
            format!("repository root cannot be resolved: {error}"),
        )
        .at_path(repository_root)
    })?;
    if !canonical.is_dir() {
        return Err(ManifestError::new(
            ErrorCode::RepositoryRootInvalid,
            "repository root is not a directory",
        )
        .at_path(&canonical));
    }
    Ok(canonical)
}

fn validate_entry_fields(reference: &ReferenceEntry) -> Result<(), ManifestError> {
    validate_identifier("reference_id", &reference.reference_id, true, reference)?;
    validate_identifier("key.screen", &reference.key.screen, false, reference)?;
    validate_identifier("key.device", &reference.key.device, false, reference)?;
    validate_identifier("key.state", &reference.key.state, false, reference)?;
    validate_identifier("key.locale", &reference.key.locale, false, reference)?;
    validate_identifier("key.theme", &reference.key.theme, false, reference)?;
    validate_viewport(reference)?;

    if !is_sha256(&reference.image.sha256) {
        return Err(ManifestError::new(
            ErrorCode::InvalidHash,
            "image.sha256 must contain 64 lowercase hexadecimal characters",
        )
        .for_reference(&reference.reference_id));
    }
    if reference.metadata.original_size.width == 0 || reference.metadata.original_size.height == 0 {
        return Err(ManifestError::new(
            ErrorCode::InvalidField,
            "metadata.original_size dimensions must be non-zero",
        )
        .for_reference(&reference.reference_id));
    }
    validate_nonempty("provenance.source", &reference.provenance.source, reference)?;
    validate_nonempty(
        "provenance.license_id",
        &reference.provenance.license_id,
        reference,
    )?;
    if let Some(source_uri) = &reference.provenance.source_uri {
        validate_nonempty("provenance.source_uri", source_uri, reference)?;
    }
    if reference.image.storage == ReferenceStorage::CommittedFixture
        && reference.provenance.authorization_status == AuthorizationStatus::LocalRestricted
    {
        return Err(ManifestError::new(
            ErrorCode::ReferenceAuthorizationInvalid,
            "committed fixtures cannot use local_restricted authorization",
        )
        .for_reference(&reference.reference_id));
    }
    validate_revision(reference)?;
    validate_nonempty(
        "allowed_differences.profile",
        &reference.allowed_differences.profile,
        reference,
    )?;
    if !reference
        .allowed_differences
        .max_changed_pixel_ratio
        .is_finite()
        || !(0.0..=1.0).contains(&reference.allowed_differences.max_changed_pixel_ratio)
    {
        return Err(ManifestError::new(
            ErrorCode::InvalidField,
            "allowed_differences.max_changed_pixel_ratio must be between 0 and 1",
        )
        .for_reference(&reference.reference_id));
    }
    if reference
        .allowed_differences
        .notes
        .iter()
        .any(|note| note.trim().is_empty())
    {
        return Err(ManifestError::new(
            ErrorCode::InvalidField,
            "allowed_differences.notes cannot contain empty entries",
        )
        .for_reference(&reference.reference_id));
    }
    Ok(())
}

fn validate_identifier(
    field: &str,
    value: &str,
    strict: bool,
    reference: &ReferenceEntry,
) -> Result<(), ManifestError> {
    validate_nonempty(field, value, reference)?;
    let valid = value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'
                || (!strict && matches!(character, '.' | '@'))
        });
    if !valid {
        return Err(ManifestError::new(
            ErrorCode::InvalidField,
            format!("{field} contains unsupported characters or exceeds 128 bytes"),
        )
        .for_reference(&reference.reference_id));
    }
    Ok(())
}

fn validate_nonempty(
    field: &str,
    value: &str,
    reference: &ReferenceEntry,
) -> Result<(), ManifestError> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(ManifestError::new(
            ErrorCode::InvalidField,
            format!(
                "{field} must be non-empty, at most 512 bytes, and contain no control characters"
            ),
        )
        .for_reference(&reference.reference_id));
    }
    Ok(())
}

fn validate_viewport(reference: &ReferenceEntry) -> Result<(), ManifestError> {
    let viewport = &reference.viewport;
    if viewport.physical_size.width == 0
        || viewport.physical_size.height == 0
        || !viewport.logical_size.width.is_finite()
        || !viewport.logical_size.height.is_finite()
        || viewport.logical_size.width <= 0.0
        || viewport.logical_size.height <= 0.0
        || !viewport.device_scale.is_finite()
        || viewport.device_scale <= 0.0
    {
        return Err(ManifestError::new(
            ErrorCode::InvalidField,
            "viewport sizes and device_scale must be finite and positive",
        )
        .for_reference(&reference.reference_id));
    }
    let expected_width = viewport.logical_size.width * viewport.device_scale;
    let expected_height = viewport.logical_size.height * viewport.device_scale;
    if (expected_width - f64::from(viewport.physical_size.width)).abs() > 1.0
        || (expected_height - f64::from(viewport.physical_size.height)).abs() > 1.0
    {
        return Err(ManifestError::new(
            ErrorCode::ReferenceViewportMismatch,
            "logical size multiplied by device_scale must match physical size within one pixel",
        )
        .for_reference(&reference.reference_id));
    }
    let actual_orientation = orientation_for(viewport.physical_size);
    if viewport.orientation != actual_orientation {
        return Err(ManifestError::new(
            ErrorCode::ReferenceViewportMismatch,
            format!(
                "declared orientation {:?} does not match physical dimensions",
                viewport.orientation
            ),
        )
        .for_reference(&reference.reference_id));
    }
    Ok(())
}

fn validate_revision(reference: &ReferenceEntry) -> Result<(), ManifestError> {
    let revision = &reference.baseline;
    validate_nonempty("baseline.update_reason", &revision.update_reason, reference).map_err(
        |mut error| {
            error.code = ErrorCode::InvalidBaselineRevision;
            error
        },
    )?;
    let previous_valid = revision.previous_sha256.as_deref().is_none_or(is_sha256);
    let valid_shape = revision.version > 0
        && previous_valid
        && ((revision.version == 1 && revision.previous_sha256.is_none())
            || (revision.version > 1 && revision.previous_sha256.is_some()));
    if !valid_shape {
        return Err(ManifestError::new(
            ErrorCode::InvalidBaselineRevision,
            "baseline version 1 must omit previous_sha256; later versions require a valid prior hash; every version requires an update reason",
        )
        .for_reference(&reference.reference_id));
    }
    if revision.previous_sha256.as_deref() == Some(reference.image.sha256.as_str()) {
        return Err(ManifestError::new(
            ErrorCode::InvalidBaselineRevision,
            "previous_sha256 must differ from the current image hash",
        )
        .for_reference(&reference.reference_id));
    }
    Ok(())
}

fn validate_image(
    repository_root: &Path,
    reference: &ReferenceEntry,
) -> Result<ResolvedReference, ManifestError> {
    let relative = safe_relative_path(&reference.image.relative_path)
        .map_err(|error| error.for_reference(&reference.reference_id))?;
    let root_relative = match reference.image.storage {
        ReferenceStorage::CommittedFixture => COMMITTED_REFERENCE_ROOT,
        ReferenceStorage::TemporaryLocal => TEMPORARY_REFERENCE_ROOT,
    };
    let expected_root = repository_root.join(root_relative);
    let candidate = expected_root.join(relative);
    if !candidate.exists() {
        return Err(ManifestError::new(
            ErrorCode::ReferenceMissing,
            "reference image does not exist in its declared storage root",
        )
        .for_reference(&reference.reference_id)
        .at_path(&candidate));
    }
    let canonical_root = fs::canonicalize(&expected_root).map_err(|error| {
        ManifestError::new(
            ErrorCode::ReferenceOutsideAllowedRoot,
            format!("reference storage root cannot be resolved: {error}"),
        )
        .for_reference(&reference.reference_id)
        .at_path(&expected_root)
    })?;
    if !canonical_root.starts_with(repository_root) {
        return Err(ManifestError::new(
            ErrorCode::ReferenceOutsideAllowedRoot,
            "reference storage root resolves outside the repository",
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical_root));
    }
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        ManifestError::new(
            ErrorCode::ReferenceMissing,
            format!("reference image cannot be resolved: {error}"),
        )
        .for_reference(&reference.reference_id)
        .at_path(&candidate)
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(ManifestError::new(
            ErrorCode::ReferenceOutsideAllowedRoot,
            "reference image resolves outside its allowed storage root",
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical));
    }
    let metadata = fs::metadata(&canonical).map_err(|error| {
        ManifestError::new(
            ErrorCode::ReferenceMissing,
            format!("reference image metadata cannot be read: {error}"),
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical)
    })?;
    if !metadata.is_file() {
        return Err(ManifestError::new(
            ErrorCode::ReferenceNotFile,
            "reference path is not a regular file",
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical));
    }
    if metadata.len() > MAX_REFERENCE_BYTES {
        return Err(ManifestError::new(
            ErrorCode::ReferenceTooLarge,
            format!("reference image exceeds the {MAX_REFERENCE_BYTES}-byte limit"),
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical));
    }
    let bytes = fs::read(&canonical).map_err(|error| {
        ManifestError::new(
            ErrorCode::ReferenceMissing,
            format!("reference image cannot be read: {error}"),
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical)
    })?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    if hash != reference.image.sha256 {
        return Err(ManifestError::new(
            ErrorCode::ReferenceHashMismatch,
            "reference image SHA-256 does not match the manifest",
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical));
    }
    let format = detect_format(&canonical, &bytes, reference)?;
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| {
        let code = if matches!(error, ImageError::Limits(_)) {
            ErrorCode::ReferenceTooLarge
        } else {
            ErrorCode::ReferenceCorrupt
        };
        ManifestError::new(code, format!("reference image cannot be decoded: {error}"))
            .for_reference(&reference.reference_id)
            .at_path(&canonical)
    })?;
    let decoded_size = PixelSize {
        width: decoded.width(),
        height: decoded.height(),
    };
    if decoded_size != reference.viewport.physical_size {
        return Err(ManifestError::new(
            ErrorCode::ReferenceDimensionsMismatch,
            format!(
                "decoded size {}x{} does not match declared physical size {}x{}",
                decoded_size.width,
                decoded_size.height,
                reference.viewport.physical_size.width,
                reference.viewport.physical_size.height
            ),
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical));
    }
    if decoded_size != reference.metadata.original_size {
        return Err(ManifestError::new(
            ErrorCode::ReferenceDimensionsMismatch,
            format!(
                "decoded size {}x{} does not match declared original size {}x{}; stage-1 references are not cropped or normalized",
                decoded_size.width,
                decoded_size.height,
                reference.metadata.original_size.width,
                reference.metadata.original_size.height
            ),
        )
        .for_reference(&reference.reference_id)
        .at_path(&canonical));
    }
    Ok(ResolvedReference {
        reference_id: reference.reference_id.clone(),
        path: canonical,
        decoded_size,
        byte_length: metadata.len(),
    })
}

fn safe_relative_path(value: &str) -> Result<PathBuf, ManifestError> {
    if value.is_empty() || value.contains('\\') {
        return Err(ManifestError::new(
            ErrorCode::UnsafeReferencePath,
            "reference image path must be a non-empty forward-slash relative path",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ManifestError::new(
            ErrorCode::UnsafeReferencePath,
            "reference image path cannot be absolute or contain '.', '..', roots, or prefixes",
        ));
    }
    Ok(path.to_owned())
}

fn detect_format(
    path: &Path,
    bytes: &[u8],
    reference: &ReferenceEntry,
) -> Result<ImageFormat, ManifestError> {
    let extension = path.extension().and_then(|value| value.to_str());
    let declared = match extension {
        Some("png") => ImageFormat::Png,
        Some("jpg" | "jpeg") => ImageFormat::Jpeg,
        _ => {
            return Err(ManifestError::new(
                ErrorCode::ReferenceUnsupportedFormat,
                "reference image extension must be lowercase .png, .jpg, or .jpeg",
            )
            .for_reference(&reference.reference_id)
            .at_path(path));
        }
    };
    let detected = image::guess_format(bytes).map_err(|error| {
        ManifestError::new(
            ErrorCode::ReferenceCorrupt,
            format!("reference image format cannot be detected: {error}"),
        )
        .for_reference(&reference.reference_id)
        .at_path(path)
    })?;
    if !matches!(detected, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(ManifestError::new(
            ErrorCode::ReferenceUnsupportedFormat,
            "reference image content must be PNG or JPEG",
        )
        .for_reference(&reference.reference_id)
        .at_path(path));
    }
    if declared != detected {
        return Err(ManifestError::new(
            ErrorCode::ReferenceFormatMismatch,
            "reference image extension does not match encoded content",
        )
        .for_reference(&reference.reference_id)
        .at_path(path));
    }
    Ok(detected)
}

fn orientation_for(size: PixelSize) -> Orientation {
    match size.width.cmp(&size.height) {
        std::cmp::Ordering::Less => Orientation::Portrait,
        std::cmp::Ordering::Greater => Orientation::Landscape,
        std::cmp::Ordering::Equal => Orientation::Square,
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CompositeKey {
    screen: String,
    device: String,
    state: String,
    locale: String,
    theme: String,
    logical_width: u64,
    logical_height: u64,
    physical_width: u32,
    physical_height: u32,
    device_scale: u64,
    orientation: u8,
}

impl From<&ReferenceEntry> for CompositeKey {
    fn from(reference: &ReferenceEntry) -> Self {
        Self {
            screen: reference.key.screen.clone(),
            device: reference.key.device.clone(),
            state: reference.key.state.clone(),
            locale: reference.key.locale.clone(),
            theme: reference.key.theme.clone(),
            logical_width: reference.viewport.logical_size.width.to_bits(),
            logical_height: reference.viewport.logical_size.height.to_bits(),
            physical_width: reference.viewport.physical_size.width,
            physical_height: reference.viewport.physical_size.height,
            device_scale: reference.viewport.device_scale.to_bits(),
            orientation: match reference.viewport.orientation {
                Orientation::Portrait => 0,
                Orientation::Landscape => 1,
                Orientation::Square => 2,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    use serde_json::{Value, json};
    use std::io::Write;

    struct Fixture {
        repository: tempfile::TempDir,
        image_bytes: Vec<u8>,
        manifest: Value,
    }

    impl Fixture {
        fn new(storage: &str) -> Self {
            let repository = tempfile::tempdir().unwrap();
            let root = if storage == "committed_fixture" {
                repository.path().join(COMMITTED_REFERENCE_ROOT)
            } else {
                repository.path().join(TEMPORARY_REFERENCE_ROOT)
            };
            fs::create_dir_all(&root).unwrap();
            let pixels = [
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
                32, 64, 96, 255, 128, 128, 128, 255, 255, 255, 0, 255,
            ];
            let mut image_bytes = Vec::new();
            PngEncoder::new(&mut image_bytes)
                .write_image(&pixels, 2, 4, ExtendedColorType::Rgba8)
                .unwrap();
            fs::create_dir_all(root.join("gallery")).unwrap();
            fs::write(root.join("gallery/default.png"), &image_bytes).unwrap();
            let hash = format!("{:x}", Sha256::digest(&image_bytes));
            let authorization = if storage == "committed_fixture" {
                "repository_owned"
            } else {
                "local_restricted"
            };
            let manifest = json!({
                "schema_version": 1,
                "references": [{
                    "reference_id": "gallery_phone_default_en_light",
                    "key": {
                        "screen": "gallery",
                        "device": "phone-small",
                        "state": "default",
                        "locale": "en-US",
                        "theme": "light"
                    },
                    "viewport": {
                        "logical_size": {"width": 2.0, "height": 4.0},
                        "physical_size": {"width": 2, "height": 4},
                        "device_scale": 1.0,
                        "orientation": "portrait"
                    },
                    "image": {
                        "storage": storage,
                        "relative_path": "gallery/default.png",
                        "sha256": hash
                    },
                    "metadata": {
                        "original_size": {"width": 2, "height": 4},
                        "color_space": "srgb"
                    },
                    "provenance": {
                        "source": "repository acceptance fixture",
                        "source_uri": null,
                        "authorization_status": authorization,
                        "license_id": "repository-owned"
                    },
                    "baseline": {
                        "version": 1,
                        "update_reason": "initial approved baseline",
                        "previous_sha256": null
                    },
                    "allowed_differences": {
                        "profile": "strict-ui",
                        "per_channel_tolerance": 2,
                        "max_changed_pixel_ratio": 0.001,
                        "notes": []
                    }
                }]
            });
            Self {
                repository,
                image_bytes,
                manifest,
            }
        }

        fn validate(&self) -> Result<ValidatedReferenceManifest, ManifestError> {
            parse_and_validate_manifest(
                self.repository.path(),
                &serde_json::to_vec(&self.manifest).unwrap(),
            )
        }
    }

    #[test]
    fn committed_and_temporary_manifests_validate_with_complete_metadata() {
        for storage in ["committed_fixture", "temporary_local"] {
            let fixture = Fixture::new(storage);
            let validated = fixture.validate().unwrap();
            assert_eq!(validated.references.len(), 1);
            assert_eq!(
                validated.references[0].decoded_size,
                PixelSize {
                    width: 2,
                    height: 4
                }
            );
            assert!(
                validated.references[0]
                    .path
                    .starts_with(fixture.repository.path().canonicalize().unwrap())
            );
        }
    }

    #[test]
    fn parser_rejects_unknown_fields_with_stable_code() {
        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["unexpected"] = json!(true);
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ManifestParseFailed
        );
    }

    #[test]
    fn duplicate_reference_id_and_composite_key_have_distinct_codes() {
        let mut duplicate_id = Fixture::new("committed_fixture");
        let entry = duplicate_id.manifest["references"][0].clone();
        duplicate_id.manifest["references"]
            .as_array_mut()
            .unwrap()
            .push(entry);
        assert_eq!(
            duplicate_id.validate().unwrap_err().code,
            ErrorCode::DuplicateReferenceId
        );

        let mut duplicate_key = Fixture::new("committed_fixture");
        let mut entry = duplicate_key.manifest["references"][0].clone();
        entry["reference_id"] = json!("a_second_reference");
        duplicate_key.manifest["references"]
            .as_array_mut()
            .unwrap()
            .push(entry);
        assert_eq!(
            duplicate_key.validate().unwrap_err().code,
            ErrorCode::DuplicateReferenceKey
        );
    }

    #[test]
    fn viewport_is_part_of_the_unique_key() {
        let mut fixture = Fixture::new("committed_fixture");
        let mut entry = fixture.manifest["references"][0].clone();
        entry["reference_id"] = json!("gallery_phone_large_en_light");
        entry["viewport"]["logical_size"] = json!({"width": 1.0, "height": 2.0});
        entry["viewport"]["device_scale"] = json!(2.0);
        fixture.manifest["references"]
            .as_array_mut()
            .unwrap()
            .push(entry);
        assert!(fixture.validate().is_ok());
    }

    #[test]
    fn unsafe_missing_and_non_file_paths_are_classified() {
        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["image"]["relative_path"] = json!("../escape.png");
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::UnsafeReferencePath
        );

        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["image"]["relative_path"] = json!("missing.png");
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceMissing
        );

        let mut fixture = Fixture::new("committed_fixture");
        let root = fixture.repository.path().join(COMMITTED_REFERENCE_ROOT);
        fs::create_dir_all(root.join("directory.png")).unwrap();
        fixture.manifest["references"][0]["image"]["relative_path"] = json!("directory.png");
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceNotFile
        );
    }

    #[test]
    fn canonical_symlink_escape_is_rejected_when_platform_allows_symlinks() {
        let mut fixture = Fixture::new("committed_fixture");
        let external = tempfile::NamedTempFile::new().unwrap();
        fs::write(external.path(), &fixture.image_bytes).unwrap();
        let link = fixture
            .repository
            .path()
            .join(COMMITTED_REFERENCE_ROOT)
            .join("gallery/escape.png");

        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(external.path(), &link);
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(external.path(), &link);
        if result.is_err() {
            // Windows requires Developer Mode or elevated symlink privileges. The
            // canonical escape assertion still runs on hosts that permit creation.
            return;
        }

        fixture.manifest["references"][0]["image"]["relative_path"] = json!("gallery/escape.png");
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceOutsideAllowedRoot
        );
    }

    #[test]
    fn hash_dimensions_and_viewport_mismatches_are_classified() {
        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["image"]["sha256"] = json!("f".repeat(64));
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceHashMismatch
        );

        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["viewport"]["physical_size"] =
            json!({"width": 4, "height": 8});
        fixture.manifest["references"][0]["viewport"]["device_scale"] = json!(2.0);
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceDimensionsMismatch
        );

        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["metadata"]["original_size"] =
            json!({"width": 1, "height": 2});
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceDimensionsMismatch
        );

        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["viewport"]["device_scale"] = json!(3.0);
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceViewportMismatch
        );
    }

    #[test]
    fn corrupt_unsupported_and_oversized_images_are_classified() {
        let mut corrupt = Fixture::new("committed_fixture");
        let root = corrupt.repository.path().join(COMMITTED_REFERENCE_ROOT);
        let corrupt_bytes = b"not a png";
        fs::write(root.join("gallery/default.png"), corrupt_bytes).unwrap();
        corrupt.manifest["references"][0]["image"]["sha256"] =
            json!(format!("{:x}", Sha256::digest(corrupt_bytes)));
        assert_eq!(
            corrupt.validate().unwrap_err().code,
            ErrorCode::ReferenceCorrupt
        );

        let mut unsupported = Fixture::new("committed_fixture");
        let old = unsupported
            .repository
            .path()
            .join(COMMITTED_REFERENCE_ROOT)
            .join("gallery/default.png");
        let new = old.with_extension("gif");
        fs::rename(old, &new).unwrap();
        unsupported.manifest["references"][0]["image"]["relative_path"] =
            json!("gallery/default.gif");
        assert_eq!(
            unsupported.validate().unwrap_err().code,
            ErrorCode::ReferenceUnsupportedFormat
        );

        let mut uppercase = Fixture::new("committed_fixture");
        let old = uppercase
            .repository
            .path()
            .join(COMMITTED_REFERENCE_ROOT)
            .join("gallery/default.png");
        let new = old.with_extension("PNG");
        fs::rename(old, &new).unwrap();
        uppercase.manifest["references"][0]["image"]["relative_path"] =
            json!("gallery/default.PNG");
        assert_eq!(
            uppercase.validate().unwrap_err().code,
            ErrorCode::ReferenceUnsupportedFormat
        );

        let oversized = Fixture::new("committed_fixture");
        let path = oversized
            .repository
            .path()
            .join(COMMITTED_REFERENCE_ROOT)
            .join("gallery/default.png");
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(MAX_REFERENCE_BYTES + 1).unwrap();
        assert_eq!(
            oversized.validate().unwrap_err().code,
            ErrorCode::ReferenceTooLarge
        );
    }

    #[test]
    fn committed_reference_requires_committable_authorization() {
        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["provenance"]["authorization_status"] =
            json!("local_restricted");
        assert_eq!(
            fixture.validate().unwrap_err().code,
            ErrorCode::ReferenceAuthorizationInvalid
        );
    }

    #[test]
    fn optional_source_uri_must_be_well_formed_when_present() {
        for invalid in ["", "   ", "https://example.invalid/\nsecret"] {
            let mut fixture = Fixture::new("committed_fixture");
            fixture.manifest["references"][0]["provenance"]["source_uri"] = json!(invalid);
            assert_eq!(
                fixture.validate().unwrap_err().code,
                ErrorCode::InvalidField
            );
        }

        let mut oversized = Fixture::new("committed_fixture");
        oversized.manifest["references"][0]["provenance"]["source_uri"] = json!("x".repeat(513));
        assert_eq!(
            oversized.validate().unwrap_err().code,
            ErrorCode::InvalidField
        );

        let mut valid = Fixture::new("committed_fixture");
        valid.manifest["references"][0]["provenance"]["source_uri"] =
            json!("https://example.invalid/design/revision-1");
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn machine_error_code_serializes_as_stable_snake_case() {
        let mut fixture = Fixture::new("committed_fixture");
        fixture.manifest["references"][0]["image"]["sha256"] = json!("f".repeat(64));
        let error = fixture.validate().unwrap_err();
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json["code"], "reference_hash_mismatch");
    }

    #[test]
    fn baseline_revision_and_transition_prevent_unrecorded_overwrite() {
        let fixture = Fixture::new("committed_fixture");
        let previous: ReferenceManifest = serde_json::from_value(fixture.manifest.clone()).unwrap();
        let previous = &previous.references[0];
        let mut candidate = previous.clone();
        candidate.image.sha256 = "b".repeat(64);
        candidate.baseline.version = 2;
        candidate.baseline.previous_sha256 = Some(previous.image.sha256.clone());
        candidate.baseline.update_reason = "approved visual refresh".to_owned();
        assert!(validate_baseline_update(previous, &candidate).is_ok());

        candidate.baseline.version = 1;
        assert_eq!(
            validate_baseline_update(previous, &candidate)
                .unwrap_err()
                .code,
            ErrorCode::UnrecordedBaselineUpdate
        );

        let mut exhausted = previous.clone();
        exhausted.baseline.version = u32::MAX;
        let mut overflow_candidate = candidate.clone();
        overflow_candidate.baseline.version = 0;
        assert_eq!(
            validate_baseline_update(&exhausted, &overflow_candidate)
                .unwrap_err()
                .code,
            ErrorCode::UnrecordedBaselineUpdate
        );

        let mut invalid = fixture.manifest;
        invalid["references"][0]["baseline"]["update_reason"] = json!("");
        assert_eq!(
            parse_and_validate_manifest(
                fixture.repository.path(),
                &serde_json::to_vec(&invalid).unwrap()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidBaselineRevision
        );
    }

    #[test]
    fn load_rejects_manifests_outside_repository() {
        let fixture = Fixture::new("committed_fixture");
        let external = tempfile::NamedTempFile::new().unwrap();
        serde_json::to_writer(external.as_file(), &fixture.manifest).unwrap();
        assert_eq!(
            load_and_validate_manifest(fixture.repository.path(), external.path())
                .unwrap_err()
                .code,
            ErrorCode::ManifestPathUnsafe
        );
    }

    #[test]
    fn malformed_json_and_invalid_hash_use_stable_codes() {
        let fixture = Fixture::new("committed_fixture");
        assert_eq!(
            parse_and_validate_manifest(fixture.repository.path(), b"{")
                .unwrap_err()
                .code,
            ErrorCode::ManifestParseFailed
        );
        let mut invalid = fixture.manifest;
        invalid["references"][0]["image"]["sha256"] = json!("ABC");
        assert_eq!(
            parse_and_validate_manifest(
                fixture.repository.path(),
                &serde_json::to_vec(&invalid).unwrap()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidHash
        );
    }

    #[test]
    fn manifest_size_limit_is_checked_before_parsing() {
        let fixture = Fixture::new("committed_fixture");
        let mut source = Vec::new();
        source
            .write_all(&vec![b' '; MAX_MANIFEST_BYTES as usize + 1])
            .unwrap();
        assert_eq!(
            parse_and_validate_manifest(fixture.repository.path(), &source)
                .unwrap_err()
                .code,
            ErrorCode::ManifestTooLarge
        );
    }

    #[test]
    fn fixture_bytes_are_real_png_data() {
        let fixture = Fixture::new("committed_fixture");
        assert_eq!(
            image::guess_format(&fixture.image_bytes).unwrap(),
            ImageFormat::Png
        );
    }
}
