//! Official GitHub Release update discovery and verified installer download.
//!
//! This module intentionally has no arbitrary URL or executable input. The frontend selects
//! only the latest stable release from the fixed repository; the host resolves the exact
//! platform asset, verifies its SHA-256, and exposes only controlled platform actions.

use reqwest::Url;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const UPDATE_PROGRESS_EVENT: &str = "app-update-progress";
const RELEASES_API: &str =
    "https://api.github.com/repos/Goldloli/image-studio/releases?per_page=20";
const RELEASES_PAGE: &str = "https://github.com/Goldloli/image-studio/releases";
const USER_AGENT: &str = "Image Studio desktop update checker";
const CANCELLED: &str = "__IMAGE_STUDIO_UPDATE_CANCELLED__";
const MAX_RELEASES_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHECKSUM_BYTES: usize = 256 * 1024;
const MAX_INSTALLER_BYTES: u64 = 1024 * 1024 * 1024;
const RESPONSE_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionInfo {
    pub version: String,
    pub target: String,
    pub installer_kind: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRelease {
    pub version: String,
    pub tag_name: String,
    pub name: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub html_url: String,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub update_available: bool,
    pub checked_at_ms: u64,
    pub release: Option<UpdateRelease>,
    pub releases_url: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadState {
    pub status: &'static str,
    pub version: Option<String>,
    pub filename: Option<String>,
    pub path: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub progress: Option<f64>,
    pub bytes_per_second: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub error: Option<String>,
}

impl Default for UpdateDownloadState {
    fn default() -> Self {
        Self {
            status: "idle",
            version: None,
            filename: None,
            path: None,
            downloaded_bytes: 0,
            total_bytes: None,
            progress: None,
            bytes_per_second: None,
            eta_seconds: None,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Clone, Debug)]
struct ResolvedRelease {
    version: Version,
    source: GitHubRelease,
}

#[derive(Clone)]
pub struct UpdateManager {
    cache_dir: PathBuf,
    state: Arc<Mutex<UpdateDownloadState>>,
    cancel: Arc<AtomicBool>,
    pending_install: Arc<Mutex<Option<PathBuf>>>,
    current_release: Arc<Mutex<Option<String>>>,
}

impl UpdateManager {
    pub fn new(cache_root: &Path) -> Self {
        Self {
            cache_dir: cache_root.join("updates"),
            state: Arc::new(Mutex::new(UpdateDownloadState::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            pending_install: Arc::new(Mutex::new(None)),
            current_release: Arc::new(Mutex::new(None)),
        }
    }

    pub fn version_info() -> AppVersionInfo {
        AppVersionInfo {
            version: env!("CARGO_PKG_VERSION").into(),
            target: target_label().into(),
            installer_kind: installer_kind(),
        }
    }

    pub fn state(&self) -> Result<UpdateDownloadState, String> {
        self.state
            .lock()
            .map(|value| value.clone())
            .map_err(|error| error.to_string())
    }

    pub async fn check(&self, current_version: &str) -> Result<UpdateCheckResult, String> {
        let current = Version::parse(current_version)
            .map_err(|error| format!("当前应用版本无效：{error}"))?;
        let releases = fetch_releases().await?;
        let selected = select_release(&releases, current_version)?;
        if let Some(candidate) = &selected {
            validate_release_assets(candidate)?;
        }
        *self
            .current_release
            .lock()
            .map_err(|error| error.to_string())? = selected
            .as_ref()
            .map(|candidate| candidate.version.to_string());
        Ok(UpdateCheckResult {
            current_version: current_version.to_string(),
            update_available: selected
                .as_ref()
                .is_some_and(|candidate| candidate.version > current),
            checked_at_ms: now_ms(),
            release: selected.as_ref().map(public_release),
            releases_url: RELEASES_PAGE,
        })
    }

    pub fn start_download(
        &self,
        app: AppHandle,
        current_version: String,
        requested_version: String,
    ) -> Result<UpdateDownloadState, String> {
        Version::parse(requested_version.trim_start_matches('v'))
            .map_err(|error| format!("更新版本无效：{error}"))?;
        let initial = {
            let mut snapshot = self.state.lock().map_err(|error| error.to_string())?;
            if matches!(
                snapshot.status,
                "preparing" | "downloading" | "verifying" | "cancelling"
            ) {
                return Err("已有更新正在下载或校验，请先等待或取消".into());
            }
            *snapshot = UpdateDownloadState {
                status: "preparing",
                version: Some(requested_version.clone()),
                ..UpdateDownloadState::default()
            };
            snapshot.clone()
        };
        self.cancel.store(false, Ordering::Release);
        app.emit(UPDATE_PROGRESS_EVENT, initial.clone())
            .map_err(|error| error.to_string())?;
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            let downloaded = manager
                .download(&app, &current_version, &requested_version)
                .await;
            manager.cancel.store(false, Ordering::Release);
            match downloaded {
                Ok(()) => {}
                Err(error) if error == CANCELLED => {
                    let _ = manager.mutate(&app, |snapshot| {
                        snapshot.status = "cancelled";
                        snapshot.progress = None;
                        snapshot.bytes_per_second = None;
                        snapshot.eta_seconds = None;
                        snapshot.error = None;
                    });
                }
                Err(error) => {
                    let _ = manager.mutate(&app, |snapshot| {
                        snapshot.status = "error";
                        snapshot.progress = None;
                        snapshot.bytes_per_second = None;
                        snapshot.eta_seconds = None;
                        snapshot.error = Some(error);
                    });
                }
            }
        });
        Ok(initial)
    }

    async fn download(
        &self,
        app: &AppHandle,
        current_version: &str,
        requested_version: &str,
    ) -> Result<(), String> {
        let releases = fetch_releases().await?;
        let selected = select_release(&releases, current_version)?
            .ok_or_else(|| "没有可用的稳定版更新".to_string())?;
        if selected.version.to_string() != requested_version.trim_start_matches('v') {
            return Err("请求下载的版本已不是当前稳定版，请重新检查更新".into());
        }
        let installer = release_asset(&selected, false)?;
        let checksums = release_asset(&selected, true)?;
        if installer.size == 0 || installer.size > MAX_INSTALLER_BYTES {
            return Err("更新安装包大小无效".into());
        }
        validate_asset_url(&installer.browser_download_url)?;
        validate_asset_url(&checksums.browser_download_url)?;
        let checksum_text =
            download_text(&checksums.browser_download_url, MAX_CHECKSUM_BYTES).await?;
        let expected = parse_checksum(&checksum_text, &installer.name)
            .ok_or_else(|| format!("checksum 文件缺少 {}", installer.name))?;
        tokio::fs::create_dir_all(&self.cache_dir)
            .await
            .map_err(|error| format!("无法创建更新缓存：{error}"))?;
        let final_path = self.cache_dir.join(&installer.name);
        let part_path = self.cache_dir.join(format!("{}.part", installer.name));
        remove_file_if_present(&part_path, "旧临时文件").await?;
        if is_regular_file_without_symlink(&final_path) {
            if hash_file(&final_path).await? == expected {
                self.mutate(app, |snapshot| {
                    snapshot.status = "completed";
                    snapshot.version = Some(selected.version.to_string());
                    snapshot.filename = Some(installer.name.clone());
                    snapshot.path = Some(final_path.to_string_lossy().into_owned());
                    snapshot.downloaded_bytes = installer.size;
                    snapshot.total_bytes = Some(installer.size);
                    snapshot.progress = Some(1.0);
                    snapshot.bytes_per_second = None;
                    snapshot.eta_seconds = Some(0);
                    snapshot.error = None;
                })?;
                return Ok(());
            }
            remove_file_if_present(&final_path, "未校验安装包").await?;
        }
        self.mutate(app, |snapshot| {
            snapshot.status = "downloading";
            snapshot.version = Some(selected.version.to_string());
            snapshot.filename = Some(installer.name.clone());
            snapshot.path = None;
            snapshot.downloaded_bytes = 0;
            snapshot.total_bytes = Some(installer.size);
            snapshot.progress = Some(0.0);
            snapshot.bytes_per_second = None;
            snapshot.eta_seconds = None;
            snapshot.error = None;
        })?;
        if let Err(error) = download_file(app, &installer, &part_path, &expected).await {
            return match remove_file_if_present(&part_path, "临时安装包").await {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(format!("{error}；{cleanup_error}")),
            };
        }
        remove_file_if_present(&final_path, "目标安装包").await?;
        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(|error| format!("无法提交更新安装包：{error}"))?;
        self.mutate(app, |snapshot| {
            snapshot.status = "completed";
            snapshot.path = Some(final_path.to_string_lossy().into_owned());
            snapshot.downloaded_bytes = installer.size;
            snapshot.total_bytes = Some(installer.size);
            snapshot.progress = Some(1.0);
            snapshot.bytes_per_second = None;
            snapshot.eta_seconds = Some(0);
            snapshot.error = None;
        })?;
        Ok(())
    }

    pub fn cancel(&self, app: &AppHandle) -> Result<UpdateDownloadState, String> {
        let status = self.state()?.status;
        if !matches!(status, "preparing" | "downloading" | "verifying") {
            return self.state();
        }
        self.cancel.store(true, Ordering::Release);
        self.mutate(app, |snapshot| {
            snapshot.status = "cancelling";
            snapshot.bytes_per_second = None;
            snapshot.eta_seconds = None;
        })
    }

    pub fn completed_path(&self) -> Result<PathBuf, String> {
        let snapshot = self.state()?;
        if snapshot.status != "completed" {
            return Err("没有已完成校验的更新安装包".into());
        }
        let path = snapshot
            .path
            .as_deref()
            .map(PathBuf::from)
            .ok_or("更新安装包路径缺失")?;
        let version = snapshot
            .version
            .as_deref()
            .and_then(parse_release_version)
            .ok_or("更新版本信息缺失")?;
        let current_release = self
            .current_release
            .lock()
            .map_err(|error| error.to_string())?
            .clone()
            .ok_or("请先检查更新")?;
        if version.to_string() != current_release {
            return Err("已完成安装包不是当前稳定版，请重新检查或下载".into());
        }
        if !is_expected_asset(
            path.file_name().and_then(OsStr::to_str).unwrap_or_default(),
            &version,
        ) {
            return Err("更新安装包文件名与版本不匹配".into());
        }
        if !is_regular_file_without_symlink(&path) {
            return Err("更新安装包不存在或不是普通文件".into());
        }
        Ok(path)
    }

    pub fn mark_pending_install(&self) -> Result<(), String> {
        let path = self.completed_path()?;
        if !installer_kind().is_some_and(|kind| kind == "nsis") {
            return Err("当前平台请手动打开已验证的更新包".into());
        }
        *self
            .pending_install
            .lock()
            .map_err(|error| error.to_string())? = Some(path);
        Ok(())
    }

    pub fn clear_pending_install(&self) {
        if let Ok(mut pending) = self.pending_install.lock() {
            pending.take();
        }
    }

    pub fn take_pending_install(&self) -> Option<PathBuf> {
        self.pending_install.lock().ok()?.take()
    }

    pub fn open_installer(&self) -> Result<(), String> {
        let path = self.completed_path()?;
        if installer_kind() != Some("dmg") {
            return Err("Windows 请在更新中心使用“安装并重启”".into());
        }
        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("open")
                .arg(&path)
                .status()
                .map_err(|error| format!("无法打开更新安装包：{error}"))?;
            if status.success() {
                Ok(())
            } else {
                Err("系统未能打开更新安装包".into())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("当前平台不通过系统预览打开安装包".into())
        }
    }

    pub fn open_location(&self) -> Result<(), String> {
        let path = self.completed_path()?;
        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("open")
                .arg("-R")
                .arg(&path)
                .status()
                .map_err(|error| format!("无法在 Finder 中定位更新安装包：{error}"))?;
            if status.success() {
                Ok(())
            } else {
                Err("Finder 未能定位更新安装包".into())
            }
        }
        #[cfg(windows)]
        {
            crate::adapters::quiet_command("explorer.exe")
                .arg("/select,")
                .arg(&path)
                .spawn()
                .map_err(|error| format!("无法在资源管理器中定位更新安装包：{error}"))?;
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = path;
            Err("当前平台不支持定位更新安装包".into())
        }
    }

    fn mutate(
        &self,
        app: &AppHandle,
        update: impl FnOnce(&mut UpdateDownloadState),
    ) -> Result<UpdateDownloadState, String> {
        let next = {
            let mut snapshot = self.state.lock().map_err(|error| error.to_string())?;
            update(&mut snapshot);
            snapshot.clone()
        };
        app.emit(UPDATE_PROGRESS_EVENT, next.clone())
            .map_err(|error| error.to_string())?;
        Ok(next)
    }
}

async fn fetch_releases() -> Result<Vec<GitHubRelease>, String> {
    let response = github_client()?
        .get(RELEASES_API)
        .timeout(Duration::from_secs(30))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| format!("无法检查 GitHub Releases：{error}"))?
        .error_for_status()
        .map_err(|error| format!("检查 GitHub Releases 失败：{error}"))?;
    validate_response_url(response.url())?;
    let bytes = read_bounded_bytes(response, MAX_RELEASES_BYTES, "Release 列表").await?;
    serde_json::from_slice(&bytes).map_err(|error| format!("GitHub Release 响应无法解析：{error}"))
}

async fn download_text(url: &str, maximum: usize) -> Result<String, String> {
    let response = github_client()?
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| format!("无法下载 checksum：{error}"))?
        .error_for_status()
        .map_err(|error| format!("下载 checksum 失败：{error}"))?;
    validate_response_url(response.url())?;
    let bytes = read_bounded_bytes(response, maximum, "checksum").await?;
    String::from_utf8(bytes).map_err(|error| format!("checksum 不是 UTF-8：{error}"))
}

async fn download_file(
    app: &AppHandle,
    asset: &GitHubAsset,
    part_path: &Path,
    expected_checksum: &str,
) -> Result<(), String> {
    let mut response = github_client()?
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|error| format!("无法下载更新安装包：{error}"))?
        .error_for_status()
        .map_err(|error| format!("下载更新安装包失败：{error}"))?;
    validate_response_url(response.url())?;
    let total = response
        .content_length()
        .or((asset.size > 0).then_some(asset.size));
    if response
        .content_length()
        .is_some_and(|length| length != asset.size)
    {
        return Err("更新安装包大小与 Release 记录不一致".into());
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part_path)
        .await
        .map_err(|error| format!("无法创建临时安装包：{error}"))?;
    let mut hasher = Sha256::new();
    let started = Instant::now();
    let mut last_emit = started - Duration::from_secs(1);
    let mut downloaded = 0_u64;
    while let Some(chunk) = tokio::time::timeout(RESPONSE_IDLE_TIMEOUT, response.chunk())
        .await
        .map_err(|_| "更新下载长时间没有响应".to_string())?
        .map_err(|error| format!("更新下载中断：{error}"))?
    {
        ensure_not_cancelled(app)?;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("无法写入更新安装包：{error}"))?;
        hasher.update(&chunk);
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > asset.size {
            return Err("更新安装包超出 Release 记录大小".into());
        }
        if last_emit.elapsed() >= Duration::from_millis(250) {
            emit_progress(app, downloaded, total, started.elapsed())?;
            last_emit = Instant::now();
        }
    }
    ensure_not_cancelled(app)?;
    if downloaded != asset.size {
        return Err("更新安装包下载字节数与 Release 记录不一致".into());
    }
    file.flush()
        .await
        .map_err(|error| format!("无法刷新更新安装包：{error}"))?;
    emit_progress(app, downloaded, total, started.elapsed())?;
    mutate_external_state(app, |snapshot| {
        snapshot.status = "verifying";
        snapshot.bytes_per_second = None;
        snapshot.eta_seconds = None;
    })?;
    ensure_not_cancelled(app)?;
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected_checksum {
        return Err(format!(
            "SHA-256 校验失败：期望 {expected_checksum}，实际 {actual}"
        ));
    }
    Ok(())
}

fn emit_progress(
    app: &AppHandle,
    downloaded: u64,
    total: Option<u64>,
    elapsed: Duration,
) -> Result<(), String> {
    let speed = (elapsed.as_secs_f64() > 0.0).then(|| downloaded as f64 / elapsed.as_secs_f64());
    let progress = total
        .filter(|value| *value > 0)
        .map(|value| downloaded as f64 / value as f64);
    let eta = match (total, speed) {
        (Some(total), Some(speed)) if speed > 0.0 && total > downloaded => {
            Some(((total - downloaded) as f64 / speed).ceil() as u64)
        }
        (Some(_), Some(_)) => Some(0),
        _ => None,
    };
    mutate_external_state(app, |snapshot| {
        snapshot.status = "downloading";
        snapshot.downloaded_bytes = downloaded;
        snapshot.total_bytes = total;
        snapshot.progress = progress;
        snapshot.bytes_per_second = speed;
        snapshot.eta_seconds = eta;
    })
}

fn mutate_external_state(
    app: &AppHandle,
    update: impl FnOnce(&mut UpdateDownloadState),
) -> Result<(), String> {
    // The update manager's snapshot is resolved through Tauri state, matching all commands.
    if let Some(manager) = app.try_state::<crate::host::HostState>() {
        manager.updates().mutate(app, update)?;
    }
    Ok(())
}

fn ensure_not_cancelled(app: &AppHandle) -> Result<(), String> {
    if app
        .try_state::<crate::host::HostState>()
        .is_some_and(|state| state.updates().cancel.load(Ordering::Acquire))
    {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

fn select_release(
    releases: &[GitHubRelease],
    current_version: &str,
) -> Result<Option<ResolvedRelease>, String> {
    let current =
        Version::parse(current_version).map_err(|error| format!("当前应用版本无效：{error}"))?;
    Ok(releases
        .iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|source| {
            Version::parse(source.tag_name.trim_start_matches('v'))
                .ok()
                .filter(|version| version > &current)
                .map(|version| ResolvedRelease {
                    version,
                    source: source.clone(),
                })
        })
        .max_by(|left, right| left.version.cmp(&right.version)))
}

fn release_asset(release: &ResolvedRelease, checksum: bool) -> Result<GitHubAsset, String> {
    let name = if checksum {
        checksum_asset_name()
    } else {
        expected_asset_name(&release.version)?
    };
    release
        .source
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .cloned()
        .ok_or_else(|| format!("Release {} 缺少 {name}", release.version))
}

fn validate_release_assets(release: &ResolvedRelease) -> Result<(), String> {
    let installer = release_asset(release, false)?;
    let checksums = release_asset(release, true)?;
    if installer.size == 0 || installer.size > MAX_INSTALLER_BYTES {
        return Err("更新安装包大小无效".into());
    }
    if checksums.size == 0 || checksums.size > MAX_CHECKSUM_BYTES as u64 {
        return Err("更新校验文件大小无效".into());
    }
    validate_asset_url(&installer.browser_download_url)?;
    validate_asset_url(&checksums.browser_download_url)?;
    Ok(())
}

fn public_release(release: &ResolvedRelease) -> UpdateRelease {
    let asset = release_asset(release, false).ok();
    UpdateRelease {
        version: release.version.to_string(),
        tag_name: release.source.tag_name.clone(),
        name: release
            .source
            .name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| release.source.tag_name.clone()),
        notes: plain_text_summary(release.source.body.as_deref().unwrap_or("")),
        published_at: release.source.published_at.clone(),
        html_url: trusted_release_page(&release.source.html_url),
        asset_name: asset.as_ref().map(|item| item.name.clone()),
        asset_size: asset.map(|item| item.size),
    }
}

fn plain_text_summary(value: &str) -> String {
    let mut output = value
        .lines()
        .map(|line| line.trim().trim_start_matches(['#', '>', '-', '*']).trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if output.chars().count() > 600 {
        output = output.chars().take(599).collect::<String>() + "…";
    }
    output
}

fn target_label() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macOS Apple Silicon"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "Windows x64"
    } else {
        "Desktop"
    }
}

fn installer_kind() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("dmg")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("nsis")
    } else {
        None
    }
}

fn checksum_asset_name() -> String {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "SHA256SUMS-macos.txt".into()
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "SHA256SUMS-windows.txt".into()
    } else {
        String::new()
    }
}

fn expected_asset_name(version: &Version) -> Result<String, String> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Ok(format!("Image-Studio-{version}-macos-arm64.dmg"))
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Ok(format!("Image.Studio_{version}_x64-setup.exe"))
    } else {
        Err("当前平台未提供应用自动更新".into())
    }
}

fn is_expected_asset(name: &str, version: &Version) -> bool {
    expected_asset_name(version).is_ok_and(|expected| name == expected)
}

fn validate_asset_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|error| format!("更新资产 URL 无效：{error}"))?;
    if url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url
            .path()
            .starts_with("/Goldloli/image-studio/releases/download/")
    {
        Ok(())
    } else {
        Err("更新资产 URL 不在受信任的官方 Release 路径".into())
    }
}

fn validate_response_url(url: &Url) -> Result<(), String> {
    let host = url.host_str().unwrap_or_default();
    if url.scheme() == "https"
        && (host == "github.com"
            || host == "api.github.com"
            || host == "objects.githubusercontent.com"
            || host.ends_with(".githubusercontent.com"))
    {
        Ok(())
    } else {
        Err("更新请求跳转到不受信任的主机".into())
    }
}

fn parse_checksum(contents: &str, filename: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let digest = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if parts.next().is_none()
            && name == filename
            && digest.len() == 64
            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            Some(digest.to_ascii_lowercase())
        } else {
            None
        }
    })
}

async fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("无法读取已有更新安装包：{error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("无法校验已有更新安装包：{error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

async fn remove_file_if_present(path: &Path, label: &str) -> Result<(), String> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_dir() => Err(format!("{label}路径是目录")),
        Ok(_) => tokio::fs::remove_file(path)
            .await
            .map_err(|error| format!("无法删除{label}：{error}")),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("无法检查{label}：{error}")),
    }
}

fn is_regular_file_without_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

async fn read_bounded_bytes(
    mut response: reqwest::Response,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(format!("{label}响应过大"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = tokio::time::timeout(RESPONSE_IDLE_TIMEOUT, response.chunk())
        .await
        .map_err(|_| format!("{label}下载超时"))?
        .map_err(|error| format!("{label}下载失败：{error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err(format!("{label}响应过大"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn github_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| format!("无法创建 GitHub 客户端：{error}"))
}

fn trusted_release_page(value: &str) -> String {
    let Ok(url) = Url::parse(value) else {
        return RELEASES_PAGE.to_string();
    };
    if url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.path().starts_with("/Goldloli/image-studio/releases/")
    {
        value.to_string()
    } else {
        RELEASES_PAGE.to_string()
    }
}

fn parse_release_version(value: &str) -> Option<Version> {
    Version::parse(value.trim_start_matches('v')).ok()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, value: &str, size: u64) -> GitHubAsset {
        GitHubAsset {
            name: name.into(),
            browser_download_url: value.into(),
            size,
        }
    }

    fn release(tag: &str, prerelease: bool, assets: Vec<GitHubAsset>) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.into(),
            name: Some(format!("Image Studio {tag}")),
            body: Some("## Changes\n- item".into()),
            draft: false,
            prerelease,
            published_at: None,
            html_url: format!("https://github.com/Goldloli/image-studio/releases/tag/{tag}"),
            assets,
        }
    }

    #[test]
    fn stable_selection_ignores_prereleases_and_drafts() {
        let releases = vec![
            release("v0.3.1-beta.1", true, vec![]),
            GitHubRelease {
                draft: true,
                ..release("v0.4.0", false, vec![])
            },
            release("v0.3.0", false, vec![]),
        ];
        let current = "0.2.2";
        let selected = select_release(&releases, current).unwrap().unwrap();
        assert_eq!(selected.version.to_string(), "0.3.0");
    }

    #[test]
    fn asset_names_are_fixed_for_platform_and_version() {
        let version = Version::parse("1.2.3").unwrap();
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(
                expected_asset_name(&version).unwrap(),
                "Image-Studio-1.2.3-macos-arm64.dmg"
            );
            assert_eq!(checksum_asset_name(), "SHA256SUMS-macos.txt");
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            assert_eq!(
                expected_asset_name(&version).unwrap(),
                "Image.Studio_1.2.3_x64-setup.exe"
            );
            assert_eq!(checksum_asset_name(), "SHA256SUMS-windows.txt");
        }
    }

    #[test]
    fn update_discovery_requires_the_current_platform_assets() {
        let version = Version::parse("1.2.3").unwrap();
        let installer_name = expected_asset_name(&version).unwrap();
        let checksums_name = checksum_asset_name();
        let installer_url = format!(
            "https://github.com/Goldloli/image-studio/releases/download/v1.2.3/{installer_name}"
        );
        let checksums_url = format!(
            "https://github.com/Goldloli/image-studio/releases/download/v1.2.3/{checksums_name}"
        );
        let complete = ResolvedRelease {
            version: version.clone(),
            source: release(
                "v1.2.3",
                false,
                vec![
                    asset(&installer_name, &installer_url, 100),
                    asset(&checksums_name, &checksums_url, 100),
                ],
            ),
        };
        assert!(validate_release_assets(&complete).is_ok());

        let missing_checksum = ResolvedRelease {
            version: version.clone(),
            source: release(
                "v1.2.3",
                false,
                vec![asset(&installer_name, &installer_url, 100)],
            ),
        };
        assert!(validate_release_assets(&missing_checksum).is_err());

        let wrong_installer = ResolvedRelease {
            version,
            source: release(
                "v1.2.3",
                false,
                vec![
                    asset("update.dmg", &installer_url, 100),
                    asset(&checksums_name, &checksums_url, 100),
                ],
            ),
        };
        assert!(validate_release_assets(&wrong_installer).is_err());
    }

    #[test]
    fn checksum_parser_requires_exact_filename_and_sha256() {
        let digest = "a".repeat(64);
        let contents = format!("{digest}  Image-Studio-1.2.3-macos-arm64.dmg\n");
        assert_eq!(
            parse_checksum(&contents, "Image-Studio-1.2.3-macos-arm64.dmg"),
            Some(digest)
        );
        assert_eq!(
            parse_checksum(&contents, "./Image-Studio-1.2.3-macos-arm64.dmg"),
            None
        );
        assert_eq!(
            parse_checksum(
                &format!("{}  Image-Studio-1.2.3-macos-arm64.dmg\n", "g".repeat(64)),
                "Image-Studio-1.2.3-macos-arm64.dmg"
            ),
            None
        );
    }

    #[test]
    fn only_official_github_release_urls_are_trusted() {
        assert!(
            validate_asset_url(
                "https://github.com/Goldloli/image-studio/releases/download/v0.3.0/file.dmg"
            )
            .is_ok()
        );
        assert!(
            validate_asset_url(
                "https://github.com/other/image-studio/releases/download/v0.3.0/file.dmg"
            )
            .is_err()
        );
        assert!(
            validate_asset_url(
                "http://github.com/Goldloli/image-studio/releases/download/v0.3.0/file.dmg"
            )
            .is_err()
        );
        assert!(
            validate_response_url(
                &Url::parse(RELEASES_API).expect("fixed GitHub API URL must parse")
            )
            .is_ok()
        );
        assert!(
            validate_response_url(
                &Url::parse("https://api.github.com.evil.example/repos")
                    .expect("test URL must parse")
            )
            .is_err()
        );
    }

    #[test]
    fn plain_text_release_notes_are_bounded() {
        let input = format!("# Title\n- item\n{}", "x".repeat(700));
        let output = plain_text_summary(&input);
        assert!(output.starts_with("Title item"));
        assert!(output.chars().count() <= 600);
        assert!(!output.contains('#'));
    }
}
