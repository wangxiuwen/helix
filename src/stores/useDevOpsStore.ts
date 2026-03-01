import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { executeTool, setSkillEnabled, addCustomSkill as addSkillToRegistry, removeCustomSkill as removeSkillFromRegistry, syncSkillStates, loadCustomSkills, loadAllAgentSkills, type OpsSkill, type ToolParameter, type AgentSkill } from '../services/opsTools';
import { invoke } from '@tauri-apps/api/core';

function syncAIProviderToBackend(providers: AIProvider[]) {
    // We now allow multiple providers, so syncing a global default is less strict.
    // The actual active provider for a chat is synced right before sendMessage.
    if (typeof window === 'undefined') return;
    const active = providers.find(p => p.enabled);
    if (!active) return;

    console.log('[syncAIProvider] Syncing default provider to backend:', active.name, active.baseUrl);
    invoke('ai_set_config', {
        provider: active.type,
        baseUrl: active.baseUrl || '',
        apiKey: active.apiKey || '',
        model: active.defaultModel || active.models?.[0] || 'qwen-plus',
        autoReply: true,
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
    images?: string[];  // base64 data URLs for image messages
    files?: Array<{ name: string; path: string; mime: string; size: string }>;
    toolCalls?: Array<{ name: string; args: Record<string, any>; result?: string; status?: 'pending' | 'done' | 'error' }>;
    pendingConfirm?: { toolName: string; args: Record<string, any>; description: string };
}

export interface TeamMessage {
    id: string;
    role: string;
    name: string;
    content?: string;
    action?: string;
    icon?: string;
    avatar?: string;
    isProgress?: boolean;
}

export interface TeamSession {
    id: string;
    title: string;
    messages: TeamMessage[];
    workspace: string;
    createdAt: string;
    updatedAt: string;
}

export interface ChatSession {
    id: string;
    title: string;
    messages: ChatMessage[];
    workspace?: string;  // working directory for this session
    model?: string;
    provider?: string;
    agentAvatarUrl?: string; // Optional custom avatar for the agent in this session
    pinned?: boolean; // Pinned to top
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

export interface BotChannel {
    id: string;
    name: string;
    type: 'feishu' | 'dingtalk' | 'wecom' | 'console' | 'discord' | 'qq' | 'imessage' | 'telegram' | 'custom';
    enabled: boolean;
    botPrefix?: string;
    config: Record<string, string>; // Store token/secret/webhookUrl etc.
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
    teamSessions: TeamSession[];
    activeTeamSessionId: string | null;
    tasks: AutoTask[];
    alerts: AlertRule[];
    logs: LogEntry[];
    config: DevOpsConfig;
    cloudConfig: CloudConfig;
    botChannels: BotChannel[];
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
    createChatSession: (title?: string, workspace?: string) => string;
    deleteChatSession: (id: string) => void;
    setActiveChatId: (id: string | null) => void;
    updateChatSession: (id: string, updates: Partial<ChatSession>) => void;
    togglePinChatSession: (id: string) => void;
    sendMessage: (sessionId: string, content: string, images?: string[]) => Promise<void>;
    confirmToolExecution: (sessionId: string, messageId: string) => Promise<void>;

    // Team Chat
    createTeamSession: (title?: string, workspace?: string) => string;
    deleteTeamSession: (id: string) => void;
    setActiveTeamSessionId: (id: string | null) => void;
    updateTeamSession: (id: string, updates: Partial<TeamSession>) => void;
    addTeamMessage: (sessionId: string, msg: Omit<TeamMessage, 'id'>) => string;
    updateTeamMessage: (sessionId: string, msgId: string, updates: Partial<TeamMessage>) => void;

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

    // Channels / Bots
    addBotChannel: (channel: Omit<BotChannel, 'id'>) => void;
    removeBotChannel: (id: string) => void;
    updateBotChannel: (id: string, updates: Partial<BotChannel>) => void;

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
            teamSessions: [],
            activeTeamSessionId: null,
            tasks: [],
            alerts: [],
            logs: [],
            config: { theme: 'light', language: 'zh', refreshInterval: 30 },
            cloudConfig: {
                aliyun: { accessKeyId: '', accessKeySecret: '', region: 'cn-beijing' },
                k8s: { kubeconfigPath: '~/.kube/config', context: '', namespace: 'default' },
            },
            botChannels: [],
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
                        return p;
                    });
                    syncAIProviderToBackend(newProviders);
                    return { aiProviders: newProviders };
                }),

            // ===== Chat with Function Calling =====
            createChatSession: (title, workspace) => {
                const id = generateId();
                const autoWorkspace = workspace || `~/.helix/sandbox/${id}`;
                const session: ChatSession = {
                    id, title: title || `新对话 ${new Date().toLocaleString()}`,
                    messages: [], workspace: autoWorkspace, createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(),
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
            updateChatSession: (id, updates) =>
                set((s) => ({
                    chatSessions: s.chatSessions.map((cs) => (cs.id === id ? { ...cs, ...updates } : cs)),
                })),
            togglePinChatSession: (id) =>
                set((s) => ({
                    chatSessions: s.chatSessions.map(cs => cs.id === id ? { ...cs, pinned: !cs.pinned } : cs)
                })),

            sendMessage: async (sessionId, content, images) => {
                const state = get();
                const session = state.chatSessions.find((s) => s.id === sessionId);
                if (!session) return;

                const newUserMsgId = generateId();
                // Add user message
                set((s) => ({
                    chatSessions: s.chatSessions.map((cs) =>
                        cs.id === sessionId ? {
                            ...cs,
                            messages: [...cs.messages, { id: newUserMsgId, role: 'user' as const, content, images, timestamp: new Date().toISOString() }],
                            updatedAt: new Date().toISOString(),
                        } : cs
                    ),
                    loading: { ...s.loading, [`chat-${sessionId}`]: true },
                }));

                try {
                    const { invoke } = await import('@tauri-apps/api/core');
                    const accountId = `chat:${sessionId}`;

                    // Ensure backend config is up-to-date with session's provider/model
                    const activeP = session.provider
                        ? get().aiProviders.find(p => p.id === session.provider)
                        : get().aiProviders.find(p => p.enabled);
                    const currentModel = session.model || activeP?.defaultModel || activeP?.models?.[0] || '';
                    if (activeP) {
                        await invoke('ai_set_config', {
                            provider: activeP.type,
                            baseUrl: activeP.baseUrl || '',
                            apiKey: activeP.apiKey || '',
                            model: currentModel,
                        });
                    }

                    // If model changed since last message in this session, clear backend history
                    if (session.messages.length > 0) {
                        const lastMsg = session.messages[session.messages.length - 1];
                        if (lastMsg.model && lastMsg.model !== currentModel) {
                            await invoke('agent_clear_history', { accountId }).catch(() => { });
                        }
                    }

                    // Track current model on user message
                    set((s) => ({
                        chatSessions: s.chatSessions.map((cs) =>
                            cs.id === sessionId ? {
                                ...cs,
                                provider: activeP?.id,
                                model: currentModel,
                                messages: cs.messages.map(m => m.id === newUserMsgId ? { ...m, model: currentModel } : m)
                            } : cs
                        ),
                    }));

                    const result = await invoke<{ content: string; files?: Array<{ name: string; path: string; mime: string; size: string }> }>('agent_chat', {
                        accountId,
                        content,
                        images: images || [],
                        workspace: session?.workspace || null,
                    });

                    const assistantMsg: ChatMessage = {
                        id: generateId(), role: 'assistant',
                        content: result.content || '(无响应)',
                        timestamp: new Date().toISOString(),
                        ...(result.files && result.files.length > 0 ? { files: result.files } : {}),
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
                    set((s) => ({ loading: { ...s.loading, [`chat-${sessionId}`]: false } }));
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

            // ===== Team Chat =====
            createTeamSession: (title, workspace) => {
                const id = generateId();
                const session: TeamSession = {
                    id, title: title || `需求讨论 ${new Date().toLocaleString()}`,
                    messages: [], workspace: workspace || '', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(),
                };
                set((s) => ({ teamSessions: [session, ...s.teamSessions], activeTeamSessionId: id }));
                return id;
            },
            deleteTeamSession: (id) =>
                set((s) => ({
                    teamSessions: s.teamSessions.filter((cs) => cs.id !== id),
                    activeTeamSessionId: s.activeTeamSessionId === id ? null : s.activeTeamSessionId,
                })),
            setActiveTeamSessionId: (id) => set({ activeTeamSessionId: id }),
            updateTeamSession: (id, updates) =>
                set((s) => ({
                    teamSessions: s.teamSessions.map((cs) => (cs.id === id ? { ...cs, ...updates } : cs)),
                })),
            addTeamMessage: (sessionId, msg) => {
                const newMsg: TeamMessage = { ...msg, id: generateId() };
                set((s) => ({
                    teamSessions: s.teamSessions.map((cs) =>
                        cs.id === sessionId ? {
                            ...cs,
                            messages: [...cs.messages, newMsg],
                            updatedAt: new Date().toISOString()
                        } : cs
                    ),
                }));
                return newMsg.id;
            },
            updateTeamMessage: (sessionId, msgId, updates) => {
                set((s) => ({
                    teamSessions: s.teamSessions.map((cs) =>
                        cs.id === sessionId ? {
                            ...cs,
                            messages: cs.messages.map(m => m.id === msgId ? { ...m, ...updates } : m),
                            updatedAt: new Date().toISOString()
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

            // ===== Channels / Bots =====
            addBotChannel: (channel) =>
                set((s) => ({ botChannels: [...s.botChannels, { ...channel, id: generateId() }] })),
            removeBotChannel: (id) =>
                set((s) => ({ botChannels: s.botChannels.filter((c) => c.id !== id) })),
            updateBotChannel: (id, updates) =>
                set((s) => ({
                    botChannels: s.botChannels.map((c) => (c.id === id ? { ...c, ...updates } : c)),
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
                teamSessions: state.teamSessions,
                tasks: state.tasks,
                alerts: state.alerts,
                config: state.config,
                cloudConfig: state.cloudConfig,
                botChannels: state.botChannels,
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
