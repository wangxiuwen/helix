import { useDevOpsStore } from '../../stores/useDevOpsStore';
import { invoke } from '@tauri-apps/api/core';

export class LLMProvider {
    async chat(messages: any[], tools: any[] = []): Promise<any> {
        const { aiProviders } = useDevOpsStore.getState();
        const activeProvider = aiProviders.find(p => p.enabled);
        if (!activeProvider) throw new Error('No active AI Provider found');

        const baseUrl = (activeProvider.baseUrl || '').replace(/\/+$/, '');
        const apiKey = activeProvider.apiKey;
        const model = activeProvider.defaultModel || activeProvider.models?.[0] || 'gpt-4o';

        const body: any = {
            model,
            messages,
            temperature: 0.7,
            max_tokens: 2048,
        };

        if (tools.length > 0) {
            body.tools = tools;
            body.tool_choice = 'auto';
        }

        const headers: any = { 'Content-Type': 'application/json' };
        if (apiKey) {
            headers['Authorization'] = `Bearer ${apiKey}`;
        }

        try {
            const data: any = await invoke('team_chat_fetch', {
                url: `${baseUrl}/chat/completions`,
                method: 'POST',
                headers,
                body
            });
            return data.choices[0].message;
        } catch (err: any) {
            throw new Error(`LLM API Error: ${err}`);
        }
    }
}
