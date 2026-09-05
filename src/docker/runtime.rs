//! The async runtime Docker API calls are driven on.
//!
//! The UI thread has no ambient Tokio runtime, and bollard is async, so calls
//! from synchronous code go through [`block_on`]. The runtime is built once,
//! on first use, so a session that never opens a Docker screen pays nothing
//! for it. Mirrors the shape `crate::k8s` uses for the same reason.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::Runtime;

use super::error::DockerError;

/// Worker threads the Docker runtime gets.
///
/// Two is enough for a list call and an event stream at the same time, and
/// small enough not to compete with the terminal's own work.
const WORKER_THREADS: usize = 2;

/// The process-wide runtime, or `None` if it could not be built.
static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();

/// Returns the runtime every Docker call is driven on.
///
/// # Errors
/// Returns [`DockerError::Runtime`] if the worker threads could not start.
pub fn runtime() -> Result<&'static Runtime, DockerError> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(WORKER_THREADS)
                .thread_name("ratterm-docker")
                .enable_all()
                .build()
                .map_err(|e| tracing::error!("the Docker runtime could not start: {e}"))
                .ok()
        })
        .as_ref()
        .ok_or_else(|| DockerError::Runtime("the worker threads could not be started".to_string()))
}

/// Runs `future` to completion on the Docker runtime and returns its value.
///
/// # Errors
/// Returns [`DockerError::Runtime`] if the runtime cannot be built, or if the
/// caller is already inside a Tokio runtime — blocking there would deadlock
/// the calling worker, so it is refused rather than attempted.
pub fn block_on<F: Future>(future: F) -> Result<F::Output, DockerError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(DockerError::Runtime(
            "this call was made from inside an async task; call the async API directly".to_string(),
        ));
    }
    Ok(runtime()?.block_on(future))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_runtime_is_built_once_and_reused() {
        let first = runtime().expect("a runtime") as *const Runtime;
        let second = runtime().expect("a runtime") as *const Runtime;
        assert_eq!(first, second, "the runtime must be a singleton");
    }

    #[test]
    fn a_future_runs_to_completion_and_returns_its_value() {
        let value = block_on(async { 6 * 7 }).expect("no runtime problem");
        assert_eq!(value, 42);
    }

    #[test]
    fn an_error_inside_the_future_is_returned_rather_than_swallowed() {
        let inner: Result<(), &str> = block_on(async { Err("inner failure") }).expect("ran");
        assert_eq!(inner, Err("inner failure"));
    }

    #[test]
    fn blocking_from_inside_a_runtime_is_refused_rather_than_deadlocking() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a test runtime");
        rt.block_on(async {
            match block_on(async { 1 }) {
                Err(DockerError::Runtime(msg)) => {
                    assert!(msg.contains("async task"), "{msg}");
                }
                other => panic!("expected a runtime refusal, got {other:?}"),
            }
        });
    }
}
