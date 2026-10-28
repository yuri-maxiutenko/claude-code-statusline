//! Turns the session JSON into the two status lines.

use serde_json::{Map, Value};

use crate::options::Hidden;

/// Parses stdin; anything that isn't a JSON object renders as empty input.
pub fn parse_input(raw: &[u8]) -> Value {
    match serde_json::from_slice::<Value>(raw) {
        Ok(value @ Value::Object(_)) => value,
        _ => Value::Object(Map::new()),
    }
}

/// A non-empty string at `pointer`, if there is one.
fn text<'a>(input: &'a Value, pointer: &str) -> Option<&'a str> {
    input
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// A finite number at `pointer`; missing, null or non-numeric values are skipped.
fn number(input: &Value, pointer: &str) -> Option<f64> {
    input
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
}

/// Whether `pointer` holds `true`; anything else, including a missing value, is false.
fn flag(input: &Value, pointer: &str) -> bool {
    input.pointer(pointer).and_then(Value::as_bool) == Some(true)
}

/// The session's working directory, or an empty string.
pub fn cwd(input: &Value) -> &str {
    text(input, "/workspace/current_dir")
        .or_else(|| text(input, "/cwd"))
        .unwrap_or("")
}

/// Colors `text`, dropping control characters first so a field such as a
/// directory name can't smuggle escape sequences into the terminal.
fn paint(code: u8, text: &str) -> String {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    format!("\x1b[{code}m{text}\x1b[0m")
}

/// Formats an amount to cents, rounding half away from zero on its decimal
/// value rather than on the f64 approximation: 0.145 is stored as
/// 0.14499999…, which `{:.2}` would show as 0.14.
pub fn money(amount: f64) -> String {
    // Display gives the shortest decimal that reads back as the same f64 (so
    // the amount as Claude Code wrote it), and never uses exponent notation
    let text = amount.abs().to_string();
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut digits: Vec<u8> = format!("{int}{frac:0<2.2}").into_bytes();

    if frac.as_bytes().get(2).is_some_and(|&d| d >= b'5') {
        // Carry the round-up through the digits, e.g. 9.995 -> 10.00
        match digits.iter().rposition(|&d| d != b'9') {
            Some(i) => {
                digits[i] += 1;
                digits[i + 1..].fill(b'0');
            }
            None => {
                digits.fill(b'0');
                digits.insert(0, b'1');
            }
        }
    }

    // A negative amount that rounds to zero shows as 0.00, not -0.00
    let sign = if amount < 0.0 && digits.iter().any(|&d| d != b'0') {
        "-"
    } else {
        ""
    };
    let digits = String::from_utf8(digits).expect("ASCII digits");
    let (int, cents) = digits.split_at(digits.len() - 2);
    format!("{sign}{int}.{cents}")
}

/// Replaces the home directory with `~`. Paths are case-insensitive only on
/// Windows, and the next character must be a separator so /home/al doesn't
/// match /home/alex.
pub fn shorten_home(cwd: &str, home: &str, windows: bool) -> String {
    let Some(prefix) = cwd.get(..home.len()).filter(|_| !home.is_empty()) else {
        return cwd.to_string();
    };
    let same = if windows {
        prefix.to_lowercase() == home.to_lowercase()
    } else {
        prefix == home
    };
    let rest = &cwd[home.len()..];
    if same && (rest.is_empty() || rest.starts_with(['\\', '/'])) {
        format!("~{rest}")
    } else {
        cwd.to_string()
    }
}

/// Time from `now` until `reset` (both Unix seconds) in its two largest units,
/// rounded up to the smaller one: `3d4h`, `2h14m`, `9m`. None once `reset` has
/// passed.
pub fn countdown(reset: f64, now: i64) -> Option<String> {
    let secs = reset - now as f64;
    if secs <= 0.0 {
        return None;
    }
    let mins = (secs / 60.0).ceil() as i64;
    Some(if mins < 60 {
        format!("{mins}m")
    } else if mins < 1440 {
        format!("{}h{}m", mins / 60, mins % 60)
    } else {
        // Whole days drop the minutes, so round up to the hour instead
        let hours = (mins + 59) / 60;
        format!("{}d{}h", hours / 24, hours % 24)
    })
}

/// `now` is the current Unix time in seconds, for the rate-limit countdowns.
/// Segments in `hidden` are left out, and the second line too when both `dir`
/// and `git` are hidden, so its height doesn't depend on being in a repo.
pub fn render(
    input: &Value,
    home: &str,
    windows: bool,
    branch: &str,
    now: i64,
    hidden: &Hidden,
) -> String {
    let mut parts = Vec::new();

    // Model, with the effort level and fast mode in parentheses
    if let Some(model) = text(input, "/model/display_name") {
        let effort = if hidden.effort {
            None
        } else {
            text(input, "/effort/level")
        };
        let fast = !hidden.fast && flag(input, "/fast_mode");
        let label = match (effort, fast) {
            (Some(level), true) => format!("{model} ({level}, fast)"),
            (Some(level), false) => format!("{model} ({level})"),
            (None, true) => format!("{model} (fast)"),
            (None, false) => model.to_string(),
        };
        parts.push(paint(33, &label));
    }

    // Output style (only shown when not the default)
    if !hidden.style
        && let Some(style) = text(input, "/output_style/name").filter(|s| *s != "default")
    {
        parts.push(paint(37, style));
    }

    // Context usage, drawn as a bar with the consumed share filled in
    if !hidden.context
        && let Some(used) = number(input, "/context_window/used_percentage")
    {
        const WIDTH: i64 = 10;
        let pct = used.round() as i64;
        let filled = pct
            .saturating_mul(WIDTH)
            .saturating_add(50)
            .div_euclid(100)
            .clamp(0, WIDTH);

        // Green while there is room, amber above 40%, red above 90%
        let color = if pct > 90 {
            31
        } else if pct > 40 {
            33
        } else {
            32
        };

        let on = "\u{2588}".repeat(filled as usize);
        let off = "\u{2591}".repeat((WIDTH - filled) as usize);
        parts.push(format!(
            "{}\x1b[90m{off}\x1b[0m {}",
            paint(color, &on),
            paint(color, &format!("{pct}%"))
        ));
    }

    // Session cost, when Claude Code reports one
    if !hidden.cost
        && let Some(cost) = number(input, "/cost/total_cost_usd")
    {
        parts.push(paint(32, &format!("${}", money(cost))));
    }

    // Claude.ai subscription rate limits (5h / 7d), white until they pass 90%,
    // each followed by the time left until its window resets
    for (name, window) in [
        ("5h", "/rate_limits/five_hour"),
        ("7d", "/rate_limits/seven_day"),
    ] {
        if !hidden.limits
            && let Some(used) = number(input, &format!("{window}/used_percentage"))
        {
            let n = used.round() as i64;
            parts.push(paint(if n > 90 { 31 } else { 37 }, &format!("{name}:{n}%")));
            if !hidden.resets
                && let Some(left) = number(input, &format!("{window}/resets_at"))
                    .and_then(|reset| countdown(reset, now))
            {
                parts.push(paint(90, &format!("\u{21bb}{left}")));
            }
        }
    }

    // Second line: directory and git branch
    if hidden.dir && hidden.git {
        return parts.join(" ");
    }
    let mut repo = Vec::new();
    if !hidden.dir {
        repo.push(paint(34, &shorten_home(cwd(input), home, windows)));
    }
    if !hidden.git && !branch.is_empty() {
        repo.push(paint(32, &format!("({branch})")));
    }

    format!("{}\n{}", parts.join(" "), repo.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = r"C:\Users\alex";
    const NOW: i64 = 1_738_400_000;

    fn input(json: &str) -> Value {
        parse_input(json.as_bytes())
    }

    /// The output with ANSI color codes removed.
    fn plain(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                chars.by_ref().find(|&c| c == 'm');
            } else {
                out.push(c);
            }
        }
        out
    }

    fn first_line(json: &str) -> String {
        let out = render(&input(json), HOME, true, "", NOW, &Hidden::default());
        plain(out.split('\n').next().unwrap())
    }

    fn bar(pct: f64) -> String {
        first_line(&format!(
            r#"{{"context_window":{{"used_percentage":{pct}}}}}"#
        ))
    }

    #[test]
    fn full_input_matches_exact_output() {
        let json = r#"{
            "model": {"display_name": "Opus 5.5"},
            "effort": {"level": "high"},
            "fast_mode": false,
            "workspace": {"current_dir": "C:\\Users\\alex\\src\\app"},
            "context_window": {"used_percentage": 45.5},
            "cost": {"total_cost_usd": 1.235},
            "rate_limits": {
                "five_hour": {"used_percentage": 92.4, "resets_at": 1738408040},
                "seven_day": {"used_percentage": 30, "resets_at": 1738673600}
            }
        }"#;
        let expected = concat!(
            "\x1b[33mOpus 5.5 (high)\x1b[0m ",
            "\x1b[33m\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\x1b[0m",
            "\x1b[90m\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\x1b[0m \x1b[33m46%\x1b[0m ",
            "\x1b[32m$1.24\x1b[0m \x1b[31m5h:92%\x1b[0m \x1b[90m\u{21bb}2h14m\x1b[0m ",
            "\x1b[37m7d:30%\x1b[0m \x1b[90m\u{21bb}3d4h\x1b[0m\n",
            "\x1b[34m~\\src\\app\x1b[0m \x1b[32m(master*+ \u{2191}2)\x1b[0m",
        );
        assert_eq!(
            render(
                &input(json),
                HOME,
                true,
                "master*+ \u{2191}2",
                NOW,
                &Hidden::default()
            ),
            expected
        );
    }

    #[test]
    fn hidden_segments_are_left_out() {
        let json = r#"{
            "model": {"display_name": "Opus"},
            "effort": {"level": "high"},
            "fast_mode": true,
            "output_style": {"name": "Explanatory"},
            "workspace": {"current_dir": "/w"},
            "context_window": {"used_percentage": 50},
            "cost": {"total_cost_usd": 1},
            "rate_limits": {"five_hour": {"used_percentage": 10, "resets_at": 1738400600}}
        }"#;
        let show = |hidden: Hidden, branch: &str| {
            plain(&render(&input(json), HOME, false, branch, NOW, &hidden))
        };
        let bar = "\u{2588}".repeat(5) + &"\u{2591}".repeat(5) + " 50%";

        assert_eq!(
            show(Hidden::default(), "main"),
            format!("Opus (high, fast) Explanatory {bar} $1.00 5h:10% \u{21bb}10m\n/w (main)")
        );
        let none = Hidden {
            effort: true,
            fast: true,
            style: true,
            context: true,
            cost: true,
            resets: true,
            ..Hidden::default()
        };
        assert_eq!(show(none, "main"), "Opus 5h:10%\n/w (main)");
        let first = Hidden {
            effort: true,
            limits: true,
            dir: true,
            ..Hidden::default()
        };
        assert_eq!(
            show(first, "main"),
            format!("Opus (fast) Explanatory {bar} $1.00\n(main)")
        );
        // Hiding only dir keeps the second line, even outside a repo
        let dir = || Hidden {
            dir: true,
            ..Hidden::default()
        };
        assert!(show(dir(), "").ends_with('\n'));
        let no_repo = Hidden {
            dir: true,
            git: true,
            ..Hidden::default()
        };
        assert!(!show(no_repo, "main").contains('\n'));
    }

    #[test]
    fn bar_fills_by_rounded_percentage() {
        assert_eq!(bar(0.0), "\u{2591}".repeat(10) + " 0%");
        assert_eq!(bar(4.4), "\u{2591}".repeat(10) + " 4%");
        assert_eq!(
            bar(4.9),
            "\u{2588}".to_string() + &"\u{2591}".repeat(9) + " 5%"
        );
        assert_eq!(
            bar(45.5),
            "\u{2588}".repeat(5) + &"\u{2591}".repeat(5) + " 46%"
        );
        assert_eq!(bar(95.0), "\u{2588}".repeat(10) + " 95%");
        assert_eq!(bar(150.0), "\u{2588}".repeat(10) + " 150%");
    }

    #[test]
    fn bar_color_follows_thresholds() {
        let color = |pct: f64| {
            let json = format!(r#"{{"context_window":{{"used_percentage":{pct}}}}}"#);
            render(&input(&json), HOME, true, "", NOW, &Hidden::default())[..5].to_string()
        };
        assert_eq!(color(40.0), "\x1b[32m");
        assert_eq!(color(41.0), "\x1b[33m");
        assert_eq!(color(90.0), "\x1b[33m");
        assert_eq!(color(91.0), "\x1b[31m");
    }

    #[test]
    fn invalid_or_empty_input_renders_empty_lines() {
        for raw in ["", "null", "42", "not json", "[1,2]"] {
            assert_eq!(
                render(&input(raw), HOME, true, "", NOW, &Hidden::default()),
                "\n\x1b[34m\x1b[0m",
                "input {raw:?}"
            );
        }
    }

    #[test]
    fn non_numeric_values_are_skipped() {
        let json = r#"{
            "context_window": {"used_percentage": "x"},
            "cost": {"total_cost_usd": null},
            "rate_limits": {"five_hour": {"used_percentage": "oops"}, "seven_day": {"used_percentage": 12}}
        }"#;
        assert_eq!(first_line(json), "7d:12%");
    }

    #[test]
    fn missing_effort_shows_model_only() {
        assert_eq!(
            first_line(r#"{"model":{"display_name":"Sonnet 5.5"}}"#),
            "Sonnet 5.5"
        );
    }

    #[test]
    fn fast_mode_follows_effort_in_parentheses() {
        let model = r#""model":{"display_name":"Opus"}"#;
        assert_eq!(
            first_line(&format!(
                r#"{{{model},"effort":{{"level":"high"}},"fast_mode":true}}"#
            )),
            "Opus (high, fast)"
        );
        assert_eq!(
            first_line(&format!(r#"{{{model},"fast_mode":true}}"#)),
            "Opus (fast)"
        );
        assert_eq!(
            first_line(&format!(
                r#"{{{model},"effort":{{"level":"high"}},"fast_mode":false}}"#
            )),
            "Opus (high)"
        );
        assert_eq!(
            first_line(&format!(r#"{{{model},"fast_mode":"yes"}}"#)),
            "Opus"
        );
        assert_eq!(first_line(r#"{"fast_mode":true}"#), "");
    }

    #[test]
    fn output_style_shown_only_when_not_default() {
        assert_eq!(first_line(r#"{"output_style":{"name":"default"}}"#), "");
        assert_eq!(
            first_line(r#"{"output_style":{"name":"Explanatory"}}"#),
            "Explanatory"
        );
    }

    #[test]
    fn cwd_falls_back_to_top_level_field() {
        let out = render(
            &input(r#"{"cwd":"D:\\work"}"#),
            HOME,
            true,
            "",
            NOW,
            &Hidden::default(),
        );
        assert_eq!(plain(&out), "\nD:\\work");
    }

    #[test]
    fn home_is_case_insensitive_on_windows_only() {
        assert_eq!(shorten_home(r"c:\users\ALEX\x", HOME, true), r"~\x");
        assert_eq!(
            shorten_home("/Home/alex/x", "/home/alex", false),
            "/Home/alex/x"
        );
        assert_eq!(shorten_home("/home/alex/x", "/home/alex", false), "~/x");
    }

    #[test]
    fn home_must_end_at_a_separator() {
        assert_eq!(
            shorten_home(r"C:\Users\alex2\x", HOME, true),
            r"C:\Users\alex2\x"
        );
        assert_eq!(
            shorten_home("/home/alexander", "/home/alex", false),
            "/home/alexander"
        );
    }

    #[test]
    fn home_itself_becomes_tilde() {
        assert_eq!(shorten_home(HOME, HOME, true), "~");
    }

    #[test]
    fn empty_home_or_cwd_is_left_alone() {
        assert_eq!(shorten_home("/srv/app", "", false), "/srv/app");
        assert_eq!(shorten_home("", HOME, true), "");
    }

    #[test]
    fn money_rounds_decimal_ties_away_from_zero() {
        // Each of these is stored just below the tie, so {:.2} rounds it down
        assert_eq!(money(0.145), "0.15");
        assert_eq!(money(1.005), "1.01");
        assert_eq!(money(2.675), "2.68");
        // Exact binary ties, which {:.2} rounds to even
        assert_eq!(money(0.125), "0.13");
        assert_eq!(money(0.375), "0.38");
        assert_eq!(money(1.235), "1.24");
    }

    #[test]
    fn money_rounds_non_ties_to_nearest_cent() {
        assert_eq!(money(0.994), "0.99");
        assert_eq!(money(0.1449), "0.14");
        assert_eq!(money(0.0000001), "0.00");
    }

    #[test]
    fn money_carries_round_up_into_whole_dollars() {
        assert_eq!(money(9.995), "10.00");
        assert_eq!(money(99.999), "100.00");
        assert_eq!(money(0.996), "1.00");
    }

    #[test]
    fn money_pads_to_two_decimals() {
        assert_eq!(money(0.0), "0.00");
        assert_eq!(money(5.0), "5.00");
        assert_eq!(money(0.1), "0.10");
        assert_eq!(money(12345.6), "12345.60");
    }

    #[test]
    fn money_signs_only_non_zero_negatives() {
        assert_eq!(money(-0.0), "0.00");
        assert_eq!(money(-0.001), "0.00");
        assert_eq!(money(-0.145), "-0.15");
    }

    #[test]
    fn cost_is_shown_with_decimal_rounding() {
        assert_eq!(first_line(r#"{"cost":{"total_cost_usd":0.145}}"#), "$0.15");
    }

    #[test]
    fn countdown_rounds_up_to_the_minute_in_two_units() {
        let at = |secs: i64| countdown((NOW + secs) as f64, NOW);
        assert_eq!(at(1).as_deref(), Some("1m"));
        assert_eq!(at(60).as_deref(), Some("1m"));
        assert_eq!(at(61).as_deref(), Some("2m"));
        assert_eq!(at(3_599).as_deref(), Some("1h0m"));
        assert_eq!(at(8_040).as_deref(), Some("2h14m"));
        assert_eq!(at(86_340).as_deref(), Some("23h59m"));
        assert_eq!(at(86_400).as_deref(), Some("1d0h"));
        assert_eq!(at(86_401).as_deref(), Some("1d1h"));
        assert_eq!(at(273_600).as_deref(), Some("3d4h"));
        assert_eq!(at(273_600 + 1_800).as_deref(), Some("3d5h"));
        assert_eq!(at(0), None);
        assert_eq!(at(-30), None);
    }

    #[test]
    fn reset_countdown_needs_a_future_reset_time() {
        let limits = |reset: &str| {
            first_line(&format!(
                r#"{{"rate_limits":{{"five_hour":{{"used_percentage":10,"resets_at":{reset}}}}}}}"#
            ))
        };
        assert_eq!(limits(&(NOW + 600).to_string()), "5h:10% \u{21bb}10m");
        assert_eq!(limits(&(NOW - 600).to_string()), "5h:10%");
        assert_eq!(limits(r#""soon""#), "5h:10%");
        assert_eq!(limits("null"), "5h:10%");
    }

    #[test]
    fn control_characters_are_dropped_from_text_fields() {
        let json = r#"{
            "model": {"display_name": "Opus\u001b[2J"},
            "effort": {"level": "hi\u0007gh"},
            "output_style": {"name": "Ex\u009bplain"},
            "workspace": {"current_dir": "/w/x\u001b]8;;http://e\u0007y"}
        }"#;
        let expected = concat!(
            "\x1b[33mOpus[2J (high)\x1b[0m \x1b[37mExplain\x1b[0m\n",
            "\x1b[34m/w/x]8;;http://ey\x1b[0m \x1b[32m(main)\x1b[0m",
        );
        assert_eq!(
            render(&input(json), HOME, false, "ma\nin", NOW, &Hidden::default()),
            expected
        );
    }
}
