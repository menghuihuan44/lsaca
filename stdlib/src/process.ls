// Process helpers.
//
// The frozen Appendix C intrinsic name for the argument vector is __proc_args.
// The legacy __args intrinsic continues to exist for the old helper only.
//
// Since 2.1.0 the standard library also exposes `exit(code: i32)`. The frozen
// Phase D intrinsic is __proc_exit; it maps onto the runtime's ls_exit, which
// flushes the standard streams and terminates the process without running
// destructors (the language has no `!` return type, so `exit` is documented as
// never returning instead).

pub extern fn __proc_args() -> Vec<String>;
pub extern fn __proc_exit(code: i32);

/// The command line arguments of the program, excluding the executable name.
pub fn args() -> Vec<String> {
    return __proc_args();
}

/// Terminates the process with the given exit code.
///
/// The standard streams are flushed, but no destructors run afterwards; this
/// function never returns to its caller.
pub fn exit(code: i32) {
    __proc_exit(code);
}

// v2.4 command execution.
//
// Commands run without a shell: the program and every argument are passed to
// the operating system as one exact argument vector. output captures both
// standard output and standard error; invalid UTF-8 in captured bytes is
// replaced with '?' by the runtime (documented lossy fallback).

pub extern fn __proc_status(argv: &Vec<String>, current_dir: &str) -> (i64, i64);
pub extern fn __proc_output(
    argv: &Vec<String>,
    current_dir: &str,
) -> (i64, i64, String, String);

/// The exit status of a finished child process.
pub struct ExitStatus {
    code: i32,
}

impl ExitStatus {
    /// The process exit code. On POSIX a child killed by a signal reports
    /// 128 + signal; a Windows process reports its 32-bit exit code.
    pub fn code(&self) -> i32 {
        return self.code;
    }

    /// True when the process exited with code zero.
    pub fn success(&self) -> bool {
        return self.code == 0;
    }
}

/// The captured result of running a child process.
pub struct Output {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl Output {
    pub fn status(&self) -> ExitStatus {
        return self.status.clone();
    }

    pub fn stdout(&self) -> String {
        return self.stdout.clone();
    }

    pub fn stderr(&self) -> String {
        return self.stderr.clone();
    }
}

/// A command builder. Arguments are passed exactly as given; no shell parsing
/// or expansion happens.
pub struct Command {
    program: String,
    arguments: Vec<String>,
    directory: String,
}

impl Command {
    /// Creates a command for program. An empty program is reported as an
    /// error when the command is run.
    pub fn new(program: String) -> Command {
        let arguments: Vec<String> = Vec::new();
        let directory: String = String::new();
        return Command {
            program: program,
            arguments: arguments,
            directory: directory,
        };
    }

    /// Appends one argument.
    pub fn arg(&mut self, argument: String) {
        self.arguments.push(argument);
    }

    /// Appends every argument in the given vector.
    pub fn args(&mut self, arguments: Vec<String>) {
        let mut index: usize = 0;
        while index < arguments.len() {
            self.arguments.push(arguments.get(index).unwrap());
            index += 1;
        }
    }

    /// Sets the working directory. An empty path inherits the current
    /// directory of the parent process.
    pub fn current_dir(&mut self, directory: String) {
        self.directory = directory;
    }

    /// Runs the command with inherited standard streams and waits for it.
    pub fn status(&self) -> Result<ExitStatus, Error> {
        let argv: Vec<String> = self.argument_vector();
        let (status, code) = __proc_status(&argv, self.directory.as_str());
        if status < 0 {
            return Err(Error::last());
        }
        return Ok(ExitStatus { code: code as i32 });
    }

    /// Runs the command, capturing standard output and standard error.
    pub fn output(&self) -> Result<Output, Error> {
        let argv: Vec<String> = self.argument_vector();
        let (status, code, stdout, stderr) =
            __proc_output(&argv, self.directory.as_str());
        if status < 0 {
            return Err(Error::last());
        }
        let exit_status: ExitStatus = ExitStatus { code: code as i32 };
        return Ok(Output {
            status: exit_status,
            stdout: stdout,
            stderr: stderr,
        });
    }

    /// The exact argument vector passed to the operating system.
    fn argument_vector(&self) -> Vec<String> {
        let mut argv: Vec<String> = Vec::new();
        argv.push(self.program.clone());
        let mut index: usize = 0;
        while index < self.arguments.len() {
            argv.push(self.arguments.get(index).unwrap());
            index += 1;
        }
        return argv;
    }
}
