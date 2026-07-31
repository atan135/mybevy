//! Controlled publisher for signed UI update releases.
//!
//! It validates the same cache contract used by the game before copying anything to the release
//! directory. The final signed manifest is written last, so a static host never observes a
//! partially published version through this layout.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Component, Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::SigningKey;
use project::framework::ui::document::{
    UiSignedUpdateManifest, UiUpdateBundle, UiUpdateBundleImport, UiUpdateCache,
    UiUpdateCacheConfig, UiUpdateRelease, UiUpdateReleasePolicy,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("ui-update-publish: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let bundle_json = fs::read(&arguments.bundle).map_err(|_| "cannot read --bundle")?;
    let bundle: UiUpdateBundle = serde_json::from_slice(&bundle_json)
        .map_err(|_| "--bundle is not a valid UiUpdateBundle JSON")?;
    if bundle.channel != arguments.channel || bundle.version != arguments.version {
        return Err("--channel/--version must exactly match the bundle".to_owned());
    }
    let files = read_bundle_files(&arguments.input_dir, &bundle)?;
    validate_with_runtime_cache(&bundle, &files)?;
    let signing_key_hex = env::var(&arguments.signing_key_env)
        .map_err(|_| "--signing-key-env is not set in the publishing environment")?;
    let signing_key = SigningKey::from_bytes(&decode_hex_32(&signing_key_hex)?);
    let signed = UiSignedUpdateManifest::sign(
        UiUpdateRelease {
            format_version: 1,
            bundle: bundle.clone(),
            policy: UiUpdateReleasePolicy::default(),
        },
        arguments.key_id,
        &signing_key,
    )
    .map_err(|error| error.code().to_owned())?;
    let release_root = arguments
        .output_dir
        .join(&arguments.channel)
        .join(&arguments.version);
    publish_no_clobber(&release_root, &files, &signed)?;
    println!(
        "published channel={} version={} canonical_release_sha256={}",
        bundle.channel,
        bundle.version,
        signed
            .canonical_release_sha256()
            .map_err(|error| error.code().to_owned())?
    );
    Ok(())
}

struct Arguments {
    bundle: PathBuf,
    input_dir: PathBuf,
    output_dir: PathBuf,
    channel: String,
    version: String,
    key_id: String,
    signing_key_env: String,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut values = BTreeMap::new();
        let mut args = env::args().skip(1);
        while let Some(name) = args.next() {
            if !name.starts_with("--") {
                return Err(usage());
            }
            let value = args.next().ok_or_else(usage)?;
            if values.insert(name, value).is_some() {
                return Err(usage());
            }
        }
        let mut required = |name: &str| values.remove(name).ok_or_else(usage);
        let result = Self {
            bundle: PathBuf::from(required("--bundle")?),
            input_dir: PathBuf::from(required("--input-dir")?),
            output_dir: PathBuf::from(required("--output-dir")?),
            channel: required("--channel")?,
            version: required("--version")?,
            key_id: required("--key-id")?,
            signing_key_env: required("--signing-key-env")?,
        };
        if values.is_empty() {
            Ok(result)
        } else {
            Err(usage())
        }
    }
}

fn usage() -> String {
    "usage: ui-update-publish --bundle <bundle.json> --input-dir <files> --output-dir <release-root> --channel <channel> --version <x.y.z> --key-id <key> --signing-key-env <environment-variable>".to_owned()
}

fn read_bundle_files(
    input_dir: &Path,
    bundle: &UiUpdateBundle,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut paths = Vec::new();
    for document in &bundle.documents {
        paths.push(document.path.as_str());
        paths.push(document.registration_path.as_str());
    }
    for asset in &bundle.assets {
        paths.push(asset.path.as_str());
    }
    let mut files = BTreeMap::new();
    for path in paths {
        if !safe_relative_path(path) || files.contains_key(path) {
            return Err("bundle contains an unsafe or duplicate file path".to_owned());
        }
        let source = input_dir.join(path);
        files.insert(
            path.to_owned(),
            fs::read(source).map_err(|_| "bundle file is missing")?,
        );
    }
    Ok(files)
}

fn validate_with_runtime_cache(
    bundle: &UiUpdateBundle,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let root = env::temp_dir().join(format!("mybevy-ui-update-publish-{}", nonce()));
    let cache = UiUpdateCache::open(
        UiUpdateCacheConfig::new(&root, &bundle.bundle_id, &bundle.channel)
            .map_err(|error| error.code().to_owned())?,
    )
    .map_err(|error| error.code().to_owned())?;
    let result = cache
        .stage(&UiUpdateBundleImport {
            manifest_json: serde_json::to_vec(bundle).map_err(|_| "cannot serialize bundle")?,
            files: files.clone(),
        })
        .map(|_| ())
        .map_err(|error| error.code().to_owned());
    let _ = fs::remove_dir_all(root);
    result
}

fn publish_no_clobber(
    release_root: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    signed: &UiSignedUpdateManifest,
) -> Result<(), String> {
    let parent = release_root
        .parent()
        .ok_or_else(|| "invalid output directory".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "cannot create release parent")?;
    fs::create_dir(release_root)
        .map_err(|_| "release version already exists or cannot be created")?;
    let copy_result = (|| {
        for (path, bytes) in files {
            let destination = release_root.join("files").join(path);
            let directory = destination
                .parent()
                .ok_or_else(|| "invalid file destination".to_owned())?;
            fs::create_dir_all(directory)
                .map_err(|_| "cannot create release file directory".to_owned())?;
            fs::write(destination, bytes).map_err(|_| "cannot write release file".to_owned())?;
        }
        let manifest = serde_json::to_vec_pretty(signed)
            .map_err(|_| "cannot serialize signed manifest".to_owned())?;
        fs::write(release_root.join("manifest.json"), manifest)
            .map_err(|_| "cannot write signed manifest".to_owned())
    })();
    if copy_result.is_err() {
        let _ = fs::remove_dir_all(release_root);
    }
    copy_result
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("signing key must contain 64 hex characters".to_owned());
    }
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "signing key must contain hexadecimal characters")?;
    }
    Ok(result)
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default()
}
