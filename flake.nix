{
  inputs = {
    # latest versions of possible packages
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
  }: let
    # define systems to support (only one required right now)
    system = "x86_64-linux";

    pkgs = import nixpkgs {
      inherit system;
      overlays = [rust-overlay.overlays.default];
    };

    # use toolchain file to specify Rust packages
    rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./toolchain.toml;
  in {
    devShells.${system} = {
      default = pkgs.mkShell {
        # packages to be available within the devenv:
        # browse here: https://search.nixos.org/packages
        packages = with pkgs; [
          rustToolchain
          just # better Make
          nushell # easier than bash
          binaryen # wasm optimization tools
          nodejs_23
          docker-compose # for local test setup
          coreutils # sha256sum for checksum generation
        ];

        RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";    
      };
    };
  };

  # sets up main nix binary cache, this speeds up development environment setup (no need to build most packages)
  nixConfig = {
    extra-substituters = [
      "https://nix-community.cachix.org"
    ];

    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };
}
