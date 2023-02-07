import { window, ExtensionContext } from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind, Trace } from "vscode-languageclient/node";
import { getServerConfiguration } from "./configuration";

let client: LanguageClient | null = null;

/**
 * The function that is invoked when the extension is activated.
 *
 * @param context The extension context.
 * @returns A `Promise` that resolves when the extension has activated.
 *
 * @public
 */
export async function activate(context: ExtensionContext): Promise<void> {
	try {
		await onTryActivate(context);
	}
	catch (err: unknown) {
		void window.showErrorMessage(`Failed to activate P4 Analyzer: ${err instanceof Error ? err.message : err}`);

		throw err;
	}
}

/**
 * The function that is invoked when the extension is deactivated by Visual Studio Code.
 *
 * @returns A `Promise` that resolves when the extension has completed its deactivation.
 *
 * @public
 */
export function deactivate(): Promise<void> | undefined {
	return client?.stop();
}

async function onTryActivate(context: ExtensionContext): Promise<void> {
	const absoluteServerPath = getServerConfiguration().get<string | null>("absoluteServerPath");
	const serverOptions: ServerOptions = {
		// If we have a server path then launch that executable, otherwise use the integrated Node.js/WASM server.
		...absoluteServerPath
			? { command: absoluteServerPath, args: getServerArguments() }
			: { module: require.resolve("p4-analyzer", { paths: [context.extensionPath] }) },
		transport: TransportKind.stdio
	}

	const clientOptions: LanguageClientOptions = {
		documentSelector: [
			{ scheme: "file", language: "p4" }
		],
		traceOutputChannel: window.createOutputChannel("P4 Analyzer Language Server - Trace", "p4")
	}

	client = new LanguageClient("p4-analyzer", "P4 Analyzer Language Server", serverOptions, clientOptions);
	client.setTrace(Trace.Messages);
	client.start();
}

function getServerArguments(): string[] {
	const serverConfiguration = getServerConfiguration();
	const logPath = serverConfiguration.get<string | null>("logPath");

	return logPath
		? ["--logpath", logPath, "--loglevel", serverConfiguration.get<string>("logLevel")]
		: []
}
