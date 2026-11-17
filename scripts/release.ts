#!/usr/bin/env bun

import { $ } from "bun";
import { existsSync } from "node:fs";
import {
  access,
  mkdir,
  mkdtemp,
  readdir,
  readlink,
  rm,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, extname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { defaultDownloadUrlPrefix, generateAppcast } from "./appcast";
import { extractReleaseNotes } from "./changelog";

const appName = "Padu";
const executableName = "Padu";
const jsReplExecutableName = "padu_js_repl";
const daemonExecutableName = "padu-daemon";
const computerUseHelperName = "Padu Computer Use";
const packageName = "padu";
const defaultNotaryProfile = "NOTARY";
const projectRoot = resolve(import.meta.dir, "..");

const help = `Build, notarize, and publish a production release of Padu.

Usage:
  bun run release [options]

The default run builds a signed, notarized DMG, packages the Sparkle update
archive, regenerates the signed appcast (with binary deltas against recent
releases), and uploads everything to Cloudflare R2 — the bucket behind
https://releases.padu.dev. One-time setup lives in RELEASING.md.

Options:
  --local                       Build, notarize, and write the DMG + zip
                                without publishing to R2
  --force                       Publish even if this version is already in R2
  --output <path>               DMG output path (default: dist/Padu-<version>.dmg)
  --signing-identity <name>     Developer ID Application identity selector
                                (or PADU_SIGNING_IDENTITY; required unless --adhoc)
  --notary-profile <name>       notarytool keychain profile
                                (default: NOTARY; or PADU_NOTARY_PROFILE)
  --build-number <number>       CFBundleVersion override (or PADU_BUILD_NUMBER;
                                default derives a monotonic number from the
                                Cargo version)
  --volume-name <name>          Mounted DMG name (default: Padu)
  --skip-build                  Reuse target/release/padu, padu_js_repl, and
                                padu-daemon
  --skip-notarize               Unnotarized signed DMG (implies --local)
  --adhoc                       Ad-hoc sign, no notarization (implies --local)
  --help                        Show this help

Environment:
  PADU_SIGNING_IDENTITY         Developer ID Application identity selector
  PADU_ANALYTICS_ENDPOINT       analytics endpoint embedded at build time
  PADU_ANALYTICS_WEBSITE_ID     analytics website ID embedded at build time
  PADU_R2_REMOTE                rclone remote name (default: r2)
  PADU_R2_BUCKET                R2 bucket name (default: padu-releases)
  PADU_DOWNLOAD_URL_PREFIX      base URL served by the bucket
                                (default: ${defaultDownloadUrlPrefix})
  PADU_HISTORY_COUNT            prior archives pulled for deltas (default: 15)
  PADU_NO_HISTORY=1             skip pulling prior archives (no deltas)
  SPARKLE_BIN                   Sparkle tools dir (default: the bundle.sh cache
                                under .padu-cache/sparkle)
  SPARKLE_PRIVATE_KEY           Sparkle EdDSA private key (otherwise keychain)

Before the first production release:
  xcrun notarytool store-credentials NOTARY   # notarization credentials
  See RELEASING.md for the R2 bucket, rclone remote, and Sparkle key setup.
`;

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    adhoc: { type: "boolean" },
    "build-number": { type: "string" },
    force: { type: "boolean" },
    help: { type: "boolean", short: "h" },
    local: { type: "boolean" },
    "notary-profile": { type: "string" },
    output: { type: "string", short: "o" },
    "signing-identity": { type: "string" },
    "skip-build": { type: "boolean" },
    "skip-notarize": { type: "boolean" },
    "volume-name": { type: "string" },
  },
  strict: true,
});

if (values.help) {
  console.log(help);
  process.exit(0);
}

if (process.platform !== "darwin") {
  throw new Error("DMG packaging must run on macOS.");
}

function requireTool(name: string): void {
  if (!Bun.which(name)) {
    throw new Error(`Required tool not found in PATH: ${name}`);
  }
}

function logStep(message: string): void {
  console.log(`\n==> ${message}`);
}

type CargoMetadata = {
  packages: Array<{
    name: string;
    version: string;
  }>;
};

/** CFBundleVersion derived from the Cargo version. Sparkle decides which of
 *  two builds is newer by comparing this value, so it must grow with every
 *  release: three digits per semver field keep 0.2.0 → 2000 ahead of
 *  0.1.9 → 1009, and every release ahead of the pre-Sparkle DMGs that
 *  shipped CFBundleVersion 1. */
function derivedBuildNumber(version: string): string {
  const match = version.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?:-|$)/);
  const major = Number(match?.[1]);
  const minor = Number(match?.[2]);
  const patch = Number(match?.[3]);
  if (![major, minor, patch].every(Number.isInteger)) {
    throw new Error(
      `Cannot derive a build number from version "${version}"; ` +
        "pass --build-number.",
    );
  }
  return String(major * 1_000_000 + minor * 1_000 + patch);
}

const adhoc = values.adhoc ?? false;
const skipNotarize = values["skip-notarize"] ?? false;
let configuredSigningIdentity =
  values["signing-identity"] ?? process.env.PADU_SIGNING_IDENTITY;

if (!adhoc && !configuredSigningIdentity) {
  try {
    const output = await $`security find-identity -v -p codesigning`.quiet().text();
    const match = output.match(/"(Developer ID Application:[^"]+)"/);
    if (match?.[1]) {
      configuredSigningIdentity = match[1];
      console.log(`Auto-detected signing identity: ${configuredSigningIdentity}`);
    }
  } catch {
    // Ignore error
  }
}
const notaryProfile =
  values["notary-profile"] ??
  process.env.PADU_NOTARY_PROFILE ??
  defaultNotaryProfile;
const explicitBuildNumber =
  values["build-number"] ?? process.env.PADU_BUILD_NUMBER;
const analyticsEndpoint = process.env.PADU_ANALYTICS_ENDPOINT?.trim();
const analyticsWebsiteId = process.env.PADU_ANALYTICS_WEBSITE_ID?.trim();
const localOnly = values.local ?? false;
const force = values.force ?? false;
// Publishing requires a Developer ID-signed, notarized DMG, so the flags that
// weaken signing imply --local.
const publishing = !localOnly && !adhoc && !skipNotarize;

const r2Remote = process.env.PADU_R2_REMOTE ?? "r2";
const r2Bucket = process.env.PADU_R2_BUCKET ?? "padu-releases";
const r2Destination = `${r2Remote}:${r2Bucket}`;
// A bucket-scoped R2 API token cannot create buckets, and rclone otherwise
// checks/creates one before writing. The bucket must already exist.
const rcloneFlags = ["--s3-no-check-bucket"];
const downloadUrlPrefix =
  process.env.PADU_DOWNLOAD_URL_PREFIX ?? defaultDownloadUrlPrefix;
const historyCount = Number(process.env.PADU_HISTORY_COUNT ?? "15");
const skipHistory = process.env.PADU_NO_HISTORY === "1";

if (adhoc && values["signing-identity"]) {
  throw new Error("Use either --adhoc or --signing-identity, not both.");
}
if (!adhoc && !configuredSigningIdentity) {
  throw new Error(
    "Set PADU_SIGNING_IDENTITY or pass --signing-identity (or use --adhoc). " +
      "No valid 'Developer ID Application:' identity was found in the keychain.",
  );
}
if (explicitBuildNumber && !/^\d+(?:\.\d+){0,2}$/.test(explicitBuildNumber)) {
  throw new Error(
    "--build-number must contain one to three period-separated integers.",
  );
}
if (!Number.isSafeInteger(historyCount) || historyCount < 0) {
  throw new Error("PADU_HISTORY_COUNT must be a non-negative integer.");
}
if (!values["skip-build"] && (!analyticsEndpoint || !analyticsWebsiteId)) {
  console.warn(
    "Notice: PADU_ANALYTICS_ENDPOINT or PADU_ANALYTICS_WEBSITE_ID is not set. Analytics will be disabled in this build.",
  );
}

for (const tool of [
  "cargo",
  "codesign",
  "create-dmg",
  "diskutil",
  "ditto",
  "plutil",
  "xattr",
]) {
  requireTool(tool);
}
if (!adhoc && !skipNotarize) {
  requireTool("xcrun");
  requireTool("spctl");
}
if (publishing) {
  requireTool("rclone");
}

process.chdir(projectRoot);

if (!process.env.RUSTC_WRAPPER && Bun.which("sccache")) {
  process.env.RUSTC_WRAPPER = "sccache";
}

const metadata = JSON.parse(
  await $`cargo metadata --no-deps --format-version 1`.quiet().text(),
) as CargoMetadata;
const cargoPackage = metadata.packages.find(
  (candidate) => candidate.name === packageName,
);
if (!cargoPackage) {
  throw new Error(`Cargo package "${packageName}" was not found.`);
}

const version = cargoPackage.version;
const shortVersion = version.split("-", 1)[0];
const buildNumber = explicitBuildNumber ?? derivedBuildNumber(version);
const dmgName = `${appName}-${version}.dmg`;
const zipName = `${appName}-${version}.zip`;
if (publishing && version !== shortVersion) {
  throw new Error(
    `Version ${version} is a prerelease, and the appcast serves a single ` +
      "stable channel. Release a stable version, or build with --local.",
  );
}
if (!publishing) {
  const reason = localOnly ? "--local" : adhoc ? "--adhoc" : "--skip-notarize";
  console.log(`Building without publishing (${reason}).`);
}

// Fail before the long build: the bucket must exist and the version must be
// new. An unreachable remote should not surface after notarization.
if (publishing) {
  logStep(`Checking ${r2Destination}`);
  const listing = await $`rclone lsf ${r2Destination} ${rcloneFlags}`
    .quiet()
    .nothrow();
  if (listing.exitCode !== 0) {
    const detail = listing.stderr.toString().trim();
    if (detail.includes("directory not found")) {
      throw new Error(
        `R2 bucket "${r2Bucket}" does not exist on remote "${r2Remote}". ` +
          "Create it in the Cloudflare dashboard and attach the " +
          "releases.padu.dev custom domain (see RELEASING.md), then re-run.",
      );
    }
    throw new Error(`Cannot reach ${r2Destination}: ${detail}`);
  }
  const published = listing.stdout.toString().split("\n").filter(Boolean);
  if (published.includes(zipName) && !force) {
    throw new Error(
      `${zipName} is already published — bump the version in Cargo.toml, ` +
        "or pass --force to re-release it.",
    );
  }
}

const outputPath = resolve(
  projectRoot,
  values.output ?? join("dist", dmgName),
);
const volumeName = values["volume-name"] ?? appName;
const releaseDirectory = resolve(
  projectRoot,
  process.env.CARGO_TARGET_DIR ?? "target",
  "release",
);
const releaseExecutable = join(releaseDirectory, packageName);
const releaseJsReplExecutable = join(
  releaseDirectory,
  jsReplExecutableName,
);
const releaseDaemonExecutable = join(releaseDirectory, daemonExecutableName);
const appBundle = join(releaseDirectory, `${appName}.app`);
const contentsDirectory = join(appBundle, "Contents");
const bundledJsReplExecutable = join(
  contentsDirectory,
  "Resources",
  jsReplExecutableName,
);
const bundledDaemonExecutable = join(
  contentsDirectory,
  "MacOS",
  daemonExecutableName,
);
const bundledComputerUseSkill = join(
  contentsDirectory,
  "Resources",
  "skills",
  "padu-computer-use",
  "SKILL.md",
);
const bundledPiComputerUseExtension = join(
  contentsDirectory,
  "Resources",
  "computer-use",
  "pi-extension.ts",
);
const bundledComputerUseHelper = join(
  contentsDirectory,
  "Helpers",
  `${computerUseHelperName}.app`,
);
const bundledSparkleFramework = join(
  contentsDirectory,
  "Frameworks",
  "Sparkle.framework",
);

async function verifyJavaScriptRepl(executable: string): Promise<void> {
  const child = Bun.spawn([executable], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
  const requests = [
    {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo: { name: "padu-release", version: "1" },
      },
    },
    { jsonrpc: "2.0", id: 2, method: "tools/list", params: {} },
    {
      jsonrpc: "2.0",
      id: 3,
      method: "tools/call",
      params: {
        name: "js",
        arguments: { code: "nodeRepl.write(typeof sky);" },
      },
    },
  ];
  child.stdin.write(
    `${requests.map((request) => JSON.stringify(request)).join("\n")}\n`,
  );
  child.stdin.end();
  const stdout = await new Response(child.stdout).text();
  const stderr = await new Response(child.stderr).text();
  const exitCode = await child.exited;
  if (exitCode !== 0) {
    throw new Error(
      `Bundled JavaScript REPL exited with ${exitCode}: ${stderr.trim()}`,
    );
  }
  const responses = stdout
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const tools = responses
    .find((response) => response.id === 2)
    ?.result?.tools?.map((tool: { name?: string }) => tool.name);
  if (JSON.stringify(tools) !== JSON.stringify(["js", "js_reset"])) {
    throw new Error(
      `Bundled JavaScript REPL exposed unexpected tools: ${stdout}`,
    );
  }
  const lazySky = responses.find((response) => response.id === 3)
    ?.result?.content?.[0]?.text;
  if (lazySky !== "undefined") {
    throw new Error(`Bundled JavaScript REPL initialized sky eagerly: ${stdout}`);
  }
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

/** create-dmg 1.3.0 locates the interstitial APFS volume by scanning
 *  `hdiutil info` exactly once (create-dmg/create-dmg#164); on a busy CI
 *  runner the volume often has not finished mounting at that instant and the
 *  run aborts with "the interstitial disk image was not found". Retry, and
 *  detach any interstitial mounts left behind by a failed attempt. */
async function detachLeftoverInterstitialMounts(): Promise<void> {
  const info = await $`hdiutil info`.quiet().nothrow().text();
  const dmgBase = basename(outputPath);
  for (const entity of info.split(/^={5,}\s*$/m)) {
    const imageBase = entity
      .match(/image-path\s*:\s*(\S+)/)?.[1]
      ?.split("/")
      .pop();
    if (!imageBase?.startsWith("rw.") || !imageBase.endsWith(dmgBase)) {
      continue;
    }
    // Prefer the volume's mount point; create-dmg can also fail before the
    // volume mounts at all, leaving only raw /dev entries. Older macOS
    // reports the disk as "dev-entry".
    const deviceFields = entity
      .split("\n")
      .filter((line) => line.trimStart().startsWith("/dev/"))
      .map((line) =>
        line.trim().split(/\t+|\s{2,}/).filter(Boolean),
      );
    const devEntry = entity.match(/dev-entry\s*:\s*(\S+)/)?.[1];
    const mountPoint = deviceFields.find((fields) =>
      fields[2]?.startsWith("/Volumes/"),
    )?.[2];
    const lastDevice = deviceFields.at(-1)?.[0];
    const candidates = [...new Set([mountPoint, devEntry, lastDevice])].filter(
      (candidate): candidate is string => Boolean(candidate),
    );
    let detached = false;
    for (const candidate of candidates) {
      const result =
        await $`hdiutil detach -force ${candidate}`.quiet().nothrow();
      if (result.exitCode === 0) {
        detached = true;
        break;
      }
    }
    if (!detached) {
      console.warn(
        `Unable to detach leftover interstitial image ${imageBase}.`,
      );
    }
  }
}

async function removeLeftoverInterstitialImages(): Promise<void> {
  const outputDirectory = dirname(outputPath);
  const dmgBase = basename(outputPath);
  let entries: string[];
  try {
    entries = await readdir(outputDirectory);
  } catch {
    return;
  }
  for (const entry of entries) {
    if (entry.startsWith("rw.") && entry.endsWith(dmgBase)) {
      await rm(join(outputDirectory, entry), { force: true });
    }
  }
}

async function createStyledDmg(stagingDirectory: string): Promise<void> {
  const maxAttempts = 3;
  for (let attempt = 1; ; attempt += 1) {
    try {
      await $`create-dmg --volname ${volumeName} --window-pos 200 120 --window-size 660 400 --text-size 13 --icon-size 128 --icon ${`${appName}.app`} 180 178 --hide-extension ${`${appName}.app`} --app-drop-link 480 178 --filesystem APFS --format ULFO --no-internet-enable --overwrite ${outputPath} ${stagingDirectory}`;
      return;
    } catch (error) {
      if (attempt >= maxAttempts) {
        throw error;
      }
      const detail = error instanceof Error ? error.message : String(error);
      console.warn(
        `create-dmg attempt ${attempt} failed: ${detail}\n` +
          "Cleaning up leftover interstitial mounts and retrying...",
      );
      await detachLeftoverInterstitialMounts();
      await removeLeftoverInterstitialImages();
      await delay(2000 * attempt);
    }
  }
}

if (extname(outputPath).toLowerCase() !== ".dmg") {
  throw new Error(`Output path must end in .dmg: ${outputPath}`);
}
if (
  !volumeName.trim() ||
  volumeName.includes("/") ||
  volumeName.length > 27
) {
  throw new Error(
    "--volume-name must be non-empty, at most 27 characters, and cannot contain '/'.",
  );
}

let temporaryDirectory: string | undefined;
let mountedDmg = false;
let mountDirectory: string | undefined;
const identity = adhoc ? "-" : configuredSigningIdentity!;

try {
  if (values["skip-build"]) {
    for (const executable of [
      releaseExecutable,
      releaseJsReplExecutable,
      releaseDaemonExecutable,
    ]) {
      try {
        await access(executable);
      } catch {
        throw new Error(
          `Release executable not found at ${executable}. ` +
            "Run without --skip-build first.",
        );
      }
    }
  }

  logStep(
    values["skip-build"]
      ? "Assembling the app bundle"
      : "Building and assembling the app bundle",
  );
  await $`env PADU_CODESIGN_IDENTITY=${identity} PADU_ANALYTICS_ENDPOINT=${analyticsEndpoint ?? ""} PADU_ANALYTICS_WEBSITE_ID=${analyticsWebsiteId ?? ""} PADU_SKIP_CARGO_BUILD=${values["skip-build"] ? "1" : "0"} ${join(projectRoot, "scripts", "bundle.sh")} release`;
  for (const artifact of [
    join(contentsDirectory, "MacOS", executableName),
    bundledDaemonExecutable,
    bundledJsReplExecutable,
    bundledComputerUseSkill,
    bundledPiComputerUseExtension,
    bundledComputerUseHelper,
    join(bundledSparkleFramework, "Sparkle"),
  ]) {
    await access(artifact);
  }
  await $`plutil -replace CFBundleShortVersionString -string ${shortVersion} ${join(contentsDirectory, "Info.plist")}`;
  await $`plutil -replace CFBundleVersion -string ${buildNumber} ${join(contentsDirectory, "Info.plist")}`;
  await $`xattr -cr ${appBundle}`;

  await $`codesign --verify --strict --verbose=2 ${bundledJsReplExecutable}`;
  await $`codesign --verify --strict --verbose=2 ${bundledDaemonExecutable}`;
  await $`codesign --verify --deep --strict --verbose=2 ${bundledComputerUseHelper}`;
  await verifyJavaScriptRepl(bundledJsReplExecutable);
  logStep(
    adhoc
      ? "Ad-hoc signing the final app bundle"
      : `Signing the final app bundle as ${identity}`,
  );
  if (adhoc) {
    // No hardened runtime here: an ad-hoc identity carries no Team ID, so
    // library validation would refuse the embedded Sparkle framework and the
    // updater could never be exercised from an ad-hoc build.
    await $`codesign --force --sign - ${appBundle}`;
  } else {
    await $`codesign --force --options runtime --timestamp --sign ${identity} ${appBundle}`;
  }
  await $`codesign --verify --deep --strict --verbose=2 ${appBundle}`;

  temporaryDirectory = await mkdtemp(join(tmpdir(), "padu-dmg-"));
  const stagingDirectory = join(temporaryDirectory, "root");
  mountDirectory = join(temporaryDirectory, "mount");
  await mkdir(stagingDirectory);
  // ditto, not fs.cp: fs.cp rewrites the Sparkle framework's relative
  // symlinks into absolute paths under target/, which breaks the framework
  // on any other machine and fails the deep verify below.
  await $`ditto ${appBundle} ${join(stagingDirectory, `${appName}.app`)}`;
  await mkdir(dirname(outputPath), { recursive: true });
  await rm(outputPath, { force: true });

  logStep(`Creating the styled DMG at ${outputPath}`);
  await createStyledDmg(stagingDirectory);

  logStep(adhoc ? "Ad-hoc signing the DMG" : "Signing the DMG");
  if (adhoc) {
    await $`codesign --force --sign - ${outputPath}`;
  } else {
    await $`codesign --force --timestamp --sign ${identity} ${outputPath}`;
  }
  await $`codesign --verify --verbose=2 ${outputPath}`;

  logStep("Verifying the DMG contents");
  await mkdir(mountDirectory);
  await $`diskutil image attach --readOnly --mountOptions nobrowse --mountPoint ${mountDirectory} ${outputPath}`;
  mountedDmg = true;
  const mountedApp = join(mountDirectory, `${appName}.app`);
  const mountedContents = join(mountedApp, "Contents");
  const mountedJsRepl = join(
    mountedContents,
    "Resources",
    jsReplExecutableName,
  );
  const mountedDaemon = join(
    mountedContents,
    "MacOS",
    daemonExecutableName,
  );
  const mountedComputerUseHelper = join(
    mountedContents,
    "Helpers",
    `${computerUseHelperName}.app`,
  );
  const mountedSparkleFramework = join(
    mountedContents,
    "Frameworks",
    "Sparkle.framework",
  );
  for (const artifact of [
    join(mountedContents, "MacOS", executableName),
    mountedDaemon,
    mountedJsRepl,
    join(
      mountedContents,
      "Resources",
      "skills",
      "padu-computer-use",
      "SKILL.md",
    ),
    join(
      mountedContents,
      "Resources",
      "computer-use",
      "pi-extension.ts",
    ),
    mountedComputerUseHelper,
    join(mountedSparkleFramework, "Sparkle"),
  ]) {
    await access(artifact);
  }
  await access(join(mountDirectory, ".DS_Store"));
  const applicationsTarget = await readlink(
    join(mountDirectory, "Applications"),
  );
  if (applicationsTarget !== "/Applications") {
    throw new Error(
      `DMG Applications link points to "${applicationsTarget}", expected "/Applications".`,
    );
  }
  await $`codesign --verify --strict --verbose=2 ${mountedJsRepl}`;
  await $`codesign --verify --strict --verbose=2 ${mountedDaemon}`;
  await $`codesign --verify --deep --strict --verbose=2 ${mountedComputerUseHelper}`;
  await $`codesign --verify --strict --verbose=2 ${mountedSparkleFramework}`;
  await $`codesign --verify --deep --strict --verbose=2 ${mountedApp}`;
  await verifyJavaScriptRepl(mountedJsRepl);
  await $`diskutil eject ${mountDirectory}`;
  mountedDmg = false;

  if (!adhoc && !skipNotarize) {
    logStep("Submitting the DMG for Apple notarization");
    const notaryArgs: string[] = [
      "submit",
      outputPath,
      "--wait",
      "--output-format",
      "json",
    ];

    let temporaryApiKeyPath: string | undefined;
    const apiKey = process.env.APPLE_API_KEY?.trim();
    const apiKeyId = (
      process.env.APPLE_API_KEY_ID ?? process.env.APPLE_KEY_ID
    )?.trim();
    const apiIssuer = (
      process.env.APPLE_API_ISSUER ?? process.env.APPLE_ISSUER_ID
    )?.trim();

    const appleId = process.env.APPLE_ID?.trim();
    const applePassword = (
      process.env.APPLE_APP_SPECIFIC_PASSWORD ?? process.env.APPLE_PASSWORD
    )?.trim();
    const appleTeamId = (
      process.env.APPLE_TEAM_ID ?? process.env.APPLE_TEAM
    )?.trim();

    if (apiKey && apiKeyId && apiIssuer) {
      if (existsSync(apiKey)) {
        temporaryApiKeyPath = apiKey;
      } else {
        const decoded = apiKey.includes("-----BEGIN")
          ? apiKey
          : Buffer.from(apiKey, "base64").toString("utf-8");
        temporaryApiKeyPath = join(tmpdir(), `AuthKey_${apiKeyId}.p8`);
        await Bun.write(temporaryApiKeyPath, decoded);
      }
      notaryArgs.push(
        "--key",
        temporaryApiKeyPath,
        "--key-id",
        apiKeyId,
        "--issuer",
        apiIssuer,
      );
      console.log(
        `Using App Store Connect API Key (${apiKeyId}) for notarization.`,
      );
    } else if (appleId && applePassword && appleTeamId) {
      notaryArgs.push(
        "--apple-id",
        appleId,
        "--password",
        applePassword,
        "--team-id",
        appleTeamId,
      );
      console.log(`Using Apple ID (${appleId}) for notarization.`);
    } else {
      notaryArgs.push("--keychain-profile", notaryProfile!);
      console.log(`Using keychain profile "${notaryProfile}" for notarization.`);
    }

    try {
      const resultText =
        await $`xcrun notarytool ${notaryArgs}`.quiet().text();
      const result = JSON.parse(resultText) as {
        id?: string;
        message?: string;
        status?: string;
      };
      if (result.status !== "Accepted") {
        throw new Error(
          `Notarization ${result.status ?? "failed"}${result.id ? ` (${result.id})` : ""}: ` +
            (result.message ?? "inspect the submission with notarytool log"),
        );
      }
      console.log(`Notarization accepted: ${result.id ?? "unknown submission"}`);
    } finally {
      if (temporaryApiKeyPath && temporaryApiKeyPath !== apiKey) {
        await rm(temporaryApiKeyPath, { force: true });
      }
    }

    logStep("Stapling and assessing the notarized DMG");
    await $`xcrun stapler staple -v ${outputPath}`;
    await $`xcrun stapler validate -v ${outputPath}`;
    await $`spctl --assess --type open --context context:primary-signature --verbose=2 ${outputPath}`;
    // Notarizing the DMG also notarized the app's code, so the same
    // submission staples the app for the Sparkle archive.
    logStep("Stapling the app for the update archive");
    await $`xcrun stapler staple -v ${appBundle}`;
  } else if (adhoc) {
    console.warn(
      "\nCreated an ad-hoc signed DMG. It is suitable for local testing only.",
    );
  } else {
    console.warn(
      "\nCreated a Developer ID-signed DMG without notarization. " +
        "Gatekeeper will reject it on other Macs until it is notarized.",
    );
  }

  const zipPath = resolve(projectRoot, "dist", zipName);
  await mkdir(dirname(zipPath), { recursive: true });
  logStep(`Packaging ${zipName}`);
  await $`ditto -c -k --keepParent ${appBundle} ${zipPath}`;

  // A clean staging directory holds this release plus, when publishing, the
  // recent history generate_appcast needs to build binary deltas.
  const updatesDirectory = join(projectRoot, "dist", "updates");
  await rm(updatesDirectory, { force: true, recursive: true });
  await mkdir(updatesDirectory, { recursive: true });

  if (publishing && !skipHistory) {
    logStep(
      `Selecting the ${historyCount} most recent archives from R2 (for deltas)`,
    );
    type RemoteFile = { Name: string; IsDir: boolean };
    const remoteFiles = JSON.parse(
      await $`rclone lsjson ${r2Destination} ${rcloneFlags} --files-only --include ${"*.zip"} --include ${"appcast.xml"}`
        .quiet()
        .text(),
    ) as RemoteFile[];
    const archivePattern = new RegExp(`^${appName}-.+\\.zip$`);
    const archiveVersion = (name: string) =>
      name.slice(appName.length + 1, -".zip".length);
    const versionOrder = new Intl.Collator("en", { numeric: true });
    const recentArchives = remoteFiles
      .filter(
        ({ Name, IsDir }) =>
          !IsDir && archivePattern.test(Name) && Name !== zipName,
      )
      .sort((a, b) =>
        versionOrder.compare(archiveVersion(b.Name), archiveVersion(a.Name)),
      )
      .slice(0, historyCount)
      .map(({ Name }) => Name);
    const historyFiles = [
      ...(remoteFiles.some(({ Name }) => Name === "appcast.xml")
        ? ["appcast.xml"]
        : []),
      ...recentArchives,
    ];
    if (historyFiles.length > 0) {
      const includeFlags = historyFiles.flatMap((name) => [
        "--include",
        `/${name}`,
      ]);
      await $`rclone copy ${r2Destination} ${updatesDirectory} ${rcloneFlags} ${includeFlags}`;
    }
    console.log(
      recentArchives.length > 0
        ? `Pulled ${recentArchives.join(", ")}`
        : "No prior archives found.",
    );
  }

  await $`ditto ${zipPath} ${join(updatesDirectory, zipName)}`;

  // Release notes: this version's CHANGELOG.md section ships next to the
  // archive as Padu-<version>.md; generate_appcast links it as the update's
  // release notes, which Sparkle renders in the prompt.
  const changelogFile = Bun.file(join(projectRoot, "CHANGELOG.md"));
  const notes = (await changelogFile.exists())
    ? extractReleaseNotes(await changelogFile.text(), version)
    : null;
  const notesName = `${appName}-${version}.md`;
  const notesContents = `${notes ?? "See CHANGELOG.md for details."}\n`;
  await Bun.write(join(updatesDirectory, notesName), notesContents);
  // The tag workflow publishes files from dist/ as GitHub release assets;
  // sync-release then mirrors those assets to R2. Keep the notes beside the
  // appcast there as well so Sparkle's release-notes URL cannot 404.
  await Bun.write(join(projectRoot, "dist", notesName), notesContents);
  console.log(
    notes
      ? `Attached release notes for ${version}.`
      : `No "${version}" section in CHANGELOG.md — attached fallback notes.`,
  );

  logStep("Generating the signed appcast");
  await generateAppcast(updatesDirectory, downloadUrlPrefix);
  await $`ditto ${join(updatesDirectory, "appcast.xml")} ${join(projectRoot, "dist", "appcast.xml")}`;

  if (publishing) {
    // Archives and the DMG are immutable once published → cache forever.
    // appcast.xml changes every release → keep it fresh so update checks are
    // never served stale.
    const immutableCache =
      "Cache-Control: public, max-age=31536000, immutable";
    logStep(`Uploading ${dmgName} to ${r2Destination}`);
    await $`rclone copyto ${outputPath} ${`${r2Destination}/${dmgName}`} ${rcloneFlags} --header-upload ${immutableCache} --progress`;
    logStep(`Uploading update archives to ${r2Destination}`);
    await $`rclone copy ${updatesDirectory} ${r2Destination} ${rcloneFlags} --exclude ${"appcast.xml"} --exclude ${"old_updates/**"} --header-upload ${immutableCache} --progress`;
    logStep("Uploading appcast.xml");
    await $`rclone copyto ${join(updatesDirectory, "appcast.xml")} ${`${r2Destination}/appcast.xml`} ${rcloneFlags} --header-upload ${"Cache-Control: public, max-age=300, must-revalidate"}`;

    console.log(`\nPadu ${version} (build ${buildNumber}) is live:`);
    console.log(`  download : ${downloadUrlPrefix}${dmgName}`);
    console.log(`  update   : ${downloadUrlPrefix}${zipName}`);
    console.log(`  feed     : ${downloadUrlPrefix}appcast.xml`);
  }

  console.log(`\nDMG ready: ${outputPath}`);
  console.log(`ZIP ready: ${zipPath}`);
} finally {
  if (mountedDmg && mountDirectory) {
    const result = await $`diskutil eject ${mountDirectory}`.quiet().nothrow();
    if (result.exitCode === 0) {
      mountedDmg = false;
    } else {
      console.warn(`Unable to detach temporary mount at ${mountDirectory}.`);
    }
  }
  if (temporaryDirectory && !mountedDmg) {
    await rm(temporaryDirectory, { force: true, recursive: true });
  } else if (temporaryDirectory) {
    console.warn(`Temporary files retained at ${temporaryDirectory}.`);
  }
}
