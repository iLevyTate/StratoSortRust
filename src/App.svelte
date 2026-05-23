<script lang="ts">
	import './app.css';
	import { ModeWatcher } from 'mode-watcher';
	import { Toaster } from 'svelte-sonner';
	import { onMount, onDestroy } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { currentPage, initializeEventListeners, cleanupEventListeners, appSettings } from '$lib/stores';
	import DiscoverPage from '$lib/components/pages/DiscoverPage.svelte';
	import AnalyzePage from '$lib/components/pages/AnalyzePage.svelte';
	import OrganizePage from '$lib/components/pages/OrganizePage.svelte';
	import SettingsPage from '$lib/components/pages/SettingsPage.svelte';
	import SmartFoldersManager from '$lib/components/SmartFoldersManager.svelte';
	import FirstRunSetupPage from '$lib/components/pages/FirstRunSetupPage.svelte';
	import NotificationCenter from '$lib/components/NotificationCenter.svelte';
	import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';
	import KeyboardShortcutsHelp from '$lib/components/KeyboardShortcutsHelp.svelte';
	import ProgressIndicator from '$lib/components/ProgressIndicator.svelte';
	import { getAppSettings, checkOllamaStatus, checkFirstRunStatus, getSystemInfo, frontendReady } from '$lib/api/tauri';
	import { keyboardShortcuts } from '$lib/utils/keyboard-shortcuts';
	import { operationInProgress } from '$lib/stores';
	import { toast } from '$lib/stores/notifications';
	import { AppInitializer, type InitializationStep, waitForCondition } from '$lib/utils/initialization';

	let page: string = 'discover';
	let initialized = false;
	let isFirstRun = false;
	let showingFirstRunSetup = false;
	let showKeyboardHelp = false;

	onMount(() => {
		// Subscribe to page changes
		const unsubscribe = currentPage.subscribe((value) => {
			page = value;
		});

		// Listen for keyboard help events
		const handleShowKeyboardHelp = () => {
			showKeyboardHelp = true;
		};
		document.addEventListener('show-keyboard-help', handleShowKeyboardHelp);

		// Async initialization
		const initAsync = async () => {
			try {
				const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
				// Check if this is a first run
				const firstRunStatus = await checkFirstRunStatus();
				isFirstRun = firstRunStatus.is_first_run;
				showingFirstRunSetup = isFirstRun;

				// In non-Tauri environments (web/e2e), skip backend initialization
				if (!isTauri) {
					// Force bypass first-run wizard in e2e
					isFirstRun = false;
					showingFirstRunSetup = false;
					initialized = true;
					return;
				}

				if (!isFirstRun) {
					// Not first run, proceed with normal initialization
					await initializeApp();
					// Only set initialized = true after initializeApp() completes successfully
					initialized = true;
					console.log('StratoSort app initialized successfully');
				}
				// If it is first run, we'll initialize after setup is complete
				// Don't set initialized = true yet for first run scenarios

			} catch (error) {
				console.error('Failed to initialize app:', error);
				if (!isFirstRun) {
					// Only show UI for non-first-run errors to avoid broken state
					initialized = true;
				}
				// For first-run errors, stay in loading state to allow retry
			}
		};

		initAsync();

		return () => {
			unsubscribe();
			document.removeEventListener('show-keyboard-help', handleShowKeyboardHelp);
		};
	});

	async function initializeApp() {
		const initializer = new AppInitializer({
			maxGlobalRetries: 3,
			globalTimeout: 45000, // 45 seconds total timeout
			onStepStart: (stepName) => console.log(`🔄 Starting: ${stepName}`),
			onStepComplete: (stepName, duration) => console.log(`✅ Completed: ${stepName} (${duration}ms)`),
			onStepError: (stepName, error, attempt) => console.warn(`❌ Error in ${stepName} (attempt ${attempt}):`, error),
			onStepSkipped: (stepName, reason) => console.warn(`⏭️ Skipped: ${stepName} - ${reason}`)
		});

		// Define robust initialization steps
		const initSteps: InitializationStep[] = [
			{
				name: 'Initialize Event Listeners',
				execute: async () => {
					await initializeEventListeners();
				},
				critical: true, // Critical - app won't work without event listeners
				retryCount: 3,
				timeout: 10000
			},
			{
				name: 'Signal Frontend Ready',
				execute: async () => {
                    await frontendReady();
				},
				critical: true, // Critical - backend needs to know frontend is ready
				retryCount: 3,
				timeout: 5000
			},
			{
				name: 'Wait for Backend Readiness',
				execute: async () => {
					// Replace hardcoded delay with intelligent waiting
					const isBackendReady = await waitForCondition(
						async () => {
							try {
								// Test if backend is responsive by checking system info
								await getSystemInfo();
								return true;
							} catch {
								return false;
							}
						},
						{
							timeout: 10000,
							pollInterval: 500,
							name: 'backend readiness'
						}
					);

					if (!isBackendReady) {
						throw new Error('Backend did not become ready within timeout');
					}
				},
				critical: true, // Critical - need backend to be ready
				retryCount: 2,
				timeout: 12000
			},
			{
				name: 'Load App Settings',
				execute: async () => {
					const settings = await getAppSettings();
					if (settings) {
						appSettings.set(settings);
					} else {
						console.warn('No settings returned from backend, using defaults');
					}
				},
				critical: false, // Non-critical - will use defaults
				retryCount: 2,
				timeout: 5000
			},
			{
				name: 'Load System Information',
				execute: async () => {
					const systemInfo = await getSystemInfo();
					console.log('System info loaded:', systemInfo);
				},
				critical: false, // Non-critical - informational only
				retryCount: 1,
				timeout: 3000
			},
			{
				name: 'Check AI Service Status',
				execute: async () => {
					const ollamaStatus = await checkOllamaStatus();
					console.log('AI service status:', ollamaStatus);
				},
				critical: false, // Non-critical - AI features may not be available
				retryCount: 2,
				timeout: 8000
			}
		];

		try {
			const result = await initializer.initialize(initSteps);

			if (!result.success) {
				const errorMsg = result.criticalFailure
					? 'Critical initialization failure - some core features may not work'
					: 'Initialization completed with some non-critical failures';

				throw new Error(errorMsg);
			}

			console.log(`🎉 App initialization completed successfully in ${result.totalDuration}ms`);
			console.log(`✅ Completed: ${result.completedSteps.length} steps`);

			if (result.skippedSteps.length > 0) {
				console.warn(`⚠️ Skipped: ${result.skippedSteps.length} non-critical steps`);
			}

		} catch (error) {
			console.error('🚨 App initialization failed:', error);
			throw error;
		}
	}

	async function onFirstRunSetupComplete() {
		showingFirstRunSetup = false;
		isFirstRun = false;

		// Now initialize the app
		try {
			await initializeApp();
			// Only set initialized = true after initializeApp() completes successfully
			initialized = true;
			console.log('StratoSort app initialized successfully after first-run setup');
		} catch (error) {
			console.error('Failed to initialize app after setup:', error);
			// Show user-facing error notification for critical failure
			// Using static import instead of dynamic import for reliable error handling
			try {
				toast.error('Failed to initialize application. Please restart the app.', {
					persistent: true,
					action: {
						label: 'Retry',
						onClick: () => window.location.reload()
					}
				});
			} catch (toastError) {
				// Fallback if toast system also fails - use native browser alert
				console.error('Toast notification also failed:', toastError);
				alert('Critical Error: Failed to initialize application. Please restart the app.');
			}
		}
	}

	function handleErrorBoundaryGoHome() {
		currentPage.set('discover');
	}

	onDestroy(() => {
		// Clean up event listeners when app is destroyed
		cleanupEventListeners();
		keyboardShortcuts.destroy();
	});
</script>

<ModeWatcher />
<Toaster richColors />

<ErrorBoundary on:goHome={handleErrorBoundaryGoHome}>
    {#if showingFirstRunSetup}
        <!-- First Run Setup -->
        <div data-testid="app-container">
            <FirstRunSetupPage onSetupComplete={onFirstRunSetupComplete} />
        </div>
    {:else}
        <!-- Main Application -->
        <a href="#main-content" data-testid="skip-to-content" class="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 bg-primary text-primary-foreground px-3 py-2 rounded">Skip to content</a>
        <div class="flex h-screen bg-background" data-testid="app-container">
            <Sidebar />
            <main id="main-content" data-testid="main-content" class="flex-1 overflow-hidden">
				<!-- Header with Global Progress and Notification Center -->
				<div class="flex items-center justify-between p-4 border-b">
					<div class="flex-1">
						{#if $operationInProgress}
							<ProgressIndicator
								variant="inline"
								compact={true}
								showCancel={false}
								showProgress={true}
							/>
						{/if}
					</div>
					<NotificationCenter />
				</div>
                <div class="h-full p-6 overflow-auto" style="height: calc(100vh - 73px);">
					{#if !initialized}
						<div class="flex items-center justify-center h-full">
							<div class="text-center">
								<div class="w-12 h-12 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
								<p class="text-lg font-semibold">Initializing StratoSort...</p>
								<p class="text-muted-foreground">Connecting to backend services</p>
							</div>
						</div>
                    {:else if page === 'discover'}
                        <ErrorBoundary fallback={false} let:captureError>
                            <DiscoverPage />
						</ErrorBoundary>
					{:else if page === 'analyze'}
                        <ErrorBoundary fallback={false} let:captureError>
                            <AnalyzePage />
						</ErrorBoundary>
					{:else if page === 'organize'}
                        <ErrorBoundary fallback={false} let:captureError>
                            <OrganizePage />
						</ErrorBoundary>
                    {:else if page === 'settings'}
                        <ErrorBoundary fallback={false} let:captureError>
                            <SettingsPage />
						</ErrorBoundary>
					{/if}
				</div>
                <!-- ARIA live regions for accessibility announcements -->
                <div aria-live="polite" class="sr-only" data-testid="live-region-polite"></div>
                <div aria-live="assertive" class="sr-only" data-testid="live-region-assertive"></div>
			</main>
		</div>
	{/if}
</ErrorBoundary>

<!-- Keyboard Shortcuts Help (Global) -->
<KeyboardShortcutsHelp bind:showHelp={showKeyboardHelp} />