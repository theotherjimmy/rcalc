let
  pkgs = import <nixos> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  };
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rust-bin.nightly.latest.rust
    cargo-crev
    cargo-watch
    cargo-binutils
    cargo-bloat
    gnuplot
  ];
}
