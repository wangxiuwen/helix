import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    MessageCircle,
    Send,
    LogOut,
    Loader2,
    CheckCircle2,
    AlertCircle,
    Bot,
    Plus,
    X,
} from 'lucide-react';

interface ChatMessage {
    content: string;
    from_me: boolean;
    is_bot: boolean;
    timestamp: number;
    msg_type: number;
}

interface Session {
    id: string;
    logged_in: boolean;
    username: string;
}

interface DbMessage {
    id: number;
    account_id: string;
    content: string;
    from_me: boolean;
    msg_type: number;
    ai_reply: boolean;
    created_at: string;
}

export default function WeChat() {
    // Multi-session state
    const [sessions, setSessions] = useState<Session[]>([]);
    const [activeSessionId, setActiveSessionId] = useState<string>('');

    // Current session UI state
    const [qrUrl, setQrUrl] = useState('');
    const [loginPhase, setLoginPhase] = useState<'idle' | 'qr' | 'scanned' | 'logging_in'>('idle');
    const loginPhaseRef = useRef(loginPhase);

    // Sync ref manually
    useEffect(() => {
        loginPhaseRef.current = loginPhase;
    }, [loginPhase]);
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [msgInput, setMsgInput] = useState('');
    const [sending, setSending] = useState(false);
    const [error, setError] = useState('');
    const [toast, setToast] = useState('');
    const [autoReply, setAutoReply] = useState(false);
    const [aiConfigured, setAiConfigured] = useState(false);

    const messagesEndRef = useRef<HTMLDivElement>(null);
    const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
    const loginPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

    // Auto-clear toast/error
    useEffect(() => {
        if (toast) { const t = setTimeout(() => setToast(''), 3000); return () => clearTimeout(t); }
    }, [toast]);
    useEffect(() => {
        if (error) { const t = setTimeout(() => setError(''), 6000); return () => clearTimeout(t); }
    }, [error]);

    // Scroll to bottom
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    // Load AI config on mount
    useEffect(() => {
        (async () => {
            try {
                const cfg = await invoke<{ auto_reply: boolean; api_key_set: boolean }>('ai_get_config');
                setAiConfigured(cfg.api_key_set);
            } catch { /* ignore */ }
        })();
        return () => {
            if (pollingRef.current) clearInterval(pollingRef.current);
            if (loginPollRef.current) clearInterval(loginPollRef.current);
        };
    }, []);

    // Refresh sessions list
    const refreshSessions = useCallback(async () => {
        try {
            const res = await invoke<{ sessions: Session[] }>('filehelper_list_sessions');
            setSessions(res.sessions);
            return res.sessions;
        } catch { return []; }
    }, []);

    // On mount: load sessions, auto-create if none
    const initRef = useRef(false);
    useEffect(() => {
        if (initRef.current) return;
        initRef.current = true;
        (async () => {
            const list = await refreshSessions();
            if (list.length === 0) {
                // Auto-create a session for convenience
                await createSessionInner();
            }
        })();
    }, []);

    // Load per-account auto_reply when switching sessions
    useEffect(() => {
        if (!activeSessionId) return;
        (async () => {
            try {
                const accounts = await invoke<Array<{ id: string; auto_reply: boolean }>>('db_list_accounts');
                const acc = accounts.find(a => a.id === activeSessionId);
                setAutoReply(acc?.auto_reply ?? false);
            } catch { /* ignore */ }
        })();
    }, [activeSessionId]);

    // ---- Create new session (inner, reusable) ----
    const createSessionInner = useCallback(async () => {
        try {
            const res = await invoke<{ session_id: string }>('filehelper_create_session');
            const newSession: Session = { id: res.session_id, logged_in: false, username: '' };
            setSessions(prev => [...prev, newSession]);
            setActiveSessionId(res.session_id);
            setMessages([]);
            setQrUrl('');
            setLoginPhase('idle');
            return res.session_id;
        } catch (e: any) {
            setError(String(e));
            return null;
        }
    }, []);

    const loadHistory = useCallback(async (sid: string) => {
        try {
            const dbMsgs = await invoke<DbMessage[]>('db_get_messages', { accountId: sid, limit: 200 });
            const msgs: ChatMessage[] = dbMsgs.map(m => ({
                content: m.content,
                from_me: m.from_me,
                is_bot: m.ai_reply,
                timestamp: Math.floor(new Date((m.created_at || '').replace(' ', 'T') + 'Z').getTime() / 1000) || Math.floor(Date.now() / 1000),
                msg_type: m.msg_type,
            }));

            // Robust Merge: Keep any pending optimistic messages that haven't hit the DB yet
            setMessages(prev => {
                // Find messages that are in prev but NOT in msgs (based on recent from_me content)
                const pendingOptimistic = prev.filter(p => {
                    // Only keep our recent optimistic messages
                    if (!p.from_me) return false;
                    // If it's too old (> 1 min), drop it (it failed or was deleted)
                    if (Date.now() / 1000 - p.timestamp > 60) return false;
                    // Keep it if there's no matching message in the DB results
                    const existsInDb = msgs.some(m => m.from_me && m.content === p.content && Math.abs(m.timestamp - p.timestamp) < 60);
                    return !existsInDb;
                });

                if (pendingOptimistic.length === 0) return msgs;
                return [...msgs, ...pendingOptimistic];
            });
        } catch { /* no history */ }
    }, []);

    // ---- Message polling ----
    const startMessagePolling = useCallback((sid: string) => {
        if (pollingRef.current) clearTimeout(pollingRef.current as unknown as number);

        const poll = async () => {
            try {
                const res = await invoke<{ has_new: boolean; messages: ChatMessage[]; expired?: boolean; error?: string }>('filehelper_poll_messages', { sessionId: sid });
                if (res.expired) {
                    setError('微信已过期或退出，请重新扫码登录');
                    setSessions(prev => prev.map(s => s.id === sid ? { ...s, logged_in: false } : s));
                    if (sid === activeSessionId) {
                        setLoginPhase('idle'); // Restore login button UI
                    }
                    if (pollingRef.current) clearTimeout(pollingRef.current as unknown as number);
                    return; // Stop polling for this expired session
                }
                if (res.has_new) {
                    // Just reload history from DB. This guarantees we see both
                    // incoming messages and our own bot replies (which were saved to DB)
                    // without any fragile deduplication logic.
                    await loadHistory(sid);
                }
            } catch { /* ignore */ }
            // Schedule next strictly AFTER previous finishes
            if (pollingRef.current) {
                pollingRef.current = setTimeout(poll, 1500) as unknown as ReturnType<typeof setInterval>;
            }
        };

        pollingRef.current = setTimeout(poll, 1500) as unknown as ReturnType<typeof setInterval>;
    }, []);

    // ---- Login flow ----
    const startLogin = useCallback(async () => {
        if (!activeSessionId) return;
        setError('');
        setLoginPhase('qr');
        try {
            const res = await invoke<{ uuid: string; qr_url: string }>('filehelper_get_qr', { sessionId: activeSessionId });
            setQrUrl(res.qr_url);

            if (loginPollRef.current) clearInterval(loginPollRef.current);
            let errorCount = 0;
            const pollLogin = async () => {
                if (loginPhaseRef.current !== 'qr' && loginPhaseRef.current !== 'scanned') return;
                try {
                    const status = await invoke<{ status: string; nickname?: string }>('filehelper_check_login', { sessionId: activeSessionId });
                    errorCount = 0;
                    if (status.status === 'scanned') {
                        setLoginPhase('scanned');
                        loginPhaseRef.current = 'scanned';
                    } else if (status.status === 'logged_in') {
                        setLoginPhase('idle');
                        loginPhaseRef.current = 'idle';
                        setError(''); // Clear any remaining errors from the UI

                        // Directly update local session state for immediate UI feedback
                        const nickname = status.nickname || '微信用户';
                        setSessions(prev => prev.map(s =>
                            s.id === activeSessionId
                                ? { ...s, logged_in: true, username: nickname }
                                : s
                        ));

                        // Also refresh from backend for consistency
                        await refreshSessions();
                        // Load history from database
                        await loadHistory(activeSessionId);
                        startMessagePolling(activeSessionId);
                        return; // Done probing
                    }
                } catch (e: any) {
                    const errStr = String(e);
                    if (errStr.includes('waiting')) {
                        // Normal — still waiting for scan, do nothing
                    } else {
                        console.error('[WeChat] Login poll error:', errStr);
                        errorCount++;
                        // Show the actual error after first non-waiting error
                        if (errorCount === 1) {
                            setError(errStr);
                        }
                        if (errorCount > 3) {
                            setError('登录失败: ' + errStr);
                            setLoginPhase('qr');
                            loginPhaseRef.current = 'qr';
                        }
                    }
                }

                // Keep polling if still in qr or scanned phase
                if (loginPhaseRef.current === 'qr' || loginPhaseRef.current === 'scanned') {
                    loginPollRef.current = setTimeout(pollLogin, 500) as unknown as ReturnType<typeof setInterval>;
                }
            };

            loginPollRef.current = setTimeout(pollLogin, 500) as unknown as ReturnType<typeof setInterval>;
        } catch (e: any) {
            setError(String(e));
            setLoginPhase('idle');
        }
    }, [activeSessionId, refreshSessions, loadHistory, startMessagePolling]);

    // ---- Send message ----
    const handleSend = useCallback(async () => {
        if (!msgInput.trim() || !activeSessionId) return;
        const content = msgInput.trim();
        setMsgInput('');
        setSending(true);
        setError('');

        // Optimistic UI: show message immediately
        const optimisticMsg: ChatMessage = {
            content,
            from_me: true,
            is_bot: false,
            timestamp: Math.floor(Date.now() / 1000),
            msg_type: 1,
        };
        setMessages(prev => [...prev, optimisticMsg]);

        // Force React to repaint the UI before blocking on Tauri IPC
        setTimeout(async () => {
            try {
                await invoke('filehelper_send_msg', { sessionId: activeSessionId, content });
                // Background sync to get server-confirmed messages
                loadHistory(activeSessionId).catch(() => { });
            } catch (e: any) {
                setError(String(e));
            } finally {
                setSending(false);
            }
        }, 10);
    }, [msgInput, activeSessionId, loadHistory]);

    const handleKeyDown = (e: React.KeyboardEvent) => {
        // Don't intercept Enter while IME is composing (e.g. typing Chinese)
        if (e.nativeEvent.isComposing || e.keyCode === 229) return;
        if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend(); }
    };

    // ---- Switch session ----
    const switchSession = useCallback(async (sid: string) => {
        if (pollingRef.current) clearInterval(pollingRef.current);
        if (loginPollRef.current) clearInterval(loginPollRef.current);
        setActiveSessionId(sid);
        setMessages([]);
        setQrUrl('');
        setLoginPhase('idle');

        const s = sessions.find(x => x.id === sid);
        if (s?.logged_in) {
            await loadHistory(sid);
            startMessagePolling(sid);
        }
    }, [sessions, loadHistory, startMessagePolling]);

    // ---- Logout ----
    const handleLogout = useCallback(async (sid?: string | React.MouseEvent) => {
        const targetSid = typeof sid === 'string' ? sid : activeSessionId;
        if (!targetSid) return;
        if (targetSid === activeSessionId) {
            if (pollingRef.current) clearInterval(pollingRef.current);
            if (loginPollRef.current) clearInterval(loginPollRef.current);
            setActiveSessionId('');
            setMessages([]);
            setQrUrl('');
            setLoginPhase('idle');
        }
        await invoke('filehelper_logout', { sessionId: targetSid }).catch(() => { });
        setSessions(prev => prev.filter(s => s.id !== targetSid));
    }, [activeSessionId]);

    // ---- Toggle auto-reply (per-account) ----
    const toggleAutoReply = useCallback(async () => {
        if (!activeSessionId) return;
        if (!aiConfigured) {
            setError('请先在设置中配置 AI API Key');
            return;
        }
        const next = !autoReply;
        try {
            await invoke('db_set_auto_reply', { accountId: activeSessionId, enabled: next });
            setAutoReply(next);
            setToast(next ? 'AI 自动回复已开启' : 'AI 自动回复已关闭');
        } catch (e: any) {
            setError(String(e));
        }
    }, [activeSessionId, autoReply, aiConfigured]);

    // ---- Format time ----
    const formatTime = (ts: number) => {
        if (!ts || isNaN(ts)) return '';
        const d = new Date(ts * 1000);
        if (isNaN(d.getTime())) return '';
        return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
    };

    const activeSession = sessions.find(s => s.id === activeSessionId);
    const isLoggedIn = activeSession?.logged_in ?? false;

    // ============================================================
    // Render
    // ============================================================
    return (
        <div className="flex-1 flex flex-col h-full overflow-hidden">
            {/* Channel Sub-Tabs */}
            <div className="px-8 py-3 border-b border-gray-200/60 dark:border-base-200/60">
                <div className="max-w-5xl mx-auto">
                    <div className="flex items-center gap-1 bg-gray-100 dark:bg-base-200 rounded-xl p-1">
                        {[
                            { key: 'wechat', label: '微信文件传输助手', active: true },
                            { key: 'feishu', label: '飞书', active: false },
                            { key: 'wecom', label: '企业微信', active: false },
                            { key: 'dingtalk', label: '钉钉', active: false },
                        ].map(ch => (
                            <button
                                key={ch.key}
                                className={`flex-1 px-4 py-2 text-sm rounded-lg transition-all ${ch.active
                                    ? 'bg-white dark:bg-base-300 shadow-sm font-medium text-gray-900 dark:text-white'
                                    : 'text-gray-400 dark:text-gray-500 cursor-not-allowed'
                                    }`}
                                disabled={!ch.active}
                                title={ch.active ? '' : '即将推出'}
                            >
                                {ch.label}
                                {!ch.active && <span className="ml-1 text-xs opacity-60">（即将推出）</span>}
                            </button>
                        ))}
                    </div>
                </div>
            </div>

            {/* Header */}
            <div className="px-8 py-5 border-b border-gray-200/60 dark:border-base-200/60">
                <div className="max-w-5xl mx-auto flex items-center justify-between">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 bg-gradient-to-br from-green-400 to-green-600 rounded-xl flex items-center justify-center shadow-lg shadow-green-500/20">
                            <MessageCircle className="w-5 h-5 text-white" />
                        </div>
                        <div>
                            <h1 className="text-xl font-bold text-gray-900 dark:text-white">微信文件传输助手</h1>
                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                {isLoggedIn ? `已登录 — ${activeSession?.username}` : '扫码登录 · 多账号'}
                            </p>
                        </div>
                    </div>
                    {isLoggedIn && (
                        <div className="flex items-center gap-3">
                            <button
                                onClick={toggleAutoReply}
                                className={`flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg transition-all ${autoReply
                                    ? 'text-green-600 bg-green-50 dark:bg-green-900/30 dark:text-green-400 shadow-sm'
                                    : 'text-gray-500 hover:text-green-500 hover:bg-green-50 dark:hover:bg-green-900/20'
                                    }`}
                                title={aiConfigured ? (autoReply ? '关闭自动回复' : '开启自动回复') : '请先配置 AI API Key'}
                            >
                                <Bot className="w-4 h-4" />
                                {autoReply ? 'AI 已开启' : 'AI 回复'}
                            </button>
                            <button
                                onClick={() => handleLogout(activeSessionId)}
                                className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-500 hover:text-red-500 transition-colors rounded-lg hover:bg-red-50 dark:hover:bg-red-900/20"
                            >
                                <LogOut className="w-4 h-4" />
                                退出
                            </button>
                        </div>
                    )}
                </div>
            </div>

            {/* Session Tabs */}
            <div className="px-8 py-2 border-b border-gray-200/60 dark:border-base-200/60 bg-gray-50/50 dark:bg-base-100/50">
                <div className="max-w-5xl mx-auto flex items-center gap-2 overflow-x-auto">
                    {sessions.map(s => (
                        <button
                            key={s.id}
                            onClick={() => switchSession(s.id)}
                            className={`group flex items-center gap-2 px-3 py-1.5 text-sm rounded-lg whitespace-nowrap transition-all ${s.id === activeSessionId
                                ? 'bg-white dark:bg-base-200 shadow-sm text-gray-900 dark:text-white font-medium'
                                : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-white/60 dark:hover:bg-base-200/60'
                                }`}
                        >
                            <span className={`w-2 h-2 rounded-full ${s.logged_in ? 'bg-green-500' : 'bg-gray-300 dark:bg-gray-600'}`} />
                            {s.logged_in ? (s.username || '微信用户') : '未登录'}
                            <span
                                onClick={e => { e.stopPropagation(); handleLogout(s.id); }}
                                className="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-gray-200 dark:hover:bg-base-300 rounded transition-all cursor-pointer"
                            >
                                <X className="w-3 h-3" />
                            </span>
                        </button>
                    ))}
                    <button
                        onClick={createSessionInner}
                        className="flex items-center gap-1 px-3 py-1.5 text-sm text-gray-500 hover:text-green-600 hover:bg-green-50 dark:hover:bg-green-900/20 rounded-lg transition-all"
                    >
                        <Plus className="w-4 h-4" />
                        添加账号
                    </button>
                </div>
            </div>

            {/* Toasts */}
            {error && (
                <div className="mx-8 mt-3 max-w-5xl mx-auto">
                    <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm">
                        <AlertCircle className="w-4 h-4 flex-shrink-0" />
                        {error}
                    </div>
                </div>
            )}
            {toast && (
                <div className="mx-8 mt-3 max-w-5xl mx-auto">
                    <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-green-50 dark:bg-green-900/20 text-green-600 dark:text-green-400 text-sm">
                        <CheckCircle2 className="w-4 h-4 flex-shrink-0" />
                        {toast}
                    </div>
                </div>
            )}

            {/* Main Content */}
            <div className="flex-1 overflow-y-auto px-8 py-6">
                <div className="max-w-5xl mx-auto">
                    {/* No session selected */}
                    {!activeSessionId && (
                        <div className="flex flex-col items-center justify-center h-64 gap-4 text-gray-400 dark:text-gray-500">
                            <MessageCircle className="w-16 h-16 opacity-30" />
                            <p className="text-lg">点击「添加账号」开始使用</p>
                            <button
                                onClick={createSessionInner}
                                className="px-4 py-2 bg-green-500 hover:bg-green-600 text-white rounded-lg transition-colors flex items-center gap-2"
                            >
                                <Plus className="w-4 h-4" />
                                添加微信账号
                            </button>
                        </div>
                    )}

                    {/* Not logged in — QR code */}
                    {activeSessionId && !isLoggedIn && (
                        <div className="flex flex-col items-center justify-center h-64 gap-4">
                            {loginPhase === 'idle' && (
                                <button
                                    onClick={startLogin}
                                    className="px-6 py-3 bg-green-500 hover:bg-green-600 text-white rounded-xl shadow-lg shadow-green-500/20 transition-all transform hover:scale-105 flex items-center gap-2 text-lg"
                                >
                                    扫码登录
                                </button>
                            )}
                            {loginPhase === 'qr' && qrUrl && (
                                <div className="flex flex-col items-center gap-3">
                                    <p className="text-sm text-gray-500 dark:text-gray-400">请使用微信扫描二维码</p>
                                    <img src={qrUrl} alt="QR Code" className="w-52 h-52 rounded-xl shadow-lg border border-gray-200 dark:border-base-200" />
                                    <button
                                        onClick={() => { if (loginPollRef.current) clearInterval(loginPollRef.current); startLogin(); }}
                                        className="text-sm text-green-500 hover:text-green-600 hover:underline transition-colors"
                                    >
                                        🔄 刷新二维码
                                    </button>
                                </div>
                            )}
                            {loginPhase === 'scanned' && (
                                <div className="flex items-center gap-2 text-green-500">
                                    <Loader2 className="w-5 h-5 animate-spin" />
                                    <span>已扫码，请在手机上确认...</span>
                                </div>
                            )}
                        </div>
                    )}

                    {/* Logged in — Messages */}
                    {activeSessionId && isLoggedIn && (
                        <div className="space-y-4">
                            {messages.length === 0 ? (
                                <div className="text-center text-gray-400 dark:text-gray-500 py-12">
                                    <MessageCircle className="w-12 h-12 mx-auto mb-3 opacity-30" />
                                    <p>暂无消息，发送一条试试</p>
                                </div>
                            ) : (
                                messages.map((msg, i) => (
                                    <div key={i} className={`flex ${!msg.is_bot ? 'justify-end' : 'justify-start'}`}>
                                        <div
                                            className={`max-w-[90%] px-4 py-2.5 rounded-2xl text-sm leading-relaxed overflow-hidden ${!msg.is_bot
                                                ? 'bg-green-500 text-white rounded-br-md shadow-sm'
                                                : 'bg-white dark:bg-base-200 text-gray-800 dark:text-gray-200 rounded-bl-md shadow-sm border border-gray-100 dark:border-base-300'
                                                }`}
                                        >
                                            <div className="flex items-start gap-2">
                                                {msg.is_bot && <Bot className="w-4 h-4 mt-0.5 opacity-70 flex-shrink-0 text-gray-500 dark:text-gray-400" />}
                                                <p className="whitespace-pre-wrap break-words flex-1" style={{ overflowWrap: 'anywhere' }}>{msg.content}</p>
                                            </div>
                                            <p className={`text-xs mt-1 flex ${!msg.is_bot ? 'justify-end text-green-100' : 'text-gray-400 dark:text-gray-500'}`}>
                                                {formatTime(msg.timestamp)}
                                            </p>
                                        </div>
                                    </div>
                                ))
                            )}
                            <div ref={messagesEndRef} />
                        </div>
                    )}
                </div>
            </div>

            {/* Input area — only when logged in */}
            {activeSessionId && isLoggedIn && (
                <div className="px-8 py-4 border-t border-gray-200/60 dark:border-base-200/60 bg-white/50 dark:bg-base-100/50">
                    <div className="max-w-5xl mx-auto flex items-end gap-3">
                        <textarea
                            value={msgInput}
                            onChange={e => setMsgInput(e.target.value)}
                            onKeyDown={handleKeyDown}
                            placeholder="输入消息..."
                            rows={1}
                            className="flex-1 px-4 py-2.5 rounded-xl border border-gray-200 dark:border-base-300 bg-white dark:bg-base-200 text-gray-900 dark:text-white resize-none text-sm focus:outline-none focus:ring-2 focus:ring-green-500/30 focus:border-green-400"
                        />
                        <button
                            onClick={handleSend}
                            disabled={!msgInput.trim() || sending}
                            className="p-2.5 bg-green-500 hover:bg-green-600 text-white rounded-xl disabled:opacity-50 disabled:cursor-not-allowed transition-colors shadow-md shadow-green-500/20"
                        >
                            {sending ? <Loader2 className="w-5 h-5 animate-spin" /> : <Send className="w-5 h-5" />}
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
}
