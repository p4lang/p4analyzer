import { readJsonFile, writeJsonFile } from "@nrwl/devkit";

export function writePackageJson(filePath: string, version: string) {
	const json = readJsonFile(filePath);

	json["version"] = version;

	writeJsonFile(filePath, json);
}

