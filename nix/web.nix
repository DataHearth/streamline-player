{
  lib,
  callPackage,
  stdenvNoCC,
  fetchPnpmDeps,
  pnpmConfigHook,
  sqlite,
  nodejs,
  pnpm,
  basePath ? "",
  streamlineUrl ? null,
}:
let
  source = callPackage ./source.nix { };
in
stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "streamline-player-web";
  inherit (source) version;
  src = source.web;

  nativeBuildInputs = [
    nodejs
    pnpm
    pnpmConfigHook
    sqlite
  ];

  pnpmDeps = fetchPnpmDeps {
    # A fixed-output path is its name and declared hash only: with a fixed
    # name, a hash left stale by a lockfile change still resolves to the old
    # deps wherever they are already in a store. Naming it after the lockfile
    # forces the fetch, and with it the hash check, whenever the lock moves.
    pname = "${finalAttrs.pname}-${
      builtins.substring 0 12 (builtins.hashFile "sha256" ../pnpm-lock.yaml)
    }-${pnpm.version}";
    inherit (finalAttrs) version src;
    inherit pnpm;
    fetcherVersion = 4;
    hash = "sha256-qhm9evabvgCGpZjAGIEba0nFj6KmkZJ5RggWhlVJVmM=";
  };

  # nixpkgs' pnpmConfigHook restores the store index without WAL mode, and
  # pnpm 12 then fails its offline install with ERR_PNPM_NO_OFFLINE_TARBALL on
  # a loaded builder (seen in ../streamline). Drop once the hook keeps WAL.
  prePnpmInstall = ''
    sqlite3 "$STORE_PATH/v11/index.db" 'PRAGMA journal_mode=WAL;' >/dev/null
  '';

  buildPhase = ''
    runHook preBuild
    BASE_PATH=${lib.escapeShellArg basePath} pnpm build
    runHook postBuild
  '';

  # `task lint:svelte` and `task test:js`.
  doCheck = true;
  checkPhase = ''
    runHook preCheck
    pnpm check
    node dev/auto-check.ts
    node dev/messages-check.ts
    node dev/web-build-check.ts
    runHook postCheck
  '';

  installPhase = ''
    runHook preInstall
    cp -r build $out
    ${lib.optionalString (streamlineUrl != null) ''
      echo ${lib.escapeShellArg (builtins.toJSON { streamline = streamlineUrl; })} > $out/config.json
    ''}
    runHook postInstall
  '';
})
