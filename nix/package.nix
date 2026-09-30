{ lib, rustPlatform }:
let
  manifest = (lib.importTOML ../Cargo.toml).package;
in
rustPlatform.buildRustPackage {
  pname = manifest.name;
  inherit (manifest) version;
  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;
}
