//! Public-destination policy for every HTTP request and HTTPS CONNECT, including
//! redirects. Connections use the validated IP, not a second DNS lookup.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use download_core::{validate_url, AppError, ErrorCode};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168 || (b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

fn denied(message: &str) -> AppError {
    AppError {
        code: ErrorCode::Validation,
        message: message.into(),
        retryable: false,
        retry_after_seconds: None,
    }
}

async fn connect_public(host: &str, port: u16) -> Result<TcpStream, AppError> {
    validate_url(&format!("https://{host}/"))?;
    let mut addresses: Vec<SocketAddr> = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| denied("Source DNS lookup timed out."))?
    .map_err(|_| denied("Source DNS lookup failed."))?
    .take(65)
    .collect();
    validate_addresses(&addresses)?;
    addresses.sort_by_key(|address| address.is_ipv6());
    for address in addresses {
        if let Ok(Ok(stream)) =
            tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address)).await
        {
            return Ok(stream);
        }
    }
    Err(denied("Could not connect to the public source."))
}

fn validate_addresses(addresses: &[SocketAddr]) -> Result<(), AppError> {
    if addresses.is_empty()
        || addresses.len() > 64
        || addresses.iter().any(|address| !public_ip(address.ip()))
    {
        return Err(denied(
            "The source resolved to a local or reserved network address.",
        ));
    }
    Ok(())
}

pub struct PublicProxy {
    pub url: String,
    cancel: CancellationToken,
    error: Arc<std::sync::Mutex<Option<AppError>>>,
}
impl Drop for PublicProxy {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl PublicProxy {
    pub fn error(&self) -> Option<AppError> {
        self.error.lock().expect("proxy error lock").clone()
    }
    pub async fn start() -> Result<Self, AppError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(AppError::storage)?;
        let address = listener.local_addr().map_err(AppError::storage)?;
        let secret = uuid::Uuid::new_v4().simple().to_string();
        let authorization = Arc::new(format!(
            "Basic {}",
            STANDARD.encode(format!("downpour:{secret}"))
        ));
        let cancel = CancellationToken::new();
        let shutdown = cancel.clone();
        let error = Arc::new(std::sync::Mutex::new(None));
        let last_error = Arc::clone(&error);
        tokio::spawn(async move {
            let permits = Arc::new(Semaphore::new(32));
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else { continue; };
                        let auth = Arc::clone(&authorization);
                        let errors = Arc::clone(&last_error);
                        tasks.spawn(async move {
                            let _permit = permit;
                            if let Ok(Err(error)) = tokio::time::timeout(Duration::from_secs(1800), serve(stream, &auth)).await {
                                *errors.lock().expect("proxy error lock") = Some(error);
                            }
                        });
                    }
                }
            }
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        });
        Ok(Self {
            url: format!("http://downpour:{secret}@{address}"),
            cancel,
            error,
        })
    }
}

async fn serve(mut client: TcpStream, authorization: &str) -> Result<(), AppError> {
    let mut header = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut byte = [0_u8; 1];
        while !header.ends_with(b"\r\n\r\n") {
            if header.len() >= 32768 || client.read(&mut byte).await? == 0 {
                return Err(std::io::Error::other("invalid proxy header"));
            }
            header.push(byte[0]);
        }
        Ok(())
    })
    .await
    .map_err(|_| denied("Proxy request timed out."))?
    .map_err(|_| denied("Invalid proxy request."))?;
    let header =
        std::str::from_utf8(&header).map_err(|_| denied("Invalid proxy header encoding."))?;
    let mut lines = header.split("\r\n");
    let parts: Vec<_> = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    if parts.len() != 3 || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return Err(denied("Invalid proxy request line."));
    }
    let headers: Vec<(&str, &str)> = lines
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split_once(':')
                .ok_or_else(|| denied("Invalid proxy header."))
        })
        .collect::<Result<_, _>>()?;
    if headers
        .iter()
        .filter(|(key, value)| {
            key.eq_ignore_ascii_case("proxy-authorization") && value.trim() == authorization
        })
        .count()
        != 1
    {
        client.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").await.map_err(AppError::storage)?;
        return Err(denied("Proxy authentication required."));
    }
    let connected = if parts[0] == "CONNECT" {
        let (host, port) = parts[1]
            .rsplit_once(':')
            .ok_or_else(|| denied("Invalid tunnel destination."))?;
        if port != "443" {
            return Err(denied("Only standard HTTPS tunnels are allowed."));
        }
        connect_public(host, 443).await
    } else {
        if !matches!(parts[0], "GET" | "HEAD" | "POST" | "OPTIONS") {
            return Err(denied("Unsupported source request method."));
        }
        let url = validate_url(parts[1])?;
        if url.scheme() != "http" {
            return Err(denied("HTTPS must use a TLS tunnel."));
        }
        let lengths: Vec<_> = headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .collect();
        let encodings: Vec<_> = headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("transfer-encoding"))
            .collect();
        if lengths.len() > 1
            || encodings.len() > 1
            || (!lengths.is_empty() && !encodings.is_empty())
            || lengths
                .iter()
                .any(|(_, v)| v.trim().parse::<u64>().is_err())
            || encodings
                .iter()
                .any(|(_, v)| !v.trim().eq_ignore_ascii_case("chunked"))
        {
            return Err(denied("Ambiguous proxy request framing."));
        }
        let host = url
            .host_str()
            .ok_or_else(|| denied("Missing source host."))?;
        match connect_public(host, 80).await {
            Ok(mut stream) => {
                let mut path = url.path().to_string();
                if let Some(query) = url.query() {
                    path.push('?');
                    path.push_str(query);
                }
                let mut forwarded = format!(
                    "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
                    parts[0], path, host
                );
                for (key, value) in &headers {
                    if ![
                        "proxy-authorization",
                        "proxy-connection",
                        "connection",
                        "host",
                    ]
                    .iter()
                    .any(|excluded| key.eq_ignore_ascii_case(excluded))
                    {
                        if !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                            || value.contains(['\r', '\n', '\0'])
                        {
                            return Err(denied("Invalid source header."));
                        }
                        forwarded.push_str(key);
                        forwarded.push(':');
                        forwarded.push_str(value);
                        forwarded.push_str("\r\n");
                    }
                }
                forwarded.push_str("\r\n");
                stream
                    .write_all(forwarded.as_bytes())
                    .await
                    .map_err(AppError::storage)?;
                Ok(stream)
            }
            Err(error) => Err(error),
        }
    };
    let mut remote = match connected {
        Ok(stream) => stream,
        Err(error) => {
            let _ = client
                .write_all(
                    b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                )
                .await;
            return Err(error);
        }
    };
    if parts[0] == "CONNECT" {
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .map_err(AppError::storage)?;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut remote).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_dns_answers_and_excessive_answers_are_rejected_as_a_unit() {
        let public: SocketAddr = "1.1.1.1:443".parse().unwrap();
        let private: SocketAddr = "10.0.0.1:443".parse().unwrap();
        assert!(validate_addresses(&[public]).is_ok());
        assert!(validate_addresses(&[public, private]).is_err());
        assert!(validate_addresses(&[]).is_err());
        assert!(validate_addresses(&vec![public; 65]).is_err());
    }
    #[test]
    fn rejects_special_networks_and_embedded_private_ipv6() {
        for ip in [
            "0.0.0.0",
            "10.2.3.4",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.31.0.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
            "::ffff:127.0.0.1",
            "64:ff9b::a00:1",
            "2001:db8::1",
            "2002:7f00:1::",
            "fc00::1",
            "fe80::1",
            "3fff::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.215.14",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(public_ip(ip.parse().unwrap()), "{ip}");
        }
    }
    #[tokio::test]
    async fn proxy_requires_auth_and_blocks_private_destinations() {
        let proxy = PublicProxy::start().await.unwrap();
        let url = url::Url::parse(&proxy.url).unwrap();
        let destination = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap());
        let mut client = TcpStream::connect(&destination).await.unwrap();
        client
            .write_all(b"CONNECT youtube.com:443 HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let mut bytes = [0_u8; 512];
        let count = client.read(&mut bytes).await.unwrap();
        assert!(String::from_utf8_lossy(&bytes[..count]).contains("407"));
        let auth = STANDARD.encode(format!("{}:{}", url.username(), url.password().unwrap()));
        let mut client = TcpStream::connect(destination).await.unwrap();
        client
            .write_all(
                format!(
                    "CONNECT 127.0.0.1:443 HTTP/1.1\r\nProxy-Authorization: Basic {auth}\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let count = client.read(&mut bytes).await.unwrap();
        assert!(String::from_utf8_lossy(&bytes[..count]).contains("403"));
    }
}
