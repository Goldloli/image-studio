//! Native protocol wrapper for the fixed Real-ESRGAN x4plus ncnn executable.

use png::{BitDepth, ColorType};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
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

/// Frozen worker identity per operating system. macOS keeps the initial native
/// worker identity; Windows registers the independent identities frozen in
/// design.md section 11 for the official Windows ncnn build.
#[cfg(windows)]
mod platform {
    pub const BUILD_ID: &str = "studio-realesrgan-worker-windows-0.1.0";
    pub const RUNTIME_LOCK_ID: &str = "realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1";
    pub const MODEL_SNAPSHOT_ID: &str = "realesrgan-x4plus-ncnn-windows-v1";
    pub const MODEL_ID: &str = "realesrgan-x4plus-windows-v1";
    pub const CAPABILITY: &str = "image.upscale.realesrgan-x4plus.windows-v1";
    pub const RECIPE_ID: &str = "realesrgan-x4plus-windows-t128-v1";
    pub const NCNN_SHA256: &str =
        "07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c";
    pub const NCNN_BYTES: u64 = 6_161_408;
    pub const NCNN_FILE_NAME: &str = "realesrgan-ncnn-vulkan.exe";
}
#[cfg(not(windows))]
mod platform {
    pub const BUILD_ID: &str = "studio-realesrgan-worker-0.1.0";
    pub const RUNTIME_LOCK_ID: &str = "realesrgan-x4plus-ncnn-macos-v0.2.0-v1";
    pub const MODEL_SNAPSHOT_ID: &str = "realesrgan-x4plus-ncnn";
    pub const MODEL_ID: &str = "realesrgan-x4plus";
    pub const CAPABILITY: &str = "image.upscale.realesrgan-x4plus.v1";
    pub const RECIPE_ID: &str = "realesrgan-x4plus-t128-v1";
    pub const NCNN_SHA256: &str =
        "c1c35d92079085de96b9d547fd7e4464bc8a2e9ccf28d7b8c712d72ade91b7cc";
    pub const NCNN_BYTES: u64 = 26_787_080;
    pub const NCNN_FILE_NAME: &str = "realesrgan-ncnn-vulkan";
}
use platform::{
    BUILD_ID, CAPABILITY, MODEL_ID, MODEL_SNAPSHOT_ID, NCNN_BYTES, NCNN_FILE_NAME, NCNN_SHA256,
    RECIPE_ID, RUNTIME_LOCK_ID,
};

/// Upstream ncnn model name; identical on both frozen platforms.
const MODEL_NAME: &str = "realesrgan-x4plus";
const MODEL_WEIGHT_FILES: [&str; 2] = ["realesrgan-x4plus.bin", "realesrgan-x4plus.param"];
const MODEL_BIN_SHA256: &str = "713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf";
const MODEL_PARAM_SHA256: &str = "35330ececcea33b6c397a72548e788d5d53becee4734c50b7fada36e89f10a86";
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

fn trusted_binary_name(name: Option<&OsStr>) -> bool {
    name == Some(OsStr::new(NCNN_FILE_NAME))
}

/// What a trusted user path must resolve to.
#[derive(Clone, Copy)]
enum PathKind {
    File,
    Directory,
}

/// Windows `Path::canonicalize` returns an extended-length `\\?\C:\...` path and
/// `\\?\UNC\server\share` for UNC paths. The native ncnn executable cannot open
/// the verbatim form (`_wfopen` failure), and a plain absolute path never compares
/// equal to it, so the worker uses the stripped form for both comparison and the
/// `-i`/`-o`/`-m` arguments.
#[cfg(windows)]
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path.to_path_buf()
}

#[cfg(not(windows))]
fn strip_verbatim(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Normalized comparison key. Windows paths are case-insensitive and accept both
/// separators, so fold both; other platforms keep byte equality semantics.
#[cfg(windows)]
fn comparable_path(path: &Path) -> String {
    strip_verbatim(path)
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
}

#[cfg(windows)]
fn same_path(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

#[cfg(not(windows))]
fn same_path(left: &Path, right: &Path) -> bool {
    left == right
}

/// Validate a user-supplied absolute path exactly as before (absolute, not a
/// symlink, canonicalize succeeds, already normalized) and return the plain
/// absolute path for native use. On Windows the verbatim prefix is stripped from
/// the canonical result and the input comparison is case-insensitive.
fn trusted_user_path(path: &Path, kind: PathKind) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{}: path is not absolute", path.display()));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{}: path is a symlink", path.display()));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if !same_path(&canonical, path) {
        return Err(format!("{}: path is not normalized", path.display()));
    }
    let trusted = match kind {
        PathKind::File if metadata.is_file() => true,
        PathKind::Directory if metadata.is_dir() => true,
        _ => false,
    };
    if !trusted {
        return Err(format!(
            "{}: path is not a trusted regular {}",
            path.display(),
            match kind {
                PathKind::File => "file",
                PathKind::Directory => "directory",
            }
        ));
    }
    Ok(strip_verbatim(&canonical))
}

fn parse_binary(args: impl IntoIterator<Item = OsString>) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(OsStr::new("--ncnn-binary")) {
        return Err("expected --ncnn-binary <path>".into());
    }
    let path = PathBuf::from(args.next().ok_or("missing ncnn binary")?);
    if args.next().is_some() {
        return Err("unexpected worker argument".into());
    }
    let path = trusted_user_path(&path, PathKind::File).map_err(|_| {
        format!("ncnn binary must be an absolute regular file named {NCNN_FILE_NAME}")
    })?;
    if !trusted_binary_name(path.file_name()) {
        return Err(format!(
            "ncnn binary must be an absolute regular file named {NCNN_FILE_NAME}"
        ));
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
    verify_file(&path, NCNN_SHA256, Some(NCNN_BYTES))?;
    Ok(path)
}

/// The frozen weight pair may sit at the install root (the existing macOS
/// layout) or in the archive's `models/` directory. Exactly one layout must
/// exist, and the bytes are still verified by hash afterwards.
fn model_weights_dir(model: &Path) -> Result<PathBuf, String> {
    let candidates = [model.to_path_buf(), model.join("models")];
    let matches = candidates
        .iter()
        .filter(|candidate| {
            MODEL_WEIGHT_FILES
                .iter()
                .all(|name| candidate.join(name).is_file())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err("model directory is missing the fixed realesrgan-x4plus weight pair".into()),
        _ => Err("model directory contains an ambiguous realesrgan-x4plus weight layout".into()),
    }
}

fn validate_request(request: &ExecuteRequestV2) -> Result<(), String> {
    request
        .validate()
        .map_err(|error| format!("invalid request: {error}"))?;
    let expected = BTreeMap::from([
        ("recipe".into(), serde_json::json!(RECIPE_ID)),
        ("recipe_version".into(), serde_json::json!(1)),
        ("model".into(), serde_json::json!(MODEL_NAME)),
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
    let model = trusted_user_path(Path::new(&request.model_path), PathKind::Directory)
        .map_err(|_| "model and output paths are not normalized local directories".to_string())?;
    let output_dir = trusted_user_path(Path::new(&request.output_dir), PathKind::Directory)
        .map_err(|_| "model and output paths are not normalized local directories".to_string())?;
    if same_path(&model, &output_dir) {
        return Err("model and output paths are not normalized local directories".into());
    }
    let weights = model_weights_dir(&model)?;
    verify_file(
        &weights.join(MODEL_WEIGHT_FILES[0]),
        MODEL_BIN_SHA256,
        Some(33_424_520),
    )?;
    verify_file(
        &weights.join(MODEL_WEIGHT_FILES[1]),
        MODEL_PARAM_SHA256,
        Some(116_029),
    )?;
    let source_path = trusted_user_path(Path::new(&source.path), PathKind::File)
        .map_err(|_| "source is not an Attempt-local regular file".to_string())?;
    if source_path
        .parent()
        .is_none_or(|parent| !same_path(parent, &output_dir))
    {
        return Err("source is not Attempt-local".into());
    }
    verify_file(&source_path, &source.sha256, None)?;
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
    // ncnn cannot open Windows verbatim (`\\?\`) paths and rejects nothing else
    // here: hand it the normalized plain form for every user path.
    let output_dir = trusted_user_path(Path::new(&request.output_dir), PathKind::Directory)?;
    let model = trusted_user_path(Path::new(&request.model_path), PathKind::Directory)?;
    let source = &request.inputs[0];
    let source_path = trusted_user_path(Path::new(&source.path), PathKind::File)?;
    let output = output_dir.join("primary.png");
    if output.exists() || output.is_symlink() {
        return Err("attempt output already exists".into());
    }
    let weights = model_weights_dir(&model)?;
    let weights = weights.to_str().ok_or("model path is not UTF-8")?;
    let mut child = NativeChild(
        Command::new(binary)
            .args([
                "-i",
                source_path.to_str().ok_or("source path is not UTF-8")?,
                "-o",
                output.to_str().ok_or("output path is not UTF-8")?,
                "-s",
                "4",
                "-t",
                "128",
                "-m",
                weights,
                "-n",
                MODEL_NAME,
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
    normalize_ncnn_png(&output)?;
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

/// The official Windows ncnn build writes RGBA PNGs even for RGB inputs, while
/// the frozen v2 output contract is exactly one 8-bit RGB candidate (the macOS
/// build already writes RGB). Normalize in place for Windows and reject
/// non-opaque alpha instead of silently dropping it; RGB output is untouched.
fn normalize_ncnn_png(path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let buffer_size = reader
        .output_buffer_size()
        .ok_or("ncnn output PNG buffer size overflow")?;
    let mut buffer = vec![0; buffer_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    match (info.bit_depth, info.color_type) {
        (BitDepth::Eight, ColorType::Rgb) => Ok(()),
        (BitDepth::Eight, ColorType::Rgba) => {
            let pixels = &buffer[..info.buffer_size()];
            let (pixels, remainder) = pixels.as_chunks::<4>();
            if !remainder.is_empty() {
                return Err("ncnn RGBA output has a partial trailing pixel".into());
            }
            if pixels.iter().any(|pixel| pixel[3] != 255) {
                return Err("ncnn output contains non-opaque alpha".into());
            }
            let mut rgb = Vec::with_capacity(pixels.len() * 3);
            for pixel in pixels {
                rgb.extend_from_slice(&pixel[..3]);
            }
            let temporary = path.with_extension("rgb-tmp");
            if fs::symlink_metadata(&temporary).is_ok() {
                return Err("ncnn normalization scratch file already exists".into());
            }
            {
                let file = File::create(&temporary).map_err(|error| error.to_string())?;
                let mut encoder = png::Encoder::new(file, info.width, info.height);
                encoder.set_color(ColorType::Rgb);
                encoder.set_depth(BitDepth::Eight);
                let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
                writer
                    .write_image_data(&rgb)
                    .map_err(|error| error.to_string())?;
                writer.finish().map_err(|error| error.to_string())?;
            }
            fs::rename(&temporary, path).map_err(|error| error.to_string())
        }
        _ => Err(format!(
            "ncnn output PNG must be 8-bit RGB (color type {:?}, bit depth {:?})",
            info.color_type, info.bit_depth
        )),
    }
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

    #[test]
    fn worker_identity_is_frozen_per_operating_system() {
        #[cfg(windows)]
        {
            assert_eq!(BUILD_ID, "studio-realesrgan-worker-windows-0.1.0");
            assert_eq!(
                RUNTIME_LOCK_ID,
                "realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1"
            );
            assert_eq!(MODEL_SNAPSHOT_ID, "realesrgan-x4plus-ncnn-windows-v1");
            assert_eq!(MODEL_ID, "realesrgan-x4plus-windows-v1");
            assert_eq!(CAPABILITY, "image.upscale.realesrgan-x4plus.windows-v1");
            assert_eq!(RECIPE_ID, "realesrgan-x4plus-windows-t128-v1");
            assert_eq!(NCNN_BYTES, 6_161_408);
            assert_eq!(NCNN_FILE_NAME, "realesrgan-ncnn-vulkan.exe");
            assert!(trusted_binary_name(Some(OsStr::new(
                "realesrgan-ncnn-vulkan.exe"
            ))));
            assert!(!trusted_binary_name(Some(OsStr::new(
                "realesrgan-ncnn-vulkan"
            ))));
        }
        #[cfg(not(windows))]
        {
            assert_eq!(BUILD_ID, "studio-realesrgan-worker-0.1.0");
            assert_eq!(RUNTIME_LOCK_ID, "realesrgan-x4plus-ncnn-macos-v0.2.0-v1");
            assert_eq!(MODEL_SNAPSHOT_ID, "realesrgan-x4plus-ncnn");
            assert_eq!(MODEL_ID, "realesrgan-x4plus");
            assert_eq!(CAPABILITY, "image.upscale.realesrgan-x4plus.v1");
            assert_eq!(RECIPE_ID, "realesrgan-x4plus-t128-v1");
            assert_eq!(NCNN_BYTES, 26_787_080);
            assert_eq!(NCNN_FILE_NAME, "realesrgan-ncnn-vulkan");
            assert!(trusted_binary_name(Some(OsStr::new(
                "realesrgan-ncnn-vulkan"
            ))));
            assert!(!trusted_binary_name(Some(OsStr::new(
                "realesrgan-ncnn-vulkan.exe"
            ))));
        }
        assert_eq!(
            identity(),
            WorkerIdentity {
                build_id: BUILD_ID.into(),
                runtime_lock_id: RUNTIME_LOCK_ID.into(),
                model_snapshot_id: MODEL_SNAPSHOT_ID.into(),
            }
        );
        assert_eq!(NCNN_SHA256.len(), 64);
        assert!(NCNN_SHA256.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(MODEL_NAME, "realesrgan-x4plus");
    }

    #[test]
    fn weights_resolve_to_exactly_one_frozen_layout() {
        let root = std::env::temp_dir().join(format!(
            "realesrgan-worker-layout-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let write_pair = |dir: &Path| {
            fs::create_dir_all(dir).unwrap();
            for name in MODEL_WEIGHT_FILES {
                fs::write(dir.join(name), b"fixture").unwrap();
            }
        };
        let flat = root.join("flat");
        write_pair(&flat);
        assert_eq!(model_weights_dir(&flat).unwrap(), flat);
        let nested = root.join("nested");
        write_pair(&nested.join("models"));
        assert_eq!(model_weights_dir(&nested).unwrap(), nested.join("models"));
        assert!(model_weights_dir(&root).is_err());
        write_pair(&flat.join("models"));
        assert!(model_weights_dir(&flat).is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn verbatim_prefix_is_stripped_before_native_use() {
        #[cfg(windows)]
        {
            assert_eq!(
                strip_verbatim(Path::new(r"\\?\C:\models\realesrgan-x4plus.bin")),
                PathBuf::from(r"C:\models\realesrgan-x4plus.bin")
            );
            assert_eq!(
                strip_verbatim(Path::new(r"\\?\UNC\server\share\model")),
                PathBuf::from(r"\\server\share\model")
            );
            assert_eq!(
                strip_verbatim(Path::new(r"C:\models\model")),
                PathBuf::from(r"C:\models\model")
            );
        }
        #[cfg(not(windows))]
        assert_eq!(
            strip_verbatim(Path::new("/tmp/model")),
            PathBuf::from("/tmp/model")
        );
    }

    #[cfg(windows)]
    fn scratch_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "realesrgan-worker-path-{}-{label}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[cfg(windows)]
    #[test]
    fn trusted_user_path_accepts_plain_and_verbatim_absolute_paths() {
        let root = scratch_root("accept");
        fs::write(root.join("input.png"), b"png").unwrap();
        let canonical = root.canonicalize().unwrap();
        let plain = strip_verbatim(&canonical);
        assert!(!plain.to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(
            trusted_user_path(&plain, PathKind::Directory).unwrap(),
            plain
        );
        let verbatim = PathBuf::from(format!(r"\\?\{}", plain.display()));
        assert_eq!(
            trusted_user_path(&verbatim, PathKind::Directory).unwrap(),
            plain
        );
        assert_eq!(
            trusted_user_path(&verbatim.join("input.png"), PathKind::File).unwrap(),
            plain.join("input.png")
        );
        // Windows paths compare case-insensitively, so an upper-cased spelling is
        // still the same normalized path.
        let upper = PathBuf::from(plain.to_string_lossy().to_uppercase());
        assert!(trusted_user_path(&upper, PathKind::Directory).is_ok());
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn trusted_user_path_rejects_relative_non_normalized_symlink_and_wrong_kind() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let root = scratch_root("reject");
        fs::create_dir_all(root.join("nested")).unwrap();
        let file = root.join("file.bin");
        fs::write(&file, b"x").unwrap();
        assert!(trusted_user_path(Path::new("relative/dir"), PathKind::Directory).is_err());
        assert!(trusted_user_path(&root.join("nested").join(".."), PathKind::Directory).is_err());
        assert!(trusted_user_path(&root, PathKind::File).is_err());
        assert!(trusted_user_path(&file, PathKind::Directory).is_err());
        // Symlinks are rejected when the platform permits creating one.
        if symlink_dir(root.join("nested"), root.join("link-dir")).is_ok() {
            assert!(trusted_user_path(&root.join("link-dir"), PathKind::Directory).is_err());
        }
        if symlink_file(&file, root.join("link-file.bin")).is_ok() {
            assert!(trusted_user_path(&root.join("link-file.bin"), PathKind::File).is_err());
        }
        // Junctions need no symlink privilege but are reparse points too.
        let junction = root.join("junction-dir");
        let created = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(root.join("nested"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if created {
            assert!(trusted_user_path(&junction, PathKind::Directory).is_err());
        }
        fs::remove_dir_all(&root).unwrap();
    }

    fn scratch_png_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "realesrgan-worker-png-{}-{label}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_png(path: &Path, color: ColorType, pixels: &[u8], width: u32, height: u32) {
        let file = File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(color);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn windows_ncnn_rgba_output_is_normalized_to_the_rgb_contract() {
        let root = scratch_png_root("rgba");
        let rgba = root.join("rgba.png");
        write_png(
            &rgba,
            ColorType::Rgba,
            &[
                10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 1, 2, 3, 255,
            ],
            2,
            2,
        );
        normalize_ncnn_png(&rgba).unwrap();
        let file = File::open(&rgba).unwrap();
        let decoder = png::Decoder::new(BufReader::new(file));
        let reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().color_type, ColorType::Rgb);
        assert_eq!(reader.info().bit_depth, BitDepth::Eight);
        assert_eq!((reader.info().width, reader.info().height), (2, 2));
        // Transparent pixels are rejected, never silently dropped.
        let transparent = root.join("transparent.png");
        write_png(
            &transparent,
            ColorType::Rgba,
            &[0, 0, 0, 255, 0, 0, 0, 128, 0, 0, 0, 255, 0, 0, 0, 255],
            2,
            2,
        );
        assert!(normalize_ncnn_png(&transparent).is_err());
        // An existing RGB output (the macOS behavior) stays byte-identical.
        let rgb = root.join("rgb.png");
        write_png(
            &rgb,
            ColorType::Rgb,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            2,
            2,
        );
        let before = fs::read(&rgb).unwrap();
        normalize_ncnn_png(&rgb).unwrap();
        assert_eq!(fs::read(&rgb).unwrap(), before);
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn trusted_user_path_keeps_macos_byte_equality_semantics() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!(
            "realesrgan-worker-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("nested")).unwrap();
        let canonical = root.canonicalize().unwrap();
        assert_eq!(
            trusted_user_path(&canonical, PathKind::Directory).unwrap(),
            canonical
        );
        assert!(trusted_user_path(Path::new("relative"), PathKind::Directory).is_err());
        assert!(trusted_user_path(&root.join(".."), PathKind::Directory).is_err());
        assert!(trusted_user_path(&canonical, PathKind::File).is_err());
        let link = root.with_extension("link");
        if symlink(&canonical, &link).is_ok() {
            assert!(trusted_user_path(&link, PathKind::Directory).is_err());
            fs::remove_file(&link).unwrap();
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
