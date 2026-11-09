{ lib, ... }:
let
  secret = "vm-test-secret-0123456789abcdef";
  # Every character systemd's unit syntax treats specially in a path list.
  odd = "/srv/odd: 50%";
  # Shaped like the engine's own e2e tests (`caps_json("h264", "mkv")`).
  decisions = path: {
    inherit path;
    capabilities = {
      video = [
        {
          codec = "h264";
          profiles = [ ];
          max_level = null;
          max_width = 3840;
          max_height = 2160;
          max_fps = 60;
        }
      ];
      hdr = {
        formats = [ ];
        dv_profiles = [ ];
      };
      audio = {
        codecs = [ "aac" ];
        max_channels = 6;
        passthrough = [ ];
      };
      containers = [ "mkv" ];
      subtitles = [ ];
      hls_keyframe_exact = true;
    };
    options = [ { } ];
  };
in
{
  name = "streamline-engine";

  nodes.machine =
    { pkgs, engineModule, ... }:
    {
      imports = [ engineModule ];
      virtualisation.memorySize = 2048;
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
      ];

      environment.etc."streamline-engine-secret".text = secret;
      environment.etc."decisions-home.json".text = builtins.toJSON (decisions "/home/media/clip.mkv");
      environment.etc."decisions-odd.json".text = builtins.toJSON (decisions "${odd}/clip.mkv");
      environment.etc."decisions-outside.json".text = builtins.toJSON (decisions "/home/other/clip.mkv");
      environment.etc."decisions-srv.json".text = builtins.toJSON (decisions "/srv/other/clip.mkv");

      services.streamline-engine = {
        enable = true;
        # `package` stays at its default, the flake module's own build, which
        # is the wiring a consumer gets.
        secretFile = "/etc/streamline-engine-secret";
        mediaDirs = [
          "/home/media"
          odd
        ];
        settings.hw_accel = "none";
        # Only the unit's device policy is asserted: the VM has no GPU.
        hardwareAcceleration.enable = true;
      };

      # Copies outside mediaDirs, in /home and in /srv: the sandbox hides both.
      systemd.services.make-clip = {
        wantedBy = [ "multi-user.target" ];
        before = [ "streamline-engine.service" ];
        requiredBy = [ "streamline-engine.service" ];
        serviceConfig.Type = "oneshot";
        script = ''
          mkdir -p /home/media /home/other '${odd}' /srv/other
          ${pkgs.ffmpeg-headless}/bin/ffmpeg -loglevel error -y -f lavfi -i testsrc=duration=2:size=320x240:rate=24 \
            -c:v libx264 -pix_fmt yuv420p /home/media/clip.mkv
          for d in /home/other '${odd}' /srv/other; do cp /home/media/clip.mkv "$d/clip.mkv"; done
          chmod -R a+rX /home/media /home/other /srv
        '';
      };
    };

  testScript = ''
    import json

    auth = "-H 'Authorization: Bearer ${secret}'"
    machine.wait_for_unit("streamline-engine.service")
    machine.wait_for_open_port(8097)

    with subtest("health answers with the secret and reports dovi_tool"):
        health = json.loads(machine.succeed(f"curl -sf {auth} http://127.0.0.1:8097/v1/health"))
        assert health["api_version"] == 1, health
        assert health["dovi_tool"] is True, health

    with subtest("health refuses a request without the secret"):
        code = machine.succeed("curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8097/v1/health")
        assert code == "401", code

    def decide(name):
        return json.loads(machine.succeed(
            f"curl -s {auth} -H 'Content-Type: application/json' -d @/etc/decisions-{name}.json http://127.0.0.1:8097/v1/decisions"
        ))

    with subtest("files under mediaDirs are read despite the hidden trees"):
        # `odd` also proves the unit quotes paths with spaces, colons and %.
        for name in ["home", "odd"]:
            out = decide(name)
            assert out.get("bit_rate", 0) > 0, (name, out)
            assert out["decisions"][0]["mode"] == "direct", (name, out)

    with subtest("the unit waits for the mounts holding mediaDirs"):
        mounts = machine.succeed("systemctl show -p RequiresMountsFor streamline-engine")
        assert "/srv/odd: 50%" in mounts, mounts

    with subtest("hardware acceleration allows the DRM devices"):
        # A directory such as /dev/dri is not a device node; systemd skips it
        # and the allow-list then denies the render node.
        allowed = machine.succeed("systemctl show -p DeviceAllow streamline-engine")
        assert "char-drm rw" in allowed, allowed

    with subtest("files outside mediaDirs stay hidden, in /home and in /srv"):
        for name in ["outside", "srv"]:
            out = decide(name)
            assert out["code"] == "file_not_found", (name, out)
  '';
}
