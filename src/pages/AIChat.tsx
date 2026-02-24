import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    Bot,
    Check,
    ChevronRight,
    MessageSquare,
    Plus,
    Send,
    Sparkles,
    Trash2,
    User,
    Wrench,
    X,
} from 'lucide-react';
import { useDevOpsStore } from '../stores/useDevOpsStore';

function AIChat() {
    const { t } = useTranslation();
    const {
        chatSessions,
        activeChatId,
        aiProviders,
        loading,
        createChatSession,
        deleteChatSession,
        setActiveChatId,
        sendMessage,
        confirmToolExecution,
    } = useDevOpsStore();

    const [input, setInput] = useState('');
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const activeSession = chatSessions.find((s) => s.id === activeChatId);
    const hasProvider = aiProviders.some((p) => p.enabled && p.apiKey);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [activeSession?.messages]);

    const handleSend = async () => {
        if (!input.trim() || !activeChatId) return;
        const msg = input.trim();
        setInput('');
        await sendMessage(activeChatId, msg);
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    return (
        <div className="flex h-full overflow-hidden">
            {/* Sidebar */}
            <div className="w-64 border-r border-base-200 bg-base-100 flex flex-col shrink-0">
                <div className="p-3 border-b border-base-200">
                    <button
                        className="btn btn-primary btn-sm w-full gap-2"
                        onClick={() => createChatSession()}
                    >
                        <Plus size={16} />
                        {t('chat.new_session', '新对话')}
                    </button>
                </div>
                <div className="flex-1 overflow-y-auto">
                    {chatSessions.length === 0 ? (
                        <div className="p-4 text-center text-base-content/40 text-sm">
                            {t('chat.no_sessions', '暂无对话')}
                        </div>
                    ) : (
                        chatSessions.map((session) => (
                            <div
                                key={session.id}
                                className={`flex items-center justify-between px-3 py-2.5 cursor-pointer hover:bg-base-200 transition-colors group ${activeChatId === session.id ? 'bg-base-200 border-r-2 border-primary' : ''
                                    }`}
                                onClick={() => setActiveChatId(session.id)}
                            >
                                <div className="flex items-center gap-2 min-w-0 flex-1">
                                    <MessageSquare size={14} className="shrink-0 text-base-content/40" />
                                    <span className="text-sm truncate text-base-content">
                                        {session.title}
                                    </span>
                                </div>
                                <button
                                    className="opacity-0 group-hover:opacity-100 btn btn-ghost btn-xs"
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

            {/* Main Chat Area */}
            <div className="flex-1 flex flex-col">
                {!activeSession ? (
                    <div className="flex-1 flex items-center justify-center">
                        <div className="text-center">
                            <div className="p-4 rounded-2xl bg-gradient-to-br from-violet-500/10 to-blue-500/10 inline-block mb-4">
                                <Sparkles size={48} className="text-violet-500" />
                            </div>
                            <h2 className="text-xl font-semibold text-base-content mb-2">
                                {t('chat.welcome', 'helix 智能助手')}
                            </h2>
                            <p className="text-sm text-base-content/50 max-w-md">
                                {t('chat.welcome_desc', '我是你的全能 AI 助手，可以帮你搜索信息、管理文件、执行命令、浏览网页，还能通过微信和你互动。')}
                            </p>
                            <div className="mt-4 flex flex-wrap justify-center gap-2 max-w-lg mx-auto">
                                {['今天天气怎么样', '帮我搜索最近的科技新闻', '查看系统信息', '帮我写一个 Python 脚本'].map((hint) => (
                                    <button key={hint} className="btn btn-ghost btn-xs bg-base-200 text-base-content/60"
                                        onClick={() => {
                                            const sid = createChatSession();
                                            sendMessage(sid, hint);
                                        }}>
                                        {hint}
                                    </button>
                                ))}
                            </div>
                            {!hasProvider && (
                                <div className="alert alert-warning mt-4 max-w-md mx-auto">
                                    <span className="text-sm">
                                        {t('chat.no_provider', '⚠️ 请先在设置中配置 AI 提供商并填入 API Key')}
                                    </span>
                                </div>
                            )}
                            <button
                                className="btn btn-primary mt-4 gap-2"
                                onClick={() => createChatSession()}
                            >
                                <Plus size={16} />
                                {t('chat.start', '开始对话')}
                            </button>
                        </div>
                    </div>
                ) : (
                    <>
                        {/* Messages */}
                        <div className="flex-1 overflow-y-auto p-6 space-y-4">
                            {activeSession.messages.length === 0 && (
                                <div className="text-center py-12 text-base-content/30">
                                    <Bot size={40} className="mx-auto mb-3" />
                                    <p>{t('chat.empty_hint', '发送消息开始对话')}</p>
                                    <p className="text-xs mt-1">试试：「帮我搜索新闻」或「查看系统信息」</p>
                                </div>
                            )}
                            {activeSession.messages.map((msg) => (
                                <div
                                    key={msg.id}
                                    className={`flex gap-3 ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
                                >
                                    {msg.role !== 'user' && (
                                        <div className="w-8 h-8 rounded-full bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center shrink-0">
                                            <Bot size={16} className="text-white" />
                                        </div>
                                    )}
                                    <div
                                        className={`max-w-[70%] rounded-2xl px-4 py-3 text-sm leading-relaxed ${msg.role === 'user'
                                            ? 'bg-primary text-primary-content'
                                            : 'bg-base-200 text-base-content'
                                            }`}
                                    >
                                        <div className="whitespace-pre-wrap">{msg.content}</div>

                                        {/* Tool execution results — collapsible */}
                                        {msg.toolCalls && msg.toolCalls.length > 0 && (
                                            <div className="mt-2 space-y-1.5">
                                                {msg.toolCalls.map((tc: any, i: number) => (
                                                    <details key={i} className="group rounded-lg bg-base-300/30 overflow-hidden">
                                                        <summary className="flex items-center gap-2 text-xs px-3 py-2 cursor-pointer select-none hover:bg-base-300/50 transition-colors">
                                                            <ChevronRight size={12} className="group-open:rotate-90 transition-transform text-base-content/40 shrink-0" />
                                                            <Wrench size={12} className="text-violet-500 shrink-0" />
                                                            <span className="font-mono font-medium">{tc.name}</span>
                                                            <span className={`ml-auto badge badge-xs ${tc.status === 'done' ? 'badge-success' : 'badge-error'}`}>
                                                                {tc.status === 'done' ? '成功' : '失败'}
                                                            </span>
                                                        </summary>
                                                        <div className="px-3 pb-2 text-xs font-mono text-base-content/70 whitespace-pre-wrap max-h-40 overflow-y-auto border-t border-base-300/50">
                                                            {tc.result?.slice(0, 500) || '(无结果)'}
                                                            {tc.result && tc.result.length > 500 && <span className="text-base-content/30">...（已截断）</span>}
                                                        </div>
                                                    </details>
                                                ))}
                                            </div>
                                        )}

                                        {/* Confirmation buttons for dangerous ops */}
                                        {msg.pendingConfirm && (
                                            <div className="mt-3 flex gap-2">
                                                <button
                                                    className="btn btn-sm btn-success gap-1"
                                                    onClick={() => confirmToolExecution(activeChatId!, msg.id)}
                                                >
                                                    <Check size={14} />确认执行
                                                </button>
                                                <button className="btn btn-sm btn-ghost gap-1">
                                                    <X size={14} />取消
                                                </button>
                                            </div>
                                        )}

                                        {msg.model && (
                                            <div className="text-xs opacity-50 mt-1">{msg.model}</div>
                                        )}
                                    </div>
                                    {msg.role === 'user' && (
                                        <div className="w-8 h-8 rounded-full bg-gradient-to-br from-blue-500 to-teal-500 flex items-center justify-center shrink-0">
                                            <User size={16} className="text-white" />
                                        </div>
                                    )}
                                </div>
                            ))}
                            {loading.chat && (
                                <div className="flex gap-3 items-center">
                                    <div className="w-8 h-8 rounded-full bg-gradient-to-br from-violet-500 to-blue-500 flex items-center justify-center">
                                        <Bot size={16} className="text-white" />
                                    </div>
                                    <div className="bg-base-200 rounded-2xl px-4 py-3">
                                        <span className="loading loading-dots loading-sm"></span>
                                    </div>
                                </div>
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Input */}
                        <div className="p-4 border-t border-base-200 bg-base-100">
                            <div className="flex gap-2 items-end max-w-4xl mx-auto">
                                <textarea
                                    className="textarea textarea-bordered flex-1 resize-none min-h-[44px] max-h-32"
                                    placeholder={t('chat.input_placeholder', '输入指令...（智能对话，自然语言指令）')}
                                    value={input}
                                    onChange={(e) => setInput(e.target.value)}
                                    onKeyDown={handleKeyDown}
                                    rows={1}
                                />
                                <button
                                    className="btn btn-primary btn-sm"
                                    onClick={handleSend}
                                    disabled={!input.trim() || loading.chat}
                                >
                                    <Send size={16} />
                                </button>
                            </div>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}

export default AIChat;
