{ pkgs }:
let
  inherit (pkgs) lib;

  engine = pkgs.rustPlatform.buildRustPackage {
    pname = "engine";
    inherit ((lib.importTOML ../engine/Cargo.toml).package) version;
    src = lib.fileset.toSource {
      root = ../.;
      # Cargo loads every workspace member's manifest and targets, even to build one of them.
      fileset = lib.fileset.unions [
        ../Cargo.toml
        ../Cargo.lock
        ../engine/Cargo.toml
        ../engine/src
        ../src-tauri/Cargo.toml
        ../src-tauri/build.rs
        ../src-tauri/src
        ../plugins/tauri-plugin-player/Cargo.toml
        ../plugins/tauri-plugin-player/build.rs
        ../plugins/tauri-plugin-player/src
      ];
    };
    cargoLock.lockFile = ../Cargo.lock;
    cargoBuildFlags = [
      "-p"
      "engine"
    ];
    # The e2e tests read engine/corpus, which is not in the repository: the suite runs in the dev shell.
    doCheck = false;
  };
in
{
  inherit engine;

  # nixpkgs links libplacebo to the Vulkan loader, so it can make the Vulkan device of its own that
  # the VAAPI tonemap needs; linuxserver/ffmpeg's cannot, and its engine would tonemap on the CPU.
  # The render node is usually root:render 0660: give the container that gid at run time
  # (Kubernetes supplementalGroups), as Streamline's VAAPI image does.
  engine-image = pkgs.dockerTools.streamLayeredImage {
    name = "ghcr.io/datahearth/streamline-player/engine";
    tag = engine.version;
    contents = [
      engine
      pkgs.ffmpeg-full
      pkgs.dovi-tool
      pkgs.mesa
    ];
    extraCommands = "mkdir -m 1777 tmp";
    config = {
      User = "1000:1000";
      Entrypoint = [ "/bin/engine" ];
      Cmd = [
        "--config"
        "/etc/engine/config.yaml"
      ];
      Labels = {
        "org.opencontainers.image.source" = "https://github.com/DataHearth/streamline-player";
        "org.opencontainers.image.description" = "Streamline's streaming engine";
        "org.opencontainers.image.version" = engine.version;
      };
      Env = [
        "PATH=/bin"
        "LIBVA_DRIVERS_PATH=${pkgs.mesa}/lib/dri"
        "VK_ICD_FILENAMES=${pkgs.mesa}/share/vulkan/icd.d/radeon_icd.x86_64.json"
      ];
    };
  };
}
