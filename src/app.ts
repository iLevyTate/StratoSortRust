import App from './App.svelte';
import { initializeEventListeners, cleanupEventListeners } from '$lib/api/events';

const isDev = import.meta.env.DEV;

// Add global error handler for debugging
window.addEventListener('error', (event) => {
	console.error('Global error:', event.error);
	console.error('Stack:', event.error?.stack);
});

window.addEventListener('unhandledrejection', (event) => {
	console.error('Unhandled promise rejection:', event.reason);
});

// Check if Tauri is available and initialize event listeners
if (typeof window !== 'undefined') {
	if (isDev) console.log('Window.__TAURI__ available:', '__TAURI__' in window);
	if ('__TAURI__' in window) {
		if (isDev) console.log('Tauri IPC available');
		// Initialize backend event listeners
		initializeEventListeners()
			.then(() => {
				if (isDev) console.log('Backend event listeners initialized');
			})
			.catch((error) => {
				console.error('Failed to initialize event listeners:', error);
			});
	} else {
		if (isDev) console.warn('Tauri IPC not available - running in web mode');
	}
}

// Clean up event listeners on window unload
window.addEventListener('beforeunload', () => {
	if ('__TAURI__' in window) {
		cleanupEventListeners();
	}
});

// Mount the app with error handling
let app: App;

try {
	const targetElement = document.getElementById('app');
	if (!targetElement) {
		throw new Error('Could not find app mount point');
	}

	app = new App({
		target: targetElement
	});

	if (isDev) console.log('App mounted successfully');
} catch (error) {
	console.error('Failed to mount app:', error);
	// Display error in DOM if mount fails
	document.body.innerHTML = `
		<div style="padding: 20px; color: red;">
			<h1>Failed to start StratoSort</h1>
			<pre>${error}</pre>
		</div>
	`;
	throw error;
}

export default app;