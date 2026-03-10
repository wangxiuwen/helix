import { useDevOpsStore } from '../../stores/useDevOpsStore';
import { invoke } from '@tauri-apps/api/core';

export class LLMProvider {
    async chat(messages: any[], tools: any[] = []): Promise<any> {
        const { aiProviders } = useDevOpsStore.getState();
        const activeProvider = aiProviders.find(p => p.enabled);
        if (!activeProvider) throw new Error('No active AI Provider found');

        let baseUrl = (activeProvider.baseUrl || '').replace(/\/+$/, '');
        const isCodingPlan = baseUrl.includes('coding.dashscope.aliyuncs.com');
        const apiKey = activeProvider.apiKey;
        const model = activeProvider.defaultModel || activeProvider.models?.[0] || 'gpt-4o';

        // CodingPlan endpoint only supports agents-sdk protocol, not raw HTTP chat/completions.
        // Route through the backend agent_chat which uses agents-sdk internally.
        if (isCodingPlan) {
            try {
                // Build a combined prompt from the messages
                const systemMsg = messages.find((m: any) => m.role === 'system');
                const nonSystemMsgs = messages.filter((m: any) => m.role !== 'system');
                const lastUserMsg = nonSystemMsgs[nonSystemMsgs.length - 1];
                const content = lastUserMsg?.content || '';

                // Use a unique account ID for team chat routing
                const accountId = `team:butler:${Date.now()}`;

                // Set the config with current provider before calling
                await invoke('ai_set_config', {
                    provider: activeProvider.type,
                    baseUrl: activeProvider.baseUrl || '',
                    apiKey: activeProvider.apiKey || '',
                    model,
                    systemPrompt: systemMsg?.content || '',
                });

                const result = await invoke<{ content: string }>('agent_chat', {
                    accountId,
                    content,
                    images: [],
                    workspace: null,
                });

                return { role: 'assistant', content: result.content || '' };
            } catch (err: any) {
                throw new Error(`LLM API Error: ${err}`);
            }
        }

        // Standard path: direct HTTP via team_chat_fetch
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
