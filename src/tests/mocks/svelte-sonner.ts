/**
 * Mock for svelte-sonner to prevent browser environment errors in tests
 */

import { vi } from 'vitest';

// Mock the Toaster component as a function (Svelte component constructor)
export function Toaster(options?: any) {
  return {
    $$: {
      fragment: {
        c: () => {},
        m: () => {},
        p: () => {},
        d: () => {}
      },
      ctx: [],
      props: options?.props || {},
      update: () => {},
      not_equal: () => false,
      bound: {},
      on_mount: [],
      on_destroy: [],
      before_update: [],
      after_update: [],
      callbacks: {}
    },
    $set: () => {},
    $destroy: () => {},
    $on: () => () => {},
    $capture_state: () => ({}),
    $inject_state: () => {}
  };
}

// Mock the toast functions
export const toast = {
  success: vi.fn((message: string) => {
    console.log('Mock toast success:', message);
  }),
  error: vi.fn((message: string) => {
    console.log('Mock toast error:', message);
  }),
  info: vi.fn((message: string) => {
    console.log('Mock toast info:', message);
  }),
  warning: vi.fn((message: string) => {
    console.log('Mock toast warning:', message);
  }),
  loading: vi.fn((message: string) => {
    console.log('Mock toast loading:', message);
    return '1';
  }),
  promise: vi.fn((promise: Promise<any>, options: any) => {
    console.log('Mock toast promise:', options);
    return promise;
  }),
  dismiss: vi.fn((id?: string) => {
    console.log('Mock toast dismiss:', id);
  }),
  custom: vi.fn((_component: any, options?: any) => {
    console.log('Mock toast custom:', options);
    return '1';
  }),
  message: vi.fn((message: string) => {
    console.log('Mock toast message:', message);
  })
};

// Export default toast for compatibility
export default toast;