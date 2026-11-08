{ lib, buildPackages }:
# package.json's engines.node is the floor everyone owes, with or without Nix.
# The devshell takes the matching nixpkgs major as-is instead of rebuilding Node
# from source: bump engines.node only to what the locked nixpkgs ships.
let
  declared = lib.removePrefix ">=" (lib.importJSON ../package.json).engines.node;
  attr = "nodejs_${lib.versions.major declared}";
  pkg = lib.throwIf (
    !buildPackages ? ${attr}
  ) "package.json declares Node ${declared}, but nixpkgs has no ${attr}" buildPackages.${attr};
in
lib.throwIf (lib.versionOlder pkg.version declared)
  "package.json declares Node ${declared}, but the locked nixpkgs ships ${pkg.version}: update the flake lock"
  pkg
