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

pub async fn api(path: &str, body: serde_json::Value) -> bool {
    let base = std::env::var("RUSTDESK_ADM_API").unwrap_or_default();
    let secret = std::env::var("RUSTDESK_ADM_SECRET").unwrap_or_default();
    if !base.starts_with("https://") || secret.len() < 32 { return false; }
    let client = match reqwest::Client::builder().user_agent("RustDesk-ADM/1.0").redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2)).timeout(Duration::from_secs(4)).build() {
        Ok(client) => client, Err(_) => return false,
    };
    match client.post(format!("{}/api/internal/rustdesk/v1/{}", base.trim_end_matches('/'), path))
        .header("X-RustDesk-Server-Secret", secret).json(&body).send().await {
        Ok(response) if response.status().is_success() => response.json::<serde_json::Value>().await
            .ok().and_then(|value| value.get("allow").and_then(|v| v.as_bool())).unwrap_or(false),
        _ => false,
    }
}

pub async fn authorize(token: &str, destination: &str, identity: Option<serde_json::Value>) -> bool {
    if token.is_empty() || identity.is_none() { return false; }
    api("authorize-connection", serde_json::json!({"token": token, "destination_id": destination, "destination_identity": identity, "attempt_id": uuid::Uuid::new_v4().to_string()})).await
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
}
