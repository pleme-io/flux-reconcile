{
  description = "pleme-io/flux-reconcile — force FluxCD reconcile + wait for Ready";

  inputs = {
    nixpkgs.follows = "substrate/nixpkgs";
    crate2nix = { url = "github:nix-community/crate2nix"; inputs.nixpkgs.follows = "nixpkgs"; };
    flake-utils.url = "github:numtide/flake-utils";
    substrate = { url = "github:pleme-io/substrate";};
  };

  outputs = inputs @ { self, nixpkgs, crate2nix, flake-utils, substrate, ... }:
    (import "${substrate}/lib/rust-action-release-flake.nix" {
      inherit nixpkgs crate2nix flake-utils;
    }) {
      toolName = "flux-reconcile";
      src = self;
      repo = "pleme-io/flux-reconcile";
      action = {
        description = "Annotate a FluxCD resource with reconcile.fluxcd.io/requestedAt and wait for Ready=True. rio-side counterpart to argocd-app-sync. Works for HelmRelease, Kustomization, GitRepository, HelmRepository, etc.";
        inputs = [
          { name = "kind"; description = "Resource kind (helmrelease / kustomization / gitrepository / ...)"; required = true; }
          { name = "name"; description = "Resource name"; required = true; }
          { name = "namespace"; description = "Resource namespace"; default = "flux-system"; }
          { name = "wait"; description = "Wait for Ready=True"; default = "true"; }
          { name = "timeout-seconds"; description = "Wait timeout"; default = "300"; }
          { name = "kubectl-context"; description = "kubectl context"; }
        ];
        outputs = [
          { name = "ready"; description = "Final Ready condition status (True / False / Unknown)"; }
          { name = "status-message"; description = "Final Ready condition message"; }
        ];
      };
    };
}
