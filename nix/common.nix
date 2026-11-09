# The service body both modules share. Not a module: plain values the NixOS
# and home-manager modules splice into their own units.
{
  lib,
  pkgs,
  cfg,
  secretFile,
}:
let
  format = pkgs.formats.yaml { };
  # The package's own tools, rebuilt around the chosen ffmpeg. ffmpeg is only
  # in passthru, so this does not rebuild the engine.
  tools = (cfg.package.override { ffmpeg = cfg.ffmpeg.package; }).tools;
in
{
  configFile = format.generate "streamline-engine.yaml" (
    cfg.settings
    // {
      ffmpeg_dir = "${tools}/bin";
      secret_file = secretFile;
    }
  );

  serviceConfig = {
    Restart = "on-failure";
    RestartSec = 5;

    LockPersonality = true;
    MemoryDenyWriteExecute = true;
    NoNewPrivileges = true;
    PrivateTmp = true;
    ProtectClock = true;
    ProtectControlGroups = true;
    ProtectHostname = true;
    ProtectKernelLogs = true;
    ProtectKernelModules = true;
    ProtectKernelTunables = true;
    ProtectProc = "invisible";
    RemoveIPC = true;
    RestrictAddressFamilies = [
      "AF_INET"
      "AF_INET6"
      "AF_UNIX"
    ];
    RestrictNamespaces = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    SystemCallArchitectures = "native";
    # Not `~@resources`: background jobs run under `nice -n 10`, and
    # setpriority is in that group.
    SystemCallFilter = [
      "@system-service"
      "~@privileged"
    ];
    UMask = "0027";
  };
}
