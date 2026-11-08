{ pkgs, lib }:
# numtide devshell runs no nixpkgs setup hooks, so nothing fills in the search
# paths pkg-config, GStreamer, GIO and GSettings read. They are derived here
# from two lists, which the desktop package takes as its buildInputs too, so
# the shell and the package cannot disagree about what the app links.
let
  libs = with pkgs; [
    webkitgtk_4_1
    gtk3
    libsoup_3
    openssl
    librsvg
    # The desktop player: libmpv through its OpenGL render API; libGL brings libEGL.
    mpv-unwrapped
    libGL
    glib-networking
  ];

  # WebKitGTK's MSE plays through GStreamer; without these Shaka sees no codecs.
  gstPlugins = with pkgs.gst_all_1; [
    gstreamer
    gst-plugins-base
    gst-plugins-good
    gst-plugins-bad
    gst-libav
  ];

  # A .pc file names its dependencies (webkit2gtk-4.1 requires glib-2.0,
  # cairo, …) and pkg-config fails unless each one is on PKG_CONFIG_PATH too.
  # nixpkgs records them as the dev outputs' propagatedBuildInputs.
  pcClosure =
    roots:
    map (x: x.drv) (
      builtins.genericClosure {
        startSet = map (d: {
          key = (lib.getDev d).outPath;
          drv = lib.getDev d;
        }) roots;
        operator =
          x:
          map (d: {
            key = d.outPath;
            drv = d;
          }) (lib.filter lib.isDerivation (x.drv.propagatedBuildInputs or [ ]));
      }
    );

  schemaDirs = map (p: "${p}/share/gsettings-schemas/${p.name}") [
    pkgs.gtk3
    pkgs.gsettings-desktop-schemas
  ];

  pc = pcClosure libs;
in
{
  inherit libs gstPlugins pcClosure;

  env = [
    {
      name = "PKG_CONFIG_PATH";
      value = lib.concatStringsSep ":" [
        (lib.makeSearchPath "lib/pkgconfig" pc)
        (lib.makeSearchPath "share/pkgconfig" pc)
      ];
    }
    {
      name = "GST_PLUGIN_SYSTEM_PATH_1_0";
      value = lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0" gstPlugins;
    }
    {
      name = "GIO_EXTRA_MODULES";
      value = "${pkgs.glib-networking}/lib/gio/modules";
    }
    {
      name = "XDG_DATA_DIRS";
      prefix = lib.concatStringsSep ":" schemaDirs;
    }
    {
      # Build scripts that emit a bare `-lmpv` give the linker no -L; the cc
      # wrapper only learns search dirs from setup hooks, which never ran. With
      # these dirs found, the wrapper also writes their rpath, even through
      # rust-lld, so dev binaries need no LD_LIBRARY_PATH (which would hand the
      # shell's OpenSSL to every host tool run inside it).
      name = "LIBRARY_PATH";
      value = lib.makeLibraryPath libs;
    }
    {
      name = "CC";
      value = "${pkgs.stdenv.cc}/bin/cc";
    }
    {
      name = "CXX";
      value = "${pkgs.stdenv.cc}/bin/c++";
    }
  ];
}
