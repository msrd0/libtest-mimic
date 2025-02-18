use std::str::FromStr;

use clap::Parser;

/// Command line arguments.
///
/// This type represents everything the user can specify via CLI args. The main
/// method is [`from_args`][Arguments::from_args] which reads the global
/// `std::env::args()` and parses them into this type.
///
/// `libtest-mimic` supports a subset of all args/flags supported by the
/// official test harness. There are also some other minor CLI differences, but
/// the main use cases should work exactly like with the built-in harness.
#[derive(Parser, Debug, Clone, Default)]
#[clap(
    help_template = "USAGE: [OPTIONS] [FILTER]\n\n{all-args}\n\n\n{after-help}",
    disable_version_flag = true,
    after_help = "By default, all tests are run in parallel. This can be altered with the \n\
        --test-threads flag when running tests (set it to 1).",
)]
pub struct Arguments {
    // ============== FLAGS ===================================================
    /// Run ignored and non-ignored tests.
    #[clap(long = "--include-ignored", help = "Run ignored tests")]
    pub include_ignored: bool,

    /// Run only ignored tests.
    #[clap(long = "--ignored", help = "Run ignored tests")]
    pub ignored: bool,

    /// Run tests, but not benchmarks.
    #[clap(
        long = "--test",
        conflicts_with = "bench",
        help = "Run tests and not benchmarks",
    )]
    pub test: bool,

    /// Run benchmarks, but not tests.
    #[clap(long = "--bench", help = "Run benchmarks instead of tests")]
    pub bench: bool,

    /// Only list all tests and benchmarks.
    #[clap(long = "--list", help = "List all tests and benchmarks")]
    pub list: bool,

    /// No-op, ignored (libtest-mimic always runs in no-capture mode)
    #[clap(long = "--nocapture", help = "No-op (libtest-mimic always runs in no-capture mode)")]
    pub nocapture: bool,

    /// If set, filters are matched exactly rather than by substring.
    #[clap(
        long = "--exact",
        help = "Exactly match filters rather than by substring",
    )]
    pub exact: bool,

    /// If set, display only one character per test instead of one line.
    /// Especially useful for huge test suites.
    ///
    /// This is an alias for `--format=terse`. If this is set, `format` is
    /// `None`.
    #[clap(
        short = 'q',
        long = "--quiet",
        conflicts_with = "format",
        help = "Display one character per test instead of one line. Alias to --format=terse",
    )]
    pub quiet: bool,

    // ============== OPTIONS =================================================
    /// Number of threads used for parallel testing.
    #[clap(
        long = "--test-threads",
        help = "Number of threads used for running tests in parallel. If set to 1, \n\
            all tests are run in the main thread.",
    )]
    pub test_threads: Option<usize>,

    /// Path of the logfile. If specified, everything will be written into the
    /// file instead of stdout.
    #[clap(
        long = "--logfile",
        value_name = "PATH",
        help = "Write logs to the specified file instead of stdout",
    )]
    pub logfile: Option<String>,

    /// A list of filters. Tests whose names contain parts of any of these
    /// filters are skipped.
    #[clap(
        long = "--skip",
        value_name = "FILTER",
        number_of_values = 1,
        help = "Skip tests whose names contain FILTER (this flag can be used multiple times)",
    )]
    pub skip: Vec<String>,

    /// Specifies whether or not to color the output.
    #[clap(
        long = "--color",
        possible_values = &["auto", "always", "never"],
        value_name = "auto|always|never",
        help = "Configure coloring of output: \n\
            - auto = colorize if stdout is a tty and tests are run on serially (default)\n\
            - always = always colorize output\n\
            - never = never colorize output\n",
    )]
    pub color: Option<ColorSetting>,

    /// Specifies the format of the output.
    #[clap(
        long = "--format",
        possible_values = &["pretty", "terse"],
        value_name = "pretty|terse|json",
        help = "Configure formatting of output: \n\
            - pretty = Print verbose output\n\
            - terse = Display one character per test\n",
    )]
    pub format: Option<FormatSetting>,

    // ============== POSITIONAL VALUES =======================================
    /// Filter string. Only tests which contain this string are run.
    #[clap(
        name = "FILTER",
        help = "The FILTER string is tested against the name of all tests, and only those tests \
                whose names contain the filter are run.",
    )]
    pub filter: Option<String>,
}

impl Arguments {
    /// Parses the global CLI arguments given to the application.
    ///
    /// If the parsing fails (due to incorrect CLI args), an error is shown and
    /// the application exits. If help is requested (`-h` or `--help`), a help
    /// message is shown and the application exits, too.
    pub fn from_args() -> Self {
        Parser::parse()
    }

    /// Like `from_args()`, but operates on an explicit iterator and not the
    /// global arguments. Note that the first element is the executable name!
    pub fn from_iter<I>(iter: I) -> Self
    where
        Self: Sized,
        I: IntoIterator,
        I::Item: Into<std::ffi::OsString> + Clone,
    {
        Parser::parse_from(iter)
    }
}

/// Possible values for the `--color` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSetting {
    /// Colorize output if stdout is a tty and tests are run on serially
    /// (default).
    Auto,

    /// Always colorize output.
    Always,

    /// Never colorize output.
    Never,
}

impl Default for ColorSetting {
    fn default() -> Self {
        ColorSetting::Auto
    }
}

impl FromStr for ColorSetting {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(ColorSetting::Auto),
            "always" => Ok(ColorSetting::Always),
            "never" => Ok(ColorSetting::Never),
            _ => Err("invalid color setting"),
        }
    }
}

/// Possible values for the `--format` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatSetting {
    /// One line per test. Output for humans. (default)
    Pretty,

    /// One character per test. Usefull for test suites with many tests.
    Terse,
}

impl Default for FormatSetting {
    fn default() -> Self {
        FormatSetting::Pretty
    }
}

impl FromStr for FormatSetting {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pretty" => Ok(FormatSetting::Pretty),
            "terse" => Ok(FormatSetting::Terse),
            _ => Err("invalid output format"),
        }
    }
}
