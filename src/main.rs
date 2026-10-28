//! Status line for Claude Code. Reads the session JSON on stdin, prints two lines.

mod git;
mod render;

use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // args_os: args() panics on a non-Unicode argument, including argv[0]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--version")
    {
        println!("{}", option_env!("STATUSLINE_VERSION").unwrap_or("dev"));
        return;
    }

    let mut raw = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut raw);
    let input = render::parse_input(&raw);

    let cwd = render::cwd(&input);
    let branch = if cwd.is_empty() {
        String::new()
    } else {
        git::branch(cwd)
    };

    let windows = cfg!(windows);
    // USERPROFILE / HOME, falling back to the profile folder / passwd entry
    let home = std::env::home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Unix time, for the rate-limit reset countdowns
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);

    let out = render::render(&input, &home, windows, &branch, now);
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
}
