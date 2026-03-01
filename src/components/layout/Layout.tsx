import { useState, useRef, useEffect, useCallback } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import ToastContainer from '../common/ToastContainer';
import { useConfigStore } from '../../stores/useConfigStore';
import { useDevOpsStore, AIProvider } from '../../stores/useDevOpsStore';
import { invoke } from '@tauri-apps/api/core';
import {
    MessageSquare,
    Blocks,
    Clock,
    Activity,
    Moon,
    Sun,
    Sparkles,
    Menu,
    Bot,
    Eye,
    EyeOff,
    Globe,
    Palette,
    Settings as SettingsIcon,
    Trash2,
    X,
    FolderOpen,
    Plug,
    KeyRound,
    Save,
    RefreshCw,
} from 'lucide-react';

type SettingsSection = 'appearance' | 'ai' | 'workspace' | 'environments' | 'notifications' | 'about';

interface WorkspaceFile { name: string; size: number; modified: string; }
interface EnvVar { key: string; value: string; secret: boolean; }

function Layout() {
    const { t } = useTranslation();
    const { i18n } = useTranslation();
    const navigate = useNavigate();
    const location = useLocation();
    const { config, saveConfig } = useConfigStore();
    const { aiProviders, updateAIProvider, addAIProvider, removeAIProvider } = useDevOpsStore();
    const isDark = config?.theme === 'dark';

    // More menu state
    const [showMoreMenu, setShowMoreMenu] = useState(false);
    const moreMenuRef = useRef<HTMLDivElement>(null);

    // Settings modal state
    const [showSettings, setShowSettings] = useState(false);
    const [settingsSection, setSettingsSection] = useState<SettingsSection>('appearance');
    const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
    const [newProvider, setNewProvider] = useState({ name: '', type: 'openai' as AIProvider['type'], baseUrl: '', apiKey: '', model: '', models: [] as string[] });
    const [showAddProvider, setShowAddProvider] = useState(false);

    // Workspace state
    const [wsFiles, setWsFiles] = useState<WorkspaceFile[]>([]);
    const [wsDir, setWsDir] = useState('');
    const [wsSelectedFile, setWsSelectedFile] = useState('');
    const [wsContent, setWsContent] = useState('');
    const [wsOrigContent, setWsOrigContent] = useState('');
    const [wsSaving, setWsSaving] = useState(false);

    // Env state
    const [envVars, setEnvVars] = useState<EnvVar[]>([]);
    const [envShowAdd, setEnvShowAdd] = useState(false);
    const [envNew, setEnvNew] = useState({ key: '', value: '', secret: false });
    const [envShowKeys, setEnvShowKeys] = useState<Record<string, boolean>>({});

    // Notifications state
    const { notificationChannels, addNotificationChannel, removeNotificationChannel, updateNotificationChannel } = useDevOpsStore();
    const [notifShowAdd, setNotifShowAdd] = useState(false);
    const [notifNew, setNotifNew] = useState({ name: '', type: 'feishu' as 'feishu' | 'dingtalk' | 'wecom', webhookUrl: '', enabled: true });

    const toggleKey = (id: string) => setShowKeys((p) => ({ ...p, [id]: !p[id] }));

    const toggleTheme = () => {
        if (!config) return;
        const newTheme = config.theme === 'light' ? 'dark' : 'light';
        saveConfig({ ...config, theme: newTheme, language: config.language }, true);
    };

    // Data loaders
    const loadWsFiles = useCallback(async () => {
        try {
            const files = await invoke<WorkspaceFile[]>('workspace_list_files');
            setWsFiles(files);
            const dir = await invoke<string>('workspace_get_dir');
            setWsDir(dir);
        } catch (e) { console.error('workspace_list_files', e); }
    }, []);

    const loadEnvVars = useCallback(async () => {
        try { setEnvVars(await invoke<EnvVar[]>('envs_list')); } catch (e) { console.error('envs_list', e); }
    }, []);

    useEffect(() => {
        if (!showSettings) return;
        if (settingsSection === 'workspace') loadWsFiles();
        if (settingsSection === 'environments') loadEnvVars();
    }, [showSettings, settingsSection, loadWsFiles, loadEnvVars]);

    // Workspace handlers
    const wsSelectFile = async (name: string) => {
        try {
            const content = await invoke<string>('workspace_read_file', { name });
            setWsSelectedFile(name);
            setWsContent(content);
            setWsOrigContent(content);
        } catch (e) { console.error(e); }
    };
    const wsSaveFile = async () => {
        if (!wsSelectedFile) return;
        setWsSaving(true);
        try { await invoke('workspace_write_file', { name: wsSelectedFile, content: wsContent }); setWsOrigContent(wsContent); } catch (e) { console.error(e); }
        setWsSaving(false);
    };

    // Env handlers
    const envAdd = async () => {
        if (!envNew.key) return;
        try { await invoke('envs_set', { key: envNew.key, value: envNew.value, secret: envNew.secret }); setEnvNew({ key: '', value: '', secret: false }); setEnvShowAdd(false); loadEnvVars(); } catch (e) { console.error(e); }
    };
    const envDelete = async (key: string) => { try { await invoke('envs_delete', { key }); loadEnvVars(); } catch (e) { console.error(e); } };

    // Notification handlers
    const notifAdd = () => {
        if (!notifNew.name || !notifNew.webhookUrl) return;
        addNotificationChannel(notifNew);
        setNotifNew({ name: '', type: 'feishu', webhookUrl: '', enabled: true });
        setNotifShowAdd(false);
    };

    // Close more menu on outside click
    useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (moreMenuRef.current && !moreMenuRef.current.contains(e.target as Node)) {
                setShowMoreMenu(false);
            }
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

    const handleAddProvider = () => {
        if (!newProvider.name || !newProvider.baseUrl) return;
        addAIProvider({
            name: newProvider.name,
            type: newProvider.type,
            baseUrl: newProvider.baseUrl,
            apiKey: newProvider.apiKey || undefined,
            models: newProvider.models,
            enabled: true,
            defaultModel: newProvider.model || newProvider.models[0] || undefined,
        });
        setNewProvider({ name: '', type: 'openai', baseUrl: '', apiKey: '', model: '', models: [] });
        setShowAddProvider(false);
    };

    const navItems = [
        { path: '/', icon: MessageSquare, label: t('nav.channels', '对话'), active: location.pathname === '/' },
        { path: '/skills', icon: Blocks, label: t('nav.skills', '技能'), active: location.pathname === '/skills' },
        { path: '/mcp', icon: Plug, label: 'MCP', active: location.pathname === '/mcp' },
        { path: '/cron-jobs', icon: Clock, label: t('nav.cron_jobs', '定时任务'), active: location.pathname === '/cron-jobs' },
        { path: '/logs', icon: Activity, label: t('nav.logs', '日志'), active: location.pathname === '/logs' },
    ];

    const PRESETS = [
        {
            label: '通义千问 (DashScope)', name: '通义千问', type: 'openai' as AIProvider['type'],
            baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus',
            models: [
                'qwen3-max', 'qwen3-max-2025-09-23', 'qwen3-max-2026-01-23',
                'qwen3-plus', 'qwen3-plus-2025-09-19',
                'qwen3-turbo', 'qwen3-turbo-2025-04-28',
                'qwen3-coder-plus',
                'qwen-max', 'qwen-max-latest',
                'qwen-plus', 'qwen-plus-latest',
                'qwen-turbo', 'qwen-turbo-latest',
                'qwen-long',
                'qwen-coder-plus', 'qwen-coder-turbo',
                'qwen2.5-coder-32b-instruct', 'qwen2.5-coder-7b-instruct',
                'qwq-32b', 'qwq-plus',
                'qwen2.5-72b-instruct', 'qwen2.5-32b-instruct', 'qwen2.5-14b-instruct',
                'qwen2.5-7b-instruct',
                'deepseek-v3', 'deepseek-r1',
            ],
        },
        {
            label: '百炼 CodingPlan', name: 'CodingPlan', type: 'openai' as AIProvider['type'],
            baseUrl: 'https://coding.dashscope.aliyuncs.com/v1', model: 'qwen3-coder-plus',
            models: ['qwen3-coder-plus', 'qwen-coder-plus', 'qwen-coder-turbo'],
        },
        {
            label: 'OpenAI', name: 'OpenAI', type: 'openai' as AIProvider['type'],
            baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o',
            models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'o3', 'o3-mini', 'o4-mini'],
        },
        {
            label: 'Anthropic', name: 'Anthropic', type: 'anthropic' as AIProvider['type'],
            baseUrl: 'https://api.anthropic.com', model: 'claude-sonnet-4-20250514',
            models: ['claude-opus-4-5', 'claude-sonnet-4-5', 'claude-sonnet-4-20250514', 'claude-haiku-4-5', 'claude-3-7-sonnet-latest'],
        },
        {
            label: 'Ollama (本地)', name: 'Ollama', type: 'ollama' as AIProvider['type'],
            baseUrl: 'http://localhost:11434/v1', model: 'qwen2.5',
            models: [],
        },
        {
            label: '火山引擎 (Ark)', name: '火山引擎', type: 'openai' as AIProvider['type'],
            baseUrl: 'https://ark.cn-beijing.volces.com/api/v3', model: 'ark-code-latest',
            models: ['ark-code-latest', 'ark-code'],
        },
        { label: '自定义', name: '自定义提供商', type: 'custom' as AIProvider['type'], baseUrl: '', model: '', models: [] },
    ];

    const SETTINGS_MENU: Array<{ key: SettingsSection; icon: typeof Palette; label: string; group: string }> = [
        { key: 'appearance', icon: Palette, label: t('settings.menu.appearance', '外观'), group: t('settings.groups.general', '通用') },
        { key: 'ai', icon: Bot, label: t('settings.menu.ai_providers', 'AI 提供商'), group: t('settings.groups.general', '通用') },
        { key: 'workspace', icon: FolderOpen, label: t('settings.menu.workspace', '工作空间'), group: t('settings.groups.agent', 'Agent') },
        { key: 'environments', icon: KeyRound, label: t('settings.menu.environments', '环境变量'), group: t('settings.groups.agent', 'Agent') },
        { key: 'notifications', icon: Activity, label: t('settings.menu.notifications', '消息通知'), group: t('settings.groups.agent', 'Agent') },
        { key: 'about', icon: Globe, label: t('settings.menu.about', '关于'), group: t('settings.groups.other', '其他') },
    ];

    const menuGroups = SETTINGS_MENU.reduce<Record<string, typeof SETTINGS_MENU>>((acc, item) => {
        if (!acc[item.group]) acc[item.group] = [];
        acc[item.group].push(item);
        return acc;
    }, {});


    const renderSettingsContent = () => {
        switch (settingsSection) {
            case 'appearance':
                return (
                    <div className="space-y-4">
                        <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-4">{t('settings.appearance.title', '外观设置')}</h3>
                        <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e]">
                            <div className="flex items-center justify-between mb-4">
                                <div><p className="text-sm font-medium text-gray-800 dark:text-gray-200">{t('settings.appearance.theme', '主题')}</p><p className="text-xs text-gray-400">{t('settings.appearance.theme_desc', '切换明暗主题')}</p></div>
                                <div
                                    className={`relative w-11 h-6 rounded-full cursor-pointer transition-colors ${isDark ? 'bg-[#07c160]' : 'bg-gray-300'}`}
                                    onClick={toggleTheme}
                                >
                                    <div className={`absolute top-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform ${isDark ? 'translate-x-5' : 'translate-x-0.5'}`} />
                                </div>
                            </div>
                            <div className="flex items-center justify-between">
                                <div><p className="text-sm font-medium text-gray-800 dark:text-gray-200">语言</p><p className="text-xs text-gray-400">选择界面语言</p></div>
                                <select className="px-2 py-1 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200" value={config?.language || 'zh'} onChange={(e) => {
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
                    <div className="space-y-4">
                        <div className="flex items-center justify-between mb-1">
                            <h3 className="text-sm font-bold text-gray-800 dark:text-white">{t('settings.ai.title', 'AI 提供商')}</h3>
                            <button className="text-xs text-[#07c160] hover:underline" onClick={() => setShowAddProvider(!showAddProvider)}>
                                {showAddProvider ? t('settings.ai.cancel', '取消') : '+ ' + t('settings.ai.add', '添加提供商')}
                            </button>
                        </div>

                        {showAddProvider && (
                            <div className="p-4 bg-white dark:bg-[#2e2e2e] rounded-xl space-y-2">
                                <select className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200" value="" onChange={(e) => {
                                    const preset = PRESETS[Number(e.target.value)];
                                    if (preset) setNewProvider({ name: preset.name, type: preset.type, baseUrl: preset.baseUrl, apiKey: '', model: preset.model, models: preset.models || [] });
                                }}>
                                    <option value="" disabled>选择 AI 提供商...</option>
                                    {PRESETS.map((p, i) => <option key={i} value={i}>{p.label}</option>)}
                                </select>
                                {newProvider.name && (
                                    <>
                                        <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder={t('settings.ai.name', '名称')} value={newProvider.name} onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })} />
                                        <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="Base URL" value={newProvider.baseUrl} onChange={(e) => setNewProvider({ ...newProvider, baseUrl: e.target.value })} />
                                        <div className="relative">
                                            <input className="w-full px-2 py-1.5 pr-8 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="API Key" type={showKeys['new'] ? 'text' : 'password'} value={newProvider.apiKey} onChange={(e) => setNewProvider({ ...newProvider, apiKey: e.target.value })} />
                                            <button className="absolute right-2 top-1/2 -translate-y-1/2" onClick={() => toggleKey('new')}>
                                                {showKeys['new'] ? <EyeOff size={14} className="text-gray-400" /> : <Eye size={14} className="text-gray-400" />}
                                            </button>
                                        </div>
                                        <button className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md flex-shrink-0" onClick={handleAddProvider} disabled={!newProvider.name || !newProvider.baseUrl}>{t('settings.ai.confirm_add', '保存')}</button>
                                    </>
                                )}
                            </div>
                        )}

                        {aiProviders.map((provider) => (
                            <div key={provider.id} className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] space-y-2">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                        <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{provider.name}</span>
                                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-400">{provider.type}</span>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        <div
                                            className={`relative w-10 h-5 rounded-full cursor-pointer transition-colors ${provider.enabled ? 'bg-[#07c160]' : 'bg-gray-300'}`}
                                            onClick={() => updateAIProvider(provider.id, { enabled: !provider.enabled })}
                                        >
                                            <div className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${provider.enabled ? 'translate-x-5' : 'translate-x-0.5'}`} />
                                        </div>
                                        <button className="text-red-400 hover:text-red-500" onClick={() => removeAIProvider(provider.id)}><Trash2 size={14} /></button>
                                    </div>
                                </div>
                                <div className="space-y-1.5">
                                    <input className="w-full px-2 py-1 text-xs bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded border-0 outline-none text-gray-600 dark:text-gray-300" value={provider.baseUrl || ''} onChange={(e) => updateAIProvider(provider.id, { baseUrl: e.target.value })} placeholder="Base URL" />
                                    <div className="flex gap-1">
                                        <input className="flex-1 px-2 py-1 text-xs bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded border-0 outline-none text-gray-600 dark:text-gray-300" type={showKeys[provider.id] ? 'text' : 'password'} value={provider.apiKey || ''} onChange={(e) => updateAIProvider(provider.id, { apiKey: e.target.value })} placeholder="API Key" />
                                        <button onClick={() => toggleKey(provider.id)} className="text-gray-400 hover:text-gray-600">
                                            {showKeys[provider.id] ? <EyeOff size={12} /> : <Eye size={12} />}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        ))}
                        {aiProviders.length === 0 && !showAddProvider && (
                            <p className="text-sm text-gray-400 text-center py-6">暂无 AI 提供商，点击"+ 添加"配置</p>
                        )}
                    </div>
                );

            case 'workspace':
                return (
                    <div className="space-y-4">
                        <div className="flex items-center justify-between">
                            <div>
                                <h3 className="text-sm font-bold text-gray-800 dark:text-white">工作空间</h3>
                                <p className="text-[10px] text-gray-400 mt-0.5">{wsDir}</p>
                            </div>
                            <button className="p-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/5 text-gray-400" onClick={loadWsFiles}><RefreshCw size={14} /></button>
                        </div>
                        <div className="flex gap-3" style={{ minHeight: 320 }}>
                            <div className="w-36 shrink-0 space-y-0.5">
                                {wsFiles.map(f => (
                                    <button
                                        key={f.name}
                                        className={`w-full text-left px-2.5 py-1.5 rounded-lg text-xs transition-colors ${wsSelectedFile === f.name ? 'bg-[#07c160]/10 text-[#07c160] font-medium' : 'text-gray-500 hover:bg-black/5 dark:hover:bg-white/5'}`}
                                        onClick={() => wsSelectFile(f.name)}
                                    >
                                        {f.name}
                                    </button>
                                ))}
                                {wsFiles.length === 0 && <p className="text-[10px] text-gray-400 p-2">无文件</p>}
                            </div>
                            <div className="flex-1 flex flex-col">
                                {wsSelectedFile ? (
                                    <>
                                        <div className="flex items-center justify-between mb-2">
                                            <span className="text-xs font-medium text-gray-600 dark:text-gray-300">{wsSelectedFile}</span>
                                            <button
                                                className="flex items-center gap-1 px-2 py-1 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md disabled:opacity-40"
                                                onClick={wsSaveFile}
                                                disabled={wsSaving || wsContent === wsOrigContent}
                                            >
                                                <Save size={12} />{wsSaving ? '...' : '保存'}
                                            </button>
                                        </div>
                                        <textarea
                                            className="flex-1 w-full p-3 text-xs font-mono leading-relaxed bg-white dark:bg-[#2e2e2e] rounded-lg border-0 outline-none resize-none text-gray-700 dark:text-gray-200"
                                            value={wsContent}
                                            onChange={e => setWsContent(e.target.value)}
                                            style={{ minHeight: 280 }}
                                        />
                                    </>
                                ) : (
                                    <div className="flex items-center justify-center h-full text-xs text-gray-400">
                                        ← 选择文件编辑
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                );

            case 'environments':
                return (
                    <div className="space-y-4">
                        <div className="flex items-center justify-between">
                            <h3 className="text-sm font-bold text-gray-800 dark:text-white">{t('settings.environments.title', '环境变量')}</h3>
                            <button className="text-xs text-[#07c160] hover:underline" onClick={() => setEnvShowAdd(!envShowAdd)}>
                                {envShowAdd ? t('settings.environments.cancel', '取消') : '+ ' + t('settings.environments.add', '添加')}
                            </button>
                        </div>

                        {envShowAdd && (
                            <div className="p-4 bg-white dark:bg-[#2e2e2e] rounded-xl space-y-2">
                                <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none font-mono" placeholder="KEY" value={envNew.key} onChange={e => setEnvNew({ ...envNew, key: e.target.value })} />
                                <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none font-mono" placeholder="VALUE" type={envNew.secret ? 'password' : 'text'} value={envNew.value} onChange={e => setEnvNew({ ...envNew, value: e.target.value })} />
                                <label className="flex items-center gap-2 text-xs text-gray-500">
                                    <input type="checkbox" checked={envNew.secret} onChange={e => setEnvNew({ ...envNew, secret: e.target.checked })} className="rounded" />
                                    {t('settings.environments.secret_label', '密钥 (界面脱敏)')}
                                </label>
                                <button className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md" onClick={envAdd} disabled={!envNew.key}>{t('settings.environments.save', '保存')}</button>
                            </div>
                        )}

                        <div className="space-y-1.5">
                            {envVars.map(env => (
                                <div key={env.key} className="flex items-center gap-2 p-3 rounded-xl bg-white dark:bg-[#2e2e2e]">
                                    <div className="flex-1 min-w-0">
                                        <span className="font-mono text-xs font-medium text-gray-800 dark:text-gray-200">{env.key}</span>
                                        <span className="mx-1.5 text-gray-300">=</span>
                                        <span className="font-mono text-xs text-gray-500">
                                            {env.secret && !envShowKeys[env.key] ? '••••••••' : env.value}
                                        </span>
                                    </div>
                                    <div className="flex items-center gap-1 shrink-0">
                                        {env.secret && (
                                            <button className="text-gray-400 hover:text-gray-600" onClick={() => setEnvShowKeys(p => ({ ...p, [env.key]: !p[env.key] }))}>
                                                {envShowKeys[env.key] ? <EyeOff size={12} /> : <Eye size={12} />}
                                            </button>
                                        )}
                                        <button className="text-red-400 hover:text-red-500" onClick={() => envDelete(env.key)}>
                                            <Trash2 size={12} />
                                        </button>
                                    </div>
                                </div>
                            ))}
                            {envVars.length === 0 && !envShowAdd && (
                                <p className="text-sm text-gray-400 text-center py-6">{t('settings.environments.empty', '暂无环境变量')}</p>
                            )}
                        </div>
                    </div>
                );

            case 'notifications':
                return (
                    <div className="space-y-4">
                        <div className="flex items-center justify-between">
                            <h3 className="text-sm font-bold text-gray-800 dark:text-white">{t('settings.notifications.title', '消息通知设置')}</h3>
                            <button className="text-xs text-[#07c160] hover:underline" onClick={() => setNotifShowAdd(!notifShowAdd)}>
                                {notifShowAdd ? t('settings.notifications.cancel', '取消') : '+ ' + t('settings.notifications.add', '添加渠道')}
                            </button>
                        </div>

                        {notifShowAdd && (
                            <div className="p-4 bg-white dark:bg-[#2e2e2e] rounded-xl space-y-2">
                                <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="名称 (如 运维二群)" value={notifNew.name} onChange={e => setNotifNew({ ...notifNew, name: e.target.value })} />
                                <select className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200" value={notifNew.type} onChange={e => setNotifNew({ ...notifNew, type: e.target.value as any })}>
                                    <option value="feishu">飞书</option>
                                    <option value="dingtalk">钉钉</option>
                                    <option value="wecom">企业微信</option>
                                </select>
                                <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="Webhook URL" value={notifNew.webhookUrl} onChange={e => setNotifNew({ ...notifNew, webhookUrl: e.target.value })} />
                                <button className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md" onClick={notifAdd} disabled={!notifNew.name || !notifNew.webhookUrl}>
                                    保存
                                </button>
                            </div>
                        )}

                        <div className="space-y-2">
                            {notificationChannels.map(channel => (
                                <div key={channel.id} className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] space-y-2">
                                    <div className="flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{channel.name}</span>
                                            <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-50 dark:bg-blue-900/30 text-blue-500">
                                                {channel.type === 'feishu' ? '飞书' : channel.type === 'dingtalk' ? '钉钉' : '企业微信'}
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <div
                                                className={`relative w-10 h-5 rounded-full cursor-pointer transition-colors ${channel.enabled ? 'bg-[#07c160]' : 'bg-gray-300'}`}
                                                onClick={() => updateNotificationChannel(channel.id, { enabled: !channel.enabled })}
                                            >
                                                <div className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${channel.enabled ? 'translate-x-5' : 'translate-x-0.5'}`} />
                                            </div>
                                            <button className="text-red-400 hover:text-red-500" onClick={() => removeNotificationChannel(channel.id)}>
                                                <Trash2 size={12} />
                                            </button>
                                        </div>
                                    </div>
                                    <p className="text-[10px] text-gray-400 break-all">{channel.webhookUrl}</p>
                                </div>
                            ))}
                        </div>
                        {notificationChannels.length === 0 && !notifShowAdd && (
                            <p className="text-sm text-gray-400 text-center py-6">暂无配置的消息通知渠道</p>
                        )}
                    </div>
                );

            case 'about':
                return (
                    <div className="space-y-4">
                        <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-4">{t('settings.about.title', '关于')}</h3>
                        <div className="p-5 rounded-xl bg-white dark:bg-[#2e2e2e] space-y-3 text-sm text-gray-600 dark:text-gray-300">
                            <div className="flex items-center gap-3 mb-2">
                                <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-[#07c160] to-[#05a050] flex items-center justify-center">
                                    <Sparkles size={20} className="text-white" />
                                </div>
                                <div>
                                    <p className="font-semibold text-gray-800 dark:text-white">Helix</p>
                                    <p className="text-xs text-gray-400">{t('settings.about.desc', 'AI 驱动的智能体平台')}</p>
                                </div>
                            </div>
                            <div className="border-t border-black/5 dark:border-white/5 pt-3 space-y-1.5 text-xs">
                                <div className="flex justify-between"><span className="text-gray-400">{t('settings.about.version', '版本')}</span><span>0.3.0</span></div>
                                <div className="flex justify-between"><span className="text-gray-400">{t('settings.about.stack', '技术栈')}</span><span>Tauri + React + Rust</span></div>
                            </div>
                        </div>
                    </div>
                );
        }
    };


    return (
        <div className="h-screen flex bg-[#FAFBFC] dark:bg-base-300">
            <ToastContainer />

            {/* Icon Sidebar */}
            <div
                className="w-[76px] shrink-0 bg-[#e9e9e9] dark:bg-[#2e2e2e] flex flex-col items-center pb-4 gap-1 border-r border-black/5 dark:border-white/5"
                style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
                data-tauri-drag-region
            >
                {/* Traffic light spacer: WebkitAppRegion no-drag carves this zone OUT
                    of the parent drag region so native macOS buttons remain clickable */}
                <div className="w-full h-[52px] shrink-0" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties} />

                <div
                    className="w-9 h-9 rounded-lg bg-white dark:bg-[#404040] flex items-center justify-center mb-4 cursor-pointer shadow-sm"
                    style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                >
                    <Sparkles size={16} className="text-[#07c160]" />
                </div>

                {navItems.map((item) => {
                    const Icon = item.icon;
                    return (
                        <button
                            key={item.path}
                            onClick={() => navigate(item.path)}
                            className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${item.active
                                ? 'text-[#07c160]'
                                : 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/5'
                                }`}
                            title={item.label}
                            style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                        >
                            <Icon size={20} />
                        </button>
                    );
                })}

                <div className="flex-1" />

                <button
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                    onClick={toggleTheme}
                    title={isDark ? 'Light' : 'Dark'}
                    style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                >
                    {isDark ? <Sun size={18} /> : <Moon size={18} />}
                </button>

                <div className="relative" ref={moreMenuRef}>
                    <button
                        className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${showMoreMenu
                            ? 'text-[#07c160]'
                            : 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/5'
                            }`}
                        onClick={() => setShowMoreMenu(!showMoreMenu)}
                        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                    >
                        <Menu size={18} />
                    </button>

                    {showMoreMenu && (
                        <div
                            className="absolute bottom-0 left-16 w-[180px] bg-white dark:bg-[#353535] rounded-lg shadow-xl border border-black/5 dark:border-white/10 py-1 z-50"
                            style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                        >
                            <button
                                onClick={() => { setShowSettings(true); setShowMoreMenu(false); }}
                                className="w-full px-4 py-2.5 text-left text-sm text-gray-700 dark:text-gray-200 hover:bg-[#f5f5f5] dark:hover:bg-[#404040] flex items-center gap-3"
                            >
                                <SettingsIcon size={16} className="text-gray-400" />{t('nav.settings', '设置')}
                            </button>
                        </div>
                    )}
                </div>
            </div>

            {/* Main content */}
            <main className="flex-1 overflow-hidden flex">
                <Outlet />
            </main>


            {/* Settings Modal */}
            {showSettings && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-[#f5f5f5] dark:bg-[#1e1e1e] rounded-xl shadow-2xl w-[860px] h-[600px] flex overflow-hidden">
                        {/* Settings sidebar */}
                        <div className="w-[170px] shrink-0 bg-[#f0f0f0] dark:bg-[#252525] rounded-l-xl pt-4 pb-4 px-2 overflow-y-auto">
                            <div className="flex items-center justify-between px-2 mb-5 mt-2">
                                <div className="flex items-center gap-1.5 cursor-pointer group" onClick={() => setShowSettings(false)}>
                                    <div className="w-3 h-3 rounded-full bg-[#ff5f56] flex items-center justify-center">
                                        <X size={8} className="text-black/50 opacity-0 group-hover:opacity-100" />
                                    </div>
                                    <div className="w-3 h-3 rounded-full bg-[#ffbd2e] flex items-center justify-center">
                                    </div>
                                    <div className="w-3 h-3 rounded-full bg-[#27c93f] flex items-center justify-center">
                                    </div>
                                </div>
                                <span className="text-xs font-medium text-gray-400">{t('settings.title', '设置')}</span>
                                <div className="w-[50px]" />
                            </div>
                            {Object.entries(menuGroups).map(([groupLabel, items]) => (
                                <div key={groupLabel} className="mb-2">
                                    <div className="text-[9px] font-semibold text-gray-400/60 uppercase tracking-wider px-3 mb-1">{groupLabel}</div>
                                    {items.map((item) => {
                                        const Icon = item.icon;
                                        return (
                                            <button
                                                key={item.key}
                                                onClick={() => setSettingsSection(item.key)}
                                                className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition-colors mb-0.5 ${settingsSection === item.key
                                                    ? 'bg-white dark:bg-[#383838] text-gray-800 dark:text-white font-medium'
                                                    : 'text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/5'
                                                    }`}
                                            >
                                                <Icon size={16} />{item.label}
                                            </button>
                                        );
                                    })}
                                </div>
                            ))}
                        </div>

                        {/* Settings content */}
                        <div className="flex-1 overflow-y-auto p-6">
                            {renderSettingsContent()}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}

export default Layout;
