{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "arcorrust-dev";

  buildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    rust-analyzer
    clippy
    rustfmt

    # Audio libraries
    alsa-lib
    pkg-config
  ];

  # Set PKG_CONFIG_PATH for alsa
  PKG_CONFIG_PATH = "${pkgs.alsa-lib.dev}/lib/pkgconfig";

  shellHook = ''
    echo "arcorrust development environment"
    echo "Rust: $(rustc --version)"
    echo "Cargo: $(cargo --version)"
    echo ""
  '';
}
