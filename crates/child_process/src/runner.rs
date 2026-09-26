//! Spawning a child in its own process group, and reaping it within a deadline.
//!
//! **Role:** the three corrections the crate documentation names: the group isolation that lets a
//! timeout kill a whole tree, the `killpg` that does it, and the signal check that keeps a killed
//! child out of the exit-code path.
//!
//! **Position:** called through [`Run`]; uses `stream.rs` for the pipes and `libc` for the process
//! group calls.
//!
//! **Signals and state:** spawns one child per call, writes its stdin once, and holds nothing
//! after it is reaped.
//!
//! **Invariants:** a timed-out run leaves no process of its group alive; a signalled child is
//! [`RunError::Signalled`], never an exit code.

use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::RunError;

use crate::stream::{SeparateDrains, start_merged_drain};
use crate::{Merged, Output, Run};

/// How often a pending child is polled while waiting on a deadline. Short enough that a timeout
/// is punctual, long enough that an hour-long worker does not spin a core.
const POLL: Duration = Duration::from_millis(20);

impl Run {
    /// Run to completion, capturing both streams separately. The raw exit code is preserved.
    pub fn output(self) -> Result<Output, RunError> {
        let label = self.display();
        let started = Instant::now();

        let mut cmd = self.command(Stdio::piped(), Stdio::piped());
        let mut child = spawn(&mut cmd, &self.program, &label)?;
        // `setsid` made the child a group leader, so its pgid equals its pid.
        let pgid = child.id() as i32;
        feed_stdin(&mut child, self.stdin.as_deref());

        // Drain both pipes for the child's whole life. See the crate documentation, invariant 3.
        let drains = SeparateDrains::start(&mut child);

        let status = match wait_within(&mut child, pgid, self.timeout, &label) {
            Ok(status) => status,
            // A timed-out group is dead, so both pipes are at EOF and the drains end at once.
            // Any other cause leaves a live child whose pipes nobody may block on.
            Err(cause) => {
                if matches!(cause, RunError::Timeout { .. }) {
                    drains.join();
                }
                return Err(cause);
            }
        };
        let (stdout, stderr) = drains.join();

        // A signal is NOT an exit code. See the crate documentation, invariant 1.
        if let Some(signal) = status.signal() {
            return Err(RunError::Signalled {
                program: label,
                signal,
            });
        }

        Ok(Output {
            code: status.code().unwrap_or(-1),
            stdout,
            stderr,
            duration: started.elapsed(),
        })
    }

    /// Run with **stdout and stderr on ONE pipe** — genuinely `2>&1`.
    ///
    /// ── WHY THIS IS NOT `output()` WITH THE TWO STRINGS CONCATENATED ─────────────────────────
    ///
    /// [`Run::output`] drains the two streams into two separate `String`s, which discards the
    /// interleaving. Joining them afterwards invents an order that the child never produced.
    ///
    /// A single shared pipe is what a shell does for `2>&1`, so the ordering is the child's own
    /// and cannot drift. `std::io::pipe` makes it cheap
    /// and adds no dependency.
    ///
    /// Reach for this whenever the captured text is shown or compared in the order it was written. Use
    /// [`Run::output`] when the two streams are handled separately — a caller that reports stderr
    /// only on failure, say.
    pub fn merged_output(self) -> Result<Merged, RunError> {
        let label = self.display();
        let started = Instant::now();

        let io_err = |e: std::io::Error| RunError::Failed {
            program: label.clone(),
            message: e.to_string(),
        };
        let (reader, writer) = std::io::pipe().map_err(io_err)?;
        let writer2 = writer.try_clone().map_err(io_err)?;

        let mut cmd = self.command(Stdio::from(writer), Stdio::from(writer2));
        let mut child = spawn(&mut cmd, &self.program, &label)?;
        let pgid = child.id() as i32;
        feed_stdin(&mut child, self.stdin.as_deref());

        // Drop OUR copies of the write end. Without this the read below never sees EOF, because
        // the pipe stays open on handles this process still holds. `cmd` owns both.
        drop(cmd);

        // One reader thread, so a timeout can still fire while the pipe fills.
        let reader_thread = start_merged_drain(reader);

        let status = match wait_within(&mut child, pgid, self.timeout, &label) {
            Ok(status) => status,
            Err(cause) => {
                if matches!(cause, RunError::Timeout { .. }) {
                    let _ = reader_thread.join();
                }
                return Err(cause);
            }
        };
        let text = reader_thread.join().unwrap_or_default();

        // A signal is NOT an exit code. See the crate documentation, invariant 1.
        if let Some(signal) = status.signal() {
            return Err(RunError::Signalled {
                program: label,
                signal,
            });
        }
        Ok(Merged {
            code: status.code().unwrap_or(-1),
            text,
            duration: started.elapsed(),
        })
    }

    /// Run and return only the raw exit code.
    pub fn status(self) -> Result<i32, RunError> {
        Ok(self.output()?.code)
    }

    /// The configured [`Command`], with the child placed in its own process group.
    ///
    /// Stdin is a pipe only when this run carries a body to write; otherwise it is `/dev/null`,
    /// so a child that reads stdin sees EOF rather than inheriting this process's terminal.
    pub(crate) fn command(&self, stdout: Stdio, stderr: Stdio) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args)
            .stdout(stdout)
            .stderr(stderr)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        if let Some(ref d) = self.cwd {
            cmd.current_dir(d);
        }
        for (k, v) in &self.envs {
            cmd.env(k, v);
        }
        for k in &self.env_removes {
            cmd.env_remove(k);
        }

        // Own process group, so a timeout can take the whole tree. See the module docs §2.
        //
        // The child is also killed when the thread that started it dies, so a killed app never
        // leaves a worker holding GPU memory or an FFmpeg decoding for nobody.
        //
        // SAFETY: `pre_exec` runs between fork and exec, where only async-signal-safe calls are
        // permitted. `setsid`, `setpgid`, `prctl` and `getppid` are all on that list and none
        // allocates; `parent` is a plain integer copied into the closure.
        let parent = unsafe { libc::getpid() };
        unsafe {
            cmd.pre_exec(move || {
                if libc::setsid() == -1 {
                    // Already a process-group leader (possible when the parent was itself
                    // spawned by a shell job-control setup); isolating the group still suffices.
                    if libc::setpgid(0, 0) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                // The parent died between the fork and the `prctl`: nobody will wait for us.
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                }
                Ok(())
            });
        }
        cmd
    }
}

/// Start the child, telling "the program is not installed" apart from every other spawn failure.
pub(crate) fn spawn(cmd: &mut Command, program: &str, label: &str) -> Result<Child, RunError> {
    match cmd.spawn() {
        Ok(child) => Ok(child),
        // The honest form of exit 127. Distinguished from every other spawn failure because
        // "you have not installed it" and "it is there and broke" are different problems.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(RunError::ProgramAbsent(program.to_string()))
        }
        Err(e) => Err(RunError::Failed {
            program: label.to_string(),
            message: format!("spawn failed: {e}"),
        }),
    }
}

/// Write `body` to the child's stdin and close it.
///
/// A closed stdin (the child exited early) is the child's business, not an error here.
pub(crate) fn feed_stdin(child: &mut Child, body: Option<&str>) {
    if let Some(body) = body
        && let Some(mut sink) = child.stdin.take()
    {
        use std::io::Write;
        let _ = sink.write_all(body.as_bytes());
    }
}

/// Reap `child`, enforcing `limit` by killing its whole process group.
///
/// On a deadline the group is killed and the child reaped before returning, so a caller's pipe
/// drains see EOF immediately and join without blocking.
fn wait_within(
    child: &mut Child,
    pgid: i32,
    limit: Option<Duration>,
    label: &str,
) -> Result<ExitStatus, RunError> {
    let Some(limit) = limit else {
        return child.wait().map_err(|e| RunError::Failed {
            program: label.to_string(),
            message: format!("wait failed: {e}"),
        });
    };

    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Kill the GROUP, not just the child.
                    //
                    // SAFETY: `killpg` on a pgid we created. A failure here means the group is
                    // already gone, which is the outcome we wanted anyway.
                    unsafe {
                        libc::killpg(pgid, libc::SIGKILL);
                    }
                    let _ = child.wait();
                    return Err(RunError::Timeout {
                        program: label.to_string(),
                        secs: limit.as_secs(),
                    });
                }
                std::thread::sleep(POLL);
            }
            Err(e) => {
                return Err(RunError::Failed {
                    program: label.to_string(),
                    message: format!("try_wait failed: {e}"),
                });
            }
        }
    }
}
