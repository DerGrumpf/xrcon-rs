{ pkgs }:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    rustc
    rustfmt
    clippy
    rust-analyzer
    maturin
    python3
  ];

  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}
