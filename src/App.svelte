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
	import FirstRunSetupPage from '$lib/components/pages/FirstRunSetupPage.svelte';
	import HeaderBar from '$lib/components/HeaderBar.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import HistoryTimeline from '$lib/components/HistoryTimeline.svelte';
	import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';
	import KeyboardShortcutsHelp from '$lib/components/KeyboardShortcutsHelp.svelte';
	import AccessibilityManager from '$lib/components/AccessibilityManager.svelte';
	import { AccessibilityToolbar } from '$lib/components/ui';
	import { getAppSettings, checkOllamaStatus, checkFirstRunStatus, getSystemInfo, frontendReady } from '$lib/api/tauri';
	import { keyboardShortcuts } from '$lib/utils/keyboard-shortcuts';
	import { operationInProgress } from '$lib/stores';
	import { toast } from '$lib/stores/notifications';
	import { AppInitializer, type InitializationStep, waitForCondition } from '$lib/utils/initialization';
	import { log } from '$lib/utils/logger';

	let page: string = 'discover';
	let initialized = false;
	let initializationError: Error | null = null;
	let isFirstRun = false;
	let showingFirstRunSetup = false;
	let showKeyboardHelp = false;
	let showHistoryTimeline = false;
	let showSidebar = true;
	let showAccessibilityToolbar = false;

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
			// Handle showing about dialog
			console.log('About dialog requested');
			// You can implement an about dialog component here
		};
		window.addEventListener('show-about-dialog', handleShowAboutDialog);

		// Setup accessibility features
		setupAccessibilityShortcuts();

		// Async initialization
		const initAsync = async () => {
			try {
				const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
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
					log.info('StratoSort app initialized successfully', undefined, 'App');
				}
				// If it is first run, we'll initialize after setup is complete
				// Don't set initialized = true yet for first run scenarios

			} catch (error) {
				log.error('Failed to initialize app', error, 'App');
				initializationError = error as Error;
				if (!isFirstRun) {
					// Don't set initialized = true on error, keep app in loading/error state
					// This prevents showing broken UI when initialization fails
					toast.error('Failed to initialize application. Some features may not work correctly.');
				}
				// For both first-run and normal errors, stay in loading state to allow retry
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
		// Handle search results - navigate to discover page and show results
		currentPage.set('discover');
		// The results will be handled by the DiscoverPage component
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
    {:else}
        <!-- Main Application -->
        <!-- Skip links for keyboard navigation -->
        <div class="skip-links">
            <a href="#main-content" class="skip-link">Skip to main content</a>
            <a href="#navigation" class="skip-link">Skip to navigation</a>
            <a href="#search" class="skip-link">Skip to search</a>
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

            <!-- Status Bar at Bottom -->
            <footer aria-label="Application status">
                <StatusBar />
            </footer>
        </div>
	{/if}
</ErrorBoundary>

<!-- Keyboard Shortcuts Help (Global) -->
<KeyboardShortcutsHelp bind:showHelp={showKeyboardHelp} />

<style>
    /* Skip links styling */
    :global(.skip-links) {
        position: fixed;
        top: 0;
        left: 0;
        z-index: 1000;
    }

    :global(.skip-link) {
        position: absolute;
        top: -40px;
        left: 6px;
        background: #000;
        color: #fff;
        padding: 8px 12px;
        text-decoration: none;
        border-radius: 0 0 4px 4px;
        font-weight: bold;
        transition: top 0.2s ease;
        z-index: 1001;
    }

    :global(.skip-link:focus) {
        top: 0;
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