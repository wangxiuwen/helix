import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    Settings2,
    Wifi,
    WifiOff,
    Send,
    Loader2,
    CheckCircle2,
    AlertCircle,
    Eye,
    EyeOff,
    TestTube,
    Power,
    PowerOff,
} from 'lucide-react';

interface FeishuStatus {
    connected: boolean;
    configured: boolean;
    enabled: boolean;
    app_id: string;
    bot_name: string;
    token_valid: boolean;
}

interface FeishuConfig {
    app_id: string;
    bot_name: string;
    enabled: boolean;
    connected: boolean;
    has_secret: boolean;
}

export default function Feishu() {
    // Config state
    const [appId, setAppId] = useState('');
    const [appSecret, setAppSecret] = useState('');
    const [botName, setBotName] = useState('Helix');
    const [enabled, setEnabled] = useState(false);
    const [showSecret, setShowSecret] = useState(false);

    // Connection state
    const [status, setStatus] = useState<FeishuStatus | null>(null);
    const [loading, setLoading] = useState(false);

    // Message state
    const [chatId, setChatId] = useState('');
    const [msgInput, setMsgInput] = useState('');
    const [sending, setSending] = useState(false);

    // Feedback
    const [toast, setToast] = useState('');
    const [error, setError] = useState('');

    const statusPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

    // Auto-clear toast/error
    useEffect(() => {
        if (toast) { const t = setTimeout(() => setToast(''), 3000); return () => clearTimeout(t); }
    }, [toast]);
    useEffect(() => {
        if (error) { const t = setTimeout(() => setError(''), 6000); return () => clearTimeout(t); }
    }, [error]);

    // Load config on mount
    useEffect(() => {
        loadConfig();
        loadStatus();
        // Poll status
        statusPollRef.current = setInterval(loadStatus, 5000);
        return () => {
            if (statusPollRef.current) clearInterval(statusPollRef.current);
        };
    }, []);

    const loadConfig = async () => {
        try {
            const cfg = await invoke<FeishuConfig>('feishu_get_config');
            setAppId(cfg.app_id || '');
            setBotName(cfg.bot_name || 'Helix');
            setEnabled(cfg.enabled);
            // Load actual secret from backend
            if ((cfg as any).app_secret) {
                setAppSecret((cfg as any).app_secret);
            }
        } catch (e) {
            console.error('Failed to load feishu config', e);
        }
    };

    const loadStatus = async () => {
        try {
            const s = await invoke<FeishuStatus>('feishu_get_status');
            setStatus(s);
        } catch (e) {
            console.error('Failed to load feishu status', e);
        }
    };

    const saveConfig = async () => {
        setLoading(true);
        try {
            await invoke('feishu_save_config', {
                appId: appId,
                appSecret: appSecret,
                botName: botName,
                enabled: enabled,
            });
            setToast('✅ 配置已保存');
            loadStatus();
        } catch (e: any) {
            setError(`保存失败: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const testConnection = async () => {
        setLoading(true);
        try {
            // Save first to ensure latest config
            await invoke('feishu_save_config', {
                appId: appId,
                appSecret: appSecret,
                botName: botName,
                enabled: enabled,
            });
            const result = await invoke<{ ok: boolean; token_prefix: string }>('feishu_test_connection');
            if (result.ok) {
                setToast(`✅ 连接成功！Token: ${result.token_prefix}...`);
            }
            loadStatus();
        } catch (e: any) {
            setError(`测试失败: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const toggleGateway = async () => {
        setLoading(true);
        try {
            if (status?.connected) {
                await invoke('feishu_disconnect');
                setToast('🔌 已断开连接');
            } else {
                // Save first
                await invoke('feishu_save_config', {
                    appId: appId,
                    appSecret: appSecret,
                    botName: botName,
                    enabled: enabled,
                });
                await invoke('feishu_connect');
                setToast('✅ WebSocket 已连接');
            }
            loadStatus();
        } catch (e: any) {
            setError(`操作失败: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const sendMessage = async () => {
        if (!chatId.trim() || !msgInput.trim()) return;
        setSending(true);
        try {
            await invoke('feishu_send_message', {
                chatId: chatId.trim(),
                content: msgInput.trim(),
            });
            setToast('✅ 消息已发送');
            setMsgInput('');
        } catch (e: any) {
            setError(`发送失败: ${e}`);
        } finally {
            setSending(false);
        }
    };

    return (
        <div className="max-w-3xl mx-auto px-4 py-6 space-y-6">
            {/* Toast / Error */}
            {toast && (
                <div className="fixed top-4 right-4 z-50 flex items-center gap-2 bg-green-50 dark:bg-green-900/30 text-green-700 dark:text-green-300 px-4 py-3 rounded-xl shadow-lg border border-green-200 dark:border-green-800 animate-fade-in">
                    <CheckCircle2 className="w-5 h-5" />
                    <span className="text-sm font-medium">{toast}</span>
                </div>
            )}
            {error && (
                <div className="fixed top-4 right-4 z-50 flex items-center gap-2 bg-red-50 dark:bg-red-900/30 text-red-700 dark:text-red-300 px-4 py-3 rounded-xl shadow-lg border border-red-200 dark:border-red-800 animate-fade-in">
                    <AlertCircle className="w-5 h-5" />
                    <span className="text-sm font-medium">{error}</span>
                </div>
            )}

            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-md">
                        <span className="text-white text-lg font-bold">飞</span>
                    </div>
                    <div>
                        <h1 className="text-xl font-bold text-gray-900 dark:text-white">飞书机器人</h1>
                        <p className="text-sm text-gray-500 dark:text-gray-400">Feishu/Lark Bot 配置与管理</p>
                    </div>
                </div>
                {/* Connection indicator */}
                <div className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium ${status?.connected
                    ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                    : 'bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400'
                    }`}>
                    {status?.connected ? <Wifi className="w-4 h-4" /> : <WifiOff className="w-4 h-4" />}
                    {status?.connected ? '已连接' : '未连接'}
                </div>
            </div>

            {/* Config Card */}
            <div className="bg-white dark:bg-base-200 rounded-2xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-700 flex items-center gap-2">
                    <Settings2 className="w-5 h-5 text-gray-500" />
                    <h2 className="font-semibold text-gray-900 dark:text-white">应用配置</h2>
                </div>
                <div className="p-6 space-y-4">
                    {/* App ID */}
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            App ID
                        </label>
                        <input
                            type="text"
                            value={appId}
                            onChange={(e) => setAppId(e.target.value)}
                            placeholder="cli_xxxxxxxxxx"
                            className="w-full px-4 py-2.5 rounded-xl border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-base-300 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all text-sm"
                        />
                    </div>

                    {/* App Secret */}
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            App Secret
                        </label>
                        <div className="relative">
                            <input
                                type={showSecret ? 'text' : 'password'}
                                value={appSecret}
                                onChange={(e) => setAppSecret(e.target.value)}
                                placeholder={status?.configured ? '••••••••（已配置）' : '输入 App Secret'}
                                className="w-full px-4 py-2.5 pr-10 rounded-xl border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-base-300 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all text-sm"
                            />
                            <button
                                onClick={() => setShowSecret(!showSecret)}
                                className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                            >
                                {showSecret ? <Eye className="w-4 h-4" /> : <EyeOff className="w-4 h-4" />}
                            </button>
                        </div>
                    </div>

                    {/* Bot Name */}
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            机器人名称
                        </label>
                        <input
                            type="text"
                            value={botName}
                            onChange={(e) => setBotName(e.target.value)}
                            placeholder="Helix"
                            className="w-full px-4 py-2.5 rounded-xl border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-base-300 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all text-sm"
                        />
                    </div>

                    {/* Enable toggle */}
                    <div className="flex items-center justify-between py-2">
                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            启动时自动连接
                        </span>
                        <label className="relative inline-flex items-center cursor-pointer">
                            <input
                                type="checkbox"
                                checked={enabled}
                                onChange={(e) => setEnabled(e.target.checked)}
                                className="sr-only peer"
                            />
                            <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:after:border-gray-600 peer-checked:bg-blue-600"></div>
                        </label>
                    </div>

                    {/* Action buttons */}
                    <div className="flex gap-3 pt-2">
                        <button
                            onClick={saveConfig}
                            disabled={loading || !appId.trim()}
                            className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-gray-900 dark:bg-white text-white dark:text-gray-900 rounded-xl font-medium text-sm hover:opacity-90 transition-all disabled:opacity-50"
                        >
                            {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <CheckCircle2 className="w-4 h-4" />}
                            保存配置
                        </button>
                        <button
                            onClick={testConnection}
                            disabled={loading || !appId.trim()}
                            className="flex items-center gap-2 px-4 py-2.5 bg-blue-50 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded-xl font-medium text-sm hover:bg-blue-100 dark:hover:bg-blue-900/50 transition-all disabled:opacity-50 border border-blue-200 dark:border-blue-800"
                        >
                            <TestTube className="w-4 h-4" />
                            测试
                        </button>
                    </div>
                </div>
            </div>

            {/* Gateway Card */}
            <div className="bg-white dark:bg-base-200 rounded-2xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-700 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <Wifi className="w-5 h-5 text-gray-500" />
                        <h2 className="font-semibold text-gray-900 dark:text-white">WebSocket 网关</h2>
                    </div>
                    {status?.token_valid && (
                        <span className="text-xs bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400 px-2 py-1 rounded-full">
                            Token 有效
                        </span>
                    )}
                </div>
                <div className="p-6">
                    <div className="flex items-center justify-between mb-4">
                        <div>
                            <p className="text-sm text-gray-600 dark:text-gray-400">
                                {status?.connected
                                    ? '✅ 已连接到飞书 WebSocket，正在监听消息'
                                    : '🔌 未连接，点击连接以接收飞书消息'}
                            </p>
                            {status?.app_id && (
                                <p className="text-xs text-gray-400 mt-1">App: {status.app_id}</p>
                            )}
                        </div>
                        <button
                            onClick={toggleGateway}
                            disabled={loading || !status?.configured}
                            className={`flex items-center gap-2 px-5 py-2.5 rounded-xl font-medium text-sm transition-all disabled:opacity-50 ${status?.connected
                                ? 'bg-red-50 text-red-700 hover:bg-red-100 dark:bg-red-900/30 dark:text-red-400 dark:hover:bg-red-900/50 border border-red-200 dark:border-red-800'
                                : 'bg-green-50 text-green-700 hover:bg-green-100 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50 border border-green-200 dark:border-green-800'
                                }`}
                        >
                            {loading ? (
                                <Loader2 className="w-4 h-4 animate-spin" />
                            ) : status?.connected ? (
                                <PowerOff className="w-4 h-4" />
                            ) : (
                                <Power className="w-4 h-4" />
                            )}
                            {status?.connected ? '断开' : '连接'}
                        </button>
                    </div>
                </div>
            </div>

            {/* Send Message Card */}
            <div className="bg-white dark:bg-base-200 rounded-2xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
                <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-700 flex items-center gap-2">
                    <Send className="w-5 h-5 text-gray-500" />
                    <h2 className="font-semibold text-gray-900 dark:text-white">发送消息</h2>
                </div>
                <div className="p-6 space-y-4">
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            Chat ID
                        </label>
                        <input
                            type="text"
                            value={chatId}
                            onChange={(e) => setChatId(e.target.value)}
                            placeholder="oc_xxxxxxxxxx（群聊 ID 或用户 open_id）"
                            className="w-full px-4 py-2.5 rounded-xl border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-base-300 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all text-sm"
                        />
                    </div>
                    <div className="flex gap-3">
                        <input
                            type="text"
                            value={msgInput}
                            onChange={(e) => setMsgInput(e.target.value)}
                            onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && sendMessage()}
                            placeholder="输入消息内容..."
                            disabled={sending}
                            className="flex-1 px-4 py-2.5 rounded-xl border border-gray-300 dark:border-gray-600 bg-gray-50 dark:bg-base-300 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all text-sm"
                        />
                        <button
                            onClick={sendMessage}
                            disabled={sending || !chatId.trim() || !msgInput.trim()}
                            className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white rounded-xl font-medium text-sm hover:bg-blue-700 transition-all disabled:opacity-50"
                        >
                            {sending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4" />}
                            发送
                        </button>
                    </div>
                </div>
            </div>

            {/* Help section */}
            <div className="text-sm text-gray-500 dark:text-gray-400 space-y-2 bg-gray-50 dark:bg-base-300 rounded-2xl p-5">
                <p className="font-medium text-gray-700 dark:text-gray-300">💡 使用说明</p>
                <ul className="list-disc list-inside space-y-1 text-xs">
                    <li>在<a href="https://open.feishu.cn/app" target="_blank" rel="noopener" className="text-blue-500 hover:underline mx-1">飞书开放平台</a>创建应用，获取 App ID 和 App Secret</li>
                    <li>应用需启用「机器人」能力，并添加 <code className="bg-gray-200 dark:bg-gray-700 px-1 rounded">im:message</code> 和 <code className="bg-gray-200 dark:bg-gray-700 px-1 rounded">im:message:send_as_bot</code> 权限</li>
                    <li>开启 WebSocket 长连接模式后，飞书发给机器人的消息会自动经 AI 处理并回复</li>
                    <li>Chat ID 可以是群聊 ID（<code className="bg-gray-200 dark:bg-gray-700 px-1 rounded">oc_xxx</code>）或用户 open_id（<code className="bg-gray-200 dark:bg-gray-700 px-1 rounded">ou_xxx</code>）</li>
                </ul>
            </div>
        </div>
    );
}
