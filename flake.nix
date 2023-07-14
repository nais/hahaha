{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {
    self,
    crane,
    flake-utils,
    nixpkgs,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

      # # Target musl when building on 64-bit linux
      # buildTarget =
      #   {"x86_64-linux" = "x86_64-unknown-linux-musl";}.${system}
      #   or (pkgs.rust.toRustTargetSpec pkgs.stdenv.hostPlatform);
      # rustToolchain = pkgs.rust-bin.stable.latest.default.override {
      #   targets = [
      #     buildTarget
      #     (pkgs.rust.toRustTargetSpec pkgs.stdenv.hostPlatform)
      #   ];
      # };
      rustToolchain = pkgs.rust-bin.stable.latest.default;

      # Set-up build dependencies and configure rust
      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      inherit (pkgs) lib;
      # Shamelessly stolen from:
      # https://github.com/fedimint/fedimint/blob/66519d5e978e22bb10a045f406101891e1bb7eb5/flake.nix#L99
      filterSrcWithRegexes = regexes: src: let
        basePath = toString src + "/";
      in
        lib.cleanSourceWith {
          filter = (
            path: type: let
              relPath = lib.removePrefix basePath (toString path);
              includePath =
                (type == "directory")
                || lib.any
                (re: builtins.match re relPath != null)
                regexes;
            in
              # uncomment to debug:
              # builtins.trace "${relPath}: ${lib.boolToString includePath}"
              includePath
          );
          inherit src;
        };

      cargo-details = lib.importTOML ./Cargo.toml;
      pname = cargo-details.package.name;
      commonArgs = {
        nativeBuildInputs = with pkgs; [
          glib
          openssl
          pkg-config
        ];
        # CARGO_BUILD_TARGET = buildTarget;
      };

      # Compile and cache only cargo dependencies
      dep-files-filter = ["Cargo.lock" "Cargo.toml" ".*/Cargo.toml"];
      cargo-deps = craneLib.buildDepsOnly (commonArgs
        // {
          src = filterSrcWithRegexes dep-files-filter ./.;
          inherit pname;
        });

      # Compile and cache only workspace code (seperately from 3rc party dependencies)
      package-file-filter =
        dep-files-filter
        ++ [
          ".*\.rs"
        ];
      cargo-package = craneLib.buildPackage (commonArgs
        // {
          inherit cargo-deps pname;
          src = filterSrcWithRegexes package-file-filter ./.;
        });
    in {
      checks = {
        # inherit # Comment in when you want tests to run on every new shell
        #   cargo-package
        #   ;
      };
      devShells.default = pkgs.mkShell {
        packages =
          (with pkgs; [
            # rust specific
            cargo-audit
            cargo-auditable
            cargo-cross
            cargo-deny
            cargo-outdated
            rust-analyzer

            # Editor stuffs
            helix
            lldb
            rust-analyzer

            # Nix stuff
            nix-output-monitor
          ])
          ++ commonArgs.nativeBuildInputs
          ++ [
            # Packages made in this flake
            rustToolchain
            # cargo-package # Comment in when you want tests to run on every new shell
          ]
          ++ lib.optionals (pkgs.stdenv.isLinux) (with pkgs; [cargo-watch]); # Currently broken on macOS

        shellHook = ''
          ${rustToolchain}/bin/cargo --version
          ${pkgs.helix}/bin/hx --version
          ${pkgs.helix}/bin/hx --health rust
        '';
      };
      packages = {
        rust = cargo-package;
        docker = pkgs.dockerTools.buildImage {
          name = pname;
          tag = "v${cargo-details.package.version}";
          extraCommands = ''mkdir -p data'';
          config = {
            Cmd = "--help";
            Entrypoint = ["${cargo-package}/bin/${pname}"];
          };
        };
      };
      packages.default = cargo-package;

      # Now `nix fmt` works!
      formatter = pkgs.nixfmt;
    });
}
