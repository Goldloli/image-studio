//! A deliberately limited communication fixture, never an image inference runtime.

use std::{
    env,
    io::{self, BufReader, Write},
    process::{self, Command},
    thread,
    time::Duration,
};

use worker_protocol::{
    DIAGNOSTIC_SCOPE, Envelope, ExecutionContext, MAX_FRAME_BYTES, Message, PROTOCOL_VERSION,
    ProtocolError, VersionRange, diagnostic_capabilities, read_frame, validate_capabilities,
    write_message,
};

const BUILD_ID: &str = concat!("diagnostic-worker/", env!("CARGO_PKG_VERSION"));
const MODES: &[&str] = &[
    "normal",
    "bad-version",
    "malformed",
    "oversize",
    "duplicate",
    "wrong-session",
    "wrong-request",
    "wrong-token",
    "wrong-challenge",
    "wrong-scope",
    "unknown-message",
    "unknown-capability",
    "crash",
    "hang",
    "handshake-hang",
    "ignore-shutdown",
    "flood",
    "descendant",
];

fn main() {
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("diagnostic-worker: {}: {error}", error.code());
            process::exit(2);
        }
    }
}

fn run() -> Result<(), ProtocolError> {
    let mut mode = "normal".to_owned();
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--child" => park(),
            "--mode" => {
                mode = arguments
                    .next()
                    .ok_or(ProtocolError::InvalidEnvelope("missing --mode value"))?;
                if !MODES.contains(&mode.as_str()) {
                    return Err(ProtocolError::InvalidEnvelope("unknown diagnostic mode"));
                }
            }
            "--help" | "-h" => {
                eprintln!("diagnostic-worker --mode <{}>", MODES.join("|"));
                return Ok(());
            }
            _ => return Err(ProtocolError::InvalidEnvelope("unknown worker argument")),
        }
    }
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    if mode == "handshake-hang" {
        park();
    }
    let hello = receive(&mut reader)?;
    hello.validate_diagnostic_context()?;
    let Message::Hello {
        versions,
        build_id,
        capabilities,
    } = &hello.message
    else {
        return Err(ProtocolError::UnexpectedMessage);
    };
    versions.validate()?;
    validate_capabilities(capabilities)?;
    if hello.version.is_some()
        || hello.execution.is_some()
        || hello.sequence != 0
        || hello.session_id.is_empty()
        || hello.request_id.is_empty()
        || build_id.is_empty()
    {
        return Err(ProtocolError::InvalidEnvelope("invalid bootstrap Hello"));
    }
    let worker_versions = if mode == "bad-version" {
        VersionRange { min: 2, max: 2 }
    } else {
        VersionRange::CURRENT
    };
    let selected = versions.highest_common(worker_versions);
    if selected.is_none() && mode != "bad-version" {
        let error = Envelope::new(
            None,
            &hello.session_id,
            &hello.request_id,
            0,
            Message::ProtocolError {
                code: "NO_COMMON_VERSION".into(),
                summary: "No supported diagnostic protocol version".into(),
                versions: Some(worker_versions),
            },
        );
        write_message(&mut writer, &error)?;
        return Ok(());
    }
    let selected = selected.unwrap_or(2);
    let mut worker = Worker {
        session_id: hello.session_id.clone(),
        version: selected,
        sequence: 0,
    };
    worker.send(
        &mut writer,
        &hello.request_id,
        Message::HelloAccepted {
            versions: worker_versions,
            selected_version: selected,
            build_id: BUILD_ID.into(),
            capabilities: if mode == "unknown-capability" {
                vec!["image.generate.v1".into()]
            } else {
                diagnostic_capabilities()
            },
        },
    )?;
    let mut last_host_sequence = hello.sequence;
    loop {
        let Some(frame) = read_frame(&mut reader)? else {
            return Ok(());
        };
        let envelope = Envelope::decode_frame(&frame)?;
        envelope.validate_diagnostic_context()?;
        if envelope.session_id != worker.session_id {
            return Err(ProtocolError::WrongSession);
        }
        if envelope.version != Some(selected) {
            return Err(ProtocolError::VersionMismatch);
        }
        if envelope.execution.is_some() {
            return Err(ProtocolError::UnexpectedExecution);
        }
        if envelope.sequence <= last_host_sequence {
            return Err(ProtocolError::Sequence {
                last: Some(last_host_sequence),
                received: envelope.sequence,
            });
        }
        last_host_sequence = envelope.sequence;
        match &envelope.message {
            Message::Probe { challenge } => {
                if challenge.trim().is_empty() || challenge.len() > 1024 {
                    return Err(ProtocolError::InvalidEnvelope("invalid probe challenge"));
                }
                probe(&mode, &mut worker, &mut writer, &envelope, challenge)?;
            }
            Message::Shutdown if mode != "ignore-shutdown" => {
                worker.send(&mut writer, &envelope.request_id, Message::ShutdownAck)?;
                return Ok(());
            }
            Message::Shutdown => {}
            _ => return Err(ProtocolError::UnexpectedMessage),
        }
    }
}

fn receive(reader: &mut impl io::BufRead) -> Result<Envelope, ProtocolError> {
    let bytes = read_frame(reader)?.ok_or(ProtocolError::TruncatedFrame)?;
    Envelope::decode_frame(&bytes)
}

struct Worker {
    session_id: String,
    version: u16,
    sequence: u64,
}

impl Worker {
    fn envelope(&mut self, request_id: &str, message: Message) -> Envelope {
        let envelope = Envelope::new(
            Some(self.version),
            &self.session_id,
            request_id,
            self.sequence,
            message,
        );
        self.sequence += 1;
        envelope
    }

    fn send(
        &mut self,
        writer: &mut impl Write,
        request_id: &str,
        message: Message,
    ) -> Result<(), ProtocolError> {
        write_message(writer, &self.envelope(request_id, message))
    }
}

fn probe(
    mode: &str,
    worker: &mut Worker,
    writer: &mut impl Write,
    envelope: &Envelope,
    challenge: &str,
) -> Result<(), ProtocolError> {
    match mode {
        "crash" => process::exit(42),
        "hang" => park(),
        "malformed" => {
            writer.write_all(b"ordinary log text on the wrong channel\n")?;
            writer.flush()?;
            park();
        }
        "oversize" => {
            // No newline: the reader must reject before a full line is allocated.
            let chunk = [b'x'; 4096];
            for _ in 0..=MAX_FRAME_BYTES / chunk.len() {
                writer.write_all(&chunk)?;
            }
            writer.flush()?;
            park();
        }
        "unknown-message" => {
            let illegal = format!(
                "{{\"version\":{PROTOCOL_VERSION},\"session_id\":\"{}\",\"request_id\":\"{}\",\"sequence\":{},\"message\":{{\"type\":\"completed\"}}}}\n",
                worker.session_id, envelope.request_id, worker.sequence
            );
            writer.write_all(illegal.as_bytes())?;
            writer.flush()?;
            park();
        }
        "duplicate" => {
            let progress = worker.envelope(
                &envelope.request_id,
                Message::Progress {
                    completed: 1,
                    total: 2,
                },
            );
            write_message(writer, &progress)?;
            write_message(writer, &progress)?;
        }
        "wrong-session" | "wrong-request" | "wrong-token" => {
            let mut rejected = worker.envelope(
                &envelope.request_id,
                Message::Progress {
                    completed: 1,
                    total: 2,
                },
            );
            match mode {
                "wrong-session" => rejected.session_id = "old-session".into(),
                "wrong-request" => rejected.request_id = "not-an-active-request".into(),
                _ => {
                    rejected.execution = Some(ExecutionContext {
                        attempt_id: "old-attempt".into(),
                        fencing_token: "old-token".into(),
                    });
                }
            }
            write_message(writer, &rejected)?;
        }
        "flood" => {
            let log_line = "diagnostic fixture log ".repeat(128);
            for completed in 0..4096 {
                worker.send(
                    writer,
                    &envelope.request_id,
                    Message::Progress {
                        completed,
                        total: 4096,
                    },
                )?;
                eprintln!("{log_line}");
            }
        }
        _ => {
            worker.send(
                writer,
                &envelope.request_id,
                Message::Progress {
                    completed: 1,
                    total: 1,
                },
            )?;
        }
    }
    let child_pid = if mode == "descendant" {
        // Deliberately leave the inherited-group child to the supervisor's cleanup test.
        let child = Command::new(env::current_exe()?).arg("--child").spawn()?;
        let pid = child.id();
        drop(child);
        Some(pid)
    } else {
        None
    };
    worker.send(
        writer,
        &envelope.request_id,
        Message::ProbeResult {
            challenge: if mode == "wrong-challenge" {
                "wrong-challenge".into()
            } else {
                challenge.to_owned()
            },
            build_id: BUILD_ID.into(),
            scope: if mode == "wrong-scope" {
                "image_generation".into()
            } else {
                DIAGNOSTIC_SCOPE.into()
            },
            child_pid,
        },
    )
}

fn park() -> ! {
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
