//! Git branch, change markers (tracked files only) and upstream distance, from
//! a single git call.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Keeps a slow repo (network share, WSL /mnt/c) from hanging every run.
const TIMEOUT: Duration = Duration::from_secs(2);

/// The branch for `dir` as `parse_status` formats it, or an empty string when
/// `dir` isn't a repo or git fails or times out.
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

    // Read on a separate thread so the main one can give up after TIMEOUT.
    // Decoded leniently: a non-UTF-8 file or branch name mustn't hide the branch.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        let read = stdout.read_to_end(&mut out).is_ok();
        let _ = tx.send(read.then(|| String::from_utf8_lossy(&out).into_owned()));
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

/// Turns `git status --porcelain=v2 --branch` output into the branch, with
/// `*` for unstaged and `+` for staged changes, then the commits ahead of and
/// behind its upstream: `master*+ ↑2↓1`.
pub fn parse_status(status: &str) -> String {
    let mut oid = "";
    let mut branch = String::new();
    let (mut ahead, mut behind) = (0, 0);
    let (mut staged, mut unstaged) = (false, false);

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("# branch.oid ") {
            oid = value.trim();
        } else if let Some(value) = line.strip_prefix("# branch.head ") {
            branch = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("# branch.ab ") {
            // "+<ahead> -<behind>", present only when there is an upstream
            let mut counts = value
                .split_whitespace()
                .map(|n| n.get(1..).and_then(|n| n.parse::<u64>().ok()).unwrap_or(0));
            ahead = counts.next().unwrap_or(0);
            behind = counts.next().unwrap_or(0);
        } else if let Some(entry) = line.strip_prefix("1 ").or_else(|| line.strip_prefix("2 ")) {
            // Changed or renamed entry: XY is the index and work tree state
            let mut xy = entry.chars();
            staged |= xy.next().is_some_and(|c| c != '.');
            unstaged |= xy.next().is_some_and(|c| c != '.');
        } else if !line.is_empty() && !line.starts_with('#') {
            // Unmerged entry: still to be resolved in the work tree
            unstaged = true;
        }
    }

    if branch == "(detached)"
        && let Some(short) = oid.get(..7)
    {
        branch = short.to_string();
    }
    if branch.is_empty() {
        return branch;
    }
    if unstaged {
        branch.push('*');
    }
    if staged {
        branch.push('+');
    }
    if ahead > 0 || behind > 0 {
        branch.push(' ');
        if ahead > 0 {
            branch.push_str(&format!("\u{2191}{ahead}"));
        }
        if behind > 0 {
            branch.push_str(&format!("\u{2193}{behind}"));
        }
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
    fn unstaged_changes_add_asterisk() {
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
        assert_eq!(parse_status(&status), "065b1c5+");
    }

    #[test]
    fn staged_changes_add_plus() {
        let status =
            format!("{OID}\n# branch.head main\n1 A. N... 000000 100644 100644 a b new.rs\n");
        assert_eq!(parse_status(&status), "main+");
    }

    #[test]
    fn staged_and_unstaged_changes_add_both() {
        let one = format!("{OID}\n# branch.head main\n1 MM N... 100644 100644 100644 a b x\n");
        assert_eq!(parse_status(&one), "main*+");
        let two = format!(
            "{OID}\n# branch.head main\n1 M. N... 100644 100644 100644 a b x\n1 .D N... 100644 100644 000000 a b y\n"
        );
        assert_eq!(parse_status(&two), "main*+");
    }

    #[test]
    fn unmerged_entry_counts_as_unstaged() {
        let status =
            format!("{OID}\n# branch.head main\nu UU N... 100644 100644 100644 100644 a b c x\n");
        assert_eq!(parse_status(&status), "main*");
    }

    #[test]
    fn upstream_distance_follows_markers() {
        let head = format!("{OID}\n# branch.head main\n# branch.upstream origin/main\n");
        let with_ab = |ab: &str| parse_status(&format!("{head}# branch.ab {ab}\n"));
        assert_eq!(with_ab("+0 -0"), "main");
        assert_eq!(with_ab("+2 -0"), "main \u{2191}2");
        assert_eq!(with_ab("+0 -13"), "main \u{2193}13");
        assert_eq!(with_ab("+2 -1"), "main \u{2191}2\u{2193}1");
        assert_eq!(with_ab("bogus"), "main");

        let dirty = format!("{head}# branch.ab +1 -0\n1 .M N... 100644 100644 100644 a b x\n");
        assert_eq!(parse_status(&dirty), "main* \u{2191}1");
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
