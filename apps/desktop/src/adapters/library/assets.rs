use super::*;
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use studio_core::recipes::{OutputRole, Recipe};

const MAX_ASSET_BYTES: u64 = 128 * 1024 * 1024;
const LEGACY_DECODE_LIMIT: usize = 64 * 1024 * 1024;
const UPSCALE_DECODE_LIMIT: usize = 256 * 1024 * 1024;
#[cfg(unix)]
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
#[cfg(unix)]
pub(super) struct CachedAssetCheck {
    fingerprint: AssetFingerprint,
    error: Option<String>,
}
#[cfg(unix)]
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

/// Registered artifacts removed as one confirmed unit: the primary work, any
/// companion mask produced by the same attempt, and nothing else.
#[derive(Clone, Debug)]
pub struct AssetDeletionGroup {
    pub assets: Vec<AssetRecord>,
}

impl AssetDeletionGroup {
    pub fn primary(&self) -> &AssetRecord {
        &self.assets[0]
    }

    pub fn state(&self) -> &str {
        &self.primary().deletion_state
    }

    pub fn all_deleted(&self) -> bool {
        self.assets
            .iter()
            .all(|record| record.deletion_state == "deleted")
    }

    /// Application-managed relative paths only; the protocol never exposes them
    /// to the frontend and each one is re-validated before removal.
    pub fn relative_paths(&self) -> Vec<String> {
        self.assets
            .iter()
            .filter_map(|record| match &record.asset.location {
                StorageLocation::Managed { relative_path, .. } => Some(relative_path.clone()),
                _ => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDeletionStatus {
    pub asset_id: String,
    pub state: String,
    pub error: Option<String>,
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
            let mut file = open_regular(&output.temporary.0, true)?;
            validate_png(&mut file, output.width, output.height, recipe, output.role)?;
            file.sync_all()
                .map_err(|error| LibraryError::Invalid(format!("同步候选失败：{error}")))?;
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)
                .map_err(|error| LibraryError::Invalid(format!("设置只读失败：{error}")))?;
            file.sync_all()
                .map_err(|error| LibraryError::Invalid(format!("同步只读候选失败：{error}")))?;
        }
        hook(CommitStage::Synced)?;
        let mut published = PublishedAssets {
            paths: Vec::with_capacity(prepared.len()),
            quarantine: self.root.join("quarantine"),
            committed: false,
        };
        for output in &prepared {
            fs::rename(&output.temporary.0, &output.final_path)
                .map_err(|error| LibraryError::Invalid(format!("发布候选文件失败：{error}")))?;
            published.paths.push(output.final_path.clone());
        }
        sync_directory(&self.root.join("assets"))
            .map_err(|error| LibraryError::Invalid(format!("同步资产目录失败：{error}")))?;
        sync_directory(&self.root.join("commits"))
            .map_err(|error| LibraryError::Invalid(format!("同步提交目录失败：{error}")))?;
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
        let row=self.db.query_row("SELECT asset_json,job_id,attempt_id,output_role,width,height,byte_count,is_favorite,deletion_state,deletion_error FROM assets WHERE id=?",[id.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get(4)?,r.get(5)?,read_unsigned(r,6)?,r.get::<_,bool>(7)?,r.get::<_,String>(8)?,r.get::<_,Option<String>>(9)?))).optional()?.ok_or_else(||LibraryError::NotFound(id.to_string()))?;
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
            is_favorite: row.7,
            deletion_state: row.8,
            deletion_error: row.9,
        };
        match record.deletion_state.as_str() {
            "active" => {
                if let Err(error) = self.verify_asset(&record) {
                    record.available = false;
                    record.availability_error = Some(error.to_string());
                }
            }
            state => {
                // A deletion intent or tombstone is not file damage: keep the
                // execution facts intact but never present the work as usable.
                record.available = false;
                record.availability_error = record.deletion_error.clone().or_else(|| {
                    Some(
                        match state {
                            "pending" => "作品正在删除",
                            "failed" => "删除未完成",
                            _ => "作品已删除",
                        }
                        .into(),
                    )
                });
            }
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

    /// Explicit favourite assignment by stable work identity. Only an active
    /// primary work may be favourite, and the caller sends the wanted value so
    /// replaying a request can never toggle it. Execution evidence, timestamps
    /// and ordering identities are untouched.
    pub fn set_asset_favorite(&mut self, id: &AssetId, favorite: bool) -> Result<bool> {
        let changed = self.db.execute(
            "UPDATE assets SET is_favorite=? WHERE id=? AND output_role='primary' AND deletion_state='active' AND job_id IN (SELECT id FROM jobs WHERE project_id='default')",
            params![i64::from(favorite), id.as_str()],
        )?;
        if changed == 1 {
            return Ok(favorite);
        }
        let row: Option<(String, String, bool)> = self
            .db
            .query_row(
                "SELECT a.output_role,a.deletion_state,j.project_id='default' FROM assets a JOIN jobs j ON j.id=a.job_id WHERE a.id=?",
                [id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        match row {
            None => Err(LibraryError::NotFound(id.to_string())),
            Some((_, _, false)) => Err(LibraryError::Invalid("资产不属于默认项目".into())),
            Some((role, _, _)) if role != "primary" => {
                Err(LibraryError::Invalid("只有作品主图可收藏".into()))
            }
            Some((_, state, _)) if state != "active" => {
                Err(LibraryError::Invalid("清理中或已删除的作品不可收藏".into()))
            }
            Some(_) => Err(LibraryError::Invalid("收藏未保存".into())),
        }
    }

    /// Resolves the confirmed deletion unit from the primary work identity. A
    /// mask or imported source can never be a deletion target by itself.
    pub fn asset_deletion_group(&self, primary: &AssetId) -> Result<AssetDeletionGroup> {
        let record = self.asset(primary)?;
        if record.output_role != "primary" {
            return Err(LibraryError::Invalid("只有作品主图可作为删除目标".into()));
        }
        let mut statement = self.db.prepare(
            "SELECT id FROM assets WHERE attempt_id=? ORDER BY CASE output_role WHEN 'primary' THEN 0 ELSE 1 END,rowid",
        )?;
        let ids = statement
            .query_map([record.attempt_id.as_str()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let assets = ids
            .iter()
            .map(|id| self.asset(&AssetId::new(id.clone())?))
            .collect::<Result<Vec<_>>>()?;
        if assets
            .first()
            .is_none_or(|first| first.asset.asset_id != *primary)
        {
            return Err(LibraryError::Invalid("删除组缺少主图".into()));
        }
        Ok(AssetDeletionGroup { assets })
    }

    /// Records the confirmed deletion intent for the whole registered unit. The
    /// files are untouched here so an interrupted cleanup can be retried from
    /// the persisted intent; a retry reuses the originally confirmed scope.
    pub fn begin_asset_deletion(&mut self, primary: &AssetId) -> Result<AssetDeletionGroup> {
        let group = self.asset_deletion_group(primary)?;
        if group.all_deleted() {
            return Ok(group);
        }
        self.db.execute(
            "UPDATE assets SET deletion_state='pending',delete_requested_at=coalesce(delete_requested_at,?),deletion_error=NULL WHERE attempt_id=? AND deletion_state!='deleted'",
            params![now(), group.primary().attempt_id.as_str()],
        )?;
        self.asset_deletion_group(primary)
    }

    /// A partially cleaned unit is kept visible as "删除未完成" with its reason
    /// and a retry entry instead of pretending the work disappeared.
    pub fn fail_asset_deletion(&mut self, primary: &AssetId, error: &str) -> Result<()> {
        let group = self.asset_deletion_group(primary)?;
        self.db.execute(
            "UPDATE assets SET deletion_state='failed',deletion_error=? WHERE attempt_id=? AND deletion_state IN ('pending','failed')",
            params![bound_detail(error), group.primary().attempt_id.as_str()],
        )?;
        Ok(())
    }

    /// Only call after every registered artifact was removed. Executes as one
    /// transaction: tombstone the group, clear the favourite and repair a
    /// selection that pointed at it.
    pub fn finish_asset_deletion(&mut self, primary: &AssetId) -> Result<()> {
        let attempt_id = String::from(primary.as_str());
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempt_id: String = tx
            .query_row(
                "SELECT attempt_id FROM assets WHERE id=?",
                [attempt_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(primary.to_string()))?;
        tx.execute(
            "UPDATE assets SET deletion_state='deleted',deleted_at=?,deletion_error=NULL,is_favorite=0 WHERE attempt_id=? AND deletion_state!='deleted'",
            params![now(), attempt_id],
        )?;
        tx.execute(
            "UPDATE projects SET selected_asset_id=NULL WHERE id='default' AND selected_asset_id IN (SELECT id FROM assets WHERE attempt_id=? AND deletion_state='deleted')",
            [attempt_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn asset_deletion_status(&self, primary: &AssetId) -> Result<AssetDeletionStatus> {
        let record = self.asset(primary)?;
        Ok(AssetDeletionStatus {
            asset_id: primary.to_string(),
            state: record.deletion_state,
            error: record.deletion_error,
        })
    }

    /// Confirmed but unfinished cleanups, resumed once the host owns the data
    /// root. Primary identities keep every group addressable after a restart.
    #[allow(dead_code)] // consumed by the confirmed-deletion recovery wired with the host command
    pub(super) fn pending_asset_deletions(&self) -> Result<Vec<AssetId>> {
        let mut statement = self.db.prepare(
            "SELECT DISTINCT a.id FROM assets a JOIN jobs j ON j.id=a.job_id WHERE j.project_id='default' AND a.output_role='primary' AND a.deletion_state='pending' ORDER BY a.created_at,a.id",
        )?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|value| Ok(AssetId::new(value?)?))
            .collect()
    }

    /// Removes exactly the registered files of a confirmed deletion unit. The
    /// relative path of each record is re-validated before removal, links and
    /// special files are refused, a missing file counts as already cleaned, and
    /// nothing outside the registered unit is touched.
    pub fn remove_asset_deletion_files(&self, primary: &AssetId) -> Result<()> {
        let group = self.asset_deletion_group(primary)?;
        if group.all_deleted() {
            return Ok(());
        }
        if group.state() != "pending" {
            return Err(LibraryError::Invalid("删除意图尚未确认".into()));
        }
        for record in &group.assets {
            let path = self.asset_path(&record.asset)?;
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_file() => {
                    remove_registered_file(&path)?;
                    sync_directory(
                        path.parent()
                            .ok_or_else(|| LibraryError::Invalid("资产目录无效".into()))?,
                    )?;
                }
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(LibraryError::Invalid("拒绝删除链接形式的资产".into()));
                }
                Ok(_) => return Err(LibraryError::Invalid("登记产物不是普通文件".into())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    /// Bounded startup recovery for confirmed cleanups interrupted by a crash.
    /// A resumed cleanup reuses the persisted scope; failures stay visible and
    /// retryable instead of silently restoring the work.
    pub(super) fn recover_asset_deletions(&mut self) -> Result<()> {
        let pending = self.pending_asset_deletions()?;
        for primary in pending.into_iter().take(64) {
            match self.remove_asset_deletion_files(&primary) {
                Ok(()) => {
                    if let Err(error) = self.finish_asset_deletion(&primary) {
                        let _ = self.fail_asset_deletion(&primary, &error.to_string());
                    }
                }
                Err(error) => {
                    let _ = self.fail_asset_deletion(&primary, &error.to_string());
                }
            }
        }
        Ok(())
    }

    /// Controlled asset URI handlers receive bytes by registered ID, never a client path.
    pub fn read_asset_png(&self, id: &AssetId) -> Result<Vec<u8>> {
        let imported: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM imported_sources WHERE id=?)",
            [id.as_str()],
            |row| row.get(0),
        )?;
        if imported {
            return self.read_source_png(id);
        }
        let record = self.asset(id)?;
        super::ensure_asset_active(&record)?;
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
        super::ensure_asset_active(&record)?;
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
            crate::adapters::safe_fs::replace_file(&temporary.0, &target)?;
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
        #[cfg(unix)]
        let id = record.asset.asset_id.to_string();
        #[cfg(unix)]
        {
            if let Some(previous) = self.asset_checks.borrow().get(&id)
                && previous.fingerprint == fingerprint
            {
                return previous
                    .error
                    .as_ref()
                    .map_or(Ok(()), |error| Err(LibraryError::Invalid(error.clone())));
            }
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
        #[cfg(unix)]
        {
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
        }
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

fn remove_registered_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) => {
            #[cfg(windows)]
            {
                if error.kind() == std::io::ErrorKind::PermissionDenied {
                    let mut permissions = fs::metadata(path)?.permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(path, permissions)?;
                    return fs::remove_file(path).map_err(Into::into);
                }
            }
            Err(error.into())
        }
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
        (Recipe::RealEsrgan | Recipe::RealEsrganWindows, OutputRole::Primary) => 16 * 1024 * 1024,
        (Recipe::BiRefNet | Recipe::BiRefNetWindows, OutputRole::Primary | OutputRole::Mask) => {
            4 * 1024 * 1024
        }
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
    recipe: Recipe,
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
        || u64::from(width) * u64::from(height) > max_output_pixels(recipe, role)
        || header[24] != 8
        || !allowed_colors.contains(&header[25])
    {
        return Err(LibraryError::Invalid(
            "PNG 头部尺寸或像素格式不符合冻结计划".into(),
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    // Reserve host output and decoder scratch from one recipe-specific budget.
    // The 16 MP upscale recipe must not expand the legacy decoder's allowance.
    let decode_limit = if matches!(recipe, Recipe::RealEsrgan | Recipe::RealEsrganWindows) {
        UPSCALE_DECODE_LIMIT
    } else {
        LEGACY_DECODE_LIMIT
    };
    let output_budget = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| LibraryError::Invalid("PNG 解码分配溢出".into()))?;
    let scratch_budget = decode_limit
        .checked_sub(output_budget)
        .ok_or_else(|| LibraryError::Invalid("PNG 解码分配超限".into()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_limits(png::Limits {
        bytes: scratch_budget,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|e| LibraryError::Invalid(format!("PNG 头部无效：{e}")))?;
    if reader.info().animation_control.is_some() {
        return Err(LibraryError::Invalid("不接受动画 PNG".into()));
    }
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= output_budget)
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

#[cfg(test)]
mod bounds_tests {
    use super::*;

    #[test]
    fn upscale_png_limits_do_not_relax_old_z_image_limits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("4096.png");
        let mut encoder = png::Encoder::new(File::create(&path).unwrap(), 4096, 4096);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .unwrap()
            .into_stream_writer()
            .unwrap();
        let row = vec![32_u8; 4096 * 3];
        for _ in 0..4096 {
            writer.write_all(&row).unwrap();
        }
        writer.finish().unwrap();
        validate_png(
            &mut File::open(&path).unwrap(),
            4096,
            4096,
            Recipe::RealEsrgan,
            OutputRole::Primary,
        )
        .unwrap();
        validate_png(
            &mut File::open(&path).unwrap(),
            4096,
            4096,
            Recipe::RealEsrganWindows,
            OutputRole::Primary,
        )
        .unwrap();
        assert_eq!(
            max_output_pixels(Recipe::RealEsrganWindows, OutputRole::Primary),
            16 * 1024 * 1024
        );
        assert_eq!(
            max_output_pixels(Recipe::BiRefNetWindows, OutputRole::Mask),
            4 * 1024 * 1024
        );
        assert!(
            validate_png(
                &mut File::open(&path).unwrap(),
                4096,
                4096,
                Recipe::ZImageTurbo,
                OutputRole::Primary
            )
            .is_err()
        );
        let mut header = fs::read(&path).unwrap()[..33].to_vec();
        header[16..20].copy_from_slice(&4097_u32.to_be_bytes());
        fs::write(&path, &header).unwrap();
        assert!(
            validate_png(
                &mut File::open(&path).unwrap(),
                4097,
                4096,
                Recipe::RealEsrgan,
                OutputRole::Primary
            )
            .is_err()
        );
        header[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        header[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        fs::write(&path, header).unwrap();
        assert!(
            validate_png(
                &mut File::open(&path).unwrap(),
                u32::MAX,
                u32::MAX,
                Recipe::RealEsrgan,
                OutputRole::Primary
            )
            .is_err()
        );
        for recipe in [Recipe::ZImageTurbo, Recipe::RealEsrgan] {
            let bytes = recipe.max_output_bytes(OutputRole::Primary).unwrap();
            let mut oversized = std::io::repeat(0).take(bytes + 1);
            assert!(copy_and_hash(&mut oversized, &mut std::io::sink(), bytes).is_err());
        }
    }
}
