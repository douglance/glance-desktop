//! Glance's existing client-upload protocol. Keep the share token (AES key)
//! local: only its hashed storage path and encrypted PNG leave the desktop.
//! Wire format matches modem-dev/agentpaste and @vercel/blob 2.3.0.
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use hkdf::Hkdf;
use image::{ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use rand::{RngCore, rngs::OsRng};
use reqwest::{
    Method,
    blocking::{Client, RequestBuilder, Response},
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Duration;

const GLANCE_URL: &str = "https://glance.sh";
const BLOB_URL: &str = "https://vercel.com/api/blob";
const MAX_UPLOAD_BYTES: usize = 15 * 1024 * 1024;
const EPOCH_MS: u64 = 1_735_689_600_000; // 2025-01-01 UTC
const MINUTE_MS: u64 = 60_000;
const HEADER: &[u8] = b"\x09image/png";
const ENVELOPE_OVERHEAD: usize = HEADER.len() + 12 + 16;

pub struct Share {
    pub url: String,
    pub expires_at: u64,
}

pub fn upload(image: RgbaImage) -> Result<Share, String> {
    UploadClient::new(GLANCE_URL, BLOB_URL)?.upload(image)
}

struct UploadClient {
    http: Client,
    origin: String,
    blob_origin: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Clock {
    now: u64,
    ttl_ms: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Proof {
    upload_proof: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadToken {
    client_token: String,
}
#[derive(Deserialize)]
struct Uploaded {
    pathname: String,
}

impl UploadClient {
    fn new(origin: &str, blob_origin: &str) -> Result<Self, String> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(90))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Pachiri/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(network_error)?;
        Ok(Self {
            http,
            origin: origin.trim_end_matches('/').into(),
            blob_origin: blob_origin.trim_end_matches('/').into(),
        })
    }

    /// Identify the desktop client on Glance's API, without forwarding these
    /// application headers to the third-party Blob service.
    fn glance_request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.origin))
            .header("X-Glance-Client", "pachiri")
            .header("X-Glance-Client-Version", env!("CARGO_PKG_VERSION"))
    }

    fn upload(&self, image: RgbaImage) -> Result<Share, String> {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("Couldn’t encode screenshot: {e}"))?;
        check_size(png.len())?;

        let clock: Clock = read_json(
            self.glance_request(Method::GET, "/api/issue")
                .send()
                .map_err(network_error)?,
        )?;
        let (token, expires_at) = issue_token(clock.now, clock.ttl_ms)?;
        let pathname = storage_path(&token);
        let proof: Proof = read_json(
            self.glance_request(Method::POST, "/api/issue")
                .json(&json!({ "pathname": pathname }))
                .send()
                .map_err(network_error)?,
        )?;
        let upload_token: UploadToken = read_json(
            self.glance_request(Method::POST, "/api/upload")
                .json(&json!({
                    "type": "blob.generate-client-token",
                    "payload": {
                        "pathname": pathname,
                        "clientPayload": proof.upload_proof,
                        "multipart": false,
                    }
                }))
                .send()
                .map_err(network_error)?,
        )?;
        let mut iv = [0u8; 12];
        OsRng
            .try_fill_bytes(&mut iv)
            .map_err(|_| "Couldn’t generate secure upload randomness.".to_string())?;
        let ciphertext = encrypt(&png, &token, &iv)?;
        let uploaded: Uploaded = read_json(
            self.http
                .put(format!("{}/", self.blob_origin))
                .query(&[("pathname", &pathname)])
                .bearer_auth(&upload_token.client_token)
                .header("x-api-version", "12")
                .header("x-vercel-blob-access", "private")
                .header("x-content-type", "application/octet-stream")
                .body(ciphertext)
                .send()
                .map_err(network_error)?,
        )?;
        if uploaded.pathname != pathname {
            return Err("Glance returned an unexpected upload path. Try again.".into());
        }
        Ok(Share {
            url: format!("{}/{token}.png", self.origin),
            expires_at,
        })
    }
}

fn check_size(png_size: usize) -> Result<(), String> {
    if png_size > MAX_UPLOAD_BYTES - ENVELOPE_OVERHEAD {
        return Err(
            "Glance uploads must be 15 MB or smaller. Resize the image and try again.".into(),
        );
    }
    Ok(())
}

fn network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "Glance upload timed out. Check your connection and try again.".into()
    } else {
        // Request URLs may contain tokens on future protocol extensions.
        format!("Couldn’t connect to Glance: {}", error.without_url())
    }
}

fn read_json<T: serde::de::DeserializeOwned>(response: Response) -> Result<T, String> {
    let status = response.status();
    if !status.is_success() {
        if status.as_u16() == 429 {
            return Err("Glance’s upload limit was reached. Try again later.".into());
        }
        let payload: serde_json::Value = response.json().unwrap_or_default();
        let detail = payload["error"].as_str().unwrap_or("Please try again.");
        return Err(format!("Glance upload failed ({status}): {detail}"));
    }
    response
        .json()
        .map_err(|_| "Glance returned an unexpected response. Try again.".into())
}

fn issue_token(now: u64, ttl_ms: u64) -> Result<(String, u64), String> {
    if !(5 * MINUTE_MS..=1440 * MINUTE_MS).contains(&ttl_ms) {
        return Err("Glance returned an invalid link lifetime.".into());
    }
    let expires_at = now
        .checked_add(ttl_ms)
        .and_then(|v| v.checked_add(MINUTE_MS - 1))
        .map(|v| v / MINUTE_MS * MINUTE_MS)
        .ok_or("Glance returned an invalid server clock.")?;
    let mut offset = expires_at
        .checked_sub(EPOCH_MS)
        .ok_or("Glance returned an invalid server clock.")?
        / MINUTE_MS;
    if offset >= 36u64.pow(5) {
        return Err("Glance’s link expiry exceeded token capacity.".into());
    }
    let mut prefix = [b'0'; 5];
    for c in prefix.iter_mut().rev() {
        *c = b"0123456789abcdefghijklmnopqrstuvwxyz"[(offset % 36) as usize];
        offset /= 36;
    }
    let mut token = String::from_utf8(prefix.to_vec()).unwrap();
    // Same rejection sampling as Glance: 248 is the largest multiple of 62
    // below 256, keeping every base62 character equally likely.
    while token.len() < 20 {
        let mut bytes = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| "Couldn’t generate secure upload randomness.".to_string())?;
        for byte in bytes {
            if byte < 248 {
                token.push(
                    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
                        [(byte % 62) as usize] as char,
                );
                if token.len() == 20 {
                    break;
                }
            }
        }
    }
    Ok((token, expires_at))
}

fn storage_path(token: &str) -> String {
    format!(
        "uploads/{}{:x}",
        &token[..5],
        Sha256::digest(token.as_bytes())
    )
}

fn encrypt(plaintext: &[u8], token: &str, iv: &[u8; 12]) -> Result<Vec<u8>, String> {
    let mut key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(b"glance.sh-image-encryption-v1"), token.as_bytes())
        .expand(b"aes-256-gcm", &mut key)
        .map_err(|_| "Couldn’t derive Glance encryption key.".to_string())?;
    let ciphertext = Aes256Gcm::new_from_slice(&key)
        .unwrap()
        .encrypt(
            Nonce::from_slice(iv),
            Payload {
                msg: plaintext,
                aad: HEADER,
            },
        )
        .map_err(|_| "Couldn’t encrypt screenshot for Glance.".to_string())?;
    Ok([HEADER, iv, &ciphertext].concat())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };

    struct Request {
        headers: String,
        body: Vec<u8>,
    }

    fn request(stream: &mut TcpStream) -> Request {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let end = loop {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
            if bytes.ends_with(b"\r\n\r\n") {
                break bytes.len();
            }
        };
        let headers = String::from_utf8(bytes).unwrap();
        let length = headers
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        let mut body = vec![0; length];
        stream.read_exact(&mut body).unwrap();
        assert!(end < 16_000);
        Request { headers, body }
    }

    fn respond(stream: &mut TcpStream, status: &str, body: &str) {
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    }

    #[test]
    fn desktop_upload_uses_existing_glance_protocol_without_sending_share_key() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            let mut path = String::new();
            for step in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let req = request(&mut stream);
                let headers = req.headers.to_ascii_lowercase();
                assert!(headers.contains(concat!(
                    "user-agent: pachiri/",
                    env!("CARGO_PKG_VERSION"),
                    "\r\n"
                )));
                if step < 3 {
                    assert!(headers.contains("x-glance-client: pachiri\r\n"));
                    assert!(headers.contains(concat!(
                        "x-glance-client-version: ",
                        env!("CARGO_PKG_VERSION"),
                        "\r\n"
                    )));
                } else {
                    assert!(!headers.contains("x-glance-client"));
                }
                let response = match step {
                    0 => {
                        assert!(req.headers.starts_with("GET /api/issue "));
                        json!({ "now": EPOCH_MS + 1, "ttlMs": 30 * MINUTE_MS })
                    }
                    1 => {
                        assert!(req.headers.starts_with("POST /api/issue "));
                        let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                        path = body["pathname"].as_str().unwrap().into();
                        assert_eq!(body, json!({ "pathname": path }));
                        json!({ "uploadProof": "signed-proof" })
                    }
                    2 => {
                        assert!(req.headers.starts_with("POST /api/upload "));
                        assert_eq!(
                            serde_json::from_slice::<serde_json::Value>(&req.body).unwrap(),
                            json!({
                                "type": "blob.generate-client-token",
                                "payload": { "pathname": path, "clientPayload": "signed-proof", "multipart": false }
                            })
                        );
                        json!({ "clientToken": "limited-client-token" })
                    }
                    _ => {
                        assert!(req.headers.starts_with("PUT /?pathname=uploads%2F"));
                        let headers = req.headers.to_ascii_lowercase();
                        assert!(headers.contains("authorization: bearer limited-client-token\r\n"));
                        assert!(headers.contains("x-vercel-blob-access: private\r\n"));
                        assert!(headers.contains("x-content-type: application/octet-stream\r\n"));
                        assert!(headers.contains("x-api-version: 12\r\n"));
                        json!({ "pathname": path })
                    }
                };
                respond(&mut stream, "200 OK", &response.to_string());
                requests.push(req);
            }
            (requests, path)
        });
        let image = RgbaImage::from_pixel(4, 3, image::Rgba([255, 56, 100, 128]));
        let share = UploadClient::new(&origin, &origin)
            .unwrap()
            .upload(image.clone())
            .unwrap();
        let (requests, path) = server.join().unwrap();
        let token = share
            .url
            .strip_prefix(&format!("{origin}/"))
            .unwrap()
            .strip_suffix(".png")
            .unwrap();
        assert_eq!(storage_path(token), path);
        assert_eq!(share.expires_at, EPOCH_MS + 31 * MINUTE_MS);
        for req in &requests {
            assert!(!req.headers.contains(token));
            assert!(!req.body.windows(token.len()).any(|w| w == token.as_bytes()));
        }
        let envelope = &requests[3].body;
        assert!(envelope.starts_with(HEADER));
        let mut key = [0; 32];
        Hkdf::<Sha256>::new(Some(b"glance.sh-image-encryption-v1"), token.as_bytes())
            .expand(b"aes-256-gcm", &mut key)
            .unwrap();
        let plaintext = Aes256Gcm::new_from_slice(&key)
            .unwrap()
            .decrypt(
                Nonce::from_slice(&envelope[HEADER.len()..HEADER.len() + 12]),
                Payload {
                    msg: &envelope[HEADER.len() + 12..],
                    aad: HEADER,
                },
            )
            .unwrap();
        assert_eq!(
            image::load_from_memory(&plaintext).unwrap().to_rgba8(),
            image
        );
    }

    #[test]
    fn rate_limit_and_unexpected_responses_fail_without_a_share_link() {
        for (status, body, expected) in [
            ("429 Too Many Requests", "{}", "upload limit"),
            ("200 OK", "not-json", "unexpected response"),
            (
                "503 Service Unavailable",
                r#"{"error":"Service unavailable"}"#,
                "Service unavailable",
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                request(&mut stream);
                respond(&mut stream, status, body);
            });
            let result = UploadClient::new(&origin, &origin)
                .unwrap()
                .upload(RgbaImage::new(1, 1));
            assert!(result.err().unwrap().contains(expected));
            server.join().unwrap();
        }
    }

    #[test]
    fn encryption_and_storage_path_match_glance_node_crypto() {
        // Generated independently with Node's hkdfSync/createCipheriv using
        // agentpaste/lib/encryption.ts constants. This guards cross-app parity.
        let token = "0d9tgI6JWDbeY11euEiw";
        let iv = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let encrypted = encrypt(b"PNG parity fixture", token, &iv).unwrap();
        let hex: String = encrypted.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "09696d6167652f706e67000102030405060708090a0bbad0b5419c4c38638faea6070ab1d872d4dd7161ea28270f1f1573e38d32267cb0e3"
        );
        assert_eq!(
            storage_path(token),
            "uploads/0d9tgff7a042943249930c68915f187c106fbf3ff46456b2b70121e14268e10ec6f01"
        );
    }

    #[test]
    fn tokens_use_server_clock_and_minute_aligned_expiry() {
        let (token, expiry) = issue_token(EPOCH_MS + 1, 30 * MINUTE_MS).unwrap();
        assert_eq!(&token[..5], "0000v");
        assert_eq!(token.len(), 20);
        assert!(token.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert_eq!(expiry, EPOCH_MS + 31 * MINUTE_MS);
        assert_eq!(
            issue_token(EPOCH_MS, 30 * MINUTE_MS).unwrap().1,
            EPOCH_MS + 30 * MINUTE_MS
        );
        assert_ne!(issue_token(EPOCH_MS, 30 * MINUTE_MS).unwrap().0, token);
        assert!(issue_token(0, 30 * MINUTE_MS).is_err());
        assert!(issue_token(u64::MAX, 30 * MINUTE_MS).is_err());
        assert!(issue_token(EPOCH_MS, 0).is_err());
        assert!(check_size(MAX_UPLOAD_BYTES - ENVELOPE_OVERHEAD).is_ok());
        assert!(check_size(MAX_UPLOAD_BYTES - ENVELOPE_OVERHEAD + 1).is_err());
    }

    #[test]
    #[ignore = "Uploads a generated test image to production Glance; run explicitly"]
    fn live_upload_round_trips_through_glance() {
        let image = RgbaImage::from_pixel(3, 2, image::Rgba([38, 182, 144, 255]));
        let share = upload(image.clone()).unwrap();
        let client = Client::new();
        let bytes = client
            .get(&share.url)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .bytes()
            .unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().to_rgba8(), image);
        assert!(share.url.starts_with("https://glance.sh/"));
        assert!(share.url.ends_with(".png"));
    }
}
