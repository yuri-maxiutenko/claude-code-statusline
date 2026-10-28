//! Status line for Claude Code. Reads the session JSON on stdin, prints two lines.

mod git;
mod options;
mod plan;
mod render;

use options::Command;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // args_os: args() panics on a non-Unicode argument, including argv[0]
    let hidden = match options::parse(std::env::args_os().skip(1)) {
        Command::Version => {
            println!("{}", option_env!("STATUSLINE_VERSION").unwrap_or("dev"));
            return;
        }
        Command::Help => {
            print!("{}", options::help());
            return;
        }
        Command::Run { hidden, warnings } => {
            for warning in warnings {
                eprintln!("claude-code-statusline: {warning}");
            }
            hidden
        }
    };

    let mut raw = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut raw);
    let input = render::parse_input(&raw);

    let cwd = render::cwd(&input);
    let branch = if cwd.is_empty() || hidden.git {
        String::new()
    } else {
        git::branch(cwd)
    };

    let windows = cfg!(windows);
    // USERPROFILE / HOME, falling back to the profile folder / passwd entry
    let home = std::env::home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();

    // The plan isn't in the session JSON, so it comes from Claude Code's config
    let plan = if hidden.plan {
        None
    } else {
        plan::config_path(std::env::var_os("CLAUDE_CONFIG_DIR"), &home)
            .and_then(|path| plan::read(&path))
    };

    // Unix time, for the rate-limit reset countdowns
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);

    let out = render::render(
        &input,
        &home,
        windows,
        &branch,
        plan.as_deref(),
        now,
        &hidden,
    );
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
}
