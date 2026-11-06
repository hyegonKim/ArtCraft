//! Explicit external Ghidra provider. Existing Rust lifting and MCP are independent.
//! Targets are imported as data; only the explicitly configured SDK launcher runs.
use crate::macho_objc::MachOImage;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EXPORTER: &str = include_str!("../ghidra-extension/ArtCraftDecompile.java");
const INPUT_LIMIT: usize = 64 * 1024 * 1024;
const C_LIMIT: usize = 1024 * 1024;
const JSON_LIMIT: usize = 2 * C_LIMIT;
const PIPE_LIMIT: usize = C_LIMIT;
type Result<T> = std::result::Result<T, BackendError>;

#[derive(Debug)]
pub struct BackendError {
    pub code: &'static str,
    pub message: String,
    pub workspace: Option<PathBuf>,
}
impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for BackendError {}
impl BackendError {
    pub fn json(&self) -> Value {
        json!({"schema":1,"status":"error","provider":"external_ghidra","code":self.code,"message":self.message,"workspace":self.workspace})
    }
}
fn error(code: &'static str, message: impl ToString) -> BackendError {
    BackendError {
        code,
        message: message.to_string(),
        workspace: None,
    }
}
fn io<T>(value: std::io::Result<T>) -> Result<T> {
    value.map_err(|e| error("io", e))
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone)]
pub struct Config {
    pub input: PathBuf,
    pub entry: u64,
    pub ghidra_home: PathBuf,
    pub java_home: PathBuf,
    pub project_root: PathBuf,
    pub timeout: Duration,
}
impl Config {
    pub fn parse(args: &[String]) -> Result<Self> {
        if args.len() < 2 {
            return Err(error("usage", "ghidra-decompile <arm64-macho> <entry-hex> --ghidra-home <absolute-sdk> --java-home <absolute-jdk> [--project-root <absolute-local-path>] [--timeout-seconds <1..180>]"));
        }
        let entry = u64::from_str_radix(args[1].trim_start_matches("0x"), 16)
            .map_err(|_| error("entry", "entry must be hexadecimal"))?;
        let mut flags = BTreeMap::new();
        for pair in args[2..].chunks(2) {
            if pair.len() != 2
                || !matches!(
                    pair[0].as_str(),
                    "--ghidra-home" | "--java-home" | "--project-root" | "--timeout-seconds"
                )
                || flags.insert(pair[0].as_str(), pair[1].as_str()).is_some()
            {
                return Err(error(
                    "usage",
                    "unknown, duplicate, or incomplete provider option",
                ));
            }
        }
        let sdk = flags
            .get("--ghidra-home")
            .ok_or_else(|| error("configuration", "explicit --ghidra-home is required"))?;
        let jdk = flags
            .get("--java-home")
            .ok_or_else(|| error("configuration", "explicit --java-home is required"))?;
        let project_root = match flags.get("--project-root") {
            Some(p) => PathBuf::from(p),
            None => PathBuf::from(
                std::env::var_os("HOME")
                    .ok_or_else(|| error("configuration", "HOME missing; pass --project-root"))?,
            )
            .join("Library/ArtCraft_Rebuild/ghidra-provider"),
        };
        let seconds = flags
            .get("--timeout-seconds")
            .unwrap_or(&"180")
            .parse::<u64>()
            .map_err(|_| error("configuration", "timeout must be 1..180 seconds"))?;
        if !(1..=180).contains(&seconds) {
            return Err(error("configuration", "timeout must be 1..180 seconds"));
        }
        Ok(Self {
            input: PathBuf::from(&args[0]),
            entry,
            ghidra_home: PathBuf::from(sdk),
            java_home: PathBuf::from(jdk),
            project_root,
            timeout: Duration::from_secs(seconds),
        })
    }
}

// The OFF policy is thread-local and restored even on error. On other platforms
// this initial provider is unavailable, rather than silently weakening its guard.
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn getiopolicy_np(kind: i32, scope: i32) -> i32;
    fn setiopolicy_np(kind: i32, scope: i32, policy: i32) -> i32;
}
struct ResidentPolicy {
    #[cfg(target_os = "macos")]
    previous: i32,
}
impl ResidentPolicy {
    fn enter() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            unsafe {
                let previous = getiopolicy_np(3, 1);
                if previous < 0 || setiopolicy_np(3, 1, 1) != 0 || getiopolicy_np(3, 1) != 1 {
                    if previous >= 0 {
                        setiopolicy_np(3, 1, previous);
                    }
                    return Err(error(
                        "residency",
                        "could not disable automatic materialization",
                    ));
                }
                Ok(Self { previous })
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(error(
                "unsupported_host",
                "external Ghidra currently requires macOS arm64",
            ))
        }
    }
    fn finish(self) -> Result<()> {
        #[cfg(target_os = "macos")]
        unsafe {
            if setiopolicy_np(3, 1, self.previous) != 0 || getiopolicy_np(3, 1) != self.previous {
                return Err(error(
                    "residency",
                    "could not restore materialization policy",
                ));
            }
        }
        Ok(())
    }
}
impl Drop for ResidentPolicy {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            setiopolicy_np(3, 1, self.previous);
        }
    }
}

fn resident(meta: &fs::Metadata) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        meta.st_flags() & 0x40000000 == 0
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = meta;
        true
    }
}
fn file_flags(m: &fs::Metadata) -> u32 {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        m.st_flags()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = m;
        0
    }
}
fn identity(m: &fs::Metadata) -> (u64, u64, u32, u64, i64, i64, i64, i64, u32) {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
        file_flags(m),
    )
}
fn path_guard(path: &Path, allow_missing: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err(error("path", "absolute paths are required"));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if !matches!(part, Component::RootDir | Component::Normal(_)) {
            return Err(error(
                "path",
                "dot and parent path components are not supported",
            ));
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() || !resident(&m) => {
                return Err(error(
                    "residency",
                    format!("symlink or nonresident path: {}", current.display()),
                ))
            }
            Ok(_) => {}
            Err(e) if allow_missing && e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(error("path", format!("{}: {e}", current.display()))),
        }
    }
    Ok(())
}
// Parent descriptors are pinned one component at a time: no path lookup may
// silently follow an intermediate symlink, even when the kernel rejects ANY.
fn open_anchored(path: &Path) -> Result<File> {
    let mut directory = io(File::open("/"))?;
    let names: Vec<_> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    for (index, name) in names.iter().enumerate() {
        use std::os::unix::ffi::OsStrExt;
        let name = CString::new(name.as_bytes()).map_err(|_| error("path", "NUL in path"))?;
        let last = index + 1 == names.len();
        #[cfg(target_os = "macos")]
        let flags = 0x100 | 0x01000000 | 4 | if last { 0 } else { 0x100000 };
        #[cfg(not(target_os = "macos"))]
        let flags = 0x20000 | 0x80000 | 0x800 | if last { 0 } else { 0x10000 };
        let fd = unsafe { openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(error("io", std::io::Error::last_os_error()));
        }
        let next = unsafe { File::from_raw_fd(fd) };
        let metadata = io(next.metadata())?;
        if !resident(&metadata) || (!last && !metadata.is_dir()) || (last && !metadata.is_file()) {
            return Err(error(
                "residency",
                "descriptor is nonresident or has the wrong file type",
            ));
        }
        if last {
            return Ok(next);
        }
        directory = next;
    }
    Err(error("path", "file path is empty"))
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    path_guard(path, false)?;
    let before = io(fs::symlink_metadata(path))?;
    if !before.is_file() || before.len() > limit as u64 {
        return Err(error(
            "input_limit",
            format!("expected regular file <= {limit} bytes: {}", path.display()),
        ));
    }
    let mut file = open_anchored(path)?;
    if identity(&io(file.metadata())?) != identity(&before) {
        return Err(error("source_changed", "file changed before read"));
    }
    let mut bytes = Vec::new();
    io((&mut file).take(limit as u64 + 1).read_to_end(&mut bytes))?;
    if bytes.len() as u64 != before.len()
        || identity(&io(file.metadata())?) != identity(&before)
        || identity(&io(fs::symlink_metadata(path))?) != identity(&before)
    {
        return Err(error(
            "source_changed",
            format!("file changed during read: {}", path.display()),
        ));
    }
    Ok(bytes)
}
fn text(path: &Path) -> Result<String> {
    String::from_utf8(read(path, 256 * 1024)?)
        .map_err(|_| error("configuration", "configuration is not UTF-8"))
}
fn property(source: &str, key: &str) -> Option<String> {
    source
        .lines()
        .filter(|s| !s.trim_start().starts_with('#'))
        .find_map(|s| {
            s.split_once('=')
                .filter(|(k, _)| k.trim() == key)
                .map(|(_, v)| v.trim().trim_matches('"').to_string())
        })
}
fn executable(path: &Path) -> Result<()> {
    path_guard(path, false)?;
    let m = io(fs::metadata(path))?;
    if !m.is_file() || m.permissions().mode() & 0o111 == 0 {
        return Err(error(
            "configuration",
            format!("not an executable file: {}", path.display()),
        ));
    }
    Ok(())
}
fn valid_project(path: &Path) -> Result<()> {
    path_guard(path, true)?;
    if path.components().any(|c| match c {
        Component::Normal(v) => v.to_str().is_none_or(|s| {
            s.starts_with('.')
                || s.chars()
                    .any(|c| c.is_whitespace() || c == '"' || c == '\'')
        }),
        _ => false,
    }) {
        return Err(error(
            "path",
            "project path components must be visible UTF-8 names without whitespace or quotes",
        ));
    }
    Ok(())
}
type Fingerprints = BTreeMap<String, Value>;
fn dependencies(config: &Config) -> Result<(Fingerprints, PathBuf)> {
    for path in [&config.ghidra_home, &config.java_home] {
        path_guard(path, false)?;
        if path
            .to_str()
            .is_none_or(|s| s.chars().any(char::is_whitespace))
        {
            return Err(error(
                "path",
                "SDK/JDK paths must be UTF-8 without whitespace",
            ));
        }
    }
    if config
        .ghidra_home
        .to_string_lossy()
        .to_lowercase()
        .contains("caches")
    {
        return Err(error("configuration","Ghidra class discovery excludes paths containing Caches; provide an installation outside Caches"));
    }
    let props = text(&config.ghidra_home.join("Ghidra/application.properties"))?;
    if property(&props, "application.version").as_deref() != Some("12.2")
        || property(&props, "application.java.min").as_deref() != Some("25")
    {
        return Err(error(
            "configuration",
            "this provider requires the validated Ghidra 12.2/JDK 25 contract",
        ));
    }
    if property(
        &text(&config.ghidra_home.join("support/launch.properties"))?,
        "JAVA_HOME_OVERRIDE",
    )
    .is_some_and(|s| !s.is_empty())
    {
        return Err(error(
            "configuration",
            "SDK JAVA_HOME_OVERRIDE must be empty; requested JDK must remain explicit",
        ));
    }
    let release = text(&config.java_home.join("release"))?;
    if !property(&release, "JAVA_VERSION").is_some_and(|s| s == "25" || s.starts_with("25."))
        || property(&release, "OS_ARCH").as_deref() != Some("aarch64")
    {
        return Err(error("configuration", "JDK 25 aarch64 required"));
    }
    let build = config
        .ghidra_home
        .join("Ghidra/Features/Decompiler/build/os/mac_arm_64/decompile");
    let native = if build.try_exists().map_err(|e| error("configuration", e))? {
        build
    } else {
        config
            .ghidra_home
            .join("Ghidra/Features/Decompiler/os/mac_arm_64/decompile")
    };
    for path in [
        config.ghidra_home.join("support/analyzeHeadless"),
        config.java_home.join("bin/java"),
        config.java_home.join("bin/javac"),
        native.clone(),
    ] {
        executable(&path)?;
    }
    let native_bytes = read(&native, 64 * 1024 * 1024)?;
    if native_bytes.len() < 32 || native_bytes[..8] != [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1] {
        return Err(error(
            "configuration",
            "native decompiler must be Mach-O arm64",
        ));
    }
    let mut paths = vec![
        config.ghidra_home.join("Ghidra/application.properties"),
        config.ghidra_home.join("support/launch.properties"),
        config.ghidra_home.join("support/analyzeHeadless"),
        config.ghidra_home.join("support/launch.sh"),
        config.ghidra_home.join("support/LaunchSupport.jar"),
        config.java_home.join("release"),
        config.java_home.join("bin/java"),
        config.java_home.join("bin/javac"),
        native.clone(),
    ];
    let mut pending = vec![config.ghidra_home.join("Ghidra")];
    let mut directories = 0;
    while let Some(dir) = pending.pop() {
        path_guard(&dir, false)?;
        directories += 1;
        if directories > 10000 {
            return Err(error(
                "configuration",
                "SDK directory inventory exceeds limit",
            ));
        }
        for entry in io(fs::read_dir(dir))? {
            let path = io(entry)?.path();
            let metadata = io(fs::symlink_metadata(&path))?;
            if metadata.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "jar") {
                paths.push(path);
            }
            if paths.len() > 2048 {
                return Err(error("configuration", "SDK JAR inventory exceeds limit"));
            }
        }
    }
    let mut map = BTreeMap::new();
    let mut total = 0usize;
    for path in paths {
        let bytes = read(&path, 128 * 1024 * 1024)?;
        total += bytes.len();
        if total > 1024 * 1024 * 1024 {
            return Err(error("configuration", "SDK fingerprint budget exceeded"));
        }
        map.insert(
            path.to_string_lossy().into_owned(),
            json!({"sha256":sha(&bytes),"bytes":bytes.len()}),
        );
    }
    if !map.contains_key(
        &config
            .ghidra_home
            .join("Ghidra/Framework/Utility/lib/Utility.jar")
            .to_string_lossy()
            .into_owned(),
    ) {
        return Err(error("configuration", "SDK Utility.jar missing"));
    }
    Ok((map, native))
}
fn create_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = io(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path))?;
    io(file.write_all(bytes))
}
static WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn unique_name(prefix: &str, timestamp: u128) -> String {
    format!(
        "{prefix}-{}-{timestamp}-{}",
        std::process::id(),
        WORKSPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
fn workspace(root: &Path) -> Result<PathBuf> {
    valid_project(root)?;
    io(fs::create_dir_all(root))?;
    path_guard(root, false)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| error("clock", e))?
        .as_nanos();
    let path = root.join(unique_name("run", nonce));
    io(fs::DirBuilder::new().mode(0o700).create(&path))?;
    for name in [
        "home", "settings", "cache", "tmp", "prefs", "scripts", "project",
    ] {
        io(fs::DirBuilder::new().mode(0o700).create(path.join(name)))?;
    }
    Ok(path)
}

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
    fn fcntl(fd: i32, command: i32, ...) -> i32;
    fn openat(fd: i32, path: *const std::ffi::c_char, flags: i32, ...) -> i32;
}
struct OwnedChild {
    child: Child,
    group: i32,
    active: bool,
}
impl OwnedChild {
    fn stop(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        unsafe {
            kill(-self.group, 15);
        }
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(150) {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        unsafe {
            kill(-self.group, 9);
        }
        let _ = self.child.wait();
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.stop();
    }
}
fn nonblocking<T: std::os::fd::AsRawFd>(pipe: &T) -> Result<()> {
    #[cfg(target_os = "macos")]
    let nonblock = 4;
    #[cfg(not(target_os = "macos"))]
    let nonblock = 0x800;
    let fd = pipe.as_raw_fd();
    let flags = unsafe { fcntl(fd, 3) };
    if flags < 0 || unsafe { fcntl(fd, 4, flags | nonblock) } < 0 {
        return Err(error("process", std::io::Error::last_os_error()));
    }
    Ok(())
}
fn drain(pipe: &mut impl Read, bytes: &mut Vec<u8>, limit: usize) -> Result<bool> {
    let mut chunk = [0u8; 8192];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                if bytes.len() + n > limit {
                    bytes.extend_from_slice(&chunk[..limit - bytes.len()]);
                    return Err(error(
                        "output_limit",
                        "headless stdout/stderr exceeds its byte limit",
                    ));
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(error("process", e)),
        }
    }
}
fn run_process(
    command: &mut Command,
    timeout: Duration,
    result_path: &Path,
) -> Result<(Vec<u8>, Vec<u8>)> {
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = io(command.spawn())?;
    let group = child.id() as i32;
    if group <= 1 {
        return Err(error("process", "invalid owned process group"));
    }
    let mut owned = OwnedChild {
        child,
        group,
        active: true,
    };
    let mut stdout = owned
        .child
        .stdout
        .take()
        .ok_or_else(|| error("process", "stdout missing"))?;
    let mut stderr = owned
        .child
        .stderr
        .take()
        .ok_or_else(|| error("process", "stderr missing"))?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let start = Instant::now();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let outcome = (|| -> Result<()> {
        loop {
            if start.elapsed() >= timeout {
                return Err(error(
                    "timeout",
                    format!(
                        "owned headless process group exceeded {} seconds",
                        timeout.as_secs_f64()
                    ),
                ));
            }
            if let Ok(m) = fs::symlink_metadata(result_path) {
                if !m.is_file() || m.len() > JSON_LIMIT as u64 {
                    return Err(error(
                        "output_limit",
                        "result file exceeds its byte limit or is not regular",
                    ));
                }
            }
            let out_done = drain(&mut stdout, &mut out, PIPE_LIMIT)?;
            let err_done = drain(&mut stderr, &mut err, PIPE_LIMIT)?;
            if let Some(status) = io(owned.child.try_wait())? {
                if out_done && err_done {
                    if !status.success() {
                        return Err(error(
                            "headless_failed",
                            format!(
                                "headless exited {status}; stderr: {}",
                                String::from_utf8_lossy(&err[..err.len().min(4096)])
                            ),
                        ));
                    }
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    owned.stop();
    let parent = result_path
        .parent()
        .ok_or_else(|| error("path", "result parent missing"))?;
    create_file(&parent.join("stdout.log"), &out)?;
    create_file(&parent.join("stderr.log"), &err)?;
    outcome?;
    Ok((out, err))
}

fn validate(
    result: &Value,
    request: &Value,
    config: &Config,
    native: &Path,
    native_sha: &Value,
) -> Result<()> {
    if result.get("schema") != Some(&json!(1))
        || result.get("request_id") != request.get("request_id")
    {
        return Err(error("protocol", "schema/request identity mismatch"));
    }
    if result.get("status") != Some(&json!("complete")) {
        return Err(error(
            "provider_failed",
            result
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("headless exporter did not report complete"),
        ));
    }
    for (key, expected) in [
        ("input_sha256", request["input_sha256"].clone()),
        ("input_bytes", request["input_bytes"].clone()),
        ("entry", request["entry"].clone()),
        ("sdk_home", json!(config.ghidra_home)),
        ("sdk_version", json!("12.2")),
        ("java_home", json!(config.java_home)),
        ("native_path", json!(native)),
        ("native_sha256", native_sha.clone()),
        ("exporter_sha256", json!(sha(EXPORTER.as_bytes()))),
        ("architecture", json!("AARCH64")),
        ("high_function", json!(true)),
        ("decompile_completed", json!(true)),
        ("analysis_timed_out", json!(false)),
    ] {
        if result.get(key) != Some(&expected) {
            return Err(error(
                "provenance",
                format!("observed {key} does not match the request/prerequisites"),
            ));
        }
    }
    if !result
        .get("java_version")
        .and_then(Value::as_str)
        .is_some_and(|v| v == "25" || v.starts_with("25."))
        || !result
            .get("function_name")
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty() && v.len() <= 4096)
    {
        return Err(error(
            "protocol",
            "missing/invalid actual Java version or function name",
        ));
    }
    if !result
        .get("c")
        .and_then(Value::as_str)
        .is_some_and(|v| !v.trim().is_empty() && v.len() <= C_LIMIT)
    {
        return Err(error(
            "output_limit",
            "native C is empty, missing or too large",
        ));
    }
    Ok(())
}

fn bounded_result(result: Value) -> Result<Value> {
    let encoded = serde_json::to_vec(&result).map_err(|e| error("protocol", e))?;
    if encoded.len() > JSON_LIMIT {
        return Err(error(
            "output_limit",
            "complete result with provenance exceeds the JSON byte limit",
        ));
    }
    Ok(result)
}

pub fn decompile(config: &Config) -> Result<Value> {
    let policy = ResidentPolicy::enter()?;
    let result = decompile_inner(config);
    policy.finish()?;
    result
}
fn decompile_inner(config: &Config) -> Result<Value> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err(error(
            "unsupported_host",
            "external provider currently supports macOS arm64 only",
        ));
    }
    if config.timeout.is_zero() || config.timeout > Duration::from_secs(180) {
        return Err(error(
            "configuration",
            "overall process timeout must be positive and <=180 seconds",
        ));
    }
    let input = read(&config.input, INPUT_LIMIT)?;
    if !matches!(
        input.get(..4),
        Some(
            [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xbf, 0xba, 0xfe, 0xca]
        )
    ) {
        return Err(error("unsupported_input", "only ARM64 Mach-O is supported"));
    }
    let image = MachOImage::parse(&input).map_err(|e| error("input", e))?;
    if image.selected_arch != "arm64" {
        return Err(error("unsupported_input", "only ARM64 Mach-O is supported"));
    }
    if config.entry % 4 != 0
        || !image.segments.iter().any(|s| {
            s.permissions.is_some_and(|p| p & 4 != 0) && s.read_bytes(config.entry, 4).is_some()
        })
    {
        return Err(error(
            "entry",
            "requested entry must be aligned and backed by executable Mach-O bytes",
        ));
    }
    let original_metadata = identity(&io(fs::symlink_metadata(&config.input))?);
    let engine_path = io(std::env::current_exe())?;
    let engine_sha = sha(&read(&engine_path, 256 * 1024 * 1024)?);
    let (before, native) = dependencies(config)?;
    let work = workspace(&config.project_root)?;
    let result = (|| {
        let input_path = work.join("input.macho");
        let script = work.join("scripts/ArtCraftDecompile.java");
        let output = work.join("result.json");
        create_file(&input_path, &input)?;
        create_file(&script, EXPORTER.as_bytes())?;
        let request = json!({"schema":1,"request_id":sha(format!("{}:{}:{}",work.display(),sha(&input),config.entry).as_bytes()),"input_path":input_path,"input_sha256":sha(&input),"input_bytes":input.len(),"entry":format!("0x{:x}",config.entry),"exporter_path":script});
        let request_path = work.join("request.json");
        create_file(
            &request_path,
            &serde_json::to_vec(&request).map_err(|e| error("protocol", e))?,
        )?;
        let options=format!("-Duser.home={} -Dapplication.settingsdir={} -Dapplication.cachedir={} -Dapplication.tempdir={} -Djava.io.tmpdir={} -Djava.util.prefs.userRoot={} -XX:ActiveProcessorCount=2 -Xmx1G",work.join("home").display(),work.join("settings").display(),work.join("cache").display(),work.join("tmp").display(),work.join("tmp").display(),work.join("prefs").display());
        let mut command = Command::new(config.ghidra_home.join("support/analyzeHeadless"));
        command
            .env_clear()
            .env("HOME", work.join("home"))
            .env("JAVA_HOME", &config.java_home)
            .env("JAVA_TOOL_OPTIONS", options)
            .env(
                "PATH",
                format!(
                    "{}:/usr/bin:/bin:/usr/sbin:/sbin",
                    config.java_home.join("bin").display()
                ),
            )
            .env("LANG", "en_US.UTF-8")
            .env("LC_ALL", "en_US.UTF-8")
            .env("TMPDIR", work.join("tmp"))
            .env("XDG_CONFIG_HOME", work.join("settings"))
            .env("XDG_CACHE_HOME", work.join("cache"))
            .env("GHIDRA_HEADLESS_MAXMEM", "1G")
            .current_dir(&work)
            .arg(work.join("project"))
            .arg("ArtCraftProvider")
            .args(["-import"])
            .arg(&input_path)
            .args([
                "-analysisTimeoutPerFile",
                "60",
                "-max-cpu",
                "2",
                "-scriptPath",
            ])
            .arg(work.join("scripts"))
            .args(["-postScript", "ArtCraftDecompile.java"])
            .arg(&request_path)
            .arg(&output);
        run_process(&mut command, config.timeout, &output)?;
        let response: Value = serde_json::from_slice(&read(&output, JSON_LIMIT)?)
            .map_err(|e| error("protocol", format!("result is not valid JSON: {e}")))?;
        validate(
            &response,
            &request,
            config,
            &native,
            &before[&native.to_string_lossy().into_owned()]["sha256"],
        )?;
        let (after, after_native) = dependencies(config)?;
        if after != before
            || after_native != native
            || read(&config.input, INPUT_LIMIT)? != input
            || identity(&io(fs::symlink_metadata(&config.input))?) != original_metadata
            || read(&input_path, INPUT_LIMIT)? != input
            || read(&script, EXPORTER.len())? != EXPORTER.as_bytes()
            || sha(&read(&engine_path, 256 * 1024 * 1024)?) != engine_sha
        {
            return Err(error(
                "source_changed",
                "input, exporter or SDK/JDK changed during provider execution",
            ));
        }
        bounded_result(
            json!({"schema":1,"status":"complete","provider":"external_ghidra","runtime_checked":true,"engine_version":env!("CARGO_PKG_VERSION"),"engine_path":engine_path,"engine_sha256":engine_sha,"workspace":work,"limits":{"input_bytes":INPUT_LIMIT,"c_bytes":C_LIMIT,"json_bytes":JSON_LIMIT,"stdout_bytes":PIPE_LIMIT,"stderr_bytes":PIPE_LIMIT,"overall_seconds":config.timeout.as_secs_f64(),"analysis_seconds":60,"function_seconds":30,"cpu":2,"heap":"1G"},"result":response,"fingerprints":before,"exporter_sha256":sha(EXPORTER.as_bytes())}),
        )
    })();
    result.map_err(|mut e: BackendError| {
        e.workspace = Some(work);
        e
    })
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let base = fs::canonicalize(std::env::temp_dir()).unwrap();
            let p = base.join(unique_name(
                "artcraft-provider-test",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn put(path: &Path, bytes: &[u8], exec: bool) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if exec { 0o700 } else { 0o600 }),
        )
        .unwrap();
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 8192];
        for (offset, value) in [
            (0, 0xfeedfacfu32),
            (4, 0x0100000c),
            (8, 0),
            (12, 2),
            (16, 1),
            (20, 72),
            (32, 0x19),
            (36, 72),
            (88, 7),
            (92, 5),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[40..46].copy_from_slice(b"__TEXT");
        for (offset, value) in [(56, 0x100000000u64), (64, 8192), (72, 0), (80, 8192)] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[4096..4104].copy_from_slice(&[0x40, 0x05, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]);
        bytes
    }
    fn fake_config(temp: &Temp, mode: &str) -> Config {
        let sdk = temp.0.join("sdk");
        let jdk = temp.0.join("jdk");
        put(
            &sdk.join("Ghidra/application.properties"),
            b"application.version=12.2\napplication.java.min=25\n",
            false,
        );
        put(
            &sdk.join("support/launch.properties"),
            b"JAVA_HOME_OVERRIDE=\n",
            false,
        );
        put(
            &sdk.join("support/launch.sh"),
            b"# fake provider unit only\n",
            false,
        );
        put(
            &sdk.join("Ghidra/Framework/Utility/lib/Utility.jar"),
            b"unit fake jar; never loaded",
            false,
        );
        put(
            &sdk.join("support/LaunchSupport.jar"),
            b"unit launch support",
            false,
        );
        let native = sdk.join("Ghidra/Features/Decompiler/build/os/mac_arm_64/decompile");
        let mut header = vec![0; 32];
        header[..8].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1]);
        put(&native, &header, true);
        put(
            &jdk.join("release"),
            b"JAVA_VERSION=\"25.0.5\"\nOS_ARCH=\"aarch64\"\n",
            false,
        );
        for bin in ["java", "javac"] {
            put(
                &jdk.join("bin").join(bin),
                b"unit fake JDK; never launched",
                true,
            );
        }
        let spec=json!({"mode":mode,"sdk_home":sdk,"java_home":jdk,"native_path":native,"native_sha256":sha(&header)}).to_string();
        fn quote(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
        let launcher=format!("#!/bin/sh\nprintf '%s' {} > provider-test.json\nexec {} --exact ghidra_backend::tests::fake_process_child --nocapture\n",quote(&spec),quote(std::env::current_exe().unwrap().to_str().unwrap()));
        put(
            &sdk.join("support/analyzeHeadless"),
            launcher.as_bytes(),
            true,
        );
        put(&temp.0.join("input.macho"), &fixture(), false);
        Config {
            input: temp.0.join("input.macho"),
            entry: 0x100001000,
            ghidra_home: sdk,
            java_home: jdk,
            project_root: temp.0.join("projects"),
            timeout: Duration::from_secs(5),
        }
    }
    // This test doubles as an owned child responder. A normal test invocation
    // has no provider-test.json, so it never consults user state or launches Java.
    #[test]
    fn fake_process_child() {
        let spec_path = Path::new("provider-test.json");
        if !spec_path.is_file() {
            return;
        }
        let spec: Value = serde_json::from_slice(&fs::read(spec_path).unwrap()).unwrap();
        let request: Value = serde_json::from_slice(&fs::read("request.json").unwrap()).unwrap();
        let mut out = json!({"schema":1,"request_id":request["request_id"],"status":"complete","input_sha256":request["input_sha256"],"input_bytes":request["input_bytes"],"entry":request["entry"],"sdk_home":spec["sdk_home"],"sdk_version":"12.2","java_home":spec["java_home"],"java_version":"25.0.5","native_path":spec["native_path"],"native_sha256":spec["native_sha256"],"exporter_sha256":sha(EXPORTER.as_bytes()),"architecture":"AARCH64","high_function":true,"decompile_completed":true,"analysis_timed_out":false,"function_name":"fixture","c":"int fixture(void) { return 42; }"});
        match spec["mode"].as_str().unwrap() {
            "wrong_id" => out["request_id"] = json!("wrong"),
            "wrong_sha" => out["input_sha256"] = json!("00"),
            "wrong_entry" => out["entry"] = json!("0x100001004"),
            "wrong_native" => out["native_sha256"] = json!("00"),
            "wrong_jdk" => out["java_home"] = json!("/wrong"),
            "no_high" => out["high_function"] = json!(false),
            "analysis_timeout" => out["analysis_timed_out"] = json!(true),
            "script_error" => {
                out["status"] = json!("error");
                out["error"] = json!("deliberate exporter failure");
            }
            "huge_c" => out["c"] = json!("a".repeat(C_LIMIT + 1)),
            "malformed" => {
                fs::write("result.json", b"{").unwrap();
                return;
            }
            "missing" => return,
            "exit" => std::process::exit(7),
            "huge_result" => {
                fs::write("result.json", vec![b'x'; JSON_LIMIT + 1]).unwrap();
                return;
            }
            "stdout" => {
                std::io::stdout()
                    .write_all(&vec![b'x'; PIPE_LIMIT + 8192])
                    .unwrap();
                return;
            }
            "stderr" => {
                std::io::stderr()
                    .write_all(&vec![b'x'; PIPE_LIMIT + 8192])
                    .unwrap();
                return;
            }
            "mutate_native" => {
                fs::write(spec["native_path"].as_str().unwrap(), b"changed").unwrap();
            }
            "mutate_input" => {
                fs::write(request["input_path"].as_str().unwrap(), b"changed").unwrap();
            }
            "good" => {}
            other => panic!("unknown test mode {other}"),
        }
        fs::write("result.json", serde_json::to_vec(&out).unwrap()).unwrap();
    }
    #[test]
    fn fake_provider_success_is_bound_to_observed_request_and_dependencies() {
        let t = Temp::new();
        let config = fake_config(&t, "good");
        let before = unsafe { getiopolicy_np(3, 1) };
        let result = decompile(&config).unwrap();
        assert_eq!(result["status"], "complete");
        assert_eq!(result["runtime_checked"], true);
        assert_eq!(result["result"]["entry"], "0x100001000");
        assert!(result["fingerprints"].as_object().unwrap().len() >= 9);
        assert_eq!(unsafe { getiopolicy_np(3, 1) }, before);
        assert_eq!(fs::read(&config.input).unwrap(), fixture());
    }
    #[test]
    fn fake_provider_protocol_failures_never_report_success() {
        for (mode, code) in [
            ("wrong_id", "protocol"),
            ("wrong_sha", "provenance"),
            ("wrong_entry", "provenance"),
            ("wrong_native", "provenance"),
            ("wrong_jdk", "provenance"),
            ("no_high", "provenance"),
            ("analysis_timeout", "provenance"),
            ("script_error", "provider_failed"),
            ("huge_c", "output_limit"),
            ("malformed", "protocol"),
            ("missing", "path"),
            ("exit", "headless_failed"),
            ("huge_result", "output_limit"),
            ("stdout", "output_limit"),
            ("stderr", "output_limit"),
            ("mutate_native", "configuration"),
            ("mutate_input", "source_changed"),
        ] {
            let t = Temp::new();
            let config = fake_config(&t, mode);
            let policy = unsafe { getiopolicy_np(3, 1) };
            let failure = decompile(&config).unwrap_err();
            assert_eq!(failure.code, code, "mode {mode}: {failure}");
            assert!(failure.workspace.is_some());
            assert_eq!(failure.json()["status"], "error");
            assert_eq!(unsafe { getiopolicy_np(3, 1) }, policy, "{mode}");
        }
    }
    #[test]
    fn invalid_entries_and_configuration_fail_before_launch() {
        let t = Temp::new();
        let mut config = fake_config(&t, "good");
        for entry in [0x100001001, 0x200000000, u64::MAX] {
            config.entry = entry;
            assert_eq!(decompile(&config).unwrap_err().code, "entry");
            assert!(!config.project_root.exists());
        }
        config.entry = 0x100001000;
        put(
            &config.ghidra_home.join("support/launch.properties"),
            b"JAVA_HOME_OVERRIDE=/other\n",
            false,
        );
        assert_eq!(decompile(&config).unwrap_err().code, "configuration");
        assert!(!config.project_root.exists());
    }
    #[test]
    fn inputs_paths_size_and_project_collisions_are_guarded() {
        assert_ne!(
            unique_name("same-clock", 0),
            unique_name("same-clock", 0),
            "coarse clock readings must not collide"
        );
        let t = Temp::new();
        let mut config = fake_config(&t, "good");
        let original = config.input.clone();
        let link = t.0.join("input-link");
        std::os::unix::fs::symlink(&original, &link).unwrap();
        config.input = link;
        assert_eq!(decompile(&config).unwrap_err().code, "residency");
        config.input = original;
        let file = OpenOptions::new().write(true).open(&config.input).unwrap();
        file.set_len(INPUT_LIMIT as u64 + 1).unwrap();
        assert_eq!(decompile(&config).unwrap_err().code, "input_limit");
        drop(file);
        put(&config.input, &fixture(), false);
        config.project_root = t.0.join(".hidden/project");
        assert_eq!(decompile(&config).unwrap_err().code, "path");
        config.project_root = t.0.join("visible");
        put(&config.project_root, b"existing", false);
        assert!(decompile(&config).is_err());
        assert_eq!(fs::read(&config.project_root).unwrap(), b"existing");
    }
    #[test]
    fn timeout_kills_only_owned_process_group_and_reaps_its_child() {
        let t = Temp::new();
        let script = t.0.join("timeout.sh");
        put(
            &script,
            b"#!/bin/sh\ntrap 'wait; exit 0' TERM\nsleep 30 &\necho $! > child.pid\nwait\n",
            true,
        );
        let mut command = Command::new(&script);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&t.0);
        let start = Instant::now();
        let failure = run_process(
            &mut command,
            Duration::from_millis(300),
            &t.0.join("unused.json"),
        )
        .unwrap_err();
        assert_eq!(failure.code, "timeout");
        assert!(start.elapsed() < Duration::from_secs(3));
        let pid: i32 = fs::read_to_string(t.0.join("child.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        for _ in 0..100 {
            if unsafe { kill(pid, 0) } != 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            unsafe { kill(pid, 0) },
            -1,
            "owned descendant {pid} survived timeout cleanup"
        );
        assert_eq!(
            unsafe { kill(std::process::id() as i32, 0) },
            0,
            "test parent must remain live"
        );
    }
    #[test]
    fn inherited_pipe_from_a_descendant_cannot_hang_the_reader() {
        let t = Temp::new();
        let script = t.0.join("held-pipe.sh");
        put(
            &script,
            b"#!/bin/sh\nsleep 30 &\necho $! > child.pid\nexit 0\n",
            true,
        );
        let mut command = Command::new(&script);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&t.0);
        let start = Instant::now();
        assert_eq!(
            run_process(
                &mut command,
                Duration::from_millis(150),
                &t.0.join("unused.json")
            )
            .unwrap_err()
            .code,
            "timeout"
        );
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn complete_response_cap_counts_json_escaping_and_provenance() {
        assert!(bounded_result(json!({"c":"int x(void) { return 42; }"})).is_ok());
        let error =
            bounded_result(json!({"c":"\"".repeat(C_LIMIT),"fingerprints":{"host":"checked"}}))
                .unwrap_err();
        assert_eq!(error.code, "output_limit");
    }
    #[test]
    fn parser_requires_explicit_prerequisites_and_bounded_options() {
        for args in [
            vec![],
            vec!["a", "1"],
            vec!["a", "xyz"],
            vec![
                "a",
                "1",
                "--ghidra-home",
                "/a",
                "--java-home",
                "/b",
                "--timeout-seconds",
                "0",
            ],
            vec![
                "a",
                "1",
                "--ghidra-home",
                "/a",
                "--java-home",
                "/b",
                "--timeout-seconds",
                "181",
            ],
            vec![
                "a",
                "1",
                "--ghidra-home",
                "/a",
                "--java-home",
                "/b",
                "--java-home",
                "/c",
            ],
        ] {
            assert!(
                Config::parse(&args.into_iter().map(str::to_string).collect::<Vec<_>>()).is_err()
            );
        }
    }
}
