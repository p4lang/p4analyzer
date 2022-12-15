import { workspace, ConfigurationScope } from "vscode";

const CONFIG_BASE = "p4-analyzer";

/**
 * A utility type that provides keyed access to the values of a `WorkspaceConfiguration` object.
 *
 * @typeParam T
 * The type from which the keyed properties can be read.
 *
 * @internal
 */
 export type WorkspaceConfigurationAccessor<T> = {
	get: <R>(key: keyof T) => R;

	has: (key: keyof T) => boolean;
}

/**
 * Defines the configuration properties that are available under the `'server'` section.
 *
 * @internal
 */
export interface ServerConfiguration {
	/**
	 * Gets the absolute path to the configured P4 Analyzer server.
	 *
	 * @remarks
	 * If `null`, indicating that no path is set, then the extension should fall back to using the integrated
	 * WebAssembly based server instead.
	 *
	 */
	absoluteServerPath: string | null;
}

/**
 * Retrieves the server configuration.
 *
 * @param scope An optional scope for which the configuration is required for.
 * @returns A typed `WorkspaceConfiguration` object that can access the {@link ServerConfiguration}.
 */
export function getServerConfiguration(scope?: ConfigurationScope): WorkspaceConfigurationAccessor<ServerConfiguration> {
	return workspace.getConfiguration(`${CONFIG_BASE}.server`, scope);
}
