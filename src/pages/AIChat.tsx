import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import 'highlight.js/styles/github-dark.min.css';
import {
    Bot,
    Check,
    ChevronLeft,
    ChevronRight,
    Clock,
    Blocks,
    MessageSquare,
    Moon,
    Plus,
    Send,
    Settings as SettingsIcon,
    Sparkles,
    Sun,
    Trash2,
    User,
    Wrench,
    X,
    MoreVertical,
    Info,
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
    const [sidebarOpen, setSidebarOpen] = useState(true);
    const [menuOpen, setMenuOpen] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const activeSession = chatSessions.find((s) => s.id === activeChatId);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [activeSession?.messages]);

    // Close dropdown on outside click
    useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                setMenuOpen(false);
            }
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

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

    return (
        <div className="flex h-full overflow-hidden">
            {/* ===== Collapsible Sidebar ===== */}
            <div
                className={`flex flex-col border-r border-base-200/60 bg-base-100/80 backdrop-blur-sm transition-all duration-300 ease-in-out shrink-0 ${sidebarOpen ? 'w-64' : 'w-0 overflow-hidden border-r-0'
                    }`}
            >
                {/* Logo + collapse */}
                <div className="flex items-center justify-between px-4 h-14 shrink-0">
                    <div className="flex items-center gap-2">
                        <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-violet-600 to-blue-500 flex items-center justify-center">
                            <Sparkles size={14} className="text-white" />
                        </div>
                        <span className="font-bold text-base tracking-tight text-base-content">Helix</span>
                    </div>
                    <button
                        className="btn btn-ghost btn-xs btn-square"
                        onClick={() => setSidebarOpen(false)}
                        title={t('chat.collapse_sidebar', '收起侧边栏')}
                    >
                        <ChevronLeft size={16} />
                    </button>
                </div>

                {/* New Chat button */}
                <div className="px-3 pb-2">
                    <button
                        className="btn btn-outline btn-sm w-full gap-2 rounded-xl border-base-300 hover:border-primary/50 hover:bg-primary/5 transition-all"
                        onClick={() => createChatSession()}
                    >
                        <Plus size={14} />
                        {t('chat.new_session', '新对话')}
                    </button>
                </div>

                {/* Session list */}
                <div className="flex-1 overflow-y-auto px-2">
                    <div className="text-xs font-medium text-base-content/40 px-2 py-2 uppercase tracking-wider">
                        {t('chat.all_sessions', '所有对话')}
                    </div>
                    {chatSessions.length === 0 ? (
                        <div className="px-3 py-8 text-center text-base-content/30 text-xs">
                            {t('chat.no_sessions', '暂无对话')}
                        </div>
                    ) : (
                        chatSessions.map((session) => (
                            <div
                                key={session.id}
                                className={`flex items-center justify-between px-3 py-2 rounded-lg cursor-pointer transition-colors group mb-0.5 ${activeChatId === session.id
                                    ? 'bg-primary/10 text-primary'
                                    : 'hover:bg-base-200/60 text-base-content/70'
                                    }`}
                                onClick={() => setActiveChatId(session.id)}
                            >
                                <div className="flex items-center gap-2 min-w-0 flex-1">
                                    <MessageSquare size={14} className="shrink-0 opacity-50" />
                                    <span className="text-sm truncate">{session.title}</span>
                                </div>
                                <button
                                    className="opacity-0 group-hover:opacity-100 btn btn-ghost btn-xs btn-square transition-opacity"
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        deleteChatSession(session.id);
                                    }}
                                >
                                    <Trash2 size={12} />
                                </button>
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* ===== Main Area ===== */}
            <div className="flex-1 flex flex-col min-w-0">
                {/* Top bar */}
                <div className="flex items-center justify-between px-4 h-14 shrink-0">
                    {/* Left: sidebar toggle */}
                    <div className="flex items-center gap-2">
                        {!sidebarOpen && (
                            <button
                                className="btn btn-ghost btn-sm btn-square"
                                onClick={() => setSidebarOpen(true)}
                                title={t('chat.expand_sidebar', '展开侧边栏')}
                            >
                                <ChevronRight size={16} />
                            </button>
                        )}
                    </div>

                    {/* Right: nav buttons + menu */}
                    <div className="flex items-center gap-1">
                        <button
                            className="btn btn-ghost btn-sm gap-1.5 rounded-lg"
                            onClick={() => navigate('/cron-jobs')}
                        >
                            <Clock size={14} />
                            <span className="text-xs">{t('nav.cron_jobs', '定时任务')}</span>
                        </button>
                        <button
                            className="btn btn-ghost btn-sm gap-1.5 rounded-lg"
                            onClick={() => navigate('/skills')}
                        >
                            <Blocks size={14} />
                            <span className="text-xs">{t('nav.skills', '技能')}</span>
                        </button>

                        {/* Theme toggle */}
                        <button
                            className="btn btn-ghost btn-sm btn-square rounded-lg"
                            onClick={toggleTheme}
                            title={isDark ? 'Light mode' : 'Dark mode'}
                        >
                            {isDark ? <Sun size={14} /> : <Moon size={14} />}
                        </button>

                        {/* Dropdown menu */}
                        <div className="relative" ref={menuRef}>
                            <button
                                className="btn btn-ghost btn-sm btn-square rounded-lg"
                                onClick={() => setMenuOpen(!menuOpen)}
                            >
                                <MoreVertical size={16} />
                            </button>
                            {menuOpen && (
                                <div className="absolute right-0 top-full mt-1 w-44 bg-base-100 rounded-xl shadow-lg border border-base-200 py-1 z-50">
                                    <button
                                        className="flex items-center gap-3 w-full px-4 py-2.5 text-sm hover:bg-base-200/60 transition-colors text-left"
                                        onClick={() => { setMenuOpen(false); navigate('/settings'); }}
                                    >
                                        <SettingsIcon size={15} className="text-base-content/50" />
                                        {t('nav.settings', '设置')}
                                    </button>
                                    <button
                                        className="flex items-center gap-3 w-full px-4 py-2.5 text-sm hover:bg-base-200/60 transition-colors text-left"
                                        onClick={() => { setMenuOpen(false); navigate('/logs'); }}
                                    >
                                        <Info size={15} className="text-base-content/50" />
                                        {t('nav.logs', '日志')}
                                    </button>
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                {/* Chat content */}
                {!activeSession || activeSession.messages.length === 0 ? (
                    /* Welcome / empty state */
                    <div className="flex-1 flex items-center justify-center px-6">
                        <div className="text-center max-w-lg">
                            <h1 className="text-2xl font-bold bg-gradient-to-r from-violet-600 to-blue-500 bg-clip-text text-transparent mb-3">
                                {t('chat.welcome', 'Hi, how can I help you?')}
                            </h1>
                            <p className="text-sm text-base-content/40 mb-8">
                                {t('chat.welcome_desc', 'System management, automation, and more — all through natural language.')}
                            </p>

                            {/* Input card in center */}
                            <div className="bg-base-100 border border-base-200/60 rounded-2xl shadow-sm p-4">
                                <textarea
                                    className="w-full bg-transparent resize-none border-0 outline-none text-sm placeholder:text-base-content/30 min-h-[80px]"
                                    placeholder={t('chat.input_placeholder', 'Ask me anything...')}
                                    value={input}
                                    onChange={(e) => setInput(e.target.value)}
                                    onKeyDown={handleKeyDown}
                                    rows={3}
                                />
                                <div className="flex items-center justify-between mt-2">
                                    <div className="text-xs text-base-content/30">
                                        {t('chat.ai_disclaimer', 'Content is generated by AI for reference only.')}
                                    </div>
                                    <button
                                        className="btn btn-primary btn-sm btn-circle"
                                        onClick={handleSend}
                                        disabled={!input.trim() || loading.chat}
                                    >
                                        <Send size={14} />
                                    </button>
                                </div>
                            </div>

                            {/* Quick hints */}
                            <div className="mt-4 flex flex-wrap justify-center gap-2">
                                {[
                                    t('chat.hint_weather', '今天天气怎么样'),
                                    t('chat.hint_news', '帮我搜索新闻'),
                                    t('chat.hint_sysinfo', '查看系统信息'),
                                    t('chat.hint_script', '写一个 Python 脚本'),
                                ].map((hint) => (
                                    <button
                                        key={hint}
                                        className="btn btn-ghost btn-xs rounded-full border border-base-200/60 text-base-content/50 hover:text-base-content hover:border-base-300"
                                        onClick={() => {
                                            const sid = activeChatId || createChatSession();
                                            setInput('');
                                            sendMessage(sid, hint);
                                        }}
                                    >
                                        {hint}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </div>
                ) : (
                    <>
                        {/* Messages */}
                        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
                            {activeSession.messages.map((msg) => (
                                <div
                                    key={msg.id}
                                    className={`flex gap-3 ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
                                >
                                    {msg.role !== 'user' && (
                                        <div className="w-8 h-8 rounded-full bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center shrink-0 mt-0.5">
                                            <Bot size={15} className="text-white" />
                                        </div>
                                    )}
                                    <div
                                        className={`max-w-[70%] rounded-2xl px-4 py-3 text-sm leading-relaxed ${msg.role === 'user'
                                            ? 'bg-primary text-primary-content'
                                            : 'bg-base-200/60 text-base-content'
                                            }`}
                                    >
                                        <div className="prose prose-sm dark:prose-invert max-w-none break-words [&_pre]:bg-base-300/50 [&_pre]:rounded-lg [&_pre]:p-3 [&_pre]:overflow-x-auto [&_code]:text-xs [&_p]:my-1 [&_ul]:my-1 [&_ol]:my-1 [&_li]:my-0.5 [&_h1]:text-lg [&_h2]:text-base [&_h3]:text-sm [&_table]:text-xs">
                                            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                                                {msg.content}
                                            </ReactMarkdown>
                                        </div>

                                        {/* Tool calls */}
                                        {msg.toolCalls && msg.toolCalls.length > 0 && (
                                            <div className="mt-2 space-y-1.5">
                                                {msg.toolCalls.map((tc: any, i: number) => (
                                                    <details key={i} className="group rounded-lg bg-base-300/30 overflow-hidden">
                                                        <summary className="flex items-center gap-2 text-xs px-3 py-2 cursor-pointer select-none hover:bg-base-300/50 transition-colors">
                                                            <ChevronRight size={12} className="group-open:rotate-90 transition-transform text-base-content/40 shrink-0" />
                                                            <Wrench size={12} className="text-violet-500 shrink-0" />
                                                            <span className="font-mono font-medium">{tc.name}</span>
                                                            <span className={`ml-auto badge badge-xs ${tc.status === 'done' ? 'badge-success' : 'badge-error'}`}>
                                                                {tc.status === 'done' ? '✓' : '✗'}
                                                            </span>
                                                        </summary>
                                                        <div className="px-3 pb-2 text-xs font-mono text-base-content/70 whitespace-pre-wrap max-h-40 overflow-y-auto border-t border-base-300/50">
                                                            {tc.result?.slice(0, 500) || '(no result)'}
                                                            {tc.result && tc.result.length > 500 && <span className="text-base-content/30">...</span>}
                                                        </div>
                                                    </details>
                                                ))}
                                            </div>
                                        )}

                                        {/* Confirm buttons */}
                                        {msg.pendingConfirm && (
                                            <div className="mt-3 flex gap-2">
                                                <button
                                                    className="btn btn-sm btn-success gap-1"
                                                    onClick={() => confirmToolExecution(activeChatId!, msg.id)}
                                                >
                                                    <Check size={14} />{t('chat.confirm', '确认')}
                                                </button>
                                                <button className="btn btn-sm btn-ghost gap-1">
                                                    <X size={14} />{t('chat.cancel', '取消')}
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                    {msg.role === 'user' && (
                                        <div className="w-8 h-8 rounded-full bg-gradient-to-br from-blue-500 to-teal-500 flex items-center justify-center shrink-0 mt-0.5">
                                            <User size={15} className="text-white" />
                                        </div>
                                    )}
                                </div>
                            ))}
                            {loading.chat && (
                                <div className="flex gap-3 items-start">
                                    <div className="w-8 h-8 rounded-full bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center shrink-0">
                                        <Bot size={15} className="text-white" />
                                    </div>
                                    <div className="bg-base-200/60 rounded-2xl px-4 py-3">
                                        <span className="loading loading-dots loading-sm"></span>
                                    </div>
                                </div>
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Bottom input */}
                        <div className="px-6 pb-4 pt-2">
                            <div className="flex gap-2 items-end max-w-4xl mx-auto bg-base-100 border border-base-200/60 rounded-2xl px-4 py-3 shadow-sm">
                                <textarea
                                    className="flex-1 bg-transparent resize-none border-0 outline-none text-sm placeholder:text-base-content/30 min-h-[24px] max-h-32"
                                    placeholder={t('chat.input_placeholder', 'Ask me anything...')}
                                    value={input}
                                    onChange={(e) => setInput(e.target.value)}
                                    onKeyDown={handleKeyDown}
                                    rows={1}
                                />
                                <button
                                    className="btn btn-primary btn-sm btn-circle shrink-0"
                                    onClick={handleSend}
                                    disabled={!input.trim() || loading.chat}
                                >
                                    <Send size={14} />
                                </button>
                            </div>
                            <div className="text-center mt-2">
                                <span className="text-xs text-base-content/25">
                                    {t('chat.ai_disclaimer', 'Content is generated by AI for reference only.')}
                                </span>
                            </div>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}

export default AIChat;
