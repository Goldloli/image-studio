//! Generation v2 wrapper for the fixed stable-diffusion.cpp MiniMax-H3-Image runtime.

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
    time::{Duration, Instant},
};
use tokenizers::Tokenizer;
use worker_protocol::generation::{
    self, Envelope, ExecuteRequestV2, ExecutionContext, Message, OutputCandidateV2, ProgressStage,
    VERSIONS_V2, WorkerIdentity,
};

#[cfg(windows)]
mod platform {
    pub const BUILD_ID: &str = "studio-minimax-h3-worker-windows-0.1.0";
    pub const RUNTIME_LOCK_ID: &str = "minimax-h3-sdcpp-3b12781-windows-cuda12-v1";
    pub const MODEL_SNAPSHOT_ID: &str = "minimax-h3-image-q3ks-sdcpp-windows-v1";
    pub const MODEL_ID: &str = "minimax-h3-image-q3ks-sdcpp-windows-v1";
    pub const CAPABILITY: &str = "image.text-to-image.minimax-h3-image.gguf.windows-v1";
    pub const RECIPE_ID: &str = "minimax-h3-image-txt2img-q3-q2te-6-windows-v1";
    pub const SD_CLI_NAME: &str = "sd-cli.exe";
    pub const BACKEND: &str = "cuda0";
}

#[cfg(not(windows))]
mod platform {
    pub const BUILD_ID: &str = "studio-minimax-h3-worker-macos-0.1.0";
    pub const RUNTIME_LOCK_ID: &str = "minimax-h3-sdcpp-3b12781-macos-arm64-metal-v1";
    pub const MODEL_SNAPSHOT_ID: &str = "minimax-h3-image-q3ks-sdcpp-macos-v1";
    pub const MODEL_ID: &str = "minimax-h3-image-q3ks-sdcpp-macos-v1";
    pub const CAPABILITY: &str = "image.text-to-image.minimax-h3-image.gguf.macos-v1";
    pub const RECIPE_ID: &str = "minimax-h3-image-txt2img-q3-q2te-6-macos-v1";
    pub const SD_CLI_NAME: &str = "sd-cli";
    pub const BACKEND: &str = "MTL0";
}

use platform::{
    BACKEND, BUILD_ID, CAPABILITY, MODEL_ID, MODEL_SNAPSHOT_ID, RECIPE_ID, RUNTIME_LOCK_ID,
    SD_CLI_NAME,
};

const MAX_TOKEN_COUNT: usize = 4096;

fn identity() -> WorkerIdentity {
    WorkerIdentity {
        build_id: BUILD_ID.into(),
        runtime_lock_id: RUNTIME_LOCK_ID.into(),
        model_snapshot_id: MODEL_SNAPSHOT_ID.into(),
    }
}

fn log(message: impl AsRef<str>) {
    eprintln!("minimax-h3 worker: {}", message.as_ref());
}

enum Inbound {
    Frame(Result<Box<Envelope>, String>),
    Eof,
}

fn read_messages() -> mpsc::Receiver<Inbound> {
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

struct Paths {
    sd_cli: PathBuf,
    diffusion_model: PathBuf,
    text_encoder: PathBuf,
    vae: PathBuf,
    tokenizer: PathBuf,
}

fn parse_paths(args: impl IntoIterator<Item = OsString>) -> Result<Paths, String> {
    let mut values = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let flag = flag.to_str().ok_or("worker flag is not UTF-8")?;
        if !matches!(
            flag,
            "--sd-cli" | "--diffusion-model" | "--llm" | "--vae" | "--tokenizer"
        ) {
            return Err(format!("unexpected worker argument: {flag}"));
        }
        if values.contains_key(flag) {
            return Err(format!("duplicate worker argument: {flag}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        values.insert(flag.to_owned(), PathBuf::from(value));
    }
    let mut take = |name: &str| {
        values
            .remove(name)
            .ok_or_else(|| format!("missing worker argument: {name}"))
    };
    let paths = Paths {
        sd_cli: take("--sd-cli")?,
        diffusion_model: take("--diffusion-model")?,
        text_encoder: take("--llm")?,
        vae: take("--vae")?,
        tokenizer: take("--tokenizer")?,
    };
    if !values.is_empty() {
        return Err("unexpected worker arguments".into());
    }
    Ok(paths)
}

fn strip_verbatim(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

fn trusted_file(path: &Path, expected_name: Option<&str>) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{}: path is not absolute", path.display()));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{}: not a regular file", path.display()));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if let Some(name) = expected_name
        && canonical.file_name().and_then(|value| value.to_str()) != Some(name)
    {
        return Err(format!("expected {name}"));
    }
    Ok(strip_verbatim(&canonical))
}

fn trusted_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{}: path is not absolute", path.display()));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!("{}: not a regular directory", path.display()));
    }
    Ok(strip_verbatim(
        &path
            .canonicalize()
            .map_err(|error| format!("{}: {error}", path.display()))?,
    ))
}

fn token_count(tokenizer: &Tokenizer, text: &str) -> Result<usize, String> {
    tokenizer
        .encode(text, true)
        .map(|encoding| encoding.get_ids().len())
        .map_err(|error| format!("tokenizer failed: {error}"))
}

fn validate_request(request: &ExecuteRequestV2, paths: &Paths) -> Result<PathBuf, String> {
    request
        .validate()
        .map_err(|error| format!("invalid request: {error}"))?;
    if request.model_id != MODEL_ID
        || request.capability_id != CAPABILITY
        || request.recipe_id != RECIPE_ID
        || request.runtime_lock_id != RUNTIME_LOCK_ID
        || request.model_snapshot_id != MODEL_SNAPSHOT_ID
    {
        return Err("model, recipe, runtime or snapshot identity mismatch".into());
    }
    let model = trusted_directory(Path::new(&request.model_path))?;
    let output = trusted_directory(Path::new(&request.output_dir))?;
    if model == output || output.starts_with(&model) {
        return Err("output directory must be outside the model snapshot".into());
    }
    let diffusion = trusted_file(&paths.diffusion_model, None)?;
    let text = trusted_file(&paths.text_encoder, None)?;
    let vae = trusted_file(&paths.vae, None)?;
    let tokenizer_path = trusted_file(&paths.tokenizer, Some("tokenizer.json"))?;
    for component in [&diffusion, &text, &vae, &tokenizer_path] {
        if component.parent() != Some(model.as_path()) {
            return Err("model component is outside the installed snapshot".into());
        }
    }
    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|error| format!("tokenizer load failed: {error}"))?;
    let count = token_count(&tokenizer, &request.instruction)?;
    if count > MAX_TOKEN_COUNT {
        return Err(format!(
            "prompt uses {count} tokens; MiniMax-H3-Image limit is {MAX_TOKEN_COUNT}"
        ));
    }
    Ok(output)
}

fn forward_logs(mut reader: impl Read + Send + 'static) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(count) => {
                    let _ = std::io::stderr().write_all(&buffer[..count]);
                    let _ = std::io::stderr().flush();
                }
            }
        }
    })
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
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
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

struct NativeChild(Child);

impl Drop for NativeChild {
    fn drop(&mut self) {
        let _ = terminate(&mut self.0);
    }
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

fn run_native(
    paths: &Paths,
    request: &ExecuteRequestV2,
    receiver: &mpsc::Receiver<Inbound>,
    writer: &mut Writer,
    request_id: &str,
    execution: &ExecutionContext,
) -> Result<Result<OutputCandidateV2, String>, String> {
    let output_dir = validate_request(request, paths)?;
    let output = output_dir.join("primary.png");
    if output.exists() || output.is_symlink() {
        return Err("attempt output already exists".into());
    }
    let sd_cli = trusted_file(&paths.sd_cli, Some(SD_CLI_NAME))?;
    let mut child = NativeChild(
        Command::new(&sd_cli)
            .args([
                OsString::from("--diffusion-model"),
                strip_verbatim(&paths.diffusion_model).into_os_string(),
                OsString::from("--llm"),
                strip_verbatim(&paths.text_encoder).into_os_string(),
                OsString::from("--vae"),
                strip_verbatim(&paths.vae).into_os_string(),
                OsString::from("--backend"),
                OsString::from(BACKEND),
                OsString::from("-p"),
                request.instruction.clone().into(),
                OsString::from("-s"),
                OsString::from(request.seed.unwrap_or_default().to_string()),
                OsString::from("--steps"),
                OsString::from("6"),
                OsString::from("--cfg-scale"),
                OsString::from("1.0"),
                OsString::from("--sampling-method"),
                OsString::from("euler"),
                OsString::from("--sigmas"),
                OsString::from("minimax-h3-image"),
                OsString::from("-W"),
                OsString::from(request.outputs[0].width.to_string()),
                OsString::from("-H"),
                OsString::from(request.outputs[0].height.to_string()),
                OsString::from("--rng"),
                OsString::from("cpu"),
                OsString::from("--offload-to-cpu"),
                OsString::from("--vae-tiling"),
                OsString::from("--flash-attn"),
                OsString::from("off,diffusion=on"),
                OsString::from("--preview"),
                OsString::from("none"),
                OsString::from("-v"),
                OsString::from("-o"),
                output.clone().into_os_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start stable-diffusion.cpp: {error}"))?,
    );
    let stdout = child.0.stdout.take().ok_or("missing sd-cli stdout")?;
    let stderr = child.0.stderr.take().ok_or("missing sd-cli stderr")?;
    let stdout_thread = forward_logs(stdout);
    let stderr_thread = forward_logs(stderr);
    writer.progress(request_id, ProgressStage::Loading)?;
    writer.progress(request_id, ProgressStage::Inference)?;
    let mut cancelled = false;
    let status = loop {
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
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            if output.is_file() {
                let _ = fs::remove_file(&output);
            }
            return Ok(Err("cancelled".into()));
        }
        if let Some(status) = child.0.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    if !status.success() {
        return Ok(Err(format!("stable-diffusion.cpp exited with {status}")));
    }
    writer.progress(request_id, ProgressStage::Writing)?;
    let (width, height) = png_dimensions(&output)?;
    let expected = (request.outputs[0].width, request.outputs[0].height);
    if (width, height) != expected {
        return Err(format!(
            "stable-diffusion.cpp output is {width}x{height}, expected {}x{}",
            expected.0, expected.1
        ));
    }
    let size = output.metadata().map_err(|error| error.to_string())?.len();
    if size == 0 || size > request.outputs[0].max_bytes {
        return Err("output PNG exceeds its byte budget".into());
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
        return Err("output must be 8-bit RGB PNG".into());
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
    let paths = parse_paths(args)?;
    let receiver = read_messages();
    let hello = recv(&receiver, Duration::from_secs(10))?;
    let Message::Hello {
        versions,
        expected_worker,
        capabilities,
        ..
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
    let result = run_native(
        &paths,
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
            Err("MiniMax-H3-Image inference failed".into())
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

    fn execution() -> ExecutionContext {
        ExecutionContext {
            job_id: "job".into(),
            attempt_id: "attempt".into(),
            execution_token: "token".into(),
        }
    }

    #[test]
    fn cancel_requires_exact_active_identity_and_sequence() {
        let execution = execution();
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
        value.execution.as_mut().unwrap().execution_token = "old".into();
        assert!(!is_expected_cancel(
            &value, "session", "execute", &execution
        ));
    }

    #[test]
    fn path_parser_requires_each_component_once() {
        let args = [
            "--sd-cli",
            "/tmp/sd-cli",
            "--diffusion-model",
            "/tmp/dit",
            "--llm",
            "/tmp/text",
            "--vae",
            "/tmp/vae",
            "--tokenizer",
            "/tmp/tokenizer.json",
        ]
        .map(OsString::from);
        assert!(parse_paths(args).is_ok());
        assert!(parse_paths(["--sd-cli", "/tmp/sd-cli"].map(OsString::from)).is_err());
    }
}
