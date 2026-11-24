//! Command-line parsing.

use std::fmt;

/// What the scope draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// One control-change number on one MIDI channel.
    Cc { cc: u8, channel: u8 },
    /// Note activity on one channel (not implemented yet).
    Notes { channel: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn to_array(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub port: String,
    pub color: Rgb,
    /// Echo every incoming message to stdout.
    pub verbose: bool,
}

pub const DEFAULT_PORT: &str = "octaglide";
pub const DEFAULT_COLOR: Rgb = Rgb {
    r: 0xff,
    g: 0xff,
    b: 0xff,
};

#[derive(Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Parses arguments, excluding the program name.
pub fn parse<I>(args: I) -> Result<Options, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let mut cc = None;
    let mut channel = None;
    let mut notes = false;
    let mut color = DEFAULT_COLOR;
    let mut port = DEFAULT_PORT.to_owned();
    let mut verbose = true;

    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| UsageError(format!("{name} needs a value")))
        };
        match flag.as_str() {
            "--cc" => cc = Some(parse_u8(&value("--cc")?, "--cc", 127)?),
            "--channel" => {
                let raw = parse_u8(&value("--channel")?, "--channel", 16)?;
                if raw == 0 {
                    return Err(UsageError("--channel is 1-16".into()));
                }
                channel = Some(raw - 1);
            }
            "--color" => color = parse_color(&value("--color")?)?,
            "--port" => port = value("--port")?,
            "--quiet" => verbose = false,
            "--notes" => notes = true,
            "--help" | "-h" => return Err(UsageError(HELP.into())),
            other => {
                return Err(UsageError(format!(
                    "unknown argument '{other}' (try --help)"
                )))
            }
        }
    }

    let channel = channel.unwrap_or(0);
    let mode = if notes {
        Mode::Notes { channel }
    } else {
        let cc = cc.ok_or_else(|| {
            UsageError("--cc is required (or use --notes); try --help".into())
        })?;
        Mode::Cc { cc, channel }
    };

    Ok(Options {
        mode,
        port,
        color,
        verbose,
    })
}

pub const HELP: &str = "\
midi-scope - rolling MIDI monitor

USAGE:
    midi-scope --cc <NUMBER> [--channel <1-16>] [--color <RRGGBB>] [--port <NAME>]
    midi-scope --notes [--channel <1-16>] [--color <RRGGBB>] [--port <NAME>]

OPTIONS:
    --cc <NUMBER>      Control-change number to plot (0-127)
    --channel <1-16>   MIDI channel to listen on (default 1)
    --color <RRGGBB>   Line colour, hex, no leading '#' (default ffffff)
    --port <NAME>      MIDI input port to attach to (default octaglide)
    --quiet            Do not echo incoming messages to the terminal
    --notes            Plot note activity instead of a CC (not implemented)
    -h, --help         Show this help
";

fn parse_u8(raw: &str, name: &str, max: u8) -> Result<u8, UsageError> {
    let value: u8 = raw
        .parse()
        .map_err(|_| UsageError(format!("{name} must be a number, got '{raw}'")))?;
    if value > max {
        return Err(UsageError(format!("{name} must be 0-{max}, got {value}")));
    }
    Ok(value)
}

fn parse_color(raw: &str) -> Result<Rgb, UsageError> {
    let hex = raw.strip_prefix('#').unwrap_or(raw);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(UsageError(format!(
            "--color must be six hex digits (RRGGBB), got '{raw}'"
        )));
    }
    let byte = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16).expect("validated hex digits")
    };
    Ok(Rgb {
        r: byte(0..2),
        g: byte(2..4),
        b: byte(4..6),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_cc_channel_and_color() {
        let options = parse(args(&[
            "--cc", "3", "--channel", "1", "--color", "ff8800",
        ]))
        .unwrap();
        assert_eq!(options.mode, Mode::Cc { cc: 3, channel: 0 });
        assert_eq!(
            options.color,
            Rgb {
                r: 0xff,
                g: 0x88,
                b: 0x00
            }
        );
        assert_eq!(options.port, DEFAULT_PORT);
        assert!(options.verbose, "logging is on unless --quiet is given");
    }

    #[test]
    fn quiet_disables_terminal_logging() {
        let options = parse(args(&["--cc", "3", "--quiet"])).unwrap();
        assert!(!options.verbose);
    }

    #[test]
    fn channel_is_one_based_on_the_command_line() {
        let options = parse(args(&["--cc", "1", "--channel", "16"])).unwrap();
        assert_eq!(options.mode, Mode::Cc { cc: 1, channel: 15 });
    }

    #[test]
    fn channel_defaults_to_one() {
        let options = parse(args(&["--cc", "3"])).unwrap();
        assert_eq!(options.mode, Mode::Cc { cc: 3, channel: 0 });
    }

    #[test]
    fn accepts_color_with_leading_hash() {
        let options = parse(args(&["--cc", "3", "--color", "#00ff00"])).unwrap();
        assert_eq!(
            options.color,
            Rgb {
                r: 0,
                g: 0xff,
                b: 0
            }
        );
    }

    #[test]
    fn notes_mode_is_accepted() {
        let options = parse(args(&["--notes", "--channel", "2"])).unwrap();
        assert_eq!(options.mode, Mode::Notes { channel: 1 });
    }

    #[test]
    fn rejects_missing_cc_without_notes() {
        assert!(parse(args(&[])).is_err());
    }

    #[test]
    fn rejects_channel_zero_and_out_of_range() {
        assert!(parse(args(&["--cc", "1", "--channel", "0"])).is_err());
        assert!(parse(args(&["--cc", "1", "--channel", "17"])).is_err());
    }

    #[test]
    fn rejects_cc_out_of_range() {
        assert!(parse(args(&["--cc", "128"])).is_err());
    }

    #[test]
    fn rejects_bad_color() {
        assert!(parse(args(&["--cc", "1", "--color", "fff"])).is_err());
        assert!(parse(args(&["--cc", "1", "--color", "gggggg"])).is_err());
    }

    #[test]
    fn rejects_unknown_flag() {
        assert!(parse(args(&["--cc", "1", "--bogus"])).is_err());
    }

    #[test]
    fn rejects_flag_without_value() {
        assert!(parse(args(&["--cc"])).is_err());
    }
}
