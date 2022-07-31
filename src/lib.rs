//! Write your own test scripts that look and behave like built-in tests!
//!
//! This is a simple and small testing framework that mimics the original
//! `libtest` (used by `cargo test`/`rustc --test`). That means: all output
//! looks pretty much like `cargo test` and most CLI arguments are understood
//! and used. With that plumbing work out of the way, your test runner can
//! concentrate on the actual testing.
//!
//! The central function of this crate is [`run_tests`].
//!
//! # Example
//!
//! ```
//! extern crate libtest_mimic;
//!
//! use libtest_mimic::{Arguments, Test, Outcome, run_tests};
//!
//!
//! // Parse command line arguments
//! let args = Arguments::from_args();
//!
//! // Create a list of tests (in this case: three dummy tests)
//! let tests = vec![
//!     Test::test("toph"),
//!     Test::test("sokka"),
//!     Test {
//!         name: "long_computation".into(),
//!         kind: "".into(),
//!         is_ignored: true,
//!         is_bench: false,
//!         data: (),
//!     },
//! ];
//!
//! // Run all tests and exit the application appropriatly (in this case, the
//! // test runner is a dummy runner which does nothing and says that all tests
//! // passed).
//! run_tests(&args, tests, |test| Outcome::Passed).exit();
//! ```
//!
//! For more examples, see [`examples/` in the repository][repo-examples].
//!
//!
//! [repo-examples]: https://github.com/LukasKalbertodt/libtest-mimic/tree/master/examples

use std::{process, sync::mpsc};

mod args;
mod printer;

use printer::Printer;
use threadpool::ThreadPool;

pub use crate::args::{Arguments, ColorSetting, FormatSetting};


/// Description of a single test.
#[derive(Clone, Debug)]
pub struct Test<D = ()> {
    /// The name of the test. It's displayed in the output and used for all
    /// kinds of filtering.
    pub name: String,

    /// Optional string to describe the kind of test. If this string is not
    /// empty, it is printed in brackets before the test name (e.g.
    /// `test [my-kind] test_name`).
    pub kind: String,

    /// Whether or not this test should be ignored. If the `--ignored` flag is
    /// set, ignored tests are executed, too.
    pub is_ignored: bool,

    /// Whether this test is actually a benchmark.
    pub is_bench: bool,

    /// Custom data. This field is not used by this library and can instead be
    /// used to store arbitrary data per test.
    pub data: D,
}

impl<D: Default> Test<D> {
    /// Creates a test with the given name, an empty `kind` and default data.
    /// The test is not ignored and is not a benchmark.
    pub fn test(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: String::new(),
            is_ignored: false,
            is_bench: false,
            data: D::default(),
        }
    }

    /// Creates a benchmark with the given name, an empty `kind` and default
    /// data. The benchmark is not ignored.
    pub fn bench(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: String::new(),
            is_ignored: false,
            is_bench: true,
            data: D::default(),
        }
    }
}

/// The outcome of performing a test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The test passed.
    Passed,

    /// The test or benchmark failed (either compiler error or panicked).
    Failed {
        /// A message that is shown after all tests have been run.
        msg: Option<String>,
    },

    /// The test or benchmark was ignored.
    Ignored,

    /// The benchmark was successfully run.
    Measured {
        /// Average time in ns.
        avg: u64,
        /// Variance in ns.
        variance: u64,
    },
}

/// Contains information about the entire test run. Is returned by
/// [`run_tests`].
///
/// This type is marked as `#[must_use]`. Usually, you just call
/// [`exit()`][Conclusion::exit] on the result of `run_tests` to exit the application
/// with the correct exit code. But you can also store this value and inspect
/// its data.
#[derive(Clone, Debug)]
#[must_use]
pub struct Conclusion {
    /// Number of tests and benchmarks that were filtered out (either by the
    /// filter-in pattern or by `--skip` arguments).
    pub num_filtered_out: u64,

    /// Number of passed tests.
    pub num_passed: u64,

    /// Number of failed tests and benchmarks.
    pub num_failed: u64,

    /// Number of ignored tests and benchmarks.
    pub num_ignored: u64,

    /// Number of benchmarks that successfully ran.
    pub num_benches: u64,
}

impl Conclusion {
    /// Exits the application with an appropriate error code (0 if all tests
    /// have passed, 101 if there have been failures).
    pub fn exit(&self) -> ! {
        self.exit_if_failed();
        process::exit(0);
    }

    /// Exits the application with error code 101 if there were any failures.
    /// Otherwise, returns normally.
    pub fn exit_if_failed(&self) {
        if self.has_failed() {
            process::exit(101)
        }
    }

    /// Returns whether there have been any failures.
    pub fn has_failed(&self) -> bool {
        self.num_failed > 0
    }

    fn empty() -> Self {
        Self {
            num_filtered_out: 0,
            num_passed: 0,
            num_failed: 0,
            num_ignored: 0,
            num_benches: 0,
        }
    }
}

impl Arguments {
    /// Returns `true` if the given test should be ignored.
    fn is_ignored<D>(&self, test: &Test<D>) -> bool {
        (test.is_ignored && !self.ignored)
            || (test.is_bench && self.test)
            || (!test.is_bench && self.bench)
    }

    fn is_filtered_out<D>(&self, test: &Test<D>) -> bool {
        // If a filter was specified, apply this
        if let Some(filter) = &self.filter_string {
            match self.exact {
                true if &test.name != filter => return true,
                false if !test.name.contains(filter) => return true,
                _ => {}
            };
        }

        // If any skip pattern were specified, test for all patterns.
        for skip_filter in &self.skip {
            match self.exact {
                true if &test.name == skip_filter => return true,
                false if test.name.contains(skip_filter) => return true,
                _ => {}
            }
        }

        false
    }
}

/// Runs all given tests with the given test runner.
///
/// This is the central function of this crate. It provides the framework for
/// the testing harness. It does all the printing and house keeping.
///
/// This function tries to respect most options configured via CLI args. For
/// example, filtering, output format and coloring are respected. However, some
/// things cannot be handled by this function and *you* (as a user) need to
/// take care of it yourself. The following options are ignored by this
/// function and need to be manually checked:
///
/// - `--nocapture` and capturing in general. It is expected that during the
///   test, nothing writes to `stdout` and `stderr`, unless `--nocapture` was
///   specified. If the test is ran as a seperate process, this is fairly easy.
///   If however, the test is part of the current application and it uses
///   `println!()` and friends, it might be impossible to capture the output.
///
/// Currently, the following CLI arg is ignored, but is planned to be used
/// in the future:
/// - `--format=json`. If specified, this function will
///   panic.
///
/// All other flags and options are used properly.
///
/// The returned value contains a couple of useful information. See the
/// [`Conclusion`] documentation for more information. If `--list` was
/// specified, a list is printed and a dummy `Conclusion` is returned.
pub fn run_tests<D: 'static + Send + Sync>(
    args: &Arguments,
    mut tests: Vec<Test<D>>,
    runner: impl Fn(&Test<D>) -> Outcome + 'static + Send + Sync,
) -> Conclusion {
    let mut conclusion = Conclusion::empty();

    // Apply filtering
    if args.filter_string.is_some() || !args.skip.is_empty() {
        let len_before = tests.len() as u64;
        tests.retain(|test| !args.is_filtered_out(test));
        conclusion.num_filtered_out = len_before - tests.len() as u64;
    }
    let tests = tests;

    // Create printer which is used for all output.
    let mut printer = printer::Printer::new(args, &tests);

    // If `--list` is specified, just print the list and return.
    if args.list {
        printer.print_list(&tests, args.ignored);
        return Conclusion::empty();
    }

    // Print number of tests
    printer.print_title(tests.len() as u64);

    let mut failed_tests = Vec::new();
    let mut handle_outcome = |outcome: Outcome, test: Test<D>, printer: &mut Printer| {
        printer.print_single_outcome(&outcome);

        if test.is_bench {
            conclusion.num_benches += 1;
        }

        // Handle outcome
        match outcome {
            Outcome::Passed => conclusion.num_passed += 1,
            Outcome::Failed { msg } => {
                failed_tests.push((test, msg));
                conclusion.num_failed += 1;
            },
            Outcome::Ignored => conclusion.num_ignored += 1,
            Outcome::Measured { .. } => {}
        }
    };

    // Execute all tests.
    if args.num_threads == Some(1) {
        // Run test sequentially in main thread
        for test in tests {
            // Print `test foo    ...`, run the test, then print the outcome in
            // the same line.
            printer.print_test(&test.name, &test.kind);
            let outcome = if args.is_ignored(&test) {
                Outcome::Ignored
            } else {
                runner(&test)
            };
            handle_outcome(outcome, test, &mut printer);
        }
    } else {
        // Run test in thread pool.
        let pool = ThreadPool::default();
        let (sender, receiver) = mpsc::channel();

        let runner = std::sync::Arc::new(runner);
        let num_tests = tests.len();
        for test in tests {
            if args.is_ignored(&test) {
                sender.send((Outcome::Ignored, test)).unwrap();
            } else {
                let runner = runner.clone();
                let sender = sender.clone();
                pool.execute(move || {
                    // It's fine to ignore the result of sending. If the
                    // receiver has hung up, everything will wind down soon
                    // anyway.
                    let outcome = runner(&test);
                    let _ = sender.send((outcome, test));
                });
            }
        }

        for (outcome, test) in receiver.iter().take(num_tests) {
            // In multithreaded mode, we do only print the start of the line
            // after the test ran, as otherwise it would lead to terribly
            // interleaved output.
            printer.print_test(&test.name, &test.kind);
            handle_outcome(outcome, test, &mut printer);
        }
    }

    // Print failures if there were any, and the final summary.
    if !failed_tests.is_empty() {
        printer.print_failures(&failed_tests);
    }

    printer.print_summary(&conclusion);

    conclusion
}
