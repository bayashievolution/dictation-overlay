//! Chrome Native Messaging stdio protocol.
//!
//! Wire format: `[4-byte little-endian length][JSON payload]`.
//! Extension → host limit is 1 MiB; host → extension limit is 64 MiB.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_IN_SIZE: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InMessage {
    ShowCaption {
        text: String,
        #[serde(default)]
        settings: Option<Value>,
    },
    HideCaption,
    UpdateStyle {
        settings: Value,
    },
    SetPosition {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    /// Phase 2: toggle WS_EX_TRANSPARENT (Win32) / ignoresMouseEvents (macOS).
    SetClickThrough {
        enabled: bool,
    },
    /// Phase 2: position on a specific monitor (bottom-center).
    SetMonitor {
        index: usize,
    },
    /// Phase 2: ask for the list of monitors + their geometry.
    ListMonitors,
    Ping,
    Exit,
}

#[derive(Debug, Serialize)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutMessage<'a> {
    Ready {
        version: &'a str,
        platform: &'a str,
        capabilities: &'a [&'a str],
    },
    Pong,
    Error {
        code: &'a str,
        message: String,
    },
    #[allow(dead_code)]
    PositionChanged {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    MonitorList {
        monitors: Vec<MonitorInfo>,
    },
    ClickThrough {
        enabled: bool,
    },
}

/// Read a single Native Messaging frame from stdin.
///
/// Returns `Ok(None)` on clean EOF (Chrome disconnected).
/// Returns `Err` if the frame is malformed or too large.
pub fn read_message() -> io::Result<Option<InMessage>> {
    let mut stdin = io::stdin().lock();
    let mut len_buf = [0u8; 4];
    match stdin.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "zero-length frame"));
    }
    if len > MAX_IN_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {} bytes", len),
        ));
    }
    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf)?;
    let msg: InMessage = serde_json::from_slice(&buf).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("json parse: {}", e))
    })?;
    Ok(Some(msg))
}

/// Send a single Native Messaging frame on stdout.
pub fn send_message(msg: &OutMessage) -> io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("json serialize: {}", e))
    })?;
    let len = body.len() as u32;
    let mut stdout = io::stdout().lock();
    stdout.write_all(&len.to_le_bytes())?;
    stdout.write_all(&body)?;
    stdout.flush()?;
    Ok(())
}
