//! Native protocol wrapper for the fixed Real-ESRGAN x4plus ncnn executable.

use png::{BitDepth, ColorType};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use worker_protocol::generation::{
    self, Envelope, ExecuteRequestV2, ExecutionContext, Message, OutputCandidateV2, ProgressStage,
    VERSIONS_V2, WorkerIdentity,
};

const BUILD_ID: &str = "studio-realesrgan-worker-0.1.0";
const RUNTIME_LOCK_ID: &str = "realesrgan-x4plus-ncnn-macos-v0.2.0-v1";
const MODEL_SNAPSHOT_ID: &str = "realesrgan-x4plus-ncnn";
const MODEL_ID: &str = "realesrgan-x4plus";
const CAPABILITY: &str = "image.upscale.realesrgan-x4plus.v1";
const RECIPE_ID: &str = "realesrgan-x4plus-t128-v1";
const MODEL_BIN_SHA256: &str = "713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf";
const MODEL_PARAM_SHA256: &str = "35330ececcea33b6c397a72548e788d5d53becee4734c50b7fada36e89f10a86";
const NCNN_SHA256: &str = "c1c35d92079085de96b9d547fd7e4464bc8a2e9ccf28d7b8c712d72ade91b7cc";
const MAX_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;

fn identity() -> WorkerIdentity {
    WorkerIdentity {
        build_id: BUILD_ID.into(),
        runtime_lock_id: RUNTIME_LOCK_ID.into(),
        model_snapshot_id: MODEL_SNAPSHOT_ID.into(),
    }
}

fn log(message: impl AsRef<str>) {
    eprintln!("realesrgan worker: {}", message.as_ref());
}

enum Inbound {
    Frame(Result<Box<Envelope>, String>),
    Eof,
}

fn read_messages() -> mpsc::Receiver<Inbound> {
    // Backpressure bounds memory even if the peer floods valid 64 KiB frames.
    let (sender, receiver) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        loop {
            match generation::read_frame(&mut reader) {
                Ok(Some(frame)) => {
                    let value = Envelope::decode_frame(&frame)
                        .map(Box::new)
                        .map_err(|error| error.to_string());
                    if sender.send(Inbound::Frame(value)).is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = sender.send(Inbound::Eof);
                    return;
                }
                Err(error) => {
                    let _ = sender.send(Inbound::Frame(Err(error.to_string())));
                    let _ = sender.send(Inbound::Eof);
                    return;
                }
            }
        }
    });
    receiver
}

fn recv(receiver: &mpsc::Receiver<Inbound>, timeout: Duration) -> Result<Envelope, String> {
    match receiver.recv_timeout(timeout) {
        Ok(Inbound::Frame(Ok(value))) => Ok(*value),
        Ok(Inbound::Frame(Err(error))) => Err(error),
        Ok(Inbound::Eof) => Err("control channel closed".into()),
        Err(mpsc::RecvTimeoutError::Timeout) => Err("control frame timed out".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("control reader stopped".into()),
    }
}

struct Writer {
    output: std::io::StdoutLock<'static>,
    session_id: String,
    sequence: u64,
    execution: Option<ExecutionContext>,
}

impl Writer {
    fn new(session_id: String) -> Self {
        Self {
            output: std::io::stdout().lock(),
            session_id,
            sequence: 0,
            execution: None,
        }
    }

    fn send(
        &mut self,
        request_id: &str,
        message: Message,
        version: Option<u16>,
    ) -> Result<(), String> {
        let envelope = Envelope {
            family: generation::FAMILY.into(),
            version,
            session_id: self.session_id.clone(),
            request_id: request_id.into(),
            sequence: self.sequence,
            execution: self.execution.clone(),
            message,
        };
        generation::write_message(&mut self.output, &envelope)
            .map_err(|error| error.to_string())?;
        self.sequence += 1;
        Ok(())
    }

    fn progress(&mut self, request_id: &str, stage: ProgressStage) -> Result<(), String> {
        self.send(
            request_id,
            Message::Progress {
                stage,
                completed: None,
                total: None,
            },
            Some(generation::PROTOCOL_VERSION_V2),
        )
    }
}

fn parse_binary(args: impl IntoIterator<Item = OsString>) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--ncnn-binary")) {
        return Err("expected --ncnn-binary <path>".into());
    }
    let path = PathBuf::from(args.next().ok_or("missing ncnn binary")?);
    if args.next().is_some() {
        return Err("unexpected worker argument".into());
    }
    if !path.is_absolute()
        || path.is_symlink()
        || !path.is_file()
        || path.canonicalize().map_err(|error| error.to_string())? != path
        || path.file_name().and_then(|value| value.to_str()) != Some("realesrgan-ncnn-vulkan")
    {
        return Err("ncnn binary is not a trusted absolute regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path
            .metadata()
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("ncnn binary is not executable".into());
        }
    }
    verify_file(&path, NCNN_SHA256, Some(26_787_080))?;
    Ok(path)
}

fn validate_request(request: &ExecuteRequestV2) -> Result<(), String> {
    request
        .validate()
        .map_err(|error| format!("invalid request: {error}"))?;
    let expected = BTreeMap::from([
        ("recipe".into(), serde_json::json!(RECIPE_ID)),
        ("recipe_version".into(), serde_json::json!(1)),
        ("model".into(), serde_json::json!("realesrgan-x4plus")),
        ("scale".into(), serde_json::json!(4)),
        ("tile".into(), serde_json::json!(128)),
        ("threads".into(), serde_json::json!("1:1:1")),
        ("tta".into(), serde_json::json!(false)),
    ]);
    if request.model_id != MODEL_ID
        || request.capability_id != CAPABILITY
        || request.recipe_id != RECIPE_ID
        || request.recipe_version != 1
        || request.runtime_lock_id != RUNTIME_LOCK_ID
        || request.model_snapshot_id != MODEL_SNAPSHOT_ID
        || !request.instruction.is_empty()
        || request.seed.is_some()
        || request.sampling_steps.is_some()
        || request.parameters != expected
        || request.inputs.len() != 1
        || request.outputs.len() != 1
    {
        return Err("Real-ESRGAN identity, recipe or cardinality differs".into());
    }
    let source = &request.inputs[0];
    if source.role != "source"
        || source.width == 0
        || source.height == 0
        || source.width > 1024
        || source.height > 1024
        || u64::from(source.width) * u64::from(source.height) > 1024 * 1024
    {
        return Err("Real-ESRGAN source exceeds the one-megapixel recipe".into());
    }
    let output = &request.outputs[0];
    if output.role != "primary"
        || output.data_type != "image_rgb"
        || output.width != source.width * 4
        || output.height != source.height * 4
        || output.max_bytes == 0
        || output.max_bytes > MAX_OUTPUT_BYTES
    {
        return Err("Real-ESRGAN output differs from exact 4x RGB".into());
    }
    let model = Path::new(&request.model_path);
    let output_dir = Path::new(&request.output_dir);
    if !model.is_absolute()
        || model.is_symlink()
        || !model.is_dir()
        || model.canonicalize().map_err(|error| error.to_string())? != model
        || !output_dir.is_absolute()
        || output_dir.is_symlink()
        || !output_dir.is_dir()
        || output_dir
            .canonicalize()
            .map_err(|error| error.to_string())?
            != output_dir
        || model == output_dir
    {
        return Err("model and output paths are not normalized local directories".into());
    }
    verify_file(
        &model.join("realesrgan-x4plus.bin"),
        MODEL_BIN_SHA256,
        Some(33_424_520),
    )?;
    verify_file(
        &model.join("realesrgan-x4plus.param"),
        MODEL_PARAM_SHA256,
        Some(116_029),
    )?;
    let source_path = Path::new(&source.path);
    if source_path.parent() != Some(output_dir) {
        return Err("source is not Attempt-local".into());
    }
    verify_file(source_path, &source.sha256, None)?;
    Ok(())
}

fn verify_file(path: &Path, sha256: &str, expected_bytes: Option<u64>) -> Result<u64, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file()
        || path.is_symlink()
        || expected_bytes.is_some_and(|value| value != metadata.len())
    {
        return Err(format!("{}: file type or size differs", path.display()));
    }
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|error| error.to_string())?;
    if format!("{:x}", digest.finalize()) != sha256 {
        return Err(format!("{}: SHA-256 differs", path.display()));
    }
    Ok(metadata.len())
}

fn terminate(child: &mut Child) -> Result<(), String> {
    if child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        #[cfg(unix)]
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = child.kill();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
    }
    child.wait().map_err(|error| error.to_string())?;
    Ok(())
}

fn is_expected_cancel(
    value: &Envelope,
    session_id: &str,
    request_id: &str,
    execution: &ExecutionContext,
) -> bool {
    value.session_id == session_id
        && value.request_id == request_id
        && value.version == Some(generation::PROTOCOL_VERSION_V2)
        && value.sequence == 2
        && value.execution.as_ref() == Some(execution)
        && value.message == Message::Cancel
}

fn completed_tile(line: &[u8]) -> bool {
    std::str::from_utf8(line)
        .ok()
        .and_then(|line| line.trim().strip_suffix('%'))
        .and_then(|value| value.parse::<f32>().ok())
        .is_some_and(|percent| percent.is_finite() && percent > 0.0 && percent <= 100.0)
}

/// Keep native diagnostics on stderr. Only an upstream percentage greater than zero
/// proves that at least one tile finished; process spawn/GPU enumeration do not.
fn observe_tiles(mut stderr: impl Read + Send + 'static) -> mpsc::Receiver<()> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        let mut line = Vec::with_capacity(1024);
        let mut oversized = false;
        let mut observed = false;
        loop {
            let count = match stderr.read(&mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(count) => count,
            };
            let _ = std::io::stderr().write_all(&buffer[..count]);
            if observed {
                continue;
            }
            for byte in &buffer[..count] {
                if matches!(byte, b'\n' | b'\r') {
                    if !observed && !oversized && completed_tile(&line) {
                        let _ = sender.try_send(());
                        observed = true;
                    }
                    line.clear();
                    oversized = false;
                } else if line.len() < 1024 {
                    line.push(*byte);
                } else {
                    oversized = true;
                }
            }
        }
    });
    receiver
}

struct NativeChild(Child);

impl Drop for NativeChild {
    fn drop(&mut self) {
        // Also reap on a broken stdout/control channel or output validation error.
        let _ = terminate(&mut self.0);
    }
}

fn run_ncnn(
    binary: &Path,
    request: &ExecuteRequestV2,
    receiver: &mpsc::Receiver<Inbound>,
    writer: &mut Writer,
    request_id: &str,
    execution: &ExecutionContext,
) -> Result<Result<OutputCandidateV2, String>, String> {
    let output = Path::new(&request.output_dir).join("primary.png");
    if output.exists() || output.is_symlink() {
        return Err("attempt output already exists".into());
    }
    let source = &request.inputs[0];
    let mut child = NativeChild(
        Command::new(binary)
            .args([
                "-i",
                &source.path,
                "-o",
                output.to_str().ok_or("output path is not UTF-8")?,
                "-s",
                "4",
                "-t",
                "128",
                "-m",
                &request.model_path,
                "-n",
                "realesrgan-x4plus",
                "-g",
                "0",
                "-j",
                "1:1:1",
                "-f",
                "png",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start ncnn: {error}"))?,
    );
    let tiles = observe_tiles(child.0.stderr.take().ok_or("missing ncnn stderr")?);
    let mut inference_reported = false;
    let mut cancelled = false;
    loop {
        match receiver.try_recv() {
            Ok(Inbound::Frame(Ok(value))) => {
                if !is_expected_cancel(&value, &writer.session_id, request_id, execution) {
                    terminate(&mut child.0)?;
                    return Err("invalid control frame during inference".into());
                }
                if cancelled {
                    terminate(&mut child.0)?;
                    return Err("duplicate cancel".into());
                }
                cancelled = true;
            }
            Ok(Inbound::Frame(Err(error))) => {
                terminate(&mut child.0)?;
                return Err(error);
            }
            Ok(Inbound::Eof) => {
                terminate(&mut child.0)?;
                return Err("control channel closed during inference".into());
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                terminate(&mut child.0)?;
                return Err("control reader stopped during inference".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if cancelled {
            terminate(&mut child.0)?;
            return Ok(Err("cancelled".into()));
        }
        if !inference_reported && tiles.try_recv().is_ok() {
            writer.progress(request_id, ProgressStage::Inference)?;
            inference_reported = true;
        }
        if let Some(status) = child.0.try_wait().map_err(|error| error.to_string())? {
            if !status.success() {
                return Ok(Err(format!("ncnn exited with {status}")));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    match receiver.try_recv() {
        Ok(Inbound::Frame(Ok(value)))
            if is_expected_cancel(&value, &writer.session_id, request_id, execution) =>
        {
            return Ok(Err("cancelled".into()));
        }
        Ok(Inbound::Frame(Ok(_))) => return Err("unexpected control frame after inference".into()),
        Ok(Inbound::Frame(Err(error))) => return Err(error),
        Ok(Inbound::Eof) => return Err("control channel closed before commit".into()),
        Err(mpsc::TryRecvError::Disconnected) => {
            return Err("control reader stopped before commit".into());
        }
        Err(mpsc::TryRecvError::Empty) => {}
    }
    let (width, height) = png_dimensions(&output)?;
    if (width, height) != (source.width * 4, source.height * 4) {
        return Err("ncnn output dimensions differ from exact 4x".into());
    }
    let size = output.metadata().map_err(|error| error.to_string())?.len();
    if size == 0 || size > request.outputs[0].max_bytes {
        return Err("ncnn output PNG exceeds its byte budget".into());
    }
    let sha256 = digest_file(&output)?;
    Ok(Ok(OutputCandidateV2 {
        role: "primary".into(),
        data_type: "image_rgb".into(),
        relative_path: "primary.png".into(),
        width,
        height,
        size_bytes: size,
        sha256,
    }))
}

fn png_dimensions(path: &Path) -> Result<(u32, u32), String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let reader = decoder.read_info().map_err(|error| error.to_string())?;
    let info = reader.info();
    if info.color_type != ColorType::Rgb || info.bit_depth != BitDepth::Eight {
        return Err("ncnn output must be 8-bit RGB PNG".into());
    }
    Ok((info.width, info.height))
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", digest.finalize()))
}

fn execute(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let binary = parse_binary(args)?;
    let receiver = read_messages();
    let hello = recv(&receiver, Duration::from_secs(10))?;
    let Message::Hello {
        versions,
        build_id: _,
        expected_worker,
        capabilities,
    } = &hello.message
    else {
        return Err("first message is not hello".into());
    };
    if hello.version.is_some()
        || hello.execution.is_some()
        || hello.sequence != 0
        || *versions != VERSIONS_V2
        || expected_worker != &identity()
        || capabilities != &[CAPABILITY.to_string()]
    {
        return Err("handshake identity or version differs".into());
    }
    let session_id = hello.session_id.clone();
    let hello_request = hello.request_id.clone();
    let mut writer = Writer::new(session_id.clone());
    writer.send(
        &hello_request,
        Message::HelloAccepted {
            versions: VERSIONS_V2,
            selected_version: generation::PROTOCOL_VERSION_V2,
            identity: identity(),
            capabilities: vec![CAPABILITY.into()],
        },
        Some(generation::PROTOCOL_VERSION_V2),
    )?;

    let active = recv(&receiver, Duration::from_secs(10))?;
    let Message::ExecuteV2 { request } = &active.message else {
        return Err("second message is not execute_v2".into());
    };
    if active.session_id != session_id
        || active.version != Some(generation::PROTOCOL_VERSION_V2)
        || active.sequence != 1
        || active.execution.is_none()
    {
        return Err("execute envelope differs".into());
    }
    let execution = active.execution.clone().ok_or("missing execution")?;
    let request_id = active.request_id.clone();
    writer.execution = Some(execution.clone());
    writer.progress(&request_id, ProgressStage::Loading)?;
    validate_request(request)?;
    let result = run_ncnn(
        &binary,
        request,
        &receiver,
        &mut writer,
        &request_id,
        &execution,
    )?;
    match result {
        Ok(candidate) => {
            writer.send(
                &request_id,
                Message::OutputCandidateV2 { candidate },
                Some(generation::PROTOCOL_VERSION_V2),
            )?;
            Ok(())
        }
        Err(error) if error == "cancelled" => writer.send(
            &request_id,
            Message::Cancelled,
            Some(generation::PROTOCOL_VERSION_V2),
        ),
        Err(error) => {
            writer.send(
                &request_id,
                Message::Error {
                    code: "INFERENCE_ERROR".into(),
                    summary: error.chars().take(4096).collect(),
                },
                Some(generation::PROTOCOL_VERSION_V2),
            )?;
            Err("ncnn inference failed".into())
        }
    }
}

fn main() {
    if let Err(error) = execute(std::env::args_os().skip(1)) {
        log(error);
        std::process::exit(70);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_positive_native_percentage_proves_a_completed_tile() {
        for line in [
            "0.00%",
            "[0 Apple M3 Max]",
            "1.56% garbage",
            "NaN%",
            "inf%",
            "101%",
            "-1%",
        ] {
            assert!(!completed_tile(line.as_bytes()), "{line}");
        }
        assert!(completed_tile(b"1.56%"));
        assert!(completed_tile(b"100.00%"));
    }

    #[test]
    fn native_logs_are_streamed_without_manufacturing_compute_from_startup() {
        let reader = std::io::Cursor::new(b"[0 GPU] ready\n0.00%\n1.56%\n3.12%\n");
        let receiver = observe_tiles(reader);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(receiver.recv_timeout(Duration::from_millis(10)).is_err());
        let receiver = observe_tiles(std::io::Cursor::new(b"[0 GPU] ready\n0.00%\n"));
        assert!(receiver.recv_timeout(Duration::from_secs(2)).is_err());
        let mut line = vec![b' '; 4096];
        line.extend_from_slice(b"1.56%\n");
        let receiver = observe_tiles(std::io::Cursor::new(line));
        assert!(receiver.recv_timeout(Duration::from_secs(2)).is_err());
    }

    #[test]
    fn cancel_requires_the_exact_active_attempt_and_wire_sequence() {
        let execution = ExecutionContext {
            job_id: "job".into(),
            attempt_id: "attempt".into(),
            execution_token: "token".into(),
        };
        let mut value = Envelope {
            family: generation::FAMILY.into(),
            version: Some(2),
            session_id: "session".into(),
            request_id: "execute".into(),
            sequence: 2,
            execution: Some(execution.clone()),
            message: Message::Cancel,
        };
        assert!(is_expected_cancel(&value, "session", "execute", &execution));
        value.execution.as_mut().unwrap().execution_token = "old-token".into();
        assert!(!is_expected_cancel(
            &value, "session", "execute", &execution
        ));
        value.execution = Some(execution.clone());
        value.sequence = 3;
        assert!(!is_expected_cancel(
            &value, "session", "execute", &execution
        ));
        value.sequence = 2;
        value.version = Some(1);
        assert!(!is_expected_cancel(
            &value, "session", "execute", &execution
        ));
    }
}
