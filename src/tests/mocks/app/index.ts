// Mock for SvelteKit $app modules (if needed)
export const page = {
	subscribe: (fn: Function) => {
		fn({ url: new URL('http://localhost'), params: {} });
		return () => {};
	}
};

export const navigating = {
	subscribe: (fn: Function) => {
		fn(null);
		return () => {};
	}
};

export const goto = async (url: string) => {
	console.log('Mock navigation to:', url);
	return Promise.resolve();
};

export const invalidate = async (url: string) => {
	console.log('Mock invalidation of:', url);
	return Promise.resolve();
};

export const invalidateAll = async () => {
	console.log('Mock invalidation of all');
	return Promise.resolve();
};