//! Command-line options: `--version`, `--help` and `--hide <segments>`.

use std::ffi::OsString;

/// Segments `--hide` can leave out, in display order.
pub const SEGMENTS: [&str; 9] = [
    "effort", "fast", "style", "context", "cost", "limits", "resets", "dir", "git",
];

/// Which segments to leave out; everything is shown by default.
#[derive(Debug, Default, PartialEq)]
pub struct Hidden {
    pub effort: bool,
    pub fast: bool,
    pub style: bool,
    pub context: bool,
    pub cost: bool,
    /// The 5h / 7d percentages, and their countdowns with them
    pub limits: bool,
    pub resets: bool,
    pub dir: bool,
    pub git: bool,
}

impl Hidden {
    /// Hides the segment called `name`; false if there is no such segment.
    fn hide(&mut self, name: &str) -> bool {
        let segment = match name {
            "effort" => &mut self.effort,
            "fast" => &mut self.fast,
            "style" => &mut self.style,
            "context" => &mut self.context,
            "cost" => &mut self.cost,
            "limits" => &mut self.limits,
            "resets" => &mut self.resets,
            "dir" => &mut self.dir,
            "git" => &mut self.git,
            _ => return false,
        };
        *segment = true;
        true
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Version,
    Help,
    /// Render the status line. Warnings go to stderr: a mistake in the
    /// arguments mustn't blank the status line.
    Run {
        hidden: Hidden,
        warnings: Vec<String>,
    },
}

/// Parses the arguments after the program name. `--hide` takes a
/// comma-separated list, as `--hide a,b` or `--hide=a,b`, and may repeat.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Command {
    let mut hidden = Hidden::default();
    let mut warnings = Vec::new();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        let list = match arg.to_str() {
            Some("--version") => return Command::Version,
            Some("--help") => return Command::Help,
            Some("--hide") => match args.next() {
                Some(list) => list,
                None => {
                    warnings.push("--hide needs a list of segments".to_string());
                    continue;
                }
            },
            Some(arg) if arg.starts_with("--hide=") => OsString::from(&arg["--hide=".len()..]),
            _ => {
                warnings.push(format!("unknown argument {arg:?}"));
                continue;
            }
        };

        let list = list.to_string_lossy();
        for name in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if !hidden.hide(name) {
                warnings.push(format!("unknown segment {name:?} in --hide"));
            }
        }
    }

    Command::Run { hidden, warnings }
}

pub fn help() -> String {
    format!(
        "\
A Claude Code status line. Reads the session JSON on stdin and prints the status line.

Usage: claude-code-statusline [--hide <segments>]

Options:
  --hide <segments>  Comma-separated segments to leave out:
                     {}
  --version          Print the version
  --help             Print this help
",
        SEGMENTS.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Command {
        parse(args.iter().map(OsString::from))
    }

    fn hidden(args: &[&str]) -> Hidden {
        match run(args) {
            Command::Run { hidden, warnings } => {
                assert_eq!(warnings, Vec::<String>::new());
                hidden
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn no_arguments_hide_nothing() {
        assert_eq!(hidden(&[]), Hidden::default());
    }

    #[test]
    fn hide_takes_a_list_in_either_form_and_repeats() {
        let expected = Hidden {
            cost: true,
            git: true,
            resets: true,
            ..Hidden::default()
        };
        assert_eq!(hidden(&["--hide", "cost,git", "--hide=resets"]), expected);
        assert_eq!(
            hidden(&["--hide", " cost , ,git,", "--hide", "resets"]),
            expected
        );
    }

    #[test]
    fn every_listed_segment_can_be_hidden() {
        let all = hidden(&["--hide", &SEGMENTS.join(",")]);
        assert_eq!(
            all,
            Hidden {
                effort: true,
                fast: true,
                style: true,
                context: true,
                cost: true,
                limits: true,
                resets: true,
                dir: true,
                git: true,
            }
        );
    }

    #[test]
    fn mistakes_warn_but_still_run() {
        let Command::Run { hidden, warnings } =
            run(&["--hide", "cots,cost", "--verbose", "--hide"])
        else {
            panic!("expected Run");
        };
        assert_eq!(
            hidden,
            Hidden {
                cost: true,
                ..Hidden::default()
            }
        );
        assert_eq!(
            warnings,
            [
                r#"unknown segment "cots" in --hide"#,
                r#"unknown argument "--verbose""#,
                "--hide needs a list of segments",
            ]
        );
    }

    #[test]
    fn version_and_help_win() {
        assert_eq!(run(&["--version"]), Command::Version);
        assert_eq!(run(&["--hide", "cost", "--help"]), Command::Help);
    }

    #[test]
    fn help_lists_every_segment() {
        assert!(help().contains(&SEGMENTS.join(", ")));
    }
}
