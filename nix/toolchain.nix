{ inputs, ... }:
{
  perSystem = { system, ... }:
    let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [ inputs.rust-overlay.overlays.default ];
      };
      toolchain = pkgs.rust-bin.stable.latest.default;
      toolchainWithExtensions = toolchain.override {
        extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
      };
      nightlyToolchain = pkgs.rust-bin.nightly.latest.default.override {
        extensions = [ "rust-src" "llvm-tools" ];
      };
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchain;
      src = craneLib.cleanCargoSource ./..;
      crateInfo = craneLib.crateNameFromCargoToml { cargoToml = ./../Cargo.toml; };
      commonArgs = {
        inherit src;
        inherit (crateInfo) pname version;
        nativeBuildInputs = with pkgs; [ pkg-config ];
      };
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;
    in {
      _module.args = {
        inherit pkgs craneLib toolchainWithExtensions nightlyToolchain src commonArgs cargoArtifacts;
      };
    };
}
