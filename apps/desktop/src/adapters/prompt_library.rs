//! Built-in prompt inspiration catalog, favorites, and constrained image cache.

use super::paths::AppPaths;
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const CATALOG_JSON: &str = include_str!("../../../../resources/prompt-library/catalog.json");
const STATE_SCHEMA_VERSION: u32 = 1;
const STATE_FILE: &str = "prompt-library-state.json";
const CACHE_DIRECTORY: &str = "prompt-library";
const CACHE_LIMIT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 64_000_000;
const MAX_FAVORITES_BYTES: u64 = 1024 * 1024;
const DOWNLOAD_CONCURRENCY: usize = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
struct CaseAsset {
    id: String,
    source_case_id: u64,
    image_path: String,
    filename: String,
    sha256: String,
    bytes: u64,
    mime_type: String,
    width: u32,
    height: u32,
}

#[derive(Clone, Debug)]
struct FavoriteState {
    ids: Vec<String>,
    load_error: Option<String>,
    save_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FavoritesDocument {
    schema_version: u32,
    favorites: Vec<String>,
}

struct DownloadGate {
    active: Mutex<usize>,
    available: Condvar,
}

struct DownloadPermit {
    gate: Arc<DownloadGate>,
}

impl Drop for DownloadPermit {
    fn drop(&mut self) {
        let mut active = self
            .gate
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = active.saturating_sub(1);
        self.gate.available.notify_one();
    }
}

struct InflightGate {
    active: Mutex<HashSet<String>>,
    available: Condvar,
}

struct InflightPermit {
    case_id: String,
    gate: Arc<InflightGate>,
}

impl Drop for InflightPermit {
    fn drop(&mut self) {
        let mut active = self
            .gate
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.remove(&self.case_id);
        self.gate.available.notify_all();
    }
}

impl DownloadGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            active: Mutex::new(0),
            available: Condvar::new(),
        })
    }

    fn acquire(self: &Arc<Self>) -> DownloadPermit {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= DOWNLOAD_CONCURRENCY {
            active = self
                .available
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *active += 1;
        DownloadPermit { gate: self.clone() }
    }
}

impl InflightGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            active: Mutex::new(HashSet::new()),
            available: Condvar::new(),
        })
    }

    fn acquire(self: &Arc<Self>, case_id: &str) -> InflightPermit {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while active.contains(case_id) {
            active = self
                .available
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        active.insert(case_id.to_owned());
        InflightPermit {
            case_id: case_id.to_owned(),
            gate: self.clone(),
        }
    }
}

pub struct PromptLibraryStore {
    catalog: Value,
    cases: HashMap<String, CaseAsset>,
    client: Client,
    state_path: PathBuf,
    cache_directory: PathBuf,
    favorites: Mutex<FavoriteState>,
    downloads: Arc<DownloadGate>,
    inflight: Arc<InflightGate>,
}

impl PromptLibraryStore {
    pub fn load(paths: &AppPaths) -> Result<Self, String> {
        let catalog: Value = serde_json::from_str(CATALOG_JSON)
            .map_err(|error| format!("灵感库目录无效：{error}"))?;
        let cases = catalog_cases(&catalog)?;
        let state_path = paths.data.join(STATE_FILE);
        let favorites = load_favorites(&state_path);
        let cache_directory = paths.cache.join(CACHE_DIRECTORY);
        fs::create_dir_all(&cache_directory)
            .map_err(|error| format!("无法创建灵感库图片缓存目录：{error}"))?;
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(concat!("ImageStudio/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("无法初始化灵感库图片客户端：{error}"))?;
        Ok(Self {
            catalog,
            cases,
            client,
            state_path,
            cache_directory,
            favorites: Mutex::new(favorites),
            downloads: DownloadGate::new(),
            inflight: InflightGate::new(),
        })
    }

    pub fn snapshot(&self) -> Value {
        let favorites = self
            .favorites
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let cache = self.cache_status();
        json!({
            "catalog": self.catalog,
            "favorites": favorites.ids,
            "favoritesLoadError": favorites.load_error,
            "favoritesSaveError": favorites.save_error,
            "cache": cache
        })
    }

    pub fn set_favorite(&self, case_id: &str, favorite: bool) -> Result<Value, String> {
        if !self.cases.contains_key(case_id) {
            return Err("未登记的灵感案例".into());
        }
        let mut guard = self
            .favorites
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut next = guard.ids.clone();
        if favorite && !next.iter().any(|id| id == case_id) {
            next.push(case_id.to_owned());
        } else if !favorite {
            next.retain(|id| id != case_id);
        }
        sort_favorites(&mut next, &self.cases);
        let document = FavoritesDocument {
            schema_version: STATE_SCHEMA_VERSION,
            favorites: next.clone(),
        };
        if let Err(error) = write_favorites(&self.state_path, &document) {
            guard.save_error = Some(error.clone());
            return Err(error);
        }
        guard.ids = next;
        guard.load_error = None;
        guard.save_error = None;
        Ok(json!({
            "favorites": guard.ids,
            "favoritesLoadError": guard.load_error,
            "favoritesSaveError": guard.save_error
        }))
    }

    pub fn ensure_image(&self, case_id: &str) -> Result<Value, String> {
        let case = self.case(case_id)?;
        if self.read_cached_image(case_id).is_err() {
            let _inflight = self.inflight.acquire(case_id);
            // Another request may have completed while this one waited.
            if self.read_cached_image(case_id).is_err() {
                let _download = self.downloads.acquire();
                self.download_image(&case)?;
            }
        }
        touch(&self.cache_path(&case));
        Ok(json!({
            "caseId": case.id,
            "sourceCaseId": case.source_case_id,
            "mimeType": case.mime_type,
            "bytes": case.bytes,
            "width": case.width,
            "height": case.height
        }))
    }

    pub fn read_cached_image(&self, case_id: &str) -> Result<(Vec<u8>, String), String> {
        let case = self.case(case_id)?;
        let path = self.cache_path(&case);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err("图片尚未缓存".into());
            }
            Err(error) => return Err(format!("无法读取灵感库图片缓存：{error}")),
        };
        if !metadata.file_type().is_file() || metadata.len() != case.bytes {
            return Err("灵感库图片缓存不是有效普通文件".into());
        }
        let bytes = fs::read(&path).map_err(|error| format!("无法读取灵感库图片缓存：{error}"))?;
        validate_image_bytes(&bytes, &case)?;
        Ok((bytes, case.mime_type))
    }

    pub fn cache_status(&self) -> Value {
        let (bytes, count) = self.cache_usage();
        json!({
            "bytes": bytes,
            "count": count,
            "limitBytes": CACHE_LIMIT_BYTES
        })
    }

    pub fn clear_cache(&self) -> Result<Value, String> {
        let mut removed_bytes = 0_u64;
        let mut removed_count = 0_u64;
        for case in self.cases.values() {
            let path = self.cache_path(case);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("无法检查灵感库图片缓存：{error}")),
            };
            if !metadata.file_type().is_file() {
                continue;
            }
            fs::remove_file(&path).map_err(|error| format!("无法清理灵感库图片缓存：{error}"))?;
            removed_bytes = removed_bytes.saturating_add(metadata.len());
            removed_count += 1;
        }
        Ok(json!({
            "removedBytes": removed_bytes,
            "removedCount": removed_count,
            "cache": self.cache_status()
        }))
    }

    fn case(&self, case_id: &str) -> Result<CaseAsset, String> {
        self.cases
            .get(case_id)
            .cloned()
            .ok_or_else(|| "未登记的灵感案例".into())
    }

    fn cache_path(&self, case: &CaseAsset) -> PathBuf {
        self.cache_directory.join(&case.filename)
    }

    fn cache_usage(&self) -> (u64, u64) {
        let mut bytes = 0_u64;
        let mut count = 0_u64;
        for case in self.cases.values() {
            let path = self.cache_path(case);
            if let Ok(metadata) = fs::symlink_metadata(path)
                && metadata.file_type().is_file()
            {
                bytes = bytes.saturating_add(metadata.len());
                count += 1;
            }
        }
        (bytes, count)
    }

    fn download_image(&self, case: &CaseAsset) -> Result<(), String> {
        let url = format!(
            "https://raw.githubusercontent.com/freestylefly/awesome-gpt-image-2/{}/{}",
            "63e6176fa01be51c000108dacd72716d3e46784b", case.image_path
        );
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| format!("示例图片下载失败：{error}"))?;
        validate_final_url(&response)?;
        if !response.status().is_success() {
            return Err(format!("示例图片下载失败：HTTP {}", response.status()));
        }
        if let Some(length) = response.content_length()
            && length > MAX_IMAGE_BYTES
        {
            return Err("示例图片超过 16 MiB 下载上限".into());
        }
        let mut limited = response.take(MAX_IMAGE_BYTES.saturating_add(1));
        let mut bytes = Vec::with_capacity(usize::try_from(case.bytes).unwrap_or(0));
        limited
            .read_to_end(&mut bytes)
            .map_err(|error| format!("示例图片读取失败：{error}"))?;
        if bytes.len() as u64 != case.bytes {
            return Err("示例图片大小与登记清单不符".into());
        }
        validate_image_bytes(&bytes, case)?;
        self.evict_for(case, bytes.len() as u64)?;
        let target = self.cache_path(case);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = self.cache_directory.join(format!(
            ".prompt-library-{}-{stamp}.part",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("无法创建灵感库图片临时文件：{error}"))?;
        if let Err(error) = file
            .write_all(&bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::rename(&temporary, &target))
        {
            let _ = fs::remove_file(&temporary);
            return Err(format!("无法提交灵感库图片缓存：{error}"));
        }
        Ok(())
    }

    fn evict_for(&self, incoming: &CaseAsset, incoming_bytes: u64) -> Result<(), String> {
        let (mut total, _) = self.cache_usage();
        if total.saturating_add(incoming_bytes) <= CACHE_LIMIT_BYTES {
            return Ok(());
        }
        let protected = self.cache_path(incoming);
        let mut candidates = self
            .cases
            .values()
            .filter_map(|case| {
                let path = self.cache_path(case);
                if path == protected {
                    return None;
                }
                let metadata = fs::symlink_metadata(&path).ok()?;
                if !metadata.file_type().is_file() {
                    return None;
                }
                let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
                Some((path, metadata.len(), modified))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, _, modified)| *modified);
        for (path, bytes, _) in candidates {
            fs::remove_file(&path).map_err(|error| format!("无法淘汰灵感库旧缓存：{error}"))?;
            total = total.saturating_sub(bytes);
            if total.saturating_add(incoming_bytes) <= CACHE_LIMIT_BYTES {
                return Ok(());
            }
        }
        Err("清理旧缓存后仍无法容纳示例图片".into())
    }
}

fn catalog_cases(catalog: &Value) -> Result<HashMap<String, CaseAsset>, String> {
    if catalog.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("灵感库目录 schemaVersion 不受支持".into());
    }
    if catalog.get("upstreamCommit").and_then(Value::as_str)
        != Some("63e6176fa01be51c000108dacd72716d3e46784b")
    {
        return Err("灵感库目录上游提交不受支持".into());
    }
    let cases = catalog
        .get("cases")
        .and_then(Value::as_array)
        .ok_or("灵感库目录缺少 cases")?;
    if cases.len() != 541 {
        return Err("灵感库目录案例数不是 541".into());
    }
    let categories = catalog
        .get("categories")
        .and_then(Value::as_array)
        .ok_or("灵感库目录缺少分类")?;
    if categories.len() != 13 {
        return Err("灵感库目录分类数不是 13".into());
    }
    let mut result = HashMap::with_capacity(cases.len());
    let mut source_ids = HashSet::with_capacity(cases.len());
    let mut filenames = HashSet::with_capacity(cases.len());
    for case in cases {
        let id = case
            .get("id")
            .and_then(Value::as_str)
            .ok_or("灵感库案例缺少 ID")?;
        let source_case_id = case
            .get("sourceCaseId")
            .and_then(Value::as_u64)
            .ok_or("灵感库案例缺少 sourceCaseId")?;
        let image = case
            .get("image")
            .and_then(Value::as_object)
            .ok_or("灵感库案例缺少图片信息")?;
        let image_path = image
            .get("path")
            .and_then(Value::as_str)
            .ok_or("灵感库图片缺少路径")?;
        let filename = image
            .get("filename")
            .and_then(Value::as_str)
            .ok_or("灵感库图片缺少文件名")?;
        let sha256 = image
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or("灵感库图片缺少散列")?;
        let bytes = image
            .get("bytes")
            .and_then(Value::as_u64)
            .ok_or("灵感库图片缺少字节数")?;
        let mime_type = image
            .get("mimeType")
            .and_then(Value::as_str)
            .ok_or("灵感库图片缺少 MIME")?;
        let width = image
            .get("width")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("灵感库图片缺少有效宽度")?;
        let height = image
            .get("height")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("灵感库图片缺少有效高度")?;
        if !image_path.starts_with("data/images/")
            || Path::new(filename)
                .file_name()
                .and_then(|value| value.to_str())
                != Some(filename)
            || filename.starts_with('.')
            || sha256.len() != 64
            || !sha256.bytes().all(|value| value.is_ascii_hexdigit())
            || bytes == 0
            || bytes > MAX_IMAGE_BYTES
            || !matches!(mime_type, "image/jpeg" | "image/png")
            || width == 0
            || height == 0
            || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
        {
            return Err(format!("灵感库案例 {source_case_id} 图片元数据无效"));
        }
        if !source_ids.insert(source_case_id) || !filenames.insert(filename.to_owned()) {
            return Err("灵感库案例 ID 或图片映射重复".into());
        }
        let case = CaseAsset {
            id: id.to_owned(),
            source_case_id,
            image_path: image_path.to_owned(),
            filename: filename.to_owned(),
            sha256: sha256.to_ascii_lowercase(),
            bytes,
            mime_type: mime_type.to_owned(),
            width,
            height,
        };
        if result.insert(id.to_owned(), case).is_some() {
            return Err("灵感库案例 ID 重复".into());
        }
    }
    Ok(result)
}

fn load_favorites(path: &Path) -> FavoriteState {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return FavoriteState {
                ids: Vec::new(),
                load_error: None,
                save_error: None,
            };
        }
        Err(error) => {
            return FavoriteState {
                ids: Vec::new(),
                load_error: Some(format!("无法读取灵感库收藏：{error}")),
                save_error: None,
            };
        }
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_FAVORITES_BYTES {
        return FavoriteState {
            ids: Vec::new(),
            load_error: Some("灵感库收藏路径不是有效的小型普通文件".into()),
            save_error: None,
        };
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return FavoriteState {
                ids: Vec::new(),
                load_error: Some(format!("无法读取灵感库收藏：{error}")),
                save_error: None,
            };
        }
    };
    let document: FavoritesDocument = match serde_json::from_slice(&bytes) {
        Ok(document) => document,
        Err(error) => {
            return FavoriteState {
                ids: Vec::new(),
                load_error: Some(format!("灵感库收藏 JSON 无效：{error}")),
                save_error: None,
            };
        }
    };
    if document.schema_version != STATE_SCHEMA_VERSION
        || document.favorites.len() > 541
        || document
            .favorites
            .iter()
            .any(|id| id.is_empty() || id.len() > 128)
    {
        return FavoriteState {
            ids: Vec::new(),
            load_error: Some("灵感库收藏版本或内容无效".into()),
            save_error: None,
        };
    }
    let mut seen = HashSet::new();
    let ids = document
        .favorites
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect();
    FavoriteState {
        ids,
        load_error: None,
        save_error: None,
    }
}

fn sort_favorites(ids: &mut [String], cases: &HashMap<String, CaseAsset>) {
    ids.sort_by_key(|id| {
        cases
            .get(id)
            .map(|case| case.source_case_id)
            .unwrap_or(u64::MAX)
    });
}

fn write_favorites(path: &Path, document: &FavoritesDocument) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| format!("无法编码灵感库收藏：{error}"))?;
    let parent = path.parent().ok_or("灵感库收藏缺少父目录")?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".prompt-library-state-{}-{stamp}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("无法创建灵感库收藏临时文件：{error}"))?;
    if let Err(error) = file
        .write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, path))
    {
        let _ = fs::remove_file(&temporary);
        return Err(format!("无法保存灵感库收藏：{error}"));
    }
    Ok(())
}

fn validate_final_url(response: &Response) -> Result<(), String> {
    let url = response.url();
    if url.scheme() != "https" || url.host_str() != Some("raw.githubusercontent.com") {
        return Err("示例图片重定向到了未登记来源".into());
    }
    Ok(())
}

fn validate_image_bytes(bytes: &[u8], case: &CaseAsset) -> Result<(), String> {
    if bytes.len() as u64 != case.bytes {
        return Err("图片字节数与登记清单不符".into());
    }
    let digest = Sha256::digest(bytes);
    if format!("{digest:x}") != case.sha256 {
        return Err("图片散列与登记清单不符".into());
    }
    if case.mime_type == "image/jpeg" {
        if !bytes.starts_with(b"\xff\xd8") || !bytes.trim_ascii_end().ends_with(b"\xff\xd9") {
            return Err("图片来源不是完整 JPEG".into());
        }
    } else if !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || !bytes.ends_with(b"\x00\x00\x00\x00IEND\xaeB`\x82")
    {
        return Err("图片来源不是完整 PNG".into());
    }
    Ok(())
}

fn touch(path: &Path) {
    if let Ok(file) = fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, AppPaths, PromptLibraryStore) {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        let store = PromptLibraryStore::load(&paths).unwrap();
        (root, paths, store)
    }

    #[test]
    fn built_in_catalog_is_complete_and_rejects_unknown_ids() {
        let (_root, _paths, store) = fixture();
        assert_eq!(store.cases.len(), 541);
        let value = store.snapshot();
        assert_eq!(value["catalog"]["cases"].as_array().unwrap().len(), 541);
        assert_eq!(value["catalog"]["categories"].as_array().unwrap().len(), 13);
        assert!(store.ensure_image("not-a-case").is_err());
        assert!(store.set_favorite("not-a-case", true).is_err());
    }

    #[test]
    fn catalog_validation_rejects_duplicate_and_incomplete_entries() {
        let mut catalog: Value = serde_json::from_str(CATALOG_JSON).unwrap();
        let duplicate = catalog["cases"][0]["id"].clone();
        catalog["cases"][1]["id"] = duplicate;
        assert!(catalog_cases(&catalog).is_err());

        let mut incomplete: Value = serde_json::from_str(CATALOG_JSON).unwrap();
        incomplete["cases"].as_array_mut().unwrap().pop();
        assert!(catalog_cases(&incomplete).is_err());
    }

    #[test]
    fn favorites_are_atomic_and_unknown_removed_ids_are_preserved() {
        let (_root, paths, store) = fixture();
        let id = "awesome-gpt-image-2:case:544";
        store.set_favorite(id, true).unwrap();
        drop(store);

        let orphan = "awesome-gpt-image-2:case:removed";
        let document = FavoritesDocument {
            schema_version: STATE_SCHEMA_VERSION,
            favorites: vec![id.into(), orphan.into()],
        };
        write_favorites(&paths.data.join(STATE_FILE), &document).unwrap();
        let reloaded = PromptLibraryStore::load(&paths).unwrap();
        let snapshot = reloaded.snapshot();
        let favorites = snapshot["favorites"].as_array().unwrap();
        assert!(favorites.iter().any(|value| value == id));
        assert!(favorites.iter().any(|value| value == orphan));
    }

    #[test]
    fn corrupt_favorites_are_reported_without_overwriting_the_file() {
        let (_root, paths, _store) = fixture();
        let path = paths.data.join(STATE_FILE);
        fs::write(&path, b"{not-json").unwrap();
        let reloaded = PromptLibraryStore::load(&paths).unwrap();
        let snapshot = reloaded.snapshot();
        assert!(snapshot["favoritesLoadError"].as_str().is_some());
        assert_eq!(fs::read(&path).unwrap(), b"{not-json");
    }

    #[test]
    fn cache_requires_exact_catalog_hash() {
        let (_root, _paths, mut store) = fixture();
        let id = "awesome-gpt-image-2:case:544";
        let png = include_bytes!("../../../../resources/brand/app-icon-32.png");
        let cache_path = {
            let case = store.cases.get_mut(id).unwrap();
            case.bytes = png.len() as u64;
            case.sha256 = format!("{:x}", Sha256::digest(png));
            case.mime_type = "image/png".into();
            case.width = 32;
            case.height = 32;
            store.cache_directory.join(&case.filename)
        };
        fs::write(&cache_path, png).unwrap();
        assert!(store.read_cached_image(id).is_ok());
        let mut changed = png.to_vec();
        changed[20] ^= 1;
        fs::write(&cache_path, changed).unwrap();
        assert!(store.read_cached_image(id).is_err());
    }

    #[test]
    fn cache_clear_leaves_unrelated_files_untouched() {
        let (_root, _paths, store) = fixture();
        let case = store.case("awesome-gpt-image-2:case:544").unwrap();
        fs::write(store.cache_path(&case), b"partial").unwrap();
        let unrelated = store.cache_directory.join("unrelated.txt");
        fs::write(&unrelated, b"keep").unwrap();
        store.clear_cache().unwrap();
        assert!(!store.cache_path(&case).exists());
        assert!(unrelated.exists());
    }

    #[test]
    #[ignore = "requires the fixed GitHub image source and network access"]
    fn real_image_download_matches_the_catalog() {
        let (_root, _paths, store) = fixture();
        let id = "awesome-gpt-image-2:case:544";
        store.ensure_image(id).unwrap();
        let (bytes, mime) = store.read_cached_image(id).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert_eq!(bytes.len() as u64, 113_443);
    }
}
