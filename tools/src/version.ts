import { ExecutorContext, logger, readJsonFile } from "@nrwl/devkit";
import { execSync } from "node:child_process";
import { join, resolve } from "node:path";
import { writePackageJson } from "./jsonUtils";
import { writeTomlManifest } from "./tomlUtils";

const SemVerRegEx = /^(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)(?:-(?<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

interface Options {
	local?: boolean;
	packages: string[];
}

/**
 * Retrieves the current tag for HEAD.
 *
 * @returns The current tag if one is on HEAD; otherwise `undefined`.
 */
export function getCurrentTag(): string | undefined {
  try {
    return execSync(`git describe --exact-match HEAD`, { stdio: "ignore" }).toString().trim()
  }
  catch(err) {
    return undefined;
  }
}

/**
 * Retrieves the commit hash of HEAD, or a specified file.
 *
 * @param filePath - The optional file path for which a commit hash is required.
 * @returns The commit hash of `filePath`; or the commit hash of HEAD if `filePath` is `undefined`.
 */
export function getLatestCommit(filePath?: string): string {
  try {
    return filePath
      ? execSync(`git log --format=%h -1 -- ${filePath}`).toString().trim()
      : execSync("git log --format=%h -1").toString().trim()
  }
  catch(err) {
    throw new Error("Failed to retrieve the latest commit.");
  }
}

/**
 * Retrieves the current branch name.
 *
 * @returns The current branch name.
 */
export function getCurrentBranch(): string | undefined {
  try {
    return execSync(`git rev-parse --abbrev-ref HEAD`).toString().trim()
  }
  catch(err) {
    throw new Error("Failed to retrieve the current branch name.");
  }
}

/**
 * Computes and returns a valid semantic version based on the content of the `version.json` file in the
 * workspace root, the git height since the file was last updated, and the commit hash if the current branch is
 * not designated as a branch that packages should be released from.
 *
 * @param rootDir - The path representing the workspace root.
 * @param local When `true`, forces the returned version to be based on the current commit identifier.
 * @returns A string representing the computed version.
 */
export function getVersion(context: ExecutorContext, local: boolean = false): string {
  const currentTag = getCurrentTag();
  const currentTagVersionParts = SemVerRegEx.exec(currentTag)?.groups;

  if(currentTagVersionParts) return currentTag;

  const versionInfoFile = join(context.root, "version.json");
  const versionFileCommit = getLatestCommit(versionInfoFile);

  if(!versionFileCommit) {
    logger.warn("Unable to determine a valid version. The 'version.json' file is not part of the working tree.");

    return "0.0.0";
  }

  const versionInfo = readJsonFile(versionInfoFile);
  const versionParts = SemVerRegEx.exec(versionInfo.version)?.groups;

  if(!versionParts) {
  	logger.fatal(`The 'version.json' file is invalid. '${versionInfo.version}' is an invalid version number.`);

    return "0.0.0";
  }

  const currentBranchName = getCurrentBranch();

  if(versionInfo.releaseBranches.find((releaseBranch) => currentBranchName.startsWith(releaseBranch)) && !local) {
    const gitHeight = Number((execSync(`git log --format=%h ${versionFileCommit}.. | wc -l`)).toString().trim()) + 1;

    return `${versionInfo.version}.${gitHeight}`;
  }

  const commitId = getLatestCommit();

  return `${versionInfo.version}-g${commitId}`;
}

/**
 * An Nx executor that updates the Rust crate and Node.js packages that are present in the current
 * workspace.
 */
export default async function versionPackagesExecutor(options: Options, context: ExecutorContext) {
	const version = getVersion(context);
	const projectRoot = join(
		context.root,
		context.workspace.projects[context.projectName].root);

	for (const packageTarget of options.packages) {
		const packageTargetPath = resolve(projectRoot, packageTarget);

		logger.info(`Setting '${packageTargetPath}' to version '${version}'.`);

		if (packageTargetPath.endsWith("/package.json")) {
			writePackageJson(packageTargetPath, version);
		}
		else if (packageTargetPath.endsWith("/Cargo.toml")) {
			await writeTomlManifest(packageTargetPath, version);
		}
		else {
			logger.fatal(`Unsupported package target: '${packageTargetPath}'.`);
		}
	}

	return {
		success: true
	}
}

