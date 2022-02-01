# HAHAHA

Your leader has been eliminated, it's time for the rest of you to die!

Pods spawned by Jobs that also contain sidecars might never run to completion.
That is, unless a particular villain shows up when the main container has died and terminates the others.

I'm attempting to reimplement [Ginuudan](https://github.com/nais/ginuudan) with kube-rs, primarily for fun and exploring how Kubernetes operators can be written with Rust.


## Differences between HAHAHA and Ginuudan

1. Ginuudan looks for pods with a specific annotation, HAHAHA uses a label.
    * Labels should better leverage underlying Kubernetes APIs for watching Pods.
2. HAHAHA doesn't post events yet. I hope this won't take too much effort to implement.
3. HAHAHA defines actions through hardcoding them in `actions.rs/generate()`, as opposed to a yaml file.
    * Using the functions from the ActionInsertions trait to define actions will catch the simpler misconfigurations of actions during compile time.