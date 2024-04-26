{
  inputs = {
    nixpkgs = {
      url = "github:NixOs/nixpkgs/nixpkgs-unstable";
    };
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, fenix, ... }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "aarch64-linux"
        "x86_64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      mkToolchain = system: with fenix.packages.${system}; combine [
        latest.cargo
        latest.rustc
        latest.clippy
        latest.llvm-tools
        latest.rustfmt
      ];
    in
    {
      devShells = forAllSystems
        (system:
          let
            pkgs = nixpkgs.legacyPackages.${system};
          in
          {
            default =
              (pkgs.mkShell.override (
                pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
                  stdenv = pkgs.darwin.apple_sdk_11_0.stdenv;
                }
              )) ({
                name = "vst-buildenv";
                packages = [
                  (mkToolchain system)
                ];
              });
          });
    };
}
