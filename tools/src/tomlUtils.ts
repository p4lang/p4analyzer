import { parse, stringify  } from "@iarna/toml";
import { readFile, writeFile} from "node:fs";

export function writeTomlManifest(filePath: string, version: string): Promise<void> {
	return new Promise<void>((resolve, reject) => {
		readFile(filePath, "utf8", (err, data) => {
			if (err) return reject(err);

			const toml = parse(data.toString());

			toml["package"]["version"] = version;

			writeFile(filePath, stringify(toml), "utf8", (err) => {
				if (err) return reject(err);

				resolve();
			});
		});
	});
}
