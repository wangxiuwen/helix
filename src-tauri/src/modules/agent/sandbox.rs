//! Process Sandbox — Secure execution environment
//!
//! Ported from OpenClaw's process-supervisor.
//! Provides timeouts, process tree killing, and basic resource monitoring
//! using the `sysinfo` crate.

use std::process::Stdio;
use std::time::Duration;
use sysinfo::{System, Pid};
use tokio::io::{AsyncReadExt, BufReader};
use tracing::{info, warn};

pub struct SandboxOptions {
    pub timeout_secs: u64,
    pub max_output_bytes: usize,
    pub max_memory_mb: Option<u64>,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            max_output_bytes: 512 * 1024, // 512KB max output
            max_memory_mb: Some(512),     // 512MB max memory
        }
    }
}

pub struct SandboxResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub killed_by_sandbox: bool,
    pub kill_reason: Option<String>,
}

/// Helper: Kill a process and all its children.
/// In macOS/Linux, it's safer to use process groups, but sysinfo gives us a cross-platform way to find children.
fn kill_process_tree(root_pid: u32) {
    let mut sys = System::new_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    
    // Find all descendants
    let mut to_kill = vec![root_pid];
    let mut i = 0;
    while i < to_kill.len() {
        let current_pid = to_kill[i];
        for (pid, process) in sys.processes() {
            if let Some(parent) = process.parent() {
                if parent.as_u32() == current_pid && !to_kill.contains(&pid.as_u32()) {
                    to_kill.push(pid.as_u32());
                }
            }
        }
        i += 1;
    }

    // Kill from bottom up (children first)
    for pid in to_kill.into_iter().rev() {
        if let Some(process) = sys.process(Pid::from_u32(pid)) {
            info!("Sandbox: Killing process {}", pid);
            process.kill();
        }
    }
}

/// Execute a command within the sandbox
pub async fn exec_sandboxed(command: &str, working_dir: &str, opts: SandboxOptions) -> Result<SandboxResult, String> {
    info!("Sandbox executing: {} (dir: {})", &command[..command.len().min(50)], working_dir);

    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true) // Ensure it dies if we drop the future
        .spawn()
        .map_err(|e| format!("Failed to spawn process: {}", e))?;

    let child_pid = child.id().ok_or("Failed to get child PID")?;
    
    let mut stdout_reader = BufReader::new(child.stdout.take().unwrap());
    let mut stderr_reader = BufReader::new(child.stderr.take().unwrap());

    let mut stdout = String::new();
    let mut stderr = String::new();
    
    let mut killed_by_sandbox = false;
    let mut kill_reason = None;

    // We will poll the process memory and output sizes in a loop
    let mut sys_monitor = System::new();
    let start_time = tokio::time::Instant::now();
    let timeout_duration = Duration::from_secs(opts.timeout_secs);

    let mut stdout_buf = [0u8; 4096];
    let mut stderr_buf = [0u8; 4096];

    let exit_status = loop {
        // 1. Check if process exited
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }

        // 2. Check timeout
        if start_time.elapsed() > timeout_duration {
            killed_by_sandbox = true;
            kill_reason = Some(format!("Timeout of {}s exceeded", opts.timeout_secs));
            break tokio::process::Command::new("false").status().await.unwrap(); // Dummy status, we'll override code
        }

        // 3. Monitor Resource Usage
        if let Some(max_mem) = opts.max_memory_mb {
            // Need to refresh to get latest memory
            sys_monitor.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(child_pid)]), true);
            if let Some(process) = sys_monitor.process(Pid::from_u32(child_pid)) {
                let mem_mb = process.memory() / 1024 / 1024;
                if mem_mb > max_mem {
                    killed_by_sandbox = true;
                    kill_reason = Some(format!("Memory limit exceeded ({}MB > {}MB)", mem_mb, max_mem));
                    break tokio::process::Command::new("false").status().await.unwrap();
                }
            }
        }

        // 4. Read available output (non-blocking chunk)
        let stdout_f = tokio::time::timeout(Duration::from_millis(10), stdout_reader.read(&mut stdout_buf));
        if let Ok(Ok(n)) = stdout_f.await {
            if n > 0 {
                stdout.push_str(&String::from_utf8_lossy(&stdout_buf[..n]));
            }
        }

        let stderr_f = tokio::time::timeout(Duration::from_millis(10), stderr_reader.read(&mut stderr_buf));
        if let Ok(Ok(n)) = stderr_f.await {
            if n > 0 {
                stderr.push_str(&String::from_utf8_lossy(&stderr_buf[..n]));
            }
        }

        // 5. Check output limits
        if stdout.len() + stderr.len() > opts.max_output_bytes {
            killed_by_sandbox = true;
            kill_reason = Some(format!("Output exceeded max {} bytes", opts.max_output_bytes));
            break tokio::process::Command::new("false").status().await.unwrap();
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    // If sandbox triggered a kill, we need to nuke the tree
    if killed_by_sandbox {
        warn!("Sandbox kill triggered: {:?}", kill_reason);
        kill_process_tree(child_pid);
        
        // Try to wait for it one last time, or kill the immediate child
        let _ = child.kill().await;
    } else {
        // Read remaining output
        let _ = stdout_reader.read_to_string(&mut stdout).await;
        let _ = stderr_reader.read_to_string(&mut stderr).await;
    }

    // Prepare final output string (truncate if too long, though we already bounded it)
    if stdout.len() > opts.max_output_bytes {
        stdout.truncate(opts.max_output_bytes);
        stdout.push_str("\n...[truncated by sandbox]");
    }
    if stderr.len() > opts.max_output_bytes {
        stderr.truncate(opts.max_output_bytes);
        stderr.push_str("\n...[truncated by sandbox]");
    }

    let code = if killed_by_sandbox {
        -9 // SIGKILL equivalent
    } else {
        exit_status.code().unwrap_or(-1)
    };

    Ok(SandboxResult {
        exit_code: code,
        stdout,
        stderr,
        killed_by_sandbox,
        kill_reason,
    })
}
