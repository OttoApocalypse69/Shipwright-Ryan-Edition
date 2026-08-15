#![cfg_attr(windows, windows_subsystem = "windows")]

//! Click-to-run entrypoint for this checkout's local SRE development stack.
//! It owns only the `pnpm tauri dev` process it starts; SRE itself owns the
//! temporary local FTEP child and shuts that child down when its window closes.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use fs2::FileExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const PACKAGE_MANAGER: &str = "pnpm.cmd";
#[cfg(not(windows))]
const PACKAGE_MANAGER: &str = "pnpm";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const VITE_ADDRESS: &str = "127.0.0.1:1420";
const VITE_STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const INSTANCE_LOCK_FILE: &str = "sre-local-launcher.lock";

fn main() {
    if let Err(error) = run() {
        show_error(&error);
    }
}

fn run() -> Result<(), String> {
    let workspace = workspace_root()?;
    let log_path = workspace.join("target").join("sre-local-launcher.log");

    // Keep an OS-level lock for the lifetime of this helper. A second click is
    // therefore an idempotent no-op instead of a scary startup dialog, even if
    // both clicks race before either process has bound the Vite port.
    let _instance_lock = match acquire_instance_lock(&workspace)? {
        Some(lock) => lock,
        None => return Ok(()),
    };
    let mut log = create_log(&log_path)?;

    writeln!(log, "Starting local SRE from {}", workspace.display())
        .map_err(|error| format!("Cannot write {log_path:?}: {error}"))?;

    // Older helper builds did not hold the lock above. If one of those builds
    // is still open, recognize its live launcher and quietly leave it alone.
    // If only Vite survived a forced close, reuse that already-bound frontend
    // and start a fresh SRE window instead of reporting port 1420 as fatal.
    let mut vite_child = if vite_is_listening() {
        if sre_process_is_running() {
            writeln!(
                log,
                "SRE is already running on {VITE_ADDRESS}; reusing the existing instance."
            )
            .map_err(|error| format!("Cannot write {log_path:?}: {error}"))?;
            return Ok(());
        }
        writeln!(
            log,
            "A previous SRE frontend is already listening on {VITE_ADDRESS}; reusing it."
        )
        .map_err(|error| format!("Cannot write {log_path:?}: {error}"))?;
        None
    } else {
        let vite_stderr = log
            .try_clone()
            .map_err(|error| format!("Cannot duplicate {log_path:?}: {error}"))?;
        let mut vite = Command::new(PACKAGE_MANAGER);
        vite.args(["--filter", "@sre/launcher", "dev:web"])
            .current_dir(&workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(vite_stderr));
        #[cfg(windows)]
        vite.creation_flags(CREATE_NO_WINDOW);

        Some(vite.spawn().map_err(|error| {
            format!(
                "Could not start {PACKAGE_MANAGER}. Install the project's Node.js and pnpm prerequisites, then try again. Details are in {}: {error}",
                log_path.display()
            )
        })?)
    };

    if let Some(vite) = vite_child.as_mut() {
        if let Err(error) = wait_for_vite(vite) {
            stop_owned_vite(&mut vite_child);
            return Err(format!(
                "{error} See {} for the exact error.",
                log_path.display()
            ));
        }
    }

    let sre_stderr = File::options()
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("Cannot append {}: {error}", log_path.display()))?;
    let sre_stdout = sre_stderr
        .try_clone()
        .map_err(|error| format!("Cannot duplicate {log_path:?}: {error}"))?;
    let sre_binary = workspace
        .join("target")
        .join("debug")
        .join(if cfg!(windows) {
            "sre-launcher.exe"
        } else {
            "sre-launcher"
        });
    if !sre_binary.is_file() {
        stop_owned_vite(&mut vite_child);
        return Err(format!(
            "The built SRE launcher is missing at {}. Rebuild START SRE.exe, then try again.",
            sre_binary.display()
        ));
    }
    let mut sre = Command::new(&sre_binary);
    sre.current_dir(&workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::from(sre_stdout))
        .stderr(Stdio::from(sre_stderr));
    #[cfg(windows)]
    sre.creation_flags(CREATE_NO_WINDOW);

    let status = sre.status().map_err(|error| {
        stop_owned_vite(&mut vite_child);
        format!(
            "Could not start {}. See {} for the exact error: {error}",
            sre_binary.display(),
            log_path.display()
        )
    })?;
    stop_owned_vite(&mut vite_child);

    if !status.success() {
        return Err(format!(
            "SRE closed with an error. See {} for the exact error.",
            log_path.display()
        ));
    }

    Ok(())
}

fn acquire_instance_lock(workspace: &Path) -> Result<Option<File>, String> {
    let path = workspace.join("target").join(INSTANCE_LOCK_FILE);
    let parent = path
        .parent()
        .ok_or_else(|| "The local launcher lock path has no parent directory.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;

    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(error) if lock_is_contended(&error) => Ok(None),
        Err(error) => Err(format!("Cannot lock {}: {error}", path.display())),
    }
}

fn lock_is_contended(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::WouldBlock {
        return true;
    }

    // Windows reports an existing byte-range lock as ERROR_LOCK_VIOLATION or
    // ERROR_SHARING_VIOLATION rather than ErrorKind::WouldBlock.
    matches!(error.raw_os_error(), Some(32 | 33))
}

fn stop_owned_vite(vite: &mut Option<Child>) {
    if let Some(child) = vite.as_mut() {
        stop_process_tree(child);
    }
}

fn sre_process_is_running() -> bool {
    #[cfg(windows)]
    {
        let mut tasklist = Command::new("tasklist");
        tasklist
            .args(["/FI", "IMAGENAME eq sre-launcher.exe", "/NH"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW);
        return tasklist
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .any(|line| line.trim_start().starts_with("sre-launcher.exe"))
            })
            .unwrap_or(false);
    }

    #[cfg(not(windows))]
    {
        Command::new("pgrep")
            .args(["-x", "sre-launcher"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn wait_for_vite(vite: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + VITE_STARTUP_TIMEOUT;

    while Instant::now() < deadline {
        if vite_is_listening() {
            return Ok(());
        }
        if let Some(status) = vite
            .try_wait()
            .map_err(|error| format!("Cannot inspect the Vite process: {error}"))?
        {
            return Err(format!(
                "The SRE frontend exited before it became ready ({status})"
            ));
        }
        thread::sleep(Duration::from_millis(250));
    }

    Err(format!(
        "The SRE frontend did not become ready at {VITE_ADDRESS} within {} seconds.",
        VITE_STARTUP_TIMEOUT.as_secs()
    ))
}

fn vite_is_listening() -> bool {
    let address: SocketAddr = VITE_ADDRESS
        .parse()
        .expect("VITE_ADDRESS must be a valid loopback socket address");
    TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
}

fn stop_process_tree(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }

    #[cfg(windows)]
    {
        let mut taskkill = Command::new("taskkill");
        taskkill
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let _ = taskkill.status();
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn workspace_root() -> Result<PathBuf, String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .map_err(|error| format!("Cannot find the SRE workspace: {error}"))?;
    if workspace.join("apps/launcher/package.json").is_file()
        && workspace.join("apps/web/package.json").is_file()
    {
        Ok(workspace)
    } else {
        Err(
            "The SRE source checkout is incomplete; apps/launcher or apps/web is missing."
                .to_owned(),
        )
    }
}

fn create_log(path: &Path) -> Result<File, String> {
    let directory = path
        .parent()
        .ok_or_else(|| "The local launcher log path has no parent directory.".to_owned())?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
    File::create(path).map_err(|error| format!("Cannot create {}: {error}", path.display()))
}

#[cfg(windows)]
fn show_error(message: &str) {
    use windows::{
        core::HSTRING,
        Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK},
    };

    let title = HSTRING::from("SRE Local Launcher");
    let body = HSTRING::from(message);
    // The executable normally has no console, so startup errors need a native dialog.
    unsafe {
        let _ = MessageBoxW(None, &body, &title, MB_ICONERROR | MB_OK);
    }
}

#[cfg(not(windows))]
fn show_error(message: &str) {
    eprintln!("SRE Local Launcher: {message}");
}
