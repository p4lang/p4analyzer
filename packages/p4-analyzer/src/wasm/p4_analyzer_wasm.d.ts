/*
 * This file declares the types that are exported from the P4 Analyzer webassembly module.
 */

/**
 * A callback for receiving `Buffer` instances that contain LSP response data.
 */
export type OnReceiveResponseBufferCallback = (data: Buffer) => void;

/**
 * The P4 Language Server Protocol (LSP) server.
 */
export declare class LspServer {
	/**
	 * Initializes a new {@link LspServer}.
	 *k
	 * @param onReceiveResponseCallback An {@link OnReceiveResponseBufferCallback} function that will receive the
	 * buffers of data representing responses.
	 */
	constructor(onReceiveResponseCallback: OnReceiveResponseBufferCallback);

	/**
	 * Starts the {@link LspServer}.
	 */
	start(): Promise<void>;

	/**
	 * Sends a buffer containing data representing LSP request data to the {@link LspServer}.
	 *
	 * @param requestBuffer The `Buffer` that contains the request data.
	 * @returns A `Promise` that yields a `Buffer`.
	 *
	 * @remarks
	 * The {@link LspServer} will attempt to read a message from `requestBuffer`. If a message could not be read, then the
	 * returned `Buffer` will be `requestBuffer`. However, if a message could be read, then the message is consumed and the
	 * returned `Buffer` will represent the slice of unconsumed data that follows it.
	 *
	 * Typical calling code will maintain a running `Buffer`, and on receiving new messsage data, calls
	 * {@link LspServer.sendRequestBuffer} supplying the concatination of the current running buffer and the new data, before
	 * assigning the returned buffer to the running buffer. This results in a running buffer that expands and contracts as
	 * messages are processed by the P4 Analyzer webassembly instance.
	 */
	sendRequestBuffer(requestBuffer: Buffer): Promise<Buffer>;

	/**
	 * Stops the {@link LspServer}.
	 */
	stop(): void;
}
