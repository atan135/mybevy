use crate::{
    analysis::{
        AnalysisBoundingBox, AnalysisCoordinateSpace, AnalysisElement, UiReferenceAnalysis,
        VisualElementKind,
    },
    contract::{
        AdditionalReferenceRole, GenerationTask, ImageAuthorization, ImageColorSpace,
        MAX_REFERENCE_IMAGE_BYTES, ReferenceImage, TargetViewport,
    },
    directory::RunId,
    lifecycle::{TaskFailure, TaskFailureKind},
    planning::{PLANNING_PROTOCOL_VERSION, UiGenerationPlan},
    preprocess::{
        AppliedOrientation, ArtifactKind, CoordinateMapping, CoordinateSpace, FloatPoint,
        FloatRect, MAX_SYSTEM_UI_EXCLUSION_REGIONS, PREPROCESS_IMPLEMENTATION_VERSION,
        PREPROCESS_PROTOCOL_VERSION, PixelRect, ReferencePreprocessManifest,
        ReferenceValidationProfile,
    },
};
use image::{ColorType, ImageEncoder, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufWriter, Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const ASSET_STRATEGY_PROTOCOL_VERSION: u32 = 1;
pub const UI_ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const MAX_ASSET_ENTRIES: usize = 512;
pub const MAX_ASSET_SPEC_EDGE: u32 = 4096;
pub const MAX_ASSET_PIXELS: u64 = 16_777_216;
pub const MAX_DRAFT_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_STANDARD_PREVIEW_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ASSET_DECODE_ALLOC: u64 = 96 * 1024 * 1024;
const MAX_IMAGE_HEADER_ALLOC: u64 = 4 * 1024 * 1024;
const MAX_CATALOG_JSON_BYTES: usize = 1024 * 1024;
const MAX_PREPROCESS_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_PREPROCESS_ARTIFACTS: usize = 3;
const MAX_STANDARD_PREVIEW_PIXELS: u64 = 4_194_304;
const CATALOG_JSON: &str = include_str!("../assets/ui_asset_catalog.v1.json");
const ASSET_ID_MAX_BYTES: usize = 128;
const MAX_SEARCH_TERMS: usize = 8;
const MAX_TAG_BYTES: usize = 48;
const MAX_DIAGNOSTICS: usize = 512;
const MAX_CATALOG_TREE_ENTRIES: usize = 4096;
const MAX_CATALOG_DIRECTORY_DEPTH: usize = 32;
const MAX_FORMAL_ASSET_SNAPSHOT_ENTRIES: usize = 16_384;
const MAX_FORMAL_ASSET_SNAPSHOT_DEPTH: usize = 64;
const MAX_FORMAL_ASSET_SNAPSHOT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAssetKind {
    Raster,
    Font,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMode {
    Opaque,
    Straight,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogLicenseStatus {
    ProjectOwned,
    Redistributable,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogLicense {
    pub status: CatalogLicenseStatus,
    pub reference: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogAsset {
    pub asset_id: String,
    pub path: String,
    pub kind: CatalogAssetKind,
    pub sha256: String,
    pub byte_length: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub alpha: AlphaMode,
    pub license: CatalogLicense,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AssetCatalogDocument {
    schema_version: u32,
    assets: Vec<CatalogAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetCatalog {
    schema_version: u32,
    assets: Vec<CatalogAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetSearchQuery {
    pub kind: Option<CatalogAssetKind>,
    pub terms: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetSearchMatch {
    pub asset_id: String,
    pub score: u32,
    pub matched_terms: Vec<String>,
}

impl AssetCatalog {
    /// Loads the checked-in catalog and verifies it against the current production UI assets.
    pub fn load_repository(repository_root: &Path) -> Result<Self, TaskFailure> {
        Self::load_and_validate(repository_root, CATALOG_JSON.as_bytes())
    }

    pub fn load_and_validate(
        repository_root: &Path,
        catalog_json: &[u8],
    ) -> Result<Self, TaskFailure> {
        if catalog_json.is_empty() || catalog_json.len() > MAX_CATALOG_JSON_BYTES {
            return Err(TaskFailure::invalid(format!(
                "asset catalog JSON must be 1-{MAX_CATALOG_JSON_BYTES} bytes"
            )));
        }
        let mut document: AssetCatalogDocument =
            serde_json::from_slice(catalog_json).map_err(|error| {
                TaskFailure::invalid(format!(
                    "asset catalog does not match the strict schema: {error}"
                ))
            })?;
        if document.schema_version != UI_ASSET_CATALOG_SCHEMA_VERSION {
            return Err(TaskFailure::invalid(format!(
                "unsupported asset catalog schema_version {}; expected {UI_ASSET_CATALOG_SCHEMA_VERSION}",
                document.schema_version
            )));
        }
        let repository_root = canonical_regular_directory(repository_root, "repository root")?;
        let packaged_root = repository_root.join("project/assets");
        let ui_root =
            canonical_regular_directory(&packaged_root.join("ui"), "project UI asset root")?;
        if !ui_root.starts_with(&repository_root) {
            return Err(unsafe_path(
                &ui_root,
                "project UI asset root escapes repository",
            ));
        }
        document
            .assets
            .extend(load_generated_catalog_assets(&ui_root)?);
        if document.assets.is_empty() || document.assets.len() > MAX_ASSET_ENTRIES {
            return Err(TaskFailure::invalid(format!(
                "asset catalog must contain 1-{MAX_ASSET_ENTRIES} entries"
            )));
        }

        let discovered = discover_production_assets(&ui_root)?;
        let mut ids = BTreeSet::new();
        let mut folded_ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut folded_paths = BTreeSet::new();
        let mut assets = document.assets;
        for asset in &assets {
            validate_catalog_asset(asset)?;
            if !ids.insert(asset.asset_id.as_str())
                || !folded_ids.insert(asset.asset_id.to_ascii_lowercase())
            {
                return Err(TaskFailure::invalid(format!(
                    "asset catalog contains a duplicate or case-colliding asset_id `{}`",
                    asset.asset_id
                )));
            }
            if !paths.insert(asset.path.clone())
                || !folded_paths.insert(asset.path.to_ascii_lowercase())
            {
                return Err(TaskFailure::invalid(format!(
                    "asset catalog contains a duplicate or case-colliding path `{}`",
                    asset.path
                )));
            }
            validate_catalog_file(&packaged_root, &ui_root, asset)?;
        }
        if paths != discovered {
            let missing: Vec<_> = discovered.difference(&paths).cloned().collect();
            let stale: Vec<_> = paths.difference(&discovered).cloned().collect();
            return Err(TaskFailure::invalid(format!(
                "asset catalog coverage differs from production files; uncatalogued={missing:?}; stale={stale:?}"
            )));
        }
        assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        Ok(Self {
            schema_version: document.schema_version,
            assets,
        })
    }

    pub fn resolve(&self, asset_id: &str) -> Option<&CatalogAsset> {
        self.assets
            .binary_search_by(|asset| asset.asset_id.as_str().cmp(asset_id))
            .ok()
            .map(|index| &self.assets[index])
    }

    /// Looks up a verified packaged path without allowing callers to infer arbitrary filesystem
    /// locations from an asset ID or model output.
    pub fn resolve_by_path(&self, path: &str) -> Option<&CatalogAsset> {
        self.assets.iter().find(|asset| asset.path == path)
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn assets(&self) -> &[CatalogAsset] {
        &self.assets
    }

    pub fn search(&self, query: &AssetSearchQuery) -> Result<Vec<AssetSearchMatch>, TaskFailure> {
        if query.terms.is_empty() || query.terms.len() > MAX_SEARCH_TERMS {
            return Err(TaskFailure::invalid(format!(
                "asset search requires 1-{MAX_SEARCH_TERMS} terms"
            )));
        }
        let mut terms = BTreeSet::new();
        for term in &query.terms {
            let term = term.trim().to_ascii_lowercase();
            if !is_safe_tag(&term) {
                return Err(TaskFailure::invalid(
                    "asset search terms must be bounded lowercase ASCII labels",
                ));
            }
            terms.insert(term);
        }
        let mut matches = Vec::new();
        for asset in &self.assets {
            if query.kind.is_some_and(|kind| kind != asset.kind) {
                continue;
            }
            let id_parts: BTreeSet<_> = asset.asset_id.split(['.', '_']).collect();
            let tag_set: BTreeSet<_> = asset.tags.iter().map(String::as_str).collect();
            let mut score = 0;
            let mut matched_terms = Vec::new();
            for term in &terms {
                let exact_tag = tag_set.contains(term.as_str());
                let exact_id = id_parts.contains(term.as_str());
                if exact_tag || exact_id {
                    score += if exact_id { 4 } else { 3 };
                    matched_terms.push(term.clone());
                }
            }
            if score > 0 {
                matches.push(AssetSearchMatch {
                    asset_id: asset.asset_id.clone(),
                    score,
                    matched_terms,
                });
            }
        }
        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.asset_id.cmp(&right.asset_id))
        });
        Ok(matches)
    }
}

/// Loads one checked catalog fragment per promoted page. Fragments are deliberately constrained
/// to `ui/documents/approved/<page>/...` so a promotion cannot extend the stable catalog for
/// arbitrary existing production paths.
fn load_generated_catalog_assets(ui_root: &Path) -> Result<Vec<CatalogAsset>, TaskFailure> {
    let approved_root = ui_root.join("documents/approved");
    match fs::symlink_metadata(&approved_root) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(unsafe_path(
                    &approved_root,
                    "approved document root is not a real directory",
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(TaskFailure::invalid(format!(
                "approved document root metadata unavailable: {error}"
            )));
        }
    }
    let mut directories = Vec::new();
    for entry in fs::read_dir(&approved_root).map_err(|error| {
        TaskFailure::invalid(format!("approved document root cannot be read: {error}"))
    })? {
        let entry = entry.map_err(|error| {
            TaskFailure::invalid(format!("approved document entry failed: {error}"))
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            TaskFailure::invalid(format!(
                "approved document entry metadata unavailable: {error}"
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(unsafe_path(
                &path,
                "approved document pages must use one real directory per promotion",
            ));
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !is_safe_tag(name) {
            return Err(TaskFailure::invalid(
                "approved document promotion directory name is not a safe lowercase label",
            ));
        }
        directories.push(path);
    }
    if directories.len() > MAX_ASSET_ENTRIES {
        return Err(TaskFailure::invalid(
            "approved document catalog directory count exceeds budget",
        ));
    }
    directories.sort();
    let mut assets = Vec::new();
    for directory in directories {
        let manifest = directory.join("catalog.v1.json");
        match fs::symlink_metadata(&manifest) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(unsafe_path(
                        &manifest,
                        "approved document catalog fragment is not a regular file",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(TaskFailure::invalid(format!(
                    "approved document catalog fragment metadata unavailable: {error}"
                )));
            }
        }
        let bytes = read_bounded(&manifest, MAX_CATALOG_JSON_BYTES as u64)?;
        let fragment: AssetCatalogDocument = serde_json::from_slice(&bytes).map_err(|error| {
            TaskFailure::invalid(format!(
                "approved document catalog fragment is invalid: {error}"
            ))
        })?;
        if fragment.schema_version != UI_ASSET_CATALOG_SCHEMA_VERSION || fragment.assets.is_empty()
        {
            return Err(TaskFailure::invalid(
                "approved document catalog fragment has an unsupported schema or no assets",
            ));
        }
        let folder = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let prefix = format!("ui/documents/approved/{folder}/");
        for asset in fragment.assets {
            if !asset.path.starts_with(&prefix) {
                return Err(TaskFailure::invalid(
                    "approved document catalog fragment references a path outside its promotion directory",
                ));
            }
            assets.push(asset);
        }
    }
    Ok(assets)
}

fn validate_catalog_asset(asset: &CatalogAsset) -> Result<(), TaskFailure> {
    if !is_safe_asset_id(&asset.asset_id) {
        return Err(TaskFailure::invalid(format!(
            "asset_id `{}` is not a stable dotted lowercase ASCII ID",
            asset.asset_id
        )));
    }
    validate_packaged_path(&asset.path)?;
    if !is_sha256(&asset.sha256) || asset.byte_length == 0 {
        return Err(TaskFailure::invalid(format!(
            "asset `{}` has invalid hash or byte length",
            asset.asset_id
        )));
    }
    match asset.kind {
        CatalogAssetKind::Raster => {
            if asset
                .width
                .is_none_or(|value| value == 0 || value > MAX_ASSET_SPEC_EDGE)
                || asset
                    .height
                    .is_none_or(|value| value == 0 || value > MAX_ASSET_SPEC_EDGE)
                || asset.alpha == AlphaMode::NotApplicable
                || !asset.path.to_ascii_lowercase().ends_with(".png")
            {
                return Err(TaskFailure::invalid(format!(
                    "raster asset `{}` has invalid dimensions, alpha, or extension",
                    asset.asset_id
                )));
            }
        }
        CatalogAssetKind::Font => {
            if asset.width.is_some()
                || asset.height.is_some()
                || asset.alpha != AlphaMode::NotApplicable
                || !(asset.path.to_ascii_lowercase().ends_with(".otf")
                    || asset.path.to_ascii_lowercase().ends_with(".ttf"))
            {
                return Err(TaskFailure::invalid(format!(
                    "font asset `{}` has invalid raster metadata or extension",
                    asset.asset_id
                )));
            }
        }
    }
    if asset.tags.is_empty() || asset.tags.len() > 16 {
        return Err(TaskFailure::invalid("asset tags must contain 1-16 entries"));
    }
    let mut tags = BTreeSet::new();
    if asset
        .tags
        .iter()
        .any(|tag| !is_safe_tag(tag) || !tags.insert(tag))
    {
        return Err(TaskFailure::invalid(format!(
            "asset `{}` contains invalid or duplicate tags",
            asset.asset_id
        )));
    }
    let has_license = asset
        .license
        .reference
        .as_deref()
        .is_some_and(|reference| validate_packaged_path(reference).is_ok());
    if asset.license.status == CatalogLicenseStatus::Unknown {
        if asset.license.reference.is_some() {
            return Err(TaskFailure::invalid(
                "unknown catalog licenses cannot claim a reference",
            ));
        }
    } else if !has_license {
        return Err(TaskFailure::invalid(format!(
            "asset `{}` requires a safe license reference",
            asset.asset_id
        )));
    }
    Ok(())
}

fn validate_catalog_file(
    packaged_root: &Path,
    ui_root: &Path,
    asset: &CatalogAsset,
) -> Result<(), TaskFailure> {
    if let Some(reference) = &asset.license.reference {
        let license_path = packaged_root.join(reference);
        reject_symlink_chain(packaged_root, &license_path)?;
        let canonical_license = fs::canonicalize(&license_path).map_err(|error| {
            TaskFailure::invalid(format!(
                "catalog asset `{}` license reference cannot be resolved: {error}",
                asset.asset_id
            ))
        })?;
        let metadata = fs::symlink_metadata(&license_path).map_err(|error| {
            TaskFailure::invalid(format!("catalog license metadata unavailable: {error}"))
        })?;
        if !canonical_license.starts_with(ui_root)
            || !metadata.is_file()
            || metadata.file_type().is_symlink()
        {
            return Err(unsafe_path(
                &license_path,
                "catalog license reference is not a regular file within project/assets/ui",
            ));
        }
    }
    let path = packaged_root.join(Path::new(&asset.path));
    reject_symlink_chain(packaged_root, &path)?;
    let canonical = fs::canonicalize(&path).map_err(|error| {
        TaskFailure::invalid(format!(
            "catalog asset `{}` cannot be resolved: {error}",
            asset.asset_id
        ))
    })?;
    if !canonical.starts_with(ui_root) {
        return Err(unsafe_path(
            &path,
            "catalog asset resolves outside project/assets/ui",
        ));
    }
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        TaskFailure::invalid(format!("catalog asset metadata unavailable: {error}"))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(unsafe_path(&path, "catalog asset is not a regular file"));
    }
    let bytes = read_bounded(&path, MAX_DRAFT_ASSET_BYTES.max(12 * 1024 * 1024))?;
    if bytes.len() as u64 != asset.byte_length || sha256_bytes(&bytes) != asset.sha256 {
        return Err(TaskFailure::invalid(format!(
            "catalog asset `{}` hash or byte length changed",
            asset.asset_id
        )));
    }
    if asset.kind == CatalogAssetKind::Raster {
        let reader = bounded_image_reader(&bytes, true).map_err(|error| {
            TaskFailure::invalid(format!("catalog raster format error: {error}"))
        })?;
        if reader.format() != Some(ImageFormat::Png) {
            return Err(TaskFailure::invalid("catalog raster must be PNG"));
        }
        let image = reader.decode().map_err(|error| {
            TaskFailure::invalid(format!("catalog raster decode failed: {error}"))
        })?;
        if Some(image.width()) != asset.width || Some(image.height()) != asset.height {
            return Err(TaskFailure::invalid(format!(
                "catalog asset `{}` dimensions changed",
                asset.asset_id
            )));
        }
        let has_alpha = image.color().has_alpha();
        if (asset.alpha == AlphaMode::Straight) != has_alpha {
            return Err(TaskFailure::invalid(format!(
                "catalog asset `{}` alpha metadata changed",
                asset.asset_id
            )));
        }
    }
    Ok(())
}

fn discover_production_assets(ui_root: &Path) -> Result<BTreeSet<String>, TaskFailure> {
    let mut found = BTreeSet::new();
    let mut folded_paths = BTreeMap::new();
    let mut visited_entries = 0_usize;
    for directory in ["atlas", "icons", "images", "fonts"] {
        let root = ui_root.join(directory);
        reject_symlink_chain(ui_root, &root)?;
        let mut pending = vec![(root, 0_usize)];
        while let Some((current, depth)) = pending.pop() {
            if depth > MAX_CATALOG_DIRECTORY_DEPTH {
                return Err(TaskFailure::invalid(format!(
                    "production asset directory depth exceeds {MAX_CATALOG_DIRECTORY_DEPTH}"
                )));
            }
            reject_symlink_chain(ui_root, &current)?;
            let directory_entries = fs::read_dir(&current).map_err(|error| {
                TaskFailure::invalid(format!(
                    "production asset directory cannot be read: {error}"
                ))
            })?;
            let mut entries = Vec::new();
            for entry in directory_entries {
                visited_entries = visited_entries.checked_add(1).ok_or_else(|| {
                    TaskFailure::invalid("production asset traversal count overflow")
                })?;
                if visited_entries > MAX_CATALOG_TREE_ENTRIES {
                    return Err(TaskFailure::invalid(format!(
                        "production asset tree exceeds {MAX_CATALOG_TREE_ENTRIES} entries"
                    )));
                }
                entries.push(entry.map_err(|error| TaskFailure::invalid(error.to_string()))?);
            }
            entries.sort_by_key(|entry| entry.file_name());
            let mut child_directories = Vec::new();
            for entry in entries {
                let entry_path = entry.path();
                let file_type = entry
                    .file_type()
                    .map_err(|error| TaskFailure::invalid(error.to_string()))?;
                if file_type.is_symlink() {
                    return Err(unsafe_path(
                        &entry_path,
                        "production asset entry is a symlink",
                    ));
                }
                let relative = entry_path
                    .strip_prefix(ui_root)
                    .map_err(|_| unsafe_path(&entry_path, "discovered asset escapes UI root"))?;
                let relative = path_to_forward_slashes(relative)?;
                record_case_insensitive_path(&mut folded_paths, &relative)?;
                if file_type.is_dir() {
                    child_directories.push(entry_path);
                    continue;
                }
                if !file_type.is_file() {
                    return Err(unsafe_path(
                        &entry_path,
                        "production asset entry is not a regular file or directory",
                    ));
                }
                let extension = entry_path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "otf" | "ttf") {
                    found.insert(format!("ui/{relative}"));
                }
            }
            child_directories.sort();
            for child in child_directories.into_iter().rev() {
                pending.push((child, depth + 1));
            }
        }
    }
    Ok(found)
}

fn record_case_insensitive_path(
    folded_paths: &mut BTreeMap<String, String>,
    relative: &str,
) -> Result<(), TaskFailure> {
    let folded = relative.to_ascii_lowercase();
    if let Some(previous) = folded_paths.insert(folded, relative.to_owned())
        && previous != relative
    {
        return Err(TaskFailure::invalid(format!(
            "production UI asset tree contains case-colliding paths `{previous}` and `{relative}`"
        )));
    }
    Ok(())
}

fn validate_packaged_path(value: &str) -> Result<(), TaskFailure> {
    if value.is_empty()
        || value.contains('\\')
        || !value.starts_with("ui/")
        || Path::new(value).is_absolute()
        || Path::new(value)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(TaskFailure::invalid(format!(
            "packaged path `{value}` must be a normalized relative ui/... path"
        )));
    }
    if path_to_forward_slashes(Path::new(value))? != value {
        return Err(TaskFailure::invalid(format!(
            "packaged path `{value}` is not canonically normalized"
        )));
    }
    Ok(())
}

fn path_to_forward_slashes(path: &Path) -> Result<String, TaskFailure> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(unsafe_path(path, "path is not normalized"));
        };
        parts.push(
            part.to_str()
                .ok_or_else(|| unsafe_path(path, "asset paths must be valid UTF-8"))?,
        );
    }
    Ok(parts.join("/"))
}

fn is_safe_asset_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= ASSET_ID_MAX_BYTES
        && value.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars.next().is_some_and(|first| first.is_ascii_lowercase())
                && chars.all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        })
}

fn is_safe_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TAG_BYTES
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetDisposition {
    ExistingAsset,
    Programmatic,
    AuthorizedCrop,
    Recreate,
    Generate,
    Placeholder,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammaticRepresentation {
    LayoutContainer,
    Text,
    SolidSurface,
    Border,
    ShadowOrDecoration,
    StatusPrimitive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetUsage {
    Background,
    ContentImage,
    Icon,
    Decoration,
    NineSlice,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredColorSpace {
    Srgb,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SliceInsets {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetSpecification {
    pub width: u32,
    pub height: u32,
    pub alpha: AlphaMode,
    pub slice_insets: Option<SliceInsets>,
    pub color_space: RequiredColorSpace,
    pub usage: AssetUsage,
}

impl AssetSpecification {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        let pixels = u64::from(self.width) * u64::from(self.height);
        if self.width == 0
            || self.height == 0
            || self.width > MAX_ASSET_SPEC_EDGE
            || self.height > MAX_ASSET_SPEC_EDGE
            || pixels > MAX_ASSET_PIXELS
            || self.alpha == AlphaMode::NotApplicable
        {
            return Err(TaskFailure::invalid(format!(
                "asset specification must be a 1-{MAX_ASSET_SPEC_EDGE}px raster within {MAX_ASSET_PIXELS} pixels"
            )));
        }
        match (self.usage, self.slice_insets) {
            (AssetUsage::NineSlice, Some(insets)) => {
                let horizontal = insets.left.checked_add(insets.right).ok_or_else(|| {
                    TaskFailure::invalid("nine-slice horizontal insets overflow u32")
                })?;
                let vertical = insets.top.checked_add(insets.bottom).ok_or_else(|| {
                    TaskFailure::invalid("nine-slice vertical insets overflow u32")
                })?;
                if horizontal >= self.width || vertical >= self.height {
                    return Err(TaskFailure::invalid(
                        "nine-slice insets must leave a non-empty center region",
                    ));
                }
            }
            (AssetUsage::NineSlice, None) => {
                return Err(TaskFailure::invalid(
                    "nine-slice specifications require explicit slice insets",
                ));
            }
            (_, Some(_)) => {
                return Err(TaskFailure::invalid(
                    "slice insets are only valid for nine-slice assets",
                ));
            }
            (_, None) => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedApprovalStatus {
    PendingHumanReview,
    Approved,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedLicenseStatus {
    Pending,
    ProjectOwned,
    Redistributable,
    Denied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedAssetLicense {
    pub status: GeneratedLicenseStatus,
    pub reference: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationPromptSummary {
    /// Controlled labels only. Full prompts and free-form sensitive text are deliberately absent.
    pub subject_tags: Vec<String>,
    pub style_tags: Vec<String>,
}

impl GenerationPromptSummary {
    fn validate(&self) -> Result<(), TaskFailure> {
        for (name, values) in [
            ("subject_tags", &self.subject_tags),
            ("style_tags", &self.style_tags),
        ] {
            if values.is_empty() || values.len() > 12 {
                return Err(TaskFailure::invalid(format!(
                    "prompt summary {name} must contain 1-12 controlled tags"
                )));
            }
            let mut unique = BTreeSet::new();
            if values
                .iter()
                .any(|value| !is_safe_tag(value) || !unique.insert(value))
            {
                return Err(TaskFailure::invalid(format!(
                    "prompt summary {name} contains invalid or duplicate tags"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationProvenance {
    pub tool_id: String,
    pub tool_version: String,
    pub prompt_summary: GenerationPromptSummary,
    pub license: GeneratedAssetLicense,
    pub approval_status: RecordedApprovalStatus,
}

impl GenerationProvenance {
    fn validate_for_draft(&self) -> Result<(), TaskFailure> {
        if !is_safe_tool_label(&self.tool_id) || !is_safe_tool_label(&self.tool_version) {
            return Err(TaskFailure::invalid(
                "generation tool ID/version must be bounded printable ASCII labels",
            ));
        }
        self.prompt_summary.validate()?;
        if self.approval_status != RecordedApprovalStatus::PendingHumanReview {
            return Err(TaskFailure::invalid(
                "new generated drafts must begin pending human review; model output cannot approve itself",
            ));
        }
        match self.license.status {
            GeneratedLicenseStatus::Pending => {
                if self.license.reference.is_some() {
                    return Err(TaskFailure::invalid(
                        "pending generated licenses cannot claim a license reference",
                    ));
                }
            }
            GeneratedLicenseStatus::Denied => {
                return Err(TaskFailure::invalid(
                    "generation with denied licensing cannot enter the draft strategy",
                ));
            }
            GeneratedLicenseStatus::ProjectOwned | GeneratedLicenseStatus::Redistributable => {
                if self
                    .license
                    .reference
                    .as_deref()
                    .is_none_or(|value| !is_bounded_record(value, 512))
                {
                    return Err(TaskFailure::invalid(
                        "non-pending generated licenses require a bounded reference",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceholderReason {
    NoApprovedMatch,
    AuthorizationRejected,
    AwaitingRecreation,
    AwaitingGeneration,
    UnsupportedAssetKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "disposition", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssetDecision {
    ExistingAsset {
        asset_id: String,
    },
    Programmatic {
        representation: ProgrammaticRepresentation,
    },
    AuthorizedCrop {
        specification: AssetSpecification,
    },
    Recreate {
        specification: AssetSpecification,
    },
    Generate {
        specification: AssetSpecification,
        provenance: GenerationProvenance,
    },
    Placeholder {
        reason: PlaceholderReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetDecisionRequest {
    pub element_id: String,
    pub decision: AssetDecision,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CropSourceRecord {
    run_id: String,
    reference_id: String,
    source_sha256: String,
    source_byte_length: u64,
    preprocess_cache_key: String,
    preprocess_manifest_sha256: String,
    standard_preview_sha256: String,
    standard_preview_byte_length: u64,
    standard_preview_width: u32,
    standard_preview_height: u32,
    standard_preview_file_name: String,
    preview_crop: PixelRect,
    exif_normalized_crop: FloatRect,
    coordinate_convention: String,
    authorization: ImageAuthorization,
    license_reference: String,
    declared_color_space: ImageColorSpace,
    processing_steps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetStrategyEntry {
    strategy_id: String,
    element_id: String,
    disposition: AssetDisposition,
    existing_asset_id: Option<String>,
    programmatic: Option<ProgrammaticRepresentation>,
    crop: Option<CropSourceRecord>,
    specification: Option<AssetSpecification>,
    generation: Option<GenerationProvenance>,
    placeholder_reason: Option<PlaceholderReason>,
    approval_status: RecordedApprovalStatus,
}

impl AssetStrategyEntry {
    pub fn element_id(&self) -> &str {
        &self.element_id
    }

    pub fn disposition(&self) -> AssetDisposition {
        self.disposition
    }

    pub fn existing_asset_id(&self) -> Option<&str> {
        self.existing_asset_id.as_deref()
    }

    pub fn specification(&self) -> Option<&AssetSpecification> {
        self.specification.as_ref()
    }
}

impl CropSourceRecord {
    pub fn reference_id(&self) -> &str {
        &self.reference_id
    }

    pub fn preview_crop(&self) -> PixelRect {
        self.preview_crop
    }

    pub fn exif_normalized_crop(&self) -> FloatRect {
        self.exif_normalized_crop
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetStrategyDiagnostic {
    pub code: String,
    pub element_id: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetStrategyManifest {
    pub protocol_version: u32,
    pub analysis_id: String,
    pub planning_protocol_version: u32,
    pub catalog_schema_version: u32,
    pub entries: Vec<AssetStrategyEntry>,
    pub diagnostics: Vec<AssetStrategyDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct TrustedAssetSource {
    run_id: String,
    manifest: ReferencePreprocessManifest,
    manifest_sha256: String,
    authorization: ImageAuthorization,
    license_reference: Option<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TrustedCacheKeyInput<'a> {
    protocol_version: u32,
    implementation_version: &'a str,
    source_sha256: &'a str,
    reference_id: &'a str,
    declared_metadata: &'a crate::contract::ImageInputMetadata,
    viewport: TargetViewport,
    validation_profile: ReferenceValidationProfile,
    options: &'a crate::preprocess::ReferencePreprocessOptions,
}

impl TrustedAssetSource {
    pub fn from_task_and_manifest(
        task: &GenerationTask,
        manifest: &ReferencePreprocessManifest,
        manifest_bytes: &[u8],
    ) -> Result<Self, TaskFailure> {
        task.validate()?;
        if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_PREPROCESS_MANIFEST_BYTES {
            return Err(TaskFailure::invalid(format!(
                "trusted preprocess manifest must be 1-{MAX_PREPROCESS_MANIFEST_BYTES} bytes"
            )));
        }
        let decoded: ReferencePreprocessManifest = serde_json::from_slice(manifest_bytes)
            .map_err(|_| TaskFailure::invalid("trusted preprocess manifest bytes are invalid"))?;
        if decoded != *manifest {
            return Err(TaskFailure::invalid(
                "trusted preprocess bytes do not describe the supplied manifest",
            ));
        }
        let reference = find_task_reference(task, &manifest.reference_id).ok_or_else(|| {
            TaskFailure::invalid("preprocess manifest reference is absent from generation task")
        })?;
        let (viewport, validation_profile) = task_reference_context(task, &manifest.reference_id)?;
        if manifest.protocol_version != PREPROCESS_PROTOCOL_VERSION
            || manifest.implementation_version != PREPROCESS_IMPLEMENTATION_VERSION
            || manifest.validation_profile != validation_profile
        {
            return Err(TaskFailure::invalid(
                "trusted preprocess manifest protocol, implementation, or profile is unsupported",
            ));
        }
        if reference.metadata.sha256 != manifest.source_sha256
            || reference.metadata.original_size != manifest.source_raw_size
            || reference.metadata.original_size != manifest.coordinate_mapping.raw_size
            || reference.metadata.orientation != manifest.embedded_metadata.declared_orientation
            || reference.metadata.color_space != manifest.embedded_metadata.declared_color_space
            || manifest.source_byte_length == 0
            || manifest.source_byte_length > MAX_REFERENCE_IMAGE_BYTES
        {
            return Err(TaskFailure::invalid(
                "task source metadata disagrees with preprocess manifest",
            ));
        }
        let expected_cache_key =
            trusted_cache_key(reference, manifest, viewport, validation_profile)?;
        if manifest.cache_key != expected_cache_key {
            return Err(TaskFailure::invalid(
                "preprocess cache key does not bind the trusted task metadata and options",
            ));
        }
        validate_manifest_mapping(manifest, viewport)?;
        validate_manifest_artifacts(manifest)?;
        let preview = authoritative_preview(manifest)?;
        let preview_name = Path::new(&preview.file_name);
        if !manifest.original_remains_authoritative
            || !is_sha256(&manifest.source_sha256)
            || !is_sha256(&manifest.cache_key)
            || !is_sha256(&preview.sha256)
            || preview_name.components().count() != 1
            || !preview.file_name.ends_with(".png")
        {
            return Err(TaskFailure::invalid(
                "trusted preprocess manifest identity or preview file name is unsafe",
            ));
        }
        if preview.width != manifest.coordinate_mapping.preview_size.width
            || preview.height != manifest.coordinate_mapping.preview_size.height
        {
            return Err(TaskFailure::invalid(
                "preprocess preview dimensions disagree with coordinate mapping",
            ));
        }
        Ok(Self {
            run_id: task.run_id.clone(),
            manifest: manifest.clone(),
            manifest_sha256: sha256_bytes(manifest_bytes),
            authorization: reference.metadata.provenance.authorization,
            license_reference: reference.metadata.provenance.license_reference.clone(),
        })
    }

    pub fn reference_id(&self) -> &str {
        &self.manifest.reference_id
    }
}

fn task_reference_context(
    task: &GenerationTask,
    reference_id: &str,
) -> Result<(TargetViewport, ReferenceValidationProfile), TaskFailure> {
    let default_viewport = task.target_viewport.ok_or_else(|| {
        TaskFailure::new(
            TaskFailureKind::TargetViewportMissing,
            "target viewport is required for trusted preprocess evidence",
            None,
        )
    })?;
    if task.primary_reference.reference_id == reference_id {
        return Ok((default_viewport, ReferenceValidationProfile::PageReference));
    }
    let reference = task
        .additional_references
        .iter()
        .find(|candidate| candidate.image.reference_id == reference_id)
        .ok_or_else(|| TaskFailure::invalid("trusted preprocess reference is absent from task"))?;
    Ok(match reference.role {
        AdditionalReferenceRole::Viewport { viewport } => {
            (viewport, ReferenceValidationProfile::PageReference)
        }
        AdditionalReferenceRole::Detail { .. } => (
            default_viewport,
            ReferenceValidationProfile::DetailReference,
        ),
        AdditionalReferenceRole::State { .. } => {
            (default_viewport, ReferenceValidationProfile::PageReference)
        }
    })
}

fn trusted_cache_key(
    reference: &ReferenceImage,
    manifest: &ReferencePreprocessManifest,
    viewport: TargetViewport,
    validation_profile: ReferenceValidationProfile,
) -> Result<String, TaskFailure> {
    let input = TrustedCacheKeyInput {
        protocol_version: PREPROCESS_PROTOCOL_VERSION,
        implementation_version: PREPROCESS_IMPLEMENTATION_VERSION,
        source_sha256: &manifest.source_sha256,
        reference_id: &manifest.reference_id,
        declared_metadata: &reference.metadata,
        viewport,
        validation_profile,
        options: &manifest.options,
    };
    serde_json::to_vec(&input)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| TaskFailure::invalid(format!("preprocess cache key failed: {error}")))
}

fn validate_manifest_mapping(
    manifest: &ReferencePreprocessManifest,
    viewport: TargetViewport,
) -> Result<(), TaskFailure> {
    let normalized_size = normalized_size(
        manifest.source_raw_size,
        manifest.embedded_metadata.applied_orientation,
    );
    let crop = manifest
        .options
        .crop
        .unwrap_or_else(|| PixelRect::full(normalized_size));
    let expected = CoordinateMapping::new(
        manifest.source_raw_size,
        manifest.embedded_metadata.applied_orientation,
        crop,
        manifest.coordinate_mapping.preview_size,
        viewport,
    )?;
    if expected != manifest.coordinate_mapping {
        return Err(TaskFailure::invalid(
            "trusted preprocess coordinate mapping is not derived from its source/options/viewport",
        ));
    }
    if manifest.options.system_ui_exclusions.len() > MAX_SYSTEM_UI_EXCLUSION_REGIONS {
        return Err(TaskFailure::invalid(format!(
            "trusted preprocess manifest exceeds {MAX_SYSTEM_UI_EXCLUSION_REGIONS} system UI exclusions"
        )));
    }
    Ok(())
}

fn validate_manifest_artifacts(manifest: &ReferencePreprocessManifest) -> Result<(), TaskFailure> {
    if manifest.artifacts.is_empty() || manifest.artifacts.len() > MAX_PREPROCESS_ARTIFACTS {
        return Err(TaskFailure::invalid(format!(
            "trusted preprocess manifest must contain 1-{MAX_PREPROCESS_ARTIFACTS} artifacts"
        )));
    }
    let mut names = BTreeSet::new();
    for artifact in &manifest.artifacts {
        let file_name = Path::new(&artifact.file_name);
        let pixels = u64::from(artifact.width) * u64::from(artifact.height);
        if file_name.components().count() != 1
            || !artifact.file_name.ends_with(".png")
            || !names.insert(artifact.file_name.as_str())
            || !is_sha256(&artifact.sha256)
            || artifact.byte_length == 0
            || artifact.byte_length > MAX_STANDARD_PREVIEW_BYTES
            || artifact.width == 0
            || artifact.height == 0
            || artifact.width > MAX_ASSET_SPEC_EDGE
            || artifact.height > MAX_ASSET_SPEC_EDGE
            || pixels > MAX_STANDARD_PREVIEW_PIXELS
        {
            return Err(TaskFailure::invalid(
                "trusted preprocess artifact metadata exceeds path, hash, byte, or pixel budgets",
            ));
        }
    }
    Ok(())
}

fn normalized_size(
    raw: crate::contract::PixelSize,
    orientation: AppliedOrientation,
) -> crate::contract::PixelSize {
    match orientation {
        AppliedOrientation::Rotate90
        | AppliedOrientation::Rotate270
        | AppliedOrientation::Rotate90MirrorHorizontal
        | AppliedOrientation::Rotate270MirrorHorizontal => crate::contract::PixelSize {
            width: raw.height,
            height: raw.width,
        },
        _ => raw,
    }
}

pub fn build_asset_strategy(
    analysis: &UiReferenceAnalysis,
    plan: &UiGenerationPlan,
    catalog: &AssetCatalog,
    trusted_sources: &[TrustedAssetSource],
    decisions: &[AssetDecisionRequest],
) -> Result<AssetStrategyManifest, TaskFailure> {
    let semantic_report = analysis.validate_semantics();
    if !semantic_report.valid {
        let first = semantic_report
            .diagnostics
            .first()
            .map(|diagnostic| {
                format!(
                    "{} at {}: {}",
                    diagnostic.code, diagnostic.path, diagnostic.message
                )
            })
            .unwrap_or_else(|| "unknown semantic failure".to_owned());
        return Err(TaskFailure::invalid(format!(
            "asset strategy requires a semantically valid analysis; {first}"
        )));
    }
    if plan.protocol_version != PLANNING_PROTOCOL_VERSION {
        return Err(TaskFailure::invalid(format!(
            "asset strategy requires planning protocol version {PLANNING_PROTOCOL_VERSION}"
        )));
    }
    if plan.analysis_id != analysis.analysis_id {
        return Err(TaskFailure::invalid(
            "asset strategy plan and analysis IDs differ",
        ));
    }
    if analysis.elements.len() > MAX_ASSET_ENTRIES || decisions.len() > MAX_ASSET_ENTRIES {
        return Err(TaskFailure::invalid(format!(
            "asset strategy exceeds the {MAX_ASSET_ENTRIES}-entry budget"
        )));
    }
    let sources: BTreeMap<_, _> = trusted_sources
        .iter()
        .map(|source| (source.reference_id(), source))
        .collect();
    if sources.len() != trusted_sources.len() {
        return Err(TaskFailure::invalid(
            "trusted asset sources contain duplicate reference IDs",
        ));
    }
    if trusted_sources
        .iter()
        .any(|source| source.run_id != analysis.run_id)
    {
        return Err(TaskFailure::invalid(
            "trusted asset source run ID differs from analysis run ID",
        ));
    }
    let mut decision_map = BTreeMap::new();
    for request in decisions {
        if !is_safe_asset_id(&request.element_id)
            || decision_map
                .insert(request.element_id.as_str(), &request.decision)
                .is_some()
        {
            return Err(TaskFailure::invalid(
                "asset decisions contain an invalid or duplicate element ID",
            ));
        }
    }

    let mut elements: Vec<_> = analysis.elements.iter().collect();
    elements.sort_by_key(|element| &element.element_id);
    let element_ids: BTreeSet<_> = elements
        .iter()
        .map(|element| element.element_id.as_str())
        .collect();
    if decision_map.keys().any(|id| !element_ids.contains(id)) {
        return Err(TaskFailure::invalid(
            "asset decision refers to an unknown analysis element",
        ));
    }

    let analysis_references: BTreeMap<_, _> = analysis
        .references
        .iter()
        .map(|reference| (reference.reference_id.as_str(), reference))
        .collect();
    let mut entries = Vec::with_capacity(elements.len());
    let mut diagnostics = Vec::new();
    for element in elements {
        let decision = decision_map.get(element.element_id.as_str()).copied();
        let entry = strategy_entry(
            element,
            decision,
            catalog,
            &sources,
            &analysis_references,
            &mut diagnostics,
        )?;
        entries.push(entry);
    }
    if diagnostics.len() > MAX_DIAGNOSTICS {
        return Err(TaskFailure::invalid(format!(
            "asset diagnostics exceed the {MAX_DIAGNOSTICS}-entry budget"
        )));
    }
    diagnostics.sort_by(|left, right| {
        left.element_id
            .cmp(&right.element_id)
            .then_with(|| left.code.cmp(&right.code))
    });
    Ok(AssetStrategyManifest {
        protocol_version: ASSET_STRATEGY_PROTOCOL_VERSION,
        analysis_id: analysis.analysis_id.clone(),
        planning_protocol_version: plan.protocol_version,
        catalog_schema_version: catalog.schema_version,
        entries,
        diagnostics,
    })
}

fn strategy_entry(
    element: &AnalysisElement,
    decision: Option<&AssetDecision>,
    catalog: &AssetCatalog,
    sources: &BTreeMap<&str, &TrustedAssetSource>,
    analysis_references: &BTreeMap<&str, &crate::analysis::AnalysisReference>,
    diagnostics: &mut Vec<AssetStrategyDiagnostic>,
) -> Result<AssetStrategyEntry, TaskFailure> {
    let default_programmatic = programmatic_default(element.kind);
    let decision = match decision {
        Some(decision) => decision,
        None if default_programmatic.is_some() => {
            return Ok(programmatic_entry(element, default_programmatic.unwrap()));
        }
        None => {
            diagnostics.push(AssetStrategyDiagnostic {
                code: "ASSET_PLACEHOLDER_NO_APPROVED_MATCH".to_owned(),
                element_id: element.element_id.clone(),
                message: "asset-bearing element has no explicit approved match; using placeholder"
                    .to_owned(),
            });
            return Ok(placeholder_entry(
                element,
                PlaceholderReason::NoApprovedMatch,
            ));
        }
    };

    match decision {
        AssetDecision::ExistingAsset { asset_id } => {
            if !is_asset_bearing(element.kind) {
                return Err(TaskFailure::invalid(format!(
                    "element `{}` cannot be replaced by a raster asset",
                    element.element_id
                )));
            }
            let asset = catalog.resolve(asset_id).ok_or_else(|| {
                TaskFailure::invalid(format!(
                    "element `{}` requested unknown stable asset ID `{asset_id}`",
                    element.element_id
                ))
            })?;
            if asset.kind != CatalogAssetKind::Raster {
                return Err(TaskFailure::invalid(
                    "visual elements can only match raster catalog assets",
                ));
            }
            if !existing_asset_compatible(element.kind, asset) {
                return Err(TaskFailure::invalid(format!(
                    "stable asset ID `{asset_id}` is incompatible with `{}` element `{}`",
                    visual_kind_name(element.kind),
                    element.element_id
                )));
            }
            if asset.license.status == CatalogLicenseStatus::Unknown {
                diagnostics.push(AssetStrategyDiagnostic {
                    code: "ASSET_EXISTING_LICENSE_UNKNOWN".to_owned(),
                    element_id: element.element_id.clone(),
                    message: "existing packaged asset is matched, but catalog license is unknown and requires review before promotion".to_owned(),
                });
            }
            Ok(AssetStrategyEntry {
                strategy_id: stable_strategy_id(&element.element_id),
                element_id: element.element_id.clone(),
                disposition: AssetDisposition::ExistingAsset,
                existing_asset_id: Some(asset.asset_id.clone()),
                programmatic: None,
                crop: None,
                specification: None,
                generation: None,
                placeholder_reason: None,
                approval_status: RecordedApprovalStatus::PendingHumanReview,
            })
        }
        AssetDecision::Programmatic { representation } => {
            if !programmatic_compatible(element.kind, *representation) {
                return Err(TaskFailure::invalid(format!(
                    "programmatic representation `{}` is incompatible with `{}` element `{}`",
                    programmatic_name(*representation),
                    visual_kind_name(element.kind),
                    element.element_id
                )));
            }
            if default_programmatic.is_none() {
                diagnostics.push(AssetStrategyDiagnostic {
                    code: "ASSET_PROGRAMMATIC_EXPLICIT".to_owned(),
                    element_id: element.element_id.clone(),
                    message: "asset-bearing observation is intentionally replaced by a programmatic representation".to_owned(),
                });
            }
            Ok(programmatic_entry(element, *representation))
        }
        AssetDecision::AuthorizedCrop { specification } => {
            if !is_asset_bearing(element.kind) {
                return Err(TaskFailure::invalid(
                    "only asset-bearing observations may request a crop",
                ));
            }
            specification.validate()?;
            validate_specification_usage(element, specification)?;
            let source = sources
                .get(element.bounding_box.reference_id.as_str())
                .ok_or_else(|| {
                    TaskFailure::invalid("authorized crop lacks trusted Stage 3 source evidence")
                })?;
            let analysis_reference = analysis_references
                .get(element.bounding_box.reference_id.as_str())
                .ok_or_else(|| TaskFailure::invalid("crop reference is absent from analysis"))?;
            let crop = build_crop_record(&element.bounding_box, analysis_reference, source)?;
            Ok(AssetStrategyEntry {
                strategy_id: stable_strategy_id(&element.element_id),
                element_id: element.element_id.clone(),
                disposition: AssetDisposition::AuthorizedCrop,
                existing_asset_id: None,
                programmatic: None,
                crop: Some(crop),
                specification: Some(specification.clone()),
                generation: None,
                placeholder_reason: None,
                approval_status: RecordedApprovalStatus::PendingHumanReview,
            })
        }
        AssetDecision::Recreate { specification } => {
            if !is_asset_bearing(element.kind) {
                return Err(TaskFailure::invalid(
                    "only asset-bearing observations may request recreation",
                ));
            }
            specification.validate()?;
            validate_specification_usage(element, specification)?;
            Ok(specification_entry(
                element,
                AssetDisposition::Recreate,
                specification,
                None,
            ))
        }
        AssetDecision::Generate {
            specification,
            provenance,
        } => {
            if !is_asset_bearing(element.kind) {
                return Err(TaskFailure::invalid(
                    "only asset-bearing observations may request generation",
                ));
            }
            specification.validate()?;
            validate_specification_usage(element, specification)?;
            provenance.validate_for_draft()?;
            Ok(specification_entry(
                element,
                AssetDisposition::Generate,
                specification,
                Some(provenance.clone()),
            ))
        }
        AssetDecision::Placeholder { reason } => {
            diagnostics.push(AssetStrategyDiagnostic {
                code: "ASSET_PLACEHOLDER_EXPLICIT".to_owned(),
                element_id: element.element_id.clone(),
                message: "explicit placeholder fallback requires human resolution before promotion"
                    .to_owned(),
            });
            Ok(placeholder_entry(element, *reason))
        }
    }
}

fn programmatic_default(kind: VisualElementKind) -> Option<ProgrammaticRepresentation> {
    match kind {
        VisualElementKind::Container => Some(ProgrammaticRepresentation::LayoutContainer),
        VisualElementKind::Text => Some(ProgrammaticRepresentation::Text),
        VisualElementKind::Surface | VisualElementKind::Background => {
            Some(ProgrammaticRepresentation::SolidSurface)
        }
        VisualElementKind::Border => Some(ProgrammaticRepresentation::Border),
        VisualElementKind::StatusIndicator => Some(ProgrammaticRepresentation::StatusPrimitive),
        VisualElementKind::Decoration => Some(ProgrammaticRepresentation::ShadowOrDecoration),
        VisualElementKind::Image
        | VisualElementKind::Icon
        | VisualElementKind::NineSliceCandidate => None,
    }
}

fn programmatic_compatible(
    kind: VisualElementKind,
    representation: ProgrammaticRepresentation,
) -> bool {
    matches!(
        (kind, representation),
        (
            VisualElementKind::Container,
            ProgrammaticRepresentation::LayoutContainer
        ) | (VisualElementKind::Text, ProgrammaticRepresentation::Text)
            | (
                VisualElementKind::Surface | VisualElementKind::Background,
                ProgrammaticRepresentation::SolidSurface
            )
            | (
                VisualElementKind::Border,
                ProgrammaticRepresentation::Border
            )
            | (
                VisualElementKind::StatusIndicator,
                ProgrammaticRepresentation::StatusPrimitive
            )
            | (
                VisualElementKind::Decoration,
                ProgrammaticRepresentation::ShadowOrDecoration
            )
            | (
                VisualElementKind::NineSliceCandidate,
                ProgrammaticRepresentation::SolidSurface
            )
    )
}

fn existing_asset_compatible(kind: VisualElementKind, asset: &CatalogAsset) -> bool {
    let has = |tag: &str| asset.tags.iter().any(|candidate| candidate == tag);
    match kind {
        VisualElementKind::Icon => has("icon"),
        VisualElementKind::Background => has("background"),
        VisualElementKind::Image => {
            (has("image") || has("atlas")) && !has("background") && !has("icon")
        }
        VisualElementKind::Decoration => has("decoration"),
        VisualElementKind::NineSliceCandidate => has("nine_slice"),
        VisualElementKind::Container
        | VisualElementKind::Text
        | VisualElementKind::Surface
        | VisualElementKind::Border
        | VisualElementKind::StatusIndicator => false,
    }
}

fn validate_specification_usage(
    element: &AnalysisElement,
    specification: &AssetSpecification,
) -> Result<(), TaskFailure> {
    let compatible = matches!(
        (element.kind, specification.usage),
        (VisualElementKind::Background, AssetUsage::Background)
            | (VisualElementKind::Image, AssetUsage::ContentImage)
            | (VisualElementKind::Icon, AssetUsage::Icon)
            | (VisualElementKind::Decoration, AssetUsage::Decoration)
            | (VisualElementKind::NineSliceCandidate, AssetUsage::NineSlice)
    );
    if compatible {
        Ok(())
    } else {
        Err(TaskFailure::invalid(format!(
            "asset usage `{}` is incompatible with `{}` element `{}`",
            asset_usage_name(specification.usage),
            visual_kind_name(element.kind),
            element.element_id
        )))
    }
}

fn visual_kind_name(kind: VisualElementKind) -> &'static str {
    match kind {
        VisualElementKind::Container => "container",
        VisualElementKind::Text => "text",
        VisualElementKind::Image => "image",
        VisualElementKind::Background => "background",
        VisualElementKind::Surface => "surface",
        VisualElementKind::Border => "border",
        VisualElementKind::Icon => "icon",
        VisualElementKind::StatusIndicator => "status_indicator",
        VisualElementKind::NineSliceCandidate => "nine_slice_candidate",
        VisualElementKind::Decoration => "decoration",
    }
}

fn programmatic_name(representation: ProgrammaticRepresentation) -> &'static str {
    match representation {
        ProgrammaticRepresentation::LayoutContainer => "layout_container",
        ProgrammaticRepresentation::Text => "text",
        ProgrammaticRepresentation::SolidSurface => "solid_surface",
        ProgrammaticRepresentation::Border => "border",
        ProgrammaticRepresentation::ShadowOrDecoration => "shadow_or_decoration",
        ProgrammaticRepresentation::StatusPrimitive => "status_primitive",
    }
}

fn asset_usage_name(usage: AssetUsage) -> &'static str {
    match usage {
        AssetUsage::Background => "background",
        AssetUsage::ContentImage => "content_image",
        AssetUsage::Icon => "icon",
        AssetUsage::Decoration => "decoration",
        AssetUsage::NineSlice => "nine_slice",
    }
}

fn is_asset_bearing(kind: VisualElementKind) -> bool {
    matches!(
        kind,
        VisualElementKind::Image
            | VisualElementKind::Icon
            | VisualElementKind::NineSliceCandidate
            | VisualElementKind::Background
            | VisualElementKind::Decoration
    )
}

fn programmatic_entry(
    element: &AnalysisElement,
    representation: ProgrammaticRepresentation,
) -> AssetStrategyEntry {
    AssetStrategyEntry {
        strategy_id: stable_strategy_id(&element.element_id),
        element_id: element.element_id.clone(),
        disposition: AssetDisposition::Programmatic,
        existing_asset_id: None,
        programmatic: Some(representation),
        crop: None,
        specification: None,
        generation: None,
        placeholder_reason: None,
        approval_status: RecordedApprovalStatus::PendingHumanReview,
    }
}

fn placeholder_entry(element: &AnalysisElement, reason: PlaceholderReason) -> AssetStrategyEntry {
    AssetStrategyEntry {
        strategy_id: stable_strategy_id(&element.element_id),
        element_id: element.element_id.clone(),
        disposition: AssetDisposition::Placeholder,
        existing_asset_id: None,
        programmatic: None,
        crop: None,
        specification: None,
        generation: None,
        placeholder_reason: Some(reason),
        approval_status: RecordedApprovalStatus::PendingHumanReview,
    }
}

fn specification_entry(
    element: &AnalysisElement,
    disposition: AssetDisposition,
    specification: &AssetSpecification,
    generation: Option<GenerationProvenance>,
) -> AssetStrategyEntry {
    AssetStrategyEntry {
        strategy_id: stable_strategy_id(&element.element_id),
        element_id: element.element_id.clone(),
        disposition,
        existing_asset_id: None,
        programmatic: None,
        crop: None,
        specification: Some(specification.clone()),
        generation,
        placeholder_reason: None,
        approval_status: RecordedApprovalStatus::PendingHumanReview,
    }
}

fn build_crop_record(
    bounding_box: &AnalysisBoundingBox,
    analysis_reference: &crate::analysis::AnalysisReference,
    source: &TrustedAssetSource,
) -> Result<CropSourceRecord, TaskFailure> {
    if source.authorization != ImageAuthorization::DerivativesAllowed {
        return Err(TaskFailure::invalid(format!(
            "reference `{}` does not explicitly allow derivatives; crop authorization is fail-closed",
            source.reference_id()
        )));
    }
    let license_reference = source
        .license_reference
        .as_deref()
        .filter(|value| is_bounded_record(value, 512))
        .ok_or_else(|| {
            TaskFailure::invalid(
                "derivative authorization requires a recorded license or permission reference",
            )
        })?;
    validate_analysis_source(analysis_reference, source)?;
    if bounding_box.coordinate_space != AnalysisCoordinateSpace::StandardPreviewPixel {
        return Err(TaskFailure::invalid(
            "crop bounding box must use Stage 4 standard preview coordinates",
        ));
    }
    let preview_crop = rasterize_bounding_box(
        bounding_box,
        source.manifest.coordinate_mapping.preview_size.width,
        source.manifest.coordinate_mapping.preview_size.height,
    )?;
    let top_left = source.manifest.coordinate_mapping.map_point(
        FloatPoint {
            x: f64::from(preview_crop.x),
            y: f64::from(preview_crop.y),
        },
        CoordinateSpace::PreviewPixel,
        CoordinateSpace::ExifNormalizedPixel,
    )?;
    let bottom_right = source.manifest.coordinate_mapping.map_point(
        FloatPoint {
            x: f64::from(preview_crop.x + preview_crop.width),
            y: f64::from(preview_crop.y + preview_crop.height),
        },
        CoordinateSpace::PreviewPixel,
        CoordinateSpace::ExifNormalizedPixel,
    )?;
    let preview = authoritative_preview(&source.manifest)?;
    if analysis_reference.reference_id != source.manifest.reference_id {
        return Err(TaskFailure::invalid(
            "analysis reference ID differs from trusted crop source",
        ));
    }
    Ok(CropSourceRecord {
        run_id: source.run_id.clone(),
        reference_id: source.manifest.reference_id.clone(),
        source_sha256: source.manifest.source_sha256.clone(),
        source_byte_length: source.manifest.source_byte_length,
        preprocess_cache_key: source.manifest.cache_key.clone(),
        preprocess_manifest_sha256: source.manifest_sha256.clone(),
        standard_preview_sha256: preview.sha256.clone(),
        standard_preview_byte_length: preview.byte_length,
        standard_preview_width: preview.width,
        standard_preview_height: preview.height,
        standard_preview_file_name: preview.file_name.clone(),
        preview_crop,
        exif_normalized_crop: FloatRect {
            x: top_left.x,
            y: top_left.y,
            width: bottom_right.x - top_left.x,
            height: bottom_right.y - top_left.y,
        },
        coordinate_convention: source
            .manifest
            .coordinate_mapping
            .coordinate_convention
            .clone(),
        authorization: source.authorization,
        license_reference: license_reference.to_owned(),
        declared_color_space: source
            .manifest
            .embedded_metadata
            .declared_color_space
            .clone(),
        processing_steps: vec![
            "verify task authorization and source SHA-256".to_owned(),
            "verify Stage 3 manifest/cache/standard-preview identity".to_owned(),
            "round Stage 4 preview pixel edges outward (floor left/top, ceil right/bottom)"
                .to_owned(),
            "crop authoritative standard-preview PNG without resampling".to_owned(),
            "encode deterministic RGBA8 PNG into controlled run assets staging".to_owned(),
        ],
    })
}

fn validate_analysis_source(
    analysis: &crate::analysis::AnalysisReference,
    source: &TrustedAssetSource,
) -> Result<(), TaskFailure> {
    let preview = authoritative_preview(&source.manifest)?;
    if analysis.reference_id != source.manifest.reference_id
        || analysis.source_sha256 != source.manifest.source_sha256
        || analysis.preprocess_cache_key != source.manifest.cache_key
        || analysis.preprocess_protocol_version != source.manifest.protocol_version
        || analysis.preprocess_implementation_version != source.manifest.implementation_version
        || analysis.preprocess_manifest_sha256 != source.manifest_sha256
        || analysis.standard_preview_sha256 != preview.sha256
        || analysis.coordinate_space != AnalysisCoordinateSpace::StandardPreviewPixel
        || analysis.coordinate_convention
            != source.manifest.coordinate_mapping.coordinate_convention
        || analysis.width != preview.width
        || analysis.height != preview.height
    {
        return Err(TaskFailure::invalid(
            "analysis crop evidence does not match trusted Stage 3 manifest identity",
        ));
    }
    Ok(())
}

fn rasterize_bounding_box(
    bounding_box: &AnalysisBoundingBox,
    width: u32,
    height: u32,
) -> Result<PixelRect, TaskFailure> {
    let values = [
        bounding_box.x,
        bounding_box.y,
        bounding_box.width,
        bounding_box.height,
    ];
    if values.iter().any(|value| !value.is_finite())
        || bounding_box.width <= 0.0
        || bounding_box.height <= 0.0
    {
        return Err(TaskFailure::invalid(
            "crop bounding box must be finite and non-empty",
        ));
    }
    let left = bounding_box.x.floor();
    let top = bounding_box.y.floor();
    let right = (bounding_box.x + bounding_box.width).ceil();
    let bottom = (bounding_box.y + bounding_box.height).ceil();
    if left < 0.0
        || top < 0.0
        || right > f64::from(width)
        || bottom > f64::from(height)
        || right <= left
        || bottom <= top
    {
        return Err(TaskFailure::invalid(
            "crop bounding box escapes the trusted standard preview",
        ));
    }
    Ok(PixelRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

fn authoritative_preview(
    manifest: &ReferencePreprocessManifest,
) -> Result<&crate::preprocess::PreprocessArtifact, TaskFailure> {
    let mut matches = manifest.artifacts.iter().filter(|artifact| {
        artifact.kind == ArtifactKind::StandardPreview && !artifact.auxiliary_only
    });
    let preview = matches
        .next()
        .ok_or_else(|| TaskFailure::invalid("preprocess manifest lacks authoritative preview"))?;
    if matches.next().is_some()
        || manifest.artifacts.first() != Some(preview)
        || manifest
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == ArtifactKind::StandardPreview && artifact != preview)
    {
        return Err(TaskFailure::invalid(
            "preprocess manifest must contain exactly one first authoritative preview",
        ));
    }
    Ok(preview)
}

fn find_task_reference<'a>(
    task: &'a GenerationTask,
    reference_id: &str,
) -> Option<&'a ReferenceImage> {
    if task.primary_reference.reference_id == reference_id {
        Some(&task.primary_reference)
    } else {
        task.additional_references
            .iter()
            .find(|reference| reference.image.reference_id == reference_id)
            .map(|reference| &reference.image)
    }
}

fn stable_strategy_id(element_id: &str) -> String {
    format!("asset.{element_id}")
}

fn is_safe_tool_label(value: &str) -> bool {
    is_bounded_record(value, 128)
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'/')
        })
}

fn is_bounded_record(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && value.chars().all(|character| !character.is_control())
}

pub fn draft_asset_file_name(draft_asset_id: &str) -> Result<String, TaskFailure> {
    if !is_safe_asset_id(draft_asset_id) {
        return Err(TaskFailure::invalid(
            "draft asset ID must be a stable dotted lowercase ASCII ID",
        ));
    }
    Ok(format!(
        "id-{}.png",
        encode_base32_no_padding(draft_asset_id.as_bytes())
    ))
}

pub fn draft_asset_id_from_file_name(file_name: &str) -> Result<String, TaskFailure> {
    if Path::new(file_name).components().count() != 1
        || !file_name.starts_with("id-")
        || !file_name.ends_with(".png")
    {
        return Err(TaskFailure::invalid(
            "draft asset file name does not use the version 1 base32 mapping",
        ));
    }
    let encoded = &file_name[3..file_name.len() - 4];
    let bytes = decode_base32_no_padding(encoded)?;
    let draft_asset_id = String::from_utf8(bytes)
        .map_err(|_| TaskFailure::invalid("decoded draft asset ID is not UTF-8"))?;
    if !is_safe_asset_id(&draft_asset_id)
        || draft_asset_file_name(&draft_asset_id)?.as_str() != file_name
    {
        return Err(TaskFailure::invalid(
            "draft asset file name is not a canonical stable ID mapping",
        ));
    }
    Ok(draft_asset_id)
}

fn encode_base32_no_padding(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut output = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer = 0_u32;
    let mut bits = 0_u32;
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
        }
        if bits == 0 {
            buffer = 0;
        } else {
            buffer &= (1_u32 << bits) - 1;
        }
    }
    if bits > 0 {
        output.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    output
}

fn decode_base32_no_padding(value: &str) -> Result<Vec<u8>, TaskFailure> {
    if value.is_empty() {
        return Err(TaskFailure::invalid("base32 draft asset ID is empty"));
    }
    let mut output = Vec::with_capacity(value.len() * 5 / 8);
    let mut buffer = 0_u32;
    let mut bits = 0_u32;
    for byte in value.bytes() {
        let decoded = match byte {
            b'a'..=b'z' => byte - b'a',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => {
                return Err(TaskFailure::invalid(
                    "draft asset file name contains invalid base32 data",
                ));
            }
        };
        buffer = (buffer << 5) | u32::from(decoded);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
        if bits == 0 {
            buffer = 0;
        } else {
            buffer &= (1_u32 << bits) - 1;
        }
    }
    if buffer != 0 {
        return Err(TaskFailure::invalid(
            "draft asset file name contains non-zero base32 padding",
        ));
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetQualitySeverity {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetQualityVerdict {
    Pass,
    ReviewRequired,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetQualityFinding {
    pub code: String,
    pub severity: AssetQualitySeverity,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetQualityReport {
    pub verdict: AssetQualityVerdict,
    pub format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub color_type: Option<String>,
    pub byte_length: u64,
    pub findings: Vec<AssetQualityFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DraftCropResult {
    pub draft_asset_id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub quality: AssetQualityReport,
}

/// Extracts a crop only from the exact Stage 3 standard preview inside an existing run.
/// It cannot create or modify anything below `project/assets`.
pub fn extract_authorized_crop(
    repository_root: &Path,
    run_id: &str,
    entry: &AssetStrategyEntry,
    draft_asset_id: &str,
) -> Result<DraftCropResult, TaskFailure> {
    RunId::parse(run_id)?;
    if !is_safe_asset_id(draft_asset_id) {
        return Err(TaskFailure::invalid(
            "draft asset ID must be a stable dotted lowercase ASCII ID",
        ));
    }
    if entry.disposition != AssetDisposition::AuthorizedCrop {
        return Err(TaskFailure::invalid(
            "only an authorized crop strategy entry may extract pixels",
        ));
    }
    let crop = entry
        .crop
        .as_ref()
        .ok_or_else(|| TaskFailure::invalid("authorized crop entry lacks crop provenance"))?;
    if crop.run_id != run_id {
        return Err(TaskFailure::invalid(
            "authorized crop capability is bound to a different run ID",
        ));
    }
    let specification = entry
        .specification
        .as_ref()
        .ok_or_else(|| TaskFailure::invalid("authorized crop entry lacks output specification"))?;
    specification.validate()?;
    if crop.authorization != ImageAuthorization::DerivativesAllowed
        || !is_bounded_record(&crop.license_reference, 512)
    {
        return Err(TaskFailure::invalid(
            "serialized crop entry no longer contains explicit derivative authorization",
        ));
    }

    let repository_root = canonical_regular_directory(repository_root, "repository root")?;
    let project_assets = repository_root.join("project/assets");
    let project_assets_before = snapshot_regular_files(&project_assets)?;
    let generation_root = canonical_regular_directory(
        &repository_root.join("summary/ui-generation"),
        "generation root",
    )?;
    let run_root = generation_root.join(run_id);
    reject_symlink_chain(&generation_root, &run_root)?;
    let run_root = canonical_regular_directory(&run_root, "run root")?;
    if !run_root.starts_with(&generation_root) {
        return Err(unsafe_path(&run_root, "run root escapes generation root"));
    }
    let assets = run_root.join("assets");
    reject_symlink_chain(&run_root, &assets)?;
    fs::create_dir_all(&assets)
        .map_err(|error| output_failure(&assets, "create run asset directory", error))?;
    let assets = canonical_regular_directory(&assets, "run asset directory")?;
    if !assets.starts_with(&run_root) {
        return Err(unsafe_path(&assets, "run asset directory escapes run root"));
    }

    let preview_artifact_name = Path::new(&crop.standard_preview_file_name);
    if preview_artifact_name.components().count() != 1
        || !crop.standard_preview_file_name.ends_with(".png")
    {
        return Err(TaskFailure::invalid(
            "crop provenance contains an unsafe standard preview file name",
        ));
    }
    let preview = run_root
        .join("input/preprocessed")
        .join(&crop.reference_id)
        .join(preview_artifact_name);
    reject_symlink_chain(&run_root, &preview)?;
    let expected_preview_root = run_root.join("input/preprocessed").join(&crop.reference_id);
    let preview_metadata = fs::symlink_metadata(&preview).map_err(|error| {
        TaskFailure::invalid(format!("standard preview metadata unavailable: {error}"))
    })?;
    if !preview_metadata.is_file() || preview_metadata.file_type().is_symlink() {
        return Err(unsafe_path(
            &preview,
            "standard preview is not a regular file",
        ));
    }
    let canonical_preview = fs::canonicalize(&preview).map_err(|error| {
        TaskFailure::invalid(format!("standard preview cannot be resolved: {error}"))
    })?;
    let canonical_preview_root = fs::canonicalize(&expected_preview_root).map_err(|error| {
        TaskFailure::invalid(format!(
            "preprocessed reference root cannot be resolved: {error}"
        ))
    })?;
    if !canonical_preview.starts_with(&canonical_preview_root)
        || !canonical_preview_root.starts_with(&run_root)
    {
        return Err(unsafe_path(
            &preview,
            "standard preview escapes controlled preprocessed reference root",
        ));
    }
    let preview_bytes = read_bounded(&preview, MAX_STANDARD_PREVIEW_BYTES)?;
    if preview_bytes.len() as u64 != crop.standard_preview_byte_length
        || sha256_bytes(&preview_bytes) != crop.standard_preview_sha256
    {
        return Err(TaskFailure::new(
            TaskFailureKind::PreprocessCacheCorrupt,
            "standard preview hash or byte length changed after strategy planning",
            Some(preview.display().to_string()),
        ));
    }
    let reader = bounded_image_reader(&preview_bytes, true)
        .map_err(|error| TaskFailure::invalid(format!("preview format is invalid: {error}")))?;
    if reader.format() != Some(ImageFormat::Png) {
        return Err(TaskFailure::invalid(
            "authorized crop source must be Stage 3 PNG",
        ));
    }
    let image = reader
        .decode()
        .map_err(|error| TaskFailure::invalid(format!("preview decode failed: {error}")))?;
    if image.width() != crop.standard_preview_width
        || image.height() != crop.standard_preview_height
    {
        return Err(TaskFailure::new(
            TaskFailureKind::PreprocessCacheCorrupt,
            "standard preview dimensions changed after strategy planning",
            Some(preview.display().to_string()),
        ));
    }
    let preview_crop = crop.preview_crop;
    let right = preview_crop.x.checked_add(preview_crop.width);
    let bottom = preview_crop.y.checked_add(preview_crop.height);
    if right.is_none_or(|right| right > image.width())
        || bottom.is_none_or(|bottom| bottom > image.height())
        || preview_crop.width == 0
        || preview_crop.height == 0
    {
        return Err(TaskFailure::invalid(
            "authorized crop rectangle no longer fits the standard preview",
        ));
    }
    if specification.width != preview_crop.width || specification.height != preview_crop.height {
        return Err(TaskFailure::invalid(
            "crop output specification must equal the trusted crop size; implicit resampling is forbidden",
        ));
    }
    let cropped = image
        .crop_imm(
            preview_crop.x,
            preview_crop.y,
            preview_crop.width,
            preview_crop.height,
        )
        .to_rgba8();

    let file_name = draft_asset_file_name(draft_asset_id)?;
    let staging_stem = file_name.trim_end_matches(".png");
    let destination = assets.join(&file_name);
    ensure_direct_child(&assets, &destination)?;
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(TaskFailure::new(
            TaskFailureKind::OutputDirectoryConflict,
            "draft asset output already exists",
            Some(destination.display().to_string()),
        ));
    }
    let staging = assets.join(format!(
        ".{staging_stem}.staging-{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    ensure_direct_child(&assets, &staging)?;
    let result = (|| {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|error| output_failure(&staging, "create draft crop staging file", error))?;
        let mut writer = BufWriter::new(file);
        image::codecs::png::PngEncoder::new(&mut writer)
            .write_image(
                cropped.as_raw(),
                cropped.width(),
                cropped.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|error| {
                TaskFailure::invalid(format!("encode deterministic draft crop failed: {error}"))
            })?;
        writer
            .flush()
            .map_err(|error| output_failure(&staging, "flush draft crop", error))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|error| output_failure(&staging, "sync draft crop", error))?;
        drop(writer);
        let output_bytes = read_bounded(&staging, MAX_DRAFT_ASSET_BYTES)?;
        let quality = inspect_asset_bytes(&output_bytes, specification);
        if quality.verdict == AssetQualityVerdict::Reject {
            return Err(TaskFailure::invalid(format!(
                "draft crop failed asset quality checks: {:?}",
                quality
                    .findings
                    .iter()
                    .map(|finding| finding.code.as_str())
                    .collect::<Vec<_>>()
            )));
        }
        commit_staged_file_no_clobber(&staging, &destination, &assets)?;
        Ok(DraftCropResult {
            draft_asset_id: draft_asset_id.to_owned(),
            path: destination.clone(),
            sha256: sha256_bytes(&output_bytes),
            byte_length: output_bytes.len() as u64,
            width: cropped.width(),
            height: cropped.height(),
            quality,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    let project_assets_after = snapshot_regular_files(&project_assets)?;
    if project_assets_before != project_assets_after {
        if result.is_ok() {
            let _ = fs::remove_file(&destination);
        }
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "draft extraction observed a concurrent or accidental project/assets modification",
            Some(project_assets.display().to_string()),
        ));
    }
    result
}

pub fn inspect_asset_file(
    path: &Path,
    specification: &AssetSpecification,
) -> Result<AssetQualityReport, TaskFailure> {
    specification.validate()?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        TaskFailure::invalid(format!("draft asset metadata unavailable: {error}"))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(unsafe_path(
            path,
            "draft asset must be a regular non-symlink file",
        ));
    }
    let bytes = read_bounded(path, MAX_DRAFT_ASSET_BYTES.saturating_add(1))?;
    Ok(inspect_asset_bytes(&bytes, specification))
}

pub fn inspect_asset_bytes(bytes: &[u8], specification: &AssetSpecification) -> AssetQualityReport {
    let mut findings = Vec::new();
    if let Err(failure) = specification.validate() {
        quality_finding(
            &mut findings,
            "ASSET_SPECIFICATION_INVALID",
            AssetQualitySeverity::Error,
            failure.message().to_owned(),
        );
    }
    if bytes.is_empty() || bytes.len() as u64 > MAX_DRAFT_ASSET_BYTES {
        quality_finding(
            &mut findings,
            "ASSET_ENCODED_SIZE_UNSAFE",
            AssetQualitySeverity::Error,
            format!("encoded asset must be 1-{MAX_DRAFT_ASSET_BYTES} bytes"),
        );
        return finish_quality_report("unknown", None, bytes.len() as u64, findings);
    }
    let reader = bounded_image_reader(bytes, false);
    let Ok(reader) = reader else {
        quality_finding(
            &mut findings,
            "ASSET_FORMAT_UNSUPPORTED",
            AssetQualitySeverity::Error,
            "asset format could not be identified".to_owned(),
        );
        return finish_quality_report("unknown", None, bytes.len() as u64, findings);
    };
    let format = reader.format();
    let format_name = match format {
        Some(ImageFormat::Png) => "png",
        Some(ImageFormat::Jpeg) => "jpeg",
        _ => "unsupported",
    };
    if format_name == "unsupported" {
        quality_finding(
            &mut findings,
            "ASSET_FORMAT_UNSUPPORTED",
            AssetQualitySeverity::Error,
            "Android UI drafts support only PNG and JPEG".to_owned(),
        );
    }
    let dimensions = reader.into_dimensions();
    let Ok((header_width, header_height)) = dimensions else {
        quality_finding(
            &mut findings,
            "ASSET_HEADER_DECODE_FAILED",
            AssetQualitySeverity::Error,
            "asset dimensions cannot be read within the header allocation budget".to_owned(),
        );
        return finish_quality_report(format_name, None, bytes.len() as u64, findings);
    };
    let header_pixels = u64::from(header_width) * u64::from(header_height);
    if header_width == 0
        || header_height == 0
        || header_width > MAX_ASSET_SPEC_EDGE
        || header_height > MAX_ASSET_SPEC_EDGE
        || header_pixels > MAX_ASSET_PIXELS
    {
        quality_finding(
            &mut findings,
            "ASSET_ANDROID_TEXTURE_LIMIT",
            AssetQualitySeverity::Error,
            format!(
                "texture {header_width}x{header_height} exceeds the {MAX_ASSET_SPEC_EDGE}px/{MAX_ASSET_PIXELS}-pixel Android budget"
            ),
        );
        return finish_quality_report(
            format_name,
            Some((header_width, header_height, "header_only".to_owned())),
            bytes.len() as u64,
            findings,
        );
    }
    let reader = match bounded_image_reader(bytes, true) {
        Ok(reader) => reader,
        Err(_) => {
            quality_finding(
                &mut findings,
                "ASSET_FORMAT_UNSUPPORTED",
                AssetQualitySeverity::Error,
                "asset format could not be identified".to_owned(),
            );
            return finish_quality_report(format_name, None, bytes.len() as u64, findings);
        }
    };
    let decoded = reader.decode();
    let Ok(image) = decoded else {
        quality_finding(
            &mut findings,
            "ASSET_DECODE_FAILED",
            AssetQualitySeverity::Error,
            "asset cannot be decoded by the project image stack".to_owned(),
        );
        return finish_quality_report(format_name, None, bytes.len() as u64, findings);
    };
    let width = image.width();
    let height = image.height();
    let color = image.color();
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_ASSET_SPEC_EDGE
        || height > MAX_ASSET_SPEC_EDGE
        || pixels > MAX_ASSET_PIXELS
    {
        quality_finding(
            &mut findings,
            "ASSET_ANDROID_TEXTURE_LIMIT",
            AssetQualitySeverity::Error,
            format!(
                "texture {width}x{height} exceeds the {MAX_ASSET_SPEC_EDGE}px/{MAX_ASSET_PIXELS}-pixel Android budget"
            ),
        );
    }
    if width != specification.width || height != specification.height {
        quality_finding(
            &mut findings,
            "ASSET_DIMENSION_MISMATCH",
            AssetQualitySeverity::Error,
            format!(
                "decoded dimensions {width}x{height} differ from specification {}x{}",
                specification.width, specification.height
            ),
        );
    }
    if !matches!(
        color,
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8
    ) {
        quality_finding(
            &mut findings,
            "ASSET_ANDROID_COLOR_TYPE_UNSUPPORTED",
            AssetQualitySeverity::Error,
            format!("decoded color type {color:?} is not an approved 8-bit Android texture format"),
        );
    }
    if specification.alpha == AlphaMode::Straight && !color.has_alpha() {
        quality_finding(
            &mut findings,
            "ASSET_ALPHA_REQUIRED",
            AssetQualitySeverity::Error,
            "asset specification requires a straight alpha channel".to_owned(),
        );
    }
    if specification.alpha == AlphaMode::Opaque && color.has_alpha() {
        let rgba = image.to_rgba8();
        if rgba.pixels().any(|pixel| pixel.0[3] != 255) {
            quality_finding(
                &mut findings,
                "ASSET_OPAQUE_ALPHA_PRESENT",
                AssetQualitySeverity::Error,
                "opaque specification contains non-opaque pixels".to_owned(),
            );
        }
    }
    if color.has_alpha() {
        let rgba = image.to_rgba8();
        if transparent_edge_is_occupied(&rgba) {
            quality_finding(
                &mut findings,
                "ASSET_TRANSPARENT_EDGE_OCCUPIED",
                AssetQualitySeverity::Warning,
                "non-transparent pixels touch the texture edge; review filtering and atlas bleed"
                    .to_owned(),
            );
        }
        if rgba
            .pixels()
            .any(|pixel| pixel.0[3] == 0 && pixel.0[..3] != [0, 0, 0])
        {
            quality_finding(
                &mut findings,
                "ASSET_TRANSPARENT_RGB_BLEED",
                AssetQualitySeverity::Warning,
                "fully transparent pixels contain RGB data; review edge halos under filtering"
                    .to_owned(),
            );
        }
    }
    match format {
        Some(ImageFormat::Jpeg) => {
            quality_finding(
                &mut findings,
                "ASSET_LOSSY_COMPRESSION_REVIEW",
                AssetQualitySeverity::Warning,
                "JPEG is lossy; compression artifacts around text, icons, and hard edges require review"
                    .to_owned(),
            );
            quality_finding(
                &mut findings,
                "ASSET_COLOR_SPACE_UNVERIFIED",
                AssetQualitySeverity::Warning,
                "JPEG ICC/color conversion is not verified by the current offline checker"
                    .to_owned(),
            );
        }
        Some(ImageFormat::Png) => {
            let chunks = inspect_png_chunks(bytes);
            if chunks.animated {
                quality_finding(
                    &mut findings,
                    "ASSET_ANDROID_APNG_UNSUPPORTED",
                    AssetQualitySeverity::Error,
                    "animated PNG is not an approved static Android UI texture".to_owned(),
                );
            }
            if chunks.icc {
                quality_finding(
                    &mut findings,
                    "ASSET_ICC_CONVERSION_UNVERIFIED",
                    AssetQualitySeverity::Warning,
                    "embedded ICC profile is preserved but conversion to sRGB is not verified"
                        .to_owned(),
                );
            } else if !chunks.srgb {
                quality_finding(
                    &mut findings,
                    "ASSET_COLOR_SPACE_ASSUMED_SRGB",
                    AssetQualitySeverity::Warning,
                    "PNG has no explicit sRGB/ICC declaration; encoded samples are treated as sRGB pending review"
                        .to_owned(),
                );
            }
        }
        _ => {}
    }
    finish_quality_report(
        format_name,
        Some((width, height, format!("{color:?}"))),
        bytes.len() as u64,
        findings,
    )
}

fn bounded_image_reader<'a>(
    bytes: &'a [u8],
    enforce_texture_dimensions: bool,
) -> std::io::Result<ImageReader<Cursor<&'a [u8]>>> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(if enforce_texture_dimensions {
        MAX_ASSET_DECODE_ALLOC
    } else {
        MAX_IMAGE_HEADER_ALLOC
    });
    if enforce_texture_dimensions {
        limits.max_image_width = Some(MAX_ASSET_SPEC_EDGE);
        limits.max_image_height = Some(MAX_ASSET_SPEC_EDGE);
    }
    reader.limits(limits);
    Ok(reader)
}

#[derive(Default)]
struct PngChunks {
    srgb: bool,
    icc: bool,
    animated: bool,
}

fn inspect_png_chunks(bytes: &[u8]) -> PngChunks {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) {
        return PngChunks::default();
    }
    let mut result = PngChunks::default();
    let mut offset = SIGNATURE.len();
    while offset.checked_add(12).is_some_and(|end| end <= bytes.len()) {
        let length = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let Some(end) = offset
            .checked_add(12)
            .and_then(|base| base.checked_add(length))
        else {
            break;
        };
        if end > bytes.len() {
            break;
        }
        let kind = &bytes[offset + 4..offset + 8];
        result.srgb |= kind == b"sRGB";
        result.icc |= kind == b"iCCP";
        result.animated |= kind == b"acTL";
        offset = end;
        if kind == b"IEND" {
            break;
        }
    }
    result
}

fn transparent_edge_is_occupied(image: &image::RgbaImage) -> bool {
    if image.width() == 0 || image.height() == 0 {
        return false;
    }
    let last_x = image.width() - 1;
    let last_y = image.height() - 1;
    (0..image.width()).any(|x| image.get_pixel(x, 0).0[3] != 0)
        || (0..image.width()).any(|x| image.get_pixel(x, last_y).0[3] != 0)
        || (0..image.height()).any(|y| image.get_pixel(0, y).0[3] != 0)
        || (0..image.height()).any(|y| image.get_pixel(last_x, y).0[3] != 0)
}

fn quality_finding(
    findings: &mut Vec<AssetQualityFinding>,
    code: &str,
    severity: AssetQualitySeverity,
    message: String,
) {
    findings.push(AssetQualityFinding {
        code: code.to_owned(),
        severity,
        message,
    });
}

fn finish_quality_report(
    format: &str,
    decoded: Option<(u32, u32, String)>,
    byte_length: u64,
    mut findings: Vec<AssetQualityFinding>,
) -> AssetQualityReport {
    findings.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then_with(|| left.code.cmp(&right.code))
    });
    let verdict = if findings
        .iter()
        .any(|finding| finding.severity == AssetQualitySeverity::Error)
    {
        AssetQualityVerdict::Reject
    } else if findings.is_empty() {
        AssetQualityVerdict::Pass
    } else {
        AssetQualityVerdict::ReviewRequired
    };
    let (width, height, color_type) = decoded
        .map(|(width, height, color)| (Some(width), Some(height), Some(color)))
        .unwrap_or((None, None, None));
    AssetQualityReport {
        verdict,
        format: format.to_owned(),
        width,
        height,
        color_type,
        byte_length,
        findings,
    }
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, TaskFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            format!("{label} metadata unavailable: {error}"),
            Some(path.display().to_string()),
        )
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(unsafe_path(
            path,
            format!("{label} must be a regular directory"),
        ));
    }
    fs::canonicalize(path).map_err(|error| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            format!("{label} cannot be resolved: {error}"),
            Some(path.display().to_string()),
        )
    })
}

fn reject_symlink_chain(root: &Path, candidate: &Path) -> Result<(), TaskFailure> {
    if !candidate.starts_with(root) {
        return Err(unsafe_path(candidate, "path escapes controlled root"));
    }
    let relative = candidate
        .strip_prefix(root)
        .map_err(|_| unsafe_path(candidate, "path escapes controlled root"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(unsafe_path(candidate, "path contains unsafe components"));
        };
        current.push(component);
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(unsafe_path(&current, "controlled path contains a symlink"));
        }
    }
    Ok(())
}

fn ensure_direct_child(root: &Path, child: &Path) -> Result<(), TaskFailure> {
    if child.parent() == Some(root) {
        Ok(())
    } else {
        Err(unsafe_path(
            child,
            "output is not a direct child of run assets",
        ))
    }
}

fn commit_staged_file_no_clobber(
    staging: &Path,
    destination: &Path,
    directory: &Path,
) -> Result<(), TaskFailure> {
    ensure_direct_child(directory, staging)?;
    ensure_direct_child(directory, destination)?;
    match fs::hard_link(staging, destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(TaskFailure::new(
                TaskFailureKind::OutputDirectoryConflict,
                "draft asset output already exists and was not overwritten",
                Some(destination.display().to_string()),
            ));
        }
        Err(error) => {
            return Err(output_failure(
                destination,
                "atomically create draft crop without overwrite",
                error,
            ));
        }
    }
    if let Err(error) = fs::remove_file(staging) {
        let _ = fs::remove_file(destination);
        return Err(output_failure(
            staging,
            "remove committed draft crop staging link",
            error,
        ));
    }
    if let Err(failure) = sync_directory(directory) {
        let _ = fs::remove_file(destination);
        return Err(failure);
    }
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> Result<(), TaskFailure> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| output_failure(path, "sync run asset directory", error))
}

#[cfg(windows)]
fn sync_directory(_path: &Path) -> Result<(), TaskFailure> {
    // Windows does not allow opening a directory with std::fs::File. The staged file itself is
    // flushed and synced before the no-clobber hard-link commit.
    Ok(())
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, TaskFailure> {
    let metadata = fs::metadata(path).map_err(|error| {
        TaskFailure::invalid(format!("bounded file metadata unavailable: {error}"))
    })?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(TaskFailure::invalid(format!(
            "file must be regular and no larger than {max_bytes} bytes"
        )));
    }
    let file = File::open(path)
        .map_err(|error| TaskFailure::invalid(format!("bounded file cannot be opened: {error}")))?;
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| TaskFailure::invalid("file length does not fit memory budget"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| TaskFailure::invalid(format!("bounded file read failed: {error}")))?;
    if bytes.len() as u64 > max_bytes || bytes.len() as u64 != metadata.len() {
        return Err(TaskFailure::invalid(
            "file changed or exceeded its size budget during read",
        ));
    }
    Ok(bytes)
}

fn snapshot_regular_files(root: &Path) -> Result<BTreeMap<String, (u64, String)>, TaskFailure> {
    if !root.exists() {
        return Ok(BTreeMap::new());
    }
    let root = canonical_regular_directory(root, "formal asset root")?;
    let mut pending = vec![(root.clone(), 0_usize)];
    let mut snapshot = BTreeMap::new();
    let mut visited_entries = 0_usize;
    let mut total_bytes = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_FORMAL_ASSET_SNAPSHOT_DEPTH {
            return Err(TaskFailure::invalid(format!(
                "formal asset snapshot depth exceeds {MAX_FORMAL_ASSET_SNAPSHOT_DEPTH}"
            )));
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory)
            .map_err(|error| TaskFailure::invalid(format!("asset snapshot failed: {error}")))?
        {
            visited_entries = visited_entries
                .checked_add(1)
                .ok_or_else(|| TaskFailure::invalid("asset snapshot entry count overflow"))?;
            if visited_entries > MAX_FORMAL_ASSET_SNAPSHOT_ENTRIES {
                return Err(TaskFailure::invalid(format!(
                    "formal asset snapshot exceeds {MAX_FORMAL_ASSET_SNAPSHOT_ENTRIES} entries"
                )));
            }
            entries.push(entry.map_err(|error| TaskFailure::invalid(error.to_string()))?);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_type = entry
                .file_type()
                .map_err(|error| TaskFailure::invalid(error.to_string()))?;
            if file_type.is_symlink() {
                return Err(unsafe_path(
                    &entry.path(),
                    "formal asset tree contains a symlink",
                ));
            }
            if file_type.is_dir() {
                pending.push((entry.path(), depth + 1));
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(&root)
                    .map_err(|_| unsafe_path(&entry.path(), "asset snapshot path escaped"))?
                    .to_path_buf();
                let relative = path_to_forward_slashes(&relative)?;
                let (byte_length, hash) = hash_regular_file(&entry.path())?;
                total_bytes = total_bytes
                    .checked_add(byte_length)
                    .ok_or_else(|| TaskFailure::invalid("asset snapshot byte count overflow"))?;
                if total_bytes > MAX_FORMAL_ASSET_SNAPSHOT_BYTES {
                    return Err(TaskFailure::invalid(format!(
                        "formal asset snapshot exceeds {MAX_FORMAL_ASSET_SNAPSHOT_BYTES} bytes"
                    )));
                }
                snapshot.insert(relative, (byte_length, hash));
            }
        }
    }
    Ok(snapshot)
}

fn hash_regular_file(path: &Path) -> Result<(u64, String), TaskFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        TaskFailure::invalid(format!("asset snapshot metadata failed: {error}"))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(unsafe_path(
            path,
            "asset snapshot source is not a regular file",
        ));
    }
    let mut file = File::open(path)
        .map_err(|error| TaskFailure::invalid(format!("asset snapshot open failed: {error}")))?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            TaskFailure::invalid(format!("asset snapshot read failed: {error}"))
        })?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| TaskFailure::invalid("asset snapshot byte count overflow"))?;
        hasher.update(&buffer[..read]);
    }
    if total != metadata.len() {
        return Err(TaskFailure::invalid(
            "formal asset changed while its snapshot was computed",
        ));
    }
    Ok((total, format!("{:x}", hasher.finalize())))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn unsafe_path(path: &Path, message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::UnsafeOutputPath,
        message,
        Some(path.display().to_string()),
    )
}

fn output_failure(path: &Path, action: &str, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::UnsafeOutputPath,
        format!("{action} failed: {error}"),
        Some(path.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::parse_analysis_json,
        contract::{ImageProvenance, PixelSize, TargetViewport},
        planning::plan_analysis,
        preprocess::{
            AppliedOrientation, CoordinateMapping, EmbeddedMetadata, PreprocessArtifact,
            ReferencePreprocessOptions, ReferenceValidationProfile,
        },
    };
    use image::{Rgb, RgbImage, Rgba, RgbaImage, codecs::jpeg::JpegEncoder};
    use serde_json::Value;

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn regular_analysis() -> UiReferenceAnalysis {
        parse_analysis_json(include_bytes!("../fixtures/analysis/regular_page.json")).unwrap()
    }

    fn raster_spec(width: u32, height: u32, usage: AssetUsage) -> AssetSpecification {
        AssetSpecification {
            width,
            height,
            alpha: AlphaMode::Straight,
            slice_insets: None,
            color_space: RequiredColorSpace::Srgb,
            usage,
        }
    }

    fn png_bytes(image: &RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }

    fn crop_fixture(
        authorization: ImageAuthorization,
    ) -> (
        GenerationTask,
        ReferencePreprocessManifest,
        Vec<u8>,
        Vec<u8>,
        UiReferenceAnalysis,
    ) {
        let preview_image =
            RgbaImage::from_fn(8, 8, |x, y| Rgba([(x * 20) as u8, (y * 20) as u8, 80, 255]));
        let preview_bytes = png_bytes(&preview_image);
        let source_sha256 = "a".repeat(64);
        let mut manifest = ReferencePreprocessManifest {
            protocol_version: 1,
            implementation_version: "ui-reference-preprocess-1".to_owned(),
            cache_key: "b".repeat(64),
            reference_id: "primary".to_owned(),
            source_sha256: source_sha256.clone(),
            source_byte_length: 128,
            source_raw_size: PixelSize {
                width: 8,
                height: 8,
            },
            validation_profile: ReferenceValidationProfile::PageReference,
            embedded_metadata: EmbeddedMetadata {
                format: "png".to_owned(),
                decoded_color_type: "Rgba8".to_owned(),
                original_color_type: "Rgba8".to_owned(),
                has_alpha_channel: true,
                exif_present: false,
                exif_byte_length: 0,
                exif_sha256: None,
                embedded_orientation: None,
                declared_orientation: crate::contract::ImageOrientation::Normal,
                applied_orientation: AppliedOrientation::Normal,
                icc_profile_present: false,
                icc_profile_byte_length: 0,
                icc_profile_sha256: None,
                declared_color_space: ImageColorSpace::Srgb,
                preview_sample_encoding: "test RGBA8".to_owned(),
            },
            coordinate_mapping: CoordinateMapping::new(
                PixelSize {
                    width: 8,
                    height: 8,
                },
                AppliedOrientation::Normal,
                PixelRect {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                PixelSize {
                    width: 8,
                    height: 8,
                },
                TargetViewport {
                    logical_width: 8.0,
                    logical_height: 8.0,
                    device_scale: 1.0,
                },
            )
            .unwrap(),
            explicit_safe_area: None,
            explicit_system_ui_exclusions: Vec::new(),
            options: ReferencePreprocessOptions::default(),
            artifacts: vec![PreprocessArtifact {
                kind: ArtifactKind::StandardPreview,
                file_name: "standard-preview.png".to_owned(),
                sha256: sha256_bytes(&preview_bytes),
                byte_length: preview_bytes.len() as u64,
                width: 8,
                height: 8,
                auxiliary_only: false,
            }],
            original_remains_authoritative: true,
        };
        let mut task =
            GenerationTask::parse_json(include_bytes!("../fixtures/task.valid.json")).unwrap();
        task.run_id = "crop-run".to_owned();
        task.target_viewport = Some(TargetViewport {
            logical_width: 8.0,
            logical_height: 8.0,
            device_scale: 1.0,
        });
        task.primary_reference.reference_id = "primary".to_owned();
        task.primary_reference.metadata.original_size = PixelSize {
            width: 8,
            height: 8,
        };
        task.primary_reference.metadata.orientation = crate::contract::ImageOrientation::Normal;
        task.primary_reference.metadata.color_space = ImageColorSpace::Srgb;
        task.primary_reference.metadata.sha256 = source_sha256;
        task.primary_reference.metadata.provenance = ImageProvenance {
            source: "test fixture".to_owned(),
            source_uri: None,
            authorization,
            license_reference: Some("permission-record-42".to_owned()),
        };
        task.additional_references.clear();
        manifest.cache_key = trusted_cache_key(
            &task.primary_reference,
            &manifest,
            task.target_viewport.unwrap(),
            ReferenceValidationProfile::PageReference,
        )
        .unwrap();
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();

        let mut analysis = regular_analysis();
        analysis.run_id = "crop-run".to_owned();
        let reference = &mut analysis.references[0];
        reference.source_sha256 = manifest.source_sha256.clone();
        reference.preprocess_cache_key = manifest.cache_key.clone();
        reference.preprocess_protocol_version = manifest.protocol_version;
        reference.preprocess_implementation_version = manifest.implementation_version.clone();
        reference.preprocess_manifest_sha256 = sha256_bytes(&manifest_bytes);
        reference.standard_preview_sha256 = sha256_bytes(&preview_bytes);
        reference.width = 8;
        reference.height = 8;
        analysis.regions[0].bounding_box.width = 8.0;
        analysis.regions[0].bounding_box.height = 8.0;
        analysis.elements.truncate(1);
        analysis.uncertainties.clear();
        analysis.elements[0].kind = VisualElementKind::Image;
        analysis.elements[0].bounding_box = AnalysisBoundingBox {
            reference_id: "primary".to_owned(),
            coordinate_space: AnalysisCoordinateSpace::StandardPreviewPixel,
            x: 2.0,
            y: 2.0,
            width: 4.0,
            height: 4.0,
            evidence_ids: analysis.elements[0].bounding_box.evidence_ids.clone(),
        };
        (task, manifest, manifest_bytes, preview_bytes, analysis)
    }

    #[test]
    fn repository_catalog_is_complete_and_searches_by_stable_id_metadata() {
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        assert_eq!(catalog.assets.len(), 22);
        assert!(catalog.resolve("ui.icon.close").is_some());
        assert!(catalog.resolve("ui/icons/close.png").is_none());
        let matches = catalog
            .search(&AssetSearchQuery {
                kind: Some(CatalogAssetKind::Raster),
                terms: vec!["icon".to_owned(), "close".to_owned()],
            })
            .unwrap();
        assert_eq!(matches[0].asset_id, "ui.icon.close");
        assert_eq!(matches[0].matched_terms, vec!["close", "icon"]);
    }

    #[test]
    fn generated_catalog_fragment_is_scoped_and_reuses_formal_asset_validation() {
        let repository = tempfile::tempdir().unwrap();
        let packaged_root = repository.path().join("project/assets");
        let ui_root = packaged_root.join("ui");
        let promotion_root = ui_root.join("documents/approved/promotion_fixture");
        fs::create_dir_all(promotion_root.join("assets")).unwrap();
        let image = RgbaImage::from_pixel(1, 1, Rgba([12, 34, 56, 200]));
        let bytes = png_bytes(&image);
        let image_path = promotion_root.join("assets/promotion.png");
        fs::write(&image_path, &bytes).unwrap();
        let mut fragment = serde_json::json!({
            "schema_version": UI_ASSET_CATALOG_SCHEMA_VERSION,
            "assets": [{
                "asset_id": "ui.generated.promotion_fixture.image",
                "path": "ui/documents/approved/promotion_fixture/assets/promotion.png",
                "kind": "raster",
                "sha256": sha256_bytes(&bytes),
                "byte_length": bytes.len(),
                "width": 1,
                "height": 1,
                "alpha": "straight",
                "license": {"status": "redistributable", "reference": "ui/documents/approved/promotion_fixture/LICENSES.md"},
                "tags": ["generated", "promotion_fixture"]
            }]
        });
        fs::write(
            promotion_root.join("catalog.v1.json"),
            serde_json::to_vec_pretty(&fragment).unwrap(),
        )
        .unwrap();
        fs::write(promotion_root.join("LICENSES.md"), b"fixture license\n").unwrap();

        let assets = load_generated_catalog_assets(&ui_root).unwrap();
        assert_eq!(assets.len(), 1);
        validate_catalog_asset(&assets[0]).unwrap();
        validate_catalog_file(
            &packaged_root.canonicalize().unwrap(),
            &ui_root.canonicalize().unwrap(),
            &assets[0],
        )
        .unwrap();

        fragment["assets"][0]["path"] = Value::String("ui/images/escape.png".to_owned());
        fs::write(
            promotion_root.join("catalog.v1.json"),
            serde_json::to_vec_pretty(&fragment).unwrap(),
        )
        .unwrap();
        assert!(load_generated_catalog_assets(&ui_root).is_err());

        let outside = repository.path().join("outside-catalog.json");
        fs::write(&outside, b"{}\n").unwrap();
        let catalog_path = promotion_root.join("catalog.v1.json");
        fs::remove_file(&catalog_path).unwrap();
        if create_file_symlink(&outside, &catalog_path).is_ok() {
            assert!(load_generated_catalog_assets(&ui_root).is_err());
        }
    }

    #[test]
    fn catalog_rejects_escape_duplicate_case_and_stale_metadata() {
        let mut document: Value = serde_json::from_str(CATALOG_JSON).unwrap();
        document["assets"][0]["path"] = Value::String("ui/../escape.png".to_owned());
        assert!(
            AssetCatalog::load_and_validate(
                &repository_root(),
                &serde_json::to_vec(&document).unwrap()
            )
            .is_err()
        );

        let mut document: Value = serde_json::from_str(CATALOG_JSON).unwrap();
        document["assets"][1]["asset_id"] = document["assets"][0]["asset_id"].clone();
        assert!(
            AssetCatalog::load_and_validate(
                &repository_root(),
                &serde_json::to_vec(&document).unwrap()
            )
            .is_err()
        );

        assert!(!is_safe_asset_id("UI.Icon.Close"));
        assert!(validate_packaged_path("ui/icons/close.png").is_ok());
        assert!(validate_packaged_path("ui\\icons\\close.png").is_err());
        assert!(validate_packaged_path("ui//icons/close.png").is_err());
        assert!(
            AssetCatalog::load_and_validate(
                &repository_root(),
                &vec![b' '; MAX_CATALOG_JSON_BYTES + 1]
            )
            .is_err()
        );
        let mut folded_paths = BTreeMap::new();
        record_case_insensitive_path(&mut folded_paths, "images/Nested/item.png").unwrap();
        assert!(record_case_insensitive_path(&mut folded_paths, "images/nested/item.png").is_err());
    }

    #[test]
    fn strategy_classifies_existing_programmatic_and_placeholder_deterministically() {
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let analysis = regular_analysis();
        let plan = plan_analysis(&analysis);
        let manifest = build_asset_strategy(
            &analysis,
            &plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::ExistingAsset {
                    asset_id: "ui.image.battlepass_dragon_01".to_owned(),
                },
            }],
        )
        .unwrap();
        assert_eq!(manifest.entries[0].element_id, "page.root");
        assert_eq!(
            manifest.entries[0].disposition,
            AssetDisposition::ExistingAsset
        );
        assert_eq!(
            manifest.entries[1].programmatic,
            Some(ProgrammaticRepresentation::Text)
        );
        assert_eq!(
            manifest.diagnostics[0].code,
            "ASSET_EXISTING_LICENSE_UNKNOWN"
        );

        let mut analysis = analysis;
        analysis.elements[0].kind = VisualElementKind::Image;
        let plan = plan_analysis(&analysis);
        let fallback = build_asset_strategy(&analysis, &plan, &catalog, &[], &[]).unwrap();
        assert_eq!(
            fallback.entries[0].disposition,
            AssetDisposition::Placeholder
        );
        assert_eq!(
            fallback.entries[0].placeholder_reason,
            Some(PlaceholderReason::NoApprovedMatch)
        );
        assert_eq!(
            fallback.diagnostics[0].code,
            "ASSET_PLACEHOLDER_NO_APPROVED_MATCH"
        );
    }

    #[test]
    fn strategy_records_recreate_generate_and_explicit_programmatic_categories() {
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let mut analysis = regular_analysis();
        let plan = plan_analysis(&analysis);
        let programmatic = build_asset_strategy(
            &analysis,
            &plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::Programmatic {
                    representation: ProgrammaticRepresentation::SolidSurface,
                },
            }],
        )
        .unwrap();
        assert_eq!(
            programmatic.entries[0].disposition(),
            AssetDisposition::Programmatic
        );

        analysis.elements[0].kind = VisualElementKind::Image;
        let plan = plan_analysis(&analysis);
        let generated = GenerationProvenance {
            tool_id: "offline.fixture".to_owned(),
            tool_version: "1.0.0".to_owned(),
            prompt_summary: GenerationPromptSummary {
                subject_tags: vec!["badge".to_owned()],
                style_tags: vec!["flat".to_owned()],
            },
            license: GeneratedAssetLicense {
                status: GeneratedLicenseStatus::Pending,
                reference: None,
            },
            approval_status: RecordedApprovalStatus::PendingHumanReview,
        };
        let cases = [
            (
                AssetDecision::Recreate {
                    specification: raster_spec(32, 32, AssetUsage::ContentImage),
                },
                AssetDisposition::Recreate,
            ),
            (
                AssetDecision::Generate {
                    specification: raster_spec(32, 32, AssetUsage::ContentImage),
                    provenance: generated,
                },
                AssetDisposition::Generate,
            ),
            (
                AssetDecision::Placeholder {
                    reason: PlaceholderReason::AuthorizationRejected,
                },
                AssetDisposition::Placeholder,
            ),
        ];
        for (decision, expected) in cases {
            let strategy = build_asset_strategy(
                &analysis,
                &plan,
                &catalog,
                &[],
                &[AssetDecisionRequest {
                    element_id: "page.root".to_owned(),
                    decision,
                }],
            )
            .unwrap();
            assert_eq!(strategy.entries[0].disposition(), expected);
            if expected == AssetDisposition::Placeholder {
                assert_eq!(
                    strategy.entries[0].placeholder_reason,
                    Some(PlaceholderReason::AuthorizationRejected)
                );
            }
        }
    }

    #[test]
    fn strategy_rejects_incompatible_assets_programmatic_representations_and_usage() {
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let analysis = regular_analysis();
        let plan = plan_analysis(&analysis);
        let wrong_background_asset = build_asset_strategy(
            &analysis,
            &plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::ExistingAsset {
                    asset_id: "ui.icon.close".to_owned(),
                },
            }],
        )
        .unwrap_err();
        assert!(wrong_background_asset.message().contains("incompatible"));

        let wrong_programmatic = build_asset_strategy(
            &analysis,
            &plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::Programmatic {
                    representation: ProgrammaticRepresentation::Border,
                },
            }],
        )
        .unwrap_err();
        assert!(wrong_programmatic.message().contains("incompatible"));

        let mut image_analysis = analysis;
        image_analysis.elements[0].kind = VisualElementKind::Image;
        let image_plan = plan_analysis(&image_analysis);
        let background_as_content = build_asset_strategy(
            &image_analysis,
            &image_plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::ExistingAsset {
                    asset_id: "ui.image.battlepass_dragon_01".to_owned(),
                },
            }],
        )
        .unwrap_err();
        assert!(background_as_content.message().contains("incompatible"));
        let wrong_usage = build_asset_strategy(
            &image_analysis,
            &image_plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::Recreate {
                    specification: raster_spec(32, 32, AssetUsage::Icon),
                },
            }],
        )
        .unwrap_err();
        assert!(wrong_usage.message().contains("asset usage"));
    }

    #[test]
    fn strategy_rejects_invalid_analysis_and_forged_planning_protocol() {
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let analysis = regular_analysis();
        let mut forged_plan = plan_analysis(&analysis);
        forged_plan.protocol_version = PLANNING_PROTOCOL_VERSION + 1;
        let failure =
            build_asset_strategy(&analysis, &forged_plan, &catalog, &[], &[]).unwrap_err();
        assert!(failure.message().contains("planning protocol version"));

        let mut invalid = analysis;
        invalid.root_element_id = "missing.root".to_owned();
        let failure = build_asset_strategy(&invalid, &plan_analysis(&invalid), &catalog, &[], &[])
            .unwrap_err();
        assert!(failure.message().contains("semantically valid analysis"));
        assert!(failure.message().contains("ANALYSIS_GRAPH_ROOT_MISMATCH"));
    }

    #[test]
    fn crop_authorization_is_fail_closed_and_binds_stage3_identity() {
        for denied in [
            ImageAuthorization::AnalysisOnly,
            ImageAuthorization::DistributionAllowed,
            ImageAuthorization::Unknown,
            ImageAuthorization::Denied,
        ] {
            let (task, manifest, bytes, _, analysis) = crop_fixture(denied);
            let trusted = TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes);
            if denied == ImageAuthorization::Denied {
                assert!(
                    trusted
                        .unwrap_err()
                        .message()
                        .contains("denied authorization")
                );
                continue;
            }
            let trusted = trusted.unwrap();
            let failure = build_asset_strategy(
                &analysis,
                &plan_analysis(&analysis),
                &AssetCatalog {
                    schema_version: 1,
                    assets: vec![],
                },
                &[trusted],
                &[AssetDecisionRequest {
                    element_id: "page.root".to_owned(),
                    decision: AssetDecision::AuthorizedCrop {
                        specification: raster_spec(4, 4, AssetUsage::ContentImage),
                    },
                }],
            )
            .unwrap_err();
            assert!(failure.message().contains("fail-closed"));
        }

        let (mut task, mut manifest, _, _, mut analysis) =
            crop_fixture(ImageAuthorization::DerivativesAllowed);
        task.primary_reference.metadata.provenance.license_reference = None;
        manifest.cache_key = trusted_cache_key(
            &task.primary_reference,
            &manifest,
            task.target_viewport.unwrap(),
            ReferenceValidationProfile::PageReference,
        )
        .unwrap();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        analysis.references[0].preprocess_cache_key = manifest.cache_key.clone();
        analysis.references[0].preprocess_manifest_sha256 = sha256_bytes(&bytes);
        let trusted = TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap();
        let failure = build_asset_strategy(
            &analysis,
            &plan_analysis(&analysis),
            &AssetCatalog {
                schema_version: 1,
                assets: vec![],
            },
            &[trusted],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::AuthorizedCrop {
                    specification: raster_spec(4, 4, AssetUsage::ContentImage),
                },
            }],
        )
        .unwrap_err();
        assert!(
            failure
                .message()
                .contains("license or permission reference")
        );

        let (task, manifest, bytes, _, mut analysis) =
            crop_fixture(ImageAuthorization::DerivativesAllowed);
        let trusted = TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap();
        analysis.references[0].preprocess_cache_key = "c".repeat(64);
        let failure = build_asset_strategy(
            &analysis,
            &plan_analysis(&analysis),
            &AssetCatalog {
                schema_version: 1,
                assets: vec![],
            },
            &[trusted],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::AuthorizedCrop {
                    specification: raster_spec(4, 4, AssetUsage::ContentImage),
                },
            }],
        )
        .unwrap_err();
        assert!(failure.message().contains("does not match trusted Stage 3"));
    }

    #[test]
    fn trusted_crop_source_rejects_forged_task_manifest_and_mapping() {
        let (mut task, manifest, _, _, _) = crop_fixture(ImageAuthorization::DerivativesAllowed);
        task.primary_reference.reference_id = "../escape".to_owned();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let failure =
            TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap_err();
        assert!(failure.message().contains("safe stable identifier"));

        let (task, mut manifest, _, _, _) = crop_fixture(ImageAuthorization::DerivativesAllowed);
        manifest.protocol_version += 1;
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let failure =
            TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap_err();
        assert!(failure.message().contains("protocol"));

        let (task, mut manifest, _, _, _) = crop_fixture(ImageAuthorization::DerivativesAllowed);
        manifest.coordinate_mapping.logical_to_physical_scale.x = 2.0;
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let failure =
            TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap_err();
        assert!(failure.message().contains("coordinate mapping"));

        let (task, mut manifest, _, _, _) = crop_fixture(ImageAuthorization::DerivativesAllowed);
        manifest.artifacts.push(manifest.artifacts[0].clone());
        manifest.artifacts[1].file_name = "duplicate-preview.png".to_owned();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let failure =
            TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap_err();
        assert!(failure.message().contains("exactly one"));
    }

    #[test]
    fn authorized_crop_maps_coordinates_and_writes_only_run_assets() {
        let (task, manifest, bytes, preview, analysis) =
            crop_fixture(ImageAuthorization::DerivativesAllowed);
        let trusted = TrustedAssetSource::from_task_and_manifest(&task, &manifest, &bytes).unwrap();
        let strategy = build_asset_strategy(
            &analysis,
            &plan_analysis(&analysis),
            &AssetCatalog {
                schema_version: 1,
                assets: vec![],
            },
            &[trusted],
            &[AssetDecisionRequest {
                element_id: "page.root".to_owned(),
                decision: AssetDecision::AuthorizedCrop {
                    specification: raster_spec(4, 4, AssetUsage::ContentImage),
                },
            }],
        )
        .unwrap();
        let crop = strategy.entries[0].crop.as_ref().unwrap();
        assert_eq!(
            crop.preview_crop,
            PixelRect {
                x: 2,
                y: 2,
                width: 4,
                height: 4
            }
        );
        assert_eq!(crop.exif_normalized_crop.x, 2.0);
        assert_eq!(crop.exif_normalized_crop.width, 4.0);

        let repository = tempfile::tempdir().unwrap();
        fs::create_dir_all(repository.path().join("project/assets")).unwrap();
        let reference_root = repository
            .path()
            .join("summary/ui-generation/crop-run/input/preprocessed/primary");
        fs::create_dir_all(&reference_root).unwrap();
        fs::create_dir_all(
            repository
                .path()
                .join("summary/ui-generation/crop-run/assets"),
        )
        .unwrap();
        fs::write(reference_root.join("standard-preview.png"), preview).unwrap();
        let before = snapshot_regular_files(&repository.path().join("project/assets")).unwrap();
        let result = extract_authorized_crop(
            repository.path(),
            "crop-run",
            &strategy.entries[0],
            "draft.primary_crop",
        )
        .unwrap();
        assert_eq!((result.width, result.height), (4, 4));
        assert!(
            result.path.starts_with(
                repository
                    .path()
                    .join("summary/ui-generation/crop-run/assets")
                    .canonicalize()
                    .unwrap()
            )
        );
        assert_eq!(
            before,
            snapshot_regular_files(&repository.path().join("project/assets")).unwrap()
        );
        let committed_bytes = fs::read(&result.path).unwrap();
        let conflict = extract_authorized_crop(
            repository.path(),
            "crop-run",
            &strategy.entries[0],
            "draft.primary_crop",
        )
        .unwrap_err();
        assert_eq!(conflict.kind(), TaskFailureKind::OutputDirectoryConflict);
        assert_eq!(fs::read(&result.path).unwrap(), committed_bytes);

        let dotted =
            extract_authorized_crop(repository.path(), "crop-run", &strategy.entries[0], "a.b")
                .unwrap();
        let underscored =
            extract_authorized_crop(repository.path(), "crop-run", &strategy.entries[0], "a_b")
                .unwrap();
        assert_ne!(dotted.path, underscored.path);
        assert_eq!(
            draft_asset_id_from_file_name(dotted.path.file_name().unwrap().to_str().unwrap())
                .unwrap(),
            "a.b"
        );
        assert_eq!(
            draft_asset_id_from_file_name(underscored.path.file_name().unwrap().to_str().unwrap())
                .unwrap(),
            "a_b"
        );
    }

    #[test]
    fn atomic_commit_never_replaces_a_competing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let staging = directory.path().join("staging");
        let destination = directory.path().join("destination");
        fs::write(&staging, b"new draft bytes").unwrap();
        fs::write(&destination, b"competing writer bytes").unwrap();
        let failure =
            commit_staged_file_no_clobber(&staging, &destination, directory.path()).unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::OutputDirectoryConflict);
        assert_eq!(fs::read(&destination).unwrap(), b"competing writer bytes");
        assert_eq!(fs::read(&staging).unwrap(), b"new draft bytes");
    }

    #[test]
    fn draft_asset_file_mapping_is_lossless_and_collision_free() {
        let dotted = draft_asset_file_name("a.b").unwrap();
        let underscored = draft_asset_file_name("a_b").unwrap();
        assert_ne!(dotted, underscored);
        assert_eq!(draft_asset_id_from_file_name(&dotted).unwrap(), "a.b");
        assert_eq!(draft_asset_id_from_file_name(&underscored).unwrap(), "a_b");

        let longest = format!("a{}", "_".repeat(ASSET_ID_MAX_BYTES - 1));
        let file_name = draft_asset_file_name(&longest).unwrap();
        assert!(file_name.len() <= 255);
        assert_eq!(draft_asset_id_from_file_name(&file_name).unwrap(), longest);
    }

    #[test]
    fn generation_summary_is_structured_and_cannot_self_approve() {
        let valid = GenerationProvenance {
            tool_id: "offline.fixture".to_owned(),
            tool_version: "1.2.0".to_owned(),
            prompt_summary: GenerationPromptSummary {
                subject_tags: vec!["dragon".to_owned()],
                style_tags: vec!["flat".to_owned()],
            },
            license: GeneratedAssetLicense {
                status: GeneratedLicenseStatus::Pending,
                reference: None,
            },
            approval_status: RecordedApprovalStatus::PendingHumanReview,
        };
        assert!(valid.validate_for_draft().is_ok());
        let mut self_approved = valid;
        self_approved.approval_status = RecordedApprovalStatus::Approved;
        assert!(self_approved.validate_for_draft().is_err());
        self_approved.approval_status = RecordedApprovalStatus::PendingHumanReview;
        self_approved.prompt_summary.subject_tags = vec!["Bearer secret".to_owned()];
        assert!(self_approved.validate_for_draft().is_err());

        let missing_insets = AssetSpecification {
            width: 32,
            height: 32,
            alpha: AlphaMode::Straight,
            slice_insets: None,
            color_space: RequiredColorSpace::Srgb,
            usage: AssetUsage::NineSlice,
        };
        assert!(missing_insets.validate().is_err());
        let oversized = AssetSpecification {
            width: MAX_ASSET_SPEC_EDGE + 1,
            height: 1,
            alpha: AlphaMode::Opaque,
            slice_insets: None,
            color_space: RequiredColorSpace::Srgb,
            usage: AssetUsage::Background,
        };
        assert!(oversized.validate().is_err());
        let overflowing_insets = AssetSpecification {
            width: 32,
            height: 32,
            alpha: AlphaMode::Straight,
            slice_insets: Some(SliceInsets {
                left: u32::MAX,
                right: 1,
                top: 1,
                bottom: 1,
            }),
            color_space: RequiredColorSpace::Srgb,
            usage: AssetUsage::NineSlice,
        };
        let failure = overflowing_insets.validate().unwrap_err();
        assert!(failure.message().contains("overflow u32"));
    }

    #[test]
    fn quality_check_reports_alpha_edges_color_compression_and_android_limits() {
        let image = RgbaImage::from_fn(3, 3, |x, y| {
            if x == 1 && y == 1 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([12, 34, 56, 0])
            }
        });
        let png = png_bytes(&image);
        let report = inspect_asset_bytes(&png, &raster_spec(3, 3, AssetUsage::Icon));
        assert_eq!(report.verdict, AssetQualityVerdict::ReviewRequired);
        let codes: BTreeSet<_> = report
            .findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect();
        assert!(codes.contains("ASSET_TRANSPARENT_RGB_BLEED"));
        assert!(codes.contains("ASSET_COLOR_SPACE_ASSUMED_SRGB"));

        let rgb = RgbImage::from_pixel(8, 8, Rgb([80, 90, 100]));
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 80)
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        let jpeg_report = inspect_asset_bytes(
            &jpeg,
            &AssetSpecification {
                width: 8,
                height: 8,
                alpha: AlphaMode::Opaque,
                slice_insets: None,
                color_space: RequiredColorSpace::Srgb,
                usage: AssetUsage::Background,
            },
        );
        assert_eq!(jpeg_report.verdict, AssetQualityVerdict::ReviewRequired);
        assert!(
            jpeg_report
                .findings
                .iter()
                .any(|finding| finding.code == "ASSET_LOSSY_COMPRESSION_REVIEW")
        );

        let mismatch = inspect_asset_bytes(&png, &raster_spec(4, 3, AssetUsage::Icon));
        assert_eq!(mismatch.verdict, AssetQualityVerdict::Reject);
        assert!(
            mismatch
                .findings
                .iter()
                .any(|finding| finding.code == "ASSET_DIMENSION_MISMATCH")
        );

        let oversized = RgbaImage::from_pixel(MAX_ASSET_SPEC_EDGE + 1, 1, Rgba([10, 20, 30, 255]));
        let oversized_report =
            inspect_asset_bytes(&png_bytes(&oversized), &raster_spec(1, 1, AssetUsage::Icon));
        assert_eq!(oversized_report.verdict, AssetQualityVerdict::Reject);
        assert!(
            oversized_report
                .findings
                .iter()
                .any(|finding| finding.code == "ASSET_ANDROID_TEXTURE_LIMIT")
        );

        let over_encoded = vec![0_u8; MAX_DRAFT_ASSET_BYTES as usize + 1];
        let over_encoded_report =
            inspect_asset_bytes(&over_encoded, &raster_spec(1, 1, AssetUsage::Icon));
        assert_eq!(over_encoded_report.verdict, AssetQualityVerdict::Reject);
        assert_eq!(over_encoded_report.findings.len(), 1);
        assert_eq!(
            over_encoded_report.findings[0].code,
            "ASSET_ENCODED_SIZE_UNSAFE"
        );
    }

    #[test]
    fn catalog_recursively_requires_nested_production_assets() {
        let repository = tempfile::tempdir().unwrap();
        let ui_root = repository.path().join("project/assets/ui");
        for directory in ["atlas", "icons", "images", "fonts"] {
            fs::create_dir_all(ui_root.join(directory)).unwrap();
        }
        let image = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 40]));
        let bytes = png_bytes(&image);
        fs::write(ui_root.join("images/top.png"), &bytes).unwrap();
        fs::create_dir_all(ui_root.join("images/nested/deeper")).unwrap();
        fs::write(ui_root.join("images/nested/deeper/item.png"), &bytes).unwrap();
        let entry = |asset_id: &str, path: &str| {
            serde_json::json!({
                "asset_id": asset_id,
                "path": path,
                "kind": "raster",
                "sha256": sha256_bytes(&bytes),
                "byte_length": bytes.len(),
                "width": 2,
                "height": 2,
                "alpha": "straight",
                "license": {"status": "unknown", "reference": null},
                "tags": ["image"]
            })
        };
        let mut catalog = serde_json::json!({
            "schema_version": 1,
            "assets": [entry("ui.image.top", "ui/images/top.png")]
        });
        let failure = AssetCatalog::load_and_validate(
            repository.path(),
            &serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap_err();
        assert!(
            failure
                .message()
                .contains("ui/images/nested/deeper/item.png")
        );

        catalog["assets"].as_array_mut().unwrap().push(entry(
            "ui.image.nested_item",
            "ui/images/nested/deeper/item.png",
        ));
        let loaded = AssetCatalog::load_and_validate(
            repository.path(),
            &serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap();
        assert_eq!(loaded.assets().len(), 2);
    }

    #[test]
    fn catalog_rejects_symlinked_production_assets_when_platform_allows_fixture() {
        let repository = tempfile::tempdir().unwrap();
        let ui_root = repository.path().join("project/assets/ui");
        for directory in ["atlas", "icons", "images", "fonts"] {
            fs::create_dir_all(ui_root.join(directory)).unwrap();
        }
        let outside = repository.path().join("outside.png");
        fs::write(&outside, b"not a real png").unwrap();
        fs::create_dir_all(ui_root.join("icons/nested")).unwrap();
        let link = ui_root.join("icons/nested/link.png");
        if create_file_symlink(&outside, &link).is_err() {
            return;
        }
        let catalog = serde_json::json!({
            "schema_version": 1,
            "assets": [{
                "asset_id": "ui.icon.link",
                "path": "ui/icons/nested/link.png",
                "kind": "raster",
                "sha256": "0".repeat(64),
                "byte_length": 1,
                "width": 1,
                "height": 1,
                "alpha": "straight",
                "license": {"status": "unknown", "reference": null},
                "tags": ["icon"]
            }]
        });
        let failure = AssetCatalog::load_and_validate(
            repository.path(),
            &serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::UnsafeOutputPath);
    }

    #[cfg(unix)]
    fn create_file_symlink(source: &Path, destination: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, destination)
    }

    #[cfg(windows)]
    fn create_file_symlink(source: &Path, destination: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(source, destination)
    }
}
