{ lib, buildPackages }:
# packageManager pins one exact pnpm, unlike engines.node's floor: corepack and
# pnpm's own version switch run exactly that release, and fetchPnpmDeps lays
# its store out the way the pnpm running it does. So the nixpkgs pnpm feeding
# the devshell and every package build must be that release too.
let
  declared = lib.head (
    lib.splitString "+" (lib.removePrefix "pnpm@" (lib.importJSON ../package.json).packageManager)
  );
  attr = "pnpm_${lib.versions.major declared}";
  pkg = lib.throwIf (
    !buildPackages ? ${attr}
  ) "package.json declares pnpm ${declared}, but nixpkgs has no ${attr}" buildPackages.${attr};
in
lib.throwIf (pkg.version != declared)
  "package.json pins pnpm ${declared}, but the locked nixpkgs ships ${pkg.version}: run `pnpm self-update ${pkg.version}` (or update the flake lock) so both agree"
  pkg
