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
    in
    {
      packages = forAll (pkgs: rec {
        default = xrcon;
        xrcon = pkgs.callPackage ./nix/package.nix { };
      });

      checks = forAll (pkgs: {
        build = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      });

      devShells = forAll (pkgs: {
        default = import ./nix/devshell.nix { inherit pkgs; };
      });
    };
}
