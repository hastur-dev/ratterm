//! Blocking wrappers over the async Docker client.
//!
//! Split out of `client.rs`. The render loop is synchronous, so every call it
//! makes goes through [`super::runtime::block_on`]; keeping the wrappers here
//! leaves the async surface in one file and the bridge in another.

use super::client::{DockerClient, HostSnapshot};
use super::error::DockerError;
use super::runtime::block_on;

impl DockerClient {
    // -- blocking wrappers, for the synchronous UI ------------------------

    /// [`DockerClient::snapshot`] from synchronous code.
    ///
    /// # Errors
    /// Returns the reason the refresh failed.
    pub fn snapshot_blocking(&self) -> Result<HostSnapshot, DockerError> {
        block_on(self.snapshot())?
    }

    /// [`DockerClient::ping`] from synchronous code.
    ///
    /// # Errors
    /// Returns the reason the daemon did not answer.
    pub fn ping_blocking(&self) -> Result<String, DockerError> {
        block_on(self.ping())?
    }

    /// [`DockerClient::start_container`] from synchronous code.
    ///
    /// # Errors
    /// Returns the reason the call failed.
    pub fn start_container_blocking(&self, id: &str) -> Result<(), DockerError> {
        block_on(self.start_container(id))?
    }

    /// [`DockerClient::stop_container`] from synchronous code.
    ///
    /// # Errors
    /// Returns the reason the call failed.
    pub fn stop_container_blocking(&self, id: &str) -> Result<(), DockerError> {
        block_on(self.stop_container(id))?
    }

    /// [`DockerClient::restart_container`] from synchronous code.
    ///
    /// # Errors
    /// Returns the reason the call failed.
    pub fn restart_container_blocking(&self, id: &str) -> Result<(), DockerError> {
        block_on(self.restart_container(id))?
    }

    /// [`DockerClient::remove_container`] from synchronous code.
    ///
    /// # Errors
    /// Returns the reason the call failed.
    pub fn remove_container_blocking(&self, id: &str, force: bool) -> Result<(), DockerError> {
        block_on(self.remove_container(id, force))?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::transport::TransportChoice;
    use super::*;

    /// A client pointed at a port nothing listens on: every call must fail
    /// rather than block the caller for ever.
    fn dead_client() -> DockerClient {
        let choice = TransportChoice::Environment("http://127.0.0.1:1/".to_string());
        DockerClient::connect_with(&choice).expect("http clients build lazily")
    }

    #[test]
    fn every_blocking_call_returns_an_error_against_a_dead_endpoint() {
        let client = dead_client();
        assert!(client.ping_blocking().is_err());
        assert!(client.snapshot_blocking().is_err());
        assert!(client.start_container_blocking("abc").is_err());
        assert!(client.stop_container_blocking("abc").is_err());
        assert!(client.restart_container_blocking("abc").is_err());
        assert!(client.remove_container_blocking("abc", true).is_err());
    }

    #[test]
    fn a_blocking_call_names_the_operation_that_failed() {
        let client = dead_client();
        let error = client
            .start_container_blocking("abc")
            .expect_err("nothing listens on port 1");
        assert!(error.to_string().contains("start container"), "{error}");
    }

    #[test]
    fn a_blocking_call_from_inside_a_runtime_is_refused_rather_than_deadlocking() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a test runtime");
        rt.block_on(async {
            let client = dead_client();
            match client.ping_blocking() {
                Err(DockerError::Runtime(msg)) => assert!(msg.contains("async task"), "{msg}"),
                other => panic!("expected a runtime refusal, got {other:?}"),
            }
        });
    }
}
