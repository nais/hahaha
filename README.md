# HAHAHA

Your leader has been eliminated, it's time for the rest of you to die!

Pods spawned by Jobs that also contain sidecars might never run to completion.
That is, unless a particular villain shows up when the main container has died and terminates the others.

In other words, I'm attempting to reimplement [Ginuudan](https://github.com/nais/ginuudan) with kube-rs, primarily for fun and exploring how Kubernetes operators can be written with Rust.

## Differences between HAHAHA and Ginuudan

1. Ginuudan looks for pods with a specific annotation, HAHAHA uses a label.
    * Labels should better leverage underlying Kubernetes APIs for watching Pods.
2. HAHAHA defines actions through hardcoding them in `actions.rs/generate()`, as opposed to a yaml file.
    * Using the functions from the ActionInsertions trait to define actions will catch the simpler misconfigurations of actions during compile time.

## Reaching my idea of an ideal reaper operator

- [x] Be able to `exec` into Containers in Pods (sorted out in [c1a9628](https://github.com/chinatsu/hahaha/commit/c1a9a6285b4df5707b295e29b91fed37b8e5a602))
- [x] Be able to `portforward` into Pods (sorted out in [4f9c95c](https://github.com/chinatsu/hahaha/commit/4f9c95c546c3565e96d8b8af005bc78c30f6ef30))
- [ ] Report to Prometheus (or Stackdriver?) when killing off sidecars
- [x] Post Events to Pods about what's done