//! Verified local storage for immutable, approved UI update generations.
//!
//! Network transport, signatures, and release-channel policy are intentionally outside this
//! module. Callers import a complete bundle into `staging`; this module validates every byte and
//! exposes a generation only after an immutable activation record has been committed.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Component, Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::{AssetApp, Resource};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UiApprovedDocumentHostContract,
    UiAssetDeclaredSize, UiAssetId, UiAssetKind, UiAssetSource, UiDocument, UiDocumentId,
    UiDocumentInputMode, UiDocumentPlatform, UiDocumentSourcePath, UiDocumentSourceRoot,
    UiSafeAreaClass, UiTargetProfile, parse_approved_document_registration,
};

pub const UI_UPDATE_BUNDLE_FORMAT_VERSION: u32 = 1;
pub const UI_UPDATE_CLIENT_REVISION: u32 = 1;
pub const UI_UPDATE_POLICY_REVISION: u32 = 1;
pub const UI_UPDATE_MANIFEST_MAX_BYTES: usize = 64 * 1024;
pub const UI_UPDATE_MAX_DOCUMENTS: usize = 64;
pub const UI_UPDATE_MAX_ASSETS: usize = 128;
pub const UI_UPDATE_MAX_FILES: usize = UI_UPDATE_MAX_DOCUMENTS * 2 + UI_UPDATE_MAX_ASSETS;

const MANIFEST_FILE: &str = "manifest.v1.json";
const COMMIT_FORMAT_VERSION: u32 = 1;
const CACHE_LAYOUT_DIRECTORIES: [&str; 5] =
    ["staging", "generations", "active", "previous", "quarantine"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateBundle {
    pub format_version: u32,
    pub bundle_id: String,
    pub channel: String,
    pub version: String,
    pub client_compatibility: UiUpdateRevisionRange,
    pub schema_compatibility: UiUpdateRevisionRange,
    pub policy_revision: u32,
    pub total_bytes: u64,
    pub documents: Vec<UiUpdateDocumentEntry>,
    pub assets: Vec<UiUpdateAssetEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateRevisionRange {
    pub minimum: u32,
    pub maximum: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateDocumentEntry {
    pub document_id: String,
    /// Bundle-internal payload location, never a runtime filesystem path.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub registration_path: String,
    pub registration_bytes: u64,
    pub registration_sha256: String,
    /// The reviewed `approved` logical source recorded in the promotion registration.
    pub approved_relative_path: String,
    /// Exact set of `content_cache` logical asset IDs referenced by this document.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateAssetEntry {
    pub logical_id: String,
    pub kind: UiAssetKind,
    /// Bundle-internal payload location, never a URL or host path.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub media_type: String,
    #[serde(default)]
    pub declared_size: Option<UiAssetDeclaredSize>,
    pub license: UiUpdateAssetLicense,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateAssetLicense {
    pub license_id: String,
    pub attribution: String,
    pub redistribution_permitted: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UiUpdateBundleImport {
    pub manifest_json: Vec<u8>,
    /// Every expected document, registration, and asset payload keyed by its manifest path.
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct UiUpdateCacheConfig {
    cache_root: PathBuf,
    bundle_id: String,
    channel: String,
    client_revision: u32,
    policy_revision: u32,
    policy: UiUpdateCachePolicy,
    host_contracts: BTreeMap<UiDocumentId, UiApprovedDocumentHostContract>,
}

impl UiUpdateCacheConfig {
    pub fn new(
        cache_root: impl Into<PathBuf>,
        bundle_id: impl Into<String>,
        channel: impl Into<String>,
    ) -> Result<Self, UiUpdateCacheError> {
        let bundle_id = bundle_id.into();
        let channel = channel.into();
        if !safe_label(&bundle_id) || !safe_label(&channel) {
            return Err(UiUpdateCacheError::new("UI_UPDATE_CACHE_IDENTITY_INVALID"));
        }
        Ok(Self {
            cache_root: cache_root.into(),
            bundle_id,
            channel,
            client_revision: UI_UPDATE_CLIENT_REVISION,
            policy_revision: UI_UPDATE_POLICY_REVISION,
            policy: UiUpdateCachePolicy::default(),
            host_contracts: BTreeMap::new(),
        })
    }

    pub fn with_policy(mut self, policy: UiUpdateCachePolicy) -> Result<Self, UiUpdateCacheError> {
        policy.validate()?;
        self.policy = policy;
        Ok(self)
    }

    pub fn with_revisions(mut self, client_revision: u32, policy_revision: u32) -> Self {
        self.client_revision = client_revision;
        self.policy_revision = policy_revision;
        self
    }

    /// Contracts are supplied by the game binary, never by a downloaded bundle.
    pub fn with_host_contract(
        mut self,
        document_id: UiDocumentId,
        contract: UiApprovedDocumentHostContract,
    ) -> Self {
        self.host_contracts.insert(document_id, contract);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiUpdateCachePolicy {
    pub max_cache_bytes: u64,
    pub max_bundle_bytes: u64,
    pub max_generations: usize,
}

impl Default for UiUpdateCachePolicy {
    fn default() -> Self {
        Self {
            max_cache_bytes: 256 * 1024 * 1024,
            max_bundle_bytes: 96 * 1024 * 1024,
            max_generations: 3,
        }
    }
}

impl UiUpdateCachePolicy {
    fn validate(self) -> Result<(), UiUpdateCacheError> {
        if self.max_bundle_bytes == 0
            || self.max_cache_bytes < self.max_bundle_bytes
            || self.max_generations < 2
        {
            return Err(UiUpdateCacheError::new("UI_UPDATE_CACHE_POLICY_INVALID"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct UiUpdateCache {
    config: UiUpdateCacheConfig,
    root: PathBuf,
    leases: Arc<Mutex<BTreeMap<String, u32>>>,
}

impl UiUpdateCache {
    /// Opens only a user-data cache root. A repository or packaged-asset directory is rejected.
    pub fn open(config: UiUpdateCacheConfig) -> Result<Self, UiUpdateCacheError> {
        config.policy.validate()?;
        let root = resolve_cache_root(&config.cache_root)?;
        let cache = Self {
            config,
            root,
            leases: Arc::default(),
        };
        cache.ensure_layout()?;
        cache.recover_interrupted_staging()?;
        cache.prune()?;
        Ok(cache)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Registers the named reader used by `UiDocumentContentCacheAssets`. The caller must invoke
    /// this before `DefaultPlugins` creates Bevy's `AssetPlugin`; this module never registers a
    /// repository or APK path as writable content storage.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_asset_source(&self, app: &mut bevy::prelude::App) {
        use bevy::asset::io::{AssetSourceBuilder, file::FileAssetReader};

        let generations_root = self.root.join("generations").to_string_lossy().into_owned();
        app.register_asset_source(
            "content_cache",
            AssetSourceBuilder::new(move || {
                Box::new(FileAssetReader::new(generations_root.clone()))
            }),
        );
    }

    /// Validates and writes an import only under `<cache>/staging`. It is not visible to UI code.
    pub fn stage(
        &self,
        import: &UiUpdateBundleImport,
    ) -> Result<UiUpdateStagedBundle, UiUpdateCacheError> {
        let bundle = self.verify_import(import)?;
        let incoming_bytes = bundle
            .total_bytes
            .checked_add(import.manifest_json.len() as u64)
            .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_SIZE_INVALID"))?;
        self.ensure_capacity_for(incoming_bytes)?;
        let generation_id = generation_id(&bundle, &import.manifest_json)?;
        let staging = self.root.join("staging").join(&generation_id);
        if staging.exists() {
            self.quarantine_path(&staging, "staging-replaced")?;
        }
        fs::create_dir_all(&staging)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_CREATE_FAILED"))?;
        let write_result = (|| {
            write_staging_file(&staging.join(MANIFEST_FILE), &import.manifest_json)?;
            for (path, bytes) in &import.files {
                write_staging_file(&staging.join(path), bytes)?;
            }
            self.verify_generation_directory(&staging, Some(&generation_id))?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = self.quarantine_path(&staging, "staging-invalid");
            return Err(error);
        }
        Ok(UiUpdateStagedBundle { generation_id })
    }

    /// Moves a fully verified staging directory into immutable storage and creates a new active
    /// commit record last. If the process stops before that record exists, the old generation
    /// remains active on the next startup.
    pub fn activate(
        &self,
        staged: UiUpdateStagedBundle,
    ) -> Result<UiUpdateActiveGeneration, UiUpdateCacheError> {
        if !safe_generation_id(&staged.generation_id) {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_CACHE_STAGING_ID_INVALID",
            ));
        }
        let staging = self.root.join("staging").join(&staged.generation_id);
        let verified = self.verify_generation_directory(&staging, Some(&staged.generation_id))?;
        let previous = self.active_generation()?;
        let generation = self.root.join("generations").join(&staged.generation_id);
        if generation.exists() {
            let existing =
                self.verify_generation_directory(&generation, Some(&staged.generation_id))?;
            if existing != verified {
                let _ = self.quarantine_path(&staging, "generation-conflict");
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_CACHE_GENERATION_CONFLICT",
                ));
            }
            fs::remove_dir_all(&staging)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_CLEANUP_FAILED"))?;
        } else {
            fs::rename(&staging, &generation)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_GENERATION_COMMIT_FAILED"))?;
        }

        let sequence = self.next_commit_sequence()?;
        if let Some(previous) = previous {
            self.write_commit("previous", sequence, &previous)?;
        }
        let active =
            UiUpdateActiveGeneration::new(self.root.clone(), staged.generation_id, verified);
        self.write_commit("active", sequence, &active)?;
        self.prune()?;
        Ok(active)
    }

    pub fn install(
        &self,
        import: &UiUpdateBundleImport,
    ) -> Result<UiUpdateActiveGeneration, UiUpdateCacheError> {
        let staged = self.stage(import)?;
        self.activate(staged)
    }

    /// Returns the newest fully verified active generation. Broken records are quarantined and
    /// the preceding verified record becomes active automatically.
    pub fn active_generation(
        &self,
    ) -> Result<Option<UiUpdateActiveGeneration>, UiUpdateCacheError> {
        self.latest_verified_generation("active", true)
    }

    pub fn previous_generation(
        &self,
    ) -> Result<Option<UiUpdateActiveGeneration>, UiUpdateCacheError> {
        self.latest_verified_generation("previous", false)
    }

    pub fn lease_active(&self) -> Result<Option<UiUpdateGenerationLease>, UiUpdateCacheError> {
        let Some(generation) = self.active_generation()? else {
            return Ok(None);
        };
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_LEASE_POISONED"))?;
        *leases.entry(generation.generation_id.clone()).or_default() += 1;
        Ok(Some(UiUpdateGenerationLease {
            generation,
            leases: Arc::clone(&self.leases),
        }))
    }

    /// Retains active, previous, and leased generations before evicting old unreferenced data.
    pub fn prune(&self) -> Result<(), UiUpdateCacheError> {
        let protected = self.protected_generation_ids()?;
        let mut candidates = self.generation_recency()?;
        candidates.retain(|(id, _)| !protected.contains(id));
        candidates.sort_by_key(|(_, sequence)| *sequence);

        let generation_count = self.generation_ids()?.len();
        let mut remaining = generation_count;
        let mut used = directory_size(&self.root)?;
        for (generation_id, _) in candidates {
            if remaining <= self.config.policy.max_generations
                && used <= self.config.policy.max_cache_bytes
            {
                break;
            }
            let path = self.root.join("generations").join(&generation_id);
            let bytes = directory_size(&path)?;
            fs::remove_dir_all(&path)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_EVICTION_FAILED"))?;
            self.remove_commit_records_for(&generation_id)?;
            remaining = remaining.saturating_sub(1);
            used = used.saturating_sub(bytes);
        }
        Ok(())
    }

    fn verify_import(
        &self,
        import: &UiUpdateBundleImport,
    ) -> Result<UiUpdateBundle, UiUpdateCacheError> {
        if import.manifest_json.len() > UI_UPDATE_MANIFEST_MAX_BYTES {
            return Err(UiUpdateCacheError::new("UI_UPDATE_MANIFEST_BYTES_EXCEEDED"));
        }
        let manifest = parse_manifest(&import.manifest_json)?;
        self.verify_bundle(&manifest, &import.files)?;
        Ok(manifest)
    }

    fn verify_generation_directory(
        &self,
        directory: &Path,
        expected_generation_id: Option<&str>,
    ) -> Result<UiUpdateBundle, UiUpdateCacheError> {
        if !directory.is_dir() {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_CACHE_GENERATION_MISSING",
            ));
        }
        let manifest_json = read_file_up_to_limit(
            &directory.join(MANIFEST_FILE),
            UI_UPDATE_MANIFEST_MAX_BYTES as u64,
        )?;
        let manifest = parse_manifest(&manifest_json)?;
        if let Some(expected_generation_id) = expected_generation_id
            && generation_id(&manifest, &manifest_json)? != expected_generation_id
        {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_CACHE_GENERATION_ID_MISMATCH",
            ));
        }
        let expected_paths = manifest_file_paths(&manifest)?;
        if stored_payload_paths(directory)? != expected_paths {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_BUNDLE_FILE_SET_MISMATCH",
            ));
        }
        let mut files = BTreeMap::new();
        for path in expected_paths {
            let entry = file_requirement(&manifest, &path)?;
            files.insert(
                path.clone(),
                read_limited_file(&directory.join(path), entry.bytes)?,
            );
        }
        self.verify_bundle(&manifest, &files)?;
        Ok(manifest)
    }

    fn verify_bundle(
        &self,
        bundle: &UiUpdateBundle,
        files: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), UiUpdateCacheError> {
        validate_bundle_header(bundle, &self.config)?;
        let required_paths = manifest_file_paths(bundle)?;
        if files.len() != required_paths.len()
            || files.keys().any(|path| !required_paths.contains(path))
        {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_BUNDLE_FILE_SET_MISMATCH",
            ));
        }
        let mut total = 0_u64;
        for path in &required_paths {
            let requirement = file_requirement(bundle, path)?;
            let bytes = files
                .get(path)
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_MISSING"))?;
            if u64::try_from(bytes.len()).ok() != Some(requirement.bytes)
                || sha256_hex(bytes) != requirement.sha256
            {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_BUNDLE_FILE_HASH_MISMATCH",
                ));
            }
            total = total
                .checked_add(requirement.bytes)
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_SIZE_INVALID"))?;
        }
        if total != bundle.total_bytes || total > self.config.policy.max_bundle_bytes {
            return Err(UiUpdateCacheError::new("UI_UPDATE_BUNDLE_SIZE_INVALID"));
        }
        self.verify_documents_and_assets(bundle, files)
    }

    fn verify_documents_and_assets(
        &self,
        bundle: &UiUpdateBundle,
        files: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), UiUpdateCacheError> {
        let mut assets = BTreeMap::new();
        for asset in &bundle.assets {
            validate_asset_entry(asset)?;
            let logical_id = UiAssetId::from_str(&asset.logical_id)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_ASSET_ID_INVALID"))?;
            if assets.insert(logical_id, asset).is_some() {
                return Err(UiUpdateCacheError::new("UI_UPDATE_ASSET_DUPLICATE"));
            }
        }
        let mut used_assets = BTreeSet::new();
        let mut document_ids = BTreeSet::new();
        for entry in &bundle.documents {
            let document_id = UiDocumentId::from_str(&entry.document_id)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_ID_INVALID"))?;
            if !document_ids.insert(document_id.clone()) {
                return Err(UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_DUPLICATE"));
            }
            let source = files
                .get(&entry.path)
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_MISSING"))?;
            let validation = UiDocument::validate_json_bytes(source);
            let document = validation
                .validated()
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_INVALID"))?
                .document();
            if document.document_id != document_id {
                return Err(UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_ID_MISMATCH"));
            }
            let source = std::str::from_utf8(source)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_UTF8_INVALID"))?;
            let registration_source = files
                .get(&entry.registration_path)
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_MISSING"))?;
            let registration_source = std::str::from_utf8(registration_source)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_REGISTRATION_UTF8_INVALID"))?;
            let registration = parse_approved_document_registration(registration_source)
                .map_err(|error| UiUpdateCacheError::new(error.code()))?;
            if registration.document_id() != &document_id {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_REGISTRATION_DOCUMENT_ID_MISMATCH",
                ));
            }
            let expected_source = UiDocumentSourcePath::new(
                UiDocumentSourceRoot::Approved,
                &entry.approved_relative_path,
            )
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_REGISTRATION_SOURCE_INVALID"))?;
            if registration.source_path() != &expected_source {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_REGISTRATION_SOURCE_MISMATCH",
                ));
            }
            match registration.host_contract() {
                Some(_) => {
                    let expected_contract = self
                        .config
                        .host_contracts
                        .get(&document_id)
                        .ok_or_else(|| {
                            UiUpdateCacheError::new("UI_UPDATE_HOST_CONTRACT_REQUIRED")
                        })?;
                    registration
                        .to_preview_registration_with_contract(
                            source.to_owned(),
                            validation_target_profile(),
                            Some(expected_contract),
                        )
                        .map_err(|error| UiUpdateCacheError::new(error.code()))?;
                }
                None => {
                    if self.config.host_contracts.contains_key(&document_id) {
                        return Err(UiUpdateCacheError::new("UI_UPDATE_HOST_CONTRACT_REQUIRED"));
                    }
                    registration
                        .validate_document_source_contract(source)
                        .map_err(|error| UiUpdateCacheError::new(error.code()))?;
                }
            }

            let dependencies = entry
                .dependencies
                .iter()
                .map(|value| {
                    UiAssetId::from_str(value).map_err(|_| {
                        UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_DEPENDENCY_INVALID")
                    })
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            if dependencies.len() != entry.dependencies.len() {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_DOCUMENT_DEPENDENCY_DUPLICATE",
                ));
            }
            let mut document_assets = BTreeSet::new();
            for document_asset in document.assets.values() {
                let UiAssetSource::ContentCache { logical_id } = &document_asset.source else {
                    continue;
                };
                let logical_id = UiAssetId::from_str(logical_id).map_err(|_| {
                    UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_DEPENDENCY_INVALID")
                })?;
                let asset = assets
                    .get(&logical_id)
                    .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_ASSET_MISSING"))?;
                if asset.kind != document_asset.kind
                    || asset.declared_size != document_asset.declared_size
                {
                    return Err(UiUpdateCacheError::new(
                        "UI_UPDATE_DOCUMENT_ASSET_METADATA_MISMATCH",
                    ));
                }
                document_assets.insert(logical_id.clone());
                used_assets.insert(logical_id);
            }
            if dependencies != document_assets {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_DOCUMENT_DEPENDENCY_MISMATCH",
                ));
            }
        }
        if used_assets != assets.keys().cloned().collect() {
            return Err(UiUpdateCacheError::new("UI_UPDATE_ASSET_UNUSED"));
        }
        Ok(())
    }

    fn ensure_layout(&self) -> Result<(), UiUpdateCacheError> {
        fs::create_dir_all(&self.root)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_CREATE_FAILED"))?;
        for directory in CACHE_LAYOUT_DIRECTORIES {
            fs::create_dir_all(self.root.join(directory))
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_LAYOUT_CREATE_FAILED"))?;
        }
        Ok(())
    }

    fn recover_interrupted_staging(&self) -> Result<(), UiUpdateCacheError> {
        let staging = self.root.join("staging");
        for entry in fs::read_dir(staging)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_READ_FAILED"))?
        {
            let entry =
                entry.map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_READ_FAILED"))?;
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                self.quarantine_path(&entry.path(), "staging-interrupted")?;
            }
        }
        Ok(())
    }

    fn ensure_capacity_for(&self, incoming_bytes: u64) -> Result<(), UiUpdateCacheError> {
        self.prune()?;
        let used = directory_size(&self.root)?;
        if incoming_bytes > self.config.policy.max_bundle_bytes
            || used.saturating_add(incoming_bytes) > self.config.policy.max_cache_bytes
        {
            return Err(UiUpdateCacheError::new("UI_UPDATE_CACHE_CAPACITY_EXCEEDED"));
        }
        Ok(())
    }

    fn latest_verified_generation(
        &self,
        kind: &str,
        quarantine_invalid: bool,
    ) -> Result<Option<UiUpdateActiveGeneration>, UiUpdateCacheError> {
        let mut records = self.read_commit_records(kind)?;
        records.sort_by_key(|record| std::cmp::Reverse(record.sequence));
        for record in records {
            let generation = self.root.join("generations").join(&record.generation_id);
            let result = self.verify_generation_directory(&generation, Some(&record.generation_id));
            match result {
                Ok(bundle)
                    if sha256_hex(&read_file_up_to_limit(
                        &generation.join(MANIFEST_FILE),
                        UI_UPDATE_MANIFEST_MAX_BYTES as u64,
                    )?) == record.manifest_sha256 =>
                {
                    return Ok(Some(UiUpdateActiveGeneration::new(
                        self.root.clone(),
                        record.generation_id,
                        bundle,
                    )));
                }
                Ok(_) | Err(_) if quarantine_invalid => {
                    self.quarantine_path(&record.path, "commit-invalid")?;
                }
                Ok(_) | Err(_) => {}
            }
        }
        Ok(None)
    }

    fn write_commit(
        &self,
        kind: &str,
        sequence: u64,
        generation: &UiUpdateActiveGeneration,
    ) -> Result<(), UiUpdateCacheError> {
        let manifest = read_file_up_to_limit(
            &generation.directory().join(MANIFEST_FILE),
            UI_UPDATE_MANIFEST_MAX_BYTES as u64,
        )?;
        let record = UiUpdateCommitRecord {
            format_version: COMMIT_FORMAT_VERSION,
            sequence,
            generation_id: generation.generation_id.clone(),
            manifest_sha256: sha256_hex(&manifest),
            path: PathBuf::new(),
        };
        let bytes = serde_json::to_vec_pretty(&record)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_SERIALIZE_FAILED"))?;
        let path = self
            .root
            .join(kind)
            .join(commit_file_name(sequence, &record.generation_id));
        write_new_file(&path, &bytes)
    }

    fn next_commit_sequence(&self) -> Result<u64, UiUpdateCacheError> {
        let max = self
            .read_commit_records("active")?
            .into_iter()
            .chain(self.read_commit_records("previous")?)
            .map(|record| record.sequence)
            .max()
            .unwrap_or_default();
        max.checked_add(1)
            .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_SEQUENCE_EXHAUSTED"))
    }

    fn protected_generation_ids(&self) -> Result<BTreeSet<String>, UiUpdateCacheError> {
        let mut protected = BTreeSet::new();
        if let Some(active) = self.active_generation()? {
            protected.insert(active.generation_id);
        }
        if let Some(previous) = self.previous_generation()? {
            protected.insert(previous.generation_id);
        }
        let leases = self
            .leases
            .lock()
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_LEASE_POISONED"))?;
        protected.extend(
            leases
                .iter()
                .filter_map(|(generation, count)| (*count > 0).then_some(generation.clone())),
        );
        Ok(protected)
    }

    fn generation_recency(&self) -> Result<Vec<(String, u64)>, UiUpdateCacheError> {
        let mut recency = self
            .generation_ids()?
            .into_iter()
            .map(|id| (id, 0_u64))
            .collect::<BTreeMap<_, _>>();
        for record in self
            .read_commit_records("active")?
            .into_iter()
            .chain(self.read_commit_records("previous")?)
        {
            if let Some(value) = recency.get_mut(&record.generation_id) {
                *value = (*value).max(record.sequence);
            }
        }
        Ok(recency.into_iter().collect())
    }

    fn generation_ids(&self) -> Result<BTreeSet<String>, UiUpdateCacheError> {
        let mut ids = BTreeSet::new();
        for entry in fs::read_dir(self.root.join("generations"))
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_GENERATION_READ_FAILED"))?
        {
            let entry = entry
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_GENERATION_READ_FAILED"))?;
            let id = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                && safe_generation_id(&id)
            {
                ids.insert(id);
            }
        }
        Ok(ids)
    }

    fn read_commit_records(
        &self,
        kind: &str,
    ) -> Result<Vec<UiUpdateCommitRecord>, UiUpdateCacheError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(self.root.join(kind))
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_READ_FAILED"))?
        {
            let entry =
                entry.map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_READ_FAILED"))?;
            if !entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
            {
                continue;
            }
            let path = entry.path();
            let bytes = match read_file_up_to_limit(&path, 8 * 1024) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let Ok(mut record) = serde_json::from_slice::<UiUpdateCommitRecord>(&bytes) else {
                continue;
            };
            if record.format_version != COMMIT_FORMAT_VERSION
                || !safe_generation_id(&record.generation_id)
                || !is_sha256(&record.manifest_sha256)
            {
                continue;
            }
            record.path = path;
            records.push(record);
        }
        Ok(records)
    }

    fn remove_commit_records_for(&self, generation_id: &str) -> Result<(), UiUpdateCacheError> {
        for kind in ["active", "previous"] {
            for record in self.read_commit_records(kind)? {
                if record.generation_id == generation_id {
                    fs::remove_file(record.path).map_err(|_| {
                        UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_REMOVE_FAILED")
                    })?;
                }
            }
        }
        Ok(())
    }

    fn quarantine_path(&self, path: &Path, reason: &str) -> Result<(), UiUpdateCacheError> {
        if !path.exists() {
            return Ok(());
        }
        let suffix = unique_suffix();
        let target = self
            .root
            .join("quarantine")
            .join(format!("{reason}-{suffix}"));
        fs::rename(path, target)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_QUARANTINE_FAILED"))
    }
}

#[derive(Clone, Debug)]
pub struct UiUpdateStagedBundle {
    generation_id: String,
}

#[derive(Clone, Debug)]
pub struct UiUpdateActiveGeneration {
    root: PathBuf,
    generation_id: String,
    bundle: UiUpdateBundle,
}

impl UiUpdateActiveGeneration {
    fn new(root: PathBuf, generation_id: String, bundle: UiUpdateBundle) -> Self {
        Self {
            root,
            generation_id,
            bundle,
        }
    }

    pub fn generation_id(&self) -> &str {
        &self.generation_id
    }

    pub fn bundle(&self) -> &UiUpdateBundle {
        &self.bundle
    }

    /// Returns source JSON with a logical `content_cache` source path, never a local path.
    pub fn document(
        &self,
        document_id: &UiDocumentId,
    ) -> Result<Option<UiUpdateCachedDocument>, UiUpdateCacheError> {
        let Some(entry) = self
            .bundle
            .documents
            .iter()
            .find(|entry| entry.document_id == document_id.as_str())
        else {
            return Ok(None);
        };
        let source = read_limited_file(&self.directory().join(&entry.path), entry.bytes)?;
        if sha256_hex(&source) != entry.sha256 {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_BUNDLE_FILE_HASH_MISMATCH",
            ));
        }
        let source_json = String::from_utf8(source)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_DOCUMENT_UTF8_INVALID"))?;
        let source_path = UiDocumentSourcePath::new(
            UiDocumentSourceRoot::ContentCache,
            format!("{}/{}", self.generation_id, entry.path),
        )
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_SOURCE_PATH_INVALID"))?;
        Ok(Some(UiUpdateCachedDocument {
            source_path,
            source_json,
        }))
    }

    fn directory(&self) -> PathBuf {
        self.root.join("generations").join(&self.generation_id)
    }
}

#[derive(Clone, Debug)]
pub struct UiUpdateCachedDocument {
    pub source_path: UiDocumentSourcePath,
    pub source_json: String,
}

/// Keeps a generation alive while a caller uses it. Eviction never removes leased generations.
#[derive(Debug)]
pub struct UiUpdateGenerationLease {
    generation: UiUpdateActiveGeneration,
    leases: Arc<Mutex<BTreeMap<String, u32>>>,
}

impl UiUpdateGenerationLease {
    pub fn generation(&self) -> &UiUpdateActiveGeneration {
        &self.generation
    }
}

impl Drop for UiUpdateGenerationLease {
    fn drop(&mut self) {
        if let Ok(mut leases) = self.leases.lock()
            && let Some(count) = leases.get_mut(&self.generation.generation_id)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                leases.remove(&self.generation.generation_id);
            }
        }
    }
}

/// Runtime-only catalog built from a verified active generation. Documents retain logical IDs;
/// the catalog is the sole authority that turns one into a named Bevy asset-source path.
#[derive(Clone, Debug, Default, Resource)]
pub struct UiDocumentContentCacheAssets {
    generation_id: Option<String>,
    assets: BTreeMap<UiAssetId, UiDocumentContentCacheAsset>,
}

#[derive(Clone, Debug)]
struct UiDocumentContentCacheAsset {
    kind: UiAssetKind,
    path: String,
}

impl UiDocumentContentCacheAssets {
    pub fn activate(&mut self, generation: &UiUpdateActiveGeneration) {
        self.generation_id = Some(generation.generation_id.clone());
        self.assets = generation
            .bundle
            .assets
            .iter()
            .filter_map(|entry| {
                UiAssetId::from_str(&entry.logical_id)
                    .ok()
                    .map(|logical_id| {
                        (
                            logical_id,
                            UiDocumentContentCacheAsset {
                                kind: entry.kind,
                                path: format!(
                                    "content_cache://{}/{}",
                                    generation.generation_id, entry.path
                                ),
                            },
                        )
                    })
            })
            .collect();
    }

    pub fn clear(&mut self) {
        self.generation_id = None;
        self.assets.clear();
    }

    pub fn generation_id(&self) -> Option<&str> {
        self.generation_id.as_deref()
    }

    pub(crate) fn asset_path(&self, logical_id: &str, kind: UiAssetKind) -> Option<&str> {
        let logical_id = UiAssetId::from_str(logical_id).ok()?;
        self.assets
            .get(&logical_id)
            .filter(|entry| entry.kind == kind)
            .map(|entry| entry.path.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UiUpdateCommitRecord {
    format_version: u32,
    sequence: u64,
    generation_id: String,
    manifest_sha256: String,
    #[serde(skip)]
    path: PathBuf,
}

#[derive(Clone, Copy)]
struct UiUpdateFileRequirement<'a> {
    bytes: u64,
    sha256: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiUpdateCacheError {
    code: &'static str,
}

impl UiUpdateCacheError {
    fn new(code: &'static str) -> Self {
        Self { code }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for UiUpdateCacheError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for UiUpdateCacheError {}

fn parse_manifest(source: &[u8]) -> Result<UiUpdateBundle, UiUpdateCacheError> {
    if source.len() > UI_UPDATE_MANIFEST_MAX_BYTES {
        return Err(UiUpdateCacheError::new("UI_UPDATE_MANIFEST_BYTES_EXCEEDED"));
    }
    serde_json::from_slice(source)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_MANIFEST_INVALID"))
}

fn validate_bundle_header(
    bundle: &UiUpdateBundle,
    config: &UiUpdateCacheConfig,
) -> Result<(), UiUpdateCacheError> {
    if bundle.format_version != UI_UPDATE_BUNDLE_FORMAT_VERSION
        || bundle.bundle_id != config.bundle_id
        || bundle.channel != config.channel
        || !safe_label(&bundle.version)
        || !range_contains(bundle.client_compatibility, config.client_revision)
        || !range_contains(bundle.schema_compatibility, CURRENT_SCHEMA_VERSION)
        || bundle.schema_compatibility.minimum < MIN_SUPPORTED_SCHEMA_VERSION
        || bundle.policy_revision != config.policy_revision
        || bundle.documents.is_empty()
        || bundle.documents.len() > UI_UPDATE_MAX_DOCUMENTS
        || bundle.assets.len() > UI_UPDATE_MAX_ASSETS
    {
        return Err(UiUpdateCacheError::new(
            "UI_UPDATE_MANIFEST_COMPATIBILITY_INVALID",
        ));
    }
    Ok(())
}

fn manifest_file_paths(bundle: &UiUpdateBundle) -> Result<BTreeSet<String>, UiUpdateCacheError> {
    let mut paths = BTreeSet::new();
    for document in &bundle.documents {
        if !valid_bundle_path(&document.path, "documents", true)
            || !valid_bundle_path(&document.registration_path, "registrations", true)
            || UiDocumentSourcePath::new(
                UiDocumentSourceRoot::Approved,
                &document.approved_relative_path,
            )
            .is_err()
            || document.bytes == 0
            || document.registration_bytes == 0
            || !is_sha256(&document.sha256)
            || !is_sha256(&document.registration_sha256)
        {
            return Err(UiUpdateCacheError::new(
                "UI_UPDATE_DOCUMENT_MANIFEST_INVALID",
            ));
        }
        if !paths.insert(document.path.clone()) || !paths.insert(document.registration_path.clone())
        {
            return Err(UiUpdateCacheError::new("UI_UPDATE_BUNDLE_PATH_DUPLICATE"));
        }
    }
    for asset in &bundle.assets {
        if !valid_bundle_path(&asset.path, "assets", false)
            || asset.bytes == 0
            || !is_sha256(&asset.sha256)
            || !safe_media_type(&asset.media_type)
        {
            return Err(UiUpdateCacheError::new("UI_UPDATE_ASSET_MANIFEST_INVALID"));
        }
        if !paths.insert(asset.path.clone()) {
            return Err(UiUpdateCacheError::new("UI_UPDATE_BUNDLE_PATH_DUPLICATE"));
        }
    }
    if paths.len() > UI_UPDATE_MAX_FILES {
        return Err(UiUpdateCacheError::new(
            "UI_UPDATE_BUNDLE_FILE_COUNT_EXCEEDED",
        ));
    }
    Ok(paths)
}

fn file_requirement<'a>(
    bundle: &'a UiUpdateBundle,
    path: &str,
) -> Result<UiUpdateFileRequirement<'a>, UiUpdateCacheError> {
    for document in &bundle.documents {
        if document.path == path {
            return Ok(UiUpdateFileRequirement {
                bytes: document.bytes,
                sha256: &document.sha256,
            });
        }
        if document.registration_path == path {
            return Ok(UiUpdateFileRequirement {
                bytes: document.registration_bytes,
                sha256: &document.registration_sha256,
            });
        }
    }
    for asset in &bundle.assets {
        if asset.path == path {
            return Ok(UiUpdateFileRequirement {
                bytes: asset.bytes,
                sha256: &asset.sha256,
            });
        }
    }
    Err(UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_UNKNOWN"))
}

fn validate_asset_entry(asset: &UiUpdateAssetEntry) -> Result<(), UiUpdateCacheError> {
    if !safe_label(&asset.license.license_id)
        || asset.license.attribution.is_empty()
        || asset.license.attribution.len() > 1024
        || !asset.license.redistribution_permitted
        || !media_type_matches(asset.kind, &asset.path, &asset.media_type)
        || matches!(
            asset.kind,
            UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas
        ) && asset.declared_size.is_none()
        || matches!(asset.kind, UiAssetKind::Font | UiAssetKind::Material)
            && asset.declared_size.is_some()
    {
        return Err(UiUpdateCacheError::new("UI_UPDATE_ASSET_METADATA_INVALID"));
    }
    Ok(())
}

fn validation_target_profile() -> UiTargetProfile {
    UiTargetProfile::new(
        1.0,
        1.0,
        UiSafeAreaClass::None,
        UiDocumentInputMode::MouseTouch,
        UiDocumentPlatform::Windows,
    )
    .expect("fixed validation target profile is valid")
}

fn generation_id(bundle: &UiUpdateBundle, manifest: &[u8]) -> Result<String, UiUpdateCacheError> {
    let id = format!(
        "{}-{}-{}-{}",
        bundle.bundle_id,
        bundle.channel,
        bundle.version,
        &sha256_hex(manifest)[..16]
    );
    if safe_generation_id(&id) {
        Ok(id)
    } else {
        Err(UiUpdateCacheError::new(
            "UI_UPDATE_CACHE_GENERATION_ID_INVALID",
        ))
    }
}

fn resolve_cache_root(root: &Path) -> Result<PathBuf, UiUpdateCacheError> {
    let candidate = canonicalized_future_path(root)?;
    let project = canonicalized_future_path(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    if candidate == project || candidate.starts_with(&project) {
        return Err(UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_FORBIDDEN"));
    }
    Ok(candidate)
}

/// Resolves all existing ancestors before creating the cache. This prevents a caller from using
/// a junction/symlink whose lexical spelling appears outside the repository.
fn canonicalized_future_path(path: &Path) -> Result<PathBuf, UiUpdateCacheError> {
    let absolute = normalized_absolute_path(path)?;
    let mut existing = absolute.clone();
    let mut missing = Vec::new();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_INVALID"))?
            .to_owned();
        missing.push(name);
        existing = existing
            .parent()
            .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_INVALID"))?
            .to_owned();
    }
    let mut resolved = fs::canonicalize(existing)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_INVALID"))?;
    for segment in missing.into_iter().rev() {
        resolved.push(segment);
    }
    Ok(resolved)
}

fn normalized_absolute_path(path: &Path) -> Result<PathBuf, UiUpdateCacheError> {
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir().map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_INVALID"))?
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(UiUpdateCacheError::new("UI_UPDATE_CACHE_ROOT_INVALID"));
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn valid_bundle_path(path: &str, root: &str, json: bool) -> bool {
    path.starts_with(&format!("{root}/"))
        && (!json || path.ends_with(".json"))
        && path.len() <= 240
        && path.is_ascii()
        && !path.contains(['\\', ':', '\0', '\n', '\r'])
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-' | b'.')
                })
        })
}

fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

fn safe_generation_id(value: &str) -> bool {
    safe_label(value) && value.len() <= 220
}

fn safe_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'/' | b'+' | b'-' | b'.')
        })
}

fn media_type_matches(kind: UiAssetKind, path: &str, media_type: &str) -> bool {
    let extension = Path::new(path).extension().and_then(|value| value.to_str());
    matches!(
        (kind, extension, media_type),
        (
            UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas,
            Some("png"),
            "image/png"
        ) | (
            UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas,
            Some("jpg" | "jpeg"),
            "image/jpeg"
        ) | (
            UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas,
            Some("webp"),
            "image/webp"
        ) | (UiAssetKind::Font, Some("ttf"), "font/ttf")
            | (UiAssetKind::Font, Some("otf"), "font/otf")
    )
}

fn range_contains(range: UiUpdateRevisionRange, revision: u32) -> bool {
    range.minimum > 0
        && range.minimum <= range.maximum
        && (range.minimum..=range.maximum).contains(&revision)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_limited_file(path: &Path, expected_bytes: u64) -> Result<Vec<u8>, UiUpdateCacheError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_MISSING"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != expected_bytes
    {
        return Err(UiUpdateCacheError::new(
            "UI_UPDATE_BUNDLE_FILE_SIZE_MISMATCH",
        ));
    }
    fs::read(path).map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_READ_FAILED"))
}

fn read_file_up_to_limit(path: &Path, max_bytes: u64) -> Result<Vec<u8>, UiUpdateCacheError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_MISSING"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(UiUpdateCacheError::new("UI_UPDATE_MANIFEST_BYTES_EXCEEDED"));
    }
    fs::read(path).map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_READ_FAILED"))
}

fn stored_payload_paths(directory: &Path) -> Result<BTreeSet<String>, UiUpdateCacheError> {
    let mut paths = BTreeSet::new();
    let mut pending = vec![directory.to_owned()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_READ_FAILED"))?
        {
            let entry =
                entry.map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_READ_FAILED"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_READ_FAILED"))?;
            if file_type.is_symlink() {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_BUNDLE_FILE_SET_MISMATCH",
                ));
            }
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                return Err(UiUpdateCacheError::new(
                    "UI_UPDATE_BUNDLE_FILE_SET_MISMATCH",
                ));
            }
            let relative = path
                .strip_prefix(directory)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_SET_MISMATCH"))?
                .to_str()
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_BUNDLE_FILE_SET_MISMATCH"))?
                .replace('\\', "/");
            if relative != MANIFEST_FILE {
                paths.insert(relative);
            }
        }
    }
    Ok(paths)
}

fn write_staging_file(path: &Path, bytes: &[u8]) -> Result<(), UiUpdateCacheError> {
    let parent = path
        .parent()
        .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_PATH_INVALID"))?;
    fs::create_dir_all(parent)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_CREATE_FAILED"))?;
    let mut file = fs::File::create(path)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_WRITE_FAILED"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_STAGE_WRITE_FAILED"))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), UiUpdateCacheError> {
    let parent = path
        .parent()
        .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_PATH_INVALID"))?;
    fs::create_dir_all(parent)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_WRITE_FAILED"))?;
    let temporary = parent.join(format!(".tmp-{}", unique_suffix()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_COMMIT_WRITE_FAILED"))?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if result.is_err() || fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(UiUpdateCacheError::new(
            "UI_UPDATE_CACHE_COMMIT_WRITE_FAILED",
        ));
    }
    Ok(())
}

fn directory_size(path: &Path) -> Result<u64, UiUpdateCacheError> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0_u64;
    let mut pending = vec![path.to_owned()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_SIZE_READ_FAILED"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| UiUpdateCacheError::new("UI_UPDATE_CACHE_SIZE_OVERFLOW"))?;
            continue;
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)
                .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_SIZE_READ_FAILED"))?
            {
                pending.push(
                    entry
                        .map_err(|_| UiUpdateCacheError::new("UI_UPDATE_CACHE_SIZE_READ_FAILED"))?
                        .path(),
                );
            }
        }
    }
    Ok(total)
}

fn commit_file_name(sequence: u64, generation_id: &str) -> String {
    format!("{sequence:020}-{generation_id}.json")
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(test: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("mybevy-ui-update-{test}-{}", unique_suffix()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn open_cache(root: &Path) -> UiUpdateCache {
        UiUpdateCache::open(UiUpdateCacheConfig::new(root, "game-ui", "stable").unwrap()).unwrap()
    }

    fn hash(bytes: &[u8]) -> String {
        sha256_hex(bytes)
    }

    fn static_document(document_id: &str) -> String {
        format!(
            r#"{{"schema_version":1,"document_id":"{document_id}","root":{{"type":"text","id":"page.title","content":{{"literal":"Ready"}}}}}}"#
        )
    }

    fn registration(document_id: &str, relative: &str) -> String {
        serde_json::json!({
            "protocol_version": 1,
            "kind": "ui_document_promotion_registration",
            "template_version": 1,
            "document_id": document_id,
            "source": { "root": "approved", "relative_path": relative },
            "owner": "update_owner",
            "route": "update_route",
            "panel": "page",
            "layer": "page",
            "page_state": "initial",
            "audit_profiles": ["desktop", "phone-landscape", "phone-1080p-landscape", "tablet-landscape"],
            "i18n_keys": [],
            "theme_tokens": [],
            "action_or_binding_registration": []
        })
        .to_string()
    }

    fn import(version: &str) -> UiUpdateBundleImport {
        let document = static_document("update.page").into_bytes();
        let registration = registration("update.page", "update/page.v1.json").into_bytes();
        let document_path = "documents/update_page.json".to_owned();
        let registration_path = "registrations/update_page.json".to_owned();
        let bundle = UiUpdateBundle {
            format_version: 1,
            bundle_id: "game-ui".to_owned(),
            channel: "stable".to_owned(),
            version: version.to_owned(),
            client_compatibility: UiUpdateRevisionRange {
                minimum: 1,
                maximum: 1,
            },
            schema_compatibility: UiUpdateRevisionRange {
                minimum: 1,
                maximum: 1,
            },
            policy_revision: 1,
            total_bytes: (document.len() + registration.len()) as u64,
            documents: vec![UiUpdateDocumentEntry {
                document_id: "update.page".to_owned(),
                path: document_path.clone(),
                bytes: document.len() as u64,
                sha256: hash(&document),
                registration_path: registration_path.clone(),
                registration_bytes: registration.len() as u64,
                registration_sha256: hash(&registration),
                approved_relative_path: "update/page.v1.json".to_owned(),
                dependencies: Vec::new(),
            }],
            assets: Vec::new(),
        };
        UiUpdateBundleImport {
            manifest_json: serde_json::to_vec(&bundle).unwrap(),
            files: BTreeMap::from([(document_path, document), (registration_path, registration)]),
        }
    }

    #[test]
    fn verified_bundle_activates_without_writing_project_assets() {
        let root = temp_root("activate");
        let cache = open_cache(&root);
        let generation = cache.install(&import("v1")).unwrap();
        assert_eq!(generation.bundle().version, "v1");
        assert!(root.join("generations").is_dir());
        assert!(root.join("active").read_dir().unwrap().next().is_some());
        let cached = generation
            .document(&UiDocumentId::from_str("update.page").unwrap())
            .unwrap()
            .unwrap();
        assert!(
            cached
                .source_path
                .as_str()
                .starts_with("ui-documents/cache/")
        );
        assert!(cached.source_json.contains("update.page"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_hash_missing_file_and_invalid_json_never_activate() {
        let root = temp_root("invalid");
        let cache = open_cache(&root);
        let mut hash_mismatch = import("v1");
        hash_mismatch
            .files
            .get_mut("documents/update_page.json")
            .unwrap()
            .push(b' ');
        assert_eq!(
            cache.stage(&hash_mismatch).unwrap_err().code(),
            "UI_UPDATE_BUNDLE_FILE_HASH_MISMATCH"
        );
        let mut missing = import("v2");
        missing.files.remove("registrations/update_page.json");
        assert_eq!(
            cache.stage(&missing).unwrap_err().code(),
            "UI_UPDATE_BUNDLE_FILE_SET_MISMATCH"
        );
        let mut invalid_json = import("v3");
        let bytes = b"{invalid".to_vec();
        let manifest: UiUpdateBundle = serde_json::from_slice(&invalid_json.manifest_json).unwrap();
        invalid_json
            .files
            .insert("documents/update_page.json".to_owned(), bytes.clone());
        let mut manifest = manifest;
        manifest.documents[0].bytes = bytes.len() as u64;
        manifest.documents[0].sha256 = hash(&bytes);
        manifest.total_bytes =
            manifest.documents[0].bytes + manifest.documents[0].registration_bytes;
        invalid_json.manifest_json = serde_json::to_vec(&manifest).unwrap();
        assert_eq!(
            cache.stage(&invalid_json).unwrap_err().code(),
            "UI_UPDATE_DOCUMENT_INVALID"
        );
        assert!(cache.active_generation().unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn capacity_limit_keeps_the_existing_active_generation() {
        let root = temp_root("capacity");
        let first = import("v1");
        let required_cache = first.manifest_json.len() as u64
            + first
                .files
                .values()
                .map(|value| value.len() as u64)
                .sum::<u64>();
        let bundle: UiUpdateBundle = serde_json::from_slice(&first.manifest_json).unwrap();
        let config = UiUpdateCacheConfig::new(&root, "game-ui", "stable")
            .unwrap()
            .with_policy(UiUpdateCachePolicy {
                max_cache_bytes: required_cache - 1,
                max_bundle_bytes: bundle.total_bytes,
                max_generations: 2,
            })
            .unwrap();
        let cache = UiUpdateCache::open(config).unwrap();
        assert_eq!(
            cache.stage(&first).unwrap_err().code(),
            "UI_UPDATE_CACHE_CAPACITY_EXCEEDED"
        );
        assert!(cache.active_generation().unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_staging_is_quarantined_and_corrupt_active_rolls_back() {
        let root = temp_root("rollback");
        let cache = open_cache(&root);
        let first = cache.install(&import("v1")).unwrap();
        let second = cache.install(&import("v2")).unwrap();
        fs::write(
            second.directory().join("documents/update_page.json"),
            b"corrupt",
        )
        .unwrap();
        fs::create_dir_all(root.join("staging").join("interrupted")).unwrap();
        drop(cache);
        let cache = open_cache(&root);
        assert!(root.join("quarantine").read_dir().unwrap().next().is_some());
        let active = cache.active_generation().unwrap().unwrap();
        assert_eq!(active.generation_id(), first.generation_id());
        assert_eq!(
            cache
                .previous_generation()
                .unwrap()
                .unwrap()
                .generation_id(),
            first.generation_id()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn content_cache_catalog_only_exposes_verified_logical_assets() {
        let root = temp_root("catalog");
        let cache = open_cache(&root);
        let mut import = import("v1");
        let image = vec![0_u8; 4];
        let image_path = "assets/banner.png".to_owned();
        let document = serde_json::json!({
            "schema_version": 1,
            "document_id": "update.page",
            "assets": {
                "banner": {
                    "kind": "image",
                    "source": { "kind": "content_cache", "logical_id": "event.banner" },
                    "declared_size": { "width": 1, "height": 1, "decoded_bytes": 4 }
                }
            },
            "root": { "type": "image", "id": "page.banner", "asset": "banner" }
        })
        .to_string()
        .into_bytes();
        import
            .files
            .insert("documents/update_page.json".to_owned(), document.clone());
        let mut bundle: UiUpdateBundle = serde_json::from_slice(&import.manifest_json).unwrap();
        bundle.documents[0].bytes = document.len() as u64;
        bundle.documents[0].sha256 = hash(&document);
        bundle.documents[0].dependencies = vec!["event.banner".to_owned()];
        bundle.assets = vec![UiUpdateAssetEntry {
            logical_id: "event.banner".to_owned(),
            kind: UiAssetKind::Image,
            path: image_path.clone(),
            bytes: image.len() as u64,
            sha256: hash(&image),
            media_type: "image/png".to_owned(),
            declared_size: Some(UiAssetDeclaredSize {
                width: 1,
                height: 1,
                decoded_bytes: 4,
            }),
            license: UiUpdateAssetLicense {
                license_id: "cc0-1.0".to_owned(),
                attribution: "Test asset".to_owned(),
                redistribution_permitted: true,
            },
        }];
        bundle.total_bytes =
            bundle.documents[0].bytes + bundle.documents[0].registration_bytes + image.len() as u64;
        import.files.insert(image_path, image);
        import.manifest_json = serde_json::to_vec(&bundle).unwrap();
        let generation = cache.install(&import).unwrap();
        let mut catalog = UiDocumentContentCacheAssets::default();
        catalog.activate(&generation);
        assert_eq!(
            catalog.asset_path("event.banner", UiAssetKind::Image),
            Some(
                format!(
                    "content_cache://{}/assets/banner.png",
                    generation.generation_id()
                )
                .as_str()
            )
        );
        assert!(
            catalog
                .asset_path("event.banner", UiAssetKind::Font)
                .is_none()
        );
        assert!(
            catalog
                .asset_path("not.declared", UiAssetKind::Image)
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
