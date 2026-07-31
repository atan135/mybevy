//! Signed, bounded remote delivery for verified UI update bundles.
//!
//! This module deliberately owns only transport policy and cache activation. Downloaded data can
//! never select a new endpoint, trust root, host contract, or Bevy system. `UiUpdateCache` remains
//! the sole authority that validates a complete bundle before it becomes visible to a document.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use bevy::prelude::*;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::framework::{
    network::{HttpRequest, HttpResponse, NetworkCommand, NetworkEvent, RequestId},
    ui::{
        core::input::UiInputState,
        document::{
            UI_UPDATE_CLIENT_REVISION, UI_UPDATE_MANIFEST_MAX_BYTES, UiDocumentContentCacheAssets,
            UiUpdateBundle, UiUpdateBundleImport, UiUpdateCache, UiUpdateCacheConfig,
            UiUpdateStagedBundle,
        },
    },
};

pub const UI_UPDATE_RELEASE_FORMAT_VERSION: u32 = 1;
pub const UI_UPDATE_SIGNATURE_ALGORITHM: &str = "ed25519";
pub const UI_UPDATE_SIGNATURE_ALGORITHM_VERSION: u32 = 1;
pub const UI_UPDATE_MAX_CONCURRENT_DOWNLOADS: usize = 4;
pub const UI_UPDATE_MAX_RETRIES: u8 = 2;

/// The caller selects one of these compile-time policy modes. A remote response cannot change it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiUpdateEnvironment {
    Local,
    Production,
}

/// Trusted endpoint configuration. It is constructed by the game binary, never deserialized from
/// a UI document or update manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiUpdateEndpoint {
    environment: UiUpdateEnvironment,
    base_url: String,
    bundle_id: String,
    channel: String,
}

impl UiUpdateEndpoint {
    pub fn local(
        base_url: impl Into<String>,
        bundle_id: impl Into<String>,
        channel: impl Into<String>,
    ) -> Result<Self, UiUpdateClientError> {
        Self::new(
            UiUpdateEnvironment::Local,
            base_url.into(),
            bundle_id.into(),
            channel.into(),
        )
    }

    pub fn production(
        bundle_id: impl Into<String>,
        channel: impl Into<String>,
    ) -> Result<Self, UiUpdateClientError> {
        // Production is intentionally not environment-variable configurable. Release builds can
        // receive only content signed by keys paired with this endpoint policy.
        Self::new(
            UiUpdateEnvironment::Production,
            "https://api.game.zergzerg.cn/api/v1/ui-updates".to_owned(),
            bundle_id.into(),
            channel.into(),
        )
    }

    fn new(
        environment: UiUpdateEnvironment,
        base_url: String,
        bundle_id: String,
        channel: String,
    ) -> Result<Self, UiUpdateClientError> {
        if !safe_label(&bundle_id)
            || !safe_label(&channel)
            || !trusted_base_url(environment, &base_url)
            || environment == UiUpdateEnvironment::Local
                && !cfg!(all(debug_assertions, not(target_os = "android")))
        {
            return Err(UiUpdateClientError::new("UI_UPDATE_ENDPOINT_INVALID"));
        }
        Ok(Self {
            environment,
            base_url: base_url.trim_end_matches('/').to_owned(),
            bundle_id,
            channel,
        })
    }

    pub const fn environment(&self) -> UiUpdateEnvironment {
        self.environment
    }

    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }
}

/// Request construction is injectable for tests, while the production implementation still uses
/// the existing Bevy `NetworkCommand::Http` interface.
pub trait UiUpdateProvider: Send + Sync + 'static {
    fn endpoint(&self) -> &UiUpdateEndpoint;

    fn manifest_request(&self, etag: Option<&str>) -> HttpRequest;
    fn file_request(
        &self,
        bundle: &UiUpdateBundle,
        path: &str,
        expected_bytes: u64,
        offset: u64,
    ) -> Result<HttpRequest, UiUpdateClientError>;
}

#[derive(Clone, Debug)]
pub struct UiHttpUpdateProvider {
    endpoint: UiUpdateEndpoint,
    timeout: Duration,
}

impl UiHttpUpdateProvider {
    pub fn new(endpoint: UiUpdateEndpoint) -> Self {
        Self {
            endpoint,
            timeout: Duration::from_secs(10),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout.max(Duration::from_millis(100));
        self
    }

    pub fn endpoint(&self) -> &UiUpdateEndpoint {
        &self.endpoint
    }
}

impl UiUpdateProvider for UiHttpUpdateProvider {
    fn endpoint(&self) -> &UiUpdateEndpoint {
        &self.endpoint
    }

    fn manifest_request(&self, etag: Option<&str>) -> HttpRequest {
        let mut request = HttpRequest::get(format!(
            "{}/manifests/{}/{}.json",
            self.endpoint.base_url, self.endpoint.bundle_id, self.endpoint.channel
        ))
        .with_header("Accept", "application/json")
        .with_timeout(self.timeout)
        .with_max_response_bytes(UI_UPDATE_MANIFEST_MAX_BYTES);
        if let Some(etag) = etag.filter(|value| valid_etag(value)) {
            request = request.with_header("If-None-Match", etag);
        }
        request
    }

    fn file_request(
        &self,
        bundle: &UiUpdateBundle,
        path: &str,
        expected_bytes: u64,
        offset: u64,
    ) -> Result<HttpRequest, UiUpdateClientError> {
        if bundle.bundle_id != self.endpoint.bundle_id
            || bundle.channel != self.endpoint.channel
            || !safe_bundle_path(path)
            || offset >= expected_bytes
        {
            return Err(UiUpdateClientError::new("UI_UPDATE_FILE_REQUEST_INVALID"));
        }
        let remaining = expected_bytes - offset;
        let max_response_bytes = usize::try_from(remaining)
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_FILE_SIZE_UNSUPPORTED"))?;
        let mut request = HttpRequest::get(format!(
            "{}/bundles/{}/{}/{}",
            self.endpoint.base_url, bundle.bundle_id, bundle.version, path
        ))
        .with_header("Accept", "application/octet-stream")
        .with_timeout(self.timeout)
        .with_max_response_bytes(max_response_bytes);
        if offset > 0 {
            request = request.with_header("Range", format!("bytes={offset}-"));
        }
        Ok(request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiUpdateAvailability {
    Optional,
    Required,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateReleasePolicy {
    pub availability: UiUpdateAvailability,
    /// A downgrade must be explicitly signed and explained by the release authority. The client
    /// otherwise rejects any semantic version below the active remote generation.
    #[serde(default)]
    pub downgrade_authorized: bool,
}

impl Default for UiUpdateReleasePolicy {
    fn default() -> Self {
        Self {
            availability: UiUpdateAvailability::Optional,
            downgrade_authorized: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateRelease {
    pub format_version: u32,
    pub bundle: UiUpdateBundle,
    #[serde(default)]
    pub policy: UiUpdateReleasePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUpdateSignature {
    pub algorithm: String,
    pub algorithm_version: u32,
    pub key_id: String,
    /// Lowercase hex, so tools do not need a permissive binary encoding layer.
    pub signature_hex: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSignedUpdateManifest {
    pub release: UiUpdateRelease,
    pub signature: UiUpdateSignature,
}

impl UiSignedUpdateManifest {
    pub fn sign(
        release: UiUpdateRelease,
        key_id: impl Into<String>,
        key: &SigningKey,
    ) -> Result<Self, UiUpdateClientError> {
        let key_id = key_id.into();
        if !safe_label(&key_id) {
            return Err(UiUpdateClientError::new(
                "UI_UPDATE_SIGNATURE_KEY_ID_INVALID",
            ));
        }
        validate_release_shape(&release)?;
        let payload = canonical_release_bytes(&release)?;
        let signature = key.sign(&payload).to_bytes();
        Ok(Self {
            release,
            signature: UiUpdateSignature {
                algorithm: UI_UPDATE_SIGNATURE_ALGORITHM.to_owned(),
                algorithm_version: UI_UPDATE_SIGNATURE_ALGORITHM_VERSION,
                key_id,
                signature_hex: hex_encode(&signature),
            },
        })
    }

    pub fn verify(&self, trust: &UiUpdateTrustStore) -> Result<(), UiUpdateClientError> {
        validate_release_shape(&self.release)?;
        if self.signature.algorithm != UI_UPDATE_SIGNATURE_ALGORITHM
            || self.signature.algorithm_version != UI_UPDATE_SIGNATURE_ALGORITHM_VERSION
            || !safe_label(&self.signature.key_id)
        {
            return Err(UiUpdateClientError::new(
                "UI_UPDATE_SIGNATURE_ALGORITHM_INVALID",
            ));
        }
        let key = trust.key(&self.signature.key_id)?;
        let bytes: [u8; 64] = hex_decode(&self.signature.signature_hex)
            .ok_or_else(|| UiUpdateClientError::new("UI_UPDATE_SIGNATURE_ENCODING_INVALID"))?;
        let payload = canonical_release_bytes(&self.release)?;
        key.verify_strict(&payload, &Signature::from_bytes(&bytes))
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_SIGNATURE_INVALID"))
    }

    pub fn canonical_release_sha256(&self) -> Result<String, UiUpdateClientError> {
        Ok(sha256_hex(&canonical_release_bytes(&self.release)?))
    }
}

/// The application carries current and retired signing keys. A response may name only a live key;
/// this supports rotation without trusting a key delivered by the response itself.
#[derive(Clone, Debug, Default)]
pub struct UiUpdateTrustStore {
    keys: BTreeMap<String, VerifyingKey>,
    revoked: BTreeSet<String>,
}

impl UiUpdateTrustStore {
    pub fn insert_key(
        &mut self,
        key_id: impl Into<String>,
        public_key: [u8; 32],
    ) -> Result<(), UiUpdateClientError> {
        let key_id = key_id.into();
        if !safe_label(&key_id) || self.revoked.contains(&key_id) {
            return Err(UiUpdateClientError::new("UI_UPDATE_TRUST_KEY_INVALID"));
        }
        let key = VerifyingKey::from_bytes(&public_key)
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_TRUST_KEY_INVALID"))?;
        self.keys.insert(key_id, key);
        Ok(())
    }

    pub fn revoke_key(&mut self, key_id: impl Into<String>) {
        let key_id = key_id.into();
        self.keys.remove(&key_id);
        self.revoked.insert(key_id);
    }

    fn key(&self, key_id: &str) -> Result<&VerifyingKey, UiUpdateClientError> {
        if self.revoked.contains(key_id) {
            return Err(UiUpdateClientError::new("UI_UPDATE_SIGNATURE_KEY_REVOKED"));
        }
        self.keys
            .get(key_id)
            .ok_or_else(|| UiUpdateClientError::new("UI_UPDATE_SIGNATURE_KEY_UNKNOWN"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiUpdateClientPolicy {
    pub max_concurrent_downloads: usize,
    pub max_retries: u8,
    pub retry_backoff: Duration,
}

impl Default for UiUpdateClientPolicy {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: UI_UPDATE_MAX_CONCURRENT_DOWNLOADS,
            max_retries: UI_UPDATE_MAX_RETRIES,
            retry_backoff: Duration::from_millis(500),
        }
    }
}

impl UiUpdateClientPolicy {
    fn validate(self) -> Result<Self, UiUpdateClientError> {
        if self.max_concurrent_downloads == 0
            || self.max_concurrent_downloads > UI_UPDATE_MAX_CONCURRENT_DOWNLOADS
        {
            return Err(UiUpdateClientError::new("UI_UPDATE_CLIENT_POLICY_INVALID"));
        }
        Ok(self)
    }
}

#[derive(Resource)]
pub struct UiUpdateClient {
    cache: UiUpdateCache,
    provider: Arc<dyn UiUpdateProvider>,
    trust: UiUpdateTrustStore,
    policy: UiUpdateClientPolicy,
    etag: Option<String>,
    pending: HashMap<RequestId, UiUpdatePendingRequest>,
    retries: Vec<UiUpdateRetry>,
    session: Option<UiUpdateDownloadSession>,
    staged: Option<UiUpdateStagedBundle>,
    telemetry: VecDeque<UiUpdateTelemetry>,
}

impl UiUpdateClient {
    pub fn open<P: UiUpdateProvider>(
        cache_config: UiUpdateCacheConfig,
        provider: P,
        trust: UiUpdateTrustStore,
        policy: UiUpdateClientPolicy,
    ) -> Result<Self, UiUpdateClientError> {
        Ok(Self {
            cache: UiUpdateCache::open(cache_config)
                .map_err(|_| UiUpdateClientError::new("UI_UPDATE_CACHE_OPEN_FAILED"))?,
            provider: Arc::new(provider),
            trust,
            policy: policy.validate()?,
            etag: None,
            pending: HashMap::new(),
            retries: Vec::new(),
            session: None,
            staged: None,
            telemetry: VecDeque::new(),
        })
    }

    /// Must be called before Bevy creates `AssetPlugin`; it only registers the verified cache
    /// reader and never registers a remote or repository path as an asset source.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_asset_source(&self, app: &mut App) {
        self.cache.register_asset_source(app);
    }

    pub fn cache(&self) -> &UiUpdateCache {
        &self.cache
    }

    pub fn telemetry(&self) -> impl Iterator<Item = &UiUpdateTelemetry> {
        self.telemetry.iter()
    }

    pub fn check_now(&mut self) -> Vec<NetworkCommand> {
        if !self.pending.is_empty() || self.session.is_some() || self.staged.is_some() {
            return Vec::new();
        }
        self.start(UiUpdatePendingKind::Manifest, 0)
    }

    pub fn cancel(&mut self) -> Vec<NetworkCommand> {
        let commands = self
            .pending
            .keys()
            .copied()
            .map(|request_id| NetworkCommand::CancelHttp { request_id })
            .collect();
        self.pending.clear();
        self.retries.clear();
        self.session = None;
        self.staged = None;
        self.record(UiUpdateTelemetryKind::Canceled, None, None, 0);
        commands
    }

    pub fn handle_network_event(
        &mut self,
        event: &NetworkEvent,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        match event {
            NetworkEvent::HttpResponse(response) => self.handle_response(response),
            NetworkEvent::HttpError { request_id, .. } => self.handle_failure(*request_id),
            _ => (Vec::new(), Vec::new()),
        }
    }

    pub fn poll_retries(&mut self) -> Vec<NetworkCommand> {
        let now = Instant::now();
        let mut ready = Vec::new();
        self.retries.retain(|retry| {
            if retry.at <= now {
                ready.push((retry.kind.clone(), retry.attempt));
                false
            } else {
                true
            }
        });
        ready
            .into_iter()
            .flat_map(|(kind, attempt)| self.start(kind, attempt))
            .collect()
    }

    fn handle_response(
        &mut self,
        response: &HttpResponse,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        let Some(pending) = self.pending.remove(&response.request_id) else {
            return (Vec::new(), Vec::new());
        };
        match pending.kind {
            UiUpdatePendingKind::Manifest if response.status == 304 => {
                self.record(
                    UiUpdateTelemetryKind::NoUpdate,
                    Some(304),
                    None,
                    pending.attempt,
                );
                (Vec::new(), vec![UiUpdateClientEvent::NoUpdate])
            }
            UiUpdatePendingKind::Manifest if response.status == 200 => {
                self.handle_manifest(response, pending.attempt)
            }
            UiUpdatePendingKind::File { path }
                if response.status == 200 || response.status == 206 =>
            {
                self.handle_file(response, path, pending.attempt)
            }
            _ => self.retry_or_fail(pending, Some(response.status)),
        }
    }

    fn handle_manifest(
        &mut self,
        response: &HttpResponse,
        attempt: u8,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        let manifest = match serde_json::from_slice::<UiSignedUpdateManifest>(&response.body) {
            Ok(manifest) => manifest,
            Err(_) => {
                return self.fail(
                    "UI_UPDATE_REMOTE_MANIFEST_INVALID",
                    Some(response.status),
                    attempt,
                );
            }
        };
        if let Err(error) = manifest.verify(&self.trust) {
            return self.fail(error.code(), Some(response.status), attempt);
        }
        if let Err(error) = self.validate_remote_release(&manifest.release) {
            return self.fail(error.code(), Some(response.status), attempt);
        }
        self.etag = response_header(&response.headers, "etag");
        let mut session = match UiUpdateDownloadSession::new(manifest.release, self.cache.root()) {
            Ok(session) => session,
            Err(error) => return self.fail(error.code(), Some(response.status), attempt),
        };
        if let Err(error) = session.restore_completed_files() {
            return self.fail(error.code(), Some(response.status), attempt);
        }
        let version = session.release.bundle.version.clone();
        let required = session.release.policy.availability == UiUpdateAvailability::Required;
        self.session = Some(session);
        self.record(
            UiUpdateTelemetryKind::ManifestAccepted,
            Some(response.status),
            None,
            attempt,
        );
        let (commands, mut events) = self.pump_downloads();
        events.insert(0, UiUpdateClientEvent::UpdateReady { version, required });
        (commands, events)
    }

    fn handle_file(
        &mut self,
        response: &HttpResponse,
        path: String,
        attempt: u8,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        let Some(session) = self.session.as_mut() else {
            return self.fail(
                "UI_UPDATE_DOWNLOAD_SESSION_MISSING",
                Some(response.status),
                attempt,
            );
        };
        let result = session.accept_file_response(&path, response.status, &response.body);
        if let Err(error) = result {
            return self.fail(error.code(), Some(response.status), attempt);
        }
        self.record(
            UiUpdateTelemetryKind::FileDownloaded,
            Some(response.status),
            None,
            attempt,
        );
        self.pump_downloads()
    }

    fn pump_downloads(&mut self) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        let Some(session) = self.session.as_mut() else {
            return (Vec::new(), Vec::new());
        };
        let mut commands = Vec::new();
        while self.pending.len() < self.policy.max_concurrent_downloads {
            let Some(file) = session.next_file() else {
                break;
            };
            let offset = match session.partial_len(&file.path) {
                Ok(offset) => offset,
                Err(error) => return self.fail(error.code(), None, 0),
            };
            let request = match self.provider.file_request(
                &session.release.bundle,
                &file.path,
                file.bytes,
                offset,
            ) {
                Ok(request) => request,
                Err(error) => return self.fail(error.code(), None, 0),
            };
            let request_id = request.request_id;
            self.pending.insert(
                request_id,
                UiUpdatePendingRequest {
                    kind: UiUpdatePendingKind::File { path: file.path },
                    attempt: 0,
                },
            );
            commands.push(NetworkCommand::Http(request));
        }
        if self.pending.is_empty() && session.complete() {
            let import = match session.import() {
                Ok(import) => import,
                Err(error) => return self.fail(error.code(), None, 0),
            };
            match self.cache.stage(&import) {
                Ok(staged) => {
                    let _ = session.cleanup();
                    self.staged = Some(staged);
                    self.session = None;
                    self.record(UiUpdateTelemetryKind::BundleVerified, None, None, 0);
                }
                Err(_) => return self.fail("UI_UPDATE_BUNDLE_VERIFY_FAILED", None, 0),
            }
        }
        (commands, Vec::new())
    }

    fn start(&mut self, kind: UiUpdatePendingKind, attempt: u8) -> Vec<NetworkCommand> {
        let request = match &kind {
            UiUpdatePendingKind::Manifest => self.provider.manifest_request(self.etag.as_deref()),
            UiUpdatePendingKind::File { path } => {
                let Some(session) = self.session.as_ref() else {
                    return Vec::new();
                };
                let Some(file) = session.requirement(path) else {
                    return Vec::new();
                };
                let Ok(offset) = session.partial_len(path) else {
                    return Vec::new();
                };
                match self
                    .provider
                    .file_request(&session.release.bundle, path, file.bytes, offset)
                {
                    Ok(request) => request,
                    Err(_) => return Vec::new(),
                }
            }
        };
        let request_id = request.request_id;
        self.pending
            .insert(request_id, UiUpdatePendingRequest { kind, attempt });
        vec![NetworkCommand::Http(request)]
    }

    fn handle_failure(
        &mut self,
        request_id: RequestId,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        let Some(pending) = self.pending.remove(&request_id) else {
            return (Vec::new(), Vec::new());
        };
        self.retry_or_fail(pending, None)
    }

    fn retry_or_fail(
        &mut self,
        pending: UiUpdatePendingRequest,
        status: Option<u16>,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        if pending.attempt < self.policy.max_retries {
            let attempt = pending.attempt.saturating_add(1);
            self.retries.push(UiUpdateRetry {
                at: Instant::now() + self.policy.retry_backoff,
                kind: pending.kind,
                attempt,
            });
            self.record(UiUpdateTelemetryKind::RetryScheduled, status, None, attempt);
            return (Vec::new(), Vec::new());
        }
        self.fail(
            "UI_UPDATE_NETWORK_RETRIES_EXHAUSTED",
            status,
            pending.attempt,
        )
    }

    fn fail(
        &mut self,
        code: &'static str,
        status: Option<u16>,
        attempt: u8,
    ) -> (Vec<NetworkCommand>, Vec<UiUpdateClientEvent>) {
        self.pending.clear();
        self.retries.clear();
        self.session = None;
        self.staged = None;
        self.record(UiUpdateTelemetryKind::Failed, status, Some(code), attempt);
        (Vec::new(), vec![UiUpdateClientEvent::Failed { code }])
    }

    fn validate_remote_release(
        &self,
        release: &UiUpdateRelease,
    ) -> Result<(), UiUpdateClientError> {
        validate_release_shape(release)?;
        let endpoint = self.provider.endpoint();
        if release.bundle.bundle_id != endpoint.bundle_id
            || release.bundle.channel != endpoint.channel
        {
            return Err(UiUpdateClientError::new(
                "UI_UPDATE_RELEASE_ENDPOINT_MISMATCH",
            ));
        }
        let next = parse_release_version(&release.bundle.version)
            .ok_or_else(|| UiUpdateClientError::new("UI_UPDATE_RELEASE_VERSION_INVALID"))?;
        if release.bundle.client_compatibility.minimum > UI_UPDATE_CLIENT_REVISION
            || release.bundle.client_compatibility.maximum < UI_UPDATE_CLIENT_REVISION
        {
            return Err(UiUpdateClientError::new(
                "UI_UPDATE_RELEASE_CLIENT_INCOMPATIBLE",
            ));
        }
        if let Some(active) = self
            .cache
            .active_generation()
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_CACHE_ACTIVE_READ_FAILED"))?
            && let Some(active_version) = parse_release_version(&active.bundle().version)
            && next < active_version
            && !release.policy.downgrade_authorized
        {
            return Err(UiUpdateClientError::new(
                "UI_UPDATE_RELEASE_DOWNGRADE_REJECTED",
            ));
        }
        Ok(())
    }

    fn activate_if_safe(
        &mut self,
        safe: bool,
        catalog: &mut UiDocumentContentCacheAssets,
    ) -> Option<UiUpdateClientEvent> {
        let staged = self.staged.take()?;
        if !safe {
            self.staged = Some(staged);
            self.record(UiUpdateTelemetryKind::ActivationDeferred, None, None, 0);
            return Some(UiUpdateClientEvent::ActivationDeferred);
        }
        match self.cache.activate(staged) {
            Ok(generation) => {
                let version = generation.bundle().version.clone();
                catalog.activate(&generation);
                self.record(UiUpdateTelemetryKind::Activated, None, None, 0);
                Some(UiUpdateClientEvent::Activated { version })
            }
            Err(_) => {
                self.record(
                    UiUpdateTelemetryKind::Failed,
                    None,
                    Some("UI_UPDATE_ACTIVATION_FAILED"),
                    0,
                );
                Some(UiUpdateClientEvent::Failed {
                    code: "UI_UPDATE_ACTIVATION_FAILED",
                })
            }
        }
    }

    fn record(
        &mut self,
        kind: UiUpdateTelemetryKind,
        status: Option<u16>,
        code: Option<&'static str>,
        attempt: u8,
    ) {
        self.telemetry.push_back(UiUpdateTelemetry {
            kind,
            status,
            code,
            attempt,
        });
        while self.telemetry.len() > 64 {
            self.telemetry.pop_front();
        }
    }
}

#[derive(Clone, Debug, Message)]
pub enum UiUpdateClientCommand {
    CheckNow,
    Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq, Message)]
pub enum UiUpdateClientEvent {
    NoUpdate,
    UpdateReady { version: String, required: bool },
    ActivationDeferred,
    Activated { version: String },
    Failed { code: &'static str },
}

/// Game hosts raise this while a non-UI critical request is in flight. Text focus and blocking
/// modal state are read from the existing UI framework; all three conditions defer activation.
#[derive(Clone, Copy, Debug, Default, Resource)]
pub struct UiUpdateActivationGate {
    pub critical_request_in_flight: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiUpdateTelemetryKind {
    ManifestAccepted,
    NoUpdate,
    FileDownloaded,
    BundleVerified,
    RetryScheduled,
    ActivationDeferred,
    Activated,
    Failed,
    Canceled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiUpdateTelemetry {
    pub kind: UiUpdateTelemetryKind,
    pub status: Option<u16>,
    pub code: Option<&'static str>,
    pub attempt: u8,
}

pub struct UiUpdateClientPlugin;

impl Plugin for UiUpdateClientPlugin {
    fn build(&self, app: &mut App) {
        // UI framework tests can install this plugin without `NetworkPlugin`; creating the
        // shared message resources is idempotent when the real network plugin is present.
        app.add_message::<NetworkCommand>()
            .add_message::<NetworkEvent>()
            .add_message::<UiUpdateClientCommand>()
            .add_message::<UiUpdateClientEvent>()
            .init_resource::<UiUpdateActivationGate>()
            .init_resource::<UiDocumentContentCacheAssets>()
            .add_systems(Update, drive_ui_update_client);
    }
}

fn drive_ui_update_client(
    client: Option<ResMut<UiUpdateClient>>,
    input_state: Option<Res<UiInputState>>,
    gate: Res<UiUpdateActivationGate>,
    mut catalog: ResMut<UiDocumentContentCacheAssets>,
    mut commands: MessageReader<UiUpdateClientCommand>,
    mut network_events: MessageReader<NetworkEvent>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut client_events: MessageWriter<UiUpdateClientEvent>,
) {
    let Some(mut client) = client else {
        return;
    };
    for command in commands.read() {
        let requests = match command {
            UiUpdateClientCommand::CheckNow => client.check_now(),
            UiUpdateClientCommand::Cancel => client.cancel(),
        };
        for request in requests {
            network_commands.write(request);
        }
    }
    for event in network_events.read() {
        let (requests, events) = client.handle_network_event(event);
        for request in requests {
            network_commands.write(request);
        }
        for event in events {
            client_events.write(event);
        }
    }
    for request in client.poll_retries() {
        network_commands.write(request);
    }
    let safe =
        !gate.critical_request_in_flight && input_state.is_none_or(|input| !input.pointer_blocked);
    if let Some(event) = client.activate_if_safe(safe, &mut catalog) {
        client_events.write(event);
    }
}

#[derive(Clone, Debug)]
struct UiUpdatePendingRequest {
    kind: UiUpdatePendingKind,
    attempt: u8,
}

#[derive(Clone, Debug)]
enum UiUpdatePendingKind {
    Manifest,
    File { path: String },
}

#[derive(Clone, Debug)]
struct UiUpdateRetry {
    at: Instant,
    kind: UiUpdatePendingKind,
    attempt: u8,
}

#[derive(Clone, Debug)]
struct UiUpdateFileRequirement {
    path: String,
    bytes: u64,
    sha256: String,
}

struct UiUpdateDownloadSession {
    release: UiUpdateRelease,
    requirements: BTreeMap<String, UiUpdateFileRequirement>,
    remaining: VecDeque<String>,
    files: BTreeMap<String, Vec<u8>>,
    root: PathBuf,
}

impl UiUpdateDownloadSession {
    fn new(release: UiUpdateRelease, cache_root: &Path) -> Result<Self, UiUpdateClientError> {
        let requirements = file_requirements(&release.bundle)?;
        let fingerprint = sha256_hex(&canonical_release_bytes(&release)?);
        let root = cache_root.join("downloads").join(&fingerprint[..24]);
        fs::create_dir_all(&root)
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_STORE_CREATE_FAILED"))?;
        let remaining = requirements.keys().cloned().collect();
        Ok(Self {
            release,
            requirements,
            remaining,
            files: BTreeMap::new(),
            root,
        })
    }

    fn restore_completed_files(&mut self) -> Result<(), UiUpdateClientError> {
        let restored = self
            .requirements
            .values()
            .filter_map(|requirement| {
                self.read_complete(requirement)
                    .transpose()
                    .map(|value| value.map(|bytes| (requirement.path.clone(), bytes)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (path, bytes) in restored {
            self.files.insert(path.clone(), bytes);
            self.remaining.retain(|candidate| candidate != &path);
        }
        Ok(())
    }

    fn next_file(&mut self) -> Option<UiUpdateFileRequirement> {
        while let Some(path) = self.remaining.pop_front() {
            if !self.files.contains_key(&path) {
                return self.requirement(&path).cloned();
            }
        }
        None
    }

    fn requirement(&self, path: &str) -> Option<&UiUpdateFileRequirement> {
        self.requirements.get(path)
    }

    fn partial_len(&self, path: &str) -> Result<u64, UiUpdateClientError> {
        let expected = self
            .requirement(path)
            .ok_or_else(|| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_FILE_UNKNOWN"))?
            .bytes;
        let temporary = self.temporary_path(path)?;
        let length = fs::metadata(temporary)
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        if length > expected {
            return Err(UiUpdateClientError::new(
                "UI_UPDATE_DOWNLOAD_PARTIAL_OVERSIZE",
            ));
        }
        Ok(length)
    }

    fn accept_file_response(
        &mut self,
        path: &str,
        status: u16,
        bytes: &[u8],
    ) -> Result<(), UiUpdateClientError> {
        let requirement = self
            .requirement(path)
            .cloned()
            .ok_or_else(|| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_FILE_UNKNOWN"))?;
        let temporary = self.temporary_path(path)?;
        let previous_len = fs::metadata(&temporary)
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let append = status == 206 && previous_len > 0;
        let next_len = if append {
            previous_len.saturating_add(bytes.len() as u64)
        } else {
            bytes.len() as u64
        };
        if next_len > requirement.bytes {
            return Err(UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_BODY_OVERSIZE"));
        }
        let mut options = fs::OpenOptions::new();
        options.create(true).write(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_WRITE_FAILED"))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_WRITE_FAILED"))?;
        if next_len == requirement.bytes {
            let bytes = fs::read(&temporary)
                .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_READ_FAILED"))?;
            if sha256_hex(&bytes) != requirement.sha256 {
                return Err(UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_HASH_MISMATCH"));
            }
            self.files.insert(path.to_owned(), bytes);
        } else {
            self.remaining.push_back(path.to_owned());
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.files.len() == self.requirements.len()
    }

    fn import(&self) -> Result<UiUpdateBundleImport, UiUpdateClientError> {
        if !self.complete() {
            return Err(UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_INCOMPLETE"));
        }
        Ok(UiUpdateBundleImport {
            manifest_json: serde_json::to_vec(&self.release.bundle)
                .map_err(|_| UiUpdateClientError::new("UI_UPDATE_BUNDLE_SERIALIZE_FAILED"))?,
            files: self.files.clone(),
        })
    }

    fn cleanup(&self) -> Result<(), UiUpdateClientError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)
                .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_CLEANUP_FAILED"))?;
        }
        Ok(())
    }

    fn read_complete(
        &self,
        requirement: &UiUpdateFileRequirement,
    ) -> Result<Option<Vec<u8>>, UiUpdateClientError> {
        let temporary = self.temporary_path(&requirement.path)?;
        let Ok(metadata) = fs::metadata(&temporary) else {
            return Ok(None);
        };
        if metadata.len() != requirement.bytes {
            return Ok(None);
        }
        let bytes = fs::read(temporary)
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_READ_FAILED"))?;
        if sha256_hex(&bytes) != requirement.sha256 {
            return Ok(None);
        }
        Ok(Some(bytes))
    }

    fn temporary_path(&self, path: &str) -> Result<PathBuf, UiUpdateClientError> {
        if !safe_bundle_path(path) {
            return Err(UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_PATH_INVALID"));
        }
        let target = self.root.join(path).with_extension("part");
        let parent = target
            .parent()
            .ok_or_else(|| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_PATH_INVALID"))?;
        fs::create_dir_all(parent)
            .map_err(|_| UiUpdateClientError::new("UI_UPDATE_DOWNLOAD_STORE_CREATE_FAILED"))?;
        Ok(target)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiUpdateClientError {
    code: &'static str,
}

impl UiUpdateClientError {
    const fn new(code: &'static str) -> Self {
        Self { code }
    }

    pub const fn code(self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for UiUpdateClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for UiUpdateClientError {}

fn validate_release_shape(release: &UiUpdateRelease) -> Result<(), UiUpdateClientError> {
    if release.format_version != UI_UPDATE_RELEASE_FORMAT_VERSION
        || !safe_label(&release.bundle.bundle_id)
        || !safe_label(&release.bundle.channel)
        || parse_release_version(&release.bundle.version).is_none()
    {
        return Err(UiUpdateClientError::new("UI_UPDATE_RELEASE_INVALID"));
    }
    file_requirements(&release.bundle).map(|_| ())
}

fn file_requirements(
    bundle: &UiUpdateBundle,
) -> Result<BTreeMap<String, UiUpdateFileRequirement>, UiUpdateClientError> {
    let mut requirements = BTreeMap::new();
    for document in &bundle.documents {
        insert_requirement(
            &mut requirements,
            &document.path,
            document.bytes,
            &document.sha256,
        )?;
        insert_requirement(
            &mut requirements,
            &document.registration_path,
            document.registration_bytes,
            &document.registration_sha256,
        )?;
    }
    for asset in &bundle.assets {
        insert_requirement(&mut requirements, &asset.path, asset.bytes, &asset.sha256)?;
    }
    if requirements.is_empty() {
        return Err(UiUpdateClientError::new("UI_UPDATE_RELEASE_FILES_EMPTY"));
    }
    Ok(requirements)
}

fn insert_requirement(
    requirements: &mut BTreeMap<String, UiUpdateFileRequirement>,
    path: &str,
    bytes: u64,
    sha256: &str,
) -> Result<(), UiUpdateClientError> {
    if !safe_bundle_path(path) || bytes == 0 || !is_sha256(sha256) {
        return Err(UiUpdateClientError::new("UI_UPDATE_RELEASE_FILE_INVALID"));
    }
    if requirements
        .insert(
            path.to_owned(),
            UiUpdateFileRequirement {
                path: path.to_owned(),
                bytes,
                sha256: sha256.to_owned(),
            },
        )
        .is_some()
    {
        return Err(UiUpdateClientError::new("UI_UPDATE_RELEASE_FILE_DUPLICATE"));
    }
    Ok(())
}

fn canonical_release_bytes(release: &UiUpdateRelease) -> Result<Vec<u8>, UiUpdateClientError> {
    let mut value = serde_json::to_value(release)
        .map_err(|_| UiUpdateClientError::new("UI_UPDATE_RELEASE_CANONICALIZE_FAILED"))?;
    sort_json_objects(&mut value);
    serde_json::to_vec(&value)
        .map_err(|_| UiUpdateClientError::new("UI_UPDATE_RELEASE_CANONICALIZE_FAILED"))
}

fn sort_json_objects(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut entries {
                sort_json_objects(child);
            }
            object.extend(entries);
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json_objects),
        _ => {}
    }
}

fn parse_release_version(value: &str) -> Option<Vec<u32>> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || !parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 10
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (*part == "0" || !part.starts_with('0'))
        })
    {
        return None;
    }
    parts.iter().map(|part| part.parse::<u32>().ok()).collect()
}

fn trusted_base_url(environment: UiUpdateEnvironment, value: &str) -> bool {
    if value.len() > 240 || value.contains(['?', '#', '@', '\\', '\0', '\n', '\r']) {
        return false;
    }
    match environment {
        UiUpdateEnvironment::Production => value.starts_with("https://api.game.zergzerg.cn/"),
        UiUpdateEnvironment::Local => {
            value.starts_with("http://127.0.0.1:") || value.starts_with("http://localhost:")
        }
    }
}

fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

fn safe_bundle_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && !value.starts_with('/')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-' | b'.')
                })
        })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_etag(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.contains(['\r', '\n'])
}

fn response_header(headers: &[(String, String)], expected: &str) -> Option<String> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(expected))
        .map(|(_, value)| value.clone())
        .filter(|value| valid_etag(value))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn hex_decode<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut result = [0_u8; N];
    for (index, target) in result.iter_mut().enumerate() {
        let byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
        *target = byte;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::document::{
        UiUpdateAssetEntry, UiUpdateCachePolicy, UiUpdateDocumentEntry, UiUpdateRevisionRange,
    };
    use std::{env, time::SystemTime};

    fn temp_root(test: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("mybevy-ui-remote-{test}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn fixture_bundle() -> (UiUpdateBundle, BTreeMap<String, Vec<u8>>) {
        let document = br#"{"schema_version":1,"document_id":"update.page","root":{"type":"text","id":"page.title","content":{"literal":"Ready"}}}"#.to_vec();
        let registration = serde_json::json!({
            "protocol_version": 1,
            "kind": "ui_document_promotion_registration",
            "template_version": 1,
            "document_id": "update.page",
            "source": { "root": "approved", "relative_path": "update/page.v1.json" },
            "owner": "update_owner", "route": "update_route", "panel": "page", "layer": "page",
            "page_state": "initial", "audit_profiles": ["desktop", "phone-landscape", "phone-1080p-landscape", "tablet-landscape"], "i18n_keys": [],
            "theme_tokens": [], "action_or_binding_registration": []
        })
        .to_string()
        .into_bytes();
        let document_path = "documents/update_page.json".to_owned();
        let registration_path = "registrations/update_page.json".to_owned();
        let bundle = UiUpdateBundle {
            format_version: 1,
            bundle_id: "game-ui".to_owned(),
            channel: "stable".to_owned(),
            version: "1.0.0".to_owned(),
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
                sha256: sha256_hex(&document),
                registration_path: registration_path.clone(),
                registration_bytes: registration.len() as u64,
                registration_sha256: sha256_hex(&registration),
                approved_relative_path: "update/page.v1.json".to_owned(),
                dependencies: Vec::new(),
            }],
            assets: Vec::<UiUpdateAssetEntry>::new(),
        };
        (
            bundle,
            BTreeMap::from([(document_path, document), (registration_path, registration)]),
        )
    }

    fn signed_manifest() -> (
        UiSignedUpdateManifest,
        SigningKey,
        BTreeMap<String, Vec<u8>>,
    ) {
        let (bundle, files) = fixture_bundle();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let signed = UiSignedUpdateManifest::sign(
            UiUpdateRelease {
                format_version: 1,
                bundle,
                policy: UiUpdateReleasePolicy::default(),
            },
            "release_2026",
            &key,
        )
        .unwrap();
        (signed, key, files)
    }

    fn client(root: &Path, key: &SigningKey) -> UiUpdateClient {
        let endpoint =
            UiUpdateEndpoint::local("http://127.0.0.1:39001", "game-ui", "stable").unwrap();
        let mut trust = UiUpdateTrustStore::default();
        trust
            .insert_key("release_2026", key.verifying_key().to_bytes())
            .unwrap();
        UiUpdateClient::open(
            UiUpdateCacheConfig::new(root, "game-ui", "stable")
                .unwrap()
                .with_policy(UiUpdateCachePolicy::default())
                .unwrap(),
            UiHttpUpdateProvider::new(endpoint),
            trust,
            UiUpdateClientPolicy {
                retry_backoff: Duration::ZERO,
                ..default()
            },
        )
        .unwrap()
    }

    fn response(request: &NetworkCommand, status: u16, body: Vec<u8>) -> NetworkEvent {
        let NetworkCommand::Http(request) = request else {
            panic!("expected HTTP request")
        };
        NetworkEvent::HttpResponse(HttpResponse {
            request_id: request.request_id,
            status,
            headers: Vec::new(),
            body,
        })
    }

    struct MockUpdateServer {
        etag: &'static str,
    }

    impl MockUpdateServer {
        fn not_modified(&self, request: &NetworkCommand) -> NetworkEvent {
            let NetworkCommand::Http(request) = request else {
                panic!("expected HTTP request")
            };
            assert!(
                request
                    .headers
                    .iter()
                    .any(|(name, value)| name == "If-None-Match" && value == self.etag)
            );
            NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 304,
                headers: Vec::new(),
                body: Vec::new(),
            })
        }

        fn manifest(
            &self,
            request: &NetworkCommand,
            signed: &UiSignedUpdateManifest,
        ) -> NetworkEvent {
            let NetworkCommand::Http(request) = request else {
                panic!("expected HTTP request")
            };
            NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: vec![("ETag".to_owned(), self.etag.to_owned())],
                body: serde_json::to_vec(signed).unwrap(),
            })
        }
    }

    #[test]
    fn signed_manifest_accepts_rotated_live_key_and_rejects_revoked_key() {
        let (signed, key, _) = signed_manifest();
        let mut trust = UiUpdateTrustStore::default();
        trust
            .insert_key("release_2026", key.verifying_key().to_bytes())
            .unwrap();
        assert!(signed.verify(&trust).is_ok());
        trust.revoke_key("release_2026");
        assert_eq!(
            signed.verify(&trust).unwrap_err().code(),
            "UI_UPDATE_SIGNATURE_KEY_REVOKED"
        );
    }

    #[test]
    fn remote_client_downloads_signed_files_and_defers_activation_until_safe() {
        let root = temp_root("download");
        let (signed, key, files) = signed_manifest();
        let mut client = client(&root, &key);
        let manifest_request = client.check_now().pop().unwrap();
        let (requests, events) = client.handle_network_event(&response(
            &manifest_request,
            200,
            serde_json::to_vec(&signed).unwrap(),
        ));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, UiUpdateClientEvent::UpdateReady { .. }))
        );
        assert_eq!(requests.len(), files.len());
        let mut next = Vec::new();
        let mut download_events = Vec::new();
        for request in requests {
            let NetworkCommand::Http(http) = &request else {
                unreachable!()
            };
            let path = http.url.split("/1.0.0/").nth(1).unwrap();
            let (more, events) =
                client.handle_network_event(&response(&request, 200, files[path].clone()));
            next.extend(more);
            download_events.extend(events);
        }
        assert!(next.is_empty());
        assert!(download_events.is_empty());
        let mut catalog = UiDocumentContentCacheAssets::default();
        assert_eq!(
            client.activate_if_safe(false, &mut catalog),
            Some(UiUpdateClientEvent::ActivationDeferred)
        );
        assert!(client.cache.active_generation().unwrap().is_none());
        assert!(matches!(
            client.activate_if_safe(true, &mut catalog),
            Some(UiUpdateClientEvent::Activated { .. })
        ));
        assert_eq!(
            client
                .cache
                .active_generation()
                .unwrap()
                .unwrap()
                .bundle()
                .version,
            "1.0.0"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn old_or_bad_signature_keeps_active_generation_and_telemetry_is_redacted() {
        let root = temp_root("reject");
        let (mut signed, key, _) = signed_manifest();
        signed.signature.signature_hex.replace_range(..2, "00");
        let mut client = client(&root, &key);
        let request = client.check_now().pop().unwrap();
        let (_, events) = client.handle_network_event(&response(
            &request,
            200,
            serde_json::to_vec(&signed).unwrap(),
        ));
        assert_eq!(
            events,
            vec![UiUpdateClientEvent::Failed {
                code: "UI_UPDATE_SIGNATURE_INVALID"
            }]
        );
        assert!(client.cache.active_generation().unwrap().is_none());
        assert!(
            client
                .telemetry()
                .all(|entry| entry.code != Some("http://127.0.0.1:39001"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mock_server_304_and_timeout_use_etag_and_finite_retry_budget() {
        let root = temp_root("retry");
        let (signed, key, _) = signed_manifest();
        let server = MockUpdateServer {
            etag: "\"fixture-v1\"",
        };
        let mut client = client(&root, &key);
        let first = client.check_now().pop().unwrap();
        let (_, events) = client.handle_network_event(&server.manifest(&first, &signed));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, UiUpdateClientEvent::UpdateReady { .. }))
        );
        client.cancel();
        let second = client.check_now().pop().unwrap();
        let (_, events) = client.handle_network_event(&server.not_modified(&second));
        assert_eq!(events, vec![UiUpdateClientEvent::NoUpdate]);

        let request = client.check_now().pop().unwrap();
        let (_, events) = client.handle_network_event(&NetworkEvent::HttpError {
            request_id: match request {
                NetworkCommand::Http(ref request) => request.request_id,
                _ => unreachable!(),
            },
            error: "timeout".to_owned(),
        });
        assert!(events.is_empty());
        for _ in 0..UI_UPDATE_MAX_RETRIES {
            let retry = client.poll_retries().pop().unwrap();
            let request_id = match retry {
                NetworkCommand::Http(ref request) => request.request_id,
                _ => unreachable!(),
            };
            let (_, events) = client.handle_network_event(&NetworkEvent::HttpError {
                request_id,
                error: "timeout".to_owned(),
            });
            if events.is_empty() {
                continue;
            }
            assert_eq!(
                events,
                vec![UiUpdateClientEvent::Failed {
                    code: "UI_UPDATE_NETWORK_RETRIES_EXHAUSTED"
                }]
            );
        }
        assert!(
            client
                .telemetry()
                .all(|entry| entry.code != Some("timeout"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mock_server_resumes_a_segmented_file_without_unbounded_requests() {
        let root = temp_root("resume");
        let (signed, key, files) = signed_manifest();
        let server = MockUpdateServer {
            etag: "\"fixture-v1\"",
        };
        let mut client = client(&root, &key);
        let manifest = client.check_now().pop().unwrap();
        let (requests, _) = client.handle_network_event(&server.manifest(&manifest, &signed));
        let document_request = requests
            .iter()
            .find(|request| matches!(request, NetworkCommand::Http(http) if http.url.ends_with("documents/update_page.json")))
            .unwrap()
            .clone();
        let registration_request = requests
            .iter()
            .find(|request| matches!(request, NetworkCommand::Http(http) if http.url.ends_with("registrations/update_page.json")))
            .unwrap()
            .clone();
        let document = &files["documents/update_page.json"];
        let first_half = document[..document.len() / 2].to_vec();
        let (more, events) =
            client.handle_network_event(&response(&document_request, 206, first_half));
        assert!(events.is_empty());
        let resume = more.into_iter().next().unwrap();
        let NetworkCommand::Http(resume_http) = &resume else {
            unreachable!()
        };
        assert!(
            resume_http
                .headers
                .iter()
                .any(|(name, value)| name == "Range" && value.starts_with("bytes="))
        );
        let (_, events) = client.handle_network_event(&response(
            &registration_request,
            200,
            files["registrations/update_page.json"].clone(),
        ));
        assert!(events.is_empty());
        let (_, events) = client.handle_network_event(&response(
            &resume,
            206,
            document[document.len() / 2..].to_vec(),
        ));
        assert!(events.is_empty());
        assert!(matches!(
            client.activate_if_safe(true, &mut UiDocumentContentCacheAssets::default()),
            Some(UiUpdateClientEvent::Activated { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn endpoint_policy_and_versions_are_closed() {
        assert!(UiUpdateEndpoint::local("https://other.example", "game-ui", "stable").is_err());
        assert!(UiUpdateEndpoint::local("http://127.0.0.1:39001", "game-ui", "stable").is_ok());
        assert!(
            UiUpdateEndpoint::production("game-ui", "stable")
                .unwrap()
                .base_url
                .starts_with("https://")
        );
        assert!(parse_release_version("1.2.3").is_some());
        assert!(parse_release_version("v1").is_none());
        assert!(parse_release_version("1.2.3.4").is_none());
    }

    #[test]
    fn incompatible_or_unauthorized_downgrade_manifests_are_rejected_before_download() {
        let root = temp_root("version-policy");
        let (bundle, files) = fixture_bundle();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let client = client(&root, &key);
        client
            .cache
            .install(&UiUpdateBundleImport {
                manifest_json: serde_json::to_vec(&bundle).unwrap(),
                files,
            })
            .unwrap();
        let mut old_bundle = bundle.clone();
        old_bundle.version = "0.9.0".to_owned();
        let old_release = UiUpdateRelease {
            format_version: 1,
            bundle: old_bundle,
            policy: UiUpdateReleasePolicy::default(),
        };
        assert_eq!(
            client
                .validate_remote_release(&old_release)
                .unwrap_err()
                .code(),
            "UI_UPDATE_RELEASE_DOWNGRADE_REJECTED"
        );
        let mut incompatible = bundle;
        incompatible.client_compatibility = UiUpdateRevisionRange {
            minimum: 2,
            maximum: 3,
        };
        let incompatible_release = UiUpdateRelease {
            format_version: 1,
            bundle: incompatible,
            policy: UiUpdateReleasePolicy::default(),
        };
        assert_eq!(
            client
                .validate_remote_release(&incompatible_release)
                .unwrap_err()
                .code(),
            "UI_UPDATE_RELEASE_CLIENT_INCOMPATIBLE"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn activation_failure_keeps_the_previous_generation_available() {
        let root = temp_root("activation-rollback");
        let (initial_bundle, files) = fixture_bundle();
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let mut client = client(&root, &key);
        client
            .cache
            .install(&UiUpdateBundleImport {
                manifest_json: serde_json::to_vec(&initial_bundle).unwrap(),
                files: files.clone(),
            })
            .unwrap();

        let mut next_bundle = initial_bundle;
        next_bundle.version = "1.0.1".to_owned();
        let signed = UiSignedUpdateManifest::sign(
            UiUpdateRelease {
                format_version: 1,
                bundle: next_bundle,
                policy: UiUpdateReleasePolicy::default(),
            },
            "release_2026",
            &key,
        )
        .unwrap();
        let manifest = client.check_now().pop().unwrap();
        let (requests, _) = client.handle_network_event(&response(
            &manifest,
            200,
            serde_json::to_vec(&signed).unwrap(),
        ));
        for request in requests {
            let NetworkCommand::Http(http) = &request else {
                unreachable!()
            };
            let path = http.url.split("/1.0.1/").nth(1).unwrap();
            client.handle_network_event(&response(&request, 200, files[path].clone()));
        }
        let staging = fs::read_dir(root.join("staging"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(staging.join("documents/update_page.json"), b"corrupt").unwrap();
        assert_eq!(
            client.activate_if_safe(true, &mut UiDocumentContentCacheAssets::default()),
            Some(UiUpdateClientEvent::Failed {
                code: "UI_UPDATE_ACTIVATION_FAILED"
            })
        );
        assert_eq!(
            client
                .cache
                .active_generation()
                .unwrap()
                .unwrap()
                .bundle()
                .version,
            "1.0.0"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
