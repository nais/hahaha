# HAHAHA

Your leader has been eliminated, it's time for the rest of you to die!

Pods spawned by Jobs that also contain sidecars might never run to completion.
That is, unless a particular villain shows up when the main container has died and terminates the others.

## ???

I'm attempting to reimplement [Ginuudan](https://github.com/nais/ginuudan) with kube-rs, primarily for fun and exploring how Kubernetes operators can be written with Rust.

Business logic is a little split up now. I'm not really sure how to best structure this. :(
```r
src 
├── actions.rs # hardcoded actions
├── haha
│   ├── api.rs # contains some business logic around actually shutting down containers
│   ├── mod.rs
│   └── pod # functionality mostly built as traits on Pod
│       ├── app_pod.rs # just a helper that determines the "app" container and fetches the container statuses
│       ├── containers.rs # for getting the running sidecars that needs shutting down
│       ├── handleable.rs # contains the rest of the business logic, basically
│       │                 # like getting the correct action for a given sidecar and triggering an api action
│       └── mod.rs
└── main.rs # the main loop lol, triggers `pod.handle()`
```