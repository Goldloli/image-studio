//! Synchronous local library. The host calls this behind its background Mutex boundary.
//! SQLite commit is the only success point; files without a committed reference are orphans.
mod assets;
mod conversations;
mod queries;
mod schema;
mod sources;

pub use assets::{AssetDeletionGroup, AssetDeletionStatus};
pub use conversations::{
    AcceptTurnInput, AcceptedTurn, ConversationDeleteResult, ConversationIdentity,
    ConversationListPage, ConversationRecord, CreatedConversation, DEFAULT_MODEL_ID, TurnPage,
    TurnRecord, TurnStatus,
};
pub use queries::{
    AssetConversationFacet, AssetLibraryFacets, AssetLibraryItem, AssetLibraryPage,
    AssetLibraryQuery, AssetModelFacet, AssetSizeFacet, StableCursor, TaskAdvancedView,
    TaskAttemptView, TaskCenterDetail, TaskCenterItem, TaskCenterPage, TaskCenterQuery, TaskCounts,
};
pub use sources::{AssetReferenceConflict, ImportedSourceRecord, ResolvedSource};

use fs2::FileExt;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(windows)]
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use studio_core::{
    ContractError,
    contracts::*,
    domain::*,
    generation::AttemptState,
    recipes::{PRIMARY_ROLE, validate_recipe_plan},
};
use thiserror::Error;
use uuid::Uuid;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
    System::Threading::CreateMutexW,
};

pub use assets::{CandidateDescriptor, RoleCandidate};

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("数据目录正在由另一个 Image Studio 实例使用")]
    InUse,
    #[error("本地库版本不受支持或结构异常：{0}")]
    Schema(String),
    #[error("存在活动任务，请等待执行和清理完成")]
    Busy,
    #[error("同一提交键对应了不同内容")]
    IdempotencyConflict,
    #[error("草稿版本已更新，请重新读取后保存")]
    DraftConflict,
    #[error("会话或发送轮次状态已变化，请重新读取")]
    TurnConflict,
    #[error("记录不存在：{0}")]
    NotFound(String),
    #[error("当前执行身份已失效或已终结")]
    StaleExecution,
    #[error("执行已接受取消，不能登记结果")]
    CancelAccepted,
    #[error("无效操作：{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, LibraryError>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftRecord {
    pub id: u64,
    pub name: String,
    pub prompt: String,
    pub params: Value,
    pub collapsed: bool,
    pub version: u64,
    #[serde(rename = "promptSourceId", default)]
    pub prompt_source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: JobId,
    pub turn_id: Option<TurnId>,
    pub conversation_id: Option<ConversationId>,
    pub attempt: Attempt,
    pub state: AttemptState,
    pub request: UserRequest,
    pub plan: ExecutionPlan,
    pub manifest: Option<RunManifest>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetRecord {
    pub asset: AssetRef,
    pub job_id: JobId,
    pub attempt_id: AttemptId,
    pub output_role: String,
    pub width: u32,
    pub height: u32,
    pub byte_count: u64,
    pub available: bool,
    pub availability_error: Option<String>,
    pub is_favorite: bool,
    /// `active | pending | failed | deleted`; user deletion never rewrites the
    /// immutable execution record, only this attached state.
    pub deletion_state: String,
    pub deletion_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibrarySnapshot {
    pub project_id: ProjectId,
    pub drafts: Vec<DraftRecord>,
    pub active_draft_id: u64,
    pub active_conversation_id: ConversationId,
    pub conversations: Vec<ConversationRecord>,
    pub conversations_has_more: bool,
    pub conversations_next_cursor: Option<u64>,
    pub turns: TurnPage,
    pub selected_asset_id: Option<AssetId>,
    pub jobs: Vec<JobRecord>,
    pub assets: Vec<AssetRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    pub job: JobRecord,
    pub created: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CancelOutcome {
    Accepted { job: JobRecord },
    AlreadyFinished { job: JobRecord },
}

#[derive(Clone, Debug)]
pub enum Completion {
    Failed { code: String, detail: String },
    Cancelled,
}

/// `flock` follows the open file description, including temporary fork copies.
/// Closing only the parent File can therefore extend retired host ownership
/// until a child execs/closes its inherited descriptor. Explicit unlock retires
/// this ownership even while those copies exist.
static ACTIVE_HOST_ROOTS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

struct ProcessRootClaim(PathBuf);

impl ProcessRootClaim {
    fn acquire(root: &Path) -> Result<Self> {
        let mut roots = ACTIVE_HOST_ROOTS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .map_err(|_| LibraryError::Invalid("宿主锁登记不可用".into()))?;
        if !roots.insert(root.to_path_buf()) {
            return Err(LibraryError::InUse);
        }
        Ok(Self(root.to_path_buf()))
    }
}

impl Drop for ProcessRootClaim {
    fn drop(&mut self) {
        if let Ok(mut roots) = ACTIVE_HOST_ROOTS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
        {
            roots.remove(&self.0);
        }
    }
}

struct HostLock {
    file: File,
    _root_claim: ProcessRootClaim,
    #[cfg(windows)]
    _root_mutex: WindowsRootMutex,
}

impl Drop for HostLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(windows)]
struct WindowsRootMutex(usize);

#[cfg(windows)]
impl WindowsRootMutex {
    fn acquire(root: &Path) -> Result<Self> {
        let mut digest = Sha256::new();
        digest.update(root.to_string_lossy().to_lowercase().as_bytes());
        let name = format!("Local\\ImageStudio-{:x}\0", digest.finalize())
            .encode_utf16()
            .collect::<Vec<_>>();
        // SAFETY: The name is a stable, NUL-terminated local session name and
        // the returned handle is owned by this guard.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(LibraryError::Io(std::io::Error::last_os_error()));
        }
        // SAFETY: GetLastError has no preconditions and is read immediately
        // after CreateMutexW.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: This process owns the new handle returned above.
            unsafe {
                CloseHandle(handle);
            }
            return Err(LibraryError::InUse);
        }
        Ok(Self(handle as usize))
    }
}

#[cfg(windows)]
impl Drop for WindowsRootMutex {
    fn drop(&mut self) {
        // SAFETY: The handle is owned by this guard and closed exactly once.
        unsafe {
            CloseHandle(self.0 as HANDLE);
        }
    }
}

pub struct Library {
    // Fields drop in declaration order: close SQLite before releasing ownership.
    db: Connection,
    root: PathBuf,
    protected_roots: Vec<PathBuf>,
    #[cfg(unix)]
    asset_checks: std::cell::RefCell<assets::AssetCheckCache>,
    _host_lock: HostLock,
}

const SCHEMA_V3: &str = "
CREATE TABLE projects(id TEXT PRIMARY KEY, active_draft_id INTEGER, active_conversation_id TEXT, selected_asset_id TEXT);
CREATE TABLE drafts(id INTEGER PRIMARY KEY AUTOINCREMENT, project_id TEXT NOT NULL REFERENCES projects(id), name TEXT NOT NULL, prompt TEXT NOT NULL, prompt_source TEXT, params_json TEXT NOT NULL, collapsed INTEGER NOT NULL, version INTEGER NOT NULL CHECK(version > 0));
CREATE TABLE conversations(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), draft_id INTEGER NOT NULL UNIQUE REFERENCES drafts(id), title TEXT NOT NULL, title_source TEXT NOT NULL CHECK(title_source IN ('auto','manual')), created_at INTEGER NOT NULL, activity_seq INTEGER NOT NULL);
CREATE INDEX conversations_project_activity ON conversations(project_id, activity_seq DESC, id);
CREATE TABLE jobs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), submission_key TEXT NOT NULL, request_json TEXT NOT NULL, current_attempt_id TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(project_id, submission_key));
CREATE TABLE attempts(id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id), token TEXT NOT NULL UNIQUE, storage_key TEXT NOT NULL UNIQUE, state TEXT NOT NULL CHECK(state IN ('submitted','preparing','running','committing','cancel_requested','succeeded','failed','cancelled','interrupted')), plan_json TEXT NOT NULL, manifest_json TEXT, error TEXT, created_at INTEGER NOT NULL, completed_at INTEGER);
CREATE UNIQUE INDEX one_active_attempt ON attempts((1)) WHERE state IN ('submitted','preparing','running','committing','cancel_requested');
CREATE INDEX attempts_job ON attempts(job_id, created_at);
CREATE TABLE generation_turns(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), conversation_id TEXT NOT NULL REFERENCES conversations(id), sequence INTEGER NOT NULL CHECK(sequence > 0), submission_key TEXT NOT NULL, source_draft_id INTEGER REFERENCES drafts(id), prompt TEXT NOT NULL, model_id TEXT NOT NULL, aspect TEXT NOT NULL, count INTEGER NOT NULL CHECK(count > 0), seed TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending','cancel_requested','cancelled','rejected','interrupted','job')), job_id TEXT UNIQUE REFERENCES jobs(id), error TEXT, progress_stage TEXT, progress_step INTEGER, progress_total INTEGER, received_at INTEGER NOT NULL, UNIQUE(project_id, submission_key), UNIQUE(conversation_id, sequence));
CREATE INDEX generation_turns_conversation_sequence ON generation_turns(conversation_id, sequence DESC);
CREATE INDEX generation_turns_job ON generation_turns(job_id);
CREATE TABLE assets(id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id), attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id), asset_json TEXT NOT NULL, relative_path TEXT NOT NULL UNIQUE, width INTEGER NOT NULL, height INTEGER NOT NULL, byte_count INTEGER NOT NULL, created_at INTEGER NOT NULL);
CREATE INDEX jobs_project_created ON jobs(project_id, created_at DESC, id DESC);
CREATE INDEX assets_created ON assets(created_at DESC, id DESC);
CREATE TABLE installations(installation_id TEXT PRIMARY KEY, record_json TEXT NOT NULL);
PRAGMA user_version=4;
";

impl Library {
    /// Acquires host ownership before touching SQLite or recovery state.
    pub fn open(data_root: impl AsRef<Path>, protected_roots: Vec<PathBuf>) -> Result<Self> {
        fs::create_dir_all(data_root.as_ref())?;
        let root = data_root.as_ref().canonicalize()?;
        let root_claim = ProcessRootClaim::acquire(&root)?;
        #[cfg(windows)]
        let root_mutex = WindowsRootMutex::acquire(&root)?;
        let host_lock = open_regular(&root.join("host.lock"), true)?;
        host_lock.try_lock_exclusive().map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                LibraryError::InUse
            } else {
                LibraryError::Io(error)
            }
        })?;
        let host_lock = HostLock {
            file: host_lock,
            _root_claim: root_claim,
            #[cfg(windows)]
            _root_mutex: root_mutex,
        };
        for name in ["runs", "commits", "assets", "inputs", "quarantine"] {
            managed_directory(&root.join(name))?;
        }
        let db_path = root.join("library.sqlite3");
        if db_path.exists() && !fs::symlink_metadata(&db_path)?.file_type().is_file() {
            return Err(LibraryError::Schema("数据库不是普通文件".into()));
        }
        let mut db = Connection::open(db_path)?;
        // A competing writer must surface as a recoverable `BUSY` instead of blocking a send
        // forever; the desktop shell serialises its own writes, so five seconds is generous.
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 0 {
            let tables: u32 = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r|r.get(0))?;
            if tables != 0 {
                return Err(LibraryError::Schema("未标版本的非空数据库".into()));
            }
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(SCHEMA_V3)?;
            tx.execute("INSERT INTO projects(id) VALUES('default')", [])?;
            tx.execute(
                "INSERT INTO drafts(project_id,name,prompt,prompt_source,params_json,collapsed,version) VALUES('default','新会话','',NULL,?1,0,1)",
                [serde_json::json!({
                    "modelId": DEFAULT_MODEL_ID,
                    "aspect": "1:1",
                    "count": 1,
                    "seed": ""
                })
                .to_string()],
            )?;
            tx.execute("INSERT INTO conversations(id,project_id,draft_id,title,title_source,created_at,activity_seq) VALUES('conv-default','default',1,'新会话','auto',?,1)", [now()])?;
            tx.execute(
                "UPDATE projects SET active_draft_id=1,active_conversation_id='conv-default' WHERE id='default'",
                [],
            )?;
            tx.commit()?;
        } else if version == 1 {
            conversations::migrate_v1_to_v2(&mut db)?;
            queries::migrate_v2_to_v3(&mut db)?;
            conversations::migrate_v3_to_v4(&mut db)?;
        } else if version == 2 {
            queries::migrate_v2_to_v3(&mut db)?;
            conversations::migrate_v3_to_v4(&mut db)?;
        } else if version == 3 {
            conversations::migrate_v3_to_v4(&mut db)?;
        } else if version > schema::CURRENT_USER_VERSION {
            return Err(LibraryError::Schema(format!("user_version={version}")));
        }
        if version <= 4 {
            schema::migrate_v4_to_v5(&mut db)?;
        }
        if version <= schema::V5_USER_VERSION {
            schema::migrate_v5_to_v6(&mut db)?;
        }
        let integrity: String = db.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
        if integrity != "ok" {
            return Err(LibraryError::Schema(integrity));
        }
        // Preparing these queries also detects a version-number-only or incomplete schema.
        db.prepare("SELECT p.id,p.active_conversation_id,d.version,c.title,c.deleted_at,t.status,j.request_json,a.plan_json,s.asset_json,s.is_favorite,s.deletion_state,i.record_json FROM projects p LEFT JOIN drafts d ON d.project_id=p.id LEFT JOIN conversations c ON c.project_id=p.id LEFT JOIN generation_turns t ON t.conversation_id=c.id LEFT JOIN jobs j ON j.project_id=p.id LEFT JOIN attempts a ON a.job_id=j.id LEFT JOIN assets s ON s.attempt_id=a.id LEFT JOIN installations i ON 0 LIMIT 0")?;
        let mut protected = vec![root.clone()];
        for path in protected_roots {
            fs::create_dir_all(&path)?;
            protected.push(path.canonicalize()?);
        }
        let mut library = Self {
            db,
            root,
            protected_roots: protected,
            #[cfg(unix)]
            asset_checks: Default::default(),
            _host_lock: host_lock,
        };
        library.recover()?;
        Ok(library)
    }

    pub fn data_root(&self) -> &Path {
        &self.root
    }
    pub fn default_project_id(&self) -> ProjectId {
        ProjectId::new("default").expect("constant ID")
    }

    pub fn snapshot(&self) -> Result<LibrarySnapshot> {
        let (active, active_conversation, selected): (u64, String, Option<String>) = self.db.query_row(
            "SELECT active_draft_id,active_conversation_id,selected_asset_id FROM projects WHERE id='default'",
            [],
            |r| Ok((read_unsigned(r, 0)?, r.get(1)?, r.get(2)?)),
        )?;
        let active_conversation_id = ConversationId::new(active_conversation)?;
        let selected_asset_id = selected.map(AssetId::new).transpose()?;
        let mut assets = self.assets(100, 0)?;
        if let Some(id) = &selected_asset_id
            && !assets.iter().any(|record| &record.asset.asset_id == id)
        {
            // The last selected result may be older than the first history page.
            // Keep it resolvable without inflating paginated task totals.
            assets.push(self.asset(id)?);
        }
        let conversation_page = self.conversations(100, None)?;
        let mut conversations = conversation_page.conversations;
        if !conversations
            .iter()
            .any(|conversation| conversation.id == active_conversation_id)
        {
            conversations.push(self.conversation(&active_conversation_id)?);
        }
        Ok(LibrarySnapshot {
            project_id: self.default_project_id(),
            drafts: self.drafts()?,
            active_draft_id: active,
            active_conversation_id: active_conversation_id.clone(),
            conversations,
            conversations_has_more: conversation_page.has_more,
            conversations_next_cursor: conversation_page.next_cursor,
            turns: self.turns(&active_conversation_id, 30, None)?,
            selected_asset_id,
            jobs: self.jobs(100, 0)?,
            assets,
        })
    }

    pub fn drafts(&self) -> Result<Vec<DraftRecord>> {
        let mut statement = self.db.prepare("SELECT d.id,d.name,d.prompt,d.prompt_source,d.params_json,d.collapsed,d.version FROM drafts d WHERE d.project_id='default' AND NOT EXISTS(SELECT 1 FROM conversations c WHERE c.draft_id=d.id AND c.deleted_at IS NOT NULL) ORDER BY d.id")?;
        let rows = statement.query_map([], |r| {
            Ok((
                read_unsigned(r, 0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get(5)?,
                read_unsigned(r, 6)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, prompt, prompt_source, p, collapsed, version) = row?;
            Ok(DraftRecord {
                id,
                name,
                prompt,
                params: serde_json::from_str(&p)?,
                collapsed,
                version,
                prompt_source,
            })
        })
        .collect()
    }

    pub fn create_draft(&mut self) -> Result<DraftRecord> {
        Ok(self.create_conversation()?.draft)
    }

    pub fn save_draft(&mut self, draft: DraftRecord, expected_version: u64) -> Result<DraftRecord> {
        if draft.name.is_empty()
            || draft.name.len() > 1024
            || draft.prompt.len() > 1024 * 1024
            || !draft.params.is_object()
            // Five bounded capability slots may each retain the existing 1 MiB draft budget.
            || serde_json::to_vec(&draft.params)?.len() > 6 * 1024 * 1024
            || draft
                .prompt_source
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 128)
        {
            return Err(LibraryError::Invalid("草稿超出保存范围".into()));
        }
        let version = expected_version
            .checked_add(1)
            .ok_or_else(|| LibraryError::Invalid("草稿版本溢出".into()))?;
        let retired: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE draft_id=? AND project_id='default' AND deleted_at IS NOT NULL)",
            [sqlite_integer(draft.id)?],
            |row| row.get(0),
        )?;
        if retired {
            return Err(LibraryError::NotFound(format!("draft {}", draft.id)));
        }
        let changed=self.db.execute("UPDATE drafts SET name=?,prompt=?,prompt_source=?,params_json=?,collapsed=?,version=? WHERE id=? AND project_id='default' AND version=? AND NOT EXISTS(SELECT 1 FROM conversations c WHERE c.draft_id=drafts.id AND c.deleted_at IS NOT NULL)",params![draft.name,draft.prompt,draft.prompt_source,serde_json::to_string(&draft.params)?,draft.collapsed,sqlite_integer(version)?,sqlite_integer(draft.id)?,sqlite_integer(expected_version)?])?;
        if changed != 1 {
            return Err(LibraryError::DraftConflict);
        }
        Ok(DraftRecord { version, ..draft })
    }

    pub fn select_draft(&mut self, id: u64) -> Result<()> {
        let id = sqlite_integer(id)?;
        let changed=self.db.execute("UPDATE projects SET active_draft_id=?,active_conversation_id=(SELECT id FROM conversations WHERE draft_id=? AND deleted_at IS NULL),selected_asset_id=NULL WHERE id='default' AND EXISTS(SELECT 1 FROM drafts WHERE id=? AND project_id='default') AND EXISTS(SELECT 1 FROM conversations WHERE draft_id=? AND project_id='default' AND deleted_at IS NULL)",params![id,id,id,id])?;
        if changed != 1 {
            return Err(LibraryError::NotFound(format!("draft {id}")));
        }
        Ok(())
    }

    pub fn select_asset(&mut self, id: Option<AssetId>) -> Result<()> {
        if let Some(id) = &id {
            let record = self.asset(id)?;
            ensure_asset_active(&record)?;
        }
        self.db.execute(
            "UPDATE projects SET selected_asset_id=? WHERE id='default'",
            [id.map(|v| v.to_string())],
        )?;
        Ok(())
    }

    pub fn submit(&mut self, request: UserRequest, plan: ExecutionPlan) -> Result<Submission> {
        let source = self.source_facts_for_request(&request)?;
        validate_recipe_plan(&request, &plan, source)?;
        if request.project_id != self.default_project_id() {
            return Err(LibraryError::Invalid("本阶段仅支持默认项目".into()));
        }
        let request_json = request.to_json()?;
        let plan_json = plan.to_json()?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,request_json FROM jobs WHERE project_id=? AND submission_key=?",
                params![
                    request.project_id.as_str(),
                    request.idempotency_key.as_str()
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, old)) = existing {
            if UserRequest::from_json(&old)? != request {
                return Err(LibraryError::IdempotencyConflict);
            }
            drop(tx);
            return Ok(Submission {
                job: self.job(&JobId::new(id)?)?,
                created: false,
            });
        }
        ensure_idle(&tx)?;
        let job_id = JobId::new(new_key("job"))?;
        let attempt = new_attempt(&job_id)?;
        let key = new_key("run");
        tx.execute("INSERT INTO jobs(id,project_id,submission_key,request_json,current_attempt_id,created_at) VALUES(?,?,?,?,?,?)",params![job_id.as_str(),request.project_id.as_str(),request.idempotency_key.as_str(),request_json,attempt.attempt_id.as_str(),now()])?;
        insert_attempt(&tx, &attempt, &key, &plan_json)?;
        tx.commit()?;
        // No process may start until the transaction above has committed.
        self.prepare_run_directory(&attempt, &key)?;
        if let Some(source) = request.inputs.first()
            && let Err(error) = self
                .copy_source_snapshot(&source.asset.asset_id, &self.root.join("runs").join(&key))
        {
            self.finish_after_reap(
                &attempt,
                Completion::Failed {
                    code: "SOURCE_SNAPSHOT".into(),
                    detail: error.to_string(),
                },
            )?;
            return Err(error);
        }
        Ok(Submission {
            job: self.job(&job_id)?,
            created: true,
        })
    }

    pub fn retry(&mut self, job_id: &JobId) -> Result<JobRecord> {
        let previous = self.job(job_id)?;
        if !matches!(
            previous.state,
            AttemptState::Failed | AttemptState::Cancelled | AttemptState::Interrupted
        ) {
            return Err(LibraryError::Invalid("仅失败、取消或中断任务可重试".into()));
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_idle(&tx)?;
        let attempt = new_attempt(job_id)?;
        let key = new_key("run");
        insert_attempt(&tx, &attempt, &key, &previous.plan.to_json()?)?;
        tx.execute(
            "UPDATE jobs SET current_attempt_id=? WHERE id=? AND current_attempt_id=?",
            params![
                attempt.attempt_id.as_str(),
                job_id.as_str(),
                previous.attempt.attempt_id.as_str()
            ],
        )?;
        tx.commit()?;
        self.prepare_run_directory(&attempt, &key)?;
        if let Some(source) = previous.request.inputs.first()
            && let Err(error) = self
                .copy_source_snapshot(&source.asset.asset_id, &self.root.join("runs").join(&key))
        {
            self.finish_after_reap(
                &attempt,
                Completion::Failed {
                    code: "SOURCE_SNAPSHOT".into(),
                    detail: error.to_string(),
                },
            )?;
            return Err(error);
        }
        self.job(job_id)
    }

    pub fn job(&self, id: &JobId) -> Result<JobRecord> {
        load_job(&self.db, id)
    }

    /// Indexed lookup for an IPC retry, independent of the visible history page.
    pub fn job_for_submission(&self, key: &IdempotencyKey) -> Result<Option<JobRecord>> {
        let id: Option<String> = self
            .db
            .query_row(
                "SELECT id FROM jobs WHERE project_id='default' AND submission_key=?",
                [key.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        id.map(|id| self.job(&JobId::new(id)?)).transpose()
    }

    /// Counts Jobs by their current Attempt; retry history never inflates totals.
    pub fn job_counts(&self) -> Result<(u64, u64, u64)> {
        Ok(self.db.query_row(
            "SELECT count(*), coalesce(sum(a.state IN ('submitted','preparing','running','committing','cancel_requested')),0), coalesce(sum(a.state='succeeded'),0) FROM jobs j JOIN attempts a ON a.id=j.current_attempt_id WHERE j.project_id='default'",
            [], |row| Ok((read_unsigned(row,0)?, read_unsigned(row,1)?, read_unsigned(row,2)?)),
        )?)
    }

    pub fn jobs(&self, limit: u32, offset: u32) -> Result<Vec<JobRecord>> {
        let mut statement = self
            .db
            .prepare("SELECT id FROM jobs ORDER BY created_at DESC,rowid DESC LIMIT ? OFFSET ?")?;
        let ids = statement
            .query_map(params![limit.clamp(1, 100), offset], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.job(&JobId::new(id)?))
            .collect()
    }

    pub fn attempts(&self, job_id: &JobId) -> Result<Vec<JobRecord>> {
        let mut statement = self
            .db
            .prepare("SELECT id FROM attempts WHERE job_id=? ORDER BY created_at,rowid")?;
        let ids = statement
            .query_map([job_id.as_str()], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| load_attempt(&self.db, &id))
            .collect()
    }

    pub fn mark_phase(&mut self, attempt: &Attempt, next: AttemptState) -> Result<JobRecord> {
        if !matches!(
            next,
            AttemptState::Preparing | AttemptState::Running | AttemptState::Committing
        ) {
            return Err(LibraryError::Invalid("此入口仅接受执行阶段".into()));
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current_job(&tx, attempt)?;
        if !current.state.can_advance_to(next) {
            return Err(if current.state == AttemptState::CancelRequested {
                LibraryError::CancelAccepted
            } else {
                LibraryError::StaleExecution
            });
        }
        tx.execute(
            "UPDATE attempts SET state=? WHERE id=?",
            params![next.as_str(), attempt.attempt_id.as_str()],
        )?;
        tx.commit()?;
        self.job(&attempt.job_id)
    }

    pub fn request_cancel(&mut self, attempt: &Attempt) -> Result<CancelOutcome> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current_job(&tx, attempt)?;
        if current.state.is_terminal() {
            return Ok(CancelOutcome::AlreadyFinished { job: current });
        }
        tx.execute(
            "UPDATE attempts SET state='cancel_requested' WHERE id=?",
            [attempt.attempt_id.as_str()],
        )?;
        tx.commit()?;
        Ok(CancelOutcome::Accepted {
            job: self.job(&attempt.job_id)?,
        })
    }

    /// Caller must have confirmed the computation and all descendants are reaped.
    /// An accepted cancel wins against later worker errors and timeout diagnostics.
    pub fn finish_after_reap(
        &mut self,
        attempt: &Attempt,
        completion: Completion,
    ) -> Result<JobRecord> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = current_job(&tx, attempt)?;
        if current.state.is_terminal() {
            return Err(LibraryError::StaleExecution);
        }
        let (state, result, error) = match completion {
            Completion::Failed { code, detail }
                if current.state != AttemptState::CancelRequested =>
            {
                if code.trim().is_empty() {
                    return Err(LibraryError::Invalid("失败代码不能为空".into()));
                }
                (
                    AttemptState::Failed,
                    FinalResult::Failed { code },
                    Some(bound_detail(&detail)),
                )
            }
            Completion::Failed { detail, .. } => (
                AttemptState::Cancelled,
                FinalResult::Cancelled,
                Some(bound_detail(&detail)),
            ),
            Completion::Cancelled if current.state == AttemptState::CancelRequested => {
                (AttemptState::Cancelled, FinalResult::Cancelled, None)
            }
            Completion::Cancelled => return Err(LibraryError::Invalid("未接受取消意图".into())),
        };
        write_terminal(&tx, &current, state, result, vec![], error)?;
        tx.commit()?;
        self.job(&attempt.job_id)
    }

    pub fn attempt_run_dir(&self, attempt: &Attempt) -> Result<PathBuf> {
        let _ = current_job(&self.db, attempt)?;
        let key: String = self.db.query_row(
            "SELECT storage_key FROM attempts WHERE id=?",
            [attempt.attempt_id.as_str()],
            |r| r.get(0),
        )?;
        validate_storage_key(&key)?;
        Ok(self.root.join("runs").join(key))
    }

    /// Explicit cleanup after supervisor lease/reaping confirmation. Recovery never calls this.
    pub fn cleanup_after_reap(&self, attempt: &Attempt) -> Result<()> {
        let record = load_attempt(&self.db, attempt.attempt_id.as_str())?;
        record.attempt.validate_event(attempt)?;
        if !record.state.is_terminal() {
            return Err(LibraryError::Invalid("活动执行暂存目录不可清理".into()));
        }
        let key: String = self.db.query_row(
            "SELECT storage_key FROM attempts WHERE id=?",
            [attempt.attempt_id.as_str()],
            |r| r.get(0),
        )?;
        validate_storage_key(&key)?;
        let path = self.root.join("runs").join(key);
        match fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_dir() => fs::remove_dir_all(path)?,
            Ok(_) => return Err(LibraryError::Invalid("执行目录类型异常".into())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        };
        Ok(())
    }

    pub fn save_installation(&mut self, id: &str, record: &Value) -> Result<()> {
        validate_storage_key(id)?;
        self.db.execute("INSERT INTO installations(installation_id,record_json) VALUES(?,?) ON CONFLICT(installation_id) DO UPDATE SET record_json=excluded.record_json",params![id,serde_json::to_string(record)?])?;
        Ok(())
    }
    pub fn installation(&self, id: &str) -> Result<Option<Value>> {
        self.db
            .query_row(
                "SELECT record_json FROM installations WHERE installation_id=?",
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|json| Ok(serde_json::from_str(&json)?))
            .transpose()
    }

    fn ensure_run_directory(&self, key: &str) -> Result<()> {
        validate_storage_key(key)?;
        fs::create_dir(self.root.join("runs").join(key))?;
        sync_directory(&self.root.join("runs"))?;
        Ok(())
    }

    fn prepare_run_directory(&mut self, attempt: &Attempt, key: &str) -> Result<()> {
        if let Err(error) = self.ensure_run_directory(key) {
            // Acceptance is already durable, but no worker has started. Do not leave
            // an unlaunchable Submitted record occupying the single-active slot.
            self.finish_after_reap(
                attempt,
                Completion::Failed {
                    code: "STORAGE_PREPARE".into(),
                    detail: error.to_string(),
                },
            )?;
            return Err(error);
        }
        Ok(())
    }

    fn recover(&mut self) -> Result<()> {
        self.recover_pending_turns()?;
        self.repair_active_conversation()?;
        self.recover_asset_deletions()?;
        let ids = {
            let mut s=self.db.prepare("SELECT id FROM attempts WHERE state IN ('submitted','preparing','running','committing','cancel_requested')")?;
            s.query_map([], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for id in ids {
            let record = load_attempt(&tx, &id)?;
            write_terminal(
                &tx,
                &record,
                AttemptState::Interrupted,
                FinalResult::Interrupted,
                vec![],
                Some("宿主中断；请确认旧执行已清理后显式重试".into()),
            )?;
        }
        tx.commit()?;
        self.quarantine_orphans()
    }
}

fn ensure_idle(db: &Connection) -> Result<()> {
    let active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM attempts WHERE state IN ('submitted','preparing','running','committing','cancel_requested'))",[],|r|r.get(0))?;
    let pending:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM generation_turns WHERE status IN ('pending','cancel_requested') AND job_id IS NULL)",[],|r|r.get(0))?;
    if active || pending {
        Err(LibraryError::Busy)
    } else {
        Ok(())
    }
}
fn insert_attempt(db: &Connection, attempt: &Attempt, key: &str, plan: &str) -> Result<()> {
    db.execute("INSERT INTO attempts(id,job_id,token,storage_key,state,plan_json,created_at) VALUES(?,?,?,?,'submitted',?,?)",params![attempt.attempt_id.as_str(),attempt.job_id.as_str(),attempt.execution_token.as_str(),key,plan,now()])?;
    Ok(())
}
fn new_attempt(job_id: &JobId) -> Result<Attempt> {
    Ok(Attempt {
        attempt_id: AttemptId::new(new_key("attempt"))?,
        job_id: job_id.clone(),
        execution_token: ExecutionToken::new(new_key("token"))?,
    })
}
fn new_key(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}
fn sqlite_integer(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| LibraryError::Invalid("整数超出本地库范围".into()))
}
fn read_unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn bound_detail(value: &str) -> String {
    value.chars().take(4096).collect()
}
fn ensure_asset_active(record: &AssetRecord) -> Result<()> {
    if record.deletion_state == "active" {
        return Ok(());
    }
    Err(LibraryError::Invalid(
        match record.deletion_state.as_str() {
            "pending" => "作品正在删除，暂不可读取或导出",
            "failed" => "删除未完成，请先重试删除",
            _ => "作品已删除",
        }
        .into(),
    ))
}

fn validate_storage_key(key: &str) -> Result<()> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(LibraryError::Invalid("非法存储键".into()));
    }
    Ok(())
}

fn load_job(db: &Connection, id: &JobId) -> Result<JobRecord> {
    let attempt: String = db
        .query_row(
            "SELECT current_attempt_id FROM jobs WHERE id=?",
            [id.as_str()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| LibraryError::NotFound(id.to_string()))?;
    load_attempt(db, &attempt)
}
fn load_attempt(db: &Connection, id: &str) -> Result<JobRecord> {
    let row=db.query_row("SELECT j.id,j.request_json,a.id,a.token,a.state,a.plan_json,a.manifest_json,a.error,t.id,t.conversation_id FROM attempts a JOIN jobs j ON j.id=a.job_id LEFT JOIN generation_turns t ON t.job_id=j.id WHERE a.id=?",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?))).optional()?.ok_or_else(||LibraryError::NotFound(id.into()))?;
    let job_id = JobId::new(row.0)?;
    Ok(JobRecord {
        turn_id: row.8.map(TurnId::new).transpose()?,
        conversation_id: row.9.map(ConversationId::new).transpose()?,
        attempt: Attempt {
            job_id: job_id.clone(),
            attempt_id: AttemptId::new(row.2)?,
            execution_token: ExecutionToken::new(row.3)?,
        },
        job_id,
        state: serde_json::from_value(Value::String(row.4))?,
        request: UserRequest::from_json(&row.1)?,
        plan: ExecutionPlan::from_json(&row.5)?,
        manifest: row.6.map(|j| RunManifest::from_json(&j)).transpose()?,
        error: row.7,
    })
}
fn current_job(db: &Connection, attempt: &Attempt) -> Result<JobRecord> {
    let current = load_job(db, &attempt.job_id)?;
    current.attempt.validate_event(attempt)?;
    Ok(current)
}
fn write_terminal(
    db: &Connection,
    record: &JobRecord,
    state: AttemptState,
    result: FinalResult,
    outputs: Vec<AssetRef>,
    error: Option<String>,
) -> Result<()> {
    if record.state.is_terminal() {
        return Err(LibraryError::StaleExecution);
    }
    let manifest = RunManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        evidence_kind: EvidenceKind::ExecutionRecord,
        job_id: record.job_id.clone(),
        attempt: record.attempt.clone(),
        request: record.request.clone(),
        plan: record.plan.clone(),
        input_assets: record
            .request
            .inputs
            .iter()
            .map(|v| v.asset.clone())
            .collect(),
        outputs,
        final_result: result,
    };
    let changed = db.execute(
        "UPDATE attempts SET state=?,manifest_json=?,error=?,completed_at=? WHERE id=? AND manifest_json IS NULL",
        params![
            state.as_str(),
            manifest.to_json()?,
            error,
            now(),
            record.attempt.attempt_id.as_str()
        ],
    )?;
    if changed != 1 {
        return Err(LibraryError::StaleExecution);
    }
    Ok(())
}

fn managed_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_dir() => Ok(()),
        Ok(_) => Err(LibraryError::Invalid(format!(
            "管理目录不是普通目录：{}",
            path.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
fn open_regular(path: &Path, create: bool) -> Result<File> {
    super::safe_fs::open_regular(path, create, create, false).map_err(Into::into)
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        OpenOptions::new()
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)?
            .sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
