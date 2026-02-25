import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, Bot, Eye, EyeOff, Globe, Moon, Palette, Settings as SettingsIcon, Sun, Trash2 } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useDevOpsStore, AIProvider } from '../stores/useDevOpsStore';
import { useConfigStore } from '../stores/useConfigStore';

type SettingsSection = 'appearance' | 'ai' | 'about';

const MENU_ITEMS: Array<{ key: SettingsSection; icon: typeof Palette; label: string; group: string }> = [
    { key: 'appearance', icon: Palette, label: '外观设置', group: '通用' },
    { key: 'ai', icon: Bot, label: 'AI 提供商', group: '通用' },
    { key: 'about', icon: Globe, label: '关于', group: '其他' },
];



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

    const toggleKey = (id: string) => setShowKeys((p) => ({ ...p, [id]: !p[id] }));


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
