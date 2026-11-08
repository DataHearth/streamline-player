{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.devshell.flakeModule ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      perSystem =
        {
          config,
          pkgs,
          lib,
          system,
          self',
          ...
        }:
        let
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          nodejs = pkgs.callPackage ./nix/node.nix { };
          pnpm = pkgs.callPackage ./nix/pnpm.nix { };
          desktop = import ./nix/libs.nix { inherit pkgs lib; };
          mingwCc = pkgs.pkgsCross.mingwW64.stdenv.cc;
          mingwPthreads = pkgs.pkgsCross.mingwW64.windows.pthreads;

          android = pkgs.androidenv.composeAndroidPackages {
            # The system image is API 36; Tauri's Android template compiles against 37.
            platformVersions = [
              "36"
              "37.0"
            ];
            buildToolsVersions = [
              "36.0.0"
              "37.0.0"
            ];
            includeNDK = true;
            ndkVersions = [ "29.0.14206865" ];
            includeCmake = false;
            includeEmulator = true;
            includeSystemImages = true;
            systemImageTypes = [ "android-tv" ];
            abiVersions = [ "x86_64" ];
          };
          androidSdk = android.androidsdk;
          androidHome = "${androidSdk}/libexec/android-sdk";
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
            config = {
              android_sdk.accept_license = true;
              allowUnfree = true;
            };
          };

          packages = import ./nix/engine.nix { inherit pkgs; };

          # numtide's shell is a bare `derivation` that only symlinks its
          # profile, without preferLocalBuild. A machine with remote builders
          # would ship the whole closure (the Android SDK included) to one just
          # to make that link.
          devShells = lib.mapAttrs (
            _: shell:
            lib.mkForce (
              derivation (
                shell.devshell.shell.drvAttrs
                // {
                  preferLocalBuild = true;
                  allowSubstitutes = false;
                }
              )
            )
          ) config.devshells;

          checks = {
            fmt = pkgs.runCommandLocal "check-rust-fmt" { nativeBuildInputs = [ rust ]; } ''
              cd ${./.}
              cargo fmt --check
              touch $out
            '';

            # Enters the shell from an empty environment: a terminal that never
            # loaded direnv is where missing search paths show up.
            devshell-env = pkgs.runCommandLocal "check-devshell-env" { } ''
              env -i HOME=$TMPDIR PATH=${pkgs.coreutils}/bin ${self'.devShells.default}/entrypoint \
                ${pkgs.bash}/bin/bash ${./nix/devshell-env-check.sh}
              touch $out
            '';
          };

          devshells = {
            default = {
              packages = with pkgs; [
                rust
                cargo-tauri
                nodejs
                pnpm
                stdenv.cc
                pkg-config
                go-task
                biome
                nixfmt
                typescript-language-server
                playwright-mcp
                jq
                python3
                ffmpeg-full
                dovi-tool
                mingwCc
              ];
              env = desktop.env ++ [
                {
                  name = "PLAYWRIGHT_BROWSERS_PATH";
                  value = pkgs.playwright-driver.browsers;
                }
                {
                  name = "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS";
                  value = true;
                }
                {
                  name = "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER";
                  value = "${mingwCc}/bin/x86_64-w64-mingw32-cc";
                }
                {
                  # Rust's windows-gnu std links libpthread.a, which the bare
                  # mingw compiler does not ship.
                  name = "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS";
                  value = "-L native=${mingwPthreads}/lib";
                }
                # cc-rs takes the global CC above over its own target lookup, so
                # C dependencies (aws-lc-sys) would build for Linux; the
                # per-target names win over CC.
                {
                  name = "CC_x86_64_pc_windows_gnu";
                  value = "${mingwCc}/bin/x86_64-w64-mingw32-cc";
                }
                {
                  name = "CXX_x86_64_pc_windows_gnu";
                  value = "${mingwCc}/bin/x86_64-w64-mingw32-c++";
                }
                {
                  name = "AR_x86_64_pc_windows_gnu";
                  value = "${mingwCc}/bin/x86_64-w64-mingw32-ar";
                }
                {
                  name = "CFLAGS_x86_64_pc_windows_gnu";
                  value = "-I${mingwPthreads}/include";
                }
              ];
            };

          }
          # Separate from the default shell so direnv does not pull the multi-GB SDK
          # for desktop work. androidenv only builds the SDK for x86_64-linux.
          // lib.optionalAttrs (system == "x86_64-linux") {
            android = {
              packages = with pkgs; [
                rust
                cargo-tauri
                nodejs
                pnpm
                jdk17
                androidSdk
                jq
                go-task
              ];
              env = [
                {
                  name = "ANDROID_HOME";
                  value = androidHome;
                }
                {
                  name = "ANDROID_SDK_ROOT";
                  value = androidHome;
                }
                {
                  name = "NDK_HOME";
                  value = "${androidHome}/ndk/29.0.14206865";
                }
                {
                  name = "JAVA_HOME";
                  value = pkgs.jdk17.home;
                }
                {
                  # The streamline-tv AVD lives outside ~/.android.
                  name = "ANDROID_AVD_HOME";
                  eval = "$HOME/.config/.android/avd";
                }
                {
                  # AGP fetches aapt2 from Maven as a generic-linux binary; point it at the
                  # patched one from the SDK so the build does not depend on nix-ld.
                  name = "GRADLE_OPTS";
                  value = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidHome}/build-tools/36.0.0/aapt2";
                }
              ];
            };
          };
        };
    };
}
