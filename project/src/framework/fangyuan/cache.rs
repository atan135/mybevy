use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::PathBuf,
};

use super::{
    FangyuanBlueprintIdentity, FangyuanIdentityDependency, FangyuanIdentityResourceKind,
    FangyuanIdentitySourceKind, fangyuan_bake_hash_bytes,
};

pub const FANGYUAN_BLUEPRINT_CACHE_MANIFEST_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBlueprintCacheManifest {
    pub version: u16,
    pub root_dir: PathBuf,
    pub max_bytes: u64,
    pub used_bytes: u64,
    pub entries: BTreeMap<String, FangyuanBlueprintCacheEntry>,
}

impl FangyuanBlueprintCacheManifest {
    pub fn new(root_dir: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            version: FANGYUAN_BLUEPRINT_CACHE_MANIFEST_VERSION,
            root_dir: root_dir.into(),
            max_bytes,
            used_bytes: 0,
            entries: BTreeMap::new(),
        }
    }

    pub fn to_ron_string(&self) -> Result<String, FangyuanBlueprintCacheError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|source| {
            FangyuanBlueprintCacheError::ManifestSerialize {
                message: source.to_string(),
            }
        })
    }

    pub fn from_ron_str(source: &str) -> Result<Self, FangyuanBlueprintCacheError> {
        let manifest = ron::from_str::<Self>(source).map_err(|source| {
            FangyuanBlueprintCacheError::ManifestParse {
                message: source.to_string(),
            }
        })?;
        if manifest.version != FANGYUAN_BLUEPRINT_CACHE_MANIFEST_VERSION {
            return Err(FangyuanBlueprintCacheError::ManifestVersionMismatch {
                expected: FANGYUAN_BLUEPRINT_CACHE_MANIFEST_VERSION,
                actual: manifest.version,
            });
        }
        Ok(manifest)
    }

    fn recompute_used_bytes(&mut self) {
        self.used_bytes = self.entries.values().map(|entry| entry.size).sum();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanBlueprintCacheEntry {
    pub identity: FangyuanBlueprintIdentity,
    pub relative_path: PathBuf,
    pub content_hash: u64,
    pub version: String,
    pub size: u64,
    pub last_used: u64,
    pub use_count: u64,
    pub dependencies: Vec<FangyuanIdentityDependency>,
    pub source_kind: FangyuanIdentitySourceKind,
}

impl FangyuanBlueprintCacheEntry {
    pub fn has_dependency_key(&self, key: &str) -> bool {
        self.dependencies
            .iter()
            .any(|dependency| dependency.cache_key() == key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanBlueprintCacheHit {
    pub entry: FangyuanBlueprintCacheEntry,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanBlueprintCache {
    pub manifest: FangyuanBlueprintCacheManifest,
    storage: BTreeMap<PathBuf, Vec<u8>>,
    clock: u64,
}

impl FangyuanBlueprintCache {
    pub fn new(root_dir: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            manifest: FangyuanBlueprintCacheManifest::new(root_dir, max_bytes),
            storage: BTreeMap::new(),
            clock: 0,
        }
    }

    pub fn write(
        &mut self,
        identity: FangyuanBlueprintIdentity,
        bytes: Vec<u8>,
        dependencies: Vec<FangyuanIdentityDependency>,
    ) -> Result<(), FangyuanBlueprintCacheError> {
        let size = bytes.len() as u64;
        if size > self.manifest.max_bytes {
            return Err(FangyuanBlueprintCacheError::EntryTooLarge {
                key: identity.cache_key(),
                size,
                max_bytes: self.manifest.max_bytes,
            });
        }

        let actual_hash = fangyuan_bake_hash_bytes(&bytes);
        if actual_hash != identity.content_hash {
            return Err(FangyuanBlueprintCacheError::HashMismatch {
                key: identity.cache_key(),
                expected: identity.content_hash,
                actual: actual_hash,
            });
        }

        self.clock = self.clock.saturating_add(1);
        let key = identity.cache_key();
        let relative_path = cache_relative_path(&identity);
        if let Some(existing) = self.manifest.entries.remove(&key) {
            self.storage.remove(&existing.relative_path);
        }

        self.storage.insert(relative_path.clone(), bytes);
        self.manifest.entries.insert(
            key.clone(),
            FangyuanBlueprintCacheEntry {
                identity: identity.clone(),
                relative_path,
                content_hash: identity.content_hash,
                version: identity.version.clone(),
                size,
                last_used: self.clock,
                use_count: 1,
                dependencies,
                source_kind: identity.source_kind,
            },
        );
        self.manifest.recompute_used_bytes();
        self.evict_to_capacity(&key);
        Ok(())
    }

    pub fn read(
        &mut self,
        kind: FangyuanIdentityResourceKind,
        id: &str,
        expected_version: &str,
        expected_content_hash: u64,
        available_dependency_keys: impl IntoIterator<Item = String>,
    ) -> Result<FangyuanBlueprintCacheHit, FangyuanBlueprintCacheError> {
        let key = cache_key(kind, id);
        let available_dependency_keys = available_dependency_keys
            .into_iter()
            .collect::<BTreeSet<_>>();

        let entry = self.manifest.entries.get(&key).cloned().ok_or_else(|| {
            FangyuanBlueprintCacheError::Miss {
                key: key.clone(),
                reason: FangyuanBlueprintCacheMissReason::NotFound,
            }
        })?;

        if entry.version != expected_version {
            return Err(FangyuanBlueprintCacheError::Miss {
                key,
                reason: FangyuanBlueprintCacheMissReason::VersionMismatch {
                    expected: expected_version.to_string(),
                    actual: entry.version,
                },
            });
        }

        if entry.content_hash != expected_content_hash {
            return Err(FangyuanBlueprintCacheError::Miss {
                key,
                reason: FangyuanBlueprintCacheMissReason::HashMismatch {
                    expected: expected_content_hash,
                    actual: entry.content_hash,
                },
            });
        }

        if let Some(missing) = entry
            .dependencies
            .iter()
            .map(FangyuanIdentityDependency::cache_key)
            .find(|dependency_key| !available_dependency_keys.contains(dependency_key))
        {
            return Err(FangyuanBlueprintCacheError::Miss {
                key,
                reason: FangyuanBlueprintCacheMissReason::MissingDependency { key: missing },
            });
        }

        let bytes = self
            .storage
            .get(&entry.relative_path)
            .cloned()
            .ok_or_else(|| FangyuanBlueprintCacheError::CorruptFile {
                key: key.clone(),
                path: entry.relative_path.clone(),
            })?;

        let actual_hash = fangyuan_bake_hash_bytes(&bytes);
        if actual_hash != entry.content_hash {
            return Err(FangyuanBlueprintCacheError::HashMismatch {
                key,
                expected: entry.content_hash,
                actual: actual_hash,
            });
        }

        self.clock = self.clock.saturating_add(1);
        if let Some(entry) = self.manifest.entries.get_mut(&entry.identity.cache_key()) {
            entry.last_used = self.clock;
            entry.use_count = entry.use_count.saturating_add(1);
        }
        let entry = self
            .manifest
            .entries
            .get(&entry.identity.cache_key())
            .cloned()
            .expect("cache entry should exist after hit update");

        Ok(FangyuanBlueprintCacheHit { entry, bytes })
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.manifest.entries.contains_key(key)
    }

    pub fn entry(&self, key: &str) -> Option<&FangyuanBlueprintCacheEntry> {
        self.manifest.entries.get(key)
    }

    fn evict_to_capacity(&mut self, protected_key: &str) {
        while self.manifest.used_bytes > self.manifest.max_bytes {
            let Some(victim_key) = self
                .manifest
                .entries
                .iter()
                .filter(|(key, _)| key.as_str() != protected_key)
                .min_by_key(|(_, entry)| (entry.last_used, entry.use_count, entry.size))
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            if let Some(entry) = self.manifest.entries.remove(&victim_key) {
                self.storage.remove(&entry.relative_path);
            }
            self.manifest.recompute_used_bytes();
        }
    }
}

pub fn cache_key(kind: FangyuanIdentityResourceKind, id: &str) -> String {
    format!("{}:{id}", kind.as_str())
}

fn cache_relative_path(identity: &FangyuanBlueprintIdentity) -> PathBuf {
    let id = identity
        .id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    PathBuf::from(identity.kind.as_str()).join(format!(
        "{}-{}-{:016x}.fycache",
        id, identity.version, identity.content_hash
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanBlueprintCacheMissReason {
    NotFound,
    VersionMismatch { expected: String, actual: String },
    HashMismatch { expected: u64, actual: u64 },
    MissingDependency { key: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanBlueprintCacheError {
    Miss {
        key: String,
        reason: FangyuanBlueprintCacheMissReason,
    },
    HashMismatch {
        key: String,
        expected: u64,
        actual: u64,
    },
    CorruptFile {
        key: String,
        path: PathBuf,
    },
    EntryTooLarge {
        key: String,
        size: u64,
        max_bytes: u64,
    },
    ManifestVersionMismatch {
        expected: u16,
        actual: u16,
    },
    ManifestSerialize {
        message: String,
    },
    ManifestParse {
        message: String,
    },
}

impl fmt::Display for FangyuanBlueprintCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Miss { key, reason } => {
                write!(formatter, "fangyuan cache miss for {key}: {reason:?}")
            }
            Self::HashMismatch {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "fangyuan cache hash mismatch for {key}: expected {expected:016x}, actual {actual:016x}"
            ),
            Self::CorruptFile { key, path } => write!(
                formatter,
                "fangyuan cache corrupt file for {key}: {}",
                path.display()
            ),
            Self::EntryTooLarge {
                key,
                size,
                max_bytes,
            } => write!(
                formatter,
                "fangyuan cache entry {key} is too large: size={size}, max_bytes={max_bytes}"
            ),
            Self::ManifestVersionMismatch { expected, actual } => write!(
                formatter,
                "fangyuan cache manifest version mismatch: expected {expected}, actual {actual}"
            ),
            Self::ManifestSerialize { message } => {
                write!(
                    formatter,
                    "fangyuan cache manifest serialize failed: {message}"
                )
            }
            Self::ManifestParse { message } => {
                write!(formatter, "fangyuan cache manifest parse failed: {message}")
            }
        }
    }
}

impl Error for FangyuanBlueprintCacheError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{FangyuanIdentityHashes, FangyuanIdentitySourceKind};

    #[test]
    fn fangyuan_blueprint_cache_hits_and_updates_lru_usage() {
        let bytes = b"blueprint-cache-hit".to_vec();
        let dependency = FangyuanIdentityDependency::new(
            FangyuanIdentityResourceKind::MaterialProfile,
            "fx/test",
        );
        let identity = test_identity("avatar_a", "1", &bytes);
        let mut cache = FangyuanBlueprintCache::new("fangyuan/cache", 1024);

        cache
            .write(identity.clone(), bytes.clone(), vec![dependency.clone()])
            .unwrap();
        let hit = cache
            .read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "1",
                identity.content_hash,
                [dependency.cache_key()],
            )
            .unwrap();

        assert_eq!(hit.bytes, bytes);
        assert_eq!(hit.entry.use_count, 2);
        assert!(hit.entry.last_used > 1);
        assert!(hit.entry.has_dependency_key("material_profile:fx/test"));
    }

    #[test]
    fn fangyuan_blueprint_cache_reports_not_found_version_hash_and_missing_dependency_misses() {
        let bytes = b"blueprint-cache-miss".to_vec();
        let dependency = FangyuanIdentityDependency::new(
            FangyuanIdentityResourceKind::MaterialProfile,
            "fx/test",
        );
        let identity = test_identity("avatar_a", "1", &bytes);
        let mut cache = FangyuanBlueprintCache::new("fangyuan/cache", 1024);
        cache
            .write(identity.clone(), bytes, vec![dependency.clone()])
            .unwrap();

        assert!(matches!(
            cache.read(
                FangyuanIdentityResourceKind::Blueprint,
                "missing",
                "1",
                identity.content_hash,
                Vec::<String>::new(),
            ),
            Err(FangyuanBlueprintCacheError::Miss {
                reason: FangyuanBlueprintCacheMissReason::NotFound,
                ..
            })
        ));
        assert!(matches!(
            cache.read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "2",
                identity.content_hash,
                [dependency.cache_key()],
            ),
            Err(FangyuanBlueprintCacheError::Miss {
                reason: FangyuanBlueprintCacheMissReason::VersionMismatch { .. },
                ..
            })
        ));
        assert!(matches!(
            cache.read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "1",
                identity.content_hash.wrapping_add(1),
                [dependency.cache_key()],
            ),
            Err(FangyuanBlueprintCacheError::Miss {
                reason: FangyuanBlueprintCacheMissReason::HashMismatch { .. },
                ..
            })
        ));
        assert!(matches!(
            cache.read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "1",
                identity.content_hash,
                Vec::<String>::new(),
            ),
            Err(FangyuanBlueprintCacheError::Miss {
                reason: FangyuanBlueprintCacheMissReason::MissingDependency { .. },
                ..
            })
        ));
    }

    #[test]
    fn fangyuan_blueprint_cache_evicts_lru_entries_when_capacity_is_exceeded() {
        let mut cache = FangyuanBlueprintCache::new("fangyuan/cache", 25);
        let a = b"aaaaaaaaaa".to_vec();
        let b = b"bbbbbbbbbb".to_vec();
        let c = b"cccccccccc".to_vec();
        let id_a = test_identity("avatar_a", "1", &a);
        let id_b = test_identity("avatar_b", "1", &b);
        let id_c = test_identity("avatar_c", "1", &c);

        cache.write(id_a.clone(), a, Vec::new()).unwrap();
        cache.write(id_b.clone(), b, Vec::new()).unwrap();
        cache
            .read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "1",
                id_a.content_hash,
                Vec::<String>::new(),
            )
            .unwrap();
        cache.write(id_c, c, Vec::new()).unwrap();

        assert!(cache.contains_key(&id_a.cache_key()));
        assert!(!cache.contains_key(&id_b.cache_key()));
        assert_eq!(cache.manifest.used_bytes, 20);
    }

    #[test]
    fn fangyuan_blueprint_cache_reports_hash_mismatch_and_corrupted_file() {
        let bytes = b"blueprint-cache-corrupt".to_vec();
        let identity = test_identity("avatar_a", "1", &bytes);
        let mut cache = FangyuanBlueprintCache::new("fangyuan/cache", 1024);
        cache.write(identity.clone(), bytes, Vec::new()).unwrap();

        let entry = cache.entry(&identity.cache_key()).unwrap().clone();
        cache
            .storage
            .insert(entry.relative_path.clone(), b"tampered".to_vec());
        assert!(matches!(
            cache.read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "1",
                identity.content_hash,
                Vec::<String>::new(),
            ),
            Err(FangyuanBlueprintCacheError::HashMismatch { .. })
        ));

        cache.storage.remove(&entry.relative_path);
        assert!(matches!(
            cache.read(
                FangyuanIdentityResourceKind::Blueprint,
                "avatar_a",
                "1",
                identity.content_hash,
                Vec::<String>::new(),
            ),
            Err(FangyuanBlueprintCacheError::CorruptFile { .. })
        ));
    }

    #[test]
    fn fangyuan_blueprint_cache_manifest_roundtrips_capacity_and_entry_metadata() {
        let bytes = b"blueprint-cache-manifest".to_vec();
        let identity = test_identity("avatar_a", "1", &bytes);
        let mut cache = FangyuanBlueprintCache::new("fangyuan/cache", 1024);
        cache.write(identity.clone(), bytes, Vec::new()).unwrap();

        let manifest =
            FangyuanBlueprintCacheManifest::from_ron_str(&cache.manifest.to_ron_string().unwrap())
                .unwrap();

        assert_eq!(manifest.max_bytes, 1024);
        assert_eq!(manifest.used_bytes, cache.manifest.used_bytes);
        assert_eq!(
            manifest.entries[&identity.cache_key()].content_hash,
            identity.content_hash
        );
        assert_eq!(manifest.entries[&identity.cache_key()].version, "1");
    }

    fn test_identity(id: &str, version: &str, bytes: &[u8]) -> FangyuanBlueprintIdentity {
        FangyuanBlueprintIdentity::new(
            FangyuanIdentityResourceKind::Blueprint,
            id,
            version,
            FangyuanIdentityHashes::from_bytes(
                FangyuanIdentityResourceKind::Blueprint,
                bytes,
                bytes,
                &[],
            ),
            FangyuanIdentitySourceKind::RuntimeCache,
        )
        .unwrap()
    }
}
