{ ... }:
{
  perSystem = { pkgs, toolchainWithExtensions, nightlyToolchain, ... }: {
    devShells.default = pkgs.mkShell {
      name = "evalbox-dev";
      buildInputs = with pkgs; [
        toolchainWithExtensions
        pkg-config
        gcc
        python3
        go
        cargo-deny
      ];
      RUST_SRC_PATH = "${toolchainWithExtensions}/lib/rustlib/src/rust/library";
      RUST_BACKTRACE = "1";
    };

    devShells.fuzz = pkgs.mkShell {
      name = "evalbox-fuzz";
      buildInputs = with pkgs; [
        nightlyToolchain
        pkg-config
        gcc
        cargo-fuzz
      ];
      RUST_BACKTRACE = "1";
    };

  };
}
