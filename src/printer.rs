use std::fs::File;

use termcolor::{Ansi, Color, ColorChoice, ColorSpec, NoColor, StandardStream, WriteColor};

use ::{Arguments, ColorSetting, Conclusion, FormatSetting, Outcome, Test};

pub(crate) struct Printer {
    out: Box<dyn WriteColor>,
    format: FormatSetting,
    name_width: usize,
    kind_width: usize,
}

impl Printer {
    /// Creates a new printer configured by the given arguments (`format`,
    /// `color` and `logfile` options).
    pub(crate) fn new<D>(args: &Arguments, tests: &[Test<D>]) -> Self {
        let color_arg = args.color.unwrap_or(ColorSetting::Auto);

        // Determine target of all output
        let out = if let Some(logfile) = &args.logfile {
            let f = File::create(logfile).expect("failed to create logfile");
            if color_arg == ColorSetting::Always {
                Box::new(Ansi::new(f)) as Box<dyn WriteColor>
            } else {
                Box::new(NoColor::new(f))
            }
        } else {
            let choice = match color_arg {
                ColorSetting::Auto=> ColorChoice::Auto,
                ColorSetting::Always => ColorChoice::Always,
                ColorSetting::Never => ColorChoice::Never,
            };
            Box::new(StandardStream::stdout(choice))
        };

        // Determine correct format
        let format = if args.quiet {
            FormatSetting::Terse
        } else {
            args.format.unwrap_or(FormatSetting::Pretty)
        };

        // Determine max test name length to do nice formatting later.
        //
        // Unicode is hard and there is no way we can properly align/pad the
        // test names and outcomes. Counting the number of code points is just
        // a cheap way that works in most cases.
        let name_width = tests.iter()
            .map(|test| test.name.chars().count())
            .max()
            .unwrap_or(0);

        let kind_width = tests.iter()
            .map(|test| {
                if test.kind.is_empty() {
                    0
                } else {
                    // The two braces [] and one space
                    test.kind.chars().count() + 3
                }
            })
            .max()
            .unwrap_or(0);

        Self {
            out,
            format,
            name_width,
            kind_width,
        }
    }

    /// Prints the first line "running 3 tests".
    pub(crate) fn print_title(&mut self, num_tests: u64) {
        match self.format {
            FormatSetting::Pretty | FormatSetting::Terse => {
                let plural_s = if num_tests == 1 {
                    ""
                } else {
                    "s"
                };

                writeln!(self.out).unwrap();
                writeln!(self.out, "running {} test{}", num_tests, plural_s).unwrap();
            }
            FormatSetting::Json => unimplemented!(),
        }
    }

    /// Prints the text announcing the test (e.g. "test foo::bar ... "). Prints
    /// nothing in terse mode.
    pub(crate) fn print_test(&mut self, name: &str, kind: &str) {
        match self.format {
            FormatSetting::Pretty => {
                let kind = if kind.is_empty() {
                    format!("")
                } else {
                    format!("[{}] ", kind)
                };

                write!(
                    self.out,
                    "test {: <2$}{: <3$} ... ",
                    kind,
                    name,
                    self.kind_width,
                    self.name_width,
                ).unwrap();
            }
            FormatSetting::Terse => {
                // In terse mode, nothing is printed before the job. Only
                // `print_single_outcome` prints one character.
            }
            FormatSetting::Json => unimplemented!(),
        }
    }

    /// Prints the outcome of a single tests. `ok` or `FAILED` in pretty mode
    /// and `.` or `F` in terse mode.
    pub(crate) fn print_single_outcome(&mut self, outcome: Outcome) {
        match self.format {
            FormatSetting::Pretty => {
                self.print_outcome_pretty(outcome);
                writeln!(self.out).unwrap();
            }
            FormatSetting::Terse => {
                let c = match outcome {
                    Outcome::Failed => 'F',
                    Outcome::Passed => '.',
                    Outcome::Ignored => 'i',
                };

                self.out.set_color(&color_of_outcome(outcome)).unwrap();
                write!(self.out, "{}", c).unwrap();
                self.out.reset().unwrap();
            }
            FormatSetting::Json => unimplemented!(),
        }
    }

    /// Prints the summary line after all tests have been executed.
    pub(crate) fn print_summary(
        &mut self,
        conclusion: &Conclusion,
    ) {
        match self.format {
            FormatSetting::Pretty | FormatSetting::Terse => {
                let outcome = if conclusion.has_failed() {
                    Outcome::Failed
                } else {
                    Outcome::Passed
                };

                writeln!(self.out).unwrap();
                write!(self.out, "test result: ").unwrap();
                self.print_outcome_pretty(outcome);
                writeln!(
                    self.out,
                    ". {} passed; {} failed; {} ignored; {} measured; {} filtered out",
                    conclusion.num_passed(),
                    conclusion.num_failed(),
                    conclusion.num_ignored,
                    -1, // TODO
                    conclusion.num_filtered_out(),
                ).unwrap();
                writeln!(self.out).unwrap();
            }
            FormatSetting::Json => unimplemented!(),
        }
    }

    fn print_outcome_pretty(&mut self, outcome: Outcome) {
        let s = match outcome {
            Outcome::Passed => "ok",
            Outcome::Failed => "FAILED",
            Outcome::Ignored=> "ignored",
        };

        self.out.set_color(&color_of_outcome(outcome)).unwrap();
        write!(self.out, "{}", s).unwrap();
        self.out.reset().unwrap();
    }
}

fn color_of_outcome(outcome: Outcome) -> ColorSpec {
    let mut out = ColorSpec::new();
    let color = match outcome {
        Outcome::Passed => Color::Green,
        Outcome::Failed => Color::Red,
        Outcome::Ignored => Color::Yellow,
    };
    out.set_fg(Some(color));
    out
}