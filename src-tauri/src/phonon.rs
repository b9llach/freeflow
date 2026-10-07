//! Managed local Phonon-2 speech engine.
//!
//! Phonon-2 ships as a Python package (`fermion-research`) that exposes an
//! OpenAI-compatible speech server. This module makes that invisible to the
//! user: it installs a private Python environment under the app data folder
//! (using a pinned, checksum-verified `uv`, so no system Python is needed),
//! runs `fermion serve` as a hidden child process on a private port, and
//! tears the whole process tree down when Freeflow exits.
//!
//! Nothing here depends on Tauri, so the install and run cycle can be tested
//! on its own.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};

pub const MODEL: &str = "phonon-2";
/// Private port for the managed server, chosen to stay clear of the usual
/// 8000 that a hand-started `fermion serve` uses.
pub const DEFAULT_PORT: u16 = 18_765;

const UV_VERSION: &str = "0.12.23";

/// Pinned python-build-standalone CPython 3.12.15 (the same builds uv would
/// fetch). Freeflow downloads and unpacks it itself instead of using
/// `uv python install`, because uv links the install through a directory
/// junction and Windows rejects that on some machines (os error 448).
const PYTHON_RELEASE: &str = "20261003";

fn python_asset() -> Result<(&'static str, &'static str)> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok(("x86_64-pc-windows-msvc", "6fba7f2ae506facf41d457ea8293c7497910a675c69a4e954875169410a50402")),
        ("windows", "aarch64") => Ok(("aarch64-pc-windows-msvc", "b39c6c3aac8a88ae42fd4f2ca5a832d1e78b55506f33f0498de4dd6fc38b5162")),
        ("macos", "aarch64") => Ok(("aarch64-apple-darwin", "ad8d0c637c0a36b967b310e2c07254f4d2ca8cabaa7699e55ed6290aceb481a2")),
        ("macos", "x86_64") => Ok(("x86_64-apple-darwin", "562c30864ece2cb1d3e0ad66a1acd498611a47e5a10ce81b99158bef1ccbd355")),
        (os, arch) => bail!("no bundled Python for {os} {arch}"),
    }
}
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub stage: &'static str,
    pub message: String,
    pub detail: Option<String>,
    pub percent: Option<f32>,
}

pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub port: u16,
    pub log_path: String,
}

fn emit(
    p: &Option<ProgressFn>,
    stage: &'static str,
    message: impl Into<String>,
    detail: Option<String>,
    percent: Option<f32>,
) {
    if let Some(f) = p {
        f(Progress {
            stage,
            message: message.into(),
            detail,
            percent,
        });
    }
}

/// (asset file name, is_zip) for the uv release that matches this machine.
fn uv_asset() -> Result<(&'static str, bool)> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok(("uv-x86_64-pc-windows-msvc.zip", true)),
        ("windows", "aarch64") => Ok(("uv-aarch64-pc-windows-msvc.zip", true)),
        ("macos", "aarch64") => Ok(("uv-aarch64-apple-darwin.tar.gz", false)),
        ("macos", "x86_64") => Ok(("uv-x86_64-apple-darwin.tar.gz", false)),
        (os, arch) => bail!("automatic Phonon-2 setup is not supported on {os} {arch}"),
    }
}

pub fn supported() -> bool {
    uv_asset().is_ok()
}

fn packages() -> Vec<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        vec![
            "fermion-research",
            "mlx",
            "mlx-audio",
            "mlx-lm",
            "soundfile",
            "scipy",
            "zstandard",
        ]
    } else {
        vec![
            "fermion-research",
            "torch",
            "safetensors",
            "soundfile",
            "scipy",
            "zstandard",
        ]
    }
}

#[cfg(windows)]
mod job {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// A Windows Job Object that kills every process inside it when the last
    /// handle closes, including when Freeflow itself crashes. The Python
    /// launcher stubs spawn the real interpreter as a child, so killing only
    /// the top process would orphan a 1 GB server.
    pub struct Job(HANDLE);
    unsafe impl Send for Job {}

    impl Job {
        pub fn new() -> Option<Job> {
            unsafe {
                let h = CreateJobObjectW(None, PCWSTR::null()).ok()?;
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .is_ok();
                if !ok {
                    let _ = CloseHandle(h);
                    return None;
                }
                Some(Job(h))
            }
        }

        pub fn assign(&self, child: &std::process::Child) -> bool {
            use std::os::windows::io::AsRawHandle;
            unsafe { AssignProcessToJobObject(self.0, HANDLE(child.as_raw_handle() as _)).is_ok() }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub struct Runtime {
    base: PathBuf,
    port: u16,
    client: reqwest::Client,
    child: Mutex<Option<std::process::Child>>,
    #[cfg(windows)]
    job: Mutex<Option<job::Job>>,
    start_lock: tokio::sync::Mutex<()>,
}

static GLOBAL: OnceLock<Arc<Runtime>> = OnceLock::new();

impl Runtime {
    pub fn new(base: PathBuf, port: u16) -> Runtime {
        Runtime {
            base,
            port,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(1))
                .timeout(Duration::from_secs(3))
                .build()
                .expect("reqwest client"),
            child: Mutex::new(None),
            #[cfg(windows)]
            job: Mutex::new(None),
            start_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Process-wide instance, so the exit hook can stop the server.
    pub fn init(base: PathBuf, port: u16) -> Arc<Runtime> {
        GLOBAL
            .get_or_init(|| Arc::new(Runtime::new(base, port)))
            .clone()
    }

    pub fn global() -> Option<Arc<Runtime>> {
        GLOBAL.get().cloned()
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    fn health_url(&self) -> String {
        format!("http://127.0.0.1:{}/health", self.port)
    }

    fn uv_exe(&self) -> PathBuf {
        self.base
            .join("bin")
            .join(if cfg!(windows) { "uv.exe" } else { "uv" })
    }

    fn venv_dir(&self) -> PathBuf {
        self.base.join("venv")
    }

    fn venv_python(&self) -> PathBuf {
        if cfg!(windows) {
            self.venv_dir().join("Scripts").join("python.exe")
        } else {
            self.venv_dir().join("bin").join("python")
        }
    }

    fn fermion_exe(&self) -> PathBuf {
        if cfg!(windows) {
            self.venv_dir().join("Scripts").join("fermion.exe")
        } else {
            self.venv_dir().join("bin").join("fermion")
        }
    }

    fn marker(&self) -> PathBuf {
        self.base.join("installed.json")
    }

    fn packages_marker(&self) -> PathBuf {
        self.base.join("packages.ok")
    }

    pub fn log_path(&self) -> PathBuf {
        self.base.join("server.log")
    }

    pub fn is_installed(&self) -> bool {
        self.marker().is_file() && self.fermion_exe().is_file()
    }

    pub async fn status(&self) -> Status {
        Status {
            supported: supported(),
            installed: self.is_installed(),
            running: self.healthy().await,
            port: self.port,
            log_path: self.log_path().to_string_lossy().to_string(),
        }
    }

    /// True when something on our port answers `/health` as a speech server.
    pub async fn healthy(&self) -> bool {
        let Ok(resp) = self.client.get(self.health_url()).send().await else {
            return false;
        };
        if !resp.status().is_success() {
            return false;
        }
        match resp.json::<serde_json::Value>().await {
            Ok(v) => v["kind"] == "speech",
            Err(_) => false,
        }
    }

    // ---- install ----------------------------------------------------------

    /// Idempotent: finished steps are skipped, so a retry after a failure
    /// picks up where it stopped instead of re-downloading everything.
    pub async fn install(&self, progress: ProgressFn) -> Result<()> {
        let p = Some(progress);
        uv_asset()?;
        std::fs::create_dir_all(&self.base)?;
        self.ensure_uv(&p).await?;
        self.ensure_venv(&p).await?;
        self.ensure_packages(&p).await?;
        emit(
            &p,
            "server",
            "Starting the speech engine",
            Some("The first start downloads the model (about 160 MB) and prepares the runtime".into()),
            None,
        );
        self.start(&p, Duration::from_secs(900)).await?;
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        std::fs::write(
            self.marker(),
            serde_json::json!({ "uv": UV_VERSION, "model": MODEL, "installed_at_unix": secs })
                .to_string(),
        )?;
        emit(&p, "done", "Phonon-2 is ready", None, Some(100.0));
        Ok(())
    }

    async fn ensure_uv(&self, p: &Option<ProgressFn>) -> Result<()> {
        if self.uv_exe().is_file() {
            return Ok(());
        }
        let (asset, is_zip) = uv_asset()?;
        let url =
            format!("https://github.com/astral-sh/uv/releases/download/{UV_VERSION}/{asset}");
        let bin = self.base.join("bin");
        std::fs::create_dir_all(&bin)?;
        let archive = bin.join(asset);

        emit(p, "uv", "Downloading the Python package tool", None, Some(0.0));
        let expected = fetch_sha256(&format!("{url}.sha256")).await?;
        let p2 = p.clone();
        download_verified(&url, &archive, &expected, move |pct| {
            emit(
                &p2,
                "uv",
                "Downloading the Python package tool",
                None,
                Some(pct),
            )
        })
        .await?;

        let extract_dir = bin.join("extract");
        let _ = std::fs::remove_dir_all(&extract_dir);
        std::fs::create_dir_all(&extract_dir)?;

        extract_archive(&archive, &extract_dir, is_zip).await?;

        let name = if cfg!(windows) { "uv.exe" } else { "uv" };
        let found = find_file(&extract_dir, name)
            .ok_or_else(|| anyhow!("{name} not found inside {asset}"))?;
        std::fs::copy(&found, self.uv_exe())?;
        let _ = std::fs::remove_dir_all(&extract_dir);
        let _ = std::fs::remove_file(&archive);
        Ok(())
    }

    fn base_python(&self) -> PathBuf {
        let root = self.base.join("pyroot").join("python");
        if cfg!(windows) {
            root.join("python.exe")
        } else {
            root.join("bin").join("python3")
        }
    }

    async fn ensure_python(&self, p: &Option<ProgressFn>) -> Result<()> {
        if self.base_python().is_file() {
            return Ok(());
        }
        let (triple, sha) = python_asset()?;
        let file = format!("cpython-3.12.15+{PYTHON_RELEASE}-{triple}-install_only_stripped.tar.gz");
        let url = format!(
            "https://github.com/astral-sh/python-build-standalone/releases/download/{PYTHON_RELEASE}/{}",
            file.replace('+', "%2B")
        );
        let dl = self.base.join("pydl");
        std::fs::create_dir_all(&dl)?;
        let archive = dl.join("python.tar.gz");
        let msg = "Downloading a private Python";
        emit(p, "python", msg, None, Some(0.0));
        let p2 = p.clone();
        download_verified(&url, &archive, sha, move |pct| {
            emit(&p2, "python", msg, None, Some(pct))
        })
        .await?;

        emit(p, "python", "Unpacking Python", None, None);
        let root = self.base.join("pyroot");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        if let Err(e) = extract_archive(&archive, &root, false).await {
            let _ = std::fs::remove_dir_all(&root);
            return Err(e);
        }
        let _ = std::fs::remove_dir_all(&dl);
        if !self.base_python().is_file() {
            let _ = std::fs::remove_dir_all(&root);
            bail!("the Python archive did not contain {}", self.base_python().display());
        }
        Ok(())
    }

    async fn ensure_venv(&self, p: &Option<ProgressFn>) -> Result<()> {
        if self.venv_python().is_file() {
            return Ok(());
        }
        self.ensure_python(p).await?;
        let args = vec![
            "venv".to_string(),
            self.venv_dir().to_string_lossy().to_string(),
            "--python".into(),
            self.base_python().to_string_lossy().to_string(),
            "--no-python-downloads".into(),
            "--no-cache".into(),
            "--no-config".into(),
        ];
        self.run_tool(p, "python", "Preparing the environment", &self.uv_exe(), &args)
            .await
    }

    async fn ensure_packages(&self, p: &Option<ProgressFn>) -> Result<()> {
        if self.packages_marker().is_file() && self.fermion_exe().is_file() {
            return Ok(());
        }
        let mut args = vec![
            "pip".to_string(),
            "install".into(),
            "--python".into(),
            self.venv_python().to_string_lossy().to_string(),
            "--no-cache".into(),
            "--no-config".into(),
        ];
        args.extend(packages().into_iter().map(String::from));
        self.run_tool(
            p,
            "packages",
            "Installing the speech engine (about 700 MB)",
            &self.uv_exe(),
            &args,
        )
        .await?;
        if !self.fermion_exe().is_file() {
            bail!("the speech engine installed but {} is missing", self.fermion_exe().display());
        }
        std::fs::write(self.packages_marker(), "ok")?;
        Ok(())
    }

    async fn run_tool(
        &self,
        p: &Option<ProgressFn>,
        stage: &'static str,
        message: &str,
        program: &Path,
        args: &[String],
    ) -> Result<()> {
        emit(p, stage, message, None, None);
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .current_dir(&self.base)
            .env("UV_PYTHON_INSTALL_DIR", self.base.join("python"))
            .env("NO_COLOR", "1")
            .env("UV_NO_PROGRESS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("could not start {}", program.display()))?;
        let out = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let err = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;
        let tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (_, _, status) = tokio::join!(
            pump(out, stage, message.to_string(), p.clone(), tail.clone()),
            pump(err, stage, message.to_string(), p.clone(), tail.clone()),
            child.wait()
        );
        let status = status?;
        if !status.success() {
            let t = tail.lock().join("\n");
            bail!(
                "{} failed ({status}). Last output:\n{t}",
                program.file_name().and_then(|n| n.to_str()).unwrap_or("tool")
            );
        }
        Ok(())
    }

    // ---- run --------------------------------------------------------------

    /// Start the server and wait until `/health` answers. If something
    /// already serves our port (for example the app crashed last time and the
    /// server survived) it is adopted instead of started twice.
    pub async fn start(&self, p: &Option<ProgressFn>, timeout: Duration) -> Result<()> {
        let _guard = self.start_lock.lock().await;
        if self.healthy().await {
            return Ok(());
        }
        let fermion = self.fermion_exe();
        if !fermion.is_file() {
            bail!("Phonon-2 is not set up yet. Open Settings, Speech to text, and choose Set up Phonon-2.");
        }
        self.stop();

        let log = std::fs::File::create(self.log_path())?;
        let log_err = log.try_clone()?;
        let mut cmd = std::process::Command::new(&fermion);
        cmd.args(["serve", MODEL, "--host", "127.0.0.1", "--port", &self.port.to_string()])
            .current_dir(&self.base)
            .env("FERMION_CACHE_DIR", self.base.join("cache"))
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let child = cmd
            .spawn()
            .with_context(|| format!("could not start {}", fermion.display()))?;

        #[cfg(windows)]
        {
            let job = job::Job::new();
            if let Some(j) = &job {
                if !j.assign(&child) {
                    tracing::warn!("could not assign the speech engine to a job object");
                }
            }
            *self.job.lock() = job;
        }
        *self.child.lock() = Some(child);

        let started = Instant::now();
        loop {
            if self.healthy().await {
                return Ok(());
            }
            let exited = {
                let mut g = self.child.lock();
                g.as_mut().and_then(|c| c.try_wait().ok().flatten())
            };
            if let Some(status) = exited {
                bail!("the speech engine exited early ({status}).\n{}", self.log_tail(12));
            }
            if started.elapsed() > timeout {
                self.stop();
                bail!(
                    "the speech engine did not become ready within {} seconds.\n{}",
                    timeout.as_secs(),
                    self.log_tail(12)
                );
            }
            emit(
                p,
                "server",
                "Starting the speech engine",
                self.log_tail(1).lines().last().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                None,
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Cheap check used before every transcription: the server is normally
    /// already healthy, and if it died it is started again.
    pub async fn ensure_running(&self) -> Result<()> {
        if self.healthy().await {
            return Ok(());
        }
        if !self.is_installed() {
            bail!("Phonon-2 is not set up yet. Open Settings, Speech to text, and choose Set up Phonon-2.");
        }
        self.start(&None, Duration::from_secs(180)).await
    }

    /// Stop the server and everything it spawned.
    pub fn stop(&self) {
        #[cfg(windows)]
        let had_job = self.job.lock().take().is_some(); // dropping the job kills the tree
        let child = self.child.lock().take();
        if let Some(mut c) = child {
            #[cfg(windows)]
            if !had_job {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &c.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", &format!("-{}", c.id())])
                    .status();
            }
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub async fn uninstall(&self) -> Result<()> {
        self.stop();
        let mut last: Option<std::io::Error> = None;
        for _ in 0..12 {
            match std::fs::remove_dir_all(&self.base) {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => {
                    last = Some(e);
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
            }
        }
        bail!(
            "could not remove {}: {}",
            self.base.display(),
            last.map(|e| e.to_string()).unwrap_or_default()
        )
    }

    fn log_tail(&self, n: usize) -> String {
        let text = std::fs::read_to_string(self.log_path()).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(n);
        lines[start..].join("\n")
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn pump<R: tokio::io::AsyncRead + Unpin>(
    r: R,
    stage: &'static str,
    message: String,
    p: Option<ProgressFn>,
    tail: Arc<Mutex<Vec<String>>>,
) {
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        {
            let mut t = tail.lock();
            t.push(line.clone());
            if t.len() > 30 {
                t.remove(0);
            }
        }
        emit(&p, stage, message.clone(), Some(line), None);
    }
}

async fn fetch_sha256(url: &str) -> Result<String> {
    let text = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(30))
        .build()?
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let token = text
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("empty checksum file at {url}"))?;
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("unexpected checksum format at {url}");
    }
    Ok(token.to_ascii_lowercase())
}

async fn download_verified(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    on_percent: impl Fn(f32),
) -> Result<()> {
    let resp = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .build()?
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let total = resp.content_length();
    let mut file = std::fs::File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        hasher.update(&chunk);
        done += chunk.len() as u64;
        if let Some(t) = total {
            on_percent(done as f32 / t as f32 * 100.0);
        }
    }
    file.flush()?;
    drop(file);
    let got = format!("{:x}", hasher.finalize());
    if got != expected_sha256 {
        let _ = std::fs::remove_file(dest);
        bail!("checksum mismatch for {url}: expected {expected_sha256}, got {got}");
    }
    Ok(())
}

/// Extract with the system bsdtar. Windows needs System32\tar.exe (it reads
/// zip); a Git-for-Windows GNU tar earlier on PATH cannot.
async fn extract_archive(archive: &Path, dest: &Path, is_zip: bool) -> Result<()> {
    let tar = if cfg!(windows) {
        PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
            .join("System32")
            .join("tar.exe")
    } else {
        PathBuf::from("/usr/bin/tar")
    };
    let mut cmd = std::process::Command::new(tar);
    cmd.arg(if is_zip { "-xf" } else { "-xzf" })
        .arg(archive)
        .arg("-C")
        .arg(dest);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = tokio::task::spawn_blocking(move || cmd.output()).await??;
    if !out.status.success() {
        bail!(
            "could not extract {}: {}",
            archive.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}
