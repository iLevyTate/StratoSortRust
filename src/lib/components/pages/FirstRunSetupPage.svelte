<script lang="ts">
	import { Button, Card, CardContent, CardHeader, CardTitle, Input, Label, Switch, LoadingSpinner, DirectoryPicker, StatusIndicator } from '$lib/components/ui';
	import { ChevronRight, ChevronLeft, Check, Settings, Folder, Palette, Shield } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import type { FirstRunStatus, FirstRunSetup, OllamaStatus, AppSettings } from '$lib/types/backend';
	import {
		checkFirstRunStatus,
		completeFirstRunSetup,
		checkOllamaStatus,
		openDirectoryDialog,
		getAppSettings,
		saveAppSettings
	} from '$lib/api/tauri';
	import { addNotification, appSettings } from '$lib/stores';

	export let onSetupComplete: () => void;

	type SetupStep = 'welcome' | 'ai_setup' | 'directories' | 'preferences' | 'complete';

	let currentStep: SetupStep = 'welcome';
	let setupData: FirstRunSetup = {
		ai_provider: 'ollama',
		ollama_host: 'http://localhost:11434',
		ollama_model: 'llama3.2:3b',
		watch_paths: [],
		default_smart_folder_location: '',
		theme: 'auto',
		enable_telemetry: false,
		enable_crash_reports: false
	};

	let ollamaConnected = false;
	let availableModels: string[] = [];
	let isTestingConnection = false;
	let isCompletingSetup = false;

	const steps: { id: SetupStep; title: string; description: string; icon: any }[] = [
		{
			id: 'welcome',
			title: 'Welcome to StratoSort',
			description: 'Let\'s get you started with AI-powered file organization',
			icon: Check
		},
		{
			id: 'ai_setup',
			title: 'AI Configuration',
			description: 'Configure your AI provider for intelligent file analysis',
			icon: Settings
		},
		{
			id: 'directories',
			title: 'Directory Setup',
			description: 'Choose directories to watch and organize',
			icon: Folder
		},
		{
			id: 'preferences',
			title: 'Preferences',
			description: 'Customize your experience',
			icon: Palette
		}
	];

	const stepIndex = (step: SetupStep) => steps.findIndex(s => s.id === step);
	$: currentStepIndex = stepIndex(currentStep);
	$: isLastStep = currentStep === 'preferences';

	onMount(async () => {
		try {
			// Check if this is truly a first run
			const status = await checkFirstRunStatus();
			if (!status.is_first_run) {
				// Not a first run, complete setup immediately
				onSetupComplete();
				return;
			}

			// Test Ollama connection
			await testOllamaConnection();
		} catch (error) {
			console.error('Error initializing first run setup:', error);
		}
	});

	async function testOllamaConnection() {
		isTestingConnection = true;
		try {
			const status = await checkOllamaStatus();
			ollamaConnected = status.isRunning;

			if (status.isRunning) {
				// Ensure models is always an array, even if undefined or null
				availableModels = Array.isArray(status.models) ? status.models : [];
				if (availableModels.length > 0) {
					setupData.ollama_model = availableModels[0]; // Default to first available model
				}
			} else {
				// Reset models if not running
				availableModels = [];
			}
		} catch (error) {
			console.error('Failed to test Ollama connection:', error);
			ollamaConnected = false;
		} finally {
			isTestingConnection = false;
		}
	}

	async function addWatchPath() {
		try {
			const selectedPath = await openDirectoryDialog('Select Directory to Watch');
			if (selectedPath) {
				// Ensure watch_paths is always an array
				const currentPaths = Array.isArray(setupData.watch_paths) ? setupData.watch_paths : [];
				if (!currentPaths.includes(selectedPath)) {
					setupData.watch_paths = [...currentPaths, selectedPath];
				}
			}
		} catch (error) {
			addNotification('error', 'Directory Selection Failed', String(error));
		}
	}

	async function selectSmartFolderLocation() {
		try {
			const selectedPath = await openDirectoryDialog('Select Default Smart Folder Location');
			if (selectedPath) {
				setupData.default_smart_folder_location = selectedPath;
			}
		} catch (error) {
			addNotification('error', 'Directory Selection Failed', String(error));
		}
	}

	function removeWatchPath(path: string) {
		// Ensure watch_paths is always an array before filtering
		const currentPaths = Array.isArray(setupData.watch_paths) ? setupData.watch_paths : [];
		setupData.watch_paths = currentPaths.filter(p => p !== path);
	}

	function nextStep() {
		const currentIndex = stepIndex(currentStep);
		if (currentIndex < steps.length - 1) {
			currentStep = steps[currentIndex + 1].id;
		}
	}

	function previousStep() {
		const currentIndex = stepIndex(currentStep);
		if (currentIndex > 0) {
			currentStep = steps[currentIndex - 1].id;
		}
	}

	async function completeSetup() {
		isCompletingSetup = true;
		try {
			// Apply the setup data as app settings
			const currentSettings = await getAppSettings();
			const updatedSettings: AppSettings = {
				...currentSettings,
				ai_provider: setupData.ai_provider || 'ollama',
				ollama_host: setupData.ollama_host || 'http://localhost:11434',
				ollama_model: setupData.ollama_model || 'llama3.2:3b',
				watch_paths: setupData.watch_paths || [],
				default_smart_folder_location: setupData.default_smart_folder_location || '',
				theme: setupData.theme || 'auto',
				enable_telemetry: setupData.enable_telemetry || false,
				enable_crash_reports: setupData.enable_crash_reports || false
			};

			await saveAppSettings(updatedSettings);

			// Mark first run as complete - map to backend expected format
			const backendSetupData = {
				smart_folder_location: setupData.default_smart_folder_location,
				enable_watch_mode: setupData.watch_paths && setupData.watch_paths.length > 0,
				watch_directories: setupData.watch_paths || [],
				enable_notifications: true, // Default to true for notifications
				auto_analyze: false, // Default to false for auto-analyze
				ollama_host: setupData.ollama_host
			};

			await completeFirstRunSetup(backendSetupData);

			// Update local store
			appSettings.set(updatedSettings as AppSettings);

			addNotification('success', 'Setup Complete', 'StratoSort is now ready to use!');
			onSetupComplete();
		} catch (error) {
			console.error('Failed to complete setup:', error);
			addNotification('error', 'Setup Failed', String(error));
		} finally {
			isCompletingSetup = false;
		}
	}
</script>

<div class="flex items-center justify-center min-h-screen bg-background p-6">
	<div class="w-full max-w-4xl">
		<!-- Progress Indicator -->
		<div class="flex items-center justify-center mb-8">
			{#each steps as step, index}
				<div class="flex items-center">
					<div class="flex items-center justify-center w-10 h-10 rounded-full
						{index < currentStepIndex ? 'bg-primary text-primary-foreground' :
						 index === currentStepIndex ? 'bg-primary text-primary-foreground' :
						 'bg-muted text-muted-foreground'}
						transition-colors duration-200">
						{#if index < currentStepIndex}
							<Check class="w-5 h-5" />
						{:else}
							<svelte:component this={step.icon} class="w-5 h-5" />
						{/if}
					</div>
					{#if index < steps.length - 1}
						<div class="w-16 h-0.5 mx-2
							{index < currentStepIndex ? 'bg-primary' : 'bg-muted'}
							transition-colors duration-200"></div>
					{/if}
				</div>
			{/each}
		</div>

		<!-- Step Content -->
		<Card class="min-h-96">
			<CardHeader class="text-center">
				<CardTitle class="text-2xl">
					{steps.find(s => s.id === currentStep)?.title || 'Setup'}
				</CardTitle>
				<p class="text-muted-foreground">
					{steps.find(s => s.id === currentStep)?.description || ''}
				</p>
			</CardHeader>

			<CardContent class="space-y-6">
				{#if currentStep === 'welcome'}
					<div class="text-center space-y-4">
						<div class="w-20 h-20 bg-primary/10 rounded-full flex items-center justify-center mx-auto">
							<Settings class="w-10 h-10 text-primary" />
						</div>
						<div class="space-y-2">
							<h3 class="text-lg font-semibold">Intelligent File Organization</h3>
							<p class="text-muted-foreground max-w-md mx-auto">
								StratoSort uses AI to automatically analyze and organize your files.
								This setup wizard will help you configure the application for optimal performance.
							</p>
						</div>
						<div class="bg-muted/50 rounded-lg p-4 text-sm">
							<p><strong>What you'll set up:</strong></p>
							<ul class="mt-2 space-y-1 text-muted-foreground">
								<li>• AI provider configuration</li>
								<li>• Directory watching preferences</li>
								<li>• Theme and privacy settings</li>
							</ul>
						</div>
					</div>

				{:else if currentStep === 'ai_setup'}
					<div class="space-y-6">
						<div class="space-y-4">
							<div class="space-y-2">
								<Label for="ollama-host">Ollama Host URL</Label>
								<Input
									id="ollama-host"
									bind:value={setupData.ollama_host}
									placeholder="http://localhost:11434"
									class="font-mono"
								/>
							</div>

							<div class="flex gap-2">
								<Button
									variant="outline"
									on:click={testOllamaConnection}
									disabled={isTestingConnection}
									class="flex-shrink-0"
								>
									{#if isTestingConnection}
										<LoadingSpinner size="sm" inline className="mr-2" />
									{/if}
									Test Connection
								</Button>
								<StatusIndicator
									status={ollamaConnected ? 'connected' : 'disconnected'}
									label={ollamaConnected ? 'Connected' : 'Disconnected'}
									size="sm"
								/>
							</div>
						</div>

						{#if availableModels && availableModels.length > 0}
							<div class="space-y-2">
								<Label for="model-select">Available Models</Label>
								<select
									id="model-select"
									bind:value={setupData.ollama_model}
									class="w-full p-2 border rounded-md bg-background"
								>
									{#each availableModels as model}
										<option value={model}>{model}</option>
									{/each}
								</select>
							</div>
						{:else if !isTestingConnection}
							<div class="p-4 border rounded-lg bg-muted/50">
								<p class="text-sm text-muted-foreground">
									No models found. Make sure Ollama is running and has models installed.
									<br>You can continue setup and configure AI later.
								</p>
							</div>
						{/if}
					</div>

				{:else if currentStep === 'directories'}
					<div class="space-y-6">
						<div class="space-y-4">
							<div class="space-y-2">
								<Label>Watch Directories</Label>
								<p class="text-sm text-muted-foreground">
									Select directories that StratoSort should monitor for new files
								</p>
								<DirectoryPicker
									value=""
									placeholder="Select directory to watch..."
									dialogTitle="Select Directory to Watch"
									on:pathSelected={(e) => {
										const path = e.detail;
										if (path) {
											// Ensure watch_paths is always an array
											const currentPaths = Array.isArray(setupData.watch_paths) ? setupData.watch_paths : [];
											if (!currentPaths.includes(path)) {
												setupData.watch_paths = [...currentPaths, path];
											}
										}
									}}
								/>
							</div>

							{#if Array.isArray(setupData.watch_paths) && setupData.watch_paths.length > 0}
								<div class="space-y-2">
									{#each setupData.watch_paths as path}
										<div class="flex items-center justify-between p-3 border rounded-lg">
											<span class="text-sm font-mono truncate">{path}</span>
											<Button
												variant="ghost"
												size="sm"
												on:click={() => removeWatchPath(path)}
											>
												Remove
											</Button>
										</div>
									{/each}
								</div>
							{/if}
						</div>

						<div class="space-y-2">
							<Label>Default Smart Folder Location</Label>
							<p class="text-sm text-muted-foreground">
								Choose where StratoSort should create organized folder structures
							</p>
							<DirectoryPicker
								bind:value={setupData.default_smart_folder_location}
								placeholder="No location selected"
								dialogTitle="Select Default Smart Folder Location"
								readonly
							/>
						</div>
					</div>

				{:else if currentStep === 'preferences'}
					<div class="space-y-6">
						<div class="space-y-4">
							<div class="space-y-2">
								<Label for="theme-select">Theme</Label>
								<select
									id="theme-select"
									bind:value={setupData.theme}
									class="w-full p-2 border rounded-md bg-background"
								>
									<option value="auto">Auto (System)</option>
									<option value="light">Light</option>
									<option value="dark">Dark</option>
								</select>
							</div>
						</div>

						<div class="space-y-4">
							<div class="flex items-center justify-between">
								<div class="space-y-1">
									<Label>Enable Telemetry</Label>
									<p class="text-sm text-muted-foreground">
										Help improve StratoSort by sharing anonymous usage data
									</p>
								</div>
								<Switch bind:checked={setupData.enable_telemetry} />
							</div>

							<div class="flex items-center justify-between">
								<div class="space-y-1">
									<Label>Enable Crash Reports</Label>
									<p class="text-sm text-muted-foreground">
										Automatically send crash reports to help fix bugs
									</p>
								</div>
								<Switch bind:checked={setupData.enable_crash_reports} />
							</div>
						</div>

						<div class="p-4 border rounded-lg bg-muted/50">
							<div class="flex items-start gap-2">
								<Shield class="w-5 h-5 text-muted-foreground mt-0.5" />
								<div class="text-sm">
									<p class="font-medium">Privacy First</p>
									<p class="text-muted-foreground">
										All file analysis happens locally on your device.
										No file contents are ever sent to external servers.
									</p>
								</div>
							</div>
						</div>
					</div>
				{/if}
			</CardContent>
		</Card>

		<!-- Navigation -->
		<div class="flex items-center justify-between mt-6">
			<Button
				variant="outline"
				on:click={previousStep}
				disabled={currentStep === 'welcome'}
			>
				<ChevronLeft class="w-4 h-4 mr-2" />
				Previous
			</Button>

			{#if isLastStep}
				<Button
					on:click={completeSetup}
					disabled={isCompletingSetup}
					class="min-w-24"
				>
					{#if isCompletingSetup}
						<LoadingSpinner size="sm" inline className="mr-2" />
					{/if}
					Complete Setup
				</Button>
			{:else}
				<Button on:click={nextStep}>
					Next
					<ChevronRight class="w-4 h-4 ml-2" />
				</Button>
			{/if}
		</div>
	</div>
</div>