import { Uri, workspace, RelativePattern } from "vscode";
import { BaseLanguageClient, DocumentUri, TextDocumentIdentifier } from "vscode-languageclient";
import { readFile } from "node:fs/promises";

declare module "vscode-languageclient" {
	interface BaseLanguageClient {
		/**
		 * An extension method that adds P4Analyzer specific request handling to the current
		 * language client.
		 */
		setP4AnalyzerHandlers(): void;
	}
}

interface EnumerateFolderParams {
	uri: string;
	filePattern: string;
}

function setP4AnalyzerHandlers(this: BaseLanguageClient): void {
	this.onRequest("p4analyzer/enumerateFolder", async (params: EnumerateFolderParams) => {
		const uri = Uri.parse(params.uri);
		const folder = workspace.getWorkspaceFolder(uri);

		if (!folder) throw new Error(`Invalid or unknown workspace ('${uri.toString()}')`);

		const files = await workspace.findFiles(new RelativePattern(folder, params.filePattern).pattern);

		return files.map(file => TextDocumentIdentifier.create(file.toString()));
	});

	this.onRequest("p4analyzer/fileContents", async (params: TextDocumentIdentifier) => {
		const uri = Uri.parse(params.uri);

		return await readFile(uri.fsPath, { flag: "r",  encoding: "utf-8"});
	});
}

BaseLanguageClient.prototype.setP4AnalyzerHandlers = setP4AnalyzerHandlers;
