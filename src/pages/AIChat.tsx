import { useEffect, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import 'highlight.js/styles/github-dark.min.css';
import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    type DragEndEvent
} from '@dnd-kit/core';
import {
    SortableContext,
    sortableKeyboardCoordinates,
    verticalListSortingStrategy,
    useSortable
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import {
    Bot,
    Check,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    Copy,
    FolderOpen,
    ImagePlus,
    MessageSquareMore,
    MoreHorizontal,
    Plus,
    Pin,
    RefreshCw,
    Search,
    Smile,
    Sparkles,
    Square,
    User,
    Wrench,
    X,
    Download
} from 'lucide-react';
import { useDevOpsStore, AIProvider } from '../stores/useDevOpsStore';
import { useConfigStore } from '../stores/useConfigStore';
import { AvatarPicker } from '../components/common/AvatarPicker';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';

// Models that support image input (from provider config modalities)
const IMAGE_CAPABLE_MODELS = new Set(['qwen3.5-plus', 'kimi-k2.5']);

// Fetch models from Rust backend (handles API call, Ollama, and built-in fallbacks)
async function fetchModelsFromProvider(provider: AIProvider): Promise<string[]> {
    try {
        const result = await invoke<{ models: string[] }>('ai_list_models', {
            baseUrl: (provider.baseUrl || '').replace(/\/$/, ''),
            apiKey: provider.apiKey || '',
        });
        return result.models || [];
    } catch {
        return [];
    }
}

const MIN_SIDEBAR = 180;
const MAX_SIDEBAR = 380;

// Window-level event buffer for agent progress (survives unmount, HMR, StrictMode)
declare global {
    interface Window {
        __helix_agent_status: string[];
        __helix_listeners_registered?: boolean;
    }
}
if (!window.__helix_agent_status) window.__helix_agent_status = [];

if (!window.__helix_listeners_registered) {
    window.__helix_listeners_registered = true;
    import('@tauri-apps/api/event').then(({ listen }) => {
        listen('agent-progress', (event: any) => {
            const { type, data } = event.payload;
            if (type === 'thinking') {
                const msg = i18n.t('chat.thinking', { model: data.model, defaultValue: `🤔 思考中... (模型: ${data.model})` });
                const arr = window.__helix_agent_status;
                if (arr[arr.length - 1] !== msg) arr.push(msg);
            } else if (type === 'tool_call') {
                window.__helix_agent_status.push(i18n.t('chat.tool_calling', { name: data.name, defaultValue: `🔧 调用工具: ${data.name}` }));
            } else if (type === 'tool_result') {
                window.__helix_agent_status.push(i18n.t('chat.tool_done', { name: data.name, chars: data.chars, defaultValue: `✅ ${data.name} 完成 (${data.chars} 字符)` }));
            } else if (type === 'done' || type === 'cancelled') {
                window.__helix_agent_status = [];
            }
            window.dispatchEvent(new Event('helix:update'));
        });
    });
}

function AIChat() {
    const { t } = useTranslation();
    const { config } = useConfigStore();
    const {
        chatSessions,
        activeChatId,
        createChatSession,
        deleteChatSession,
        setActiveChatId,
        sendMessage,
        confirmToolExecution,
        updateChatSession,
        togglePinChatSession,
        aiProviders,
        loading,
    } = useDevOpsStore();

    const isSessionLoading = !!useDevOpsStore(s => s.loading[`chat-${activeChatId}`]);

    const [input, setInput] = useState('');
    const [pendingImages, setPendingImages] = useState<string[]>([]);
    const [searchQuery, setSearchQuery] = useState('');
    const [sidebarWidth, setSidebarWidth] = useState(240);
    const [isDragging, setIsDragging] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const dragStartX = useRef(0);
    const dragStartWidth = useRef(0);
    const activeSession = chatSessions.find((s) => s.id === activeChatId);

    // Model picker
    const [showProviderMenu, setShowProviderMenu] = useState(false);
    const [showModelMenu, setShowModelMenu] = useState(false);
    const [fetchedModels, setFetchedModels] = useState<string[]>([]);
    const [fetchingModels, setFetchingModels] = useState(false);
    const providerMenuRef = useRef<HTMLDivElement>(null);
    const modelMenuRef = useRef<HTMLDivElement>(null);

    const activeGlobalProvider = aiProviders.find((p) => p.enabled) ?? null;
    const currentSessionProvider = (activeSession?.provider ? aiProviders.find(p => p.id === activeSession.provider) : activeGlobalProvider) ?? activeGlobalProvider;
    const currentModel = activeSession?.model || currentSessionProvider?.defaultModel || '';
    const supportsImages = IMAGE_CAPABLE_MODELS.has(currentModel);

    // Display: fetched models, always include currentModel at top if not in list
    const displayModels = currentModel && !fetchedModels.includes(currentModel)
        ? [currentModel, ...fetchedModels]
        : fetchedModels.length > 0 ? fetchedModels : (currentModel ? [currentModel] : []);

    // Agent progress — sync from window-level buffer
    const [agentStatus, setAgentStatus] = useState<string[]>(() => [...window.__helix_agent_status]);

    // Avatar Picker State
    const [showAvatarPicker, setShowAvatarPicker] = useState(false);
    const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
    const [editingSessionTitle, setEditingSessionTitle] = useState('');
    const [contextMenu, setContextMenu] = useState<{ x: number, y: number, sessionId: string } | null>(null);
    const contextMenuRef = useRef<HTMLDivElement>(null);
    const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null);

    // Setup DnD sensors
    const sensors = useSensors(
        useSensor(PointerSensor, {
            activationConstraint: {
                distance: 5, // minimum drag distance before taking effect, protects click events
            },
        }),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    const handleDragEnd = (event: DragEndEvent) => {
        const { active, over } = event;
        if (over && active.id !== over.id) {
            const oldIndex = chatSessions.findIndex((s) => s.id === active.id);
            const newIndex = chatSessions.findIndex((s) => s.id === over.id);
            useDevOpsStore.getState().reorderChatSessions(oldIndex, newIndex);
        }
    };

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [activeSession?.messages, agentStatus]);

    // Clear agent status when switching conversations
    useEffect(() => {
        window.__helix_agent_status = [];
        setAgentStatus([]);
    }, [activeChatId]);

    // Sync agent status from window buffer on mount and on every update event
    useEffect(() => {
        const sync = () => {
            setAgentStatus([...window.__helix_agent_status]);
        };
        sync();
        window.addEventListener('helix:update', sync);
        return () => window.removeEventListener('helix:update', sync);
    }, []);

    // Auto-fetch models from API when provider changes
    useEffect(() => {
        if (!currentSessionProvider?.baseUrl) { setFetchedModels([]); return; }
        let cancelled = false;
        setFetchingModels(true);
        fetchModelsFromProvider(currentSessionProvider).then((models) => {
            if (!cancelled) { setFetchedModels(models); setFetchingModels(false); }
        });
        return () => { cancelled = true; };
    }, [currentSessionProvider?.id, currentSessionProvider?.baseUrl, currentSessionProvider?.apiKey]);

    // Close menus on outside click
    useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (providerMenuRef.current && !providerMenuRef.current.contains(e.target as Node)) setShowProviderMenu(false);
            if (modelMenuRef.current && !modelMenuRef.current.contains(e.target as Node)) setShowModelMenu(false);
            if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) setContextMenu(null);
            else if (!contextMenuRef.current) setContextMenu(null);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

    // Context Menu Handlers
    const handleContextMenu = (e: React.MouseEvent, sessionId: string) => {
        e.preventDefault();
        setContextMenu({ x: e.clientX, y: e.clientY, sessionId });
    };

    // ── Resizable divider ─────────────────────────────────
    const handleDividerMouseDown = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        dragStartX.current = e.clientX;
        dragStartWidth.current = sidebarWidth;
        setIsDragging(true);
    }, [sidebarWidth]);

    useEffect(() => {
        if (!isDragging) return;
        const onMove = (e: MouseEvent) => {
            const delta = e.clientX - dragStartX.current;
            setSidebarWidth(Math.max(MIN_SIDEBAR, Math.min(MAX_SIDEBAR, dragStartWidth.current + delta)));
        };
        const onUp = () => setIsDragging(false);
        document.addEventListener('mousemove', onMove);
        document.addEventListener('mouseup', onUp);
        return () => {
            document.removeEventListener('mousemove', onMove);
            document.removeEventListener('mouseup', onUp);
        };
    }, [isDragging]);

    // ── Input helpers ─────────────────────────────────────
    const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
        setInput(e.target.value);
        const el = textareaRef.current;
        if (el) { el.style.height = 'auto'; el.style.height = Math.min(el.scrollHeight, 160) + 'px'; }
    };

    const handleSend = async () => {
        if ((!input.trim() && pendingImages.length === 0) || isSessionLoading) return;
        setAgentStatus([]);
        const msg = input.trim();
        const imgs = [...pendingImages];
        setInput('');
        setPendingImages([]);
        if (textareaRef.current) textareaRef.current.style.height = 'auto';
        let sid = activeChatId;
        if (!sid) sid = createChatSession();
        await sendMessage(sid!, msg || '(图片)', imgs.length > 0 ? imgs : undefined);
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend(); }
    };


    const handleFileUpload = () => fileInputRef.current?.click();

    const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const files = e.target.files;
        if (!files || files.length === 0) return;
        Array.from(files).forEach(file => {
            if (!file.type.startsWith('image/')) return;
            const reader = new FileReader();
            reader.onload = () => {
                if (typeof reader.result === 'string') {
                    setPendingImages(prev => [...prev, reader.result as string]);
                }
            };
            reader.readAsDataURL(file);
        });
        e.target.value = '';
    };

    const filteredSessions = chatSessions
        .filter((s) => !searchQuery || s.title.toLowerCase().includes(searchQuery.toLowerCase()))
        .sort((a, b) => {
            if (a.pinned && !b.pinned) return -1;
            if (!a.pinned && b.pinned) return 1;
            return 0;
        });

    const getLastMessage = (session: typeof chatSessions[0]) => {
        const last = session.messages[session.messages.length - 1];
        if (!last) return '';
        return last.content.length > 38 ? last.content.slice(0, 38) + '…' : last.content;
    };

    const getLastTime = (session: typeof chatSessions[0]) => {
        const last = session.messages[session.messages.length - 1];
        if (!last?.timestamp) return '';
        const d = new Date(last.timestamp);
        const now = new Date();
        if (d.toDateString() === now.toDateString())
            return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false });
        return d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' });
    };

    return (
        <>
            {/* Hidden file input */}
            <input ref={fileInputRef} type="file" multiple accept="image/*" className="hidden" onChange={handleFileChange} />

            {/* ── Session list ─────────────────────────────────────── */}
            <div
                className="shrink-0 bg-[#f7f7f7] dark:bg-[#252525] flex flex-col border-r border-black/5 dark:border-white/5"
                style={{ width: sidebarWidth }}
                data-tauri-drag-region
            >
                <div className="px-3 pt-3 pb-2 flex items-center gap-2" data-tauri-drag-region style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}>
                    <div className="flex-1 relative">
                        <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none" />
                        <input
                            type="text"
                            className="w-full pl-7 pr-3 py-1.5 text-xs bg-black/5 dark:bg-white/5 rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400"
                            placeholder={t('chat.search', '搜索')}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                        />
                    </div>
                    <button
                        className="w-7 h-7 rounded-md flex items-center justify-center text-gray-500 hover:bg-black/8 dark:hover:bg-white/10 transition-colors shrink-0"
                        onClick={() => createChatSession()}
                        title={t('chat.new_session', '新对话')}
                    >
                        <Plus size={15} />
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto">
                    {filteredSessions.length === 0 ? (
                        <div className="px-4 py-12 text-center text-gray-400 text-xs">{t('chat.no_sessions', '暂无对话')}</div>
                    ) : (
                        <DndContext
                            sensors={sensors}
                            collisionDetection={closestCenter}
                            onDragEnd={handleDragEnd}
                        >
                            <SortableContext
                                items={filteredSessions.map(s => s.id)}
                                strategy={verticalListSortingStrategy}
                            >
                                {filteredSessions.map((session) => (
                                    <ChatSessionItem
                                        key={session.id}
                                        session={session}
                                        activeChatId={activeChatId}
                                        contextMenu={contextMenu}
                                        editingSessionId={editingSessionId}
                                        editingSessionTitle={editingSessionTitle}
                                        setActiveChatId={setActiveChatId}
                                        handleContextMenu={handleContextMenu}
                                        setEditingSessionId={setEditingSessionId}
                                        setEditingSessionTitle={setEditingSessionTitle}
                                        updateChatSession={updateChatSession}
                                        getLastTime={getLastTime}
                                        getLastMessage={getLastMessage}
                                        t={t}
                                        loading={loading}
                                    />
                                ))}
                            </SortableContext>
                        </DndContext>
                    )}
                </div>
            </div>

            {/* Context Menu overlay */}
            {contextMenu && (
                <div
                    ref={contextMenuRef}
                    className="fixed z-[100] min-w-[124px] bg-white dark:bg-[#2b2b2b] rounded-lg shadow-[0_5px_15px_rgba(0,0,0,0.1)] dark:shadow-[0_5px_15px_rgba(0,0,0,0.3)] ring-1 ring-black/5 dark:ring-white/5 py-1.5 px-1.5 overflow-hidden flex flex-col gap-0.5"
                    style={{ left: contextMenu.x, top: contextMenu.y }}
                    onContextMenu={(e) => e.preventDefault()}
                >
                    <button
                        className="w-full text-left px-3 py-2 rounded-md text-[13px] text-gray-800 dark:text-gray-200 hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                        onClick={() => {
                            togglePinChatSession(contextMenu.sessionId);
                            setContextMenu(null);
                        }}
                    >
                        {chatSessions.find(s => s.id === contextMenu.sessionId)?.pinned ? t('chat.unpin', '取消置顶') : t('chat.pin', '置顶')}
                    </button>
                    <button
                        className="w-full text-left px-3 py-2 rounded-md text-[13px] text-gray-800 dark:text-gray-200 hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                        onClick={() => {
                            const session = chatSessions.find(s => s.id === contextMenu.sessionId);
                            if (session) {
                                setEditingSessionId(session.id);
                                setEditingSessionTitle(session.title);
                            }
                            setContextMenu(null);
                        }}
                    >
                        {t('chat.rename', '重命名')}
                    </button>
                    <div className="mx-3 my-0.5 h-px bg-black/5 dark:bg-white/5 shrink-0" />
                    <button
                        className="w-full text-left px-3 py-2 rounded-md text-[13px] text-[#ff4d4f] hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                        onClick={() => {
                            if (window.confirm(t('chat.confirm_delete', '确定要删除此对话吗？'))) {
                                deleteChatSession(contextMenu.sessionId);
                            }
                            setContextMenu(null);
                        }}
                    >
                        {t('chat.delete', '删除')}
                    </button>
                </div>
            )}

            {/* ── Resizable divider ────────────────────────────────── */}
            <div
                className={`w-[4px] shrink-0 cursor-col-resize group relative z-10 ${isDragging ? 'bg-[#07c160]/30' : ''}`}
                onMouseDown={handleDividerMouseDown}
            >
                {/* Wider invisible hit area */}
                <div className="absolute inset-y-0 -left-1.5 -right-1.5 group-hover:bg-[#07c160]/20 transition-colors" />
            </div>

            {/* ── Chat area ────────────────────────────────────────── */}
            <div className={`flex-1 flex flex-col min-w-0 bg-[#ededed] dark:bg-[#1a1a1a] ${isDragging ? 'select-none' : ''}`}>
                {!activeSession ? (
                    <div
                        className="flex-1 flex flex-col"
                        data-tauri-drag-region
                        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
                    >
                        {/* Top drag bar */}
                        <div className="h-12 shrink-0" data-tauri-drag-region style={{ WebkitAppRegion: 'drag' } as React.CSSProperties} />
                        <div className="flex-1 flex items-center justify-center" style={{ WebkitAppRegion: 'no-drag', pointerEvents: 'auto' } as React.CSSProperties}>
                            <div className="text-center">
                                <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-[#07c160] to-[#05a050] flex items-center justify-center mx-auto mb-4 shadow-lg">
                                    <Sparkles size={30} className="text-white" />
                                </div>
                                <h2 className="text-base font-medium text-gray-600 dark:text-gray-400 mb-1">{t('chat.welcome', 'Helix 智能助手')}</h2>
                                <p className="text-xs text-gray-400">{t('chat.welcome_desc', '选择一个对话或开始新对话')}</p>
                                <button className="mt-5 px-5 py-2 text-sm bg-[#07c160] hover:bg-[#06ad56] text-white rounded-full transition-colors shadow-sm" onClick={() => createChatSession()}>
                                    <Plus size={13} className="inline mr-1.5" />{t('chat.start', '开始对话')}
                                </button>
                            </div>
                        </div>
                    </div>
                ) : (
                    <>
                        {/* header — drag region */}
                        <div
                            className="h-12 px-5 flex items-center bg-[#f5f5f5] dark:bg-[#232323] border-b border-black/[0.06] dark:border-white/[0.06] shrink-0 select-none"
                            style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
                            data-tauri-drag-region
                        >
                            <div
                                className={`w-7 h-7 rounded-sm overflow-hidden shrink-0 flex items-center justify-center cursor-pointer hover:opacity-80 transition-opacity mr-3 ${activeSession.agentAvatarUrl ? 'bg-white dark:bg-[#404040] border border-black/5 dark:border-white/5' : 'bg-gradient-to-br from-[#07c160] to-[#05a050]'}`}
                                onClick={() => setShowAvatarPicker(true)}
                                title={t('chat.change_avatar', '更换助手头像')}
                                style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                            >
                                {activeSession.agentAvatarUrl ? (
                                    <img src={activeSession.agentAvatarUrl} alt="Agent" className="w-full h-full object-cover" />
                                ) : (
                                    <Bot size={16} className="text-white" />
                                )}
                            </div>
                            <h3 className="text-[13px] font-medium text-gray-800 dark:text-gray-200 truncate pointer-events-none">{activeSession.title}</h3>
                            {activeSession.workspace && (
                                <span className="text-[11px] text-gray-400 ml-2 flex items-center gap-1 pointer-events-none">
                                    <FolderOpen size={11} />{activeSession.workspace}
                                </span>
                            )}
                            <div className="flex-1" />
                            <div className="flex items-center gap-2.5 mr-3" style={{ WebkitAppRegion: 'no-drag', pointerEvents: 'auto' } as React.CSSProperties}>
                                <button className="p-1 rounded-md hover:bg-black/5 dark:hover:bg-white/5 transition-colors text-gray-700 dark:text-gray-300">
                                    <MessageSquareMore size={22} strokeWidth={1.5} />
                                </button>
                                <button className="px-2 py-1.5 rounded-md bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 transition-colors text-gray-800 dark:text-gray-200">
                                    <ChevronDown size={16} strokeWidth={2.5} />
                                </button>
                                <button className="p-1 rounded-md hover:bg-black/5 dark:hover:bg-white/5 transition-colors text-gray-800 dark:text-gray-200">
                                    <MoreHorizontal size={22} strokeWidth={2} />
                                </button>
                            </div>
                        </div>

                        {/* messages */}
                        <div className="flex-1 overflow-y-auto px-4 py-5 space-y-4">
                            {activeSession.messages.length === 0 && (
                                <div className="text-center py-12 text-gray-400">
                                    <Bot size={36} className="mx-auto mb-3 opacity-20" />
                                    <p className="text-xs">{t('chat.empty_hint', '发送消息开始对话')}</p>
                                </div>
                            )}
                            {activeSession.messages.map((msg) => (
                                <div key={msg.id} className={`flex gap-2.5 group/msg ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
                                    <div
                                        className={`w-9 h-9 rounded-full shrink-0 flex items-center justify-center mt-0.5 overflow-hidden shadow-sm ${msg.role === 'user'
                                            ? config?.appAvatarUrl ? 'bg-white dark:bg-[#404040]' : 'bg-white dark:bg-[#404040] border border-black/5 dark:border-white/5'
                                            : 'bg-white dark:bg-[#404040] border border-black/5 dark:border-white/5 cursor-pointer hover:shadow-md transition-shadow'
                                            }`}
                                        onClick={() => {
                                            if (msg.role !== 'user') setShowAvatarPicker(true);
                                        }}
                                        title={msg.role !== 'user' ? t('chat.change_avatar', '更换助手头像') : undefined}
                                    >
                                        {msg.role === 'user'
                                            ? config?.appAvatarUrl
                                                ? <img src={config.appAvatarUrl} alt="User" className="w-[85%] h-[85%] object-cover rounded-full" />
                                                : <User size={15} className="text-gray-700 dark:text-gray-300" />
                                            : activeSession.agentAvatarUrl
                                                ? <img src={activeSession.agentAvatarUrl} alt="Agent" className="w-[85%] h-[85%] object-cover rounded-full" />
                                                : <Bot size={15} className="text-[#07c160]" />
                                        }
                                    </div>
                                    <div className="max-w-[65%]">
                                        <div className={`rounded-xl px-3.5 py-2.5 text-[13px] leading-relaxed ${msg.role === 'user'
                                            ? 'bg-[#95ec69] dark:bg-[#3eb575] text-gray-900 dark:text-white rounded-tr-sm'
                                            : 'bg-white dark:bg-[#2c2c2c] text-gray-800 dark:text-gray-200 rounded-tl-sm shadow-sm'
                                            }`}>
                                            <div id={`msg-content-${msg.id}`} className="prose prose-sm dark:prose-invert max-w-none break-words [&_pre]:bg-gray-100 [&_pre]:dark:bg-gray-800 [&_pre]:rounded-lg [&_pre]:p-2.5 [&_pre]:overflow-x-auto [&_pre]:text-xs [&_code]:text-xs [&_p]:my-1 [&_ul]:my-1 [&_ol]:my-1 [&_li]:my-0.5 [&_h1]:text-sm [&_h2]:text-sm [&_h3]:text-sm">
                                                {msg.images && msg.images.length > 0 && (
                                                    <div className="flex gap-1.5 flex-wrap mb-2 not-prose">
                                                        {msg.images.map((img, i) => (
                                                            <img key={i} src={img} alt="" className="max-w-[200px] max-h-[200px] rounded-lg object-cover cursor-pointer hover:opacity-80 transition-opacity" onClick={() => window.open(img, '_blank')} />
                                                        ))}
                                                    </div>
                                                )}
                                                {msg.content !== '(图片)' && (() => {
                                                    // Parse __FILE_ATTACHMENT__ markers out of message content
                                                    const parts = msg.content.split(/(__FILE_ATTACHMENT__\{.*?\}(?=__|$))/s);
                                                    return parts.map((part, i) => {
                                                        if (part.startsWith('__FILE_ATTACHMENT__')) {
                                                            try {
                                                                const jsonStr = part.slice('__FILE_ATTACHMENT__'.length);
                                                                const att = JSON.parse(jsonStr);
                                                                return (
                                                                    <div key={i} className="not-prose my-2 flex items-center gap-3 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl px-4 py-3">
                                                                        <div className="text-2xl shrink-0">
                                                                            {att.mime?.startsWith('image/') ? '🖼️' : att.mime === 'application/pdf' ? '📄' : att.mime?.includes('zip') ? '📦' : '📁'}
                                                                        </div>
                                                                        <div className="flex-1 min-w-0">
                                                                            <div className="text-sm font-medium truncate text-gray-800 dark:text-gray-200">{att.name}</div>
                                                                            <div className="text-xs text-gray-500">{att.size}</div>
                                                                        </div>
                                                                        <button
                                                                            className="shrink-0 px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg transition-colors font-medium"
                                                                            onClick={() => {
                                                                                const a = document.createElement('a');
                                                                                a.href = `data:${att.mime};base64,${att.data}`;
                                                                                a.download = att.name;
                                                                                a.click();
                                                                            }}
                                                                        >
                                                                            {t('chat.download', '⬇ 下载')}
                                                                        </button>
                                                                    </div>
                                                                );
                                                            } catch { return null; }
                                                        }
                                                        return part.trim() ? <ReactMarkdown key={i} remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>{part}</ReactMarkdown> : null;
                                                    });
                                                })()}
                                            </div>
                                            {msg.toolCalls && msg.toolCalls.length > 0 && (
                                                <div className="mt-2 space-y-1">
                                                    {msg.toolCalls.map((tc: any, i: number) => (
                                                        <details key={i} className="group/tc rounded-lg bg-gray-50 dark:bg-gray-800/50 overflow-hidden">
                                                            <summary className="flex items-center gap-1.5 text-xs px-2 py-1.5 cursor-pointer select-none hover:bg-gray-100 dark:hover:bg-gray-700/50">
                                                                <ChevronRight size={10} className="group-open/tc:rotate-90 transition-transform text-gray-400 shrink-0" />
                                                                <Wrench size={10} className="text-[#07c160] shrink-0" />
                                                                <span className="font-mono">{tc.name}</span>
                                                                <span className={`ml-auto text-[10px] ${tc.status === 'done' ? 'text-green-500' : 'text-red-500'}`}>{tc.status === 'done' ? '✓' : '✗'}</span>
                                                            </summary>
                                                            <div className="px-2 pb-1.5 text-[11px] font-mono text-gray-500 whitespace-pre-wrap max-h-32 overflow-y-auto border-t border-gray-200 dark:border-gray-700">
                                                                {tc.result?.slice(0, 500) || '(no result)'}
                                                            </div>
                                                        </details>
                                                    ))}
                                                </div>
                                            )}
                                            {msg.pendingConfirm && (
                                                <div className="mt-2 flex gap-2">
                                                    <button className="px-3 py-1 text-xs bg-[#07c160] text-white rounded-full hover:bg-[#06ad56]" onClick={() => confirmToolExecution(activeChatId!, msg.id)}>
                                                        <Check size={11} className="inline mr-1" />{t('chat.confirm', '确认')}
                                                    </button>
                                                    <button className="px-3 py-1 text-xs bg-gray-200 dark:bg-gray-600 text-gray-600 dark:text-gray-300 rounded-full">
                                                        <X size={11} className="inline mr-1" />{t('chat.cancel', '取消')}
                                                    </button>
                                                </div>
                                            )}
                                            {msg.files && msg.files.length > 0 && (
                                                <div className="mt-2 space-y-2">
                                                    {msg.files.map((f, i) => (
                                                        <div key={i} className="flex items-center gap-3 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl px-4 py-3">
                                                            <div className="text-2xl shrink-0">
                                                                {f.mime?.startsWith('image/') ? '🖼️' : f.mime === 'application/pdf' ? '📄' : f.mime?.includes('zip') ? '📦' : '📁'}
                                                            </div>
                                                            <div className="flex-1 min-w-0">
                                                                <div className="text-sm font-medium truncate text-gray-800 dark:text-gray-200">{f.name}</div>
                                                                <div className="text-xs text-gray-500">{f.size}</div>
                                                            </div>
                                                            <button
                                                                className="shrink-0 px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg transition-colors font-medium flex items-center gap-1"
                                                                onClick={async () => {
                                                                    try {
                                                                        const { save } = await import('@tauri-apps/plugin-dialog');
                                                                        const dest = await save({ defaultPath: f.name });
                                                                        if (dest) {
                                                                            const { invoke } = await import('@tauri-apps/api/core');
                                                                            await invoke('save_file_to', { source: f.path, destination: dest });
                                                                        }
                                                                    } catch (e) { console.error('Save failed:', e); }
                                                                }}
                                                            >
                                                                {t('chat.save_as', '⬇ 另存为')}
                                                            </button>
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                            {/* Beautiful copy and PDF buttons integrated inside bottom right corner of agent message */}
                                            {msg.role !== 'user' && (
                                                <div className="flex justify-end mt-1.5 -mb-0.5 opacity-0 group-hover/msg:opacity-100 transition-opacity gap-1">
                                                    <button
                                                        className="flex items-center gap-1 px-1.5 py-1 text-[11px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/5 rounded transition-colors"
                                                        title={t('chat.export_pdf', '导出 PDF')}
                                                        onClick={async (e) => {
                                                            const btn = e.currentTarget;
                                                            const originalHTML = btn.innerHTML;
                                                            btn.innerHTML = `<span class="loading loading-spinner w-3 h-3 text-gray-400" align="center"></span><span style="margin-left:4px">导出中</span>`;
                                                            btn.disabled = true;
                                                            try {
                                                                const sourceElement = document.getElementById(`msg-content-${msg.id}`);
                                                                if (sourceElement) {
                                                                    // Create a hidden iframe for printing
                                                                    const iframe = document.createElement('iframe');
                                                                    iframe.style.position = 'absolute';
                                                                    iframe.style.width = '0px';
                                                                    iframe.style.height = '0px';
                                                                    iframe.style.border = 'none';
                                                                    document.body.appendChild(iframe);

                                                                    const doc = iframe.contentWindow?.document;
                                                                    if (doc) {
                                                                        // Get all stylesheets and styles from the parent document
                                                                        const styles = Array.from(document.styleSheets)
                                                                            .map(styleSheet => {
                                                                                try {
                                                                                    return Array.from(styleSheet.cssRules)
                                                                                        .map(rule => rule.cssText).join('');
                                                                                } catch (e) {
                                                                                    // Catch CORS issues with external stylesheets 
                                                                                    if (styleSheet.href) {
                                                                                        return `<link rel="stylesheet" href="${styleSheet.href}">`;
                                                                                    }
                                                                                    return '';
                                                                                }
                                                                            })
                                                                            .join('\n');

                                                                        // Capture all inline style tags
                                                                        const inlineStyles = Array.from(document.head.querySelectorAll('style'))
                                                                            .map(style => style.outerHTML)
                                                                            .join('\n');

                                                                        const isDark = document.documentElement.className.includes('dark');

                                                                        doc.open();
                                                                        doc.write(`
                                                                            <!DOCTYPE html>
                                                                            <html class="${isDark ? 'dark' : ''}">
                                                                            <head>
                                                                                <title>Chat Message Export - ${msg.id.substring(0, 6)}</title>
                                                                                <style>${styles}</style>
                                                                                ${inlineStyles}
                                                                                <style>
                                                                                    body { 
                                                                                        padding: 20px !important; 
                                                                                        margin: 0 !important; 
                                                                                        background: ${isDark ? '#1f2937' : '#ffffff'} !important;
                                                                                        color: ${isDark ? '#f3f4f6' : '#1f2937'} !important;
                                                                                    }
                                                                                    @media print {
                                                                                        @page { margin: 10mm; }
                                                                                        body { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
                                                                                    }
                                                                                </style>
                                                                            </head>
                                                                            <body>
                                                                                ${sourceElement.outerHTML}
                                                                            </body>
                                                                            </html>
                                                                        `);
                                                                        doc.close();

                                                                        // Wait for resources to load then print and cleanup
                                                                        setTimeout(() => {
                                                                            iframe.contentWindow?.focus();
                                                                            iframe.contentWindow?.print();
                                                                            setTimeout(() => {
                                                                                document.body.removeChild(iframe);
                                                                            }, 2000);
                                                                        }, 500);
                                                                    }
                                                                }
                                                            } catch (error) {
                                                                console.error('Failed to generate PDF:', error);
                                                            } finally {
                                                                btn.innerHTML = originalHTML;
                                                                btn.disabled = false;
                                                            }
                                                        }}
                                                    >
                                                        <><Download size={12} /> PDF</>
                                                    </button>
                                                    <button
                                                        className="flex items-center gap-1 px-1.5 py-1 text-[11px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/5 rounded transition-colors"
                                                        title={t('chat.copy_response', '复制回复')}
                                                        onClick={() => {
                                                            navigator.clipboard.writeText(msg.content);
                                                            setCopiedMessageId(msg.id);
                                                            setTimeout(() => setCopiedMessageId(null), 2000);
                                                        }}
                                                    >
                                                        {copiedMessageId === msg.id ? (
                                                            <><Check size={12} className="text-[#07c160]" /> <span className="text-[#07c160]">已复制</span></>
                                                        ) : (
                                                            <><Copy size={12} /> 复制</>
                                                        )}
                                                    </button>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))}
                            {isSessionLoading && (
                                <div className="flex gap-2.5">
                                    <div className={`w-9 h-9 rounded-full flex items-center justify-center shrink-0 mt-0.5 overflow-hidden shadow-sm bg-white dark:bg-[#404040] border border-black/5 dark:border-white/5`}>
                                        {activeSession.agentAvatarUrl ? (
                                            <img src={activeSession.agentAvatarUrl} alt="Agent" className="w-[85%] h-[85%] object-cover rounded-full" />
                                        ) : (
                                            <Bot size={15} className="text-[#07c160]" />
                                        )}
                                    </div>
                                    <div className="bg-white dark:bg-[#2c2c2c] rounded-xl rounded-tl-sm px-4 py-3 shadow-sm max-w-[65%]">
                                        {agentStatus.length > 0 ? (
                                            <details ref={el => { if (el && !el.hasAttribute('data-init')) { el.setAttribute('data-init', '1'); el.open = true; } }} className="text-xs text-gray-500 dark:text-gray-400 font-mono">
                                                <summary className="cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-300">
                                                    {agentStatus[agentStatus.length - 1]}
                                                    <span className="loading loading-dots loading-xs text-gray-400 ml-1" />
                                                </summary>
                                                <div className="mt-1 space-y-0.5 pl-3 border-l-2 border-gray-200 dark:border-gray-600">
                                                    {agentStatus.slice(0, -1).map((s, i) => (
                                                        <div key={i} className="opacity-60">{s}</div>
                                                    ))}
                                                </div>
                                            </details>
                                        ) : (
                                            <span className="loading loading-dots loading-sm text-gray-400" />
                                        )}
                                    </div>
                                </div>
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* ── Input zone ─────────────────────────────── */}
                        <div className="bg-[#f5f5f5] dark:bg-[#232323] border-t border-black/[0.06] dark:border-white/[0.06]">
                            {/* Toolbar row — icons above textarea, like WeChat */}
                            <div className="flex items-center gap-0.5 px-4 pt-2 pb-0">
                                <button className="w-7 h-7 flex items-center justify-center rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 hover:bg-black/5 dark:hover:bg-white/5 transition-colors" title={t('chat.emoji', '表情')}>
                                    <Smile size={17} />
                                </button>
                                {supportsImages && (
                                    <button
                                        className="w-7 h-7 flex items-center justify-center rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                                        title={t('chat.upload_image', '上传图片')}
                                        onClick={handleFileUpload}
                                    >
                                        <ImagePlus size={17} />
                                    </button>
                                )}
                            </div>

                            {/* Image preview thumbnails */}
                            {pendingImages.length > 0 && (
                                <div className="flex gap-2 px-4 pt-2 flex-wrap">
                                    {pendingImages.map((img, i) => (
                                        <div key={i} className="relative group">
                                            <img src={img} alt="" className="w-16 h-16 object-cover rounded-lg border border-black/10 dark:border-white/10" />
                                            <button
                                                onClick={() => setPendingImages(prev => prev.filter((_, idx) => idx !== i))}
                                                className="absolute -top-1.5 -right-1.5 w-5 h-5 bg-red-500 text-white rounded-full flex items-center justify-center text-xs opacity-0 group-hover:opacity-100 transition-opacity"
                                            >×</button>
                                        </div>
                                    ))}
                                </div>
                            )}

                            {/* Textarea — no border */}
                            <textarea
                                ref={textareaRef}
                                className="w-full bg-transparent border-0 outline-none resize-none text-[13px] text-gray-800 dark:text-gray-200 placeholder:text-gray-400 px-5 pt-2 pb-1 min-h-[56px] max-h-[160px]"
                                placeholder={t('chat.input_placeholder', '输入消息…')}
                                value={input}
                                onChange={handleInputChange}
                                onKeyDown={handleKeyDown}
                                onPaste={(e) => {
                                    if (!supportsImages) return;
                                    const items = e.clipboardData?.items;
                                    if (!items) return;
                                    for (const item of items) {
                                        if (item.type.startsWith('image/')) {
                                            e.preventDefault();
                                            const file = item.getAsFile();
                                            if (!file) continue;
                                            const reader = new FileReader();
                                            reader.onload = () => {
                                                if (typeof reader.result === 'string') {
                                                    setPendingImages(prev => [...prev, reader.result as string]);
                                                }
                                            };
                                            reader.readAsDataURL(file);
                                        }
                                    }
                                }}
                                rows={2}
                            />

                            {/* Bottom bar — provider / model picker + send */}
                            <div className="flex items-center gap-1.5 px-4 pb-3 pt-1">
                                {/* Provider picker */}
                                <div className="relative" ref={providerMenuRef}>
                                    <button
                                        className="flex items-center gap-1 text-[11px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 px-2 py-1 rounded-md hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                                        onClick={() => { setShowProviderMenu(!showProviderMenu); setShowModelMenu(false); }}
                                    >
                                        <ChevronUp size={12} />
                                        <span>{currentSessionProvider?.name ?? t('chat.no_provider_selected', '无提供商')}</span>
                                    </button>
                                    {showProviderMenu && (
                                        <div className="absolute bottom-full mb-1.5 left-0 min-w-[160px] bg-white dark:bg-[#2e2e2e] rounded-lg shadow-xl border border-black/5 dark:border-white/10 py-1 z-50">
                                            {aiProviders.length === 0 && <div className="px-3 py-2 text-xs text-gray-400">{t('chat.no_providers', '暂无提供商')}</div>}
                                            {aiProviders.map((p) => (
                                                <button
                                                    key={p.id}
                                                    className={`w-full text-left px-3 py-2 text-xs flex items-center gap-2 hover:bg-gray-50 dark:hover:bg-[#383838] transition-colors ${currentSessionProvider?.id === p.id ? 'text-[#07c160]' : 'text-gray-600 dark:text-gray-300'}`}
                                                    onClick={() => {
                                                        if (activeChatId) {
                                                            useDevOpsStore.getState().updateChatSession(activeChatId, { provider: p.id, model: p.defaultModel || '' });
                                                        } else {
                                                            useDevOpsStore.getState().updateAIProvider(p.id, { enabled: true });
                                                        }
                                                        setShowProviderMenu(false);
                                                    }}
                                                >
                                                    <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${currentSessionProvider?.id === p.id ? 'bg-[#07c160]' : 'bg-transparent'}`} />
                                                    {p.name}
                                                </button>
                                            ))}
                                        </div>
                                    )}
                                </div>

                                {/* Model picker */}
                                {currentSessionProvider && (
                                    <div className="relative" ref={modelMenuRef}>
                                        <button
                                            className="flex items-center gap-1 text-[11px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 px-2 py-1 rounded-md hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
                                            onClick={() => { setShowModelMenu(!showModelMenu); setShowProviderMenu(false); }}
                                        >
                                            <ChevronUp size={12} />
                                            <span className="max-w-[180px] truncate">{currentModel || t('chat.select_model', '选择模型')}</span>
                                            {fetchingModels && <RefreshCw size={10} className="animate-spin text-gray-400" />}
                                        </button>
                                        {showModelMenu && (
                                            <div className="absolute bottom-full mb-1.5 left-0 min-w-[220px] max-h-[320px] overflow-y-auto bg-white dark:bg-[#2e2e2e] rounded-lg shadow-xl border border-black/5 dark:border-white/10 py-1 z-50">
                                                {fetchingModels && displayModels.length === 0 && (
                                                    <div className="px-3 py-3 flex items-center gap-2 text-xs text-gray-400">
                                                        <RefreshCw size={11} className="animate-spin" />{t('chat.fetching_models', '获取模型列表中…')}
                                                    </div>
                                                )}
                                                {!fetchingModels && displayModels.length === 0 && (
                                                    <div className="px-3 py-2 text-xs text-gray-400">{t('chat.no_models', '无可用模型')}</div>
                                                )}
                                                {displayModels.map((m) => (
                                                    <button
                                                        key={m}
                                                        className={`w-full text-left px-3 py-2 text-xs flex items-center gap-2 hover:bg-gray-50 dark:hover:bg-[#383838] transition-colors ${m === currentModel ? 'text-[#07c160]' : 'text-gray-600 dark:text-gray-300'}`}
                                                        onClick={() => {
                                                            if (activeChatId && currentSessionProvider) useDevOpsStore.getState().updateChatSession(activeChatId, { model: m, provider: currentSessionProvider.id });
                                                            setShowModelMenu(false);
                                                        }}
                                                    >
                                                        <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${m === currentModel ? 'bg-[#07c160]' : 'bg-transparent'}`} />
                                                        {m}
                                                    </button>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                )}

                                <div className="flex-1" />

                                {/* Send / Stop */}
                                {isSessionLoading ? (
                                    <button
                                        className="px-4 py-1.5 text-xs bg-red-500 hover:bg-red-600 text-white rounded-full transition-colors flex items-center gap-1.5"
                                        onClick={() => invoke('agent_cancel', { sessionId: activeChatId })}
                                    >
                                        <Square size={11} fill="white" />
                                        {t('chat.stop', '停止')}
                                    </button>
                                ) : (
                                    <button
                                        className="px-4 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] disabled:opacity-40 text-white rounded-full transition-colors"
                                        onClick={handleSend}
                                        disabled={!input.trim() && pendingImages.length === 0}
                                    >
                                        {t('chat.send', '发送')}
                                    </button>
                                )}
                            </div>
                        </div>
                    </>
                )}
            </div>
            {/* Custom Avatar Picker */}
            <AvatarPicker
                isOpen={showAvatarPicker}
                onClose={() => setShowAvatarPicker(false)}
                currentAvatarUrl={activeSession?.agentAvatarUrl}
                title={t('chat.change_avatar', '更换助手头像')}
                onSelect={(url: string) => {
                    if (activeChatId) {
                        updateChatSession(activeChatId, { agentAvatarUrl: url });
                    }
                }}
            />
        </>
    );
}

// Subcomponent for individual sortable item
function ChatSessionItem({
    session, activeChatId, contextMenu, editingSessionId, editingSessionTitle,
    setActiveChatId, handleContextMenu, setEditingSessionId, setEditingSessionTitle,
    updateChatSession, getLastTime, getLastMessage, t, loading
}: any) {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
    } = useSortable({ id: session.id });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
    };

    const isWorking = loading[`chat-${session.id}`];

    return (
        <div
            ref={setNodeRef}
            style={style}
            className={`flex items-center px-3 py-2.5 cursor-pointer transition-colors group relative ${activeChatId === session.id || contextMenu?.sessionId === session.id
                ? 'bg-black/[0.08] dark:bg-white/[0.08]'
                : 'hover:bg-black/[0.04] dark:hover:bg-white/[0.04]'
                }`}
            onClick={() => setActiveChatId(session.id)}
            onContextMenu={(e) => handleContextMenu(e, session.id)}
            {...(editingSessionId === session.id ? {} : attributes)}
            {...(editingSessionId === session.id ? {} : listeners)}
        >
            <div className="relative shrink-0 mr-3">
                <div className={`w-10 h-10 rounded-full flex items-center justify-center overflow-hidden shadow-sm bg-white dark:bg-[#404040] border border-black/5 dark:border-white/5`}>
                    {session.agentAvatarUrl ? (
                        <img src={session.agentAvatarUrl} alt="Avatar" className="w-[85%] h-[85%] object-cover rounded-full" draggable={false} />
                    ) : (
                        <Bot size={17} className="text-[#07c160]" />
                    )}
                </div>
                {session.pinned && (
                    <div className="absolute -top-1 -right-1 w-3.5 h-3.5 bg-white dark:bg-[#252525] rounded-full flex items-center justify-center shrink-0 z-10">
                        <Pin size={9} className="text-[#07c160] rotate-45" fill="currentColor" />
                    </div>
                )}
                {/* Working Indicator */}
                {isWorking && (
                    <div className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 bg-white dark:bg-[#252525] rounded-full flex items-center justify-center shrink-0 z-10">
                        <div className="w-2.5 h-2.5 bg-[#07c160] rounded-full animate-pulse" />
                    </div>
                )}
                {/* Unread Badge */}
                {activeChatId !== session.id && (session.unreadCount || 0) > 0 && !isWorking && (
                    <div className="absolute -top-1.5 -left-1.5 w-[18px] h-[18px] bg-red-500 rounded-full text-[9px] font-bold text-white flex items-center justify-center border-2 border-[#f5f5f5] dark:border-[#252525] shrink-0 z-10">
                        {session.unreadCount! > 99 ? '99+' : session.unreadCount}
                    </div>
                )}
            </div>
            <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between min-w-0">
                    {editingSessionId === session.id ? (
                        <input
                            autoFocus
                            className="w-full text-[13px] font-medium text-gray-800 dark:text-gray-200 bg-transparent border-b border-[#07c160] outline-none"
                            value={editingSessionTitle}
                            onChange={(e) => setEditingSessionTitle(e.target.value)}
                            onKeyDown={(e) => {
                                if (e.key === 'Enter') {
                                    if (editingSessionTitle.trim()) updateChatSession(session.id, { title: editingSessionTitle.trim() });
                                    setEditingSessionId(null);
                                } else if (e.key === 'Escape') {
                                    setEditingSessionId(null);
                                }
                            }}
                            onBlur={() => {
                                if (editingSessionTitle.trim()) updateChatSession(session.id, { title: editingSessionTitle.trim() });
                                setEditingSessionId(null);
                            }}
                            onClick={(e) => e.stopPropagation()}
                            onPointerDown={(e) => e.stopPropagation()}
                        />
                    ) : (
                        <div className="flex items-center gap-1.5 min-w-0">
                            <span className="text-[13px] font-medium text-gray-800 dark:text-gray-200 truncate group-hover:text-gray-900 dark:group-hover:text-white transition-colors">{session.title}</span>
                        </div>
                    )}
                    <span className="text-[10px] text-gray-400 shrink-0 ml-2">{getLastTime(session)}</span>
                </div>
                <div className="flex items-center justify-between mt-0.5">
                    <p className="text-xs text-gray-400 truncate pr-2">
                        {session.workspace
                            ? <span className="flex items-center gap-1"><FolderOpen size={10} />{session.workspace.split('/').pop()}</span>
                            : (getLastMessage(session) || <span className="italic opacity-50">{t('chat.new_chat', '新对话')}</span>)
                        }
                    </p>
                </div>
            </div>
        </div>
    );
}

export default AIChat;
