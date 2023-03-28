import { Uri, workspace, RelativePattern } from "vscode";
import { BaseLanguageClient, DocumentUri, TextDocumentIdentifier } from "vscode-languageclient";

declare module "vscode-languageclient" {
	interface BaseLanguageClient {
		/**
		 * An extension method that adds P4Analyzer specific request handling to the current
		 * language client.
		 */
		setP4AnalyzerHandlers(): void;
	}
}

interface FolderIdentifier {
	uri: string;
}

function setP4AnalyzerHandlers(this: BaseLanguageClient): void {
	this.onRequest("p4analyzer/enumerateFolder", async (params: FolderIdentifier) => {
		const uri = Uri.parse(params.uri);
		const folder = workspace.getWorkspaceFolder(uri);

		if (!folder) throw new Error(`Invalid Workspace ('${uri.toString()}')`);

		const files = await workspace.findFiles(new RelativePattern(folder, "**/*.p4").pattern);

		return files.map(file => TextDocumentIdentifier.create(file.toString()));
	});

	this.onRequest("p4analyzer/fileContents", async (params: TextDocumentIdentifier) => {

	});
}

BaseLanguageClient.prototype.setP4AnalyzerHandlers = setP4AnalyzerHandlers;
