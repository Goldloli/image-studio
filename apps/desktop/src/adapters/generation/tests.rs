//! Coordinator process fixtures, deliberately not model execution evidence.
use super::*;
use std::{fs, os::unix::fs::PermissionsExt};
use studio_core::{
    contracts::*,
    domain::*,
    generation::{GenerationInput, PlanEnvironment, freeze_z_image},
};

struct Fixture {
    _root: tempfile::TempDir,
    library: Arc<Mutex<Library>>,
    controller: GenerationController,
    ready: ReadyPaths,
    job: JobRecord,
}
impl Fixture {
    fn new(mode: Option<&str>) -> Self {
        let root = tempfile::Builder::new()
            .prefix("执行 协调 ")
            .tempdir()
            .unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        let mut library =
            Library::open(&paths.data, vec![paths.cache.clone(), paths.logs.clone()]).unwrap();
        let frozen = freeze_z_image(
            GenerationInput {
                request_id: RequestId::new("request-fixture").unwrap(),
                project_id: ProjectId::new("default").unwrap(),
                idempotency_key: IdempotencyKey::new("intent-fixture").unwrap(),
                plan_id: PlanId::new("plan-fixture").unwrap(),
                prompt: "协调器故障材料，不是真实模型生成".into(),
                width: 1024,
                height: 768,
                count: 1,
                seed: Some(7),
            },
            7,
            PlanEnvironment {
                components: vec![ComponentSnapshot {
                    role: "transformer".into(),
                    source: SourceRevision {
                        source: "fixture".into(),
                        revision: "fixture".into(),
                    },
                    content_hash: ContentHash::new("a".repeat(64)).unwrap(),
                }],
                runtime: RuntimeSnapshot {
                    package_id: "fixture".into(),
                    engine: "fixture".into(),
                    source: SourceRevision {
                        source: "fixture".into(),
                        revision: "fixture".into(),
                    },
                },
                device: DeviceSnapshot {
                    device_id: "fixture".into(),
                    precision: "fixture".into(),
                },
                scheduler: "fixture".into(),
            },
        )
        .unwrap();
        let job = library.submit(frozen.request, frozen.plan).unwrap().job;
        let supervisor = root.path().join("fixture-supervisor");
        if let Some(mode) = mode {
            fs::write(
                &supervisor,
                format!("#!/usr/bin/env python3\nMODE={mode:?}\n{SCRIPT}"),
            )
            .unwrap();
            fs::set_permissions(&supervisor, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let library = Arc::new(Mutex::new(library));
        let controller = GenerationController::new(paths, library.clone(), supervisor);
        let ready = ReadyPaths {
            worker_program: PathBuf::from("/fixture/private/python"),
            worker_args: vec!["-I".into(), "-m".into(), "studio_mflux_worker".into()],
            model_path: PathBuf::from("/fixture/verified-model"),
            worker_module: "studio_mflux_worker".into(),
            worker_build_id: "fixture-worker".into(),
            runtime_lock_id: "fixture-runtime".into(),
            model_snapshot_id: "fixture-model".into(),
        };
        Self {
            _root: root,
            library,
            controller,
            ready,
            job,
        }
    }
    fn start(&self) {
        self.controller
            .start(self.job.clone(), self.ready.clone())
            .unwrap();
    }
    fn record(&self) -> JobRecord {
        self.library.lock().unwrap().job(&self.job.job_id).unwrap()
    }
    fn wait(&self) {
        wait_for(|| !self.controller.is_busy());
    }
}
fn wait_for(check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !check() {
        assert!(
            Instant::now() < deadline,
            "coordinator did not reach its bounded fixture state"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct TestGroupCleanup(i32);
impl Drop for TestGroupCleanup {
    fn drop(&mut self) {
        if self.0 > 1 {
            // SAFETY: this group was created by the current fixture, never restored from disk.
            unsafe {
                libc::kill(-self.0, libc::SIGKILL);
            }
        }
    }
}

#[test]
fn missing_supervisor_finishes_persisted_attempt_without_leaving_busy() {
    let f = Fixture::new(None);
    f.start();
    f.wait();
    let record = f.record();
    assert_eq!(record.state, AttemptState::Failed);
    assert!(record.manifest.is_some());
    assert_eq!(f.controller.snapshot().stage, "failed");
    assert!(f.library.lock().unwrap().retry(&f.job.job_id).is_ok());
}

#[test]
fn accepted_cancel_before_preparation_remains_cancelled() {
    let f = Fixture::new(None);
    f.controller.cancel(&f.job.job_id).unwrap();
    f.start();
    f.wait();
    assert_eq!(f.record().state, AttemptState::Cancelled);
    assert_eq!(f.controller.snapshot().stage, "cancelled");
}

#[test]
fn existing_lease_blocks_launch_and_does_not_claim_resource_release() {
    let f = Fixture::new(None);
    let path = f.library.lock().unwrap().data_root().join("execution.lock");
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    FileExt::lock_exclusive(&lock).unwrap();
    assert!(f.controller.start(f.job.clone(), f.ready.clone()).is_err());
    assert!(f.controller.snapshot().busy);
    assert_eq!(f.record().state, AttemptState::Failed);
    drop(lock);
    assert!(!f.controller.is_busy());
}

#[test]
fn candidate_waits_for_process_exit_before_durable_success() {
    let f = Fixture::new(Some("candidate"));
    let path = f
        .library
        .lock()
        .unwrap()
        .attempt_run_dir(&f.job.attempt)
        .unwrap()
        .join("candidate-sent");
    f.start();
    wait_for(|| path.exists());
    assert_ne!(f.record().state, AttemptState::Succeeded);
    assert!(f.controller.is_busy());
    f.wait();
    let record = f.record();
    assert_eq!(record.state, AttemptState::Succeeded);
    let manifest = record.manifest.unwrap();
    assert_eq!(manifest.outputs.len(), 1);
    assert!(
        !f.library
            .lock()
            .unwrap()
            .read_asset_png(&manifest.outputs[0].asset_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        f.controller.cancel(&f.job.job_id).unwrap().state,
        AttemptState::Succeeded
    );
}

#[test]
fn cleanup_incomplete_is_nonterminal_and_later_cancel_can_finish() {
    let f = Fixture::new(Some("cleanup"));
    f.start();
    wait_for(|| f.controller.snapshot().cleanup_error.is_some());
    assert!(f.controller.is_busy());
    assert!(!f.record().state.is_terminal());
    f.controller.cancel(&f.job.job_id).unwrap();
    f.wait();
    assert_eq!(f.record().state, AttemptState::Cancelled);
}

#[test]
fn framing_failure_closes_control_before_reaping_and_records_failure() {
    let f = Fixture::new(Some("malformed"));
    f.start();
    f.wait();
    assert_eq!(f.record().state, AttemptState::Failed);
    assert!(f.record().manifest.unwrap().outputs.is_empty());
}

#[test]
fn current_supervisor_sigkill_reclaims_a_gil_holding_compute_group_and_honors_cancel() {
    for cancel_first in [false, true] {
        let f = Fixture::new(Some("supervisor-crash"));
        let run = f
            .library
            .lock()
            .unwrap()
            .attempt_run_dir(&f.job.attempt)
            .unwrap();
        let lifecycle = run.join(inference_supervisor::LIFECYCLE_RECORD_NAME);
        f.start();
        wait_for(|| lifecycle.exists());
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(&lifecycle).unwrap()).unwrap();
        let pgid = record["compute_pgid"].as_u64().unwrap() as i32;
        let mut cleanup = TestGroupCleanup(pgid);
        assert!(lease_busy(
            &run.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("execution.lock")
        ));
        if cancel_first {
            f.controller.cancel(&f.job.job_id).unwrap();
        }
        // PID belongs to this still-running fixture invocation, never restart recovery metadata.
        assert_eq!(
            unsafe {
                libc::kill(
                    record["supervisor_pid"].as_u64().unwrap() as i32,
                    libc::SIGKILL,
                )
            },
            0
        );
        f.wait();
        assert_eq!(
            f.record().state,
            if cancel_first {
                AttemptState::Cancelled
            } else {
                AttemptState::Failed
            }
        );
        assert!(f.record().manifest.unwrap().outputs.is_empty());
        assert!(!f.controller.is_busy());
        wait_for(|| unsafe { libc::kill(-pgid, 0) != 0 });
        cleanup.0 = 0;
        drop(cleanup);
    }
}

#[test]
#[ignore = "explicit acceptance: build inference-supervisor and set STUDIO_TEST_NATIVE_SUPERVISOR"]
fn actual_native_supervisor_sigkill_is_reclaimed_by_its_current_host() {
    let native = PathBuf::from(
        std::env::var_os("STUDIO_TEST_NATIVE_SUPERVISOR").expect("native supervisor path required"),
    );
    assert!(native.is_absolute() && native.is_file());
    for cancel_first in [false, true] {
        let mut f = Fixture::new(None);
        let paths = AppPaths::resolve(Some(f._root.path())).unwrap();
        let run = f
            .library
            .lock()
            .unwrap()
            .attempt_run_dir(&f.job.attempt)
            .unwrap();
        let pids = run.join("real-native-pids.json");
        let worker = f._root.path().join("fixture-python");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/inference-supervisor/tests/fixtures/worker.py")
            .canonicalize()
            .unwrap();
        fs::write(&worker, format!("#!/usr/bin/env python3\nimport os,sys\nos.execv(sys.executable,[sys.executable,{fixture:?},'hold-gil',{pids:?}])\n")).unwrap();
        fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
        f.ready.worker_program = worker;
        f.controller = GenerationController::new(paths, f.library.clone(), native.clone());
        f.start();
        wait_for(|| pids.exists());
        let record: serde_json::Value = serde_json::from_slice(
            &fs::read(run.join(inference_supervisor::LIFECYCLE_RECORD_NAME)).unwrap(),
        )
        .unwrap();
        let pgid = record["compute_pgid"].as_u64().unwrap() as i32;
        let mut cleanup = TestGroupCleanup(pgid);
        if cancel_first {
            f.controller.cancel(&f.job.job_id).unwrap();
        }
        assert_eq!(
            unsafe {
                libc::kill(
                    record["supervisor_pid"].as_u64().unwrap() as i32,
                    libc::SIGKILL,
                )
            },
            0
        );
        f.wait();
        assert_eq!(
            f.record().state,
            if cancel_first {
                AttemptState::Cancelled
            } else {
                AttemptState::Failed
            }
        );
        assert!(f.record().manifest.unwrap().outputs.is_empty());
        wait_for(|| unsafe { libc::kill(-pgid, 0) != 0 });
        cleanup.0 = 0;
    }
}

const SCRIPT: &str = r#"
import json,sys,os,time,hashlib,struct,zlib,fcntl,subprocess
config=json.load(open(sys.argv[2]))
hello=json.loads(sys.stdin.readline());seq=0
def send(message,source):
 global seq
 result=dict(family='image-studio.generation',version=1,session_id=source['session_id'],request_id=source['request_id'],sequence=seq,execution=source['execution'],message=message)
 print(json.dumps(result),flush=True);seq+=1
send(dict(type='hello_accepted',versions=dict(min=1,max=1),selected_version=1,identity=config['expected_worker'],capabilities=['image.text-to-image.z-image-turbo.v1']),hello)
execute=json.loads(sys.stdin.readline());request=execute['message']['request']
if MODE=='supervisor-crash':
 lease=open(config['execution_lock'],'a');fcntl.flock(lease,fcntl.LOCK_EX)
 ready=os.path.join(request['output_dir'],'compute-started.json')
 child_code='import ctypes,json,os,subprocess,sys,time; fd=int(sys.argv[1]); child=subprocess.Popen([sys.executable,"-c","import time;time.sleep(60)"],pass_fds=(fd,),stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL); json.dump(dict(worker=os.getpid(),descendant=child.pid),open(sys.argv[2],"w")); ctypes.PyDLL(None).sleep(60)'
 child=subprocess.Popen([sys.executable,'-c',child_code,str(lease.fileno()),ready],pass_fds=(lease.fileno(),),start_new_session=True,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 while not os.path.exists(ready):time.sleep(.01)
 lifecycle=dict(schema_version=1,supervisor_pid=os.getpid(),compute_pgid=child.pid)
 temporary=os.path.join(request['output_dir'],'lifecycle.tmp');json.dump(lifecycle,open(temporary,'w'));os.replace(temporary,os.path.join(request['output_dir'],'compute-lifecycle.json'))
 send(dict(type='progress',stage='loading',completed=None,total=None),execute)
 time.sleep(60);sys.exit(9)
if MODE=='malformed':
 print('{bad json}',flush=True);sys.stdin.read();sys.exit(0)
if MODE=='cleanup':
 lease=open(config['execution_lock'],'w');fcntl.flock(lease,fcntl.LOCK_EX)
 send(dict(type='error',code='CLEANUP_INCOMPLETE',summary='fixture holds its lease'),execute)
 cancel=json.loads(sys.stdin.readline());assert cancel['message']['type']=='cancel'
 send(dict(type='cancelled'),execute);sys.exit(0)
width,height=request['width'],request['height']
def chunk(tag,data):return struct.pack('>I',len(data))+tag+data+struct.pack('>I',zlib.crc32(tag+data)&0xffffffff)
pixels=(b'\0'+b'\x80\x80\x80'*width)*height
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(pixels))+chunk(b'IEND',b'')
open(os.path.join(request['output_dir'],'result.png'),'wb').write(png)
send(dict(type='progress',stage='sampling',completed=9,total=9),execute)
send(dict(type='output_candidate',candidate=dict(relative_path='result.png',width=width,height=height,size_bytes=len(png),sha256=hashlib.sha256(png).hexdigest())),execute)
open(os.path.join(request['output_dir'],'candidate-sent'),'w').write('fixture, no real model')
time.sleep(0.5)
"#;
