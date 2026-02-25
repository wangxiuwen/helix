import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import 'highlight.js/styles/github-dark.min.css';
import {
    Bot,
    Check,
    ChevronRight,
    Clock,
    Blocks,
    MessageSquare,
    Moon,
    Plus,
    Search,
    Send,
    Settings as SettingsIcon,
    Sparkles,
    Sun,
    Trash2,
    User,
    Wrench,
    X,
    Activity,
} from 'lucide-react';
import { useDevOpsStore } from '../stores/useDevOpsStore';
import { useConfigStore } from '../stores/useConfigStore';
import { useNavigate } from 'react-router-dom';

function AIChat() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const {
        chatSessions,
        activeChatId,
        loading,
        createChatSession,
        deleteChatSession,
        setActiveChatId,
        sendMessage,
        confirmToolExecution,
    } = useDevOpsStore();

    const { config, saveConfig } = useConfigStore();
    const [input, setInput] = useState('');
    const [searchQuery, setSearchQuery] = useState('');
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const activeSession = chatSessions.find((s) => s.id === activeChatId);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [activeSession?.messages]);

    const handleSend = async () => {
        if (!input.trim()) return;
        const msg = input.trim();
        setInput('');
        let sid = activeChatId;
        if (!sid) {
            sid = createChatSession();
        }
        await sendMessage(sid, msg);
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const toggleTheme = () => {
        if (!config) return;
        const newTheme = config.theme === 'light' ? 'dark' : 'light';
        saveConfig({ ...config, theme: newTheme, language: config.language }, true);
    };

    const isDark = config?.theme === 'dark';

    // Filter sessions by search
    const filteredSessions = chatSessions.filter((s) =>
        !searchQuery || s.title.toLowerCase().includes(searchQuery.toLowerCase())
    );

    // Get last message preview for session list
    const getLastMessage = (session: typeof chatSessions[0]) => {
        if (!session.messages || session.messages.length === 0) return '';
        const last = session.messages[session.messages.length - 1];
        const content = last.content || '';
        return content.length > 40 ? content.slice(0, 40) + '...' : content;
    };

    const getLastTime = (session: typeof chatSessions[0]) => {
        if (!session.messages || session.messages.length === 0) return '';
        const last = session.messages[session.messages.length - 1];
        if (!last.timestamp) return '';
        const d = new Date(last.timestamp);
        const now = new Date();
        if (d.toDateString() === now.toDateString()) {
            return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false });
        }
        return d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' });
    };

    return (
        <div className="flex h-full overflow-hidden">
            {/* ===== Column 1: Icon sidebar (WeChat style narrow bar) ===== */}
            <div className="w-[54px] shrink-0 bg-[#e9e9e9] dark:bg-[#2e2e2e] flex flex-col items-center py-4 gap-1 border-r border-black/5 dark:border-white/5">
                {/* Avatar / Logo */}
                <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center mb-4 cursor-pointer shadow-sm">
                    <Sparkles size={16} className="text-white" />
                </div>

                {/* Nav icons */}
                <button
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-[#07c160] hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    title={t('nav.channels', '对话')}
                >
                    <MessageSquare size={20} />
                </button>
                <button
                    onClick={() => navigate('/skills')}
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    title={t('nav.skills', '技能')}
                >
                    <Blocks size={20} />
                </button>
                <button
                    onClick={() => navigate('/cron-jobs')}
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    title={t('nav.cron_jobs', '定时任务')}
                >
                    <Clock size={20} />
                </button>
                <button
                    onClick={() => navigate('/logs')}
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    title={t('nav.logs', '日志')}
                >
                    <Activity size={20} />
                </button>

                {/* Spacer */}
                <div className="flex-1" />

                {/* Bottom icons */}
                <button
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    onClick={toggleTheme}
                    title={isDark ? 'Light' : 'Dark'}
                >
                    {isDark ? <Sun size={18} /> : <Moon size={18} />}
                </button>
                <button
                    onClick={() => navigate('/settings')}
                    className="w-10 h-10 rounded-lg flex items-center justify-center text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/10 transition-colors"
                    title={t('nav.settings', '设置')}
                >
                    <SettingsIcon size={18} />
                </button>
            </div>

            {/* ===== Column 2: Session list ===== */}
            <div className="w-[250px] shrink-0 bg-[#f7f7f7] dark:bg-[#252525] flex flex-col border-r border-black/5 dark:border-white/5">
                {/* Search + New */}
                <div className="px-3 pt-4 pb-2 flex items-center gap-2">
                    <div className="flex-1 relative">
                        <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                        <input
                            type="text"
                            className="w-full pl-8 pr-3 py-1.5 text-xs bg-white dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400"
                            placeholder={t('chat.search', '搜索')}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                        />
                    </div>
                    <button
                        className="w-7 h-7 rounded-md flex items-center justify-center text-gray-500 hover:bg-black/5 dark:hover:bg-white/10 transition-colors shrink-0"
                        onClick={() => createChatSession()}
                        title={t('chat.new_session', '新对话')}
                    >
                        <Plus size={16} />
                    </button>
                </div>

                {/* Session list */}
                <div className="flex-1 overflow-y-auto">
                    {filteredSessions.length === 0 ? (
                        <div className="px-4 py-12 text-center text-gray-400 text-xs">
                            {t('chat.no_sessions', '暂无对话')}
                        </div>
                    ) : (
                        filteredSessions.map((session) => (
                            <div
                                key={session.id}
                                className={`flex items-center px-3 py-3 cursor-pointer transition-colors group ${activeChatId === session.id
                                        ? 'bg-[#c9c9c9] dark:bg-[#383838]'
                                        : 'hover:bg-[#ebebeb] dark:hover:bg-[#303030]'
                                    }`}
                                onClick={() => setActiveChatId(session.id)}
                            >
                                {/* Avatar */}
                                <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-violet-400 to-blue-500 flex items-center justify-center shrink-0 mr-3">
                                    <Bot size={18} className="text-white" />
                                </div>
                                {/* Info */}
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center justify-between">
                                        <span className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">
                                            {session.title}
                                        </span>
                                        <span className="text-[10px] text-gray-400 dark:text-gray-500 shrink-0 ml-2">
                                            {getLastTime(session)}
                                        </span>
                                    </div>
                                    <div className="flex items-center justify-between mt-0.5">
                                        <p className="text-xs text-gray-400 dark:text-gray-500 truncate">
                                            {getLastMessage(session)}
                                        </p>
                                        <button
                                            className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-black/10 dark:hover:bg-white/10 transition-all shrink-0 ml-1"
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                deleteChatSession(session.id);
                                            }}
                                        >
                                            <Trash2 size={12} className="text-gray-400" />
                                        </button>
                                    </div>
                                </div>
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* ===== Column 3: Chat area ===== */}
            <div className="flex-1 flex flex-col min-w-0 bg-[#f5f5f5] dark:bg-[#1e1e1e]">
                {!activeSession ? (
                    /* Empty state */
                    <div className="flex-1 flex items-center justify-center">
                        <div className="text-center">
                            <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-violet-500/10 to-blue-500/10 flex items-center justify-center mx-auto mb-4">
                                <Sparkles size={32} className="text-violet-400" />
                            </div>
                            <h2 className="text-lg font-medium text-gray-600 dark:text-gray-400 mb-2">
                                {t('chat.welcome', 'Helix 智能助手')}
                            </h2>
                            <p className="text-xs text-gray-400 dark:text-gray-500 max-w-sm">
                                {t('chat.welcome_desc', '选择一个对话或开始新对话')}
                            </p>
                            <button
                                className="mt-4 px-4 py-2 text-sm bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg transition-colors"
                                onClick={() => createChatSession()}
                            >
                                <Plus size={14} className="inline mr-1" />
                                {t('chat.start', '开始对话')}
                            </button>
                        </div>
                    </div>
                ) : (
                    <>
                        {/* Chat header */}
                        <div className="h-14 px-5 flex items-center justify-between border-b border-black/5 dark:border-white/5 shrink-0 bg-[#f5f5f5] dark:bg-[#1e1e1e]">
                            <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">
                                {activeSession.title}
                            </h3>
                        </div>

                        {/* Messages */}
                        <div className="flex-1 overflow-y-auto px-16 py-5 space-y-5">
                            {activeSession.messages.length === 0 && (
                                <div className="text-center py-12 text-gray-400">
                                    <Bot size={40} className="mx-auto mb-3 opacity-30" />
                                    <p className="text-sm">{t('chat.empty_hint', '发送消息开始对话')}</p>
                                </div>
                            )}
                            {activeSession.messages.map((msg) => (
                                <div
                                    key={msg.id}
                                    className={`flex gap-3 ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}
                                >
                                    {/* Avatar */}
                                    <div className={`w-9 h-9 rounded-md shrink-0 flex items-center justify-center mt-0.5 ${msg.role === 'user'
                                            ? 'bg-gradient-to-br from-blue-400 to-teal-400'
                                            : 'bg-gradient-to-br from-violet-500 to-blue-500'
                                        }`}>
                                        {msg.role === 'user'
                                            ? <User size={16} className="text-white" />
                                            : <Bot size={16} className="text-white" />
                                        }
                                    </div>

                                    {/* Bubble */}
                                    <div className={`max-w-[65%] relative ${msg.role === 'user' ? 'order-first' : ''
                                        }`}>
                                        {/* Arrow */}
                                        <div className={`absolute top-3 w-2 h-2 rotate-45 ${msg.role === 'user'
                                                ? '-right-1 bg-[#95ec69] dark:bg-[#3eb575]'
                                                : '-left-1 bg-white dark:bg-[#2e2e2e]'
                                            }`} />

                                        <div className={`rounded-md px-3 py-2.5 text-sm leading-relaxed relative ${msg.role === 'user'
                                                ? 'bg-[#95ec69] dark:bg-[#3eb575] text-gray-900 dark:text-white'
                                                : 'bg-white dark:bg-[#2e2e2e] text-gray-800 dark:text-gray-200'
                                            }`}>
                                            <div className="prose prose-sm dark:prose-invert max-w-none break-words [&_pre]:bg-gray-100 [&_pre]:dark:bg-gray-800 [&_pre]:rounded-md [&_pre]:p-2.5 [&_pre]:overflow-x-auto [&_pre]:text-xs [&_code]:text-xs [&_p]:my-1 [&_ul]:my-1 [&_ol]:my-1 [&_li]:my-0.5 [&_h1]:text-base [&_h2]:text-sm [&_h3]:text-sm [&_table]:text-xs [&_a]:text-blue-500">
                                                <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                                                    {msg.content}
                                                </ReactMarkdown>
                                            </div>

                                            {/* Tool calls */}
                                            {msg.toolCalls && msg.toolCalls.length > 0 && (
                                                <div className="mt-2 space-y-1">
                                                    {msg.toolCalls.map((tc: any, i: number) => (
                                                        <details key={i} className="group rounded bg-gray-50 dark:bg-gray-800/50 overflow-hidden">
                                                            <summary className="flex items-center gap-1.5 text-xs px-2 py-1.5 cursor-pointer select-none hover:bg-gray-100 dark:hover:bg-gray-700/50 transition-colors">
                                                                <ChevronRight size={10} className="group-open:rotate-90 transition-transform text-gray-400 shrink-0" />
                                                                <Wrench size={10} className="text-violet-500 shrink-0" />
                                                                <span className="font-mono text-xs">{tc.name}</span>
                                                                <span className={`ml-auto text-[10px] ${tc.status === 'done' ? 'text-green-500' : 'text-red-500'}`}>
                                                                    {tc.status === 'done' ? '✓' : '✗'}
                                                                </span>
                                                            </summary>
                                                            <div className="px-2 pb-1.5 text-[11px] font-mono text-gray-500 whitespace-pre-wrap max-h-32 overflow-y-auto border-t border-gray-200 dark:border-gray-700">
                                                                {tc.result?.slice(0, 500) || '(no result)'}
                                                                {tc.result && tc.result.length > 500 && <span className="text-gray-300">...</span>}
                                                            </div>
                                                        </details>
                                                    ))}
                                                </div>
                                            )}

                                            {/* Confirm buttons */}
                                            {msg.pendingConfirm && (
                                                <div className="mt-2 flex gap-2">
                                                    <button
                                                        className="px-3 py-1 text-xs bg-[#07c160] text-white rounded transition-colors hover:bg-[#06ad56]"
                                                        onClick={() => confirmToolExecution(activeChatId!, msg.id)}
                                                    >
                                                        <Check size={12} className="inline mr-1" />{t('chat.confirm', '确认')}
                                                    </button>
                                                    <button className="px-3 py-1 text-xs bg-gray-200 dark:bg-gray-600 text-gray-600 dark:text-gray-300 rounded transition-colors hover:bg-gray-300">
                                                        <X size={12} className="inline mr-1" />{t('chat.cancel', '取消')}
                                                    </button>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))}
                            {loading.chat && (
                                <div className="flex gap-3">
                                    <div className="w-9 h-9 rounded-md bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center shrink-0">
                                        <Bot size={16} className="text-white" />
                                    </div>
                                    <div className="relative">
                                        <div className="absolute top-3 -left-1 w-2 h-2 rotate-45 bg-white dark:bg-[#2e2e2e]" />
                                        <div className="bg-white dark:bg-[#2e2e2e] rounded-md px-4 py-3 relative">
                                            <span className="loading loading-dots loading-sm text-gray-400"></span>
                                        </div>
                                    </div>
                                </div>
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Input area — WeChat style */}
                        <div className="border-t border-black/5 dark:border-white/5 bg-[#f5f5f5] dark:bg-[#1e1e1e]">
                            <div className="px-5 py-3">
                                <textarea
                                    className="w-full bg-white dark:bg-[#2e2e2e] rounded-md border-0 outline-none resize-none text-sm text-gray-800 dark:text-gray-200 placeholder:text-gray-400 px-3 py-2.5 min-h-[80px] max-h-[160px]"
                                    placeholder={t('chat.input_placeholder', '输入消息...')}
                                    value={input}
                                    onChange={(e) => setInput(e.target.value)}
                                    onKeyDown={handleKeyDown}
                                    rows={3}
                                />
                                <div className="flex justify-end mt-2">
                                    <button
                                        className="px-4 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] disabled:bg-gray-300 disabled:dark:bg-gray-600 text-white rounded transition-colors"
                                        onClick={handleSend}
                                        disabled={!input.trim() || loading.chat}
                                    >
                                        {t('chat.send', '发送')}(S)
                                    </button>
                                </div>
                            </div>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}

export default AIChat;
