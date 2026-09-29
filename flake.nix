{
  description = "xrcon: DarkPlaces rcon protocol in Rust";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      manifest = (nixpkgs.lib.importTOML ./Cargo.toml).package;
    in
    {
      packages = forAll (pkgs: rec {
        default = xrcon;
        xrcon = pkgs.rustPlatform.buildRustPackage {
          inherit (manifest) version;
          pname = manifest.name;
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
      });

      checks = forAll (pkgs: {
        build = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      });

      devShells = forAll (pkgs: {
        default = import ./nix/devshell.nix { inherit pkgs; };
      });
    };
}
