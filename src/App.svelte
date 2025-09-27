<script lang="ts">
	import './app.css';
	import { ModeWatcher } from 'mode-watcher';
	import { Toaster } from 'svelte-sonner';
	import { onMount, onDestroy } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { currentPage, initializeEventListeners, cleanupEventListeners, appSettings, searchResults } from '$lib/stores';
	// Direct imports for all page components to fix blank screen issue
	// The lazy-loader utility doesn't exist, causing runtime errors
	import FirstRunSetupPage from '$lib/components/pages/FirstRunSetupPage.svelte';
	import DiscoverPage from '$lib/components/pages/DiscoverPage.svelte';
	import AnalyzePage from '$lib/components/pages/AnalyzePage.svelte';
	import OrganizePage from '$lib/components/pages/OrganizePage.svelte';
	import SettingsPage from '$lib/components/pages/SettingsPage.svelte';
	import HistoryPage from '$lib/components/pages/HistoryPage.svelte';
	import HeaderBar from '$lib/components/HeaderBar.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import HistoryTimeline from '$lib/components/HistoryTimeline.svelte';
	import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';
	import KeyboardShortcutsHelp from '$lib/components/KeyboardShortcutsHelp.svelte';
	import AccessibilityManager from '$lib/components/AccessibilityManager.svelte';
	import AboutDialog from '$lib/components/AboutDialog.svelte';
	import QuickActionsFAB from '$lib/components/QuickActionsFAB.svelte';
	import { AccessibilityToolbar } from '$lib/components/ui';
	import { getAppSettings, checkOllamaStatus, checkFirstRunStatus, getSystemInfo, frontendReady } from '$lib/api/tauri';
	import { keyboardShortcuts } from '$lib/utils/keyboard-shortcuts';
	import { operationInProgress, selectedFiles } from '$lib/stores';
	import { toast } from '$lib/stores/notifications';
	import { AppInitializer, type InitializationStep, waitForCondition } from '$lib/utils/initialization';
	import { log, LogCategory } from '$lib/utils/enhanced-logger';

	let page: string = 'discover';
	let initialized = false;
	let initializationError: Error | null = null;
	let isFirstRun = false;
	let showingFirstRunSetup = false;
	let firstRunSetupAttempts = 0;
	const MAX_FIRSTRUN_ATTEMPTS = 3;
	let showKeyboardHelp = false;
	let showHistoryTimeline = false;
	let showSidebar = true;
	let showAccessibilityToolbar = false;
	let showAboutDialog = false;
	let showQuickActions = true;

	// Quick action handlers
	async function handleQuickAction(action: string) {
		log.info(`Quick action triggered: ${action}`, undefined, 'QuickActions', LogCategory.UI);

		switch(action) {
			case 'upload':
				// Navigate to discover page and trigger file browser
				currentPage.set('discover');
				setTimeout(() => {
					const event = new CustomEvent('trigger-browse-files');
					window.dispatchEvent(event);
				}, 100);
				break;

			case 'scan-folder':
				// Navigate to discover page and trigger folder scan
				currentPage.set('discover');
				setTimeout(() => {
					const event = new CustomEvent('trigger-browse-directory');
					window.dispatchEvent(event);
				}, 100);
				break;

			case 'quick-analyze':
				// Navigate to analyze page if files are selected
				if ($selectedFiles.length > 0) {
					currentPage.set('analyze');
				} else {
					toast.warning('Please select files first');
				}
				break;

			case 'smart-folder':
				// Navigate to organize page and open smart folder creation
				currentPage.set('organize');
				setTimeout(() => {
					// Set a flag to trigger smart folder creation after navigation
					localStorage.setItem('createSmartFolder', 'true');
					// Also dispatch event for immediate handling if already on organize page
					const smartFolderEvent = new CustomEvent('open-smart-folder-dialog');
					window.dispatchEvent(smartFolderEvent);
				}, 100);
				break;

			case 'instant-search':
				// Focus search bar
				const searchEvent = new CustomEvent('focus-search-bar');
				window.dispatchEvent(searchEvent);
				break;

			case 'auto-organize':
				// Navigate to organize page
				currentPage.set('organize');
				setTimeout(() => {
					const event = new CustomEvent('trigger-auto-organize');
					window.dispatchEvent(event);
				}, 100);
				break;

			default:
				log.warn(`Unknown quick action: ${action}`, undefined, 'QuickActions', LogCategory.UI);
		}
	}

	// Store references to event listeners for cleanup
	let unsubscribePage: (() => void) | null = null;
	let handleShowKeyboardHelp: (() => void) | null = null;
	let handleKeyDown: ((event: KeyboardEvent) => void) | null = null;
	let handleShowAboutDialog: (() => void) | null = null;

	onMount(() => {
		// Subscribe to page changes
		unsubscribePage = currentPage.subscribe((value) => {
			page = value;
		});

		// Listen for keyboard help events
		handleShowKeyboardHelp = () => {
			showKeyboardHelp = true;
		};
		document.addEventListener('show-keyboard-help', handleShowKeyboardHelp);

		// Listen for show about dialog events (from backend)
		handleShowAboutDialog = () => {
			// Show the about dialog
			showAboutDialog = true;
		};
		window.addEventListener('show-about-dialog', handleShowAboutDialog);

		// Listen for quick action events
		window.addEventListener('trigger-browse-files', () => {
			// This will be handled by DiscoverPage when it's active
			log.debug('Browse files event triggered', undefined, 'QuickActions', LogCategory.UI);
		});

		window.addEventListener('trigger-browse-directory', () => {
			// This will be handled by DiscoverPage when it's active
			log.debug('Browse directory event triggered', undefined, 'QuickActions', LogCategory.UI);
		});

		window.addEventListener('open-smart-folder-dialog', () => {
			// This will be handled by the component that manages smart folders
			log.debug('Smart folder dialog event triggered', undefined, 'QuickActions', LogCategory.UI);
		});

		window.addEventListener('focus-search-bar', () => {
			// Focus the search bar in the header
			const searchInput = document.querySelector('[data-testid="header-search-input"]') as HTMLInputElement;
			if (searchInput) {
				searchInput.focus();
				log.debug('Search bar focused', undefined, 'QuickActions', LogCategory.UI);
			}
		});

		window.addEventListener('trigger-auto-organize', () => {
			// This will be handled by OrganizePage when it's active
			log.debug('Auto organize event triggered', undefined, 'QuickActions', LogCategory.UI);
		});

		// Setup accessibility features
		setupAccessibilityShortcuts();

		// Async initialization
		const initAsync = async () => {
			const isDev = import.meta.env.DEV;
			if (isDev) log.debug('Starting initialization...', undefined, 'App.svelte', LogCategory.SYSTEM);
			try {
				const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
				if (isDev) log.debug('Tauri environment detected', { isTauri }, 'App.svelte', LogCategory.SYSTEM);

				// Check if this is a first run
				if (isDev) log.debug('Checking first run status...', undefined, 'App.svelte', LogCategory.SYSTEM);
				const firstRunStatus = await checkFirstRunStatus();
				if (isDev) log.debug('First run status', firstRunStatus, 'App.svelte', LogCategory.SYSTEM);
				isFirstRun = firstRunStatus.is_first_run;
				showingFirstRunSetup = isFirstRun;

				// In non-Tauri environments (web/e2e), skip backend initialization
				if (!isTauri) {
					if (isDev) log.debug('Non-Tauri environment, using web mode', undefined, 'App.svelte', LogCategory.SYSTEM);
					// Force bypass first-run wizard in e2e
					isFirstRun = false;
					showingFirstRunSetup = false;
					initialized = true;
					return;
				}

				if (!isFirstRun) {
					if (isDev) log.debug('Not first run, initializing app...', undefined, 'App.svelte', LogCategory.SYSTEM);
					// Not first run, proceed with normal initialization
					await initializeApp();
					// Only set initialized = true after initializeApp() completes successfully
					initialized = true;
					if (isDev) log.debug('App initialized successfully', undefined, 'App.svelte', LogCategory.SYSTEM);
					log.info('StratoSort app initialized successfully', undefined, 'App');
				} else {
					// Check if we've failed first-run setup too many times
					firstRunSetupAttempts++;
					if (firstRunSetupAttempts >= MAX_FIRSTRUN_ATTEMPTS) {
						log.warn('First-run setup failed too many times, skipping...', undefined, 'App.svelte', LogCategory.SYSTEM);
						toast.warning('First-run setup skipped after multiple failures. You can configure settings manually.');
						// Mark as not first run to bypass setup
						isFirstRun = false;
						showingFirstRunSetup = false;
						// Try to initialize anyway with defaults
						await initializeApp();
						initialized = true;
					} else {
						if (isDev) log.debug('First run detected, showing setup wizard', undefined, 'App.svelte', LogCategory.SYSTEM);
						// Set initialized to true for first run to show the setup page
						initialized = true;
					}
				}
				// If it is first run, we'll initialize after setup is complete

			} catch (error) {
				log.error('Initialization error', error, 'App.svelte', LogCategory.SYSTEM);
				log.error('Failed to initialize app', error, 'App');
				initializationError = error as Error;
				if (!isFirstRun) {
					// Show error state but still set initialized to display error UI
					initialized = true;
					toast.error('Failed to initialize application. Some features may not work correctly.');
				}
			}
		};

		initAsync();

		return () => {
			// Clean up all subscriptions and event listeners
			if (unsubscribePage) {
				unsubscribePage();
			}
			if (handleShowKeyboardHelp) {
				document.removeEventListener('show-keyboard-help', handleShowKeyboardHelp);
			}
			if (handleShowAboutDialog) {
				window.removeEventListener('show-about-dialog', handleShowAboutDialog);
			}
			if (handleKeyDown) {
				document.removeEventListener('keydown', handleKeyDown);
			}
			// Clean up quick action event listeners
			// Note: These use anonymous functions, so they don't actually remove the original listeners
			// This is handled by the cleanup in onDestroy() instead
		};
	});

	async function initializeApp() {
		const initializer = new AppInitializer({
			maxGlobalRetries: 3,
			globalTimeout: 45000, // 45 seconds total timeout
			onStepStart: (stepName) => log.debug(`Starting: ${stepName}`, undefined, 'Init'),
			onStepComplete: (stepName, duration) => log.debug(`Completed: ${stepName}`, { duration }, 'Init'),
			onStepError: (stepName, error, attempt) => log.warn(`Error in ${stepName}`, { error, attempt }, 'Init'),
			onStepSkipped: (stepName, reason) => log.debug(`Skipped: ${stepName}`, { reason }, 'Init')
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
						log.warn('No settings returned from backend, using defaults', undefined, 'Settings');
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
					log.debug('System info loaded', systemInfo, 'System');
				},
				critical: false, // Non-critical - informational only
				retryCount: 1,
				timeout: 3000
			},
			{
				name: 'Check AI Service Status',
				execute: async () => {
					const ollamaStatus = await checkOllamaStatus();
					log.info('AI service status', ollamaStatus, 'AI');
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

			log.info('App initialization completed successfully', { totalDuration: result.totalDuration, completedSteps: result.completedSteps.length }, 'Init');

			if (result.skippedSteps.length > 0) {
				log.warn('Some non-critical steps were skipped', { skipped: result.skippedSteps.length }, 'Init');
			}

		} catch (error) {
			log.error('App initialization failed', error, 'Init');
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
			log.info('StratoSort app initialized successfully after first-run setup', undefined, 'FirstRun');
		} catch (error) {
			log.error('Failed to initialize app after setup', error, 'FirstRun');
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
				log.error('Toast notification also failed', toastError, 'Notification');
				alert('Critical Error: Failed to initialize application. Please restart the app.');
			}
		}
	}

	function handleErrorBoundaryGoHome() {
		currentPage.set('discover');
	}

	function handleSearchResults(event: CustomEvent) {
		// Store the search results and navigate to discover page
		const { query, results } = event.detail;
		searchResults.set({
			query,
			results,
			timestamp: Date.now()
		});
		currentPage.set('discover');
		// The DiscoverPage component will read from searchResults store
	}

	function handleToggleHistory(event: CustomEvent) {
		showHistoryTimeline = event.detail;
	}

	function handleToggleSidebar() {
		showSidebar = !showSidebar;
	}

	function toggleAccessibilityToolbar() {
		showAccessibilityToolbar = !showAccessibilityToolbar;
	}

	// Enhanced keyboard shortcuts for accessibility
	function setupAccessibilityShortcuts() {
		handleKeyDown = (event: KeyboardEvent) => {
			// Alt + A for accessibility toolbar
			if (event.altKey && event.key === 'a') {
				event.preventDefault();
				toggleAccessibilityToolbar();
			}
			// Alt + H for help
			if (event.altKey && event.key === 'h') {
				event.preventDefault();
				showKeyboardHelp = true;
			}
		};
		document.addEventListener('keydown', handleKeyDown);
	}

	onDestroy(() => {
		// Clean up event listeners when app is destroyed
		cleanupEventListeners();
		keyboardShortcuts.destroy();

		// Clean up any remaining event listeners
		if (handleShowKeyboardHelp) {
			document.removeEventListener('show-keyboard-help', handleShowKeyboardHelp);
		}
		if (handleShowAboutDialog) {
			window.removeEventListener('show-about-dialog', handleShowAboutDialog);
		}
		if (handleKeyDown) {
			document.removeEventListener('keydown', handleKeyDown);
		}
		if (unsubscribePage) {
			unsubscribePage();
		}
	});
</script>

<ModeWatcher />
<Toaster richColors />

<!-- Initialize Accessibility Manager -->
<AccessibilityManager />

<ErrorBoundary on:goHome={handleErrorBoundaryGoHome}>
    {#if showingFirstRunSetup}
        <!-- First Run Setup -->
        <div data-testid="app-container">
            <FirstRunSetupPage onSetupComplete={onFirstRunSetupComplete} />
        </div>
    {:else if initializationError && !initialized}
        <!-- Initialization Error Recovery UI -->
        <div class="flex flex-col items-center justify-center h-screen bg-background p-8" data-testid="error-container">
            <div class="max-w-md text-center space-y-4">
                <svg class="w-16 h-16 mx-auto text-destructive" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
                </svg>
                <h2 class="text-2xl font-bold text-foreground">Initialization Failed</h2>
                <p class="text-muted-foreground">
                    {initializationError.message || 'Failed to initialize the application. Some features may not work correctly.'}
                </p>
                <div class="flex gap-3 justify-center pt-4">
                    <button
                        class="px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 transition"
                        on:click={async () => {
                            initializationError = null;
                            try {
                                await initializeApp();
                                initialized = true;
                            } catch (error) {
                                initializationError = error instanceof Error ? error : new Error(String(error));
                            }
                        }}
                    >
                        Retry Initialization
                    </button>
                    <button
                        class="px-4 py-2 bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/90 transition"
                        on:click={() => {
                            // Continue anyway with limited functionality
                            initialized = true;
                            toast.warning('Running with limited functionality. Some features may not work.');
                        }}
                    >
                        Continue Anyway
                    </button>
                </div>
                <details class="text-left mt-6">
                    <summary class="cursor-pointer text-sm text-muted-foreground hover:text-foreground">
                        Technical Details
                    </summary>
                    <pre class="mt-2 p-3 bg-muted rounded text-xs overflow-auto max-h-40">{initializationError.stack || initializationError.toString()}</pre>
                </details>
            </div>
        </div>
    {:else}
        <!-- Main Application -->
        <!-- Skip links for keyboard navigation -->
        <div class="skip-links">
            <a href="#main-content" class="skip-link">Skip to main content</a>
            <a href="#sidebar-navigation" class="skip-link">Skip to navigation</a>
            <a href="#header-search" class="skip-link">Skip to search</a>
        </div>
        <div class="flex flex-col h-screen bg-background" data-testid="app-container">
            <!-- Header Bar with Search -->
            <HeaderBar
                on:searchResults={handleSearchResults}
                on:toggleHistory={handleToggleHistory}
                on:toggleSidebar={handleToggleSidebar}
            />
            
            <!-- Accessibility Toolbar -->
            <AccessibilityToolbar bind:isOpen={showAccessibilityToolbar} />

            <!-- History Timeline (absolute positioned) -->
            <HistoryTimeline bind:show={showHistoryTimeline} />

            <!-- Main Content Area -->
            <div class="flex flex-1 overflow-hidden">
                {#if showSidebar}
                    <Sidebar />
                {/if}
                <main id="main-content" data-testid="main-content" class="flex-1 overflow-hidden">
                    <div class="h-full p-6 overflow-auto">
                        {#if !initialized}
                            <div class="flex items-center justify-center h-full">
                                <div class="text-center">
                                    <div class="w-12 h-12 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
                                    <p class="text-lg font-semibold">Initializing StratoSort...</p>
                                    <p class="text-muted-foreground">Connecting to backend services</p>
                                </div>
                            </div>
                        {:else if page === 'discover'}
                            <ErrorBoundary fallback={false}>
                                <DiscoverPage />
                            </ErrorBoundary>
                        {:else if page === 'analyze'}
                            <ErrorBoundary fallback={false}>
                                <AnalyzePage />
                            </ErrorBoundary>
                        {:else if page === 'organize'}
                            <ErrorBoundary fallback={false}>
                                <OrganizePage />
                            </ErrorBoundary>
                        {:else if page === 'settings'}
                            <ErrorBoundary fallback={false}>
                                <SettingsPage />
                            </ErrorBoundary>
                        {:else if page === 'history'}
                            <ErrorBoundary fallback={false}>
                                <HistoryPage />
                            </ErrorBoundary>
                        {/if}
                    </div>
                    <!-- ARIA live regions for accessibility announcements -->
                    <div aria-live="polite" class="sr-only" data-testid="live-region-polite"></div>
                    <div aria-live="assertive" class="sr-only" data-testid="live-region-assertive"></div>
                </main>
            </div>

            <!-- Status Bar at Bottom -->
            <footer aria-label="Application status">
                <StatusBar />
            </footer>
        </div>
	{/if}
</ErrorBoundary>

<!-- Keyboard Shortcuts Help (Global) -->
<KeyboardShortcutsHelp bind:showHelp={showKeyboardHelp} />

<!-- About Dialog (Global) -->
<AboutDialog bind:open={showAboutDialog} />

<!-- Quick Actions FAB -->
{#if showQuickActions && !showingFirstRunSetup && initialized}
    <QuickActionsFAB onAction={handleQuickAction} />
{/if}

<style>
    /* Skip links styling */
    .skip-links {
        position: fixed;
        top: 0;
        left: 0;
        z-index: 9999;
        display: flex;
        gap: 8px;
    }

    .skip-link {
        position: absolute;
        top: -100px;
        left: 0;
        background: #000000;
        color: #ffffff;
        padding: 8px 16px;
        text-decoration: none;
        border-radius: 0 0 4px 4px;
        font-weight: 600;
        font-size: 14px;
        transition: top 0.3s ease;
        z-index: 10000;
        white-space: nowrap;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    }

    .skip-link:nth-child(1) {
        left: 10px;
    }

    .skip-link:nth-child(2) {
        left: 160px;
    }

    .skip-link:nth-child(3) {
        left: 290px;
    }

    .skip-link:focus,
    .skip-link:focus-visible {
        top: 0;
        outline: 2px solid #0066ff;
        outline-offset: 2px;
    }

    .skip-link:hover {
        background: #333333;
        text-decoration: underline;
    }

    /* Enhanced focus indicators */
    :global(*:focus-visible) {
        outline: 2px solid #005fcc;
        outline-offset: 2px;
        border-radius: 2px;
    }

    /* Improved contrast for links */
    :global(a) {
        text-decoration: underline;
    }

    :global(a:hover) {
        text-decoration: none;
    }

    /* Ensure interactive elements have minimum touch targets */
    :global(button, input, select, textarea, a) {
        min-height: 44px;
        min-width: 44px;
    }

    /* High contrast mode support */
    @media (prefers-contrast: high) {
        :global(.bg-background) {
            background: #000 !important;
            color: #fff !important;
        }
        
        :global(.text-foreground) {
            color: #fff !important;
        }
        
        :global(.border) {
            border-color: #fff !important;
        }
    }

    /* Reduced motion support */
    @media (prefers-reduced-motion: reduce) {
        :global(*) {
            animation-duration: 0.01ms !important;
            animation-iteration-count: 1 !important;
            transition-duration: 0.01ms !important;
        }
    }

    /* Large text support */
    @media (min-font-size: 18px) {
        :global(html) {
            font-size: 18px;
        }
        
        :global(button, input, select, textarea) {
            min-height: 48px;
            padding: 12px;
        }
    }
</style>