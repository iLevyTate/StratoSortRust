import App from './App.svelte';

const isDev = import.meta.env.DEV;

// Add global error handler for debugging
window.addEventListener('error', (event) => {
	console.error('Global error:', event.error);
	console.error('Stack:', event.error?.stack);
});

window.addEventListener('unhandledrejection', (event) => {
	console.error('Unhandled promise rejection:', event.reason);
});

// Check if Tauri is available
if (typeof window !== 'undefined') {
	if (isDev) console.log('Window.__TAURI__ available:', '__TAURI__' in window);
	if ('__TAURI__' in window) {
		if (isDev) console.log('Tauri IPC available');
	} else {
		if (isDev) console.warn('Tauri IPC not available - running in web mode');
	}
}

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