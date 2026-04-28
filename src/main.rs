//! `pleme-io/flux-reconcile` — force a FluxCD reconcile + wait for Ready.
//!
//! rio-side equivalent of `argocd-app-sync`. Annotates the resource with
//! `reconcile.fluxcd.io/requestedAt=<now>`, polls for the
//! `Ready=True` condition.
//!
//! Lifts the pattern from forge's `commands/flux.rs::health_check` into a
//! standalone action consumable by any FluxCD-driven workflow.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pleme_actions_shared::{ActionError, Input, Output, StepSummary};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Inputs {
    /// Resource kind: `helmrelease`, `kustomization`, `gitrepository`,
    /// `helmrepository`, `helmchart`, `bucket`, `imagepolicy`, etc.
    kind: String,
    name: String,
    #[serde(default = "default_namespace")]
    namespace: String,
    #[serde(default = "default_true")]
    wait: bool,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
    #[serde(default)]
    kubectl_context: Option<String>,
}

fn default_namespace() -> String { "flux-system".into() }
fn default_true() -> bool { true }
fn default_timeout() -> u64 { 300 }

fn main() {
    pleme_actions_shared::log::init();
    if let Err(e) = run() {
        e.emit_to_stdout();
        if e.is_fatal() {
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), ActionError> {
    let inputs = Input::<Inputs>::from_env()?;
    let context_args = build_context_args(&inputs.kubectl_context);

    annotate_for_reconcile(&inputs.kind, &inputs.name, &inputs.namespace, &context_args)?;

    let (ready, status_message) = if inputs.wait {
        wait_for_ready(
            &inputs.kind,
            &inputs.name,
            &inputs.namespace,
            Duration::from_secs(inputs.timeout_seconds),
            &context_args,
        )?
    } else {
        let ready = read_ready_condition(&inputs.kind, &inputs.name, &inputs.namespace, &context_args)?;
        (ready.clone(), ready)
    };

    let output = Output::from_runner_env()?;
    output.set("ready", &ready)?;
    output.set("status-message", &status_message)?;

    let mut summary = StepSummary::from_runner_env()?;
    summary
        .heading(2, &format!("flux-reconcile — {}/{}", inputs.kind, inputs.name))
        .table(
            &["Field", "Value"],
            vec![
                vec!["kind".into(), inputs.kind.clone()],
                vec!["name".into(), inputs.name.clone()],
                vec!["namespace".into(), inputs.namespace.clone()],
                vec!["ready".into(), ready.clone()],
                vec!["status".into(), status_message.clone()],
            ],
        );
    summary.commit()?;

    if ready != "True" {
        return Err(ActionError::error(format!(
            "{}/{} did not reach Ready=True (last: ready={ready} message={status_message:?})",
            inputs.kind, inputs.name
        )));
    }

    Ok(())
}

fn build_context_args(context: &Option<String>) -> Vec<String> {
    context
        .as_ref()
        .map(|c| vec!["--context".into(), c.clone()])
        .unwrap_or_default()
}

fn annotate_for_reconcile(
    kind: &str,
    name: &str,
    namespace: &str,
    context_args: &[String],
) -> Result<(), ActionError> {
    let now = unix_seconds();
    let mut args: Vec<String> = vec![
        "-n".into(),
        namespace.into(),
        "annotate".into(),
        kind.into(),
        name.into(),
        format!("reconcile.fluxcd.io/requestedAt={now}"),
        "--overwrite".into(),
    ];
    args.extend_from_slice(context_args);
    run_kubectl(&args)?;
    Ok(())
}

fn wait_for_ready(
    kind: &str,
    name: &str,
    namespace: &str,
    timeout: Duration,
    context_args: &[String],
) -> Result<(String, String), ActionError> {
    let deadline = Instant::now() + timeout;
    let mut last_ready = String::new();
    let mut last_message = String::new();
    while Instant::now() < deadline {
        last_ready = read_ready_condition(kind, name, namespace, context_args)?;
        last_message = read_ready_message(kind, name, namespace, context_args)?;
        if last_ready == "True" {
            return Ok((last_ready, last_message));
        }
        if last_ready == "False" && !last_message.is_empty() {
            // Hard failure — the controller has explicitly said "no". Surface
            // the failure rather than burning through the timeout.
            return Err(ActionError::error(format!(
                "{kind}/{name} reached Ready=False: {last_message}"
            )));
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    Err(ActionError::error(format!(
        "timed out after {}s waiting for {kind}/{name} to reach Ready=True (last: ready={last_ready} message={last_message})",
        timeout.as_secs()
    )))
}

fn read_ready_condition(
    kind: &str,
    name: &str,
    namespace: &str,
    context_args: &[String],
) -> Result<String, ActionError> {
    let mut args: Vec<String> = vec![
        "-n".into(),
        namespace.into(),
        "get".into(),
        kind.into(),
        name.into(),
        "-o".into(),
        "jsonpath={.status.conditions[?(@.type==\"Ready\")].status}".into(),
    ];
    args.extend_from_slice(context_args);
    let stdout = run_kubectl(&args)?;
    let val = stdout.trim();
    Ok(if val.is_empty() { "Unknown".into() } else { val.to_string() })
}

fn read_ready_message(
    kind: &str,
    name: &str,
    namespace: &str,
    context_args: &[String],
) -> Result<String, ActionError> {
    let mut args: Vec<String> = vec![
        "-n".into(),
        namespace.into(),
        "get".into(),
        kind.into(),
        name.into(),
        "-o".into(),
        "jsonpath={.status.conditions[?(@.type==\"Ready\")].message}".into(),
    ];
    args.extend_from_slice(context_args);
    let stdout = run_kubectl(&args)?;
    Ok(stdout.trim().to_string())
}

fn run_kubectl(args: &[String]) -> Result<String, ActionError> {
    let output = Command::new("kubectl")
        .args(args.iter().map(String::as_str))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| ActionError::error(format!("failed to spawn kubectl: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(ActionError::error(format!(
            "kubectl exited with status {} (stderr: {})",
            output.status,
            stderr.trim()
        )));
    }
    Ok(stdout.to_string())
}

fn unix_seconds() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_context_args_emits_flag() {
        assert_eq!(
            build_context_args(&Some("rio".into())),
            vec!["--context", "rio"]
        );
    }

    #[test]
    fn build_context_args_empty_when_unset() {
        assert!(build_context_args(&None).is_empty());
    }

    #[test]
    fn unix_seconds_returns_recent_time() {
        let s = unix_seconds();
        // Sometime after 2020-01-01
        assert!(s > 1_577_836_800);
    }
}
