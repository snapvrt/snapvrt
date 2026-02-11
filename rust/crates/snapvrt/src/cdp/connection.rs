use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tracing::{debug, trace, warn};

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
        debug!(url, "connecting CDP WebSocket");
        let (ws, _) = connect_async(url)
            .await
            .with_context(|| format!("Failed to connect to {url}"))?;
        debug!(url, "CDP WebSocket connected");

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

    /// Wait until all in-flight network requests have completed and no new
    /// requests arrive for 100ms. Gives up after 10s and proceeds (better to
    /// screenshot late content than hang forever).
    ///
    /// Requires `Network.enable` to have been called beforehand.
    pub async fn wait_network_idle(&mut self) -> Result<()> {
        let settle = Duration::from_millis(100);
        let timeout = Duration::from_secs(10);
        let deadline = tokio::time::Instant::now() + timeout;
        let mut pending: HashSet<String> = HashSet::new();

        // Process already-buffered network events.
        for event in &self.event_buffer {
            Self::track_network(&event.method, &event.params, &mut pending);
        }
        trace!(
            buffered_events = self.event_buffer.len(),
            pending = pending.len(),
            "network idle: initial state"
        );

        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                debug!(pending = pending.len(), "network idle: deadline hit");
                return Ok(());
            }

            // If nothing pending, use settle duration; otherwise wait up to the deadline.
            let read_timeout = if pending.is_empty() {
                settle.min(deadline - now)
            } else {
                deadline - now
            };

            match tokio::time::timeout(read_timeout, self.read_event()).await {
                Err(_) => {
                    // Timed out reading. If nothing is pending, the settle
                    // period elapsed with no new requests — network is idle.
                    // If requests are still pending, the overall deadline hit.
                    trace!(pending = pending.len(), "network idle: settled");
                    return Ok(());
                }
                Ok(result) => {
                    let (method, params) = result?;
                    Self::track_network(&method, &params, &mut pending);
                    self.event_buffer.push(CdpEvent { method, params });
                }
            }
        }
    }

    /// Evaluate a synchronous JS expression and return its value.
    pub async fn eval(&mut self, expression: &str) -> Result<Value> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .await
            .context("JS evaluation failed")?;
        Self::check_js_exception(&result)?;
        Ok(result)
    }

    /// Evaluate a JS expression and await its promise.
    pub async fn eval_async(&mut self, expression: &str) -> Result<Value> {
        let snippet: String = expression.chars().take(80).collect();
        debug!(snippet, "eval_async");
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "awaitPromise": true,
                }),
            )
            .await
            .context("JS evaluation failed")?;
        debug!("eval_async done");
        Self::check_js_exception(&result)?;
        Ok(result)
    }

    /// Capture a screenshot of the given clip region and return decoded PNG bytes.
    pub async fn capture_screenshot(&mut self, clip: &super::ClipRect) -> Result<Vec<u8>> {
        let result = self
            .call(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "clip": {
                        "x": clip.x,
                        "y": clip.y,
                        "width": clip.w,
                        "height": clip.h,
                        "scale": 1,
                    },
                }),
            )
            .await
            .context("Failed to capture screenshot")?;

        let b64_data = result["data"]
            .as_str()
            .context("No screenshot data in response")?;

        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(b64_data)
            .context("Failed to decode base64 screenshot")
    }

    /// Bail if a `Runtime.evaluate` result contains an exception.
    fn check_js_exception(result: &Value) -> Result<()> {
        if let Some(desc) = result
            .get("exceptionDetails")
            .and_then(|e| e.get("exception"))
            .and_then(|e| e.get("description"))
            .and_then(|d| d.as_str())
        {
            bail!("JS error: {desc}");
        }
        Ok(())
    }

    /// Wait for the page load event to fire.
    pub async fn wait_page_load(&mut self) -> Result<()> {
        debug!(
            buffered_events = self.event_buffer.len(),
            "waiting for Page.loadEventFired"
        );
        match tokio::time::timeout(
            Duration::from_secs(10),
            self.wait_event("Page.loadEventFired"),
        )
        .await
        {
            Ok(Ok(_)) => {
                debug!("page load event received");
                Ok(())
            }
            Ok(Err(e)) => {
                warn!(error = %format!("{e:#}"), "error waiting for page load");
                Err(e).context("Error waiting for page load")
            }
            Err(_) => {
                warn!("page load timed out after 10s, proceeding anyway");
                Ok(())
            }
        }
    }

    /// Navigate to a URL. Clears the event buffer first — events from prior
    /// navigations on this tab are stale and would pollute wait_page_load /
    /// wait_network_idle.
    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        let stale = self.event_buffer.len();
        self.event_buffer.clear();
        debug!(url, stale_events_cleared = stale, "navigating");
        let result = self
            .call("Page.navigate", json!({"url": url}))
            .await
            .context("Failed to navigate")?;
        debug!(url, frame_id = ?result.get("frameId"), "navigation started");
        Ok(())
    }

    /// Set the emulated viewport size.
    pub async fn set_viewport(&mut self, width: u32, height: u32) -> Result<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": width,
                "height": height,
                "deviceScaleFactor": 1,
                "mobile": false,
            }),
        )
        .await
        .context("Failed to set device metrics")?;
        Ok(())
    }

    /// Enable the Page and Network CDP domains for this connection.
    pub async fn enable_domains(&mut self) -> Result<()> {
        self.call("Page.enable", json!({}))
            .await
            .context("Failed to enable Page domain")?;
        self.call("Network.enable", json!({}))
            .await
            .context("Failed to enable Network domain")?;
        Ok(())
    }

    /// Read the next CDP event from the WebSocket, skipping non-event messages.
    async fn read_event(&mut self) -> Result<(String, Value)> {
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

            if let Some(method) = parsed.get("method").and_then(|v| v.as_str()) {
                let params = parsed.get("params").cloned().unwrap_or(Value::Null);
                return Ok((method.to_string(), params));
            }
            // Skip non-event messages (stale responses).
        }
    }

    /// Update pending request set based on a CDP Network event.
    fn track_network(method: &str, params: &Value, pending: &mut HashSet<String>) {
        let Some(id) = params.get("requestId").and_then(|v| v.as_str()) else {
            return;
        };
        match method {
            "Network.requestWillBeSent" => {
                pending.insert(id.to_string());
            }
            "Network.loadingFinished" | "Network.loadingFailed" => {
                pending.remove(id);
            }
            _ => {}
        }
    }
}
