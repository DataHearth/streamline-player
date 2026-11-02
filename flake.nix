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
          };
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
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
        };
    };
}
