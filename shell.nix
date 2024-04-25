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
        openssl
        pkg-config
    ];
    RUST_BACKTRACE = 1;
}
