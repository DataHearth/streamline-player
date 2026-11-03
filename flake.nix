{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];
      perSystem =
        { pkgs, system, ... }:
        let
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
            targets = [ "x86_64-pc-windows-gnu" ];
          };
          rustAndroid = rust.override {
            targets = [
              "aarch64-linux-android"
              "armv7-linux-androideabi"
              "i686-linux-android"
              "x86_64-linux-android"
            ];
          };
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

          checks.fmt = pkgs.runCommandLocal "check-rust-fmt" { nativeBuildInputs = [ rust ]; } ''
            cd ${./.}
            cargo fmt --check
            touch $out
          '';

          # mkShell rather than numtide devshell: the setup hooks of pkg-config,
          # gstreamer and wrapGAppsHook wire PKG_CONFIG_PATH, GST_PLUGIN_SYSTEM_PATH_1_0
          # and the GIO/gsettings dirs WebKitGTK needs, which devshell would need by hand.
          devShells.default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rust
              cargo-tauri
              nodejs_24
              pnpm
              pkg-config
              wrapGAppsHook3
              typescript-language-server
              playwright-mcp
              jq
              ffmpeg-full
              dovi-tool
              (writeShellScriptBin "lint" "cargo clippy --all-targets -- -D warnings")
            ];
            PLAYWRIGHT_BROWSERS_PATH = pkgs.playwright-driver.browsers;
            PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = true;
            buildInputs = with pkgs; [
              webkitgtk_4_1
              gtk3
              libsoup_3
              librsvg
              openssl
              glib-networking
              # WebKitGTK's MSE plays through GStreamer; without these Shaka sees no codecs.
              gst_all_1.gstreamer
              gst_all_1.gst-plugins-base
              gst_all_1.gst-plugins-good
              gst_all_1.gst-plugins-bad
              gst_all_1.gst-libav
            ];
          };

          # Separate from the default shell so direnv does not pull the multi-GB SDK
          # for desktop work.
          devShells.android = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rustAndroid
              cargo-tauri
              nodejs_24
              pnpm
              jdk17
              androidSdk
              jq
            ];
            ANDROID_HOME = androidHome;
            ANDROID_SDK_ROOT = androidHome;
            NDK_HOME = "${androidHome}/ndk/29.0.14206865";
            JAVA_HOME = pkgs.jdk17.home;
            # AGP fetches aapt2 from Maven as a generic-linux binary; point it at the
            # patched one from the SDK so the build does not depend on nix-ld.
            GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidHome}/build-tools/36.0.0/aapt2";
          };
        };
    };
}
