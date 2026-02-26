import { useState, useRef, useEffect } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import ToastContainer from '../common/ToastContainer';
import { useConfigStore } from '../../stores/useConfigStore';
import { useDevOpsStore, AIProvider } from '../../stores/useDevOpsStore';
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
    Info,
} from 'lucide-react';

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
    const [settingsSection, setSettingsSection] = useState<'appearance' | 'ai' | 'about'>('appearance');
    const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
    const [newProvider, setNewProvider] = useState({ name: '', type: 'openai' as AIProvider['type'], baseUrl: '', apiKey: '', model: '' });
    const [showAddProvider, setShowAddProvider] = useState(false);

    const toggleKey = (id: string) => setShowKeys((p) => ({ ...p, [id]: !p[id] }));

    const toggleTheme = () => {
        if (!config) return;
        const newTheme = config.theme === 'light' ? 'dark' : 'light';
        saveConfig({ ...config, theme: newTheme, language: config.language }, true);
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
        addAIProvider({ name: newProvider.name, type: newProvider.type, baseUrl: newProvider.baseUrl, apiKey: newProvider.apiKey || undefined, models: [], enabled: !!newProvider.apiKey, defaultModel: newProvider.model || undefined });
        setNewProvider({ name: '', type: 'openai', baseUrl: '', apiKey: '', model: '' });
        setShowAddProvider(false);
    };

    const navItems = [
        { path: '/', icon: MessageSquare, label: t('nav.channels', '对话'), active: location.pathname === '/' },
        { path: '/skills', icon: Blocks, label: t('nav.skills', '技能'), active: location.pathname === '/skills' },
        { path: '/cron-jobs', icon: Clock, label: t('nav.cron_jobs', '定时任务'), active: location.pathname === '/cron-jobs' },
        { path: '/logs', icon: Activity, label: t('nav.logs', '日志'), active: location.pathname === '/logs' },
    ];

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
        <div className="h-screen flex bg-[#FAFBFC] dark:bg-base-300">
            <ToastContainer />

            {/* Icon Sidebar */}
            <div className="w-16 shrink-0 bg-[#e9e9e9] dark:bg-[#2e2e2e] flex flex-col items-center pb-4 gap-1 border-r border-black/5 dark:border-white/5">
                {/* Draggable top area for macOS traffic lights */}
                <div className="w-full h-12 shrink-0" style={{ WebkitAppRegion: 'drag' } as React.CSSProperties} data-tauri-drag-region />
                <div className="w-9 h-9 rounded-lg bg-white dark:bg-[#404040] flex items-center justify-center mb-4 cursor-pointer shadow-sm">
                    <Sparkles size={16} className="text-[#07c160]" />
                </div>

                {navItems.map((item) => {
                    const Icon = item.icon;
                    return (
                        <button
                            key={item.path}
                            onClick={() => navigate(item.path)}
                            className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${item.active
                                ? 'text-[#07c160] bg-black/5 dark:bg-white/10'
                                : 'text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10'
                                }`}
                            title={item.label}
                        >
                            <Icon size={20} />
                        </button>
                    );
                })}

                <div className="flex-1" />

                {/* Theme toggle */}
                <button
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    onClick={toggleTheme}
                    title={isDark ? 'Light' : 'Dark'}
                >
                    {isDark ? <Sun size={18} /> : <Moon size={18} />}
                </button>

                {/* More menu (≡) */}
                <div className="relative" ref={moreMenuRef}>
                    <button
                        className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${showMoreMenu
                            ? 'text-[#07c160] bg-black/5 dark:bg-white/10'
                            : 'text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10'
                            }`}
                        onClick={() => setShowMoreMenu(!showMoreMenu)}
                    >
                        <Menu size={18} />
                    </button>

                    {/* Popup menu */}
                    {showMoreMenu && (
                        <div className="absolute bottom-0 left-16 w-[180px] bg-white dark:bg-[#353535] rounded-lg shadow-xl border border-black/5 dark:border-white/10 py-1 z-50">
                            <button
                                onClick={() => { setShowSettings(true); setShowMoreMenu(false); }}
                                className="w-full px-4 py-2.5 text-left text-sm text-gray-700 dark:text-gray-200 hover:bg-[#f5f5f5] dark:hover:bg-[#404040] flex items-center gap-3"
                            >
                                <SettingsIcon size={16} className="text-gray-400" />设置
                            </button>
                            <button
                                onClick={() => { navigate('/logs'); setShowMoreMenu(false); }}
                                className="w-full px-4 py-2.5 text-left text-sm text-gray-700 dark:text-gray-200 hover:bg-[#f5f5f5] dark:hover:bg-[#404040] flex items-center gap-3"
                            >
                                <Activity size={16} className="text-gray-400" />日志
                            </button>
                            <div className="border-t border-black/5 dark:border-white/5 my-1" />
                            <button
                                className="w-full px-4 py-2.5 text-left text-sm text-gray-700 dark:text-gray-200 hover:bg-[#f5f5f5] dark:hover:bg-[#404040] flex items-center gap-3"
                            >
                                <Info size={16} className="text-gray-400" />关于 Helix
                            </button>
                        </div>
                    )}
                </div>
            </div>

            {/* Main content */}
            <main className="flex-1 overflow-hidden flex">
                <Outlet />
            </main>

            {/* Settings Modal Overlay */}
            {showSettings && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-[#f5f5f5] dark:bg-[#1e1e1e] rounded-xl shadow-2xl w-[640px] h-[480px] flex overflow-hidden">
                        {/* Settings sidebar */}
                        <div className="w-[160px] shrink-0 bg-[#f0f0f0] dark:bg-[#252525] rounded-l-xl py-4 px-2">
                            <div className="flex items-center justify-between px-2 mb-3">
                                <span className="text-xs font-medium text-gray-400">设置</span>
                                <button onClick={() => setShowSettings(false)} className="p-0.5 rounded hover:bg-black/10 dark:hover:bg-white/10">
                                    <X size={14} className="text-gray-400" />
                                </button>
                            </div>
                            {[
                                { key: 'appearance' as const, icon: Palette, label: '外观' },
                                { key: 'ai' as const, icon: Bot, label: 'AI 提供商' },
                                { key: 'about' as const, icon: Globe, label: '关于' },
                            ].map((item) => {
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

                        {/* Settings content */}
                        <div className="flex-1 overflow-y-auto p-6">
                            {settingsSection === 'appearance' && (
                                <div className="space-y-4">
                                    <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-4">外观设置</h3>
                                    <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e]">
                                        <div className="flex items-center justify-between mb-4">
                                            <div><p className="text-sm font-medium text-gray-800 dark:text-gray-200">主题</p><p className="text-xs text-gray-400">切换明暗主题</p></div>
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
                            )}

                            {settingsSection === 'ai' && (
                                <div className="space-y-4">
                                    <div className="flex items-center justify-between mb-1">
                                        <h3 className="text-sm font-bold text-gray-800 dark:text-white">AI 提供商</h3>
                                        <button className="text-xs text-[#07c160] hover:underline" onClick={() => setShowAddProvider(!showAddProvider)}>
                                            {showAddProvider ? '取消' : '+ 添加'}
                                        </button>
                                    </div>

                                    {showAddProvider && (
                                        <div className="p-4 bg-white dark:bg-[#2e2e2e] rounded-xl space-y-2">
                                            <select className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200" value="" onChange={(e) => {
                                                const preset = PRESETS[Number(e.target.value)];
                                                if (preset) setNewProvider({ name: preset.name, type: preset.type, baseUrl: preset.baseUrl, apiKey: '', model: preset.model });
                                            }}>
                                                <option value="" disabled>选择 AI 提供商...</option>
                                                {PRESETS.map((p, i) => <option key={i} value={i}>{p.label}</option>)}
                                            </select>
                                            {newProvider.name && (
                                                <>
                                                    <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="名称" value={newProvider.name} onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })} />
                                                    <input className="w-full px-2 py-1.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="Base URL" value={newProvider.baseUrl} onChange={(e) => setNewProvider({ ...newProvider, baseUrl: e.target.value })} />
                                                    <div className="relative">
                                                        <input className="w-full px-2 py-1.5 pr-8 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none" placeholder="API Key" type={showKeys['new'] ? 'text' : 'password'} value={newProvider.apiKey} onChange={(e) => setNewProvider({ ...newProvider, apiKey: e.target.value })} />
                                                        <button className="absolute right-2 top-1/2 -translate-y-1/2" onClick={() => toggleKey('new')}>
                                                            {showKeys['new'] ? <EyeOff size={14} className="text-gray-400" /> : <Eye size={14} className="text-gray-400" />}
                                                        </button>
                                                    </div>
                                                    <button className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md" onClick={handleAddProvider}>保存</button>
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
                            )}

                            {settingsSection === 'about' && (
                                <div className="space-y-4">
                                    <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-4">关于</h3>
                                    <div className="p-5 rounded-xl bg-white dark:bg-[#2e2e2e] space-y-2 text-sm text-gray-600 dark:text-gray-300">
                                        <p><strong className="text-gray-800 dark:text-white">Helix</strong> — AI 驱动的智能体平台</p>
                                        <p>版本: 1.0.0</p>
                                        <p>基于 Tauri + React 构建</p>
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}

export default Layout;
