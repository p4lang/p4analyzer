import type { LspServer } from "../wasm/p4_analyzer_wasm";

/**
 * @internal
 */
const BODY_SPLIT_CHARS = "\r\n\r\n";

/**
 * @internal
 */
const BUFFER_CHUNK_SIZEBYTES = 2_048;

/**
 * @internal
 */
const CONTENTLENGTH_HEADER_EXP = /Content-Length:\s(?<length>\d*)\s?/;

/**
 * An adapter for the P4 Analyzer webassembly module.
 *
 * @public
 */
export default class {
	private server: LspServer | null = null;
	private requestBuffer: Buffer | null = null;
	private requestBufferOffset: number | null = null;
	private requestMessageBufferQueue: Buffer[] | null = null;

	/**
	 * Imports and then starts an underlying P4 Analyzer webassembly instance.
	 *
	 * @remarks
	 * Once started, the `'stdin'` and `'stdout'` for the process will be attached to the P4 Analyzer instance.
	 * Buffers of data that represent the requests and responses of the Language Server Protocol (LSP), will be
	 * sent and received directly by the adapter and then forwarded to the P4 Analyzer webassembly instance.
	 */
	async start(): Promise<void> {
		const {LspServer} = await import("../wasm/p4_analyzer_wasm");

		this.server = new LspServer(this.onResponseBuffer.bind(this));
		this.requestBuffer = Buffer.alloc(BUFFER_CHUNK_SIZEBYTES); // Initialize an empty request buffer.
		this.requestBufferOffset = 0;
		this.requestMessageBufferQueue = [];

		process.stdin.on("data", this.onReceiveRequestBuffer.bind(this));

		try {
			await this.server.start();
		}
		finally {
			process.stdin.pause();
			process.stdout.pause();
		}
	}

	/**
	 * Stops the underlying P4 Analyzer webassembly instance.
	 */
	stop(): void {
		if (!this.server) throw new Error("The server has not been started.");

		this.server.stop();
	}

	/**
	 * Called when data is received on `'stdin'`.
	 *
	 * @param chunk A `Buffer` representing the captured data from `'stdin'`.
	 *
	 * The received chunks are copied into a _running_ `Buffer` until a message header and body are received.
	 * That message is then read out of the _running_ buffer and sent to the P4 Analyzer webassembly instance
	 * for processing.
	 */
	private onReceiveRequestBuffer(chunk: Buffer): void {
		if (this.requestBufferOffset + chunk.byteLength > this.requestBuffer.byteLength) {
			// Reallocate a larger request buffer.
			this.requestBuffer = Buffer.concat([this.requestBuffer, Buffer.alloc(BUFFER_CHUNK_SIZEBYTES * (Math.floor(chunk.byteLength / BUFFER_CHUNK_SIZEBYTES) + 1))]);
		}

		this.requestBufferOffset += chunk.copy(this.requestBuffer, this.requestBufferOffset, chunk.byteOffset, chunk.byteLength);

		const splitOffset = this.requestBuffer.indexOf(BODY_SPLIT_CHARS);

		if (splitOffset < 0) return; // Still reading the headers. Continue until we get them.

		const headers = this.requestBuffer.subarray(0, splitOffset).toString();
		const contentLength = Number(CONTENTLENGTH_HEADER_EXP.exec(headers)?.groups["length"]);

		if (isNaN(contentLength)) throw new Error("Received malformed message. 'Content-Length' header is missing.");

		const minRequiredOffset = splitOffset + BODY_SPLIT_CHARS.length + contentLength;

		if (this.requestBufferOffset < minRequiredOffset) return; // Still reading the expected body. Continue until we get it.

		// We have now received at least one full message. Pull it out and reset the request buffer to continue reading
		// the next one.
		const requestMessageBuffer = Buffer.alloc(contentLength);

		this.requestBuffer.copy(requestMessageBuffer, 0, splitOffset + BODY_SPLIT_CHARS.length, minRequiredOffset);
		this.requestBuffer.copy(this.requestBuffer, 0, minRequiredOffset);
		this.requestBufferOffset = 0;

		this.requestMessageBufferQueue.push(requestMessageBuffer);

		setImmediate(this.sendRequestMessageBuffer.bind(this)); // Queue the request message buffer for processing.
	}

	/**
	 * Dequeues a request message and sends it to the P4 Analyzer webassembly instance.
	 */
	private async sendRequestMessageBuffer(): Promise<void> {
		await this.server.sendRequest(this.requestMessageBufferQueue.shift());
	}

	/**
	 * An event handler that writes a received response message (from the P4 Analyzer webassembly instance), to the
	 * currently attached `'stdout'`.
	 *
	 * @param message The `Buffer` representing the response message to write.
	 */
	private onResponseBuffer(message: Buffer): void {
		process.stdout.write(`Content-Length: ${message.byteLength}${BODY_SPLIT_CHARS}${message}`);
	}
}
