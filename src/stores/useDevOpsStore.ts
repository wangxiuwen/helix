import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { getToolsForAI, executeTool, findTool, setSkillEnabled, addCustomSkill as addSkillToRegistry, removeCustomSkill as removeSkillFromRegistry, syncSkillStates, loadCustomSkills, buildSkillsPrompt, ToolLoopDetector, loadAllAgentSkills, buildAgentSkillsPrompt, type OpsSkill, type ToolParameter, type ToolEvent, type AgentSkill } from '../services/opsTools';
import { invoke } from '@tauri-apps/api/core';

function syncAIProviderToBackend(providers: AIProvider[]) {
    if (typeof window === 'undefined' || !(window as any).__TAURI__) return;
    const active = providers.find(p => p.enabled);
    if (!active) return;

    console.log('[syncAIProvider] Syncing provider to backend:', active.name, active.baseUrl);
    invoke('ai_set_config', {
        provider: active.type,
        baseUrl: active.baseUrl || '',
        apiKey: active.apiKey || '',
        model: active.defaultModel || active.models?.[0] || 'qwen-plus',
        autoReply: true,
    }).then(() => {
        console.log('[syncAIProvider] Success');
    }).catch(err => {
        console.error('[syncAIProvider] Failed:', err);
    });
}

// ========== Types ==========

export interface ServerNode {
    id: string;
    name: string;
    host: string;
    port?: number;
    status: 'online' | 'offline' | 'warning';
    os?: string;
    cpu?: number;
    memory?: number;
    disk?: number;
    uptime?: number;
    lastCheck?: string;
    tags?: string[];
}

export interface AIProvider {
    id: string;
    name: string;
    type: 'openai' | 'anthropic' | 'ollama' | 'custom';
    apiKey?: string;
    baseUrl?: string;
    models: string[];
    enabled: boolean;
    defaultModel?: string;
}

export interface ChatMessage {
    id: string;
    role: 'user' | 'assistant' | 'system' | 'tool';
    content: string;
    timestamp: string;
    model?: string;
    toolCalls?: Array<{ name: string; args: Record<string, any>; result?: string; status?: 'pending' | 'done' | 'error' }>;
    pendingConfirm?: { toolName: string; args: Record<string, any>; description: string };
}

export interface ChatSession {
    id: string;
    title: string;
    messages: ChatMessage[];
    model?: string;
    provider?: string;
    createdAt: string;
    updatedAt: string;
}

export interface AutoTask {
    id: string;
    name: string;
    description?: string;
    type: 'cron' | 'webhook' | 'manual';
    schedule?: string;
    script?: string;
    status: 'active' | 'paused' | 'error';
    lastRun?: string;
    lastResult?: 'success' | 'failure';
    notifyChannel?: 'feishu' | 'dingtalk' | null;
    tags?: string[];
    history?: Array<{ time: string; result: 'success' | 'failure'; output?: string }>;
}

export interface AlertRule {
    id: string;
    name: string;
    condition: string;
    severity: 'info' | 'warning' | 'critical';
    enabled: boolean;
    targetServer?: string;
    lastTriggered?: string;
}

export interface LogEntry {
    id: string;
    timestamp: string;
    level: 'info' | 'warn' | 'error' | 'debug';
    source: string;
    message: string;
    server?: string;
}

export interface CloudConfig {
    aliyun: {
        accessKeyId: string;
        accessKeySecret: string;
        region: string;
    };
    k8s: {
        kubeconfigPath: string;
        context: string;
        namespace: string;
    };
}

export interface NotificationChannel {
    id: string;
    name: string;
    type: 'feishu' | 'dingtalk';
    webhookUrl: string;
    enabled: boolean;
}

export interface DevOpsConfig {
    theme: 'light' | 'dark';
    language: string;
    refreshInterval: number;
}

// ========== Skill Types (serializable) ==========

export interface CustomSkillDef {
    id: string;
    name: string;
    description: string;
    icon: string;
    category: OpsSkill['category'];
    tools: Array<{
        name: string;
        description: string;
        dangerous?: boolean;
        parameters: Record<string, ToolParameter>;
        script: string;
    }>;
}

// ========== Store ==========

interface helixState {
    servers: ServerNode[];
    aiProviders: AIProvider[];
    chatSessions: ChatSession[];
    activeChatId: string | null;
    tasks: AutoTask[];
    alerts: AlertRule[];
    logs: LogEntry[];
    config: DevOpsConfig;
    cloudConfig: CloudConfig;
    notificationChannels: NotificationChannel[];
    skillStates: Record<string, boolean>;
    customSkills: CustomSkillDef[];
    agentSkills: AgentSkill[];
    loading: Record<string, boolean>;

    // Server
    addServer: (server: Omit<ServerNode, 'id' | 'status'>) => void;
    removeServer: (id: string) => void;
    updateServer: (id: string, updates: Partial<ServerNode>) => void;
    checkServerStatus: (id: string) => Promise<void>;
    checkAllServers: () => Promise<void>;

    // AI Provider
    addAIProvider: (provider: Omit<AIProvider, 'id'>) => void;
    removeAIProvider: (id: string) => void;
    updateAIProvider: (id: string, updates: Partial<AIProvider>) => void;

    // Chat
    createChatSession: (title?: string) => string;
    deleteChatSession: (id: string) => void;
    setActiveChatId: (id: string | null) => void;
    sendMessage: (sessionId: string, content: string) => Promise<void>;
    confirmToolExecution: (sessionId: string, messageId: string) => Promise<void>;

    // Task
    addTask: (task: Omit<AutoTask, 'id'>) => void;
    removeTask: (id: string) => void;
    updateTask: (id: string, updates: Partial<AutoTask>) => void;
    runTask: (id: string) => Promise<void>;

    // Alert
    addAlert: (alert: Omit<AlertRule, 'id'>) => void;
    removeAlert: (id: string) => void;
    toggleAlert: (id: string) => void;

    // Log
    addLog: (log: Omit<LogEntry, 'id'>) => void;
    clearLogs: () => void;

    // Config
    updateConfig: (config: Partial<DevOpsConfig>) => void;
    updateCloudConfig: (config: Partial<CloudConfig>) => void;

    // Notification
    addNotificationChannel: (channel: Omit<NotificationChannel, 'id'>) => void;
    removeNotificationChannel: (id: string) => void;
    updateNotificationChannel: (id: string, updates: Partial<NotificationChannel>) => void;

    // Skills
    toggleSkill: (skillId: string, enabled: boolean) => void;
    addCustomSkill: (skill: CustomSkillDef) => void;
    removeCustomSkill: (skillId: string) => void;
    initSkills: () => void;

    // Agent Skills (SKILL.md)
    loadAgentSkills: () => void;
    toggleAgentSkill: (skillName: string, enabled: boolean) => void;
}

function generateId() {
    return Date.now().toString(36) + Math.random().toString(36).slice(2, 9);
}

// ========== AI call with function calling ==========

async function callAI(
    provider: AIProvider,
    messages: Array<{ role: string; content: string }>,
    tools?: any[]
): Promise<{ content: string; toolCalls?: Array<{ name: string; arguments: string }> }> {
    const model = provider.defaultModel || provider.models[0];

    if (provider.type === 'openai' || provider.type === 'custom') {
        const body: any = { model, messages, stream: false };
        if (tools && tools.length > 0) body.tools = tools;
        const res = await fetch(`${provider.baseUrl}/chat/completions`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${provider.apiKey}` },
            body: JSON.stringify(body),
        });
        const data = await res.json();
        const choice = data.choices?.[0];
        const msg = choice?.message;
        return {
            content: msg?.content || '',
            toolCalls: msg?.tool_calls?.map((tc: any) => ({
                name: tc.function.name,
                arguments: tc.function.arguments,
            })),
        };
    } else if (provider.type === 'anthropic') {
        const body: any = {
            model,
            max_tokens: 4096,
            system: messages[0]?.role === 'system' ? messages[0].content : undefined,
            messages: messages.filter(m => m.role !== 'system').map(m => ({
                role: m.role === 'system' ? 'user' : m.role,
                content: m.content,
            })),
        };
        if (tools && tools.length > 0) {
            body.tools = tools.map((t: any) => ({
                name: t.function.name,
                description: t.function.description,
                input_schema: t.function.parameters,
            }));
        }
        const res = await fetch(`${provider.baseUrl}/v1/messages`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-api-key': provider.apiKey!,
                'anthropic-version': '2023-06-01',
                'anthropic-dangerous-direct-browser-access': 'true',
            },
            body: JSON.stringify(body),
        });
        const data = await res.json();
        const textBlock = data.content?.find((b: any) => b.type === 'text');
        const toolUse = data.content?.filter((b: any) => b.type === 'tool_use');
        return {
            content: textBlock?.text || '',
            toolCalls: toolUse?.length ? toolUse.map((tu: any) => ({
                name: tu.name,
                arguments: JSON.stringify(tu.input),
            })) : undefined,
        };
    } else if (provider.type === 'ollama') {
        const res = await fetch(`${provider.baseUrl}/api/chat`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ model, messages, stream: false }),
        });
        const data = await res.json();
        return { content: data.message?.content || '(无响应)' };
    }
    return { content: '(不支持的提供商类型)' };
}

// ========== Store Implementation ==========

export const useDevOpsStore = create<helixState>()(
    persist(
        (set, get) => ({
            servers: [],
            aiProviders: [],
            chatSessions: [],
            activeChatId: null,
            tasks: [],
            alerts: [],
            logs: [],
            config: { theme: 'light', language: 'zh', refreshInterval: 30 },
            cloudConfig: {
                aliyun: { accessKeyId: '', accessKeySecret: '', region: 'cn-beijing' },
                k8s: { kubeconfigPath: '~/.kube/config', context: '', namespace: 'default' },
            },
            notificationChannels: [],
            skillStates: {},
            customSkills: [],
            agentSkills: [],
            loading: {},

            // ===== Server =====
            addServer: (server) => {
                const newServer: ServerNode = { ...server, id: generateId(), status: 'offline' };
                set((s) => ({ servers: [...s.servers, newServer] }));
                get().checkServerStatus(newServer.id);
            },
            removeServer: (id) => set((s) => ({ servers: s.servers.filter((sv) => sv.id !== id) })),
            updateServer: (id, updates) =>
                set((s) => ({ servers: s.servers.map((sv) => (sv.id === id ? { ...sv, ...updates } : sv)) })),

            checkServerStatus: async (id) => {
                const server = get().servers.find((s) => s.id === id);
                if (!server) return;
                set((s) => ({ loading: { ...s.loading, [`server-${id}`]: true } }));
                try {
                    const url = `http://${server.host}${server.port ? `:${server.port}` : ''}`;
                    const controller = new AbortController();
                    const timeout = setTimeout(() => controller.abort(), 5000);
                    try {
                        await fetch(url, { signal: controller.signal, mode: 'no-cors' });
                        clearTimeout(timeout);
                        get().updateServer(id, { status: 'online', lastCheck: new Date().toISOString() });
                    } catch {
                        clearTimeout(timeout);
                        get().updateServer(id, { status: 'offline', lastCheck: new Date().toISOString() });
                    }
                } finally {
                    set((s) => ({ loading: { ...s.loading, [`server-${id}`]: false } }));
                }
            },

            checkAllServers: async () => {
                await Promise.allSettled(get().servers.map((s) => get().checkServerStatus(s.id)));
            },

            // ===== AI Providers =====
            addAIProvider: (provider) =>
                set((s) => {
                    const newProviders = [...s.aiProviders, { ...provider, id: generateId() }];
                    syncAIProviderToBackend(newProviders);
                    return { aiProviders: newProviders };
                }),
            removeAIProvider: (id) =>
                set((s) => {
                    const newProviders = s.aiProviders.filter((p) => p.id !== id);
                    syncAIProviderToBackend(newProviders);
                    return { aiProviders: newProviders };
                }),
            updateAIProvider: (id, updates) =>
                set((s) => {
                    const newProviders = s.aiProviders.map((p) => {
                        if (p.id === id) return { ...p, ...updates };
                        if (updates.enabled === true) return { ...p, enabled: false };
                        return p;
                    });
                    syncAIProviderToBackend(newProviders);
                    return { aiProviders: newProviders };
                }),

            // ===== Chat with Function Calling =====
            createChatSession: (title) => {
                const id = generateId();
                const session: ChatSession = {
                    id, title: title || `新对话 ${new Date().toLocaleString()}`,
                    messages: [], createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(),
                };
                set((s) => ({ chatSessions: [session, ...s.chatSessions], activeChatId: id }));
                return id;
            },
            deleteChatSession: (id) =>
                set((s) => ({
                    chatSessions: s.chatSessions.filter((cs) => cs.id !== id),
                    activeChatId: s.activeChatId === id ? null : s.activeChatId,
                })),
            setActiveChatId: (id) => set({ activeChatId: id }),

            sendMessage: async (sessionId, content) => {
                const MAX_TOOL_ROUNDS = 15;
                const MAX_RETRIES_PER_TOOL = 1;
                const state = get();
                const session = state.chatSessions.find((s) => s.id === sessionId);
                if (!session) return;

                const provider = state.aiProviders.find((p) => p.enabled && p.apiKey);
                if (!provider) {
                    const errorMsg: ChatMessage = {
                        id: generateId(), role: 'assistant',
                        content: '⚠️ 请先在设置中配置并启用一个 AI 提供商，并填入 API Key。',
                        timestamp: new Date().toISOString(),
                    };
                    set((s) => ({
                        chatSessions: s.chatSessions.map((cs) =>
                            cs.id === sessionId ? {
                                ...cs,
                                messages: [...cs.messages, { id: generateId(), role: 'user' as const, content, timestamp: new Date().toISOString() }, errorMsg],
                                updatedAt: new Date().toISOString(),
                            } : cs
                        ),
                    }));
                    return;
                }

                // Add user message
                set((s) => ({
                    chatSessions: s.chatSessions.map((cs) =>
                        cs.id === sessionId ? {
                            ...cs,
                            messages: [...cs.messages, { id: generateId(), role: 'user' as const, content, timestamp: new Date().toISOString() }],
                            updatedAt: new Date().toISOString(),
                        } : cs
                    ),
                    loading: { ...s.loading, chat: true },
                }));

                try {
                    // Build system prompt — task completion oriented
                    const toolsPrompt = buildSkillsPrompt();
                    const agentSkillsPrompt = buildAgentSkillsPrompt(get().agentSkills);

                    // Retrieve relevant memory context
                    let memoryContext = '';
                    try {
                        const { invoke } = await import('@tauri-apps/api/core');
                        const memories = await invoke<Array<{ content: string; score: number }>>('memory_search', { query: content, limit: 3 });
                        if (memories && memories.length > 0) {
                            memoryContext = '\n\n## 相关记忆\n' + memories.map(m => `- ${m.content}`).join('\n');
                        }
                    } catch { /* memory not available, skip */ }

                    const systemPrompt = `你是 Helix，一个能力强大的通用 AI 智能体。

## 核心原则

1. **完整交付**: 你必须把用户交给你的任务彻底做完，不要半途而废。
2. **自我验证**: 执行操作后，验证结果是否符合预期。如果不对，修正并重试。
3. **主动解决**: 遇到错误不要直接报错给用户。先尝试替代方案，实在不行再说明。

## ⚠️ 安全规则（必须严格遵守）

### 只读操作 → 直接执行，无需确认：
- 查询、列表、搜索、状态检查
- 文件读取、日志查看
- Dry-run 和模拟执行
- 任何不改变系统状态的操作

### 写入操作 → 必须先说明方案，等待用户确认后执行：
- 创建、修改、删除文件或数据
- 重启服务、部署应用
- 修改配置、更新密钥
- 网络请求（POST/PUT/DELETE）
- 任何改变系统状态的操作

**执行流程**：
1. 先用只读操作收集信息，分析现状
2. 制定操作方案，列出即将执行的写入操作
3. 明确告知用户将要做什么，等待确认
4. 用户确认后再执行写入操作
5. 执行后验证结果

## 工作流程

- 分析用户需求 → 规划步骤 → 逐步执行 → 验证结果 → 总结交付
- 每次工具调用后评估：任务完成了吗？没完成就继续。
- 最终回复以 ✅ 开头总结完成的工作，或以 ❌ 开头说明无法完成的原因。

## 回复格式

- 用中文回复，简洁实用
- 查询结果用表格或列表展示
- 多步操作时说明当前在第几步
${memoryContext}
${toolsPrompt}
${agentSkillsPrompt}`;

                    const currentSession = get().chatSessions.find((s) => s.id === sessionId)!;
                    const messages: Array<{ role: string; content: string }> = [
                        { role: 'system', content: systemPrompt },
                        ...currentSession.messages.map((m) => ({ role: m.role === 'tool' ? 'user' : m.role, content: m.content })),
                    ];

                    const tools = getToolsForAI();
                    const loopDetector = new ToolLoopDetector();
                    const allToolEvents: ToolEvent[] = [];
                    const allToolCalls: Array<{ name: string; args: Record<string, any>; result: string; status: 'done' | 'error' }> = [];

                    // === Agentic Loop — keep going until task is complete ===
                    for (let round = 0; round < MAX_TOOL_ROUNDS; round++) {
                        const result = await callAI(provider, messages, tools);

                        // No tool calls → AI decided task is done (or gave final answer)
                        if (!result.toolCalls || result.toolCalls.length === 0) {
                            const assistantMsg: ChatMessage = {
                                id: generateId(), role: 'assistant',
                                content: result.content || '(无响应)',
                                timestamp: new Date().toISOString(),
                                model: provider.defaultModel,
                                toolCalls: allToolCalls.length > 0 ? allToolCalls : undefined,
                            };
                            set((s) => ({
                                chatSessions: s.chatSessions.map((cs) =>
                                    cs.id === sessionId ? {
                                        ...cs, messages: [...cs.messages, assistantMsg],
                                        updatedAt: new Date().toISOString(), provider: provider.id,
                                    } : cs
                                ),
                            }));

                            // Auto-save conversation summary to memory
                            try {
                                const { invoke } = await import('@tauri-apps/api/core');
                                await invoke('memory_save_conversation', {
                                    accountId: 'helix-chat',
                                    userMsg: content,
                                    assistantMsg: result.content || '',
                                });
                            } catch { /* memory save failed, non-critical */ }

                            return;
                        }

                        // Process tool calls
                        let needsConfirm = false;
                        let confirmInfo: ChatMessage['pendingConfirm'] = undefined;
                        const roundToolResults: string[] = [];

                        for (const tc of result.toolCalls) {
                            const args = JSON.parse(tc.arguments || '{}');
                            const tool = findTool(tc.name);

                            // Loop detection
                            const loopCheck = loopDetector.record(tc.name, args);
                            if (loopCheck.blocked) {
                                allToolEvents.push({ phase: 'loop_blocked', toolName: tc.name, args, meta: loopCheck.message, timestamp: Date.now() });
                                const warnMsg: ChatMessage = {
                                    id: generateId(), role: 'assistant',
                                    content: `🔁 ${loopCheck.message}\n\n已执行的工具结果：\n${allToolCalls.map(t => `• ${t.name}: ${t.result.slice(0, 200)}`).join('\n')}`,
                                    timestamp: new Date().toISOString(),
                                    toolCalls: allToolCalls.length > 0 ? allToolCalls : undefined,
                                };
                                set((s) => ({
                                    chatSessions: s.chatSessions.map((cs) =>
                                        cs.id === sessionId ? { ...cs, messages: [...cs.messages, warnMsg], updatedAt: new Date().toISOString() } : cs
                                    ),
                                }));
                                return;
                            }

                            // Dangerous operation → confirm
                            if (tool?.dangerous) {
                                needsConfirm = true;
                                confirmInfo = {
                                    toolName: tc.name,
                                    args,
                                    description: `${tool.description}\n参数: ${JSON.stringify(args, null, 2)}`,
                                };
                                break;
                            }

                            // Execute tool with retry
                            allToolEvents.push({ phase: 'start', toolName: tc.name, args, dangerous: !!tool?.dangerous, timestamp: Date.now() });
                            let toolResult = '';
                            let toolStatus: 'done' | 'error' = 'done';

                            for (let retry = 0; retry <= MAX_RETRIES_PER_TOOL; retry++) {
                                try {
                                    const { result: res } = await executeTool(tc.name, args);
                                    toolResult = res;
                                    toolStatus = 'done';
                                    break;
                                } catch (err: any) {
                                    toolResult = `执行失败: ${err.message || '未知错误'}`;
                                    toolStatus = 'error';
                                    if (retry < MAX_RETRIES_PER_TOOL) {
                                        allToolEvents.push({ phase: 'retry', toolName: tc.name, args, meta: `Retry ${retry + 1}`, timestamp: Date.now() });
                                    }
                                }
                            }

                            allToolCalls.push({ name: tc.name, args, result: toolResult, status: toolStatus });
                            allToolEvents.push({ phase: toolStatus === 'done' ? 'result' : 'error', toolName: tc.name, result: toolResult, timestamp: Date.now() });
                            roundToolResults.push(`[工具 ${tc.name}]\n${toolResult}`);
                        }

                        if (needsConfirm && confirmInfo) {
                            const confirmMsg: ChatMessage = {
                                id: generateId(), role: 'assistant',
                                content: `⚠️ 需要确认执行以下操作：\n\n**${confirmInfo.description}**\n\n请回复「确认」或「取消」`,
                                timestamp: new Date().toISOString(),
                                pendingConfirm: confirmInfo,
                                toolCalls: allToolCalls.length > 0 ? allToolCalls : undefined,
                            };
                            set((s) => ({
                                chatSessions: s.chatSessions.map((cs) =>
                                    cs.id === sessionId ? { ...cs, messages: [...cs.messages, confirmMsg], updatedAt: new Date().toISOString() } : cs
                                ),
                            }));
                            return;
                        }

                        // Feed results back for next round — include round progress
                        messages.push({ role: 'assistant', content: result.content || `[执行中 ${round + 1}/${MAX_TOOL_ROUNDS}]` });
                        messages.push({ role: 'user', content: `工具执行结果 (第${round + 1}轮)：\n${roundToolResults.join('\n\n')}\n\n请评估：任务完成了吗？如果完成，给出最终总结；如果未完成，继续执行下一步。` });
                    }

                    // Max rounds reached → summarize what happened
                    const summaryMsg: ChatMessage = {
                        id: generateId(), role: 'assistant',
                        content: `⚠️ 达到最大工具调用轮次 (${MAX_TOOL_ROUNDS})。已执行 ${allToolCalls.length} 个工具调用。\n\n${allToolCalls.map(t => `• ${t.name}: ${t.result.slice(0, 200)}`).join('\n')}`,
                        timestamp: new Date().toISOString(),
                        toolCalls: allToolCalls,
                    };
                    set((s) => ({
                        chatSessions: s.chatSessions.map((cs) =>
                            cs.id === sessionId ? { ...cs, messages: [...cs.messages, summaryMsg], updatedAt: new Date().toISOString() } : cs
                        ),
                    }));
                } catch (err: any) {
                    const errorMsg: ChatMessage = {
                        id: generateId(), role: 'assistant',
                        content: `❌ 请求失败: ${err.message || '未知错误'}`,
                        timestamp: new Date().toISOString(),
                    };
                    set((s) => ({
                        chatSessions: s.chatSessions.map((cs) =>
                            cs.id === sessionId ? { ...cs, messages: [...cs.messages, errorMsg], updatedAt: new Date().toISOString() } : cs
                        ),
                    }));
                } finally {
                    set((s) => ({ loading: { ...s.loading, chat: false } }));
                }
            },

            confirmToolExecution: async (sessionId, messageId) => {
                const session = get().chatSessions.find((s) => s.id === sessionId);
                const msg = session?.messages.find((m) => m.id === messageId);
                if (!msg?.pendingConfirm) return;

                const { toolName, args } = msg.pendingConfirm;
                const { result } = await executeTool(toolName, args);

                // Replace confirmation message with result
                set((s) => ({
                    chatSessions: s.chatSessions.map((cs) =>
                        cs.id === sessionId ? {
                            ...cs,
                            messages: cs.messages.map((m) =>
                                m.id === messageId ? {
                                    ...m,
                                    content: `✅ 已执行：${toolName}\n\n${result}`,
                                    pendingConfirm: undefined,
                                    toolCalls: [{ name: toolName, args, result, status: 'done' as const }],
                                } : m
                            ),
                            updatedAt: new Date().toISOString(),
                        } : cs
                    ),
                }));
            },

            // ===== Tasks =====
            addTask: (task) => set((s) => ({ tasks: [...s.tasks, { ...task, id: generateId() }] })),
            removeTask: (id) => set((s) => ({ tasks: s.tasks.filter((t) => t.id !== id) })),
            updateTask: (id, updates) =>
                set((s) => ({ tasks: s.tasks.map((t) => (t.id === id ? { ...t, ...updates } : t)) })),
            runTask: async (id) => {
                const task = get().tasks.find((t) => t.id === id);
                if (!task) return;
                const now = new Date().toISOString();
                get().updateTask(id, { lastRun: now, lastResult: 'success' });
                get().addLog({ timestamp: now, level: 'info', source: 'cron', message: `任务 "${task.name}" 手动触发执行成功` });
            },

            // ===== Alerts =====
            addAlert: (alert) => set((s) => ({ alerts: [...s.alerts, { ...alert, id: generateId() }] })),
            removeAlert: (id) => set((s) => ({ alerts: s.alerts.filter((a) => a.id !== id) })),
            toggleAlert: (id) =>
                set((s) => ({ alerts: s.alerts.map((a) => (a.id === id ? { ...a, enabled: !a.enabled } : a)) })),

            // ===== Logs =====
            addLog: (log) => set((s) => ({ logs: [{ ...log, id: generateId() }, ...s.logs].slice(0, 1000) })),
            clearLogs: () => set({ logs: [] }),

            // ===== Config =====
            updateConfig: (updates) => set((s) => ({ config: { ...s.config, ...updates } })),
            updateCloudConfig: (updates) =>
                set((s) => ({
                    cloudConfig: {
                        aliyun: { ...s.cloudConfig.aliyun, ...(updates.aliyun || {}) },
                        k8s: { ...s.cloudConfig.k8s, ...(updates.k8s || {}) },
                    },
                })),

            // ===== Notifications =====
            addNotificationChannel: (channel) =>
                set((s) => ({ notificationChannels: [...s.notificationChannels, { ...channel, id: generateId() }] })),
            removeNotificationChannel: (id) =>
                set((s) => ({ notificationChannels: s.notificationChannels.filter((c) => c.id !== id) })),
            updateNotificationChannel: (id, updates) =>
                set((s) => ({
                    notificationChannels: s.notificationChannels.map((c) => (c.id === id ? { ...c, ...updates } : c)),
                })),

            // ===== Skills =====
            toggleSkill: (skillId, enabled) => {
                setSkillEnabled(skillId, enabled);
                set((s) => ({ skillStates: { ...s.skillStates, [skillId]: enabled } }));
            },
            addCustomSkill: (skill) => {
                addSkillToRegistry({
                    ...skill,
                    builtin: false,
                    enabled: true,
                    tools: skill.tools.map(t => ({
                        ...t,
                        execute: async () => '(placeholder)',
                    })),
                });
                loadCustomSkills([skill]);
                set((s) => ({
                    customSkills: [...s.customSkills.filter(cs => cs.id !== skill.id), skill],
                    skillStates: { ...s.skillStates, [skill.id]: true },
                }));
            },
            removeCustomSkill: (skillId) => {
                removeSkillFromRegistry(skillId);
                set((s) => ({
                    customSkills: s.customSkills.filter((cs: CustomSkillDef) => cs.id !== skillId),
                    skillStates: Object.fromEntries(
                        Object.entries(s.skillStates).filter(([k]) => k !== skillId)
                    ),
                }));
            },
            initSkills: () => {
                const { skillStates, customSkills } = get();
                syncSkillStates(skillStates);
                if (customSkills.length > 0) loadCustomSkills(customSkills);
                // Also load agent skills on init
                const agentSkills = loadAllAgentSkills();
                // Apply persisted enable/disable states
                const states = get().skillStates;
                for (const skill of agentSkills) {
                    const key = `agent:${skill.name}`;
                    if (key in states) skill.enabled = states[key];
                }
                set({ agentSkills });
            },

            // ===== Agent Skills (SKILL.md) =====
            loadAgentSkills: () => {
                const skills = loadAllAgentSkills();
                const states = get().skillStates;
                for (const skill of skills) {
                    const key = `agent:${skill.name}`;
                    if (key in states) skill.enabled = states[key];
                }
                set({ agentSkills: skills });
            },
            toggleAgentSkill: (skillName, enabled) => {
                const key = `agent:${skillName}`;
                set((s: any) => ({
                    agentSkills: s.agentSkills.map((sk: AgentSkill) =>
                        sk.name === skillName ? { ...sk, enabled } : sk
                    ),
                    skillStates: { ...s.skillStates, [key]: enabled },
                }));
            },
        }),
        {
            name: 'devhelix-storage',
            version: 1,
            migrate: (persistedState: any, version: number) => {
                if (version === 0) {
                    // v0→v1: Remove pre-populated default providers that had no API key configured
                    const defaultIds = ['dashscope-default', 'openai-default', 'anthropic-default', 'ollama-default'];
                    if (persistedState.aiProviders) {
                        persistedState.aiProviders = persistedState.aiProviders.filter(
                            (p: any) => !defaultIds.includes(p.id) || (p.apiKey && p.apiKey.length > 0)
                        );
                    }
                }
                return persistedState;
            },
            partialize: (state) => ({
                servers: state.servers,
                aiProviders: state.aiProviders,
                chatSessions: state.chatSessions,
                tasks: state.tasks,
                alerts: state.alerts,
                config: state.config,
                cloudConfig: state.cloudConfig,
                notificationChannels: state.notificationChannels,
                skillStates: state.skillStates,
                customSkills: state.customSkills,
            }),
        }
    )
);

// Initialize skills from persisted state on load
setTimeout(() => {
    useDevOpsStore.getState().initSkills();
}, 0);
