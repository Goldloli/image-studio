use super::*;
use rusqlite::{OptionalExtension, params, params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};

const DEFAULT_ASSET_LIMIT: u32 = 24;
const DEFAULT_TASK_LIMIT: u32 = 30;
const MAX_QUERY_LIMIT: u32 = 100;
const MAX_DETAIL_LOG_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetLibraryQuery {
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub created_from: Option<u64>,
    #[serde(default)]
    pub created_to: Option<u64>,
    #[serde(default)]
    pub cursor: Option<StableCursor>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableCursor {
    pub created_at: u64,
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetLibraryItem {
    pub id: String,
    pub job_id: String,
    pub attempt_id: String,
    pub turn_id: Option<String>,
    pub turn_sequence: Option<u64>,
    pub conversation_id: Option<String>,
    pub conversation_title: String,
    pub draft_id: Option<u64>,
    pub prompt: String,
    pub model_id: String,
    pub width: u32,
    pub height: u32,
    pub seed: Option<u64>,
    pub byte_count: u64,
    pub created_at: u64,
    pub available: bool,
    pub availability_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetModelFacet {
    pub id: String,
    pub count: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSizeFacet {
    pub width: u32,
    pub height: u32,
    pub count: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetConversationFacet {
    pub id: String,
    pub title: String,
    pub count: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetLibraryFacets {
    pub models: Vec<AssetModelFacet>,
    pub sizes: Vec<AssetSizeFacet>,
    pub conversations: Vec<AssetConversationFacet>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetLibraryPage {
    pub items: Vec<AssetLibraryItem>,
    pub total: u64,
    pub has_more: bool,
    pub next_cursor: Option<StableCursor>,
    pub facets: AssetLibraryFacets,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskCenterQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub cursor: Option<StableCursor>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCounts {
    pub total: u64,
    pub active: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub interrupted: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCenterItem {
    pub id: String,
    pub turn_id: Option<String>,
    pub turn_sequence: Option<u64>,
    pub conversation_id: Option<String>,
    pub conversation_title: String,
    pub draft_id: Option<u64>,
    pub attempt_id: String,
    pub state: String,
    pub prompt: String,
    pub model_id: String,
    pub width: u32,
    pub height: u32,
    pub seed: Option<u64>,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub error: Option<String>,
    pub asset_id: Option<String>,
    pub asset_available: bool,
    pub asset_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCenterPage {
    pub items: Vec<TaskCenterItem>,
    pub counts: TaskCounts,
    pub has_more: bool,
    pub next_cursor: Option<StableCursor>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAttemptView {
    pub attempt_id: String,
    pub state: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub phase: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAdvancedView {
    pub manifest: Option<Value>,
    pub output_hash: Option<String>,
    pub logs: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCenterDetail {
    pub task: TaskCenterItem,
    pub requested_width: u32,
    pub requested_height: u32,
    pub requested_seed: Option<Value>,
    pub attempts: Vec<TaskAttemptView>,
    pub error_category: String,
    pub next_action: String,
    pub advanced: TaskAdvancedView,
}

impl Library {
    pub fn asset_library_query(&self, query: AssetLibraryQuery) -> Result<AssetLibraryPage> {
        validate_time_range(query.created_from, query.created_to)?;
        let limit = query
            .limit
            .unwrap_or(DEFAULT_ASSET_LIMIT)
            .clamp(1, MAX_QUERY_LIMIT);
        let (filter_sql, filter_args) = asset_filters(&query)?;
        let total = count_query(
            &self.db,
            &format!("SELECT count(*) FROM {} WHERE {filter_sql}", asset_from()),
            filter_args.clone(),
        )?;
        let mut page_args = filter_args;
        let cursor_sql = push_asset_cursor(&query.cursor, &mut page_args);
        let sql = format!(
            "SELECT a.id FROM {} WHERE {filter_sql}{cursor_sql} ORDER BY a.created_at DESC,a.id DESC LIMIT ?",
            asset_from()
        );
        page_args.push(SqlValue::Integer(i64::from(limit) + 1));
        let ids = query_strings(&self.db, &sql, page_args)?;
        let has_more = ids.len() > usize::try_from(limit).unwrap_or(usize::MAX);
        let mut ids = ids;
        if has_more {
            ids.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        let items = ids
            .iter()
            .map(|id| self.asset_library_item(&AssetId::new(id.clone())?))
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = has_more
            .then(|| items.last())
            .flatten()
            .map(|item| StableCursor {
                created_at: item.created_at,
                id: item.id.clone(),
            });
        Ok(AssetLibraryPage {
            items,
            total,
            has_more,
            next_cursor,
            facets: self.asset_library_facets()?,
        })
    }

    pub fn asset_library_item(&self, id: &AssetId) -> Result<AssetLibraryItem> {
        let row = self
            .db
            .query_row(
                "SELECT a.id,a.job_id,a.attempt_id,t.id,t.sequence,t.conversation_id,coalesce(c.title,'历史生成'),c.draft_id,coalesce(t.prompt,json_extract(j.request_json,'$.instruction'),'生成图片'),coalesce(t.model_id,?2),a.width,a.height,json_extract(at.plan_json,'$.effective_parameters.seed'),a.byte_count,a.created_at FROM assets a JOIN jobs j ON j.id=a.job_id JOIN attempts at ON at.id=a.attempt_id LEFT JOIN generation_turns t ON t.job_id=j.id LEFT JOIN conversations c ON c.id=t.conversation_id WHERE a.id=?1 AND j.project_id='default'",
                params![id.as_str(), DEFAULT_MODEL_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, u32>(10)?,
                        row.get::<_, u32>(11)?,
                        row.get::<_, Option<i64>>(12)?
                            .map(|value| value.max(0) as u64),
                        read_unsigned(row, 13)?,
                        read_unsigned(row, 14)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| LibraryError::NotFound(format!("asset {id}")))?;
        let record = self.asset(id)?;
        Ok(AssetLibraryItem {
            id: row.0,
            job_id: row.1,
            attempt_id: row.2,
            turn_id: row.3,
            turn_sequence: row.4.map(|value| value.max(0) as u64),
            conversation_id: row.5,
            conversation_title: row.6,
            draft_id: row.7.map(|value| value.max(0) as u64),
            prompt: row.8,
            model_id: row.9,
            width: row.10,
            height: row.11,
            seed: row.12,
            byte_count: row.13,
            created_at: row.14,
            available: record.available,
            availability_error: record.availability_error,
        })
    }

    pub fn task_center_query(&self, query: TaskCenterQuery) -> Result<TaskCenterPage> {
        let limit = query
            .limit
            .unwrap_or(DEFAULT_TASK_LIMIT)
            .clamp(1, MAX_QUERY_LIMIT);
        let status = query.status.as_deref().unwrap_or("all");
        let status_sql = task_status_sql(status)?;
        let (filter_sql, filter_args) = task_filters(&query)?;
        let counts = self.task_center_counts(&filter_sql, filter_args.clone())?;
        let mut page_args = filter_args;
        let status_sql = if status_sql.is_empty() {
            String::new()
        } else {
            format!(" AND {status_sql}")
        };
        let cursor_sql = push_job_cursor(&query.cursor, &mut page_args);
        let sql = format!(
            "SELECT j.id FROM {} WHERE {filter_sql}{status_sql}{cursor_sql} ORDER BY j.created_at DESC,j.id DESC LIMIT ?",
            task_from()
        );
        page_args.push(SqlValue::Integer(i64::from(limit) + 1));
        let ids = query_strings(&self.db, &sql, page_args)?;
        let has_more = ids.len() > usize::try_from(limit).unwrap_or(usize::MAX);
        let mut ids = ids;
        if has_more {
            ids.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        }
        let items = ids
            .iter()
            .map(|id| self.task_center_item(&JobId::new(id.clone())?))
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = has_more
            .then(|| items.last())
            .flatten()
            .map(|item| StableCursor {
                created_at: item.created_at,
                id: item.id.clone(),
            });
        Ok(TaskCenterPage {
            items,
            counts,
            has_more,
            next_cursor,
        })
    }

    pub fn task_center_item(&self, id: &JobId) -> Result<TaskCenterItem> {
        let job = self.job(id)?;
        let timestamps = self
            .db
            .query_row(
                "SELECT j.created_at,a.completed_at FROM jobs j JOIN attempts a ON a.id=j.current_attempt_id WHERE j.id=?",
                [id.as_str()],
                |row| {
                    Ok((
                        read_unsigned(row, 0)?,
                        row.get::<_, Option<i64>>(1)?
                            .map(|value| value.max(0) as u64),
                    ))
                },
            )?;
        // Deleted conversations keep their title so historical job records stay
        // readable; only live conversation entry points are closed.
        let (conversation_title, draft_id) = job
            .conversation_id
            .as_ref()
            .map(|conversation_id| {
                self.conversation_identity(conversation_id).map(|identity| {
                    identity
                        .map(|value| (value.title, Some(value.draft_id)))
                        .unwrap_or_else(|| ("历史生成".into(), None))
                })
            })
            .transpose()?
            .unwrap_or_else(|| ("历史生成".into(), None));
        let asset_id = job
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.outputs.first())
            .map(|asset| asset.asset_id.clone());
        let asset_record = asset_id.as_ref().and_then(|id| self.asset(id).ok());
        let turn_record = job
            .turn_id
            .as_ref()
            .and_then(|turn_id| self.turn(turn_id).ok());
        Ok(TaskCenterItem {
            id: job.job_id.to_string(),
            turn_id: job.turn_id.as_ref().map(ToString::to_string),
            turn_sequence: turn_record.as_ref().map(|turn| turn.sequence),
            conversation_id: job.conversation_id.map(|id| id.to_string()),
            conversation_title,
            draft_id,
            attempt_id: job.attempt.attempt_id.to_string(),
            state: format!("{:?}", job.state),
            prompt: job.request.instruction,
            model_id: turn_record
                .as_ref()
                .map_or_else(|| DEFAULT_MODEL_ID.into(), |turn| turn.model_id.clone()),
            width: job.plan.effective_output.width,
            height: job.plan.effective_output.height,
            seed: job
                .plan
                .effective_parameters
                .get("seed")
                .and_then(Value::as_u64),
            created_at: timestamps.0,
            completed_at: timestamps.1,
            error: job.error,
            asset_id: asset_id.map(|id| id.to_string()),
            asset_available: asset_record.as_ref().is_some_and(|asset| asset.available),
            asset_error: asset_record.and_then(|asset| asset.availability_error),
        })
    }

    pub fn task_center_detail(&self, id: &JobId) -> Result<TaskCenterDetail> {
        let task = self.task_center_item(id)?;
        let job = self.job(id)?;
        let current_turn = job.turn_id.as_ref().and_then(|id| self.turn(id).ok());
        let mut attempts = self
            .attempts(id)?
            .into_iter()
            .map(|attempt_record| {
                let row = self.db.query_row(
                    "SELECT created_at,completed_at FROM attempts WHERE id=?",
                    [attempt_record.attempt.attempt_id.as_str()],
                    |row| {
                        Ok((
                            read_unsigned(row, 0)?,
                            row.get::<_, Option<i64>>(1)?
                                .map(|value| value.max(0) as u64),
                        ))
                    },
                )?;
                let current = attempt_record.attempt.attempt_id == job.attempt.attempt_id;
                Ok(TaskAttemptView {
                    attempt_id: attempt_record.attempt.attempt_id.to_string(),
                    state: format!("{:?}", attempt_record.state),
                    started_at: row.0,
                    completed_at: row.1,
                    phase: if current {
                        current_turn
                            .as_ref()
                            .and_then(|turn| turn.progress_stage.clone())
                    } else {
                        None
                    },
                    error: attempt_record.error,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        attempts.reverse();
        let storage_key: String = self.db.query_row(
            "SELECT storage_key FROM attempts WHERE id=?",
            [job.attempt.attempt_id.as_str()],
            |row| row.get(0),
        )?;
        let output_hash = job
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.outputs.first())
            .map(|asset| asset.content_hash.as_str().to_string());
        Ok(TaskCenterDetail {
            requested_width: job.request.requested_output.width,
            requested_height: job.request.requested_output.height,
            requested_seed: job.request.parameters.get("seed").cloned(),
            error_category: classify_error(&format!("{:?}", job.state), job.error.as_deref()),
            next_action: next_action(&task.state, task.conversation_id.is_some()),
            advanced: TaskAdvancedView {
                manifest: serde_json::to_value(&job.manifest).ok(),
                output_hash,
                logs: self.read_attempt_log(&storage_key)?,
            },
            attempts,
            task,
        })
    }

    fn task_center_counts(
        &self,
        filter_sql: &str,
        filter_args: Vec<SqlValue>,
    ) -> Result<TaskCounts> {
        let sql = format!(
            "SELECT count(*),coalesce(sum(a.state IN ('submitted','preparing','running','committing','cancel_requested')),0),coalesce(sum(a.state='succeeded'),0),coalesce(sum(a.state='failed'),0),coalesce(sum(a.state='cancelled'),0),coalesce(sum(a.state='interrupted'),0) FROM {} WHERE {filter_sql}",
            task_from()
        );
        self.db
            .query_row(&sql, params_from_iter(filter_args), |row| {
                Ok(TaskCounts {
                    total: read_unsigned(row, 0)?,
                    active: row.get::<_, i64>(1)?.max(0) as u64,
                    succeeded: row.get::<_, i64>(2)?.max(0) as u64,
                    failed: row.get::<_, i64>(3)?.max(0) as u64,
                    cancelled: row.get::<_, i64>(4)?.max(0) as u64,
                    interrupted: row.get::<_, i64>(5)?.max(0) as u64,
                })
            })
            .map_err(Into::into)
    }

    fn asset_library_facets(&self) -> Result<AssetLibraryFacets> {
        let models = {
            let mut statement = self.db.prepare(
                "SELECT coalesce(t.model_id,?1),count(*) FROM assets a JOIN jobs j ON j.id=a.job_id LEFT JOIN generation_turns t ON t.job_id=j.id WHERE j.project_id='default' AND a.output_role='primary' GROUP BY coalesce(t.model_id,?1) ORDER BY count(*) DESC,coalesce(t.model_id,?1)",
            )?;
            statement
                .query_map([DEFAULT_MODEL_ID], |row| {
                    Ok(AssetModelFacet {
                        id: row.get(0)?,
                        count: read_unsigned(row, 1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let sizes = {
            let mut statement = self.db.prepare(
                "SELECT a.width,a.height,count(*) FROM assets a JOIN jobs j ON j.id=a.job_id WHERE j.project_id='default' AND a.output_role='primary' GROUP BY a.width,a.height ORDER BY a.width*a.height DESC,a.width DESC",
            )?;
            statement
                .query_map([], |row| {
                    Ok(AssetSizeFacet {
                        width: row.get(0)?,
                        height: row.get(1)?,
                        count: read_unsigned(row, 2)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let conversations = {
            let mut statement = self.db.prepare(
                "SELECT c.id,c.title,count(*) FROM assets a JOIN jobs j ON j.id=a.job_id JOIN generation_turns t ON t.job_id=j.id JOIN conversations c ON c.id=t.conversation_id WHERE j.project_id='default' AND a.output_role='primary' GROUP BY c.id,c.title ORDER BY count(*) DESC,c.title",
            )?;
            statement
                .query_map([], |row| {
                    Ok(AssetConversationFacet {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        count: read_unsigned(row, 2)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(AssetLibraryFacets {
            models,
            sizes,
            conversations,
        })
    }

    fn read_attempt_log(&self, storage_key: &str) -> Result<String> {
        validate_storage_key(storage_key)?;
        let path = self.root.join("runs").join(storage_key).join("worker.log");
        let mut file = match super::open_regular(&path, false) {
            Ok(file) => file,
            Err(_) => return Ok(String::new()),
        };
        let length = file.metadata()?.len();
        if length > MAX_DETAIL_LOG_BYTES {
            file.seek(SeekFrom::Start(length - MAX_DETAIL_LOG_BYTES))?;
        }
        let mut bytes = Vec::new();
        file.take(MAX_DETAIL_LOG_BYTES).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

pub(super) fn migrate_v2_to_v3(db: &mut Connection) -> Result<()> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "
ALTER TABLE attempts ADD COLUMN completed_at INTEGER;
CREATE INDEX jobs_project_created ON jobs(project_id, created_at DESC, id DESC);
CREATE INDEX assets_created ON assets(created_at DESC, id DESC);
",
    )?;
    tx.pragma_update(None, "user_version", 3)?;
    tx.commit()?;
    Ok(())
}

fn asset_from() -> &'static str {
    "assets a JOIN jobs j ON j.id=a.job_id LEFT JOIN generation_turns t ON t.job_id=j.id LEFT JOIN conversations c ON c.id=t.conversation_id"
}

fn task_from() -> &'static str {
    "jobs j JOIN attempts a ON a.id=j.current_attempt_id LEFT JOIN generation_turns t ON t.job_id=j.id LEFT JOIN conversations c ON c.id=t.conversation_id"
}

fn asset_filters(query: &AssetLibraryQuery) -> Result<(String, Vec<SqlValue>)> {
    let mut clauses = vec![
        "j.project_id='default'".to_string(),
        "a.output_role='primary'".to_string(),
    ];
    let mut args = Vec::new();
    if let Some(model_id) = nonempty_filter(&query.model_id) {
        clauses.push("coalesce(t.model_id,?) = ?".into());
        args.push(SqlValue::Text(DEFAULT_MODEL_ID.into()));
        args.push(SqlValue::Text(model_id.into()));
    }
    if let Some(conversation_id) = nonempty_filter(&query.conversation_id) {
        clauses.push("t.conversation_id=?".into());
        args.push(SqlValue::Text(conversation_id.into()));
    }
    if let Some(width) = query.width {
        clauses.push("a.width=?".into());
        args.push(SqlValue::Integer(i64::from(width)));
    }
    if let Some(height) = query.height {
        clauses.push("a.height=?".into());
        args.push(SqlValue::Integer(i64::from(height)));
    }
    if let Some(from) = query.created_from {
        clauses.push("a.created_at>=?".into());
        args.push(SqlValue::Integer(sqlite_integer(from)?));
    }
    if let Some(to) = query.created_to {
        clauses.push("a.created_at<=?".into());
        args.push(SqlValue::Integer(sqlite_integer(to)?));
    }
    Ok((clauses.join(" AND "), args))
}

fn task_filters(query: &TaskCenterQuery) -> Result<(String, Vec<SqlValue>)> {
    let mut clauses = vec!["j.project_id='default'".to_string()];
    let mut args = Vec::new();
    if let Some(search) = nonempty_filter(&query.search) {
        if search.len() > 512 {
            return Err(LibraryError::Invalid("任务搜索词过长".into()));
        }
        let pattern = format!("%{}%", search.to_lowercase());
        clauses.push(
            "(lower(coalesce(t.prompt,json_extract(j.request_json,'$.instruction'),'')) LIKE ? ESCAPE '\\' OR lower(coalesce(c.title,'历史生成')) LIKE ? ESCAPE '\\' OR lower(coalesce(t.model_id,?)) LIKE ? ESCAPE '\\')"
                .into(),
        );
        args.push(SqlValue::Text(pattern.clone()));
        args.push(SqlValue::Text(pattern.clone()));
        args.push(SqlValue::Text(DEFAULT_MODEL_ID.into()));
        args.push(SqlValue::Text(pattern));
    }
    if let Some(conversation_id) = nonempty_filter(&query.conversation_id) {
        clauses.push("t.conversation_id=?".into());
        args.push(SqlValue::Text(conversation_id.into()));
    }
    if let Some(model_id) = nonempty_filter(&query.model_id) {
        clauses.push("coalesce(t.model_id,?) = ?".into());
        args.push(SqlValue::Text(DEFAULT_MODEL_ID.into()));
        args.push(SqlValue::Text(model_id.into()));
    }
    Ok((clauses.join(" AND "), args))
}

fn task_status_sql(status: &str) -> Result<String> {
    match status {
        "all" => Ok(String::new()),
        "active" => Ok(
            "a.state IN ('submitted','preparing','running','committing','cancel_requested')".into(),
        ),
        "succeeded" => Ok("a.state='succeeded'".into()),
        "failed" => Ok("a.state='failed'".into()),
        "cancelled" => Ok("a.state='cancelled'".into()),
        "interrupted" => Ok("a.state='interrupted'".into()),
        _ => Err(LibraryError::Invalid("未知任务状态筛选".into())),
    }
}

fn push_asset_cursor(cursor: &Option<StableCursor>, args: &mut Vec<SqlValue>) -> String {
    let Some(cursor) = cursor else {
        return String::new();
    };
    args.push(SqlValue::Integer(
        cursor.created_at.min(i64::MAX as u64) as i64
    ));
    args.push(SqlValue::Integer(
        cursor.created_at.min(i64::MAX as u64) as i64
    ));
    args.push(SqlValue::Text(cursor.id.clone()));
    " AND (a.created_at<? OR (a.created_at=? AND a.id<?))".into()
}

fn push_job_cursor(cursor: &Option<StableCursor>, args: &mut Vec<SqlValue>) -> String {
    let Some(cursor) = cursor else {
        return String::new();
    };
    args.push(SqlValue::Integer(
        cursor.created_at.min(i64::MAX as u64) as i64
    ));
    args.push(SqlValue::Integer(
        cursor.created_at.min(i64::MAX as u64) as i64
    ));
    args.push(SqlValue::Text(cursor.id.clone()));
    " AND (j.created_at<? OR (j.created_at=? AND j.id<?))".into()
}

fn count_query(db: &Connection, sql: &str, args: Vec<SqlValue>) -> Result<u64> {
    Ok(db.query_row(sql, params_from_iter(args), |row| read_unsigned(row, 0))?)
}

fn query_strings(db: &Connection, sql: &str, args: Vec<SqlValue>) -> Result<Vec<String>> {
    let mut statement = db.prepare(sql)?;
    statement
        .query_map(params_from_iter(args), |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn nonempty_filter(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn validate_time_range(from: Option<u64>, to: Option<u64>) -> Result<()> {
    if from.zip(to).is_some_and(|(from, to)| from > to) {
        return Err(LibraryError::Invalid("作品日期范围无效".into()));
    }
    Ok(())
}

fn classify_error(state: &str, error: Option<&str>) -> String {
    let value = format!("{} {}", state, error.unwrap_or_default()).to_ascii_lowercase();
    if value.contains("cancel") {
        "cancel".into()
    } else if value.contains("interrupt") {
        "interrupt".into()
    } else if value.contains("storage") || value.contains("disk") || value.contains("空间") {
        "storage".into()
    } else if value.contains("memory") || value.contains("内存") {
        "memory".into()
    } else if value.contains("validation") || value.contains("invalid") || value.contains("无效")
    {
        "validation".into()
    } else if value.contains("runtime") || value.contains("worker") || value.contains("运行") {
        "runtime".into()
    } else {
        "other".into()
    }
}

fn next_action(state: &str, has_conversation: bool) -> String {
    match state {
        "Failed" => "检查错误与制作记录；运行环境一致时可显式重试，否则从草稿创建新任务。".into(),
        "Cancelled" => "需要时可显式重试；原取消记录不会改写。".into(),
        "Interrupted" => "先确认旧执行已清理，再显式重试。".into(),
        "Succeeded" if has_conversation => "预览或导出结果，也可回到所属会话继续创作。".into(),
        "Succeeded" => "预览或导出结果；该记录没有可靠的来源会话归属。".into(),
        "Submitted" | "Preparing" | "Running" | "Committing" => {
            "等待当前阶段完成；需要停止时只取消当前 Attempt。".into()
        }
        "CancelRequested" => "等待计算进程回收，完成后状态会变为已取消。".into(),
        _ => "刷新任务状态或返回所属会话查看。".into(),
    }
}
