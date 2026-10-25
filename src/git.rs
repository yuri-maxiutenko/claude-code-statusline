//! Git branch + dirty indicator (tracked changes only), from a single git call.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Keeps a slow repo (network share, WSL /mnt/c) from hanging every run.
const TIMEOUT: Duration = Duration::from_secs(2);

/// The branch for `dir`, with `*` when tracked files changed, or an empty
/// string when `dir` isn't a repo or git fails or times out.
pub fn branch(dir: &str) -> String {
    run_status(dir)
        .map(|status| parse_status(&status))
        .unwrap_or_default()
}

fn run_status(dir: &str) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args([
        "-C",
        dir,
        "--no-optional-locks",
        "status",
        "--porcelain=v2",
        "--branch",
        "--untracked-files=no",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;

    // Read on a separate thread so the main one can give up after TIMEOUT
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = String::new();
        let read = stdout.read_to_string(&mut out).is_ok();
        let _ = tx.send(read.then_some(out));
    });

    match rx.recv_timeout(TIMEOUT) {
        Ok(Some(out)) => child.wait().ok()?.success().then_some(out),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Turns `git status --porcelain=v2 --branch` output into `branch` or `branch*`.
pub fn parse_status(status: &str) -> String {
    let mut oid = "";
    let mut branch = String::new();
    let mut dirty = false;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("# branch.oid ") {
            oid = value.trim();
        } else if let Some(value) = line.strip_prefix("# branch.head ") {
            branch = value.trim().to_string();
        } else if !line.is_empty() && !line.starts_with('#') {
            dirty = true;
        }
    }

    if branch == "(detached)"
        && let Some(short) = oid.get(..7)
    {
        branch = short.to_string();
    }
    if !branch.is_empty() && dirty {
        branch.push('*');
    }
    branch
}

#[cfg(test)]
mod tests {
    use super::parse_status;

    const OID: &str = "# branch.oid 065b1c52d0c1e7a3b9f8e2d4c6a1b3e5f7d9c0a2";

    #[test]
    fn clean_repo_shows_branch() {
        let status = format!("{OID}\n# branch.head master\n# branch.upstream origin/master\n");
        assert_eq!(parse_status(&status), "master");
    }

    #[test]
    fn changed_files_add_asterisk() {
        let status = format!(
            "{OID}\n# branch.head fix/DHS-1\n1 .M N... 100644 100644 100644 a b src/main.rs\n"
        );
        assert_eq!(parse_status(&status), "fix/DHS-1*");
    }

    #[test]
    fn detached_head_shows_short_hash() {
        let status = format!("{OID}\n# branch.head (detached)\n");
        assert_eq!(parse_status(&status), "065b1c5");
    }

    #[test]
    fn detached_head_with_changes() {
        let status =
            format!("{OID}\n# branch.head (detached)\n2 R. N... 1 2 3 a b R100 new\told\n");
        assert_eq!(parse_status(&status), "065b1c5*");
    }

    #[test]
    fn empty_output_shows_nothing() {
        assert_eq!(parse_status(""), "");
    }

    #[test]
    fn changes_without_branch_show_nothing() {
        assert_eq!(parse_status("1 .M N... 100644 100644 100644 a b x\n"), "");
    }
}
