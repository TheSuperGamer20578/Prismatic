let
    pkgs = import <nixpkgs> {};
    rust-toolchain = pkgs.symlinkJoin {
        name = "rust-toolchain";
        paths = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rustPlatform.rustcSrc
        ];
    };
in with pkgs;
mkShell {
    buildInputs = [
        rust-toolchain
        clippy
        cargo-shuttle
    ];
    RUST_BACKTRACE = 1;
}
