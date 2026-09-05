//! Changing things in a cluster: scaling, rollout restarts, deletions and
//! exec.
//!
//! Nothing here shells out. Each action builds the same request `kubectl`
//! would send and returns a typed result describing what was asked for, so the
//! caller can report it without re-reading the cluster.
//!
//! The patch bodies are built by free functions that take no client, so the
//! exact JSON sent to the API server is checked by tests rather than by trying
//! it against a cluster. Pod port forwarding lives in [`super::forward`].

use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams, DeleteParams, Patch, PatchParams};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;

use super::client::K8sClient;
use super::{K8sError, Result, block_on};

/// The annotation `kubectl rollout restart` writes to force a new pod
/// template hash. Anything else would not restart the pods.
pub const RESTART_ANNOTATION: &str = "kubectl.kubernetes.io/restartedAt";

/// Largest replica count this module will send.
///
/// The API accepts more, but a scale dialog is one keystroke away from a typo
/// that would try to schedule tens of thousands of pods, and the cost of that
/// mistake is much higher than the cost of refusing it.
pub const MAX_REPLICAS: i64 = 10_000;

/// Field manager recorded on every patch, so `kubectl` shows who made the
/// change.
const FIELD_MANAGER: &str = "ratterm";

/// How long an exec is allowed to run before it is abandoned.
const EXEC_TIMEOUT: Duration = Duration::from_secs(60);

/// Largest amount of output captured from one exec stream.
const MAX_EXEC_OUTPUT_BYTES: u64 = 1024 * 1024;

/// What a scale request asked for and what came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaleResult {
    /// Deployment name.
    pub name: String,
    /// Namespace the deployment is in.
    pub namespace: String,
    /// The replica count that was requested.
    pub requested: u32,
    /// The replica count the API server reports after the patch, when it
    /// returned one.
    pub applied: Option<u32>,
}

impl ScaleResult {
    /// Returns a one-line summary for the status bar.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "scaled {}/{} to {} replicas",
            self.namespace, self.name, self.requested
        )
    }
}

/// A rollout restart that was accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutRestart {
    /// Deployment name.
    pub name: String,
    /// Namespace the deployment is in.
    pub namespace: String,
    /// The value written to the restart annotation, in RFC 3339.
    pub restarted_at: String,
}

impl RolloutRestart {
    /// Returns a one-line summary for the status bar.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "restarted rollout of {}/{} at {}",
            self.namespace, self.name, self.restarted_at
        )
    }
}

/// Captured output of a command run in a container.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecOutput {
    /// Everything the command wrote to standard output.
    pub stdout: String,
    /// Everything the command wrote to standard error.
    pub stderr: String,
    /// True when the API server reported the command succeeded. False when it
    /// failed or did not report a status.
    pub success: bool,
    /// The API server's status message, when it sent one. This is where a
    /// non-zero exit code is described.
    pub status_message: String,
}

impl ExecOutput {
    /// Returns stdout, falling back to stderr when the command wrote nothing
    /// to stdout. This is what a one-line command result should show.
    #[must_use]
    pub fn best_effort_text(&self) -> &str {
        if self.stdout.trim().is_empty() {
            self.stderr.trim_end()
        } else {
            self.stdout.trim_end()
        }
    }
}

/// Builds the body of a scale patch.
///
/// # Errors
/// [`K8sError::InvalidRequest`] for a negative count or one above
/// [`MAX_REPLICAS`].
pub fn scale_patch_body(replicas: i64) -> Result<Value> {
    if replicas < 0 {
        return Err(K8sError::InvalidRequest(format!(
            "a replica count cannot be negative ({replicas}); enter zero or more"
        )));
    }
    if replicas > MAX_REPLICAS {
        return Err(K8sError::InvalidRequest(format!(
            "a replica count of {replicas} is above the {MAX_REPLICAS} limit ratterm will send; \
             enter a smaller number, or scale from kubectl if this is deliberate"
        )));
    }
    Ok(json!({ "spec": { "replicas": replicas } }))
}

/// Builds the body of a rollout restart patch.
///
/// This is the same patch `kubectl rollout restart` sends: a new value for the
/// restart annotation on the pod template, which changes the template hash and
/// makes the deployment controller roll the pods.
#[must_use]
pub fn restart_patch_body(restarted_at: &str) -> Value {
    json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        RESTART_ANNOTATION: restarted_at
                    }
                }
            }
        }
    })
}

/// Formats a restart timestamp the way `kubectl` does: RFC 3339, whole
/// seconds, `Z` suffix.
#[must_use]
pub fn restart_timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

impl K8sClient {
    /// Sets a deployment's replica count.
    ///
    /// # Errors
    /// [`K8sError::InvalidRequest`] for an unusable count, otherwise whatever
    /// the API server reported.
    pub fn scale_deployment(
        &self,
        namespace: &str,
        name: &str,
        replicas: i64,
    ) -> Result<ScaleResult> {
        let body = scale_patch_body(replicas)?;
        let api: Api<Deployment> = Api::namespaced(self.inner().clone(), namespace);
        let params = PatchParams::apply(FIELD_MANAGER).force();
        let context = self.context().to_string();
        let action = format!("scale deployment {namespace}/{name}");

        let scale =
            block_on(async move { api.patch_scale(name, &params, &Patch::Merge(body)).await })?
                .map_err(|e| K8sError::from_kube(&e, &context, &action))?;

        Ok(ScaleResult {
            name: name.to_string(),
            namespace: namespace.to_string(),
            requested: u32::try_from(replicas).unwrap_or(0),
            applied: scale
                .spec
                .and_then(|s| s.replicas)
                .map(|r| u32::try_from(r).unwrap_or(0)),
        })
    }

    /// Restarts a deployment's pods by patching the restart annotation.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn restart_rollout(&self, namespace: &str, name: &str) -> Result<RolloutRestart> {
        self.restart_rollout_at(namespace, name, Utc::now())
    }

    /// Restarts a rollout with an explicit timestamp.
    ///
    /// Separate from [`K8sClient::restart_rollout`] so the annotation value is
    /// under the caller's control when it needs to be.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn restart_rollout_at(
        &self,
        namespace: &str,
        name: &str,
        now: DateTime<Utc>,
    ) -> Result<RolloutRestart> {
        let restarted_at = restart_timestamp(now);
        let body = restart_patch_body(&restarted_at);
        let api: Api<Deployment> = Api::namespaced(self.inner().clone(), namespace);
        let params = PatchParams::apply(FIELD_MANAGER).force();
        let context = self.context().to_string();
        let action = format!("restart deployment {namespace}/{name}");

        block_on(async move { api.patch(name, &params, &Patch::Merge(body)).await })?
            .map_err(|e| K8sError::from_kube(&e, &context, &action))?;

        Ok(RolloutRestart {
            name: name.to_string(),
            namespace: namespace.to_string(),
            restarted_at,
        })
    }

    /// Deletes a pod.
    ///
    /// `grace_period` is the number of seconds the container is given to shut
    /// down; `None` uses the pod's own setting and `Some(0)` deletes
    /// immediately.
    ///
    /// # Errors
    /// Whatever the API server reported.
    pub fn delete_pod(&self, namespace: &str, name: &str, grace_period: Option<u32>) -> Result<()> {
        let api: Api<Pod> = Api::namespaced(self.inner().clone(), namespace);
        let params = DeleteParams {
            grace_period_seconds: grace_period,
            ..DeleteParams::default()
        };
        let context = self.context().to_string();
        let action = format!("delete pod {namespace}/{name}");

        block_on(async move { api.delete(name, &params).await })?
            .map_err(|e| K8sError::from_kube(&e, &context, &action))?;
        Ok(())
    }

    /// Runs a command in a container and captures its output.
    ///
    /// Standard input is not attached: this is for one-shot commands, not an
    /// interactive shell. Each stream is capped at one mebibyte and the whole
    /// call at a minute, so a command that never exits cannot block the UI
    /// thread indefinitely.
    ///
    /// # Errors
    /// [`K8sError::InvalidRequest`] for an empty command, otherwise whatever
    /// the API server reported.
    pub fn exec_command(
        &self,
        namespace: &str,
        pod: &str,
        container: Option<&str>,
        argv: &[String],
    ) -> Result<ExecOutput> {
        if argv.is_empty() {
            return Err(K8sError::InvalidRequest(
                "no command was given; type a command to run in the container".to_string(),
            ));
        }

        let api: Api<Pod> = Api::namespaced(self.inner().clone(), namespace);
        let mut params = AttachParams::default()
            .stdin(false)
            .stdout(true)
            .stderr(true);
        if let Some(name) = container {
            params = params.container(name.to_string());
        }

        let context = self.context().to_string();
        let action = format!("exec in pod {namespace}/{pod}");
        let pod_name = pod.to_string();
        let command = argv.to_vec();

        let outcome = block_on(async move {
            tokio::time::timeout(EXEC_TIMEOUT, async move {
                let mut process = api.exec(&pod_name, command, &params).await?;

                let mut stdout = String::new();
                if let Some(reader) = process.stdout() {
                    let _ = reader
                        .take(MAX_EXEC_OUTPUT_BYTES)
                        .read_to_string(&mut stdout)
                        .await;
                }
                let mut stderr = String::new();
                if let Some(reader) = process.stderr() {
                    let _ = reader
                        .take(MAX_EXEC_OUTPUT_BYTES)
                        .read_to_string(&mut stderr)
                        .await;
                }

                let status = match process.take_status() {
                    Some(future) => future.await,
                    None => None,
                };
                let _ = process.join().await;

                // The exec subresource reports the exit state as a
                // metav1.Status: `Success` for exit code zero, `Failure` with
                // a message naming the code otherwise.
                Ok::<_, kube::Error>(ExecOutput {
                    stdout,
                    stderr,
                    success: status
                        .as_ref()
                        .is_some_and(|s| s.status.as_deref() == Some("Success")),
                    status_message: status.and_then(|s| s.message).unwrap_or_default(),
                })
            })
            .await
        })?;

        match outcome {
            Err(_elapsed) => Err(K8sError::Unreachable {
                context,
                reason: format!(
                    "the command did not finish within {} seconds",
                    EXEC_TIMEOUT.as_secs()
                ),
            }),
            Ok(Err(e)) => Err(K8sError::from_kube(&e, &context, &action)),
            Ok(Ok(output)) => Ok(output),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_patch_body_is_exactly_a_spec_replicas_merge() {
        let body = scale_patch_body(3).expect("body");
        assert_eq!(body, json!({ "spec": { "replicas": 3 } }));
        assert_eq!(body.to_string(), r#"{"spec":{"replicas":3}}"#);
    }

    #[test]
    fn scaling_to_zero_is_allowed() {
        assert_eq!(
            scale_patch_body(0).expect("body"),
            json!({ "spec": { "replicas": 0 } })
        );
    }

    #[test]
    fn a_negative_replica_count_is_refused() {
        match scale_patch_body(-1) {
            Err(K8sError::InvalidRequest(message)) => {
                assert!(message.contains("negative"), "{message}");
                assert!(message.contains("zero or more"), "{message}");
            }
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
        assert!(scale_patch_body(i64::MIN).is_err());
    }

    #[test]
    fn an_absurd_replica_count_is_refused() {
        match scale_patch_body(MAX_REPLICAS + 1) {
            Err(K8sError::InvalidRequest(message)) => {
                assert!(message.contains("limit"), "{message}");
                assert!(message.contains("smaller"), "{message}");
            }
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
        assert!(scale_patch_body(i64::MAX).is_err());
    }

    #[test]
    fn the_replica_limit_itself_is_accepted() {
        assert!(scale_patch_body(MAX_REPLICAS).is_ok());
    }

    #[test]
    fn the_restart_patch_body_carries_the_kubectl_annotation() {
        let body = restart_patch_body("2026-09-05T12:00:00Z");
        assert_eq!(
            body,
            json!({
                "spec": {
                    "template": {
                        "metadata": {
                            "annotations": {
                                "kubectl.kubernetes.io/restartedAt": "2026-09-05T12:00:00Z"
                            }
                        }
                    }
                }
            })
        );
        assert_eq!(
            body.to_string(),
            r#"{"spec":{"template":{"metadata":{"annotations":{"kubectl.kubernetes.io/restartedAt":"2026-09-05T12:00:00Z"}}}}}"#
        );
    }

    #[test]
    fn the_restart_annotation_key_is_the_one_kubectl_uses() {
        assert_eq!(RESTART_ANNOTATION, "kubectl.kubernetes.io/restartedAt");
    }

    #[test]
    fn a_restart_timestamp_is_rfc3339_with_whole_seconds() {
        let now = chrono::DateTime::from_timestamp(1_757_073_600, 123_456_789).expect("timestamp");
        let rendered = restart_timestamp(now);
        assert!(rendered.ends_with('Z'), "{rendered}");
        assert!(!rendered.contains('.'), "{rendered}");
        assert_eq!(rendered, "2025-09-05T12:00:00Z");
    }

    #[test]
    fn a_scale_result_summarises_what_was_asked_for() {
        let result = ScaleResult {
            name: "api".to_string(),
            namespace: "web".to_string(),
            requested: 3,
            applied: Some(3),
        };
        assert_eq!(result.summary(), "scaled web/api to 3 replicas");
    }

    #[test]
    fn a_rollout_restart_summarises_when_it_happened() {
        let restart = RolloutRestart {
            name: "api".to_string(),
            namespace: "web".to_string(),
            restarted_at: "2026-09-05T12:00:00Z".to_string(),
        };
        assert_eq!(
            restart.summary(),
            "restarted rollout of web/api at 2026-09-05T12:00:00Z"
        );
    }

    #[test]
    fn exec_output_prefers_stdout_and_falls_back_to_stderr() {
        let output = ExecOutput {
            stdout: "hello\n".to_string(),
            stderr: "warning\n".to_string(),
            success: true,
            status_message: String::new(),
        };
        assert_eq!(output.best_effort_text(), "hello");

        let only_stderr = ExecOutput {
            stdout: "   \n".to_string(),
            stderr: "no such file\n".to_string(),
            ..ExecOutput::default()
        };
        assert_eq!(only_stderr.best_effort_text(), "no such file");

        assert_eq!(ExecOutput::default().best_effort_text(), "");
    }
}
