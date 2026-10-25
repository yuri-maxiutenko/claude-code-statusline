//! Status line for Claude Code. Reads the session JSON on stdin, prints two lines.

mod git;
mod render;

use std::io::{Read, Write};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
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
    let home = std::env::var(if windows { "USERPROFILE" } else { "HOME" }).unwrap_or_default();

    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(render::render(&input, &home, windows, &branch).as_bytes());
    let _ = stdout.flush();
}
