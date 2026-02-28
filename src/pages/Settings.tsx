import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, Bot, Eye, EyeOff, Globe, Moon, Palette, Settings as SettingsIcon, Sun, Trash2, FolderOpen, Plug, KeyRound, Plus, Save, RefreshCw } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useDevOpsStore, AIProvider } from '../stores/useDevOpsStore';
import { useConfigStore } from '../stores/useConfigStore';
import { invoke } from '@tauri-apps/api/core';

type SettingsSection = 'appearance' | 'ai' | 'workspace' | 'mcp' | 'environments' | 'about';

const MENU_ITEMS: Array<{ key: SettingsSection; icon: typeof Palette; label: string; group: string }> = [
    { key: 'appearance', icon: Palette, label: '外观设置', group: '通用' },
    { key: 'ai', icon: Bot, label: 'AI 提供商', group: '通用' },
    { key: 'workspace', icon: FolderOpen, label: '工作空间', group: 'Agent' },
    { key: 'mcp', icon: Plug, label: 'MCP', group: 'Agent' },
    { key: 'environments', icon: KeyRound, label: '环境变量', group: 'Agent' },
    { key: 'about', icon: Globe, label: '关于', group: '其他' },
];

// ── Workspace types ──
interface WorkspaceFile {
    name: string;
    size: number;
    modified: string;
}

// ── MCP types ──
interface MCPClient {
    name: string;
    transport: string;
    command?: string;
    args?: string[];
    url?: string;
    env: Record<string, string>;
    enabled: boolean;
}

// ── Env types ──
interface EnvVar {
    key: string;
    value: string;
    secret: boolean;
}


function Settings() {
    const navigate = useNavigate();
    const { i18n } = useTranslation();
    const {
        aiProviders, updateAIProvider, addAIProvider, removeAIProvider,
    } = useDevOpsStore();
    const { config, saveConfig } = useConfigStore();
    const [activeSection, setActiveSection] = useState<SettingsSection>('appearance');
    const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
    const [newProvider, setNewProvider] = useState({ name: '', type: 'openai' as AIProvider['type'], baseUrl: '', apiKey: '', model: '' });
    const [showAddProvider, setShowAddProvider] = useState(false);

    // ── Workspace state ──
    const [wsFiles, setWsFiles] = useState<WorkspaceFile[]>([]);
    const [wsDir, setWsDir] = useState('');
    const [wsSelectedFile, setWsSelectedFile] = useState('');
    const [wsContent, setWsContent] = useState('');
    const [wsOrigContent, setWsOrigContent] = useState('');
    const [wsSaving, setWsSaving] = useState(false);

    // ── MCP state ──
    const [mcpClients, setMcpClients] = useState<MCPClient[]>([]);
    const [mcpShowCreate, setMcpShowCreate] = useState(false);
    const [mcpNew, setMcpNew] = useState<MCPClient>({ name: '', transport: 'stdio', command: '', args: [], url: '', env: {}, enabled: true });

    // ── Environments state ──
    const [envVars, setEnvVars] = useState<EnvVar[]>([]);
    const [envShowAdd, setEnvShowAdd] = useState(false);
    const [envNew, setEnvNew] = useState({ key: '', value: '', secret: false });
    const [envShowKeys, setEnvShowKeys] = useState<Record<string, boolean>>({});

    const toggleKey = (id: string) => setShowKeys((p) => ({ ...p, [id]: !p[id] }));

    // ── Data loaders ──
    const loadWorkspaceFiles = useCallback(async () => {
        try {
            const files = await invoke<WorkspaceFile[]>('workspace_list_files');
            setWsFiles(files);
            const dir = await invoke<string>('workspace_get_dir');
            setWsDir(dir);
        } catch (e) { console.error('workspace_list_files failed:', e); }
    }, []);

    const loadMcpClients = useCallback(async () => {
        try {
            const clients = await invoke<MCPClient[]>('mcp_list');
            setMcpClients(clients);
        } catch (e) { console.error('mcp_list failed:', e); }
    }, []);

    const loadEnvVars = useCallback(async () => {
        try {
            const vars = await invoke<EnvVar[]>('envs_list');
            setEnvVars(vars);
        } catch (e) { console.error('envs_list failed:', e); }
    }, []);

    // Load data when section changes
    useEffect(() => {
        if (activeSection === 'workspace') loadWorkspaceFiles();
        if (activeSection === 'mcp') loadMcpClients();
        if (activeSection === 'environments') loadEnvVars();
    }, [activeSection, loadWorkspaceFiles, loadMcpClients, loadEnvVars]);

    // ── Workspace handlers ──
    const wsSelectFile = async (name: string) => {
        try {
            const content = await invoke<string>('workspace_read_file', { name });
            setWsSelectedFile(name);
            setWsContent(content);
            setWsOrigContent(content);
        } catch (e) { console.error('workspace_read_file failed:', e); }
    };

    const wsSaveFile = async () => {
        if (!wsSelectedFile) return;
        setWsSaving(true);
        try {
            await invoke('workspace_write_file', { name: wsSelectedFile, content: wsContent });
            setWsOrigContent(wsContent);
        } catch (e) { console.error('workspace_write_file failed:', e); }
        setWsSaving(false);
    };

    // ── MCP handlers ──
    const mcpCreate = async () => {
        try {
            await invoke<MCPClient>('mcp_create', { client: mcpNew });
            setMcpShowCreate(false);
            setMcpNew({ name: '', transport: 'stdio', command: '', args: [], url: '', env: {}, enabled: true });
            loadMcpClients();
        } catch (e) { alert(String(e)); }
    };

    const mcpToggle = async (name: string) => {
        try {
            await invoke<MCPClient>('mcp_toggle', { name });
            loadMcpClients();
        } catch (e) { console.error(e); }
    };

    const mcpDelete = async (name: string) => {
        try {
            await invoke('mcp_delete', { name });
            loadMcpClients();
        } catch (e) { console.error(e); }
    };

    // ── Env handlers ──
    const envAdd = async () => {
        if (!envNew.key) return;
        try {
            await invoke('envs_set', { key: envNew.key, value: envNew.value, secret: envNew.secret });
            setEnvNew({ key: '', value: '', secret: false });
            setEnvShowAdd(false);
            loadEnvVars();
        } catch (e) { console.error(e); }
    };

    const envDelete = async (key: string) => {
        try {
            await invoke('envs_delete', { key });
            loadEnvVars();
        } catch (e) { console.error(e); }
    };


    // Note: Provider sync to backend is handled by syncAIProviderToBackend in useDevOpsStore
    // (triggered on add/update/remove, not on every render)

    const handleAddProvider = () => {
        if (!newProvider.name || !newProvider.baseUrl) return;
        addAIProvider({ name: newProvider.name, type: newProvider.type, baseUrl: newProvider.baseUrl, apiKey: newProvider.apiKey || undefined, models: [], enabled: !!newProvider.apiKey, defaultModel: newProvider.model || undefined });
        setNewProvider({ name: '', type: 'openai', baseUrl: '', apiKey: '', model: '' });
        setShowAddProvider(false);
    };



    // Group menu items
    const groups = MENU_ITEMS.reduce<Record<string, typeof MENU_ITEMS>>((acc, item) => {
        if (!acc[item.group]) acc[item.group] = [];
        acc[item.group].push(item);
        return acc;
    }, {});

    const renderContent = () => {
        switch (activeSection) {
            case 'appearance':
                return (
                    <div className="space-y-6">
                        <div>
                            <h2 className="text-lg font-semibold flex items-center gap-2 mb-1">
                                <Palette size={20} />外观设置
                            </h2>
                            <p className="text-xs text-base-content/50 mb-5">自定义界面主题和语言</p>
                        </div>
                        <div className="space-y-4">
                            <div className="flex items-center justify-between p-4 rounded-xl bg-base-200/50">
                                <div><p className="font-medium text-sm">主题</p><p className="text-xs text-base-content/50">切换明暗主题</p></div>
                                <label className="swap swap-rotate">
                                    <input type="checkbox" checked={config?.theme === 'dark'} onChange={() => {
                                        if (config) saveConfig({ ...config, theme: config.theme === 'dark' ? 'light' : 'dark', language: config.language }, true);
                                    }} />
                                    <Sun size={20} className="swap-on" /><Moon size={20} className="swap-off" />
                                </label>
                            </div>
                            <div className="flex items-center justify-between p-4 rounded-xl bg-base-200/50">
                                <div><p className="font-medium text-sm">语言</p><p className="text-xs text-base-content/50">选择界面语言</p></div>
                                <select className="select select-bordered select-sm" value={config?.language || 'zh'} onChange={(e) => {
                                    const lang = e.target.value;
                                    i18n.changeLanguage(lang);
                                    if (config) saveConfig({ ...config, language: lang, theme: config.theme }, true);
                                }}>
                                    <option value="zh">中文</option>
                                    <option value="en">English</option>
                                </select>
                            </div>
                        </div>
                    </div>
                );

            case 'ai':
                return (
                    <div className="space-y-5">
                        <div className="flex items-center justify-between">
                            <div>
                                <h2 className="text-lg font-semibold flex items-center gap-2 mb-1"><Bot size={20} />AI 提供商</h2>
                                <p className="text-xs text-base-content/50">管理大模型 API 连接</p>
                            </div>
                            <button className="btn btn-primary btn-sm" onClick={() => setShowAddProvider(!showAddProvider)}>
                                {showAddProvider ? '取消' : '+ 添加'}
                            </button>
                        </div>

                        {showAddProvider && (() => {
                            const PRESETS = [
                                { label: '通义千问 (DashScope)', name: '通义千问', type: 'openai' as AIProvider['type'], baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus' },
                                { label: '百炼 CodingPlan', name: 'CodingPlan', type: 'openai' as AIProvider['type'], baseUrl: 'https://coding.dashscope.aliyuncs.com/v1', model: 'qwen3-coder-plus' },
                                { label: 'OpenAI', name: 'OpenAI', type: 'openai' as AIProvider['type'], baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o' },
                                { label: 'Anthropic', name: 'Anthropic', type: 'anthropic' as AIProvider['type'], baseUrl: 'https://api.anthropic.com', model: 'claude-sonnet-4-20250514' },
                                { label: 'Ollama (本地)', name: 'Ollama', type: 'ollama' as AIProvider['type'], baseUrl: 'http://localhost:11434', model: 'qwen2' },
                                { label: '火山引擎 (Ark)', name: '火山引擎', type: 'openai' as AIProvider['type'], baseUrl: 'https://ark.cn-beijing.volces.com/api/v3', model: 'ark-code-latest' },
                                { label: '自定义', name: '自定义提供商', type: 'custom' as AIProvider['type'], baseUrl: '', model: '' },
                            ];
                            return (
                                <div className="p-4 bg-base-200/50 rounded-xl space-y-2">
                                    <select className="select select-bordered select-sm w-full" value="" onChange={(e) => {
                                        const preset = PRESETS[Number(e.target.value)];
                                        if (preset) setNewProvider({ name: preset.name, type: preset.type, baseUrl: preset.baseUrl, apiKey: '', model: preset.model });
                                    }}>
                                        <option value="" disabled>选择 AI 提供商...</option>
                                        {PRESETS.map((p, i) => <option key={i} value={i}>{p.label}</option>)}
                                    </select>
                                    {newProvider.name && (
                                        <>
                                            <input className="input input-bordered input-sm w-full" placeholder="名称" value={newProvider.name} onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })} />
                                            <input className="input input-bordered input-sm w-full" placeholder="Base URL" value={newProvider.baseUrl} onChange={(e) => setNewProvider({ ...newProvider, baseUrl: e.target.value })} />
                                            <div className="relative">
                                                <input className="input input-bordered input-sm w-full pr-10" placeholder="API Key" type={showKeys['new'] ? 'text' : 'password'} value={newProvider.apiKey} onChange={(e) => setNewProvider({ ...newProvider, apiKey: e.target.value })} />
                                                <button type="button" className="absolute right-2 top-1/2 -translate-y-1/2 btn btn-ghost btn-xs" onClick={() => toggleKey('new')}>
                                                    {showKeys['new'] ? <EyeOff size={14} /> : <Eye size={14} />}
                                                </button>
                                            </div>
                                            <button className="btn btn-primary btn-sm" onClick={handleAddProvider} disabled={!newProvider.name || !newProvider.baseUrl}>保存</button>
                                        </>
                                    )}
                                </div>
                            );
                        })()}

                        <div className="space-y-3">
                            {aiProviders.map((provider) => (
                                <div key={provider.id} className="p-4 rounded-xl bg-base-200/50 space-y-3">
                                    <div className="flex items-center justify-between">
                                        <div className="flex items-center gap-3">
                                            <div className="font-semibold text-sm">{provider.name}</div>
                                            <span className="text-xs px-2 py-0.5 rounded-full bg-violet-500/10 text-violet-500">{provider.type}</span>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <label className="label cursor-pointer gap-2">
                                                <span className="label-text text-xs">{provider.enabled ? '已启用' : '已禁用'}</span>
                                                <div
                                                    className={`relative w-10 h-5 rounded-full cursor-pointer transition-colors ${provider.enabled ? 'bg-green-500' : 'bg-gray-300'}`}
                                                    onClick={() => updateAIProvider(provider.id, { enabled: !provider.enabled })}
                                                >
                                                    <div className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${provider.enabled ? 'translate-x-5' : 'translate-x-0.5'}`} />
                                                </div>
                                            </label>
                                            <button className="btn btn-ghost btn-xs text-red-500" onClick={() => removeAIProvider(provider.id)}><Trash2 size={14} /></button>
                                        </div>
                                    </div>
                                    <div className="space-y-2">
                                        <div>
                                            <label className="text-xs text-base-content/50">Base URL</label>
                                            <input className="input input-bordered input-xs w-full" value={provider.baseUrl || ''} onChange={(e) => updateAIProvider(provider.id, { baseUrl: e.target.value })} />
                                        </div>
                                        <div>
                                            <label className="text-xs text-base-content/50">API Key</label>
                                            <div className="flex gap-1">
                                                <input className="input input-bordered input-xs flex-1" type={showKeys[provider.id] ? 'text' : 'password'} value={provider.apiKey || ''} onChange={(e) => updateAIProvider(provider.id, { apiKey: e.target.value })} />
                                                <button className="btn btn-ghost btn-xs" onClick={() => toggleKey(provider.id)}>
                                                    {showKeys[provider.id] ? <EyeOff size={14} /> : <Eye size={14} />}
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            ))}
                            {aiProviders.length === 0 && !showAddProvider && (
                                <p className="text-sm text-base-content/40 text-center py-6">暂无 AI 提供商，点击"+ 添加"配置</p>
                            )}
                        </div>
                    </div>
                );

            // ── Workspace ──
            case 'workspace':
                return (
                    <div className="space-y-5">
                        <div className="flex items-center justify-between">
                            <div>
                                <h2 className="text-lg font-semibold flex items-center gap-2 mb-1"><FolderOpen size={20} />工作空间</h2>
                                <p className="text-xs text-base-content/50">管理 Agent 配置文件 · {wsDir}</p>
                            </div>
                            <button className="btn btn-ghost btn-sm" onClick={loadWorkspaceFiles}><RefreshCw size={14} /></button>
                        </div>
                        <div className="flex gap-4" style={{ minHeight: 400 }}>
                            {/* File list */}
                            <div className="w-48 shrink-0 space-y-1">
                                {wsFiles.map(f => (
                                    <button
                                        key={f.name}
                                        className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${wsSelectedFile === f.name ? 'bg-primary/10 text-primary font-medium' : 'hover:bg-base-200 text-base-content/70'}`}
                                        onClick={() => wsSelectFile(f.name)}
                                    >
                                        <div className="font-medium">{f.name}</div>
                                        <div className="text-[10px] text-base-content/40">{(f.size / 1024).toFixed(1)} KB</div>
                                    </button>
                                ))}
                                {wsFiles.length === 0 && <p className="text-xs text-base-content/40 p-3">无文件</p>}
                            </div>
                            {/* Editor */}
                            <div className="flex-1 flex flex-col">
                                {wsSelectedFile ? (
                                    <>
                                        <div className="flex items-center justify-between mb-2">
                                            <span className="text-sm font-medium">{wsSelectedFile}</span>
                                            <button
                                                className="btn btn-primary btn-sm gap-1"
                                                onClick={wsSaveFile}
                                                disabled={wsSaving || wsContent === wsOrigContent}
                                            >
                                                <Save size={14} />{wsSaving ? '保存中...' : '保存'}
                                            </button>
                                        </div>
                                        <textarea
                                            className="textarea textarea-bordered flex-1 font-mono text-sm leading-relaxed"
                                            value={wsContent}
                                            onChange={e => setWsContent(e.target.value)}
                                            style={{ minHeight: 350, resize: 'vertical' }}
                                        />
                                    </>
                                ) : (
                                    <div className="flex items-center justify-center h-full text-sm text-base-content/40">
                                        选择文件进行编辑
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                );

            // ── MCP ──
            case 'mcp':
                return (
                    <div className="space-y-5">
                        <div className="flex items-center justify-between">
                            <div>
                                <h2 className="text-lg font-semibold flex items-center gap-2 mb-1"><Plug size={20} />MCP 客户端</h2>
                                <p className="text-xs text-base-content/50">管理 Model Context Protocol 连接</p>
                            </div>
                            <button className="btn btn-primary btn-sm" onClick={() => setMcpShowCreate(!mcpShowCreate)}>
                                {mcpShowCreate ? '取消' : <><Plus size={14} /> 添加</>}
                            </button>
                        </div>

                        {mcpShowCreate && (
                            <div className="p-4 bg-base-200/50 rounded-xl space-y-3">
                                <input className="input input-bordered input-sm w-full" placeholder="名称 (如 tavily_mcp)" value={mcpNew.name} onChange={e => setMcpNew({ ...mcpNew, name: e.target.value })} />
                                <select className="select select-bordered select-sm w-full" value={mcpNew.transport} onChange={e => setMcpNew({ ...mcpNew, transport: e.target.value })}>
                                    <option value="stdio">stdio (本地命令)</option>
                                    <option value="sse">SSE (远程服务)</option>
                                </select>
                                {mcpNew.transport === 'stdio' ? (
                                    <input className="input input-bordered input-sm w-full" placeholder="命令 (如 npx -y @tavily/mcp)" value={mcpNew.command || ''} onChange={e => setMcpNew({ ...mcpNew, command: e.target.value })} />
                                ) : (
                                    <input className="input input-bordered input-sm w-full" placeholder="URL (如 http://localhost:3001/sse)" value={mcpNew.url || ''} onChange={e => setMcpNew({ ...mcpNew, url: e.target.value })} />
                                )}
                                <button className="btn btn-primary btn-sm" onClick={mcpCreate} disabled={!mcpNew.name || (mcpNew.transport === 'stdio' ? !mcpNew.command : !mcpNew.url)}>
                                    创建
                                </button>
                            </div>
                        )}

                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                            {mcpClients.map(client => (
                                <div key={client.name} className="p-4 rounded-xl bg-base-200/50 space-y-2">
                                    <div className="flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <span className="font-semibold text-sm">{client.name}</span>
                                            <span className="text-[10px] px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-500">{client.transport}</span>
                                        </div>
                                        <div className={`text-xs ${client.enabled ? 'text-green-500' : 'text-base-content/40'}`}>
                                            ● {client.enabled ? '启用' : '禁用'}
                                        </div>
                                    </div>
                                    <div className="text-xs text-base-content/50 break-all">
                                        {client.transport === 'stdio' ? client.command : client.url}
                                    </div>
                                    <div className="flex gap-2 pt-1">
                                        <button className="btn btn-ghost btn-xs" onClick={() => mcpToggle(client.name)}>
                                            {client.enabled ? '禁用' : '启用'}
                                        </button>
                                        <button className="btn btn-ghost btn-xs text-red-500" onClick={() => mcpDelete(client.name)}>
                                            <Trash2 size={12} /> 删除
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                        {mcpClients.length === 0 && !mcpShowCreate && (
                            <p className="text-sm text-base-content/40 text-center py-6">暂无 MCP 客户端</p>
                        )}
                    </div>
                );

            // ── Environments ──
            case 'environments':
                return (
                    <div className="space-y-5">
                        <div className="flex items-center justify-between">
                            <div>
                                <h2 className="text-lg font-semibold flex items-center gap-2 mb-1"><KeyRound size={20} />环境变量</h2>
                                <p className="text-xs text-base-content/50">管理 Agent 运行环境变量</p>
                            </div>
                            <button className="btn btn-primary btn-sm" onClick={() => setEnvShowAdd(!envShowAdd)}>
                                {envShowAdd ? '取消' : <><Plus size={14} /> 添加</>}
                            </button>
                        </div>

                        {envShowAdd && (
                            <div className="p-4 bg-base-200/50 rounded-xl space-y-2">
                                <input className="input input-bordered input-sm w-full" placeholder="KEY" value={envNew.key} onChange={e => setEnvNew({ ...envNew, key: e.target.value })} />
                                <input className="input input-bordered input-sm w-full" placeholder="VALUE" type={envNew.secret ? 'password' : 'text'} value={envNew.value} onChange={e => setEnvNew({ ...envNew, value: e.target.value })} />
                                <label className="label cursor-pointer gap-2 justify-start">
                                    <input type="checkbox" className="checkbox checkbox-sm" checked={envNew.secret} onChange={e => setEnvNew({ ...envNew, secret: e.target.checked })} />
                                    <span className="label-text text-xs">密钥 (界面脱敏)</span>
                                </label>
                                <button className="btn btn-primary btn-sm" onClick={envAdd} disabled={!envNew.key}>保存</button>
                            </div>
                        )}

                        <div className="space-y-2">
                            {envVars.map(env => (
                                <div key={env.key} className="flex items-center gap-3 p-3 rounded-xl bg-base-200/50">
                                    <div className="flex-1 min-w-0">
                                        <span className="font-mono text-sm font-medium">{env.key}</span>
                                        <span className="mx-2 text-base-content/30">=</span>
                                        <span className="font-mono text-sm text-base-content/60">
                                            {env.secret && !envShowKeys[env.key]
                                                ? '••••••••'
                                                : env.value}
                                        </span>
                                    </div>
                                    <div className="flex items-center gap-1 shrink-0">
                                        {env.secret && (
                                            <button className="btn btn-ghost btn-xs" onClick={() => setEnvShowKeys(p => ({ ...p, [env.key]: !p[env.key] }))}>
                                                {envShowKeys[env.key] ? <EyeOff size={12} /> : <Eye size={12} />}
                                            </button>
                                        )}
                                        <button className="btn btn-ghost btn-xs text-red-500" onClick={() => envDelete(env.key)}>
                                            <Trash2 size={12} />
                                        </button>
                                    </div>
                                </div>
                            ))}
                            {envVars.length === 0 && !envShowAdd && (
                                <p className="text-sm text-base-content/40 text-center py-6">暂无环境变量</p>
                            )}
                        </div>
                    </div>
                );

            case 'about':
                return (
                    <div className="space-y-5">
                        <div>
                            <h2 className="text-lg font-semibold flex items-center gap-2 mb-1"><Globe size={20} />关于</h2>
                            <p className="text-xs text-base-content/50">应用信息</p>
                        </div>
                        <div className="p-5 rounded-xl bg-base-200/50 space-y-2 text-sm text-base-content/70">
                            <p><strong className="text-base-content">Helix</strong> — AI 驱动的智能体平台</p>
                            <p>版本: 1.0.0</p>
                            <p>基于 Tauri + React 构建</p>
                        </div>
                    </div>
                );
        }
    };

    return (
        <div className="flex h-full overflow-hidden">
            {/* Sidebar Menu */}
            <aside className="w-56 shrink-0 border-r border-base-200 bg-base-100 overflow-y-auto">
                <div className="p-4 pb-2">
                    <h1 className="text-lg font-bold text-base-content flex items-center gap-2">
                        <button
                            onClick={() => navigate('/')}
                            className="p-1 rounded-lg hover:bg-base-200 transition-colors text-base-content/50"
                            title="返回对话"
                        >
                            <ArrowLeft size={18} />
                        </button>
                        <SettingsIcon size={20} />
                        设置
                    </h1>
                </div>
                <nav className="px-2 pb-4">
                    {Object.entries(groups).map(([groupLabel, items]) => (
                        <div key={groupLabel} className="mb-3">
                            <div className="text-[10px] font-semibold text-base-content/30 uppercase tracking-wider px-3 mb-1">
                                {groupLabel}
                            </div>
                            {items.map((item) => {
                                const Icon = item.icon;
                                const isActive = activeSection === item.key;
                                return (
                                    <button
                                        key={item.key}
                                        className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition-colors ${isActive
                                            ? 'bg-primary/10 text-primary font-medium'
                                            : 'text-base-content/60 hover:bg-base-200 hover:text-base-content'
                                            }`}
                                        onClick={() => setActiveSection(item.key)}
                                    >
                                        <Icon size={16} />
                                        {item.label}
                                    </button>
                                );
                            })}
                        </div>
                    ))}
                </nav>
            </aside>

            {/* Content Panel */}
            <main className="flex-1 overflow-y-auto p-8">
                <div className="max-w-2xl">
                    {renderContent()}
                </div>
            </main>
        </div>
    );
}

export default Settings;
