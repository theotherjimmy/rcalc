{
  description = "A very basic flake";
  inputs = {
      nixpkgs.url = "github:nixos/nixpkgs/master";
      flake-utils.url = "github:numtide/flake-utils/master";
      rust-overlay.url = "github:oxalica/rust-overlay/master";
      rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
      rust-overlay.inputs.flake-utils.follows = "flake-utils";
  };
  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ rust-overlay.overlay ];
        pkgs = import nixpkgs { inherit system overlays; };
      in {
        packages.rcalc = pkgs.callPackage ({}: {}) {};
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            rust-bin.nightly.latest.default
            cargo-watch
            cargo-bloat
          ];
        };
      });
}
