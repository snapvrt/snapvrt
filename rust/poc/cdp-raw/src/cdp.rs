use anyhow::{Context, Result, bail};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// A CDP event received from the browser.
struct CdpEvent {
    method: String,
    params: Value,
}

/// Per-target WebSocket CDP connection.
///
/// Each tab gets its own connection — no multiplexing, no contention.
/// Reads are inline (no background task) since each connection is single-owner.
pub struct CdpConnection {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    event_buffer: Vec<CdpEvent>,
}

impl CdpConnection {
    /// Connect to a CDP WebSocket URL (browser or per-target).
    pub async fn connect(url: &str) -> Result<Self> {
        let (ws, _) = connect_async(url)
            .await
            .with_context(|| format!("Failed to connect to {url}"))?;

        Ok(Self {
            ws,
            next_id: 1,
            event_buffer: Vec::new(),
        })
    }

    /// Send a CDP command and wait for the matching response (by id).
    /// Events received while waiting are buffered for later retrieval.
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let msg = json!({
            "id": id,
            "method": method,
            "params": params,
        });

        self.ws
            .send(Message::Text(msg.to_string().into()))
            .await
            .with_context(|| format!("Failed to send CDP command {method}"))?;

        // Read messages until we get the matching response.
        loop {
            let raw = self
                .ws
                .next()
                .await
                .context("WebSocket closed while waiting for response")?
                .context("WebSocket error")?;

            let Message::Text(text) = raw else {
                continue; // Skip binary/ping/pong frames
            };

            let parsed: Value =
                serde_json::from_str(&text).context("Failed to parse CDP message")?;

            // Check if this is our response (has matching id).
            if parsed.get("id").and_then(|v| v.as_u64()) == Some(id) {
                if let Some(error) = parsed.get("error") {
                    bail!(
                        "CDP error for {method}: {}",
                        serde_json::to_string(error).unwrap_or_default()
                    );
                }
                return Ok(parsed.get("result").cloned().unwrap_or(Value::Null));
            }

            // Otherwise it's an event — buffer it.
            if let Some(event_method) = parsed.get("method").and_then(|v| v.as_str()) {
                self.event_buffer.push(CdpEvent {
                    method: event_method.to_string(),
                    params: parsed.get("params").cloned().unwrap_or(Value::Null),
                });
            }
        }
    }

    /// Wait for a specific CDP event (by method name).
    /// Checks the buffer first, then reads from WebSocket.
    pub async fn wait_event(&mut self, method: &str) -> Result<Value> {
        // Check buffer first.
        if let Some(idx) = self.event_buffer.iter().position(|e| e.method == method) {
            return Ok(self.event_buffer.remove(idx).params);
        }

        // Read from WebSocket until we get the event.
        loop {
            let raw = self
                .ws
                .next()
                .await
                .context("WebSocket closed while waiting for event")?
                .context("WebSocket error")?;

            let Message::Text(text) = raw else {
                continue;
            };

            let parsed: Value =
                serde_json::from_str(&text).context("Failed to parse CDP message")?;

            if let Some(event_method) = parsed.get("method").and_then(|v| v.as_str()) {
                let params = parsed.get("params").cloned().unwrap_or(Value::Null);
                if event_method == method {
                    return Ok(params);
                }
                // Buffer other events.
                self.event_buffer.push(CdpEvent {
                    method: event_method.to_string(),
                    params,
                });
            }
            // Ignore non-event messages (stale responses, etc.)
        }
    }

    /// Close the WebSocket connection.
    pub async fn close(mut self) -> Result<()> {
        self.ws.close(None).await.ok();
        Ok(())
    }
}
