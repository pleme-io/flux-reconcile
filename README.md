# pleme-io/flux-reconcile

Annotate a FluxCD resource with `reconcile.fluxcd.io/requestedAt` and wait for `Ready=True`. rio-side counterpart to `argocd-app-sync`.

```yaml
- uses: pleme-io/flux-reconcile@v1
  with:
    kind: helmrelease
    name: arc-controller
    namespace: actions-runner-controller
    timeout-seconds: 600
```

```yaml
- uses: pleme-io/flux-reconcile@v1
  with:
    kind: kustomization
    name: infrastructure-arc
    namespace: flux-system
```

## Inputs

| Name | Required | Default | Description |
|---|---|---|---|
| `kind` | yes | — | `helmrelease` / `kustomization` / `gitrepository` / etc. |
| `name` | yes | — | Resource name |
| `namespace` | no | `flux-system` | Resource namespace |
| `wait` | no | `true` | Wait for Ready=True |
| `timeout-seconds` | no | `300` | Wait timeout |
| `kubectl-context` | no | — | |

## Outputs

| Name | Description |
|---|---|
| `ready` | True / False / Unknown |
| `status-message` | Ready condition message |

## Part of the pleme-io action library

This action is one of 11 in [`pleme-io/pleme-actions`](https://github.com/pleme-io/pleme-actions) — discovery hub, version compat matrix, contributing guide, and reusable SDLC workflows shared across the library.
