# HAHAHA

Your leader has been eliminated, it's time for the rest of you to die!

Pods that also contain sidecars might never run to completion.
That is, unless a particular villain shows up when the main container has died and terminates the others.

Hahaha Watches all Pods using a [Label Selector](https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/), which defaults to `nais.io/naisjob="true"`, but this selector may be changed using the `WATCHES_SELECTOR` environment variable.

## What kind of sidecars can appear alongside my main container?

A different number of sidecars may appear alongside your main container. Here is an explanation for a few of them, some NaisJob specific and some generic.

You can view what HAHAHA tries to do to these sidecars when encountered in [actions.rs](https://github.com/nais/hahaha/blob/main/src/actions.rs#L9-L13)

### NaisJob specific

| name                         | explanation                                                               |
| ---------------------------- | ------------------------------------------------------------------------- |
| linkerd-proxy                | runs if your Naisjob runs in GCP                                          |
| cloudsql-proxy               | runs if your Naisjob provisions databases through `spec.gcp.sqlInstances` |
| secure-logs-fluentd          | runs if your Naisjob has `spec.secureLogs.enabled` set to `true`          |
| secure-logs-configmap-reload | runs if your Naisjob has `spec.secureLogs.enabled` set to `true`          |
| vks-sidecar                  | runs if your Naisjob has `spec.vault.sidecar` set to `true`               |

### Generic

| name        | explanation                                      |
| ----------- | ------------------------------------------------ |
| istio-proxy | used in clusters running with Istio service mesh |

## Things about development that you might want to know

Running HAHAHA's tests should be done by invoking `cargo test -- --test-threads 1`. The reason is that while the Prometheus test generally gets started first, it's usually the last to finish. By limiting the thread count to 1, we'll ensure that it finishes before the other tests run. The other tests are more like integration tests, and also mutate the Prometheus state, which makes it kind of hard to run them in parallel.

## Verifying the HAHAHA image and its contents

The image is signed "keylessly" (is that a word?) using [Sigstore cosign](https://github.com/sigstore/cosign).
To verify its authenticity run

```
cosign verify \
--certificate-identity "https://github.com/nais/hahaha/.github/workflows/build_and_push_image.yaml@refs/heads/main" \
--certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
europe-north1-docker.pkg.dev/nais-io/nais/images/hahaha@sha256:<shasum>
```

The images are also attested with SBOMs in the [CycloneDX](https://cyclonedx.org/) format.
You can verify these by running

```
cosign verify-attestation --type cyclonedx  \
--certificate-identity "https://github.com/nais/hahaha/.github/workflows/build_and_push_image.yaml@refs/heads/main" \
--certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
europe-north1-docker.pkg.dev/nais-io/nais/images/hahaha@sha256:<shasum>
```
