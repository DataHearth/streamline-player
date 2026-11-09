# Option declarations shared by the NixOS and home-manager modules: both put
# the engine under `services.streamline-engine`, so the tree is identical and
# only the unit around it differs.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.streamline-engine;
  format = pkgs.formats.yaml { };
in
{
  options.services.streamline-engine = {
    enable = lib.mkEnableOption "the Streamline streaming engine";

    package = lib.mkPackageOption pkgs "streamline-engine" { };

    settings = lib.mkOption {
      type = lib.types.submodule {
        freeformType = format.type;
        options = {
          listen = lib.mkOption {
            type = lib.types.str;
            default = "127.0.0.1:8097";
            description = "Address the engine serves on. Streamline and the players reach it here.";
          };
          hw_accel = lib.mkOption {
            type = lib.types.enum [
              "auto"
              "vaapi"
              "nvenc"
              "qsv"
              "amf"
              "videotoolbox"
              "none"
            ];
            default = "auto";
            description = "GPU backend for transcoding; `auto` tries each available one and falls back to the CPU.";
          };
          tmp_dir = lib.mkOption {
            type = lib.types.str;
            description = "Directory for live HLS segments.";
          };
          jobs_dir = lib.mkOption {
            type = lib.types.str;
            description = "Directory for background job output (analysis, downloads).";
          };
        };
      };
      default = { };
      description = ''
        The engine's YAML config (see the engine design's Config section).
        `ffmpeg_dir` and `secret_file` are set by the module.
      '';
    };

    secretFile = lib.mkOption {
      type = lib.types.str;
      example = "/run/secrets/streamline-engine";
      description = ''
        File holding the bearer secret Streamline signs with. Read at runtime,
        never copied into the Nix store.
      '';
    };

    ffmpeg.package = lib.mkPackageOption pkgs "ffmpeg-full" { } // {
      description = "ffmpeg the engine runs. It needs VAAPI, Vulkan and libplacebo for hardware transcoding.";
    };

    mediaDirs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "/srv/media" ];
      description = ''
        Paths the engine may read media from, granted read-only. Streamline
        hands the engine file paths as it sees them, so these are its library
        roots.

        The NixOS unit hides everything else under /home, /root, /mnt, /media
        and /srv; any other path on the system stays readable, as under
        `ProtectSystem=strict`. The home-manager unit reads what its user can.
      '';
    };
  };

  config.assertions = [
    {
      assertion = !(cfg.enable && cfg.settings ? secret);
      message = "services.streamline-engine.settings.secret would land in the world-readable Nix store; set secretFile instead.";
    }
  ];
}
