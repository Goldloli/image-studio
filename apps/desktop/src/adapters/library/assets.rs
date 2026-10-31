use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufReader, Read, Seek, SeekFrom, Write},
};
use studio_core::recipes::{OutputRole, Recipe};

const MAX_ASSET_BYTES: u64 = 128 * 1024 * 1024;
const DECODE_LIMIT: usize = 256 * 1024 * 1024;
const MAX_ASSET_CHECKS: usize = 512;

/// A lightweight poll may reuse a verified result only while the same file's
/// metadata is unchanged. URI reads and exports still hash the bytes they serve.
#[derive(Clone, Debug, Eq, PartialEq)]
struct AssetFingerprint {
    length: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}
impl AssetFingerprint {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            changed_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}
pub(super) struct CachedAssetCheck {
    fingerprint: AssetFingerprint,
    error: Option<String>,
}
pub(super) type AssetCheckCache = BTreeMap<String, CachedAssetCheck>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDescriptor {
    pub relative_path: String,
    pub width: u32,
    pub height: u32,
    pub byte_count: u64,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct RoleCandidate {
    pub role: String,
    pub candidate: CandidateDescriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommitStage {
    Copied,
    Synced,
    Renamed,
    BeforeDatabaseCommit,
    AfterDatabaseCommit,
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct PublishedAssets {
    paths: Vec<PathBuf>,
    quarantine: PathBuf,
    committed: bool,
}

impl Drop for PublishedAssets {
    fn drop(&mut self) {
        if !self.committed {
            for path in &self.paths {
                let _ = fs::rename(path, self.quarantine.join(new_key("orphan")));
            }
        }
    }
}

struct PreparedOutput {
    role: OutputRole,
    asset: AssetRef,
    temporary: TemporaryFile,
    final_path: PathBuf,
    width: u32,
    height: u32,
    bytes: u64,
}

impl Library {
    /// Trust boundary: only call after the supervisor confirms all compute descendants ended.
    pub fn commit_candidate_after_reap(
        &mut self,
        attempt: &Attempt,
        candidate: CandidateDescriptor,
    ) -> Result<JobRecord> {
        self.commit_candidate_with_hook(attempt, candidate, |_| Ok(()))
    }

    pub(super) fn commit_candidate_with_hook(
        &mut self,
        attempt: &Attempt,
        candidate: CandidateDescriptor,
        hook: impl FnMut(CommitStage) -> Result<()>,
    ) -> Result<JobRecord> {
        self.commit_outputs_with_hook(
            attempt,
            vec![RoleCandidate {
                role: PRIMARY_ROLE.into(),
                candidate,
            }],
            hook,
        )
    }

    /// Commits every frozen recipe output or none. Call only after all compute
    /// descendants have been reaped and the process has exited successfully.
    pub fn commit_outputs_after_reap(
        &mut self,
        attempt: &Attempt,
        candidates: Vec<RoleCandidate>,
    ) -> Result<JobRecord> {
        self.commit_outputs_with_hook(attempt, candidates, |_| Ok(()))
    }

    fn commit_outputs_with_hook(
        &mut self,
        attempt: &Attempt,
        candidates: Vec<RoleCandidate>,
        mut hook: impl FnMut(CommitStage) -> Result<()>,
    ) -> Result<JobRecord> {
        let record = current_job(&self.db, attempt)?;
        commit_eligible(&record)?;
        let recipe = Recipe::from_request(&record.request)?;
        let expected_roles = recipe
            .outputs()
            .iter()
            .map(|role| role.as_str())
            .collect::<BTreeSet<_>>();
        let mut supplied_roles = BTreeSet::new();
        for candidate in &candidates {
            parse_output_role(&candidate.role)?;
            if !supplied_roles.insert(candidate.role.as_str()) {
                return Err(LibraryError::Invalid("候选输出角色重复".into()));
            }
        }
        if supplied_roles != expected_roles || candidates.len() != expected_roles.len() {
            return Err(LibraryError::Invalid(
                "候选输出角色集合与冻结配方不符".into(),
            ));
        }
        let run = self.attempt_run_dir(attempt)?;
        if !fs::symlink_metadata(&run)?.file_type().is_dir() || run.canonicalize()? != run {
            return Err(LibraryError::Invalid("执行目录越界或包含链接".into()));
        }
        let mut prepared = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let role = parse_output_role(&candidate.role)?;
            validate_candidate_relative_path(&candidate.candidate.relative_path)?;
            if recipe == Recipe::ZImageTurbo && candidate.candidate.relative_path != "result.png" {
                return Err(LibraryError::Invalid("v1 候选路径必须为 result.png".into()));
            }
            let expected_dimensions = (
                record.plan.effective_output.width,
                record.plan.effective_output.height,
            );
            if (candidate.candidate.width, candidate.candidate.height) != expected_dimensions {
                return Err(LibraryError::Invalid("候选尺寸不符合冻结计划".into()));
            }
            let max_bytes = recipe
                .max_output_bytes(role)
                .ok_or_else(|| LibraryError::Invalid("候选角色不属于冻结配方".into()))?;
            if candidate.candidate.byte_count == 0 || candidate.candidate.byte_count > max_bytes {
                return Err(LibraryError::Invalid("候选字节数超过配方上限".into()));
            }
            let expected_hash = ContentHash::new(&candidate.candidate.sha256)?;
            let mut source = open_regular(&run.join(&candidate.candidate.relative_path), false)?;
            let before = source.metadata()?;
            if before.len() != candidate.candidate.byte_count || before.len() > max_bytes {
                return Err(LibraryError::Invalid("候选字节数不符".into()));
            }
            let key = new_key("asset");
            let temporary = TemporaryFile(self.root.join("commits").join(format!("{key}.tmp")));
            let mut host_copy = create_private(&temporary.0)?;
            let (bytes, hash) = copy_and_hash(&mut source, &mut host_copy, max_bytes)?;
            let after = source.metadata()?;
            if bytes != candidate.candidate.byte_count
                || hash != expected_hash
                || before.len() != after.len()
                || before.modified().ok() != after.modified().ok()
            {
                return Err(LibraryError::Invalid("候选在读取时改变或散列不符".into()));
            }
            let color_type = png_color_type(&temporary.0)?;
            let data_type = match color_type {
                0 => AssetKind::Mask,
                2 => AssetKind::ImageRgb,
                6 => AssetKind::ImageRgba,
                _ => {
                    return Err(LibraryError::Invalid("候选 PNG 像素类型不符合配方".into()));
                }
            };
            if !recipe.accepts_output(role, data_type.clone()) {
                return Err(LibraryError::Invalid("候选 PNG 类型与输出角色不符".into()));
            }
            let relative_path = format!("assets/{key}.png");
            let asset = AssetRef {
                asset_id: AssetId::new(new_key("asset"))?,
                content_hash: hash,
                data_type,
                location: StorageLocation::Managed {
                    storage_id: StorageLocationId::new("library")?,
                    relative_path: relative_path.clone(),
                },
            };
            prepared.push(PreparedOutput {
                role,
                asset,
                temporary,
                final_path: self.root.join(relative_path),
                width: candidate.candidate.width,
                height: candidate.candidate.height,
                bytes,
            });
        }
        hook(CommitStage::Copied)?;
        for output in &mut prepared {
            let mut file = open_regular(&output.temporary.0, false)?;
            validate_png(
                &mut file,
                output.width,
                output.height,
                max_output_pixels(recipe, output.role),
                output.role,
            )?;
            file.sync_all()?;
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)?;
            file.sync_all()?;
        }
        hook(CommitStage::Synced)?;
        let mut published = PublishedAssets {
            paths: Vec::with_capacity(prepared.len()),
            quarantine: self.root.join("quarantine"),
            committed: false,
        };
        for output in &prepared {
            fs::rename(&output.temporary.0, &output.final_path)?;
            published.paths.push(output.final_path.clone());
        }
        sync_directory(&self.root.join("assets"))?;
        sync_directory(&self.root.join("commits"))?;
        hook(CommitStage::Renamed)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current_job(&tx, attempt)?;
        commit_eligible(&current)?;
        for output in &prepared {
            let StorageLocation::Managed { relative_path, .. } = &output.asset.location else {
                unreachable!("host assets are managed")
            };
            tx.execute("INSERT INTO assets(id,job_id,attempt_id,output_role,asset_json,relative_path,width,height,byte_count,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",params![output.asset.asset_id.as_str(),attempt.job_id.as_str(),attempt.attempt_id.as_str(),output.role.as_str(),serde_json::to_string(&output.asset)?,relative_path,output.width,output.height,sqlite_integer(output.bytes)?,now()])?;
        }
        let outputs = recipe
            .outputs()
            .iter()
            .map(|role| {
                prepared
                    .iter()
                    .find(|output| output.role == *role)
                    .map(|output| output.asset.clone())
                    .ok_or_else(|| LibraryError::Invalid("缺少冻结输出".into()))
            })
            .collect::<Result<Vec<_>>>()?;
        let primary = prepared
            .iter()
            .find(|output| output.role == OutputRole::Primary)
            .ok_or_else(|| LibraryError::Invalid("缺少主图输出".into()))?;
        write_terminal(
            &tx,
            &current,
            AttemptState::Succeeded,
            FinalResult::Succeeded,
            outputs,
            None,
        )?;
        tx.execute(
            "UPDATE projects SET selected_asset_id=? WHERE id='default'",
            [primary.asset.asset_id.as_str()],
        )?;
        hook(CommitStage::BeforeDatabaseCommit)?;
        tx.commit()?;
        published.committed = true;
        hook(CommitStage::AfterDatabaseCommit)?;
        self.job(&attempt.job_id)
    }

    pub fn asset(&self, id: &AssetId) -> Result<AssetRecord> {
        let row=self.db.query_row("SELECT asset_json,job_id,attempt_id,output_role,width,height,byte_count FROM assets WHERE id=?",[id.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get(4)?,r.get(5)?,read_unsigned(r,6)?))).optional()?.ok_or_else(||LibraryError::NotFound(id.to_string()))?;
        let mut record = AssetRecord {
            asset: serde_json::from_str(&row.0)?,
            job_id: JobId::new(row.1)?,
            attempt_id: AttemptId::new(row.2)?,
            output_role: row.3,
            width: row.4,
            height: row.5,
            byte_count: row.6,
            available: true,
            availability_error: None,
        };
        if let Err(error) = self.verify_asset(&record) {
            record.available = false;
            record.availability_error = Some(error.to_string());
        }
        Ok(record)
    }

    pub fn assets(&self, limit: u32, offset: u32) -> Result<Vec<AssetRecord>> {
        let mut statement = self.db.prepare(
            "SELECT id FROM assets WHERE output_role='primary' ORDER BY created_at DESC,rowid DESC LIMIT ? OFFSET ?",
        )?;
        let ids = statement
            .query_map(params![limit.clamp(1, 100), offset], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.asset(&AssetId::new(id)?))
            .collect()
    }

    /// Controlled asset URI handlers receive bytes by registered ID, never a client path.
    pub fn read_asset_png(&self, id: &AssetId) -> Result<Vec<u8>> {
        let record = self.asset(id)?;
        let path = self.asset_path(&record.asset)?;
        let mut file = open_regular(&path, false)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_ASSET_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != record.byte_count
            || bytes.len() as u64 > MAX_ASSET_BYTES
            || digest(&bytes) != record.asset.content_hash
        {
            return Err(LibraryError::Invalid("资产缺失或散列不符".into()));
        }
        Ok(bytes)
    }

    /// `target` comes only from the host's native save dialog. Existing targets require its
    /// explicit overwrite confirmation; all application-managed roots are excluded.
    pub fn export_asset_png(
        &self,
        id: &AssetId,
        target: &Path,
        overwrite_confirmed: bool,
    ) -> Result<()> {
        if !target.is_absolute() {
            return Err(LibraryError::Invalid(
                "导出目标必须为原生对话框的绝对路径".into(),
            ));
        }
        let record = self.asset(id)?;
        self.verify_asset(&record)?;
        let filename = target
            .file_name()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| LibraryError::Invalid("导出文件名无效".into()))?;
        let parent = target
            .parent()
            .ok_or_else(|| LibraryError::Invalid("导出目录无效".into()))?
            .canonicalize()?;
        if self
            .protected_roots
            .iter()
            .any(|root| parent.starts_with(root))
        {
            return Err(LibraryError::Invalid("不可导出到应用管理目录".into()));
        }
        let target = parent.join(filename);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(LibraryError::Invalid("导出目标不可为链接或特殊文件".into()));
            }
            Ok(_) if !overwrite_confirmed => {
                return Err(LibraryError::Invalid("覆盖已有文件需要明确确认".into()));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        let temporary = TemporaryFile(parent.join(format!(
            ".image-studio-export-{}.tmp",
            Uuid::new_v4().simple()
        )));
        let mut destination = create_private(&temporary.0)?;
        let mut source = open_regular(&self.asset_path(&record.asset)?, false)?;
        let (bytes, hash) = copy_and_hash(&mut source, &mut destination, MAX_ASSET_BYTES)?;
        if bytes != record.byte_count || hash != record.asset.content_hash {
            return Err(LibraryError::Invalid("导出源资产散列不符".into()));
        }
        destination.flush()?;
        destination.sync_all()?;
        drop(destination);
        if overwrite_confirmed {
            fs::rename(&temporary.0, &target)?;
        } else {
            // link is an atomic no-replace publish on the destination filesystem.
            // A file created after the dialog check cannot be accidentally overwritten.
            fs::hard_link(&temporary.0, &target)?;
            fs::remove_file(&temporary.0)?;
        }
        sync_directory(&parent)?;
        Ok(())
    }

    pub(super) fn asset_path(&self, asset: &AssetRef) -> Result<PathBuf> {
        asset.validate()?;
        let StorageLocation::Managed {
            storage_id,
            relative_path,
        } = &asset.location
        else {
            return Err(LibraryError::Invalid("资产必须属于本地库".into()));
        };
        if storage_id.as_str() != "library" {
            return Err(LibraryError::Invalid("未知资产存储".into()));
        }
        let name = relative_path
            .strip_prefix("assets/")
            .and_then(|v| v.strip_suffix(".png"))
            .ok_or_else(|| LibraryError::Invalid("资产位置无效".into()))?;
        validate_storage_key(name)?;
        let path = self.root.join(relative_path);
        if path.parent() != Some(self.root.join("assets").as_path()) {
            return Err(LibraryError::Invalid("资产越界".into()));
        }
        Ok(path)
    }

    pub(super) fn verify_asset(&self, record: &AssetRecord) -> Result<()> {
        let mut source = open_regular(&self.asset_path(&record.asset)?, false)?;
        let fingerprint = AssetFingerprint::from_metadata(&source.metadata()?);
        if fingerprint.length != record.byte_count || record.byte_count > MAX_ASSET_BYTES {
            return Err(LibraryError::Invalid("资产大小已改变".into()));
        }
        let id = record.asset.asset_id.to_string();
        // Other platforms lack the Unix inode/ctime combination. Do not reuse
        // an incomplete identity check there; the first release targets macOS.
        #[cfg(unix)]
        if let Some(previous) = self.asset_checks.borrow().get(&id)
            && previous.fingerprint == fingerprint
        {
            return previous
                .error
                .as_ref()
                .map_or(Ok(()), |error| Err(LibraryError::Invalid(error.clone())));
        }
        let result = (|| {
            let (bytes, hash) = hash_reader(&mut source, MAX_ASSET_BYTES)?;
            if bytes != record.byte_count || hash != record.asset.content_hash {
                return Err(LibraryError::Invalid("资产散列不符".into()));
            }
            if fingerprint != AssetFingerprint::from_metadata(&source.metadata()?) {
                return Err(LibraryError::Invalid("资产在校验时发生变化".into()));
            }
            Ok(())
        })();
        let mut checks = self.asset_checks.borrow_mut();
        if !checks.contains_key(&id) && checks.len() >= MAX_ASSET_CHECKS {
            checks.clear();
        }
        checks.insert(
            id,
            CachedAssetCheck {
                fingerprint,
                error: result.as_ref().err().map(ToString::to_string),
            },
        );
        result
    }

    pub(super) fn quarantine_orphans(&self) -> Result<()> {
        let mut references = BTreeSet::new();
        for table in ["assets", "imported_sources"] {
            let mut statement = self
                .db
                .prepare(&format!("SELECT relative_path FROM {table}"))?;
            references.extend(
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<BTreeSet<_>, _>>()?,
            );
        }
        let mut moved = false;
        for directory in ["assets", "inputs", "commits"] {
            for entry in fs::read_dir(self.root.join(directory))? {
                let entry = entry?;
                let relative = format!("{directory}/{}", entry.file_name().to_string_lossy());
                if matches!(directory, "assets" | "inputs") && references.contains(&relative) {
                    continue;
                }
                fs::rename(
                    entry.path(),
                    self.root.join("quarantine").join(new_key("orphan")),
                )?;
                moved = true;
            }
        }
        if moved {
            for name in ["assets", "inputs", "commits", "quarantine"] {
                sync_directory(&self.root.join(name))?;
            }
        }
        Ok(())
    }
}

fn commit_eligible(record: &JobRecord) -> Result<()> {
    if record.state == AttemptState::CancelRequested {
        return Err(LibraryError::CancelAccepted);
    }
    if record.state.is_terminal() {
        return Err(LibraryError::StaleExecution);
    }
    Ok(())
}
fn parse_output_role(value: &str) -> Result<OutputRole> {
    match value {
        PRIMARY_ROLE => Ok(OutputRole::Primary),
        "mask" => Ok(OutputRole::Mask),
        _ => Err(LibraryError::Invalid("未知输出角色".into())),
    }
}
fn max_output_pixels(recipe: Recipe, role: OutputRole) -> u64 {
    match (recipe, role) {
        (Recipe::RealEsrgan, OutputRole::Primary) => 16 * 1024 * 1024,
        (Recipe::BiRefNet, OutputRole::Primary | OutputRole::Mask) => 4 * 1024 * 1024,
        _ => 1024 * 1024,
    }
}
fn validate_candidate_relative_path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || value.contains(['\\', ':', '\0'])
        || Path::new(value).components().count() != 1
        || !matches!(
            Path::new(value).components().next(),
            Some(std::path::Component::Normal(_))
        )
        || !value.ends_with(".png")
    {
        return Err(LibraryError::Invalid(
            "候选输出路径必须为同级 PNG 文件名".into(),
        ));
    }
    Ok(())
}
fn create_private(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    Ok(options.open(path)?)
}
fn copy_and_hash(
    source: &mut impl Read,
    target: &mut impl Write,
    limit: u64,
) -> Result<(u64, ContentHash)> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = source.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > limit {
            return Err(LibraryError::Invalid("PNG 超过配方字节上限".into()));
        }
        target.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
    }
    Ok((total, ContentHash::new(format!("{:x}", hash.finalize()))?))
}
fn hash_reader(source: &mut impl Read, limit: u64) -> Result<(u64, ContentHash)> {
    copy_and_hash(source, &mut std::io::sink(), limit)
}
fn digest(bytes: &[u8]) -> ContentHash {
    ContentHash::new(format!("{:x}", Sha256::digest(bytes))).expect("SHA-256 output")
}

fn validate_png(
    file: &mut File,
    width: u32,
    height: u32,
    max_pixels: u64,
    role: OutputRole,
) -> Result<()> {
    // Read a fixed 33-byte IHDR before allowing the decoder to allocate anything.
    let mut header = [0u8; 33];
    file.read_exact(&mut header)?;
    if header[..8] != [137, 80, 78, 71, 13, 10, 26, 10]
        || header[8..12] != 13u32.to_be_bytes()
        || &header[12..16] != b"IHDR"
    {
        return Err(LibraryError::Invalid("候选不是 PNG IHDR".into()));
    }
    let actual_width = u32::from_be_bytes(header[16..20].try_into().expect("four bytes"));
    let actual_height = u32::from_be_bytes(header[20..24].try_into().expect("four bytes"));
    let allowed_colors: &[u8] = if role == OutputRole::Mask {
        &[0]
    } else {
        &[2, 6]
    };
    if (actual_width, actual_height) != (width, height)
        || u64::from(width) * u64::from(height) > max_pixels
        || header[24] != 8
        || !allowed_colors.contains(&header[25])
    {
        return Err(LibraryError::Invalid(
            "PNG 头部尺寸或像素格式不符合冻结计划".into(),
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    // Reserve the host's output allocation from the same 64 MiB budget used by
    // the decoder for metadata, decompression scratch and interlacing buffers.
    let output_budget = width as usize * height as usize * 4;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_limits(png::Limits {
        bytes: DECODE_LIMIT - output_budget,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|e| LibraryError::Invalid(format!("PNG 头部无效：{e}")))?;
    if reader.info().animation_control.is_some() {
        return Err(LibraryError::Invalid("不接受动画 PNG".into()));
    }
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= DECODE_LIMIT && *size <= width as usize * height as usize * 4)
        .ok_or_else(|| LibraryError::Invalid("PNG 解码分配超限".into()))?;
    let mut output = vec![0; size];
    let frame = reader
        .next_frame(&mut output)
        .map_err(|e| LibraryError::Invalid(format!("PNG 解码失败：{e}")))?;
    if frame.width != width || frame.height != height {
        return Err(LibraryError::Invalid("PNG 解码尺寸不符".into()));
    }
    reader
        .finish()
        .map_err(|e| LibraryError::Invalid(format!("PNG 尾部损坏：{e}")))?;
    Ok(())
}
fn png_color_type(path: &Path) -> Result<u8> {
    let mut file = open_regular(path, false)?;
    let mut header = [0u8; 26];
    file.read_exact(&mut header)?;
    Ok(header[25])
}
