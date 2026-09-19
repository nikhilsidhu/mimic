use base64::Engine;
use futures_util::{SinkExt, Stream, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

use super::{LcuError, Lockfile, Result};

/// One change pushed by the client over its WebSocket.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuEvent {
    /// The REST path whose resource changed, e.g. `/lol-gameflow/v1/gameflow-phase`.
    pub uri: String,
    /// `Create`, `Update` or `Delete`.
    pub event_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl LcuEvent {
    /// Parses a `[8, "OnJsonApiEvent", {...}]` frame. Anything else is not an event.
    fn parse(text: &str) -> Option<Self> {
        let (opcode, _topic, event): (u8, String, LcuEvent) = serde_json::from_str(text).ok()?;
        (opcode == 8).then_some(event)
    }
}

/// Connects to the client's WebSocket and streams every API event until it closes.
pub async fn subscribe(lockfile: &Lockfile) -> Result<impl Stream<Item = Result<LcuEvent>>> {
    let url = format!("wss://127.0.0.1:{}/", lockfile.port);
    let mut request = url.into_client_request()?;
    let credentials = base64::engine::general_purpose::STANDARD.encode(format!("riot:{}", lockfile.password));
    let header = HeaderValue::from_str(&format!("Basic {credentials}"))
        .map_err(|_| LcuError::MalformedLockfile("password is not a valid header value".into()))?;
    request.headers_mut().insert(AUTHORIZATION, header);

    // The LCU serves a self-signed certificate on loopback.
    let tls = native_tls::TlsConnector::builder().danger_accept_invalid_certs(true).build()?;
    let (mut socket, _) =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(Connector::NativeTls(tls))).await?;

    socket.send(Message::text(r#"[5, "OnJsonApiEvent"]"#)).await?;

    Ok(socket.filter_map(|message| async move {
        match message {
            Ok(Message::Text(text)) => LcuEvent::parse(&text).map(Ok),
            Ok(_) => None,
            Err(err) => Some(Err(err.into())),
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_frames() {
        let frame = r#"[8,"OnJsonApiEvent",{"data":"ChampSelect","eventType":"Update","uri":"/lol-gameflow/v1/gameflow-phase"}]"#;
        let event = LcuEvent::parse(frame).unwrap();
        assert_eq!(event.uri, "/lol-gameflow/v1/gameflow-phase");
        assert_eq!(event.event_type, "Update");
        assert_eq!(event.data, "ChampSelect");
    }

    #[test]
    fn delete_events_have_no_data() {
        let frame = r#"[8,"OnJsonApiEvent",{"eventType":"Delete","uri":"/lol-champ-select/v1/session"}]"#;
        assert_eq!(LcuEvent::parse(frame).unwrap().data, serde_json::Value::Null);
    }

    #[test]
    fn ignores_other_frames() {
        assert_eq!(LcuEvent::parse(""), None);
        assert_eq!(LcuEvent::parse(r#"[5,"OnJsonApiEvent"]"#), None);
        assert_eq!(LcuEvent::parse(r#"[3,"OnJsonApiEvent",{"eventType":"Update","uri":"/x"}]"#), None);
    }
}
