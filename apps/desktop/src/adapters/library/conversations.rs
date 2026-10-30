use super::*;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MODEL_ID: &str = "z-image-turbo-mflux-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Pending,
    CancelRequested,
    Cancelled,
    Rejected,
    Interrupted,
    Job,
}

impl TryFrom<&str> for TurnStatus {
    type Error = LibraryError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "cancel_requested" => Ok(Self::CancelRequested),
            "cancelled" => Ok(Self::Cancelled),
            "rejected" => Ok(Self::Rejected),
            "interrupted" => Ok(Self::Interrupted),
            "job" => Ok(Self::Job),
            _ => Err(LibraryError::Schema(format!("未知会话轮次状态：{value}"))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRecord {
    pub id: ConversationId,
    pub draft_id: u64,
    pub title: String,
    pub title_source: String,
    pub created_at: u64,
    pub activity_seq: u64,
    pub running: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRecord {
    pub id: TurnId,
    pub conversation_id: ConversationId,
    pub sequence: u64,
    pub submission_key: String,
    pub source_draft_id: Option<u64>,
    pub prompt: String,
    pub model_id: String,
    pub aspect: String,
    pub count: u32,
    pub seed: String,
    pub status: TurnStatus,
    pub job: Option<JobRecord>,
    pub error: Option<String>,
    pub progress_stage: Option<String>,
    pub progress_step: Option<u32>,
    pub progress_total: Option<u32>,
    pub received_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationListPage {
    pub conversations: Vec<ConversationRecord>,
    pub has_more: bool,
    pub next_cursor: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPage {
    pub turns: Vec<TurnRecord>,
    pub has_more: bool,
    pub next_cursor: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct CreatedConversation {
    pub conversation: ConversationRecord,
    pub draft: DraftRecord,
}

#[derive(Clone, Debug)]
pub struct AcceptedTurn {
    pub turn: TurnRecord,
    pub draft: DraftRecord,
    pub created: bool,
}

#[derive(Clone, Debug)]
pub struct BoundTurn {
    pub turn: TurnRecord,
    pub job: JobRecord,
}

#[derive(Clone, Debug)]
pub struct AcceptTurnInput {
    pub conversation_id: ConversationId,
    pub draft_id: u64,
    pub expected_version: u64,
    pub submission_key: IdempotencyKey,
    pub prompt: String,
    pub model_id: String,
    pub aspect: String,
    pub count: u32,
    pub seed: String,
}

pub(super) fn migrate_v1_to_v2(db: &mut Connection) -> Result<()> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "
ALTER TABLE projects ADD COLUMN active_conversation_id TEXT;
CREATE TABLE conversations(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), draft_id INTEGER NOT NULL UNIQUE REFERENCES drafts(id), title TEXT NOT NULL, title_source TEXT NOT NULL CHECK(title_source IN ('auto','manual')), created_at INTEGER NOT NULL, activity_seq INTEGER NOT NULL);
CREATE INDEX conversations_project_activity ON conversations(project_id, activity_seq DESC, id);
CREATE TABLE generation_turns(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), conversation_id TEXT NOT NULL REFERENCES conversations(id), sequence INTEGER NOT NULL CHECK(sequence > 0), submission_key TEXT NOT NULL, source_draft_id INTEGER REFERENCES drafts(id), prompt TEXT NOT NULL, model_id TEXT NOT NULL, aspect TEXT NOT NULL, count INTEGER NOT NULL CHECK(count > 0), seed TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending','cancel_requested','cancelled','rejected','interrupted','job')), job_id TEXT UNIQUE REFERENCES jobs(id), error TEXT, progress_stage TEXT, progress_step INTEGER, progress_total INTEGER, received_at INTEGER NOT NULL, UNIQUE(project_id, submission_key), UNIQUE(conversation_id, sequence));
CREATE INDEX generation_turns_conversation_sequence ON generation_turns(conversation_id, sequence DESC);
CREATE INDEX generation_turns_job ON generation_turns(job_id);
",
    )?;
    tx.execute(
        "UPDATE drafts SET params_json=json_set(params_json,'$.modelId',?1) WHERE json_extract(params_json,'$.modelId') IS NULL",
        [DEFAULT_MODEL_ID],
    )?;
    let drafts = {
        let mut statement =
            tx.prepare("SELECT id,name FROM drafts WHERE project_id='default' ORDER BY id")?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let created_at = now();
    for (id, name) in &drafts {
        let title_source = if name == &format!("草稿 {id}") {
            "auto"
        } else {
            "manual"
        };
        tx.execute(
            "INSERT INTO conversations(id,project_id,draft_id,title,title_source,created_at,activity_seq) VALUES(?,'default',?,?,?,?,?)",
            params![
                format!("conv-draft-{id}"),
                id,
                name,
                title_source,
                created_at,
                id
            ],
        )?;
    }
    tx.execute(
        "UPDATE projects SET active_conversation_id=(SELECT id FROM conversations WHERE draft_id=projects.active_draft_id) WHERE id='default'",
        [],
    )?;

    let jobs = {
        let mut statement = tx.prepare(
            "SELECT j.id,j.request_json,a.error,j.created_at FROM jobs j LEFT JOIN attempts a ON a.id=j.current_attempt_id WHERE j.project_id='default' ORDER BY j.created_at,j.rowid",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    if !jobs.is_empty() {
        tx.execute(
            "INSERT INTO drafts(project_id,name,prompt,params_json,collapsed,version) VALUES('default','历史生成','',?,0,1)",
            [serde_json::json!({
                "modelId": DEFAULT_MODEL_ID,
                "aspect": "1:1",
                "count": 1,
                "seed": ""
            })
            .to_string()],
        )?;
        let draft_id = u64::try_from(tx.last_insert_rowid())
            .map_err(|_| LibraryError::Schema("迁移草稿 ID 为负数".into()))?;
        let activity: i64 = tx.query_row(
            "SELECT coalesce(max(activity_seq),0)+1 FROM conversations WHERE project_id='default'",
            [],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO conversations(id,project_id,draft_id,title,title_source,created_at,activity_seq) VALUES('conv-history','default',?,'历史生成','manual',?,?)",
            params![sqlite_integer(draft_id)?, created_at, activity],
        )?;
        for (index, (job_id, request_json, error, created)) in jobs.into_iter().enumerate() {
            let request = UserRequest::from_json(&request_json)?;
            let aspect = match (
                request.requested_output.width,
                request.requested_output.height,
            ) {
                (1024, 1024) => "1:1",
                (1024, 768) => "4:3",
                (768, 1024) => "3:4",
                _ => "1:1",
            };
            let seed = request
                .parameters
                .get("seed")
                .and_then(Value::as_u64)
                .map_or_else(String::new, |value| value.to_string());
            tx.execute(
                "INSERT INTO generation_turns(id,project_id,conversation_id,sequence,submission_key,source_draft_id,prompt,model_id,aspect,count,seed,status,job_id,error,received_at) VALUES(?,?,'conv-history',?,?,NULL,?,?,?,?,?,'job',?,?,?)",
                params![
                    format!("turn-history-{}", index + 1),
                    "default",
                    sqlite_integer((index + 1) as u64)?,
                    request.idempotency_key.as_str(),
                    request.instruction,
                    DEFAULT_MODEL_ID,
                    aspect,
                    request.requested_output.count,
                    seed,
                    job_id,
                    error,
                    created,
                ],
            )?;
        }
    }
    tx.pragma_update(None, "user_version", 2)?;
    tx.commit()?;
    Ok(())
}

impl Library {
    pub fn active_conversation_id(&self) -> Result<ConversationId> {
        let id: Option<String> = self.db.query_row(
            "SELECT active_conversation_id FROM projects WHERE id='default'",
            [],
            |row| row.get(0),
        )?;
        id.map(ConversationId::new)
            .transpose()?
            .ok_or_else(|| LibraryError::Schema("活动会话缺失".into()))
    }

    pub fn conversations(
        &self,
        limit: u32,
        before_activity: Option<u64>,
    ) -> Result<ConversationListPage> {
        let limit = limit.clamp(1, 100);
        let fetch = i64::from(limit) + 1;
        let rows = if let Some(before) = before_activity {
            let mut statement = self.db.prepare(
                "SELECT c.id,c.draft_id,c.title,c.title_source,c.created_at,c.activity_seq, EXISTS(SELECT 1 FROM generation_turns t LEFT JOIN jobs j ON j.id=t.job_id LEFT JOIN attempts a ON a.id=j.current_attempt_id WHERE t.conversation_id=c.id AND (t.status IN ('pending','cancel_requested') OR a.state IN ('submitted','preparing','running','committing','cancel_requested'))) FROM conversations c WHERE c.project_id='default' AND c.activity_seq<? ORDER BY c.activity_seq DESC,c.id DESC LIMIT ?",
            )?;
            statement
                .query_map(params![sqlite_integer(before)?, fetch], conversation_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let mut statement = self.db.prepare(
                "SELECT c.id,c.draft_id,c.title,c.title_source,c.created_at,c.activity_seq, EXISTS(SELECT 1 FROM generation_turns t LEFT JOIN jobs j ON j.id=t.job_id LEFT JOIN attempts a ON a.id=j.current_attempt_id WHERE t.conversation_id=c.id AND (t.status IN ('pending','cancel_requested') OR a.state IN ('submitted','preparing','running','committing','cancel_requested'))) FROM conversations c WHERE c.project_id='default' ORDER BY c.activity_seq DESC,c.id DESC LIMIT ?",
            )?;
            statement
                .query_map([fetch], conversation_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let has_more = rows.len() > usize::try_from(limit).unwrap_or(usize::MAX);
        let mut conversations = rows;
        if has_more {
            conversations.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        let next_cursor = has_more
            .then(|| conversations.last().map(|value| value.activity_seq))
            .flatten();
        Ok(ConversationListPage {
            conversations,
            has_more,
            next_cursor,
        })
    }

    pub fn conversation(&self, id: &ConversationId) -> Result<ConversationRecord> {
        self.db
            .query_row(
                "SELECT c.id,c.draft_id,c.title,c.title_source,c.created_at,c.activity_seq, EXISTS(SELECT 1 FROM generation_turns t LEFT JOIN jobs j ON j.id=t.job_id LEFT JOIN attempts a ON a.id=j.current_attempt_id WHERE t.conversation_id=c.id AND (t.status IN ('pending','cancel_requested') OR a.state IN ('submitted','preparing','running','committing','cancel_requested'))) FROM conversations c WHERE c.id=? AND c.project_id='default'",
                [id.as_str()],
                conversation_row,
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(format!("conversation {id}")))
    }

    pub fn create_conversation(&mut self) -> Result<CreatedConversation> {
        self.create_conversation_with_params(serde_json::json!({
            "modelId": DEFAULT_MODEL_ID,
            "aspect": "1:1",
            "count": 1,
            "seed": ""
        }))
    }

    pub fn create_conversation_with_params(
        &mut self,
        params: Value,
    ) -> Result<CreatedConversation> {
        if !params.is_object() || serde_json::to_vec(&params)?.len() > 65536 {
            return Err(LibraryError::Invalid("新会话参数结构无效".into()));
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO drafts(project_id,name,prompt,params_json,collapsed,version) VALUES('default','新会话','',?,0,1)",
            [serde_json::to_string(&params)?],
        )?;
        let draft_id = u64::try_from(tx.last_insert_rowid())
            .map_err(|_| LibraryError::Schema("新草稿 ID 为负数".into()))?;
        let conversation_id = new_key("conv");
        let activity: i64 = tx.query_row(
            "SELECT coalesce(max(activity_seq),0)+1 FROM conversations WHERE project_id='default'",
            [],
            |row| row.get(0),
        )?;
        let created_at = now();
        tx.execute(
            "INSERT INTO conversations(id,project_id,draft_id,title,title_source,created_at,activity_seq) VALUES(?,? ,?,'新会话','auto',?,?)",
            params![
                conversation_id,
                "default",
                sqlite_integer(draft_id)?,
                created_at,
                activity
            ],
        )?;
        tx.execute(
            "UPDATE projects SET active_draft_id=?,active_conversation_id=?,selected_asset_id=NULL WHERE id='default'",
            params![sqlite_integer(draft_id)?, conversation_id],
        )?;
        tx.commit()?;
        Ok(CreatedConversation {
            conversation: self.conversation(&ConversationId::new(conversation_id)?)?,
            draft: self.draft(draft_id)?,
        })
    }

    pub fn select_conversation(&mut self, id: &ConversationId) -> Result<()> {
        let draft_id: i64 = self
            .db
            .query_row(
                "SELECT draft_id FROM conversations WHERE id=? AND project_id='default'",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(format!("conversation {id}")))?;
        self.db.execute(
            "UPDATE projects SET active_conversation_id=?,active_draft_id=?,selected_asset_id=NULL WHERE id='default'",
            params![id.as_str(), draft_id],
        )?;
        Ok(())
    }

    pub fn apply_model_to_conversation(
        &mut self,
        id: &ConversationId,
        model_id: &str,
    ) -> Result<DraftRecord> {
        let conversation = self.conversation(id)?;
        let mut draft = self.draft(conversation.draft_id)?;
        let params = draft
            .params
            .as_object_mut()
            .ok_or_else(|| LibraryError::Invalid("会话创作参数不是对象".into()))?;
        params.insert("modelId".into(), Value::String(model_id.to_owned()));
        let version = draft.version;
        self.save_draft(draft, version)
    }

    pub fn rename_conversation(
        &mut self,
        id: &ConversationId,
        title: &str,
    ) -> Result<ConversationRecord> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 128 || title.contains('\0') {
            return Err(LibraryError::Invalid("会话名称须为 1–128 个字符".into()));
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let activity: i64 = tx.query_row(
            "SELECT coalesce(max(activity_seq),0)+1 FROM conversations WHERE project_id='default'",
            [],
            |row| row.get(0),
        )?;
        let changed = tx.execute(
            "UPDATE conversations SET title=?,title_source='manual',activity_seq=? WHERE id=? AND project_id='default'",
            params![title, activity, id.as_str()],
        )?;
        if changed != 1 {
            return Err(LibraryError::NotFound(format!("conversation {id}")));
        }
        tx.execute(
            "UPDATE drafts SET name=? WHERE id=(SELECT draft_id FROM conversations WHERE id=?)",
            params![title, id.as_str()],
        )?;
        tx.commit()?;
        self.conversation(id)
    }

    pub fn turns(
        &self,
        conversation_id: &ConversationId,
        limit: u32,
        before_sequence: Option<u64>,
    ) -> Result<TurnPage> {
        self.conversation(conversation_id)?;
        let limit = limit.clamp(1, 100);
        let fetch = i64::from(limit) + 1;
        let ids = if let Some(before) = before_sequence {
            let mut statement = self.db.prepare(
                "SELECT id FROM generation_turns WHERE conversation_id=? AND sequence<? ORDER BY sequence DESC LIMIT ?",
            )?;
            statement
                .query_map(
                    params![conversation_id.as_str(), sqlite_integer(before)?, fetch],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let mut statement = self.db.prepare(
                "SELECT id FROM generation_turns WHERE conversation_id=? ORDER BY sequence DESC LIMIT ?",
            )?;
            statement
                .query_map(params![conversation_id.as_str(), fetch], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let has_more = ids.len() > usize::try_from(limit).unwrap_or(usize::MAX);
        let mut ids = ids;
        if has_more {
            ids.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        let mut turns = ids
            .into_iter()
            .map(|id| self.turn(&TurnId::new(id)?))
            .collect::<Result<Vec<_>>>()?;
        turns.reverse();
        let next_cursor = has_more
            .then(|| turns.first().map(|turn| turn.sequence))
            .flatten();
        Ok(TurnPage {
            turns,
            has_more,
            next_cursor,
        })
    }

    pub fn turn(&self, id: &TurnId) -> Result<TurnRecord> {
        let row = self
            .db
            .query_row(
                "SELECT id,conversation_id,sequence,submission_key,source_draft_id,prompt,model_id,aspect,count,seed,status,job_id,error,progress_stage,progress_step,progress_total,received_at FROM generation_turns WHERE id=?",
                [id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, Option<i64>>(14)?,
                        row.get::<_, Option<i64>>(15)?,
                        row.get::<_, i64>(16)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(format!("turn {id}")))?;
        let job = row
            .11
            .as_deref()
            .map(JobId::new)
            .transpose()?
            .map(|id| self.job(&id))
            .transpose()?;
        Ok(TurnRecord {
            id: TurnId::new(row.0)?,
            conversation_id: ConversationId::new(row.1)?,
            sequence: unsigned_from_i64(row.2, "turn sequence")?,
            submission_key: row.3,
            source_draft_id: row
                .4
                .map(|value| unsigned_from_i64(value, "source draft"))
                .transpose()?,
            prompt: row.5,
            model_id: row.6,
            aspect: row.7,
            count: u32::try_from(unsigned_from_i64(row.8, "turn count")?)
                .map_err(|_| LibraryError::Schema("轮次数量超出范围".into()))?,
            seed: row.9,
            status: TurnStatus::try_from(row.10.as_str())?,
            job,
            error: row.12,
            progress_stage: row.13,
            progress_step: row
                .14
                .map(|value| {
                    u32::try_from(unsigned_from_i64(value, "progress step").unwrap_or(u64::MAX))
                })
                .transpose()
                .map_err(|_| LibraryError::Schema("进度步骤超出范围".into()))?,
            progress_total: row
                .15
                .map(|value| {
                    u32::try_from(unsigned_from_i64(value, "progress total").unwrap_or(u64::MAX))
                })
                .transpose()
                .map_err(|_| LibraryError::Schema("进度总数超出范围".into()))?,
            received_at: unsigned_from_i64(row.16, "received_at")?,
        })
    }

    pub fn accept_turn(&mut self, input: AcceptTurnInput) -> Result<AcceptedTurn> {
        validate_turn_input(&input)?;
        let params_json = turn_params_json(&input)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = tx
            .query_row(
                "SELECT id,conversation_id,source_draft_id,prompt,model_id,aspect,count,seed FROM generation_turns WHERE project_id='default' AND submission_key=?",
                [input.submission_key.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?
        {
            let matches = existing.1 == input.conversation_id.as_str()
                && existing.2 == Some(sqlite_integer(input.draft_id)?)
                && existing.3 == input.prompt
                && existing.4 == input.model_id
                && existing.5 == input.aspect
                && u32::try_from(existing.6).ok() == Some(input.count)
                && existing.7 == input.seed;
            if !matches {
                return Err(LibraryError::IdempotencyConflict);
            }
            drop(tx);
            return Ok(AcceptedTurn {
                turn: self.turn(&TurnId::new(existing.0)?)?,
                draft: self.draft(input.draft_id)?,
                created: false,
            });
        }
        let conversation = tx
            .query_row(
                "SELECT draft_id FROM conversations WHERE id=? AND project_id='default'",
                [input.conversation_id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or_else(|| {
                LibraryError::NotFound(format!("conversation {}", input.conversation_id))
            })?;
        if conversation != sqlite_integer(input.draft_id)? {
            return Err(LibraryError::Invalid("草稿不属于指定会话".into()));
        }
        ensure_idle(&tx)?;
        let current = tx
            .query_row(
                "SELECT prompt,params_json,version FROM drafts WHERE id=? AND project_id='default'",
                [sqlite_integer(input.draft_id)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        read_unsigned(row, 2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(format!("draft {}", input.draft_id)))?;
        if current.2 != input.expected_version
            || current.0 != input.prompt
            || current.1 != params_json
        {
            return Err(LibraryError::TurnConflict);
        }
        let sequence: i64 = tx.query_row(
            "SELECT coalesce(max(sequence),0)+1 FROM generation_turns WHERE conversation_id=?",
            [input.conversation_id.as_str()],
            |row| row.get(0),
        )?;
        let turn_id = new_key("turn");
        let received_at = now();
        tx.execute(
            "INSERT INTO generation_turns(id,project_id,conversation_id,sequence,submission_key,source_draft_id,prompt,model_id,aspect,count,seed,status,received_at) VALUES(?,? ,?,?,?,?,?,?,?,?,?,'pending',?)",
            params![
                turn_id,
                "default",
                input.conversation_id.as_str(),
                sequence,
                input.submission_key.as_str(),
                sqlite_integer(input.draft_id)?,
                input.prompt,
                input.model_id,
                input.aspect,
                input.count,
                input.seed,
                received_at,
            ],
        )?;
        let next_version = input
            .expected_version
            .checked_add(1)
            .ok_or_else(|| LibraryError::Invalid("草稿版本溢出".into()))?;
        let auto_title = auto_title(&input.prompt);
        let title_source: String = tx.query_row(
            "SELECT title_source FROM conversations WHERE id=?",
            [input.conversation_id.as_str()],
            |row| row.get(0),
        )?;
        let first_auto_turn = sequence == 1 && title_source == "auto";
        let changed = tx.execute(
            "UPDATE drafts SET prompt='',name=CASE WHEN ? THEN ? ELSE name END,version=? WHERE id=? AND version=? AND prompt=?",
            params![
                first_auto_turn,
                auto_title,
                sqlite_integer(next_version)?,
                sqlite_integer(input.draft_id)?,
                sqlite_integer(input.expected_version)?,
                input.prompt,
            ],
        )?;
        if changed != 1 {
            return Err(LibraryError::TurnConflict);
        }
        tx.execute(
            "UPDATE conversations SET title=CASE WHEN ? THEN ? ELSE title END,activity_seq=(SELECT coalesce(max(activity_seq),0)+1 FROM conversations WHERE project_id='default') WHERE id=?",
            params![first_auto_turn, auto_title, input.conversation_id.as_str()],
        )?;
        tx.execute(
            "UPDATE projects SET active_conversation_id=?,active_draft_id=?,selected_asset_id=NULL WHERE id='default'",
            params![input.conversation_id.as_str(), sqlite_integer(input.draft_id)?],
        )?;
        tx.commit()?;
        Ok(AcceptedTurn {
            turn: self.turn(&TurnId::new(turn_id)?)?,
            draft: self.draft(input.draft_id)?,
            created: true,
        })
    }

    pub fn bind_turn_job(
        &mut self,
        turn_id: &TurnId,
        request: UserRequest,
        plan: ExecutionPlan,
    ) -> Result<BoundTurn> {
        validate_z_image_plan(&request, &plan)?;
        let turn = self.turn(turn_id)?;
        let (width, height) = match turn.aspect.as_str() {
            "1:1" => (1024, 1024),
            "4:3" => (1024, 768),
            "3:4" => (768, 1024),
            _ => return Err(LibraryError::Invalid("轮次画幅无效".into())),
        };
        let requested_seed = if turn.seed.is_empty() {
            None
        } else {
            Some(
                turn.seed
                    .parse::<u32>()
                    .map_err(|_| LibraryError::Invalid("轮次种子不是 u32".into()))?,
            )
        };
        if turn.model_id != DEFAULT_MODEL_ID
            || request.instruction != turn.prompt
            || request.requested_output.width != width
            || request.requested_output.height != height
            || request.requested_output.count != turn.count
            || request
                .parameters
                .get("seed")
                .and_then(Value::as_u64)
                .map(|value| value as u32)
                != requested_seed
            || (requested_seed.is_none()
                && request
                    .parameters
                    .get("seed")
                    .is_some_and(|value| !value.is_null()))
        {
            return Err(LibraryError::Invalid(
                "冻结请求与已接收轮次快照不一致".into(),
            ));
        }
        let request_json = request.to_json()?;
        let plan_json = plan.to_json()?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = load_turn_row(&tx, turn_id)?;
        if current.status == TurnStatus::CancelRequested {
            tx.execute(
                "UPDATE generation_turns SET status='cancelled' WHERE id=? AND status='cancel_requested'",
                [turn_id.as_str()],
            )?;
            tx.commit()?;
            return Err(LibraryError::TurnConflict);
        }
        if current.status != TurnStatus::Pending || current.job.is_some() {
            return Err(LibraryError::TurnConflict);
        }
        ensure_no_active_attempt(&tx)?;
        ensure_no_other_pending_turn(&tx, turn_id)?;
        let job_id = JobId::new(new_key("job"))?;
        let attempt = new_attempt(&job_id)?;
        let storage_key = new_key("run");
        tx.execute(
            "INSERT INTO jobs(id,project_id,submission_key,request_json,current_attempt_id,created_at) VALUES(?,? ,?,?,?,?)",
            params![
                job_id.as_str(),
                "default",
                request.idempotency_key.as_str(),
                request_json,
                attempt.attempt_id.as_str(),
                now(),
            ],
        )?;
        insert_attempt(&tx, &attempt, &storage_key, &plan_json)?;
        let changed = tx.execute(
            "UPDATE generation_turns SET status='job',job_id=?,progress_stage='submitted' WHERE id=? AND status='pending' AND job_id IS NULL",
            params![job_id.as_str(), turn_id.as_str()],
        )?;
        if changed != 1 {
            return Err(LibraryError::TurnConflict);
        }
        tx.commit()?;
        self.prepare_run_directory(&attempt, &storage_key)?;
        Ok(BoundTurn {
            turn: self.turn(turn_id)?,
            job: self.job(&job_id)?,
        })
    }

    pub fn reject_turn(&mut self, turn_id: &TurnId, error: &str) -> Result<TurnRecord> {
        let detail = bound_detail(error);
        self.db.execute(
            "UPDATE generation_turns SET status=CASE WHEN status='cancel_requested' THEN 'cancelled' ELSE 'rejected' END,error=? WHERE id=? AND status IN ('pending','cancel_requested') AND job_id IS NULL",
            params![detail, turn_id.as_str()],
        )?;
        self.turn(turn_id)
    }

    pub fn request_cancel_unbound_turn(&mut self, turn_id: &TurnId) -> Result<TurnRecord> {
        let changed = self.db.execute(
            "UPDATE generation_turns SET status='cancel_requested' WHERE id=? AND status='pending' AND job_id IS NULL",
            [turn_id.as_str()],
        )?;
        let turn = self.turn(turn_id)?;
        if changed == 0
            && !matches!(
                turn.status,
                TurnStatus::CancelRequested | TurnStatus::Cancelled
            )
        {
            return Err(LibraryError::TurnConflict);
        }
        Ok(turn)
    }

    pub fn finish_cancelled_turn(&mut self, turn_id: &TurnId) -> Result<TurnRecord> {
        self.db.execute(
            "UPDATE generation_turns SET status='cancelled' WHERE id=? AND status='cancel_requested' AND job_id IS NULL",
            [turn_id.as_str()],
        )?;
        self.turn(turn_id)
    }

    pub fn update_turn_progress(
        &self,
        job_id: &JobId,
        stage: &str,
        step: Option<u32>,
        total: Option<u32>,
    ) -> Result<()> {
        self.db.execute(
            "UPDATE generation_turns SET progress_stage=?,progress_step=?,progress_total=? WHERE job_id=?",
            params![stage, step, total, job_id.as_str()],
        )?;
        Ok(())
    }

    pub fn has_active_generation(&self) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM attempts WHERE state IN ('submitted','preparing','running','committing','cancel_requested')) OR EXISTS(SELECT 1 FROM generation_turns WHERE status IN ('pending','cancel_requested') AND job_id IS NULL)",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn draft(&self, id: u64) -> Result<DraftRecord> {
        let (name, prompt, params, collapsed, version) = self
            .db
            .query_row(
                "SELECT name,prompt,params_json,collapsed,version FROM drafts WHERE id=? AND project_id='default'",
                [sqlite_integer(id)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, bool>(3)?,
                        read_unsigned(row, 4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(format!("draft {id}")))?;
        Ok(DraftRecord {
            id,
            name,
            prompt,
            params: serde_json::from_str(&params)?,
            collapsed,
            version,
        })
    }

    pub(super) fn recover_pending_turns(&mut self) -> Result<()> {
        self.db.execute(
            "UPDATE generation_turns SET status='interrupted',error=coalesce(error,'宿主在预检完成前退出；可编辑后重新发送') WHERE status='pending' AND job_id IS NULL",
            [],
        )?;
        self.db.execute(
            "UPDATE generation_turns SET status='cancelled' WHERE status='cancel_requested' AND job_id IS NULL",
            [],
        )?;
        Ok(())
    }
}

fn conversation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationRecord> {
    Ok(ConversationRecord {
        id: ConversationId::new(row.get::<_, String>(0)?)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        draft_id: read_unsigned(row, 1)?,
        title: row.get(2)?,
        title_source: row.get(3)?,
        created_at: read_unsigned(row, 4)?,
        activity_seq: read_unsigned(row, 5)?,
        running: row.get(6)?,
    })
}

fn load_turn_row(db: &Connection, id: &TurnId) -> Result<TurnRecord> {
    let job: Option<String> = db
        .query_row(
            "SELECT job_id FROM generation_turns WHERE id=?",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| LibraryError::NotFound(format!("turn {id}")))?;
    let reader = TurnRowReader {
        db,
        job,
        id: id.clone(),
    };
    reader.read()
}

struct TurnRowReader<'a> {
    db: &'a Connection,
    job: Option<String>,
    id: TurnId,
}

impl TurnRowReader<'_> {
    fn read(self) -> Result<TurnRecord> {
        let row = self.db.query_row(
            "SELECT conversation_id,sequence,submission_key,source_draft_id,prompt,model_id,aspect,count,seed,status,error,progress_stage,progress_step,progress_total,received_at FROM generation_turns WHERE id=?",
            [self.id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<i64>>(12)?,
                    row.get::<_, Option<i64>>(13)?,
                    row.get::<_, i64>(14)?,
                ))
            },
        )?;
        let job = self
            .job
            .as_deref()
            .map(JobId::new)
            .transpose()?
            .map(|job_id| load_job(self.db, &job_id))
            .transpose()?;
        Ok(TurnRecord {
            id: self.id,
            conversation_id: ConversationId::new(row.0)?,
            sequence: unsigned_from_i64(row.1, "turn sequence")?,
            submission_key: row.2,
            source_draft_id: row
                .3
                .map(|value| unsigned_from_i64(value, "source draft"))
                .transpose()?,
            prompt: row.4,
            model_id: row.5,
            aspect: row.6,
            count: u32::try_from(unsigned_from_i64(row.7, "turn count")?)
                .map_err(|_| LibraryError::Schema("轮次数量超出范围".into()))?,
            seed: row.8,
            status: TurnStatus::try_from(row.9.as_str())?,
            job,
            error: row.10,
            progress_stage: row.11,
            progress_step: optional_u32(row.12, "progress step")?,
            progress_total: optional_u32(row.13, "progress total")?,
            received_at: unsigned_from_i64(row.14, "received_at")?,
        })
    }
}

fn ensure_no_active_attempt(db: &Connection) -> Result<()> {
    let active: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM attempts WHERE state IN ('submitted','preparing','running','committing','cancel_requested'))",
        [],
        |row| row.get(0),
    )?;
    if active {
        Err(LibraryError::Busy)
    } else {
        Ok(())
    }
}

fn ensure_no_other_pending_turn(db: &Connection, turn_id: &TurnId) -> Result<()> {
    let active: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM generation_turns WHERE id<>? AND status IN ('pending','cancel_requested') AND job_id IS NULL)",
        [turn_id.as_str()],
        |row| row.get(0),
    )?;
    if active {
        Err(LibraryError::Busy)
    } else {
        Ok(())
    }
}

fn validate_turn_input(input: &AcceptTurnInput) -> Result<()> {
    if input.prompt.trim().is_empty()
        || input.prompt.len() > studio_core::generation::MAX_PROMPT_BYTES
        || !["1:1", "4:3", "3:4"].contains(&input.aspect.as_str())
        || input.count != 1
        || !valid_seed_text(&input.seed)
        || input.model_id != DEFAULT_MODEL_ID
    {
        return Err(LibraryError::Invalid("发送内容或参数无效".into()));
    }
    Ok(())
}

fn valid_seed_text(seed: &str) -> bool {
    seed.is_empty()
        || (!seed.is_empty()
            && seed.bytes().all(|value| value.is_ascii_digit())
            && seed.parse::<u32>().is_ok())
}

fn turn_params_json(input: &AcceptTurnInput) -> Result<String> {
    Ok(serde_json::json!({
        "modelId": input.model_id,
        "aspect": input.aspect,
        "count": input.count,
        "seed": input.seed
    })
    .to_string())
}

fn auto_title(prompt: &str) -> String {
    prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("新会话")
        .chars()
        .take(24)
        .collect()
}

fn unsigned_from_i64(value: i64, field: &str) -> Result<u64> {
    u64::try_from(value).map_err(|_| LibraryError::Schema(format!("{field} 为负数")))
}

fn optional_u32(value: Option<i64>, field: &str) -> Result<Option<u32>> {
    value
        .map(|value| {
            u32::try_from(unsigned_from_i64(value, field)?)
                .map_err(|_| LibraryError::Schema(format!("{field} 超出范围")))
        })
        .transpose()
}
