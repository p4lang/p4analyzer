import type { LspServer } from "../wasm/p4_analyzer_wasm";

/**
 * An adapter for the P4 Analyzer webassembly module.
 */
export default class {
	private server: LspServer | null = null;
	private requestBuffer: Buffer | null = null;

	/**
	 * Imports and then starts an underlying P4 Analyzer webassembly instance.
	 *
	 * @remarks
	 * Once started, the `stdin` and `stdout` for the process will be attached to the P4 Analyzer instance.
	 * Buffers of data that represent the requests and responses of the Language Server Protocol (LSP), will be
	 * sent and received directly by the adapter and then forwarded to the P4 Analyzer webassembly instance.
	 */
	async start(): Promise<void> {
		const {LspServer} = await import("../wasm/p4_analyzer_wasm");

		this.server = new LspServer(this.onReceiveResponseBuffer.bind(this));
		this.requestBuffer = Buffer.alloc(0); // Initialize an empty request buffer.

		process.stdin.on("data", this.onReceiveRequestBuffer.bind(this));

		await this.server.start();
	}

	/**
	 * Stops the underlying P4 Analyzer webassembly instance.
	 */
	stop(): void {
		if (!this.server) throw new Error("The server has not started.");

		this.server.stop();

		process.stdin.pause();
		process.stdout.pause();
	}

	/**
	 * An event handler that forwards a received request buffer (read from the currently attached `stdin`), to the P4 Analyzer
	 * webassembly instance.
	 *
	 * @param data The `Buffer` that should be forwarded.
	 */
	private async onReceiveRequestBuffer(data: Buffer): Promise<void> {
		this.requestBuffer = await this.server.sendRequestBuffer(Buffer.concat([this.requestBuffer, data]));
	}

	/**
	 * An event handler that writes a received response buffer (from the P4 Analyzer webassembly instance), to the
	 * currently attached `stdout`.
	 *
	 * @param data The `Buffer` to write.
	 */
	private onReceiveResponseBuffer(data: Buffer): void {
		process.stdout.write(data);
	}
}
