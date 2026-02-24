// Environment detection
const isTauri = typeof window !== 'undefined' && (!!(window as any).__TAURI_INTERNALS__ || !!(window as any).__TAURI__);

// Simplified command-to-API mapping for helix
const COMMAND_MAPPING: Record<string, { url: string; method: 'GET' | 'POST' | 'DELETE' | 'PATCH' }> = {
  'load_config': { url: '/api/config', method: 'GET' },
  'save_config': { url: '/api/config', method: 'POST' },
};

export async function request<T>(cmd: string, args?: any): Promise<T> {
  // Tauri: use invoke
  if (isTauri) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<T>(cmd, args);
    } catch (error) {
      console.error(`Tauri Invoke Error [${cmd}]:`, error);
      throw error;
    }
  }

  // Web: map to HTTP API
  const mapping = COMMAND_MAPPING[cmd];
  if (!mapping) {
    throw new Error(`Command [${cmd}] not supported in Web mode.`);
  }

  const url = mapping.url;
  const options: RequestInit = {
    method: mapping.method,
    headers: { 'Content-Type': 'application/json' },
  };

  if (mapping.method === 'POST' && args) {
    options.body = JSON.stringify(args);
  }

  const response = await fetch(url, options);
  if (!response.ok) {
    const errorData = await response.json().catch(() => ({}));
    throw errorData.error || `HTTP Error ${response.status}`;
  }

  if (response.status === 204) return null as unknown as T;

  const text = await response.text();
  if (!text) return null as unknown as T;

  return JSON.parse(text) as T;
}
