// App bootstrap helper. A small state machine that runs an ordered list of
// initialization steps with per-step retry + timeout, and surfaces which
// steps failed so the caller can decide whether to render the app anyway
// (non-critical failures) or block on a retry (critical failures).

export interface InitializationStep {
	name: string;
	execute: () => Promise<void>;
	critical: boolean;
	retryCount: number;
	timeout: number; // ms
}

export interface InitializationResult {
	success: boolean;
	criticalFailure: boolean;
	completedSteps: string[];
	failedSteps: Array<{ name: string; error: Error }>;
	skippedSteps: string[];
	totalDuration: number;
}

export interface InitializerOptions {
	maxGlobalRetries?: number;
	globalTimeout?: number;
	onStepStart?: (stepName: string) => void;
	onStepComplete?: (stepName: string, duration: number) => void;
	onStepError?: (stepName: string, error: Error, attempt: number) => void;
	onStepSkipped?: (stepName: string, reason: string) => void;
}

function withTimeout<T>(p: Promise<T>, ms: number, label: string): Promise<T> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
		p.then(
			(v) => {
				clearTimeout(timer);
				resolve(v);
			},
			(e) => {
				clearTimeout(timer);
				reject(e);
			}
		);
	});
}

export class AppInitializer {
	constructor(private readonly opts: InitializerOptions = {}) {}

	async initialize(steps: InitializationStep[]): Promise<InitializationResult> {
		const start = performance.now();
		const completed: string[] = [];
		const failed: Array<{ name: string; error: Error }> = [];
		const skipped: string[] = [];
		let criticalFailure = false;

		for (const step of steps) {
			this.opts.onStepStart?.(step.name);
			const stepStart = performance.now();
			let lastError: Error | null = null;
			let success = false;

			for (let attempt = 1; attempt <= step.retryCount; attempt++) {
				try {
					await withTimeout(step.execute(), step.timeout, step.name);
					success = true;
					break;
				} catch (e) {
					lastError = e instanceof Error ? e : new Error(String(e));
					this.opts.onStepError?.(step.name, lastError, attempt);
				}
			}

			if (success) {
				const dur = performance.now() - stepStart;
				completed.push(step.name);
				this.opts.onStepComplete?.(step.name, Math.round(dur));
			} else if (step.critical) {
				failed.push({ name: step.name, error: lastError ?? new Error('unknown failure') });
				criticalFailure = true;
				break; // Critical failures abort remaining steps.
			} else {
				skipped.push(step.name);
				this.opts.onStepSkipped?.(step.name, lastError?.message ?? 'unknown');
			}
		}

		return {
			success: !criticalFailure,
			criticalFailure,
			completedSteps: completed,
			failedSteps: failed,
			skippedSteps: skipped,
			totalDuration: Math.round(performance.now() - start)
		};
	}
}

// Helper used by the App.svelte init flow to poll for backend readiness.
export async function waitForCondition(
	check: () => Promise<boolean>,
	opts: { timeout: number; pollInterval: number; name?: string }
): Promise<boolean> {
	const deadline = Date.now() + opts.timeout;
	while (Date.now() < deadline) {
		try {
			if (await check()) return true;
		} catch {
			// Treat thrown checks as "not ready yet".
		}
		await new Promise((r) => setTimeout(r, opts.pollInterval));
	}
	return false;
}
