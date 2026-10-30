use super::assets::CommitStage;
use super::*;
use sha2::{Digest, Sha256};
use std::process::Command;
use studio_core::generation::{GenerationInput, PlanEnvironment, freeze_z_image};
use tempfile::TempDir;

fn open(root: &Path) -> Library {
    Library::open(root, vec![]).unwrap()
}
fn generation(key: &str) -> studio_core::generation::FrozenGeneration {
    generation_with_prompt(key, "一只中文测试小猫")
}

fn generation_with_prompt(key: &str, prompt: &str) -> studio_core::generation::FrozenGeneration {
    freeze_z_image(
        GenerationInput {
            request_id: RequestId::new(format!("request:{key}")).unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new(format!("submit:{key}")).unwrap(),
            plan_id: PlanId::new(format!("plan:{key}")).unwrap(),
            prompt: prompt.into(),
            width: 1024,
            height: 768,
            count: 1,
            seed: None,
        },
        73,
        PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "test-fixture".into(),
                    revision: "immutable".into(),
                },
                content_hash: ContentHash::new("a".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "fixture-mflux".into(),
                engine: "mlx".into(),
                source: SourceRevision {
                    source: "test-fixture".into(),
                    revision: "immutable".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "fixture".into(),
                precision: "bfloat16".into(),
            },
            scheduler: "fixture-scheduler".into(),
        },
    )
    .unwrap()
}

fn generation_for_turn(key: &str, turn: &TurnRecord) -> studio_core::generation::FrozenGeneration {
    let (width, height) = match turn.aspect.as_str() {
        "1:1" => (1024, 1024),
        "4:3" => (1024, 768),
        "3:4" => (768, 1024),
        _ => panic!("unsupported test aspect"),
    };
    freeze_z_image(
        GenerationInput {
            request_id: RequestId::new(format!("request:{key}")).unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new(turn.submission_key.clone()).unwrap(),
            plan_id: PlanId::new(format!("plan:{key}")).unwrap(),
            prompt: turn.prompt.clone(),
            width,
            height,
            count: turn.count,
            seed: turn.seed.parse::<u32>().ok(),
        },
        73,
        PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "test-fixture".into(),
                    revision: "immutable".into(),
                },
                content_hash: ContentHash::new("a".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "fixture-mflux".into(),
                engine: "mlx".into(),
                source: SourceRevision {
                    source: "test-fixture".into(),
                    revision: "immutable".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "fixture".into(),
                precision: "bfloat16".into(),
            },
            scheduler: "fixture-scheduler".into(),
        },
    )
    .unwrap()
}
fn submit(library: &mut Library, key: &str) -> JobRecord {
    let f = generation(key);
    library.submit(f.request, f.plan).unwrap().job
}
fn png_bytes() -> Vec<u8> {
    png_bytes_for(1024, 768)
}
fn png_bytes_for(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![];
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![128; width as usize * height as usize * 3])
            .unwrap();
    }
    bytes
}
fn candidate(library: &Library, attempt: &Attempt, bytes: &[u8]) -> CandidateDescriptor {
    fs::write(
        library.attempt_run_dir(attempt).unwrap().join("result.png"),
        bytes,
    )
    .unwrap();
    descriptor(bytes)
}
fn descriptor(bytes: &[u8]) -> CandidateDescriptor {
    CandidateDescriptor {
        relative_path: "result.png".into(),
        width: 1024,
        height: 768,
        byte_count: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}
fn commit(library: &mut Library, job: &JobRecord) -> JobRecord {
    let width = job.plan.effective_output.width;
    let height = job.plan.effective_output.height;
    let bytes = png_bytes_for(width, height);
    fs::write(
        library
            .attempt_run_dir(&job.attempt)
            .unwrap()
            .join("result.png"),
        &bytes,
    )
    .unwrap();
    let candidate = CandidateDescriptor {
        relative_path: "result.png".into(),
        width,
        height,
        byte_count: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    library
        .commit_candidate_after_reap(&job.attempt, candidate)
        .unwrap()
}

fn draft_with_prompt(library: &mut Library, id: u64, prompt: &str) -> DraftRecord {
    let mut draft = library.draft(id).unwrap();
    draft.prompt = prompt.into();
    draft.params = serde_json::json!({
        "modelId": DEFAULT_MODEL_ID,
        "aspect": "1:1",
        "count": 1,
        "seed": ""
    });
    library.save_draft(draft.clone(), draft.version).unwrap()
}

fn accept_prompt(library: &mut Library, prompt: &str, key: &str) -> AcceptedTurn {
    let conversation = library
        .conversation(&library.active_conversation_id().unwrap())
        .unwrap();
    let draft = draft_with_prompt(library, conversation.draft_id, prompt);
    library
        .accept_turn(AcceptTurnInput {
            conversation_id: conversation.id,
            draft_id: draft.id,
            expected_version: draft.version,
            submission_key: IdempotencyKey::new(key).unwrap(),
            prompt: prompt.into(),
            model_id: DEFAULT_MODEL_ID.into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        })
        .unwrap()
}

#[test]
fn host_ownership_precedes_recovery_and_default_project_is_not_reset() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("中文 data root");
    let mut first = open(&root);
    let job = submit(&mut first, "colon:id");
    assert!(matches!(
        Library::open(&root, vec![]),
        Err(LibraryError::InUse)
    ));
    assert_eq!(
        first.job(&job.job_id).unwrap().state,
        AttemptState::Submitted
    );
    let path = first.attempt_run_dir(&job.attempt).unwrap();
    assert!(!path.file_name().unwrap().to_string_lossy().contains(':'));
    assert!(!root.join("diagnostics").exists());
    drop(first);
    let second = open(&root);
    assert_eq!(second.snapshot().unwrap().drafts.len(), 1);
    assert_eq!(
        second.job(&job.job_id).unwrap().state,
        AttemptState::Interrupted
    );
    assert!(
        path.exists(),
        "recovery must preserve old scratch until reaping is confirmed"
    );
}

/// A controlled child keeps the inherited open-file-description alive after exec.
/// The parent changes CLOEXEC only in pre_exec, never on its own descriptor.
#[cfg(unix)]
#[test]
fn inherited_host_lock_child() {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    let Ok(raw) = std::env::var("IMAGE_STUDIO_INHERITED_HOST_LOCK_FD") else {
        return;
    };
    let fd: i32 = raw.parse().unwrap();
    // SAFETY: The parent explicitly passed this unique inherited descriptor to
    // this exec'd test process. No other Rust File in the child owns it.
    let _inherited = unsafe { File::from_raw_fd(fd) };
    println!("INHERITED_HOST_LOCK_READY");
    std::io::stdout().flush().unwrap();
    let mut release = [0_u8; 1];
    assert_eq!(std::io::stdin().read(&mut release).unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn retired_host_unlocks_even_while_a_child_keeps_its_inherited_descriptor() {
    use std::io::{BufRead, BufReader};
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    use std::process::{Child, Stdio};

    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let root = TempDir::new().unwrap();
    let library = open(root.path());
    let fd = library._host_lock.0.as_raw_fd();
    // SAFETY: F_GETFD only reads the still-owned host descriptor's flags.
    let original_flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(original_flags >= 0 && original_flags & libc::FD_CLOEXEC != 0);
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "adapters::library::tests::inherited_host_lock_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("IMAGE_STUDIO_INHERITED_HOST_LOCK_FD", fd.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: Only async-signal-safe fcntl calls run in the fork child; clearing
    // its descriptor flag deliberately models the otherwise transient fork gap.
    unsafe {
        command.pre_exec(move || {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = ChildGuard(command.spawn().unwrap());
    let stdout = child.0.stdout.take().unwrap();
    let (ready, receiver) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        for _ in 0..16 {
            let mut line = String::new();
            if stdout.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if line.contains("INHERITED_HOST_LOCK_READY") {
                let _ = ready.send(());
                break;
            }
        }
        stdout
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("controlled child must retain the inherited host descriptor");
    // Retain the read end until the child's test harness prints its final result.
    // Closing it after the ready marker would make that print fail with SIGPIPE.
    let _stdout = reader.join().unwrap();
    // SAFETY: Recheck the parent's live descriptor. pre_exec must not alter it.
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, original_flags);
    assert!(matches!(
        Library::open(root.path(), vec![]),
        Err(LibraryError::InUse)
    ));
    drop(library);
    let reopened = Library::open(root.path(), vec![])
        .expect("a retired host must release ownership before inherited FDs close");
    assert!(child.0.try_wait().unwrap().is_none());
    assert_eq!(reopened.snapshot().unwrap().drafts.len(), 1);
    drop(child.0.stdin.take());
    assert!(child.0.wait().unwrap().success());
    assert!(matches!(
        Library::open(root.path(), vec![]),
        Err(LibraryError::InUse)
    ));
    drop(reopened);
    assert_eq!(open(root.path()).snapshot().unwrap().drafts.len(), 1);
}

#[test]
fn sqlite_configuration_unknown_schema_and_corruption_are_explicit() {
    let root = TempDir::new().unwrap();
    let library = open(root.path());
    assert_eq!(
        library
            .db
            .pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        library
            .db
            .pragma_query_value(None, "synchronous", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        library
            .db
            .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    library.db.pragma_update(None, "user_version", 91).unwrap();
    drop(library);
    assert!(matches!(
        Library::open(root.path(), vec![]),
        Err(LibraryError::Schema(_))
    ));
    let db = Connection::open(root.path().join("library.sqlite3")).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        91
    );
    drop(db);
    let corrupt = TempDir::new().unwrap();
    let bytes = b"corrupt database must not be replaced";
    fs::write(corrupt.path().join("library.sqlite3"), bytes).unwrap();
    assert!(Library::open(corrupt.path(), vec![]).is_err());
    assert_eq!(
        fs::read(corrupt.path().join("library.sqlite3")).unwrap(),
        bytes
    );
}

#[test]
fn drafts_use_cas_and_restore_content_selection_and_unsupported_values() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let original = library.snapshot().unwrap().drafts.remove(0);
    let mut draft = original.clone();
    draft.prompt = "已确认中文\n第二行".into();
    draft.params = serde_json::json!({"aspect":"16:9","count":4,"seed":"旧值"});
    draft.collapsed = true;
    let saved = library.save_draft(draft, original.version).unwrap();
    assert_eq!(saved.version, 2);
    assert!(matches!(
        library.save_draft(original, 1),
        Err(LibraryError::DraftConflict)
    ));
    let other = library.create_draft().unwrap();
    library.select_draft(saved.id).unwrap();
    drop(library);
    let restored = open(root.path()).snapshot().unwrap();
    assert_eq!(restored.active_draft_id, saved.id);
    assert_eq!(restored.drafts[0], saved);
    assert_eq!(restored.drafts[1].id, other.id);
}

#[test]
fn failed_draft_write_and_submission_transaction_do_not_claim_success() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let mut draft = library.snapshot().unwrap().drafts.remove(0);
    library.db.execute_batch("CREATE TRIGGER reject_draft BEFORE UPDATE ON drafts BEGIN SELECT RAISE(ABORT,'fixture disk failure'); END;").unwrap();
    draft.prompt = "must not be saved".into();
    assert!(library.save_draft(draft, 1).is_err());
    assert_eq!(library.drafts().unwrap()[0].version, 1);
    assert!(library.drafts().unwrap()[0].prompt.is_empty());
    library.db.execute_batch("CREATE TRIGGER reject_attempt BEFORE INSERT ON attempts BEGIN SELECT RAISE(ABORT,'fixture transaction failure'); END;").unwrap();
    let f = generation("rollback");
    assert!(library.submit(f.request, f.plan).is_err());
    assert!(library.jobs(100, 0).unwrap().is_empty());
}

#[test]
fn database_integer_boundaries_fail_without_wrapping_identifiers_or_versions() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let original = library.drafts().unwrap().remove(0);
    let mut draft = original.clone();
    draft.id = u64::MAX;
    assert!(matches!(
        library.save_draft(draft, original.version),
        Err(LibraryError::Invalid(_))
    ));
    assert!(matches!(
        library.save_draft(original.clone(), i64::MAX as u64),
        Err(LibraryError::Invalid(_))
    ));
    assert!(matches!(
        library.select_draft(u64::MAX),
        Err(LibraryError::Invalid(_))
    ));
    assert_eq!(library.drafts().unwrap()[0], original);
    library
        .db
        .execute(
            "UPDATE projects SET active_draft_id=-1 WHERE id='default'",
            [],
        )
        .unwrap();
    assert!(library.snapshot().is_err());
}

#[test]
fn duplicate_submission_busy_gate_and_retry_preserve_immutable_history() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "one");
    let f = generation("one");
    let duplicate = library.submit(f.request, f.plan).unwrap();
    assert!(!duplicate.created);
    assert_eq!(duplicate.job.job_id, job.job_id);
    let mut f = generation("one");
    f.request.instruction = "different".into();
    assert!(matches!(
        library.submit(f.request, f.plan),
        Err(LibraryError::IdempotencyConflict)
    ));
    let f = generation("two");
    assert!(matches!(
        library.submit(f.request, f.plan),
        Err(LibraryError::Busy)
    ));
    let failed = library
        .finish_after_reap(
            &job.attempt,
            Completion::Failed {
                code: "worker_crash".into(),
                detail: "fixture".into(),
            },
        )
        .unwrap();
    let retry = library.retry(&job.job_id).unwrap();
    assert_ne!(retry.attempt, job.attempt);
    assert_eq!(retry.request, job.request);
    assert_eq!(retry.plan, job.plan);
    assert!(
        library
            .mark_phase(&job.attempt, AttemptState::Running)
            .is_err()
    );
    assert!(library.request_cancel(&job.attempt).is_err());
    let attempts = library.attempts(&job.job_id).unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].manifest, failed.manifest);
    assert_eq!(attempts[0].state, AttemptState::Failed);
    let mut stale = retry.attempt.clone();
    stale.execution_token = ExecutionToken::new("wrong-token").unwrap();
    assert!(library.mark_phase(&stale, AttemptState::Running).is_err());
}

#[test]
fn submission_lookup_and_counts_are_not_limited_to_the_history_page() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    assert_eq!(library.job_counts().unwrap(), (0, 0, 0));
    assert!(
        library
            .job_for_submission(&IdempotencyKey::new("absent").unwrap())
            .unwrap()
            .is_none()
    );
    let first = submit(&mut library, "oldest");
    library
        .finish_after_reap(
            &first.attempt,
            Completion::Failed {
                code: "TEST".into(),
                detail: "failed fixture".into(),
            },
        )
        .unwrap();
    for index in 0..101 {
        let job = submit(&mut library, &format!("page-{index}"));
        library
            .finish_after_reap(
                &job.attempt,
                Completion::Failed {
                    code: "TEST".into(),
                    detail: "failed fixture".into(),
                },
            )
            .unwrap();
    }
    assert_eq!(library.jobs(100, 0).unwrap().len(), 100);
    assert_eq!(library.jobs(100, 100).unwrap().len(), 2);
    assert_eq!(
        library
            .job_for_submission(&first.request.idempotency_key)
            .unwrap()
            .unwrap()
            .job_id,
        first.job_id
    );
    assert_eq!(library.job_counts().unwrap(), (102, 0, 0));
    let retry = library.retry(&first.job_id).unwrap();
    assert_eq!(library.job_counts().unwrap(), (102, 1, 0));
    commit(&mut library, &retry);
    assert_eq!(library.job_counts().unwrap(), (102, 0, 1));
    assert_eq!(library.attempts(&first.job_id).unwrap().len(), 2);
}

#[test]
fn scratch_preparation_failure_does_not_leave_an_active_job() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    // Simulate an unavailable managed volume after SQLite has accepted a Job.
    fs::remove_dir(root.path().join("runs")).unwrap();
    fs::write(root.path().join("runs"), b"unavailable").unwrap();
    let f = generation("no-scratch");
    assert!(library.submit(f.request, f.plan).is_err());
    let failed = library.jobs(1, 0).unwrap().remove(0);
    assert_eq!(failed.state, AttemptState::Failed);
    assert!(
        matches!(failed.manifest.unwrap().final_result, FinalResult::Failed { code } if code == "STORAGE_PREPARE")
    );
    assert_eq!(library.job_counts().unwrap(), (1, 0, 0));
    // Retry creates another immutable failure instead of getting stuck too.
    assert!(library.retry(&failed.job_id).is_err());
    assert_eq!(library.job_counts().unwrap(), (1, 0, 0));
    assert_eq!(library.attempts(&failed.job_id).unwrap().len(), 2);
    fs::remove_file(root.path().join("runs")).unwrap();
    fs::create_dir(root.path().join("runs")).unwrap();
    assert_eq!(
        library.retry(&failed.job_id).unwrap().state,
        AttemptState::Submitted
    );
}

#[test]
fn accepted_cancel_wins_over_candidate_error_and_duplicate_terminal() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "cancel");
    let candidate = candidate(&library, &job.attempt, &png_bytes());
    assert!(matches!(
        library.request_cancel(&job.attempt).unwrap(),
        CancelOutcome::Accepted { .. }
    ));
    assert!(matches!(
        library.commit_candidate_after_reap(&job.attempt, candidate),
        Err(LibraryError::CancelAccepted)
    ));
    assert!(
        library
            .mark_phase(&job.attempt, AttemptState::Running)
            .is_err()
    );
    let cancelled = library
        .finish_after_reap(
            &job.attempt,
            Completion::Failed {
                code: "timeout".into(),
                detail: "late timeout diagnostic".into(),
            },
        )
        .unwrap();
    assert_eq!(cancelled.state, AttemptState::Cancelled);
    assert_eq!(
        cancelled.manifest.as_ref().unwrap().final_result,
        FinalResult::Cancelled
    );
    assert!(cancelled.error.unwrap().contains("timeout"));
    assert!(cancelled.manifest.unwrap().outputs.is_empty());
    assert!(library.assets(100, 0).unwrap().is_empty());
    assert!(
        library
            .finish_after_reap(
                &job.attempt,
                Completion::Failed {
                    code: "late".into(),
                    detail: String::new()
                }
            )
            .is_err()
    );
    assert!(matches!(
        library.request_cancel(&job.attempt).unwrap(),
        CancelOutcome::AlreadyFinished { .. }
    ));
    library.cleanup_after_reap(&job.attempt).unwrap();
    assert!(!library.attempt_run_dir(&job.attempt).unwrap().exists());
}

#[test]
fn success_before_cancel_is_immutable_and_asset_reads_do_not_need_model() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "success");
    let success = commit(&mut library, &job);
    assert_eq!(success.state, AttemptState::Succeeded);
    assert!(library.retry(&job.job_id).is_err());
    assert!(
        matches!(library.request_cancel(&job.attempt).unwrap(),CancelOutcome::AlreadyFinished{job} if job.state==AttemptState::Succeeded)
    );
    let id = success.manifest.as_ref().unwrap().outputs[0]
        .asset_id
        .clone();
    assert_eq!(library.read_asset_png(&id).unwrap(), png_bytes());
    drop(library);
    let library = open(root.path());
    assert_eq!(library.job(&job.job_id).unwrap().manifest, success.manifest);
    assert_eq!(library.assets(100, 0).unwrap().len(), 1);
}

#[test]
fn failure_first_cannot_be_changed_by_cancel() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "failed");
    let failed = library
        .finish_after_reap(
            &job.attempt,
            Completion::Failed {
                code: "out_of_memory".into(),
                detail: "identified OOM".into(),
            },
        )
        .unwrap();
    assert!(matches!(
        library.request_cancel(&job.attempt).unwrap(),
        CancelOutcome::AlreadyFinished { .. }
    ));
    assert_eq!(library.job(&job.job_id).unwrap().manifest, failed.manifest);
}

#[test]
fn untrusted_candidates_fail_before_asset_registration() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "invalid");
    let bytes = png_bytes();
    let mut c = candidate(&library, &job.attempt, &bytes);
    c.relative_path = "../result.png".into();
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, c)
            .is_err()
    );
    let mut c = descriptor(&bytes);
    c.sha256 = "0".repeat(64);
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, c)
            .is_err()
    );
    let mut giant = bytes.clone();
    giant[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    let c = candidate(&library, &job.attempt, &giant);
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, c)
            .unwrap_err()
            .to_string()
            .contains("头部尺寸")
    );
    let mut corrupt = bytes.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0x01;
    let c = candidate(&library, &job.attempt, &corrupt);
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, c)
            .is_err()
    );
    assert!(library.assets(100, 0).unwrap().is_empty());
    assert_ne!(
        library.job(&job.job_id).unwrap().state,
        AttemptState::Succeeded
    );
}

#[cfg(unix)]
#[test]
fn symlink_and_fifo_are_rejected_without_following_or_blocking() {
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, fs::symlink},
    };
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "special");
    let bytes = png_bytes();
    let outside = root.path().join("outside.png");
    fs::write(&outside, &bytes).unwrap();
    let result = library
        .attempt_run_dir(&job.attempt)
        .unwrap()
        .join("result.png");
    symlink(&outside, &result).unwrap();
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, descriptor(&bytes))
            .is_err()
    );
    fs::remove_file(&result).unwrap();
    let name = CString::new(result.as_os_str().as_bytes()).unwrap();
    // SAFETY: the NUL-terminated path is alive and permissions are a valid mode_t.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let start = std::time::Instant::now();
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, descriptor(&bytes))
            .is_err()
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn database_asset_failure_rolls_back_manifest_and_asset_together() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "asset-failure");
    library.db.execute_batch("CREATE TRIGGER reject_terminal BEFORE UPDATE OF state ON attempts WHEN NEW.state='succeeded' BEGIN SELECT RAISE(ABORT,'fixture commit failure'); END;").unwrap();
    let c = candidate(&library, &job.attempt, &png_bytes());
    assert!(
        library
            .commit_candidate_after_reap(&job.attempt, c)
            .is_err()
    );
    assert!(library.assets(100, 0).unwrap().is_empty());
    assert!(library.job(&job.job_id).unwrap().manifest.is_none());
    drop(library);
    let library = open(root.path());
    assert_eq!(
        library.job(&job.job_id).unwrap().state,
        AttemptState::Interrupted
    );
    assert_eq!(
        fs::read_dir(root.path().join("quarantine"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn committed_asset_corruption_is_availability_not_history_mutation() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "corruption");
    let success = commit(&mut library, &job);
    let asset = success.manifest.as_ref().unwrap().outputs[0].clone();
    let StorageLocation::Managed { relative_path, .. } = &asset.location else {
        panic!("managed asset")
    };
    let path = root.path().join(relative_path);
    fs::remove_file(path).unwrap();
    let record = library.asset(&asset.asset_id).unwrap();
    assert!(!record.available);
    assert!(record.availability_error.is_some());
    assert!(library.read_asset_png(&asset.asset_id).is_err());
    assert_eq!(library.job(&job.job_id).unwrap().manifest, success.manifest);
}

#[test]
fn repeated_asset_polling_invalidates_checks_after_same_length_replacement() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let job = submit(&mut library, "same-length-corruption");
    let success = commit(&mut library, &job);
    let asset = success.manifest.as_ref().unwrap().outputs[0].clone();
    assert!(library.asset(&asset.asset_id).unwrap().available);
    assert!(library.assets(100, 0).unwrap()[0].available);
    let StorageLocation::Managed { relative_path, .. } = &asset.location else {
        panic!("managed asset")
    };
    let path = root.path().join(relative_path);
    let mut bytes = fs::read(&path).unwrap();
    bytes[40] ^= 1;
    // Atomic replacement also tests inode changes even on coarse timestamps.
    let replacement = root.path().join("replacement.png");
    fs::write(&replacement, &bytes).unwrap();
    fs::rename(replacement, &path).unwrap();
    assert!(!library.assets(100, 0).unwrap()[0].available);
    assert!(library.read_asset_png(&asset.asset_id).is_err());
    assert_eq!(library.job(&job.job_id).unwrap().manifest, success.manifest);
}

#[test]
fn selected_asset_older_than_the_first_page_survives_restart() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let bytes = png_bytes();
    let mut oldest = None;
    for index in 0..101 {
        let job = submit(&mut library, &format!("image-{index}"));
        let c = candidate(&library, &job.attempt, &bytes);
        let success = library
            .commit_candidate_after_reap(&job.attempt, c)
            .unwrap();
        if oldest.is_none() {
            oldest = Some(success.manifest.unwrap().outputs[0].asset_id.clone());
        }
    }
    let oldest = oldest.unwrap();
    assert!(
        !library
            .assets(100, 0)
            .unwrap()
            .iter()
            .any(|record| record.asset.asset_id == oldest)
    );
    library.select_asset(Some(oldest.clone())).unwrap();
    drop(library);
    let library = open(root.path());
    let snapshot = library.snapshot().unwrap();
    assert_eq!(snapshot.selected_asset_id.as_ref(), Some(&oldest));
    assert_eq!(snapshot.jobs.len(), 100);
    assert_eq!(snapshot.assets.len(), 101);
    assert!(
        snapshot
            .assets
            .iter()
            .any(|record| record.asset.asset_id == oldest)
    );
    assert_eq!(library.read_asset_png(&oldest).unwrap(), bytes);
}

#[test]
fn exports_are_atomic_external_copies_and_require_overwrite_confirmation() {
    let root = TempDir::new().unwrap();
    let extra = TempDir::new().unwrap();
    let mut library = Library::open(root.path(), vec![extra.path().to_path_buf()]).unwrap();
    let job = submit(&mut library, "export");
    let success = commit(&mut library, &job);
    let id = success.manifest.as_ref().unwrap().outputs[0]
        .asset_id
        .clone();
    let destination = TempDir::new().unwrap();
    let target = destination.path().join("中文 output.png");
    library.export_asset_png(&id, &target, false).unwrap();
    assert_eq!(fs::read(&target).unwrap(), png_bytes());
    fs::write(&target, b"existing target").unwrap();
    assert!(library.export_asset_png(&id, &target, false).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"existing target");
    library.export_asset_png(&id, &target, true).unwrap();
    assert_eq!(fs::read(&target).unwrap(), png_bytes());
    assert!(
        library
            .export_asset_png(&id, &root.path().join("new.png"), false)
            .is_err()
    );
    assert!(
        library
            .export_asset_png(&id, &extra.path().join("new.png"), true)
            .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(extra.path(), destination.path().join("internal-link")).unwrap();
        assert!(
            library
                .export_asset_png(
                    &id,
                    &destination.path().join("internal-link/new.png"),
                    false
                )
                .is_err()
        );
    }
    let as_directory = destination.path().join("directory.png");
    fs::create_dir(&as_directory).unwrap();
    assert!(library.export_asset_png(&id, &as_directory, true).is_err());
    assert_eq!(library.job(&job.job_id).unwrap().manifest, success.manifest);
    assert_eq!(library.assets(100, 0).unwrap().len(), 1);
}

/// Launched by the parent test in a separate process; exit intentionally skips every Drop.
#[test]
fn commit_crash_child() {
    let Ok(root) = std::env::var("IMAGE_STUDIO_COMMIT_CRASH_ROOT") else {
        return;
    };
    let stage = std::env::var("IMAGE_STUDIO_COMMIT_CRASH_STAGE").unwrap();
    let mut library = open(Path::new(&root));
    let job = submit(&mut library, "crash");
    let c = candidate(&library, &job.attempt, &png_bytes());
    let _ = library.commit_candidate_with_hook(&job.attempt, c, |current| {
        if format!("{current:?}") == stage {
            std::process::exit(86);
        }
        Ok(())
    });
    panic!("crash point was not reached");
}

#[test]
fn real_process_exit_at_all_commit_boundaries_recovers_exactly_once() {
    for stage in [
        CommitStage::Copied,
        CommitStage::Synced,
        CommitStage::Renamed,
        CommitStage::BeforeDatabaseCommit,
        CommitStage::AfterDatabaseCommit,
    ] {
        let root = TempDir::new().unwrap();
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "adapters::library::tests::commit_crash_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("IMAGE_STUDIO_COMMIT_CRASH_ROOT", root.path())
            .env("IMAGE_STUDIO_COMMIT_CRASH_STAGE", format!("{stage:?}"))
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(86),
            "{stage:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let library = open(root.path());
        let jobs = library.jobs(100, 0).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            fs::read_dir(root.path().join("runs")).unwrap().count(),
            1,
            "old scratch preserved"
        );
        if stage == CommitStage::AfterDatabaseCommit {
            assert_eq!(jobs[0].state, AttemptState::Succeeded);
            assert_eq!(library.assets(100, 0).unwrap().len(), 1);
            assert!(library.assets(100, 0).unwrap()[0].available);
        } else {
            assert_eq!(jobs[0].state, AttemptState::Interrupted);
            assert!(library.assets(100, 0).unwrap().is_empty());
            assert!(jobs[0].manifest.as_ref().unwrap().outputs.is_empty());
            assert_eq!(
                fs::read_dir(root.path().join("quarantine"))
                    .unwrap()
                    .count(),
                1
            );
        }
    }
}

#[test]
fn installation_records_are_separate_from_generation_evidence() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    library
        .save_installation("z-image-turbo", &serde_json::json!({"state":"installing"}))
        .unwrap();
    assert_eq!(
        library.installation("z-image-turbo").unwrap().unwrap()["state"],
        "installing"
    );
    assert!(library.jobs(100, 0).unwrap().is_empty());
    assert!(library.assets(100, 0).unwrap().is_empty());
}

#[test]
fn fresh_library_has_one_conversation_and_receive_is_fast_and_idempotent() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let snapshot = library.snapshot().unwrap();
    assert_eq!(snapshot.conversations.len(), 1);
    assert_eq!(snapshot.turns.turns.len(), 0);
    assert_eq!(snapshot.drafts[0].params["modelId"], DEFAULT_MODEL_ID);
    let accepted = accept_prompt(&mut library, "清晨的山湖\n第二行", "conversation:one");
    assert!(accepted.created);
    assert_eq!(accepted.turn.status, TurnStatus::Pending);
    assert_eq!(accepted.turn.prompt, "清晨的山湖\n第二行");
    assert_eq!(accepted.draft.prompt, "");
    assert_eq!(accepted.draft.version, 3);
    assert_eq!(
        library.conversations(10, None).unwrap().conversations[0].title,
        "清晨的山湖"
    );
    assert_eq!(library.jobs(100, 0).unwrap().len(), 0);

    let duplicate = library
        .accept_turn(AcceptTurnInput {
            conversation_id: accepted.turn.conversation_id.clone(),
            draft_id: accepted.draft.id,
            expected_version: accepted.draft.version,
            submission_key: IdempotencyKey::new("conversation:one").unwrap(),
            prompt: "清晨的山湖\n第二行".into(),
            model_id: DEFAULT_MODEL_ID.into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        })
        .unwrap();
    assert!(!duplicate.created);
    assert_eq!(duplicate.turn.id, accepted.turn.id);
    assert!(matches!(
        library.accept_turn(AcceptTurnInput {
            conversation_id: accepted.turn.conversation_id.clone(),
            draft_id: accepted.draft.id,
            expected_version: accepted.draft.version,
            submission_key: IdempotencyKey::new("conversation:one").unwrap(),
            prompt: "different".into(),
            model_id: DEFAULT_MODEL_ID.into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        }),
        Err(LibraryError::IdempotencyConflict)
    ));
}

#[test]
fn pending_turn_cancel_recovery_and_job_binding_are_explicit() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let accepted = accept_prompt(&mut library, "取消中的发送", "cancel-pending");
    let requested = library
        .request_cancel_unbound_turn(&accepted.turn.id)
        .unwrap();
    assert_eq!(requested.status, TurnStatus::CancelRequested);
    drop(library);
    let library = open(root.path());
    assert_eq!(
        library.turn(&accepted.turn.id).unwrap().status,
        TurnStatus::Cancelled
    );
    drop(library);

    let mut library = open(root.path());
    let accepted = accept_prompt(&mut library, "正常绑定", "bind-turn");
    let frozen = freeze_z_image(
        GenerationInput {
            request_id: RequestId::new(format!("request:{}", accepted.turn.id)).unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new(accepted.turn.submission_key.clone()).unwrap(),
            plan_id: PlanId::new(format!("plan:{}", accepted.turn.id)).unwrap(),
            prompt: accepted.turn.prompt.clone(),
            width: 1024,
            height: 1024,
            count: 1,
            seed: None,
        },
        41,
        PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "test".into(),
                    revision: "immutable".into(),
                },
                content_hash: ContentHash::new("b".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "fixture".into(),
                engine: "mlx".into(),
                source: SourceRevision {
                    source: "test".into(),
                    revision: "immutable".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "fixture".into(),
                precision: "bfloat16".into(),
            },
            scheduler: "fixture".into(),
        },
    )
    .unwrap();
    let binding = library
        .bind_turn_job(&accepted.turn.id, frozen.request, frozen.plan)
        .unwrap();
    assert_eq!(
        binding.job.conversation_id.as_ref(),
        Some(&accepted.turn.conversation_id)
    );
    assert_eq!(binding.job.turn_id.as_ref(), Some(&accepted.turn.id));
    assert_eq!(library.job_counts().unwrap(), (1, 1, 0));
    library
        .update_turn_progress(&binding.job.job_id, "sampling", Some(4), Some(9))
        .unwrap();
    let turn = library.turn(&accepted.turn.id).unwrap();
    assert_eq!(turn.progress_stage.as_deref(), Some("sampling"));
    assert_eq!(turn.progress_step, Some(4));
}

#[test]
fn pending_turn_without_cancel_becomes_interrupted_after_restart() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let accepted = accept_prompt(&mut library, "预检中断", "pending-interrupt");
    drop(library);
    let library = open(root.path());
    assert_eq!(
        library.turn(&accepted.turn.id).unwrap().status,
        TurnStatus::Interrupted
    );
    assert_eq!(library.jobs(100, 0).unwrap().len(), 0);
}

#[test]
fn conversation_history_is_paginated_and_conversations_are_independent() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let original = library.snapshot().unwrap().conversations[0].clone();
    for index in 0..31 {
        let accepted = accept_prompt(
            &mut library,
            &format!("轮次 {index}"),
            &format!("history:{index}"),
        );
        library
            .reject_turn(&accepted.turn.id, "fixture release")
            .unwrap();
    }
    let page = library.turns(&original.id, 30, None).unwrap();
    assert_eq!(page.turns.len(), 30);
    assert!(page.has_more);
    let older = library.turns(&original.id, 30, page.next_cursor).unwrap();
    assert_eq!(older.turns.len(), 1);
    assert!(!older.has_more);

    let second = library.create_conversation().unwrap();
    assert_ne!(second.conversation.id, original.id);
    assert_eq!(
        library.snapshot().unwrap().active_conversation_id,
        second.conversation.id
    );
    let accepted = accept_prompt(&mut library, "第二个会话", "second-conversation");
    assert_eq!(accepted.turn.conversation_id, second.conversation.id);
    assert_eq!(
        library.turns(&original.id, 30, None).unwrap().turns.len(),
        30
    );
    assert_eq!(
        library
            .turns(&second.conversation.id, 30, None)
            .unwrap()
            .turns
            .len(),
        1
    );
}

#[test]
fn conversation_list_paginates_without_repeating_activity_cursor_items() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    for _ in 0..101 {
        library.create_conversation().unwrap();
    }
    let first = library.conversations(100, None).unwrap();
    assert_eq!(first.conversations.len(), 100);
    assert!(first.has_more);
    let second = library.conversations(100, first.next_cursor).unwrap();
    assert_eq!(second.conversations.len(), 2);
    assert!(!second.has_more);
    let known = first
        .conversations
        .iter()
        .map(|conversation| conversation.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        second
            .conversations
            .iter()
            .all(|conversation| !known.contains(&conversation.id))
    );
}

#[test]
fn manual_conversation_title_is_stable_and_draft_version_conflicts_do_not_consume() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let created = library.create_conversation().unwrap();
    let renamed = library
        .rename_conversation(&created.conversation.id, "手动命名")
        .unwrap();
    assert_eq!(renamed.title, "手动命名");
    let draft = draft_with_prompt(&mut library, created.draft.id, "后续提示词");
    let stale_version = draft.version.saturating_sub(1);
    assert!(matches!(
        library.accept_turn(AcceptTurnInput {
            conversation_id: created.conversation.id.clone(),
            draft_id: draft.id,
            expected_version: stale_version,
            submission_key: IdempotencyKey::new("stale-source").unwrap(),
            prompt: draft.prompt.clone(),
            model_id: DEFAULT_MODEL_ID.into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        }),
        Err(LibraryError::TurnConflict)
    ));
    assert_eq!(library.draft(draft.id).unwrap().prompt, "后续提示词");
    let accepted = library
        .accept_turn(AcceptTurnInput {
            conversation_id: created.conversation.id.clone(),
            draft_id: draft.id,
            expected_version: draft.version,
            submission_key: IdempotencyKey::new("manual-title").unwrap(),
            prompt: draft.prompt.clone(),
            model_id: DEFAULT_MODEL_ID.into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        })
        .unwrap();
    assert_eq!(
        library
            .conversation(&created.conversation.id)
            .unwrap()
            .title,
        "手动命名"
    );
    assert_eq!(accepted.draft.name, "手动命名");
    assert_eq!(library.jobs(100, 0).unwrap().len(), 0);
}

#[test]
fn legacy_v1_migration_preserves_drafts_jobs_and_is_idempotent() {
    let root = TempDir::new().unwrap();
    let f = generation("legacy");
    {
        let db = Connection::open(root.path().join("library.sqlite3")).unwrap();
        db.pragma_update(None, "foreign_keys", "ON").unwrap();
        db.execute_batch(
            "
CREATE TABLE projects(id TEXT PRIMARY KEY, active_draft_id INTEGER, selected_asset_id TEXT);
CREATE TABLE drafts(id INTEGER PRIMARY KEY AUTOINCREMENT, project_id TEXT NOT NULL REFERENCES projects(id), name TEXT NOT NULL, prompt TEXT NOT NULL, params_json TEXT NOT NULL, collapsed INTEGER NOT NULL, version INTEGER NOT NULL CHECK(version > 0));
CREATE TABLE jobs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), submission_key TEXT NOT NULL, request_json TEXT NOT NULL, current_attempt_id TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(project_id, submission_key));
CREATE TABLE attempts(id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id), token TEXT NOT NULL UNIQUE, storage_key TEXT NOT NULL UNIQUE, state TEXT NOT NULL, plan_json TEXT NOT NULL, manifest_json TEXT, error TEXT, created_at INTEGER NOT NULL);
CREATE TABLE assets(id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id), attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id), asset_json TEXT NOT NULL, relative_path TEXT NOT NULL UNIQUE, width INTEGER NOT NULL, height INTEGER NOT NULL, byte_count INTEGER NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE installations(installation_id TEXT PRIMARY KEY, record_json TEXT NOT NULL);
PRAGMA user_version=1;
",
        )
        .unwrap();
        db.execute(
            "INSERT INTO projects(id,active_draft_id) VALUES('default',1)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO drafts(project_id,name,prompt,params_json,collapsed,version) VALUES('default','草稿 1','first','{\"aspect\":\"1:1\",\"count\":1,\"seed\":\"\"}',0,1)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO drafts(project_id,name,prompt,params_json,collapsed,version) VALUES('default','自定义草稿','second','{\"aspect\":\"1:1\",\"count\":1,\"seed\":\"\"}',0,1)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO jobs(id,project_id,submission_key,request_json,current_attempt_id,created_at) VALUES('job-legacy','default',?,?,?,1)",
            params![
                f.request.idempotency_key.as_str(),
                f.request.to_json().unwrap(),
                "attempt-legacy"
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO attempts(id,job_id,token,storage_key,state,plan_json,created_at) VALUES('attempt-legacy','job-legacy','token-legacy','run-legacy','failed',?,1)",
            [f.plan.to_json().unwrap()],
        )
        .unwrap();
    }
    let library = open(root.path());
    let snapshot = library.snapshot().unwrap();
    assert_eq!(
        library
            .db
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(snapshot.drafts.len(), 3);
    assert_eq!(snapshot.drafts[0].params["modelId"], DEFAULT_MODEL_ID);
    assert_eq!(snapshot.drafts[2].name, "历史生成");
    assert_eq!(snapshot.conversations.len(), 3);
    let history = snapshot
        .conversations
        .iter()
        .find(|conversation| conversation.title == "历史生成")
        .unwrap();
    let turns = library.turns(&history.id, 30, None).unwrap();
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(
        turns.turns[0].job.as_ref().unwrap().job_id.as_str(),
        "job-legacy"
    );
    drop(library);
    let reopened = open(root.path());
    assert_eq!(
        reopened
            .conversations(100, None)
            .unwrap()
            .conversations
            .len(),
        3
    );
    assert_eq!(reopened.drafts().unwrap().len(), 3);
}

#[test]
fn feature_work_and_task_queries_are_stable_read_only_projections() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    fs::create_dir_all(root.path().join("examples")).unwrap();
    fs::write(root.path().join("examples/not-a-work.png"), png_bytes()).unwrap();

    let first_turn = accept_prompt(&mut library, "第一张正式作品", "work-query-1");
    let first_frozen = generation_for_turn("work-query-1", &first_turn.turn);
    let first_job = library
        .bind_turn_job(&first_turn.turn.id, first_frozen.request, first_frozen.plan)
        .unwrap()
        .job;
    let first_asset_job = commit(&mut library, &first_job);
    let first_asset_id = first_asset_job
        .manifest
        .as_ref()
        .unwrap()
        .outputs
        .first()
        .unwrap()
        .asset_id
        .clone();

    let second_turn = accept_prompt(&mut library, "第二张正式作品", "work-query-2");
    let second_frozen = generation_for_turn("work-query-2", &second_turn.turn);
    let second_job = library
        .bind_turn_job(
            &second_turn.turn.id,
            second_frozen.request,
            second_frozen.plan,
        )
        .unwrap()
        .job;
    let second_asset_job = commit(&mut library, &second_job);
    let second_asset_id = second_asset_job
        .manifest
        .as_ref()
        .unwrap()
        .outputs
        .first()
        .unwrap()
        .asset_id
        .clone();

    let first_page = library
        .asset_library_query(AssetLibraryQuery {
            limit: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(first_page.total, 2);
    assert_eq!(first_page.items.len(), 1);
    assert!(first_page.facets.conversations.len() == 1);
    let second_page = library
        .asset_library_query(AssetLibraryQuery {
            limit: Some(1),
            cursor: first_page.next_cursor.clone(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(second_page.total, 2);
    assert_ne!(first_page.items[0].id, second_page.items[0].id);
    assert!(
        [first_asset_id.to_string(), second_asset_id.to_string()].contains(&first_page.items[0].id)
    );

    let conversation_id = first_turn.turn.conversation_id.to_string();
    let filtered = library
        .asset_library_query(AssetLibraryQuery {
            conversation_id: Some(conversation_id.clone()),
            width: Some(1024),
            height: Some(1024),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filtered.total, 2);
    assert!(
        filtered
            .items
            .iter()
            .all(|item| item.conversation_id.as_deref() == Some(&conversation_id))
    );

    let task_page = library
        .task_center_query(TaskCenterQuery {
            status: Some("succeeded".into()),
            search: Some("正式作品".into()),
            conversation_id: Some(conversation_id),
            limit: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(task_page.counts.total, 2);
    assert_eq!(task_page.counts.succeeded, 2);
    assert_eq!(task_page.items.len(), 1);
    assert!(task_page.has_more);
    let task_detail = library
        .task_center_detail(&JobId::new(task_page.items[0].id.clone()).unwrap())
        .unwrap();
    assert!(task_detail.advanced.manifest.is_some());
    assert!(!task_detail.attempts.is_empty());

    let record = library.asset(&first_asset_id).unwrap();
    let StorageLocation::Managed { relative_path, .. } = record.asset.location.clone() else {
        panic!("test asset must be managed");
    };
    fs::remove_file(root.path().join(relative_path)).unwrap();
    let unavailable = library.asset_library_item(&first_asset_id).unwrap();
    assert!(!unavailable.available);
    assert!(unavailable.availability_error.is_some());
}

#[test]
fn task_counts_and_search_cover_records_beyond_the_visible_page() {
    let root = TempDir::new().unwrap();
    let mut library = open(root.path());
    let prompts = ["最旧任务", "中间任务", "最新任务"];
    for (index, prompt) in prompts.into_iter().enumerate() {
        library.create_conversation().unwrap();
        let key = format!("task-page-{index}");
        let turn = accept_prompt(&mut library, prompt, &key);
        let frozen = generation_for_turn(&key, &turn.turn);
        let job = library
            .bind_turn_job(&turn.turn.id, frozen.request, frozen.plan)
            .unwrap()
            .job;
        library
            .finish_after_reap(
                &job.attempt,
                Completion::Failed {
                    code: "TEST_FAILURE".into(),
                    detail: "用于分页验证".into(),
                },
            )
            .unwrap();
    }
    let page = library
        .task_center_query(TaskCenterQuery {
            status: Some("failed".into()),
            search: Some("最旧任务".into()),
            limit: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].prompt, "最旧任务");
    assert_eq!(page.counts.total, 1);
    assert_eq!(page.counts.failed, 1);
    assert!(!page.has_more);
    assert!(page.items[0].completed_at.is_some());
}

#[test]
fn schema_v3_installs_stable_query_indexes() {
    let root = TempDir::new().unwrap();
    let library = open(root.path());
    let names = {
        let mut statement = library
            .db
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name IN ('jobs_project_created','assets_created')")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<std::collections::BTreeSet<_>, _>>()
            .unwrap()
    };
    assert!(names.contains("jobs_project_created"));
    assert!(names.contains("assets_created"));
    let plan = {
        let mut statement = library
            .db
            .prepare("EXPLAIN QUERY PLAN SELECT a.id FROM assets a WHERE a.created_at<? ORDER BY a.created_at DESC,a.id DESC")
            .unwrap();
        statement
            .query_map([i64::MAX], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    };
    assert!(plan.iter().any(|line| line.contains("assets_created")));
}

#[test]
fn failed_v1_migration_rolls_back_without_replacing_the_database() {
    let root = TempDir::new().unwrap();
    {
        let db = Connection::open(root.path().join("library.sqlite3")).unwrap();
        db.execute_batch(
            "
CREATE TABLE projects(id TEXT PRIMARY KEY, active_draft_id INTEGER, selected_asset_id TEXT);
CREATE TABLE drafts(id INTEGER PRIMARY KEY AUTOINCREMENT, project_id TEXT NOT NULL REFERENCES projects(id), name TEXT NOT NULL, prompt TEXT NOT NULL, params_json TEXT NOT NULL, collapsed INTEGER NOT NULL, version INTEGER NOT NULL CHECK(version > 0));
CREATE TABLE jobs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), submission_key TEXT NOT NULL, request_json TEXT NOT NULL, current_attempt_id TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(project_id, submission_key));
CREATE TABLE attempts(id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id), token TEXT NOT NULL UNIQUE, storage_key TEXT NOT NULL UNIQUE, state TEXT NOT NULL, plan_json TEXT NOT NULL, manifest_json TEXT, error TEXT, created_at INTEGER NOT NULL);
CREATE TABLE assets(id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id), attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id), asset_json TEXT NOT NULL, relative_path TEXT NOT NULL UNIQUE, width INTEGER NOT NULL, height INTEGER NOT NULL, byte_count INTEGER NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE installations(installation_id TEXT PRIMARY KEY, record_json TEXT NOT NULL);
PRAGMA user_version=1;
",
        )
        .unwrap();
        db.execute(
            "INSERT INTO projects(id,active_draft_id) VALUES('default',1)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO drafts(project_id,name,prompt,params_json,collapsed,version) VALUES('default','草稿 1','keep','not-json',0,1)",
            [],
        )
        .unwrap();
    }
    assert!(Library::open(root.path(), vec![]).is_err());
    let db = Connection::open(root.path().join("library.sqlite3")).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT prompt FROM drafts WHERE id=1", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "keep"
    );
}
