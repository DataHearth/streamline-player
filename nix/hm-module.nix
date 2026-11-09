{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.streamline-engine;
  app = config.programs.streamline-player;
  # A user unit's credentials directory depends on the UID, so the YAML could
  # not name it; the engine reads the user's own file instead.
  common = import ./common.nix {
    inherit lib pkgs cfg;
    inherit (cfg) secretFile;
  };
in
{
  imports = [ ./options.nix ];

  options.programs.streamline-player = {
    enable = lib.mkEnableOption "the Streamline desktop player";
    package = lib.mkPackageOption pkgs "streamline-player" { };
  };

  config = lib.mkMerge [
    (lib.mkIf app.enable { home.packages = [ app.package ]; })

    (lib.mkIf cfg.enable {
      assertions = [
        {
          assertion = pkgs.stdenv.hostPlatform.isLinux;
          message = "services.streamline-engine uses a systemd user unit and is Linux-only.";
        }
      ];

      services.streamline-engine.settings = {
        jobs_dir = lib.mkDefault "${config.xdg.stateHome}/streamline-engine/jobs";
        tmp_dir = lib.mkDefault "${config.xdg.cacheHome}/streamline-engine/live";
      };

      systemd.user.services.streamline-engine = {
        Unit.Description = "Streamline streaming engine";
        Install.WantedBy = [ "default.target" ];
        # No ProtectSystem/ProtectHome here, unlike the NixOS module: a user
        # running it under their own account keeps the library in $HOME.
        Service = common.serviceConfig // {
          ExecStart = "${lib.getExe cfg.package} --config ${common.configFile}";
        };
      };
    })
  ];
}
