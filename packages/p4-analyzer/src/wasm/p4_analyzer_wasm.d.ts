/*
 * This file declares the types that are exported from the P4 Analyzer webassembly module.
 */

/**
 * A callback for receiving the `Buffer` instances that contain LSP response data.
 *
 * @internal
 */
export type OnResponseCallback = (data: Buffer) => void;

/**
 * The P4 Language Server Protocol (LSP) server.
 *
 * @internal
 */
export declare class LspServer {
	/**
	 * Initializes a new {@link LspServer}.
	 *
	 * @param onResponse An {@link OnResponseCallback} function that will receive buffers
	 * of data that represent LSP response messages.
	 */
	constructor(onResponse: OnResponseCallback);

	/**
	 * Starts the {@link LspServer}.
	 */
	start(): Promise<void>;

	/**
	 * Sends a buffer containing data representing a LSP request message to the {@link LspServer}.
	 *
	 * @param requestBuffer The `Buffer` that contains the request message.
	 * @returns A `Promise` that yields when the message consumed from `requestBuffer` has been submitted
	 * for processing; or rejects if the message could not not be processed.
	 *
	 * @remarks

	 */
	 sendRequest(requestBuffer: Buffer): Promise<void>;

	/**
	 * Stops the {@link LspServer}.
	 */
	stop(): void;
}
