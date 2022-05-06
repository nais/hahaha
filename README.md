# HAHAHA

Your leader has been eliminated, it's time for the rest of you to die!

Pods spawned by Naisjobs that also contain sidecars might never run to completion.
That is, unless a particular villain shows up when the main container has died and terminates the others.

## What?

This is a project to reimplement [Ginuudan](https://github.com/nais/ginuudan).

The primary motivation for this is to attain some level of stability and predictability.
Ginuudan, through the library that it uses to interface with Kubernetes, attempts to leave state on objects in the cluster.
This means that Ginuudan often doesn't show up for cleaning duty if things (e.g. expected state) aren't in order in the cluster otherwise.

HAHAHA is a more naive implementation that handles Pods without requiring any state beyond what's presented by a Pod whose main Container has reached a finished state.

HAHAHA is now the default janitor for Naisjobs, no additional work needs to be done to have HAHAHA deal with your Naisjob.

## Small technical differences between HAHAHA and Ginuudan

1. Ginuudan looks for Pods with a specific annotation, HAHAHA uses a label.
    * Labels should better leverage underlying Kubernetes APIs for watching Pods.
    * This also helps with deploying HAHAHA later, as it can coexist with Ginuudan; there will be no overlap in target Pods between the two.
2. HAHAHA defines actions through hardcoding them in `actions.rs/generate()`, as opposed to a yaml file.
    * Using the functions from the ActionInsertions trait to define actions will catch the simpler misconfigurations of actions during compile time.


## What kind of sidecars can appear alongside my Job?

| name | explanation |
|------|-------------|
| linkerd-proxy | runs if your Naisjob runs in GCP | 
| cloudsql-proxy | runs if your Naisjob provisions databases through `spec.gcp.sqlInstances` |
| secure-logs-fluentd | runs if your Naisjob has `spec.secureLogs.enabled` set to `true` |
| secure-logs-configmap-reload | runs if your Naisjob has `spec.secureLogs.enabled` set to `true` |
| vks-sidecar | runs if your Naisjob has `spec.vault.sidecar` set to `true` |

You can view what HAHAHA tries to do to these sidecars when encountered in [actions.rs](https://github.com/nais/hahaha/blob/main/src/actions.rs#L9-L13)

## Things about development that you might want to know

Running HAHAHA's tests should be done by invoking `cargo test -- --test-threads 1`. The reason is that while the Prometheus test generally gets started first, it's usually the last to finish. By limiting the thread count to 1, we'll ensure that it finishes before the other tests run. The other tests are more like integration tests, and also mutate the Prometheus state, which makes it kind of hard to run them in parallel.