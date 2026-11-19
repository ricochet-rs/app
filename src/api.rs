//! Reads content from a ricochet server through its REST API, with an API key.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Why a request to a ricochet server failed, worded for the person using the app.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ApiError {
    #[error("Your sign-in expired or was revoked. Sign in again to continue.")]
    SignedOut,
    #[error("{0}")]
    Failed(String),
}

pub type Result<T> = std::result::Result<T, ApiError>;

/// A content item the API key's owner can see, as `/api/v0/user/items` lists it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub content_type: String,
    pub language: String,
    pub access_type: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub modified_at: i64,
    #[serde(default)]
    pub deployment_status: Option<String>,
    #[serde(default)]
    pub deployment_id: Option<String>,
    /// Your role on the item, or `None` in an admin's listing of every item.
    #[serde(default)]
    pub role: Option<String>,
}

impl Item {
    /// Whether ricochet runs the item on demand or on a schedule, rather than serving it.
    pub fn is_task(&self) -> bool {
        matches!(
            self.content_type.as_str(),
            "r" | "julia" | "python" | "rmd" | "quarto-r" | "quarto-jl" | "quarto-py"
        )
    }

    /// What kind of content the item is, such as "Shiny app" or "Quarto document".
    pub fn kind(&self) -> String {
        let kind = match self.content_type.as_str() {
            "r" | "julia" | "python" => "Script",
            "plumber" | "ambiorix" | "r-service" | "r-server" | "julia-service"
            | "python-service" | "fast-api" | "flask" | "serverless-r" => "API",
            "shiny" | "shiny-py" => "Shiny app",
            "rmd" => "R Markdown document",
            "rmd-shiny" => "Interactive R Markdown",
            "quarto-r" | "quarto-jl" | "quarto-py" => "Quarto document",
            "quarto-r-shiny" => "Interactive Quarto document",
            "jupyter" => "Jupyter notebook",
            "streamlit" => "Streamlit app",
            "dash" => "Dash app",
            "genie" => "Genie app",
            "python-app" => "Python app",
            other => return humanize(other),
        };
        kind.to_owned()
    }

    /// The language the item runs, as ricochet's web UI writes it.
    pub fn language_name(&self) -> &str {
        match self.language.as_str() {
            "r" => "R",
            "julia" => "Julia",
            "python" => "Python",
            other => other,
        }
    }

    /// Whether the caller may change the item's settings and environment variables.
    pub fn can_edit(&self) -> bool {
        matches!(
            self.role.as_deref(),
            Some("owner" | "admin" | "editor") | None
        )
    }

    /// Whether the caller may delete the item.
    pub fn can_delete(&self) -> bool {
        matches!(self.role.as_deref(), Some("owner" | "admin") | None)
    }
}

/// A type or status written in lowercase with dashes or underscores, as a capitalized phrase.
pub fn humanize(text: &str) -> String {
    let text = text.replace(['_', '-'], " ");
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// One deployment of an item, as `/api/v0/content/{id}/deployments` lists it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Deployment {
    pub id: String,
    #[serde(default)]
    pub content_id: String,
    pub deployed_at: i64,
    pub status: String,
    #[serde(default)]
    pub matched_ver: Option<String>,
    #[serde(default)]
    pub git_hash: Option<String>,
}

/// A running process of an app, as `/api/v0/content/{id}/instances` lists it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Instance {
    /// Also the id of the instance's runtime log.
    pub instance_id: String,
    #[serde(default)]
    pub connections: usize,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub peak_memory_bytes: u64,
}

/// A task run that has not finished, as `/api/v0/content/{id}/invocations/active` lists it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ActiveRun {
    /// Also the id of the run's log.
    pub id: String,
    #[serde(default)]
    pub started_at: String,
}

/// A Git credential deployments can clone with; ricochet never returns its secret.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// The item and deployment a deploy created or updated.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Deployed {
    pub id: String,
    pub deployment_id: String,
}

/// What to deploy from a Git repository.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitSource {
    pub url: String,
    pub branch: Option<String>,
    pub path: Option<String>,
    pub credential_id: Option<String>,
}

/// The changes to an item's settings that `PATCH .../settings` takes; `None` leaves a field alone.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct ContentSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// A request body and its media type.
struct Payload {
    content_type: String,
    bytes: Vec<u8>,
}

impl Payload {
    fn json(value: &impl serde::Serialize) -> Self {
        Self {
            content_type: "application/json".into(),
            bytes: serde_json::to_vec(value).unwrap_or_default(),
        }
    }
}

/// A `multipart/form-data` body built field by field.
struct Multipart {
    boundary: String,
    bytes: Vec<u8>,
}

impl Multipart {
    fn new() -> Self {
        let mut nonce = [0u8; 12];
        let _ = getrandom::fill(&mut nonce);
        let boundary: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        Self {
            boundary: format!("ricochet-app-{boundary}"),
            bytes: Vec::new(),
        }
    }

    fn field(mut self, name: &str, file: Option<(&str, &str)>, value: &[u8]) -> Self {
        let disposition = match file {
            Some((file_name, _)) => {
                format!("form-data; name=\"{name}\"; filename=\"{file_name}\"")
            }
            None => format!("form-data; name=\"{name}\""),
        };
        let head = format!(
            "--{}\r\nContent-Disposition: {disposition}\r\n{}\r\n",
            self.boundary,
            file.map(|(_, mime)| format!("Content-Type: {mime}\r\n"))
                .unwrap_or_default()
        );
        self.bytes.extend_from_slice(head.as_bytes());
        self.bytes.extend_from_slice(value);
        self.bytes.extend_from_slice(b"\r\n");
        self
    }

    fn finish(mut self) -> Payload {
        self.bytes
            .extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        Payload {
            content_type: format!("multipart/form-data; boundary={}", self.boundary),
            bytes: self.bytes,
        }
    }
}

const TIMEOUT: Duration = Duration::from_secs(20);

/// A signed-in connection to one ricochet server.
#[derive(Clone, Debug)]
pub struct Client {
    base: String,
    key: String,
}

impl Client {
    pub fn new(base: &str, key: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_owned(),
            key: key.to_owned(),
        }
    }

    pub fn items(&self) -> Result<Vec<Item>> {
        self.get("/api/v0/user/items")
    }

    /// The item's deployments, newest first.
    pub fn deployments(&self, item: &str) -> Result<Vec<Deployment>> {
        let mut deployments: Vec<Deployment> =
            self.get(&format!("/api/v0/content/{item}/deployments"))?;
        deployments.sort_by_key(|deployment| std::cmp::Reverse(deployment.deployed_at));
        Ok(deployments)
    }

    /// Starts a run of a task now, and returns its id, which is also its log's id.
    pub fn run(&self, item: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Started {
            id: String,
        }
        let started: Started = self.call(
            "POST",
            &format!("/api/v0/content/{item}/invoke"),
            Some(Payload::json(&serde_json::json!({}))),
        )?;
        Ok(started.id)
    }

    pub fn active_runs(&self, item: &str) -> Result<Vec<ActiveRun>> {
        self.get(&format!("/api/v0/content/{item}/invocations/active"))
    }

    pub fn stop_run(&self, item: &str, run: &str) -> Result<()> {
        self.call::<serde_json::Value>(
            "POST",
            &format!("/api/v0/content/{item}/invocations/{run}/stop"),
            None,
        )
        .map(drop)
    }

    pub fn instances(&self, item: &str) -> Result<Vec<Instance>> {
        self.get(&format!("/api/v0/content/{item}/instances"))
    }

    pub fn stop_instance(&self, item: &str, instance: &str) -> Result<()> {
        self.call::<serde_json::Value>(
            "POST",
            &format!("/api/v0/content/{item}/instances/{instance}/stop"),
            None,
        )
        .map(drop)
    }

    /// The ids of a deployment's build logs, one per architecture; waits while the deployment is pending.
    pub fn deployment_logs(&self, deployment: &str) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Logs {
            log_ids: Vec<String>,
        }
        let response = self.send(
            "GET",
            &format!("/api/v0/content/deployments/{deployment}/logs"),
            None,
            Some(Duration::from_secs(600)),
        )?;
        read::<Logs>(response, &self.base).map(|logs| logs.log_ids)
    }

    /// Reads a log line by line as ricochet writes it, until it ends or `stop` is set.
    pub fn follow_log(
        &self,
        log: &str,
        stop: &AtomicBool,
        mut line: impl FnMut(String),
    ) -> Result<()> {
        let mut response = self.send("GET", &format!("/api/v0/content/logs/{log}"), None, None)?;
        if !response.status().is_success() {
            return read::<serde_json::Value>(response, &self.base).map(drop);
        }
        let reader = BufReader::new(response.body_mut().as_reader());
        for text in reader.lines() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            match text {
                Ok(text) => line(text.trim_end_matches('\r').to_owned()),
                Err(error) => return Err(ApiError::Failed(error.to_string())),
            }
        }
        Ok(())
    }

    /// The item's `_ricochet.toml` as ricochet stores it now, without environment variables.
    pub fn ricochet_toml(&self, item: &str) -> Result<String> {
        let response = self.send(
            "GET",
            &format!("/api/v0/content/{item}/toml"),
            None,
            Some(TIMEOUT),
        )?;
        read_text(response, &self.base)
    }

    pub fn update_settings(&self, item: &str, content: &ContentSettings) -> Result<()> {
        let response = self.send(
            "PATCH",
            &format!("/api/v0/content/{item}/settings"),
            Some(Payload::json(&serde_json::json!({ "content": content }))),
            Some(TIMEOUT),
        )?;
        read_text(response, &self.base).map(drop)
    }

    /// Sets a task's cron schedule; ricochet has no way to clear one.
    pub fn set_schedule(&self, item: &str, cron: &str) -> Result<()> {
        self.call::<serde_json::Value>(
            "PATCH",
            &format!("/api/v0/content/{item}/schedule"),
            Some(Payload::json(&serde_json::json!({ "schedule": cron }))),
        )
        .map(drop)
    }

    /// The names of the item's environment variables; ricochet never returns their values.
    pub fn env_vars(&self, item: &str) -> Result<Vec<String>> {
        self.get(&format!("/api/v0/content/{item}/env-vars"))
    }

    /// Adds or replaces one environment variable, encrypted with the server's public key.
    pub fn set_env_var(&self, item: &str, name: &str, value: &str) -> Result<Vec<String>> {
        let pem = self.send("GET", "/api/v0/public-key", None, Some(TIMEOUT))?;
        let pem = read_text(pem, &self.base)?;
        let encrypted = crate::crypto::encrypt_pair(&pem, name, value)?;
        self.call(
            "PATCH",
            &format!("/api/v0/content/{item}/env-vars"),
            Some(Payload::json(&encrypted)),
        )
    }

    pub fn delete_env_var(&self, item: &str, name: &str) -> Result<Vec<String>> {
        self.call(
            "DELETE",
            &format!("/api/v0/content/{item}/env-vars/{}", encode(name)),
            None,
        )
    }

    pub fn delete_item(&self, item: &str) -> Result<()> {
        self.call::<serde_json::Value>("DELETE", &format!("/api/v0/content/{item}"), None)
            .map(drop)
    }

    /// Your Git credentials, without their secrets.
    pub fn credentials(&self) -> Result<Vec<Credential>> {
        self.get("/api/v0/user/credentials")
    }

    pub fn deploy_git(&self, source: &GitSource) -> Result<Deployed> {
        let repo = serde_json::json!({
            "url": source.url,
            "branch": source.branch,
            "path": source.path,
        });
        let mut form = Multipart::new().field("repo", None, repo.to_string().as_bytes());
        if let Some(credential) = &source.credential_id {
            form = form.field("credential_id", None, credential.as_bytes());
        }
        self.call("POST", "/api/v0/deploy/git", Some(form.finish()))
    }

    /// Uploads a bundle built by [`crate::bundle`], creating the item or deploying it again when `id` exists.
    pub fn upload(&self, bundle: &[u8], config: &str, id: Option<&str>) -> Result<Deployed> {
        let mut form = Multipart::new()
            .field(
                "bundle",
                Some(("bundle.tar.gz", "application/x-tar")),
                bundle,
            )
            .field(
                "config",
                Some(("_ricochet.toml", "application/toml")),
                config.as_bytes(),
            );
        if let Some(id) = id {
            form = form.field("id", None, id.as_bytes());
        }
        let response = self.send(
            "POST",
            "/api/v0/content/upload",
            Some(form.finish()),
            Some(Duration::from_secs(600)),
        )?;
        read(response, &self.base)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.call("GET", path, None)
    }

    fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        payload: Option<Payload>,
    ) -> Result<T> {
        let response = self.send(method, path, payload, Some(TIMEOUT))?;
        read(response, &self.base)
    }

    fn send(
        &self,
        method: &str,
        path: &str,
        payload: Option<Payload>,
        timeout: Option<Duration>,
    ) -> Result<ureq::http::Response<ureq::Body>> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(timeout)
            .build()
            .into();
        let request = ureq::http::Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.base))
            .header("Authorization", format!("Key {}", self.key));
        let result = match payload {
            Some(payload) => request
                .header("Content-Type", payload.content_type)
                .body(payload.bytes)
                .map(|request| agent.run(request)),
            None => request.body(()).map(|request| agent.run(request)),
        };
        result
            .map_err(|error| ApiError::Failed(error.to_string()))?
            .map_err(unreachable(&self.base))
    }
}

fn unreachable(base: &str) -> impl Fn(ureq::Error) -> ApiError + '_ {
    move |error| ApiError::Failed(format!("Couldn't reach {base}: {error}"))
}

fn read<T: DeserializeOwned>(response: ureq::http::Response<ureq::Body>, base: &str) -> Result<T> {
    let body = read_text(response, base)?;
    serde_json::from_str(&body).map_err(|error| {
        ApiError::Failed(format!(
            "{base} answered in a way this app doesn't understand: {error}"
        ))
    })
}

/// The body of a successful answer, or the error ricochet gave instead.
fn read_text(mut response: ureq::http::Response<ureq::Body>, base: &str) -> Result<String> {
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| body.trim().to_owned());
    match status {
        200..=299 => Ok(body),
        401 => Err(ApiError::SignedOut),
        403 if message.contains("Invalid API key") || message.contains("expired") => {
            Err(ApiError::SignedOut)
        }
        403 => Err(ApiError::Failed(format!(
            "You don't have permission to do that. {message}"
        ))),
        402 => Err(ApiError::Failed(format!(
            "This needs a ricochet license. {message}"
        ))),
        _ => Err(ApiError::Failed(format!(
            "{base} answered with status {status}. {message}"
        ))),
    }
}

/// The address of a ricochet server written by hand, with `https://` added when no scheme is given.
pub fn normalize_url(text: &str) -> Option<String> {
    let text = text.trim();
    let (scheme, rest) = text.split_once("://").unwrap_or(("https", text));
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let rest = rest.trim_end_matches('/');
    let host = rest.split(['/', ':', '?', '#']).next()?;
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']'));
    valid.then(|| format!("{scheme}://{rest}"))
}

/// A browser sign-in waiting for ricochet to send an API key back to this computer.
///
/// It is ricochet's loopback flow for the CLI: `/oauth/authorize` signs you in,
/// creates a key, and redirects to `http://127.0.0.1:{port}/callback/{nonce}?api_key=…`.
/// ricochet passes no `state` back, so the random nonce in the path is what ties the callback to this sign-in.
pub struct BrowserSignIn {
    listener: TcpListener,
    /// The callback path, which only this sign-in and the browser it opens know.
    path: String,
    /// The page to open in the browser.
    pub url: String,
    cancelled: Arc<AtomicBool>,
}

impl BrowserSignIn {
    pub fn start(base: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|error| std::io::Error::other(error.to_string()))?;
        let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = format!("/callback/{nonce}");
        let callback = format!("http://127.0.0.1:{port}{path}");
        let url = format!(
            "{}/oauth/authorize?redirect_uri={}",
            base.trim_end_matches('/'),
            encode(&callback)
        );
        Ok(Self {
            listener,
            path,
            url,
            cancelled: Arc::default(),
        })
    }

    /// A flag that stops [`Self::wait`] when set.
    pub fn canceller(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    /// Waits up to five minutes for the redirect, and returns the API key it carries.
    pub fn wait(self) -> Result<String> {
        self.listener
            .set_nonblocking(true)
            .map_err(|error| ApiError::Failed(error.to_string()))?;
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err(ApiError::Failed("Sign-in was cancelled.".into()));
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(answer) = answer_callback(stream, &self.path) {
                        return answer;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => return Err(ApiError::Failed(error.to_string())),
            }
        }
        Err(ApiError::Failed(
            "Sign-in took longer than five minutes. Try again.".into(),
        ))
    }
}

/// Reads one request, answers it, and returns the key or error it carried if it was the callback.
fn answer_callback(mut stream: TcpStream, callback: &str) -> Option<Result<String>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1)?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != callback {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return None;
    }
    let param = |name: &str| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| decode(value))
    };
    let (result, message) = match (param("api_key"), param("error")) {
        (Some(key), _) if !key.is_empty() => (
            Ok(key),
            "You're signed in. You can close this tab and go back to ricochet.",
        ),
        (_, Some(error)) => (
            Err(ApiError::Failed(format!("Sign-in failed: {error}"))),
            "Sign-in failed. Go back to ricochet to try again.",
        ),
        _ => (
            Err(ApiError::Failed(
                "ricochet didn't send an API key back. Paste one instead.".into(),
            )),
            "ricochet didn't send an API key back. Go back to ricochet and paste one instead.",
        ),
    };
    let page = format!(
        "<!doctype html><meta charset=utf-8><title>ricochet</title>\
         <body style=\"font-family:system-ui;margin:4rem auto;max-width:32rem\"><p>{message}</p>"
    );
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    Some(result)
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex =
            |offset: usize| (bytes.get(index + offset)?.to_ascii_lowercase() as char).to_digit(16);
        match bytes[index] {
            b'%' => match (hex(1), hex(2)) {
                (Some(high), Some(low)) => {
                    out.push((high * 16 + low) as u8);
                    index += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_gain_https_and_lose_trailing_slashes() {
        assert_eq!(
            normalize_url(" ricochet.example.com/ ").as_deref(),
            Some("https://ricochet.example.com")
        );
        assert_eq!(
            normalize_url("http://localhost:6188").as_deref(),
            Some("http://localhost:6188")
        );
        assert_eq!(normalize_url("ftp://example.com"), None);
        assert_eq!(normalize_url("https://"), None);
        assert_eq!(normalize_url("not a host"), None);
    }

    #[test]
    fn query_values_round_trip() {
        let callback = "http://localhost:1234/callback";
        assert_eq!(encode(callback), "http%3A%2F%2Flocalhost%3A1234%2Fcallback");
        assert_eq!(decode(&encode(callback)), callback);
        assert_eq!(decode("access+denied%21"), "access denied!");
        assert_eq!(decode("50%"), "50%");
    }

    #[test]
    fn the_callback_hands_back_the_key() {
        let sign_in = BrowserSignIn::start("http://localhost:6188/").unwrap();
        let port = sign_in.listener.local_addr().unwrap().port();
        assert!(sign_in.url.starts_with(&format!(
            "http://localhost:6188/oauth/authorize?redirect_uri=http%3A%2F%2F127.0.0.1%3A{port}%2Fcallback%2F"
        )));
        let request = format!("GET {}?api_key=rico_abc HTTP/1.1\r\n\r\n", sign_in.path);
        let browser = std::thread::spawn(move || {
            for forged in [
                "GET /favicon.ico HTTP/1.1\r\n\r\n",
                "GET /callback?api_key=rico_forged HTTP/1.1\r\n\r\n",
            ] {
                let mut stray = TcpStream::connect(("127.0.0.1", port)).unwrap();
                stray.write_all(forged.as_bytes()).unwrap();
            }
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
        });
        assert_eq!(sign_in.wait(), Ok("rico_abc".to_owned()));
        browser.join().unwrap();
    }

    #[test]
    fn items_parse_from_ricochet_json() {
        let json = r#"[{"id":"01J","name":"Sales","access_type":"private","content_type":"quarto-r",
            "slug":null,"summary":null,"thumbnail":null,"tags":["a"],"created_at":1,"modified_at":2,
            "language":"r","deployed_at":3,"deployment_id":"01K","deployment_status":"success","role":"owner"}]"#;
        let items: Vec<Item> = serde_json::from_str(json).unwrap();
        assert_eq!(items[0].kind(), "Quarto document");
        assert!(items[0].can_edit());
        assert!(items[0].is_task());
        assert_eq!(items[0].language_name(), "R");
    }
}
