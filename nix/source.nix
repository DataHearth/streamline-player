{ lib }:
# Each package builds from only the files that decide its output, so editing
# docs or Nix does not rebuild the app. Paths absent from the flake source
# (gitignored output such as engine/corpus or build/) never reach these sets.
let
  # Every workspace member: cargo refuses a workspace whose members are missing,
  # even when building one of them.
  rust = lib.fileset.unions [
    ../Cargo.toml
    ../Cargo.lock
    ../engine
    ../src-tauri
    ../plugins
  ];
  web = lib.fileset.unions [
    ../package.json
    ../pnpm-lock.yaml
    ../vite.config.ts
    ../tsconfig.json
    ../project.inlang/settings.json
    ../messages
    ../src
    ../static
    ../dev/auto-check.ts
    ../dev/messages-check.ts
  ];
  toSource =
    fileset:
    lib.fileset.toSource {
      root = ../.;
      inherit fileset;
    };
in
{
  rust = toSource rust;
  web = toSource web;
  app = toSource (lib.fileset.union rust web);
  version = (lib.importTOML ../src-tauri/Cargo.toml).package.version;
}
