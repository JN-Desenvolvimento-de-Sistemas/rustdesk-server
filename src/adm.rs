use hbb_common::{
    bytes::{Bytes, BytesMut}, bytes_codec::BytesCodec,
    futures_util::{SinkExt, StreamExt}, protobuf::Message,
    rendezvous_proto::{rendezvous_message, KeyExchange, RendezvousMessage},
    tcp::Encrypt, tokio::{net::TcpStream, time::{timeout, Duration}},
    tokio_util::codec::{Decoder, Encoder, Framed}, ResultType,
};
use sodiumoxide::crypto::{box_, sign};
use std::io;

pub struct SecureCodec { framing: BytesCodec, encryption: Option<Encrypt> }
impl SecureCodec {
    fn new() -> Self {
        let mut framing = BytesCodec::new();
        framing.set_max_packet_length(1024 * 1024);
        Self { framing, encryption: None }
    }
}
impl Decoder for SecureCodec {
    type Item = BytesMut;
    type Error = io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        let mut decoded = self.framing.decode(src)?;
        if let (Some(bytes), Some(encryption)) = (&mut decoded, &mut self.encryption) {
            if bytes.len() <= 1 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid encrypted packet")); }
            encryption.dec(bytes)?;
        }
        Ok(decoded)
    }
}
impl Encoder<Bytes> for SecureCodec {
    type Error = io::Error;
    fn encode(&mut self, data: Bytes, dst: &mut BytesMut) -> io::Result<()> {
        let data = match &mut self.encryption {
            Some(encryption) => Bytes::from(encryption.enc(&data)), None => data,
        };
        self.framing.encode(data, dst)
    }
}

pub async fn handshake(stream: TcpStream, key: &sign::SecretKey) -> ResultType<(Framed<TcpStream, SecureCodec>, Option<BytesMut>, bool)> {
    let mut framed = Framed::new(stream, SecureCodec::new());
    // Receiver messages arrive first; an authenticated initiator waits for KeyExchange.
    if let Ok(first) = timeout(Duration::from_millis(100), framed.next()).await {
        return match first { Some(Ok(bytes)) => Ok((framed, Some(bytes), false)), _ => Err(hbb_common::anyhow::anyhow!("Closed before handshake")) };
    }
    let (public, secret) = box_::gen_keypair();
    let mut hello = RendezvousMessage::new();
    hello.set_key_exchange(KeyExchange { keys: vec![sign::sign(&public.0, key).into()], ..Default::default() });
    framed.send(hello.write_to_bytes()?.into()).await?;
    let bytes = timeout(Duration::from_secs(5), framed.next()).await?
        .ok_or_else(|| hbb_common::anyhow::anyhow!("Closed during handshake"))??;
    match RendezvousMessage::parse_from_bytes(&bytes)?.union {
        Some(rendezvous_message::Union::KeyExchange(exchange)) if exchange.keys.len() == 2 => {
            let symmetric = Encrypt::decode(&exchange.keys[1], &exchange.keys[0], &secret)?;
            framed.codec_mut().encryption = Some(Encrypt::new(symmetric));
            Ok((framed, None, true))
        }
        Some(rendezvous_message::Union::KeyExchange(_)) => Err(hbb_common::anyhow::anyhow!("Invalid key exchange")),
        _ => Ok((framed, Some(bytes), false)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiFailure {
    Configuration,
    Client,
    Timeout,
    Transport,
    HttpStatus(u16),
    InvalidJson,
    InvalidDecision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authorization {
    Allowed,
    Denied,
    Unavailable(ApiFailure),
}

impl Authorization {
    pub fn message(self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::Denied => Some("Você não possui permissão para acessar esse dispositivo. Entre em contato com o administrador"),
            Self::Unavailable(_) => Some("Serviço temporariamente indisponível. Tente novamente em instantes"),
        }
    }

    pub fn punch_response(self) -> Option<RendezvousMessage> {
        let mut response = RendezvousMessage::new();
        response.set_punch_hole_response(hbb_common::rendezvous_proto::PunchHoleResponse {
            other_failure: self.message()?.into(),
            ..Default::default()
        });
        Some(response)
    }

    pub fn relay_response(self) -> Option<RendezvousMessage> {
        let mut response = RendezvousMessage::new();
        response.set_relay_response(hbb_common::rendezvous_proto::RelayResponse {
            refuse_reason: self.message()?.into(),
            ..Default::default()
        });
        Some(response)
    }
}

fn classify_response(result: Result<serde_json::Value, ApiFailure>) -> Authorization {
    match result {
        Ok(value) => match value.get("allow").and_then(|v| v.as_bool()) {
            Some(true) => Authorization::Allowed,
            Some(false) => Authorization::Denied,
            None => Authorization::Unavailable(ApiFailure::InvalidDecision),
        },
        Err(reason) => Authorization::Unavailable(reason),
    }
}

fn configured_base(base: &str, secret: &str) -> Result<reqwest::Url, ApiFailure> {
    let url = reqwest::Url::parse(base).map_err(|_| ApiFailure::Configuration)?;
    if url.scheme() != "https" || url.host_str().is_none() || secret.len() < 32 {
        return Err(ApiFailure::Configuration);
    }
    Ok(url)
}

async fn send_request(client: &reqwest::Client, url: &str, secret: &str, body: serde_json::Value) -> Result<serde_json::Value, ApiFailure> {
    let response = client.post(url).header("X-RustDesk-Server-Secret", secret).json(&body).send().await
        .map_err(|error| if error.is_timeout() { ApiFailure::Timeout } else { ApiFailure::Transport })?;
    if !response.status().is_success() {
        return Err(ApiFailure::HttpStatus(response.status().as_u16()));
    }
    response.json().await.map_err(|error| if error.is_timeout() { ApiFailure::Timeout } else if error.is_decode() { ApiFailure::InvalidJson } else { ApiFailure::Transport })
}

async fn request_api(path: &str, body: serde_json::Value) -> Result<serde_json::Value, ApiFailure> {
    let base = std::env::var("RUSTDESK_ADM_API").unwrap_or_default();
    let secret = std::env::var("RUSTDESK_ADM_SECRET").unwrap_or_default();
    let result = async {
        configured_base(&base, &secret)?;
        let client = reqwest::Client::builder().user_agent("RustDesk-ADM/1.0").redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2)).timeout(Duration::from_secs(4)).build().map_err(|_| ApiFailure::Client)?;
        send_request(&client, &format!("{}/api/internal/rustdesk/v1/{}", base.trim_end_matches('/'), path), &secret, body).await
    }.await;
    if let Err(reason) = result {
        // Log categories only: request bodies, tokens, URLs and secrets must not appear in logs.
        hbb_common::log::warn!("RustDesk ADM API unavailable operation={} reason={:?}", path.split('/').next().unwrap_or("unknown"), reason);
    }
    result
}

pub async fn api(path: &str, body: serde_json::Value) -> bool {
    request_api(path, body).await.ok().and_then(|value| value.get("allow").and_then(|v| v.as_bool())).unwrap_or(false)
}

pub async fn connection_authorization(body: serde_json::Value) -> Authorization {
    let decision = classify_response(request_api("authorize-connection", body).await);
    if decision == Authorization::Unavailable(ApiFailure::InvalidDecision) {
        hbb_common::log::warn!("RustDesk ADM API unavailable operation=authorize-connection reason=InvalidDecision");
    }
    decision
}

pub async fn authorize(token: &str, destination: &str, identity: Option<serde_json::Value>) -> Authorization {
    if token.is_empty() || identity.is_none() { return Authorization::Denied; }
    connection_authorization(serde_json::json!({"token": token, "destination_id": destination, "destination_identity": identity, "attempt_id": uuid::Uuid::new_v4().to_string()})).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::{tcp::FramedStream, tokio::net::TcpListener};
    use sodiumoxide::crypto::secretbox;

    #[hbb_common::tokio::test(crate = "hbb_common::tokio")]
    async fn official_key_exchange_encrypts_both_directions() {
        sodiumoxide::init().unwrap();
        let (signing_public, signing_secret) = sign::gen_keypair();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = hbb_common::tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (mut framed, first, secured) = handshake(socket, &signing_secret).await.unwrap();
            assert!(secured && first.is_none());
            let received = framed.next().await.unwrap().unwrap();
            let request = RendezvousMessage::parse_from_bytes(&received).unwrap();
            assert_eq!(request.punch_hole_request().token, "test-token");
            framed.send(received.freeze()).await.unwrap();
            for decision in [Authorization::Denied, Authorization::Unavailable(ApiFailure::Timeout)] {
                let received = framed.next().await.unwrap().unwrap();
                assert!(RendezvousMessage::parse_from_bytes(&received).unwrap().has_punch_hole_request());
                framed.send(decision.punch_response().unwrap().write_to_bytes().unwrap().into()).await.unwrap();
            }
        });
        let mut client = FramedStream::from(TcpStream::connect(address).await.unwrap(), address);
        let hello = client.next_timeout(5000).await.unwrap().unwrap();
        let hello = RendezvousMessage::parse_from_bytes(&hello).unwrap();
        let public = sign::verify(&hello.key_exchange().keys[0], &signing_public).unwrap();
        let public = box_::PublicKey::from_slice(&public).unwrap();
        let (client_public, client_secret) = box_::gen_keypair();
        let symmetric = secretbox::gen_key();
        let encrypted = box_::seal(&symmetric.0, &box_::Nonce([0; box_::NONCEBYTES]), &public, &client_secret);
        let mut exchange = RendezvousMessage::new();
        exchange.set_key_exchange(KeyExchange { keys: vec![client_public.0.to_vec().into(), encrypted.into()], ..Default::default() });
        client.send(&exchange).await.unwrap();
        client.set_key(symmetric);
        let mut request = RendezvousMessage::new();
        request.set_punch_hole_request(hbb_common::rendezvous_proto::PunchHoleRequest { id: "123456789".into(), token: "test-token".into(), ..Default::default() });
        client.send(&request).await.unwrap();
        let echoed = client.next_timeout(5000).await.unwrap().unwrap();
        assert_eq!(RendezvousMessage::parse_from_bytes(&echoed).unwrap().punch_hole_request().token, "test-token");
        client.send(&request).await.unwrap();
        let denied = client.next_timeout(5000).await.unwrap().unwrap();
        let denied = RendezvousMessage::parse_from_bytes(&denied).unwrap();
        assert!(denied.has_punch_hole_response());
        assert!(denied.punch_hole_response().socket_addr.is_empty());
        assert_eq!(denied.punch_hole_response().other_failure,
            "Você não possui permissão para acessar esse dispositivo. Entre em contato com o administrador");
        client.send(&request).await.unwrap();
        let unavailable = client.next_timeout(5000).await.unwrap().unwrap();
        let unavailable = RendezvousMessage::parse_from_bytes(&unavailable).unwrap();
        assert!(unavailable.punch_hole_response().socket_addr.is_empty());
        assert_eq!(unavailable.punch_hole_response().other_failure,
            "Serviço temporariamente indisponível. Tente novamente em instantes");
        server.await.unwrap();
    }

    #[hbb_common::tokio::test(crate = "hbb_common::tokio")]
    async fn legacy_receiver_message_is_not_authenticated() {
        let (_, signing_secret) = sign::gen_keypair();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut client = FramedStream::from(TcpStream::connect(address).await.unwrap(), address);
        let request = RendezvousMessage::new();
        client.send(&request).await.unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let (_, first, secured) = handshake(socket, &signing_secret).await.unwrap();
        assert!(!secured && first.is_some());
    }

    async fn mock_decision(status: u16, body: &'static str, delay_ms: u64, timeout_ms: u64) -> Authorization {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/authorize", axum::routing::post(move || async move {
            hbb_common::tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            (axum::http::StatusCode::from_u16(status).unwrap(), body)
        }));
        let server = hbb_common::tokio::spawn(axum::Server::from_tcp(listener).unwrap().serve(app.into_make_service()));
        let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(timeout_ms)).build().unwrap();
        let result = send_request(&client, &format!("http://{addr}/authorize"), "test-secret", serde_json::json!({})).await;
        server.abort();
        classify_response(result)
    }

    #[hbb_common::tokio::test(crate = "hbb_common::tokio")]
    async fn api_only_explicit_boolean_decisions_authorize_or_deny() {
        assert_eq!(mock_decision(200, r#"{"allow":true}"#, 0, 2000).await, Authorization::Allowed);
        assert_eq!(mock_decision(200, r#"{"allow":false}"#, 0, 2000).await, Authorization::Denied);
        for body in ["{}", r#"{"allow":"true"}"#, r#"{"allow":null}"#] {
            assert_eq!(mock_decision(200, body, 0, 2000).await, Authorization::Unavailable(ApiFailure::InvalidDecision));
        }
        assert_eq!(mock_decision(200, "not JSON", 0, 2000).await, Authorization::Unavailable(ApiFailure::InvalidJson));
    }

    #[hbb_common::tokio::test(crate = "hbb_common::tokio")]
    async fn api_http_errors_redirects_and_timeouts_are_unavailable() {
        for status in [302, 401, 403, 422, 429, 500, 503] {
            assert_eq!(mock_decision(status, r#"{"allow":false}"#, 0, 2000).await,
                Authorization::Unavailable(ApiFailure::HttpStatus(status)));
        }
        assert_eq!(mock_decision(200, r#"{"allow":true}"#, 300, 50).await, Authorization::Unavailable(ApiFailure::Timeout));
    }

    #[hbb_common::tokio::test(crate = "hbb_common::tokio")]
    async fn api_transport_failure_never_authorizes() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Bound but not listening after drop: local connection failure, never production.
        drop(listener);
        let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(1)).build().unwrap();
        let result = send_request(&client, &format!("http://{addr}"), "test-secret", serde_json::json!({})).await;
        assert_eq!(classify_response(result), Authorization::Unavailable(ApiFailure::Transport));
    }

    #[test]
    fn invalid_configuration_and_failure_messages_remain_fail_closed() {
        for (url, secret) in [("http://api.example.org", "a".repeat(32)), ("invalid", "a".repeat(32)), ("https://api.example.org", "short".into())] {
            assert_eq!(configured_base(url, &secret), Err(ApiFailure::Configuration));
        }
        assert!(configured_base("https://api.example.org", &"a".repeat(32)).is_ok());
        assert!(Authorization::Allowed.punch_response().is_none());
        assert!(Authorization::Allowed.relay_response().is_none());
        for decision in [Authorization::Denied, Authorization::Unavailable(ApiFailure::Configuration), Authorization::Unavailable(ApiFailure::Timeout)] {
            let punch = decision.punch_response().unwrap();
            assert!(punch.punch_hole_response().socket_addr.is_empty());
            assert_eq!(punch.punch_hole_response().other_failure, decision.message().unwrap());
            let relay = decision.relay_response().unwrap();
            assert!(relay.relay_response().socket_addr.is_empty());
            assert!(relay.relay_response().relay_server.is_empty());
            assert_eq!(relay.relay_response().refuse_reason, decision.message().unwrap());
        }
    }

}
