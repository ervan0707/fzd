{
  description = "fzd — interactive terminal directory explorer that cd's your shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Single source of truth for name + version (same as every other channel:
        # crates.io / npm / PyPI all read the version stamped into Cargo.toml).
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        pname = cargoToml.package.name;
        version = cargoToml.package.version;

        # Toolchain read from ./rust-toolchain.toml so `nix develop`, CI, and the
        # package build all use the exact same Rust (no drift onto nixpkgs' rustc).
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # Extra inputs needed to link on darwin.
        darwinInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
        ];

        fzd = rustPlatform.buildRustPackage {
          inherit pname version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          buildInputs = darwinInputs;

          # CRITICAL: the precompiled std bakes its source-location paths into the
          # binary as plain strings, which point inside the toolchain's store path.
          # Nix's scanner then treats the whole toolchain (rustc + LLVM + cctools,
          # ~2 GB) as a runtime dependency. The strings are diagnostic-only, so
          # scrub the reference to shrink the closure (~2.8 GB -> ~46 MiB).
          nativeBuildInputs = [ pkgs.removeReferencesTo ];
          postInstall = ''
            remove-references-to -t ${rustToolchain} "$out/bin/${pname}"
          '';

          meta = with pkgs.lib; {
            description = "Interactive terminal directory explorer that cd's your shell";
            license = licenses.mit;
            mainProgram = pname;
          };
        };
      in
      {
        packages.default = fzd;
        packages.${pname} = fzd;

        # `nix run github:ervan0707/fzd`
        apps.default = flake-utils.lib.mkApp { drv = fzd; };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.rust-analyzer
          ] ++ darwinInputs;
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          shellHook = ''
            echo "${pname} dev shell — rust $(rustc --version) — run: cargo run"
          '';
        };
      });
}
