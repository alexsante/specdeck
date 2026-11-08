//! Running external programs (gh, git) with a timeout, behind a trait so tests can fake them.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait Runner: Send + Sync {
    /// Run `program args…` in `cwd`, feeding `stdin`. `Err` means the program could not run at all
    /// (not found, timed out); a non-zero exit is `Ok` with `ok: false`.
    fn run(&self, program: &str, args: &[&str], stdin: Option<&str>, cwd: Option<&Path>) -> std::io::Result<Output>;
}

impl<T: Runner + ?Sized> Runner for std::sync::Arc<T> {
    fn run(&self, program: &str, args: &[&str], stdin: Option<&str>, cwd: Option<&Path>) -> std::io::Result<Output> {
        (**self).run(program, args, stdin, cwd)
    }
}

pub struct Process {
    pub timeout: Duration,
}

impl Default for Process {
    fn default() -> Self {
        Process { timeout: Duration::from_secs(15) }
    }
}

impl Runner for Process {
    fn run(&self, program: &str, args: &[&str], stdin: Option<&str>, cwd: Option<&Path>) -> std::io::Result<Output> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Never let gh or git stop to ask a question; there is no terminal.
            .env("GH_PROMPT_DISABLED", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("NO_COLOR", "1");
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        let mut child = cmd.spawn()?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            let input = input.to_string();
            std::thread::spawn(move || {
                let _ = pipe.write_all(input.as_bytes());
            });
        }
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let t_out = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let t_err = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let started = Instant::now();
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break st;
            }
            if started.elapsed() > self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, format!("{program} timed out after {}s", self.timeout.as_secs())));
            }
            std::thread::sleep(Duration::from_millis(15));
        };
        Ok(Output { ok: status.success(), stdout: t_out.join().unwrap_or_default(), stderr: t_err.join().unwrap_or_default() })
    }
}

/// A scripted runner for tests: each rule matches when every needle appears in
/// "program arg arg … <stdin>", first match wins. Calls are recorded.
#[derive(Default)]
pub struct Fake {
    pub rules: Vec<(Vec<String>, std::io::Result<Output>)>,
    pub calls: Mutex<Vec<String>>,
}

impl Fake {
    pub fn on(mut self, needles: &[&str], ok: bool, stdout: &str) -> Self {
        self.rules.push((needles.iter().map(|s| s.to_string()).collect(), Ok(Output { ok, stdout: stdout.into(), stderr: if ok { String::new() } else { stdout.into() } })));
        self
    }
    pub fn missing(mut self, program: &str) -> Self {
        self.rules.push((vec![format!("{program} ")], Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))));
        self
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Runner for Fake {
    fn run(&self, program: &str, args: &[&str], stdin: Option<&str>, _cwd: Option<&Path>) -> std::io::Result<Output> {
        let line = format!("{program} {} {}", args.join(" "), stdin.unwrap_or(""));
        self.calls.lock().unwrap().push(line.clone());
        for (needles, res) in &self.rules {
            if needles.iter().all(|n| line.contains(n.as_str())) {
                return match res {
                    Ok(o) => Ok(o.clone()),
                    Err(e) => Err(std::io::Error::new(e.kind(), e.to_string())),
                };
            }
        }
        Ok(Output { ok: false, stdout: String::new(), stderr: format!("fake: no rule for {line}") })
    }
}
