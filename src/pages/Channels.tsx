import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Terminal, Send, MessageSquare, Radio, ChevronRight, X, Save } from 'lucide-react';
import { useDevOpsStore, BotChannel } from '../stores/useDevOpsStore';

const BOT_TYPES = [
    { id: 'console', name: 'Console', icon: Terminal, color: 'bg-gray-800', desc: '本地终端测试通道' },
    { id: 'feishu', name: '飞书 (Feishu)', icon: MessageSquare, color: 'bg-blue-500', desc: '企业内部通讯与协作' },
    { id: 'dingtalk', name: '钉钉 (DingTalk)', icon: MessageSquare, color: 'bg-blue-600', desc: '企业级即时通讯平台' },
    { id: 'wecom', name: '企业微信 (WeCom)', icon: MessageSquare, color: 'bg-green-600', desc: '全路协同的办公工具' },
    { id: 'telegram', name: 'Telegram', icon: Send, color: 'bg-sky-500', desc: 'Secure cloud-based messaging' },
    { id: 'discord', name: 'Discord', icon: Radio, color: 'bg-indigo-500', desc: 'Chat for Communities and Friends' },
] as const;

export default function Channels() {
    const { t } = useTranslation();
    const { botChannels, addBotChannel, updateBotChannel, removeBotChannel } = useDevOpsStore();

    // State for the drawer
    const [drawerOpen, setDrawerOpen] = useState(false);

    // Form state for editing
    const [editingChannelId, setEditingChannelId] = useState<string | null>(null);
    const [formData, setFormData] = useState<Partial<BotChannel>>({});

    const handleOpenCard = (type: string, existingChannel?: BotChannel) => {
        if (existingChannel) {
            setEditingChannelId(existingChannel.id);
            setFormData(existingChannel);
        } else {
            setEditingChannelId(null);
            const typeInfo = BOT_TYPES.find(b => b.id === type);
            setFormData({
                name: typeInfo?.name || '',
                type: type as BotChannel['type'],
                enabled: true,
                botPrefix: '@bot',
                config: {}
            });
        }
        setDrawerOpen(true);
    };

    const handleSave = () => {
        if (!formData.name || !formData.type) return;

        if (editingChannelId) {
            updateBotChannel(editingChannelId, formData);
        } else {
            addBotChannel(formData as Omit<BotChannel, 'id'>);
        }
        setDrawerOpen(false);
    };

    const handleDelete = () => {
        if (editingChannelId) {
            removeBotChannel(editingChannelId);
            setDrawerOpen(false);
        }
    }

    return (
        <div className="flex-1 flex flex-col h-screen overflow-hidden bg-[#FAFBFC] dark:bg-base-300 relative">
            <div className="h-14 shrink-0 flex items-center justify-between px-6 border-b border-black/5 dark:border-white/5 bg-white/50 dark:bg-[#2e2e2e]/50 backdrop-blur-md" style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}>
                <div className="flex items-center gap-2">
                    <Radio size={18} className="text-[#07c160]" />
                    <span className="font-semibold text-sm text-gray-800 dark:text-gray-200">
                        {t('channels.title', '通道集成 (Channels)')}
                    </span>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-6" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
                <div className="max-w-5xl mx-auto">
                    <div className="mb-6">
                        <h2 className="text-xl font-bold text-gray-800 dark:text-white mb-2">{t('channels.header', '连接机器人生态')}</h2>
                        <p className="text-sm text-gray-500 dark:text-gray-400">
                            {t('channels.description', '配置各种即时通讯平台的 Bot，使 Agent 可以在多平台接发消息并执行任务。')}
                        </p>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        {BOT_TYPES.map(botType => {
                            const Icon = botType.icon;
                            const existingChannel = botChannels.find(c => c.type === botType.id);
                            const isActive = existingChannel && existingChannel.enabled;

                            return (
                                <div
                                    key={botType.id}
                                    onClick={() => handleOpenCard(botType.id, existingChannel)}
                                    className="group relative bg-white dark:bg-[#353535] rounded-xl p-5 border border-black/5 dark:border-white/5 shadow-sm hover:shadow-md transition-all cursor-pointer flex flex-col h-40"
                                >
                                    <div className="flex justify-between items-start mb-4">
                                        <div className={`w-12 h-12 rounded-2xl flex items-center justify-center text-white shadow-sm ${botType.color}`}>
                                            <Icon size={24} />
                                        </div>
                                        {existingChannel ? (
                                            <div className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-medium ${isActive ? 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-400' : 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'}`}>
                                                <div className={`w-1.5 h-1.5 rounded-full ${isActive ? 'bg-green-500' : 'bg-gray-400'}`} />
                                                {isActive ? t('channels.status_active', '已连接') : t('channels.status_disabled', '已禁用')}
                                            </div>
                                        ) : (
                                            <div className="flex items-center justify-center w-8 h-8 rounded-full bg-gray-50 dark:bg-gray-800 text-gray-400 opacity-0 group-hover:opacity-100 transition-opacity">
                                                <ChevronRight size={16} />
                                            </div>
                                        )}
                                    </div>
                                    <div className="mt-auto">
                                        <h3 className="text-base font-bold text-gray-800 dark:text-white mb-1 group-hover:text-[#07c160] transition-colors">{botType.name}</h3>
                                        <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-1">{botType.desc}</p>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </div>
            </div>

            {/* Config Drawer Placeholder - Will elaborate config later */}
            {drawerOpen && (
                <div className="absolute top-0 right-0 w-[400px] h-full bg-white dark:bg-[#2e2e2e] shadow-2xl border-l border-black/5 dark:border-white/10 flex flex-col z-50 transform transition-transform" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
                    <div className="flex items-center justify-between p-4 border-b border-black/5 dark:border-white/5">
                        <h3 className="font-bold text-gray-800 dark:text-white">{formData.name || '配置通道'}</h3>
                        <button onClick={() => setDrawerOpen(false)} className="p-1.5 rounded-lg text-gray-500 hover:bg-black/5 dark:hover:bg-white/5"><X size={16} /></button>
                    </div>

                    <div className="flex-1 overflow-y-auto p-6 space-y-5">
                        <div className="space-y-1.5">
                            <label className="text-xs font-semibold text-gray-600 dark:text-gray-300">显示名称</label>
                            <input
                                value={formData.name || ''}
                                onChange={e => setFormData({ ...formData, name: e.target.value })}
                                className="w-full px-3 py-2 text-sm bg-gray-50 dark:bg-[#353535] rounded-lg border-0 outline-none focus:ring-1 focus:ring-[#07c160] transition-shadow text-gray-800 dark:text-gray-200"
                            />
                        </div>

                        <div className="space-y-1.5">
                            <label className="text-xs font-semibold text-gray-600 dark:text-gray-300">触发前缀 (唤醒词)</label>
                            <input
                                value={formData.botPrefix || ''}
                                onChange={e => setFormData({ ...formData, botPrefix: e.target.value })}
                                placeholder="@bot"
                                className="w-full px-3 py-2 font-mono text-sm bg-gray-50 dark:bg-[#353535] rounded-lg border-0 outline-none focus:ring-1 focus:ring-[#07c160] transition-shadow text-gray-800 dark:text-gray-200"
                            />
                            <p className="text-[10px] text-gray-400">只有以此前缀开头的消息才会被处理</p>
                        </div>

                        <div className="space-y-1.5">
                            <label className="text-xs font-semibold text-gray-600 dark:text-gray-300">App ID / Token</label>
                            <input
                                value={formData.config?.appId || ''}
                                onChange={e => setFormData({ ...formData, config: { ...formData.config, appId: e.target.value } })}
                                className="w-full px-3 py-2 text-sm bg-gray-50 dark:bg-[#353535] rounded-lg border-0 outline-none focus:ring-1 focus:ring-[#07c160] transition-shadow text-gray-800 dark:text-gray-200"
                                placeholder="App ID 或 Bot Token"
                            />
                        </div>

                        <div className="space-y-1.5">
                            <label className="text-xs font-semibold text-gray-600 dark:text-gray-300">App Secret</label>
                            <input
                                type="password"
                                value={formData.config?.appSecret || ''}
                                onChange={e => setFormData({ ...formData, config: { ...formData.config, appSecret: e.target.value } })}
                                className="w-full px-3 py-2 text-sm bg-gray-50 dark:bg-[#353535] rounded-lg border-0 outline-none focus:ring-1 focus:ring-[#07c160] transition-shadow text-gray-800 dark:text-gray-200"
                                placeholder="App Secret 或 Webhook Key"
                            />
                        </div>

                        {editingChannelId && (
                            <div className="pt-4 border-t border-black/5 dark:border-white/5 flex items-center justify-between">
                                <label className="text-sm font-medium text-gray-700 dark:text-gray-300">启用该通道</label>
                                <div
                                    className={`relative w-11 h-6 rounded-full cursor-pointer transition-colors ${formData.enabled ? 'bg-[#07c160]' : 'bg-gray-300 dark:bg-gray-600'}`}
                                    onClick={() => setFormData({ ...formData, enabled: !formData.enabled })}
                                >
                                    <div className={`absolute top-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform ${formData.enabled ? 'translate-x-5.5' : 'translate-x-0.5'}`} />
                                </div>
                            </div>
                        )}
                    </div>

                    <div className="p-4 border-t border-black/5 dark:border-white/5 flex gap-2">
                        <button onClick={handleSave} className="flex-1 flex items-center justify-center gap-1.5 py-2 bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg text-sm font-medium transition-colors">
                            <Save size={16} />
                            保存配置
                        </button>
                        {editingChannelId && (
                            <button onClick={handleDelete} className="px-4 py-2 bg-red-50 hover:bg-red-100 dark:bg-red-900/20 dark:hover:bg-red-900/30 text-red-600 dark:text-red-400 rounded-lg text-sm font-medium transition-colors">
                                删除
                            </button>
                        )}
                    </div>
                </div>
            )}

            {/* Backdrop for drawer */}
            {drawerOpen && (
                <div
                    className="absolute inset-0 bg-black/10 dark:bg-black/30 backdrop-blur-sm z-40"
                    onClick={() => setDrawerOpen(false)}
                    style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                />
            )}
        </div>
    );
}
