import { getBlockingColdCache, type WebsiteCacheContext } from "./github-cache";

interface GitHubAsset {
  name: string;
}

export interface GitHubRelease {
  tag_name: string;
  assets: GitHubAsset[];
  prerelease: boolean;
  draft: boolean;
}

export interface ReleaseInfo {
  version: string;
  macDmgAsset: string;
  linuxX64Tarball: string;
  linuxArm64Tarball: string | null;
  windowsX64Asset: string | null;
  windowsArm64Asset: string | null;
}

export interface ReleaseChannels {
  stable: ReleaseInfo;
  /** The newest prerelease, or null when stable has caught up with it. */
  beta: ReleaseInfo | null;
}

const REQUIRED_ASSET_PATTERNS = [
  /^Padu-.*\.dmg$/,
  /^padu-.*-x86_64-unknown-linux-gnu\.tar\.gz$/,
  /(?:^Padu-.*-(?:x86_64|x64)-Setup\.exe$|^Padu-Setup-.*\.exe$)/,
];

const GITHUB_RELEASES_URL = "https://api.github.com/repos/wisnuwiry/padu/releases?per_page=10";
const RELEASE_CACHE_KEY = "github-release:v3";
const ANDROID_RELEASE_CACHE_KEY = "github-android-release:v1";

function hasRequiredAssets(release: GitHubRelease): boolean {
  return REQUIRED_ASSET_PATTERNS.every((pattern) =>
    release.assets.some((asset) => pattern.test(asset.name)),
  );
}

function pickMacDmgAsset(assets: GitHubAsset[]): string | null {
  return assets.find((asset) => /^Padu-.*\.dmg$/.test(asset.name))?.name ?? null;
}

function pickLinuxTarballAssets(assets: GitHubAsset[]) {
  const x64 = assets.find((asset) => /^padu-.*-x86_64-unknown-linux-gnu\.tar\.gz$/.test(asset.name));
  const arm64 = assets.find((asset) => /^padu-.*-aarch64-unknown-linux-gnu\.tar\.gz$/.test(asset.name));
  return {
    x64: x64?.name ?? null,
    arm64: arm64?.name ?? null,
  };
}

function pickWindowsAssets(assets: GitHubAsset[]) {
  const x64 = assets.find(
    (asset) =>
      /^Padu-.*-(?:x86_64|x64)-Setup\.exe$/.test(asset.name) ||
      /^Padu-Setup-.*-x64\.exe$/.test(asset.name) ||
      (/^Padu-Setup-.*\.exe$/.test(asset.name) &&
        !asset.name.includes("arm64") &&
        !asset.name.includes("aarch64")),
  );
  const arm64 = assets.find(
    (asset) =>
      /^Padu-.*-(?:aarch64|arm64)-Setup\.exe$/.test(asset.name) ||
      /^Padu-Setup-.*-(?:arm64|aarch64)\.exe$/.test(asset.name),
  );
  return {
    x64: x64?.name ?? null,
    arm64: arm64?.name ?? null,
  };
}

function versionFromTag(tag: string): string {
  return tag.replace(/^v/, "");
}

async function fetchGitHubReleases(): Promise<GitHubRelease[]> {
  const response = await fetch(GITHUB_RELEASES_URL, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "padu-website",
    },
    cf: {
      cacheEverything: true,
      cacheTtl: 60,
      cacheKey: "github-releases-latest",
    },
  } as RequestInit);
  if (!response.ok) throw new Error(`github releases ${response.status}`);

  return (await response.json()) as GitHubRelease[];
}

function toReleaseInfo(release: GitHubRelease): ReleaseInfo | null {
  if (release.draft || !hasRequiredAssets(release)) return null;

  const macDmgAsset = pickMacDmgAsset(release.assets);
  if (!macDmgAsset) return null;

  const linuxAssets = pickLinuxTarballAssets(release.assets);
  if (!linuxAssets.x64) return null;

  const windowsAssets = pickWindowsAssets(release.assets);
  return {
    version: versionFromTag(release.tag_name),
    macDmgAsset,
    linuxX64Tarball: linuxAssets.x64,
    linuxArm64Tarball: linuxAssets.arm64,
    windowsX64Asset: windowsAssets.x64,
    windowsArm64Asset: windowsAssets.arm64,
  };
}

function coreVersion(version: string): number[] {
  return version.split("-")[0].split(".").map(Number);
}

/**
 * A beta is only worth offering while its core version is ahead of stable.
 * Promotion ships the same core as a stable release, which retires the beta
 * channel until the next beta line opens.
 */
function leadsStable(betaVersion: string, stableVersion: string): boolean {
  const beta = coreVersion(betaVersion);
  const stable = coreVersion(stableVersion);
  for (let index = 0; index < Math.max(beta.length, stable.length); index++) {
    const betaPart = beta[index] ?? 0;
    const stablePart = stable[index] ?? 0;
    if (betaPart !== stablePart) return betaPart > stablePart;
  }
  return false;
}

export function selectReleaseChannels(releases: GitHubRelease[]): ReleaseChannels {
  const stable = releases
    .filter((release) => !release.prerelease)
    .map(toReleaseInfo)
    .find((release) => release !== null);
  if (!stable) throw new Error("no ready GitHub release found");

  const beta = releases
    .filter((release) => release.prerelease)
    .map(toReleaseInfo)
    .find((release) => release !== null);

  return { stable, beta: beta && leadsStable(beta.version, stable.version) ? beta : null };
}

const DEFAULT_FALLBACK_RELEASE: ReleaseChannels = {
  stable: {
    version: "0.1.2",
    macDmgAsset: "Padu-0.1.2.dmg",
    linuxX64Tarball: "padu-0.1.2-x86_64-unknown-linux-gnu.tar.gz",
    linuxArm64Tarball: "padu-0.1.2-aarch64-unknown-linux-gnu.tar.gz",
    windowsX64Asset: "Padu-0.1.2-x86_64-Setup.exe",
    windowsArm64Asset: "Padu-0.1.2-aarch64-Setup.exe",
  },
  beta: null,
};

async function fetchReleaseChannels(): Promise<ReleaseChannels> {
  try {
    return selectReleaseChannels(await fetchGitHubReleases());
  } catch {
    return DEFAULT_FALLBACK_RELEASE;
  }
}

export function getLatestAndroidVersionFromReleases(releases: GitHubRelease[]): string {
  const release = releases.find((candidate) => {
    if (candidate.prerelease || candidate.draft) return false;
    const version = versionFromTag(candidate.tag_name);
    if (!/^\d+\.\d+\.\d+$/.test(version)) return false;
    return candidate.assets.some(
      (asset) => asset.name === `padu-${candidate.tag_name}-android.apk`,
    );
  });
  if (!release) throw new Error("no stable GitHub release with an Android APK found");
  return versionFromTag(release.tag_name);
}

async function fetchLatestAndroidVersion(): Promise<string> {
  return getLatestAndroidVersionFromReleases(await fetchGitHubReleases());
}

function isAndroidVersion(value: unknown): value is string {
  return typeof value === "string" && /^\d+\.\d+\.\d+$/.test(value);
}

function isReleaseInfo(value: unknown): value is ReleaseInfo {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.version === "string" &&
    /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(record.version) &&
    typeof record.macDmgAsset === "string" &&
    record.macDmgAsset.endsWith(".dmg") &&
    typeof record.linuxX64Tarball === "string" &&
    record.linuxX64Tarball.endsWith(".tar.gz") &&
    (typeof record.linuxArm64Tarball === "string" || record.linuxArm64Tarball === null) &&
    (typeof record.windowsX64Asset === "string" || record.windowsX64Asset === null) &&
    (typeof record.windowsArm64Asset === "string" || record.windowsArm64Asset === null)
  );
}

function isReleaseChannels(value: unknown): value is ReleaseChannels {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return isReleaseInfo(record.stable) && (record.beta === null || isReleaseInfo(record.beta));
}

export async function getReleaseChannels(context: WebsiteCacheContext): Promise<ReleaseChannels> {
  return getBlockingColdCache({
    context,
    key: RELEASE_CACHE_KEY,
    isValue: isReleaseChannels,
    fetchFresh: fetchReleaseChannels,
  });
}

export async function getLatestAndroidVersion(context: WebsiteCacheContext): Promise<string> {
  return getBlockingColdCache({
    context,
    key: ANDROID_RELEASE_CACHE_KEY,
    isValue: isAndroidVersion,
    fetchFresh: fetchLatestAndroidVersion,
  });
}
