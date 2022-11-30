import P4Analyzer from "./server";

/**
 * The P4 Analyzer entry point.
 */
async function main(): Promise<void> {
	const analyzer = new P4Analyzer();

	let count = -1;

	process.on("SIGINT", () => {
		count++;

		if (count === 0) {
			console.error();
			console.error("(To forcibly exit, press 'Ctrl+C' again)");

			analyzer.stop();
		}

		if (count > 0) {
			process.exit(-1);
		}
	});

	try {
		await analyzer.start();
	}
	catch (err: unknown) {
		console.error(err instanceof Error ? err.message : String(err));
	}
}

void main();
