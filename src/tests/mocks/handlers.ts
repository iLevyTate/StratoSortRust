import { http, HttpResponse } from 'msw';
import { mockFileInfo, mockFileAnalysis, mockOllamaStatus, mockAppSettings, mockSmartFolder } from './tauri-api';

// MSW handlers for HTTP requests (if any)
export const handlers = [
	// Ollama API endpoints
	http.get('http://localhost:11434/api/tags', () => {
		return HttpResponse.json({
			models: [
				{ name: 'llama3.2:3b', size: 2000000000 },
				{ name: 'llava:7b', size: 4000000000 }
			]
		});
	}),

	http.get('http://localhost:11434/api/version', () => {
		return HttpResponse.json({ version: '0.1.0' });
	}),

	http.post('http://localhost:11434/api/generate', async ({ request }) => {
		const body = await request.json() as { model?: string; [key: string]: any } | null;
		return HttpResponse.json({
			model: body?.model || 'llama3.2:3b',
			response: 'Mocked AI response for testing',
			done: true
		});
	}),

	// Mock external API calls if needed
	http.get('https://api.example.com/*', () => {
		return HttpResponse.json({ success: true });
	})
];

// Helper to add custom handlers for specific tests
export const addMockHandler = (handler: any) => {
	handlers.push(handler);
};

// Helper to create error responses
export const createErrorResponse = (status: number, message: string) => {
	return HttpResponse.json({ error: message }, { status });
};