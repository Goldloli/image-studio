use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufReader, Read, Seek, SeekFrom, Write},
};

const MAX_PNG_BYTES: u64 = 32 * 1024 * 1024;
const DECODE_LIMIT: usize = 64 * 1024 * 1024;
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
        mut hook: impl FnMut(CommitStage) -> Result<()>,
    ) -> Result<JobRecord> {
        let record = current_job(&self.db, attempt)?;
        commit_eligible(&record)?;
        let output = &record.plan.effective_output;
        if candidate.relative_path != "result.png"
            || candidate.width != output.width
            || candidate.height != output.height
            || candidate.byte_count == 0
            || candidate.byte_count > MAX_PNG_BYTES
        {
            return Err(LibraryError::Invalid(
                "候选路径、尺寸或文件大小不符合冻结计划".into(),
            ));
        }
        let expected_hash = ContentHash::new(&candidate.sha256)?;
        let run = self.attempt_run_dir(attempt)?;
        if !fs::symlink_metadata(&run)?.file_type().is_dir() || run.canonicalize()? != run {
            return Err(LibraryError::Invalid("执行目录越界或包含链接".into()));
        }
        let mut source = open_regular(&run.join("result.png"), false)?;
        let before = source.metadata()?;
        if before.len() != candidate.byte_count || before.len() > MAX_PNG_BYTES {
            return Err(LibraryError::Invalid("候选字节数不符".into()));
        }
        let key = new_key("asset");
        let temporary = TemporaryFile(self.root.join("commits").join(format!("{key}.tmp")));
        let mut host_copy = create_private(&temporary.0)?;
        let (bytes, hash) = copy_and_hash(&mut source, &mut host_copy, MAX_PNG_BYTES)?;
        let after = source.metadata()?;
        if bytes != candidate.byte_count
            || hash != expected_hash
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
        {
            return Err(LibraryError::Invalid("候选在读取时改变或散列不符".into()));
        }
        hook(CommitStage::Copied)?;
        host_copy.flush()?;
        host_copy.seek(SeekFrom::Start(0))?;
        validate_png(&mut host_copy, output.width, output.height)?;
        host_copy.sync_all()?;
        hook(CommitStage::Synced)?;
        let mut permissions = host_copy.metadata()?.permissions();
        permissions.set_readonly(true);
        host_copy.set_permissions(permissions)?;
        host_copy.sync_all()?;
        drop(host_copy);
        let relative_path = format!("assets/{key}.png");
        let final_path = self.root.join(&relative_path);
        fs::rename(&temporary.0, &final_path)?;
        sync_directory(&self.root.join("assets"))?;
        sync_directory(&self.root.join("commits"))?;
        hook(CommitStage::Renamed)?;
        let asset = AssetRef {
            asset_id: AssetId::new(new_key("asset"))?,
            content_hash: hash,
            data_type: if png_color_type(&final_path)? == 6 {
                AssetKind::ImageRgba
            } else {
                AssetKind::ImageRgb
            },
            location: StorageLocation::Managed {
                storage_id: StorageLocationId::new("library")?,
                relative_path: relative_path.clone(),
            },
        };
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current_job(&tx, attempt)?;
        commit_eligible(&current)?;
        tx.execute("INSERT INTO assets(id,job_id,attempt_id,output_role,asset_json,relative_path,width,height,byte_count,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)",params![asset.asset_id.as_str(),attempt.job_id.as_str(),attempt.attempt_id.as_str(),PRIMARY_ROLE,serde_json::to_string(&asset)?,relative_path,candidate.width,candidate.height,sqlite_integer(bytes)?,now()])?;
        write_terminal(
            &tx,
            &current,
            AttemptState::Succeeded,
            FinalResult::Succeeded,
            vec![asset.clone()],
            None,
        )?;
        tx.execute(
            "UPDATE projects SET selected_asset_id=? WHERE id='default'",
            [asset.asset_id.as_str()],
        )?;
        hook(CommitStage::BeforeDatabaseCommit)?;
        tx.commit()?;
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
            .take(MAX_PNG_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != record.byte_count
            || bytes.len() as u64 > MAX_PNG_BYTES
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
        let (bytes, hash) = copy_and_hash(&mut source, &mut destination, MAX_PNG_BYTES)?;
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
        if fingerprint.length != record.byte_count || record.byte_count > MAX_PNG_BYTES {
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
            let (bytes, hash) = hash_reader(&mut source, MAX_PNG_BYTES)?;
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
            return Err(LibraryError::Invalid("PNG 超过 32 MiB".into()));
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

fn validate_png(file: &mut File, width: u32, height: u32) -> Result<()> {
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
    if (actual_width, actual_height) != (width, height)
        || u64::from(width) * u64::from(height) > 1024 * 1024
        || header[24] != 8
        || !matches!(header[25], 2 | 6)
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
