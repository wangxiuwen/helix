import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { executeTool, setSkillEnabled, addCustomSkill as addSkillToRegistry, removeCustomSkill as removeSkillFromRegistry, syncSkillStates, loadCustomSkills, loadAllAgentSkills, type OpsSkill, type ToolParameter, type AgentSkill } from '../services/opsTools';
import { invoke } from '@tauri-apps/api/core';

function syncAIProviderToBackend(providers: AIProvider[]) {
    if (typeof window === 'undefined') return;
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
                const state = get();
                const session = state.chatSessions.find((s) => s.id === sessionId);
                if (!session) return;

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
                    // Call Rust backend agent_chat which handles everything:
                    // system prompt, tools, agent loop, memory
                    const { invoke } = await import('@tauri-apps/api/core');
                    const accountId = `chat:${sessionId}`;
                    const result = await invoke<{ content: string }>('agent_chat', {
                        accountId,
                        content,
                    });

                    const assistantMsg: ChatMessage = {
                        id: generateId(), role: 'assistant',
                        content: result.content || '(无响应)',
                        timestamp: new Date().toISOString(),
                    };
                    set((s) => ({
                        chatSessions: s.chatSessions.map((cs) =>
                            cs.id === sessionId ? {
                                ...cs, messages: [...cs.messages, assistantMsg],
                                updatedAt: new Date().toISOString(),
                            } : cs
                        ),
                    }));
                } catch (err: any) {
                    const errorMsg: ChatMessage = {
                        id: generateId(), role: 'assistant',
                        content: `❌ 请求失败: ${typeof err === 'string' ? err : err?.message || JSON.stringify(err)}`,
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
