use std::{
    collections::HashMap,
    io::Read,
    process::{Child, Command, Stdio},
    sync::{Arc, OnceLock},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};

use crate::tools::driver::ChatSubprocessSpec;
use anyhow::Context;
use std::path::Path;

static TASK_RUNTIME: OnceLock<Arc<TaskRuntimeRegistry>> = OnceLock::new();

pub fn task_runtime_handle() -> Arc<TaskRuntimeRegistry> {
    TASK_RUNTIME
        .get_or_init(|| Arc::new(TaskRuntimeRegistry::default()))
        .clone()
}

struct TaskRuntimeHandle {
    cancelled: AtomicBool,
    child: Mutex<Option<Child>>,
}

#[derive(Default)]
pub struct TaskRuntimeRegistry {
    tasks: Mutex<HashMap<String, TaskRuntimeHandle>>,
}

impl TaskRuntimeRegistry {
    pub fn track(&self, task_id: &str) {
        let mut tasks = self.tasks.lock().expect("task runtime lock poisoned");
        tasks
            .entry(task_id.to_string())
            .or_insert_with(|| TaskRuntimeHandle {
                cancelled: AtomicBool::new(false),
                child: Mutex::new(None),
            });
    }

    pub fn attach_child(&self, task_id: &str, child: Child) {
        self.track(task_id);
        let tasks = self.tasks.lock().expect("task runtime lock poisoned");
        if let Some(handle) = tasks.get(task_id) {
            *handle.child.lock().expect("task child lock poisoned") = Some(child);
        }
    }

    pub fn request_stop(&self, task_id: &str) -> bool {
        let tasks = self.tasks.lock().expect("task runtime lock poisoned");
        let Some(handle) = tasks.get(task_id) else {
            return false;
        };
        handle.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut child) = handle.child.lock() {
            if let Some(running) = child.as_mut() {
                let _ = running.kill();
            }
        }
        true
    }

    pub fn is_cancelled(&self, task_id: &str) -> bool {
        self.tasks
            .lock()
            .expect("task runtime lock poisoned")
            .get(task_id)
            .map(|handle| handle.cancelled.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    pub fn unregister(&self, task_id: &str) {
        self.tasks
            .lock()
            .expect("task runtime lock poisoned")
            .remove(task_id);
    }

    pub fn run_chat_subprocess_cancellable(
        &self,
        spec: &ChatSubprocessSpec,
        cwd: Option<&Path>,
        task_id: &str,
    ) -> anyhow::Result<(String, String, i32)> {
        self.track(task_id);

        let mut cmd = Command::new(&spec.program);
        for arg in &spec.args {
            cmd.arg(arg);
        }
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().context("spawn chat subprocess")?;
        let mut stdout = child.stdout.take().context("stdout pipe")?;
        let mut stderr = child.stderr.take().context("stderr pipe")?;
        self.attach_child(task_id, child);

        let stdout_handle = thread::spawn(move || {
            let mut buf = String::new();
            stdout.read_to_string(&mut buf).context("read stdout")?;
            Ok::<String, anyhow::Error>(buf)
        });
        let stderr_handle = thread::spawn(move || {
            let mut buf = String::new();
            stderr.read_to_string(&mut buf).context("read stderr")?;
            Ok::<String, anyhow::Error>(buf)
        });

        loop {
            if self.is_cancelled(task_id) {
                self.request_stop(task_id);
                anyhow::bail!("task_cancelled");
            }

            let mut finished = None;
            {
                let tasks = self.tasks.lock().expect("task runtime lock poisoned");
                if let Some(handle) = tasks.get(task_id) {
                    if let Ok(mut child) = handle.child.lock() {
                        if let Some(running) = child.as_mut() {
                            if let Some(status) = running.try_wait().context("poll subprocess")? {
                                finished = Some(status.code().unwrap_or(1));
                            }
                        }
                    }
                }
            }

            if let Some(code) = finished {
                let stdout = stdout_handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("stdout thread panicked"))??;
                let stderr = stderr_handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("stderr thread panicked"))??;
                self.unregister(task_id);
                return Ok((stdout, stderr, code));
            }

            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_stop_marks_cancelled_and_kills_child() {
        let registry = TaskRuntimeRegistry::default();
        registry.track("task_1");
        let child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        registry.attach_child("task_1", child);
        assert!(registry.request_stop("task_1"));
        assert!(registry.is_cancelled("task_1"));
        registry.unregister("task_1");
        assert!(!registry.is_cancelled("task_1"));
    }

    #[test]
    fn request_stop_returns_false_for_unknown_task() {
        let registry = TaskRuntimeRegistry::default();
        assert!(!registry.request_stop("missing"));
    }
}
