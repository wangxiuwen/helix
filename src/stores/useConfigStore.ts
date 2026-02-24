import { create } from 'zustand';
import { AppConfig } from '../types/config';

interface ConfigState {
    config: AppConfig | null;
    loading: boolean;
    error: string | null;
    loadConfig: () => Promise<void>;
    saveConfig: (config: AppConfig, silent?: boolean) => Promise<void>;
    updateTheme: (theme: string) => Promise<void>;
    updateLanguage: (language: string) => Promise<void>;
}

const CONFIG_STORAGE_KEY = 'devhelix_config';

export const useConfigStore = create<ConfigState>((set, get) => ({
    config: null,
    loading: false,
    error: null,

    loadConfig: async () => {
        set({ loading: true, error: null });
        try {
            // Try Tauri invoke first, fallback to localStorage
            let config: AppConfig;
            try {
                const { isTauri } = await import('../utils/env');
                if (isTauri()) {
                    const { invoke } = await import('@tauri-apps/api/core');
                    config = await invoke('load_config');
                } else {
                    throw new Error('not tauri');
                }
            } catch {
                // Fallback: load from localStorage
                const saved = localStorage.getItem(CONFIG_STORAGE_KEY);
                config = saved ? JSON.parse(saved) : { language: 'zh', theme: 'dark' };
            }
            set({ config, loading: false });
        } catch (error) {
            set({ error: String(error), loading: false });
        }
    },

    saveConfig: async (config: AppConfig, silent: boolean = false) => {
        if (!silent) set({ loading: true, error: null });
        try {
            // Try Tauri invoke first, fallback to localStorage
            try {
                const { isTauri } = await import('../utils/env');
                if (isTauri()) {
                    const { invoke } = await import('@tauri-apps/api/core');
                    await invoke('save_config', { config });
                    await invoke('set_window_theme', { theme: config.theme }).catch(() => { });
                } else {
                    throw new Error('not tauri');
                }
            } catch {
                localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(config));
            }
            set({ config, loading: false });
        } catch (error) {
            set({ error: String(error), loading: false });
            throw error;
        }
    },

    updateTheme: async (theme: string) => {
        const { config } = get();
        if (!config || config.theme === theme) return;
        await get().saveConfig({ ...config, theme }, true);
    },

    updateLanguage: async (language: string) => {
        const { config } = get();
        if (!config || config.language === language) return;
        await get().saveConfig({ ...config, language }, true);
    },
}));
