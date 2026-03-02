import React, { useState, useRef, useEffect } from 'react';
import { Send, Users, Clock, Bot, MessageSquare, Plus, FolderOpen, Trash2, FileText, ExternalLink, X } from 'lucide-react';
import { TeamOrchestrator } from '../services/team/orchestrator';
import { getRole } from '../services/team/roles';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useDevOpsStore } from '../stores/useDevOpsStore';
import { invoke } from '@tauri-apps/api/core';

export default function TeamChat() {
    const {
        teamSessions,
        activeTeamSessionId,
        createTeamSession,
        setActiveTeamSessionId,
        addTeamMessage,
        deleteTeamSession
    } = useDevOpsStore();

    const [input, setInput] = useState('');
    const [isRunning, setIsRunning] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);

    const [showFiles, setShowFiles] = useState(false);
    const [workspaceFiles, setWorkspaceFiles] = useState<any[]>([]);
    const [selectedFile, setSelectedFile] = useState<string | null>(null);
    const [fileContent, setFileContent] = useState<string>('');

    // Fetch workspace files periodically if panel is open constraint by isRunning state
    useEffect(() => {
        let interval: any;
        const activeSessionWorkspace = teamSessions.find(s => s.id === activeTeamSessionId)?.workspace;

        if (activeSessionWorkspace && showFiles) {
            const fetchFiles = async () => {
                try {
                    const files = await invoke<any[]>('workspace_list_session_files', { path: activeSessionWorkspace });
                    setWorkspaceFiles(files);
                } catch (e) {
                    console.error(e);
                }
            };
            fetchFiles();
            interval = setInterval(fetchFiles, 4000);
        }
        return () => clearInterval(interval);
    }, [activeTeamSessionId, teamSessions, showFiles]);

    const handleSelectFile = async (name: string) => {
        setSelectedFile(name);
        const activeSessionWorkspace = teamSessions.find(s => s.id === activeTeamSessionId)?.workspace;
        if (!activeSessionWorkspace) return;
        try {
            const content = await invoke<string>('workspace_read_session_file', { dirPath: activeSessionWorkspace, name });
            setFileContent(content);
        } catch (e) {
            setFileContent('⚠️ Cannot load file content. Error: ' + e);
        }
    };

    // Initial session create (only set active if none is active but there are sessions)
    useEffect(() => {
        if (!activeTeamSessionId && teamSessions.length > 0) {
            setActiveTeamSessionId(teamSessions[0].id);
        }
    }, [activeTeamSessionId, teamSessions, setActiveTeamSessionId]);

    const activeSession = teamSessions.find(s => s.id === activeTeamSessionId);
    const messages = activeSession?.messages || [];

    const scrollToBottom = () => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    };

    useEffect(() => {
        scrollToBottom();
    }, [messages]);

    const handleNewSession = () => {
        createTeamSession('新需求讨论');
    };

    const handleOpenWorkspace = async () => {
        if (!activeSession?.workspace) return;
        try {
            await invoke('workspace_open_dir', { path: activeSession.workspace });
        } catch (err: any) {
            console.error('Cannot open workspace:', err);
            // Fallback for demo, since workspace_open_dir doesn't exist yet, we can ask frontend to show path
            alert(`工作空间路径:\n${activeSession.workspace}`);
        }
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!input.trim() || isRunning) return;

        const req = input.trim();
        setInput('');
        setIsRunning(true);

        let targetSessionId = activeTeamSessionId;
        // Auto-create a session if we are in an empty state
        if (!targetSessionId) {
            targetSessionId = useDevOpsStore.getState().createTeamSession(req.substring(0, 20));
            useDevOpsStore.getState().setActiveTeamSessionId(targetSessionId);
            // Re-fetch the newly created session state to use below
        }

        const stateInstance = useDevOpsStore.getState();
        const freshSession = stateInstance.teamSessions.find(s => s.id === targetSessionId);
        let wsDir = freshSession?.workspace;

        // Update session title and workspace on very first message
        if (freshSession?.messages.length === 0 || !wsDir) {
            useDevOpsStore.getState().updateTeamSession(targetSessionId, { title: req.substring(0, 20) });
            // assign a workspace string for UI context
            const baseDir = await invoke<string>('workspace_get_dir').catch(() => '~/desktop');
            wsDir = `${baseDir}/sessions/${targetSessionId}`;
            useDevOpsStore.getState().updateTeamSession(targetSessionId, { workspace: wsDir });
        }

        // Add user message to store
        useDevOpsStore.getState().addTeamMessage(targetSessionId, { role: 'user', name: '我', content: req, icon: '👤' });

        const orchestrator = new TeamOrchestrator();

        let pendingProgressId: string | null = null;

        await orchestrator.handleRequest(req, wsDir || '', (evt) => {
            const state = useDevOpsStore.getState();
            const session = state.teamSessions.find(s => s.id === targetSessionId);
            if (!session) return;

            if (evt.type === 'progress') {
                if (pendingProgressId) {
                    const existing = session.messages.find(m => m.id === pendingProgressId);
                    if (existing && existing.role === evt.data.role && existing.isProgress) {
                        state.updateTeamMessage(targetSessionId, pendingProgressId, { action: evt.data.action });
                        return;
                    }
                }
                pendingProgressId = state.addTeamMessage(targetSessionId, {
                    role: evt.data.role,
                    name: evt.data.name,
                    action: evt.data.action,
                    icon: getRole(evt.data.role)?.icon || '🤖',
                    avatar: getRole(evt.data.role)?.avatar,
                    isProgress: true
                });

            } else if (evt.type === 'result') {
                if (pendingProgressId) {
                    state.updateTeamMessage(targetSessionId, pendingProgressId, {
                        content: evt.data.content,
                        isProgress: false,
                        action: undefined
                    });
                    pendingProgressId = null;
                } else {
                    state.addTeamMessage(targetSessionId, {
                        role: evt.data.role,
                        name: evt.data.name,
                        content: evt.data.content,
                        icon: getRole(evt.data.role)?.icon || '🤖',
                        avatar: getRole(evt.data.role)?.avatar
                    });
                }
            } else if (evt.type === 'group_start') {
                state.addTeamMessage(targetSessionId, {
                    role: 'system',
                    name: 'System',
                    content: `👨‍💻 **项目组正在开启专题讨论:** ${evt.data.topic}`,
                    icon: '👥'
                });
            } else if (evt.type === 'error') {
                state.addTeamMessage(targetSessionId, {
                    role: 'system', name: '系统提示', content: evt.data, icon: '⚠️'
                });
            } else if (evt.type === 'team_done') {
                setIsRunning(false);
                pendingProgressId = null;
            }
        });
    };

    return (
        <div className="flex h-full bg-[#fcfcfc] dark:bg-base-200">
            {/* Sidebar List */}
            <div className="w-[260px] flex-none border-r border-base-200 bg-base-100 flex flex-col">
                <div className="p-4 flex flex-col gap-4 border-b border-base-200">
                    <button
                        onClick={handleNewSession}
                        className="btn btn-primary w-full shadow-sm shadow-primary/20"
                    >
                        <Plus size={18} /> 发起新需求
                    </button>
                </div>
                <div className="flex-1 overflow-y-auto p-2 space-y-1">
                    {teamSessions.map(session => (
                        <div
                            key={session.id}
                            onClick={() => setActiveTeamSessionId(session.id)}
                            className={`flex items-center justify-between px-3 py-2.5 rounded-lg cursor-pointer transition-colors group ${activeTeamSessionId === session.id
                                ? 'bg-primary/10 text-primary font-medium'
                                : 'hover:bg-base-200 text-base-content/80'
                                }`}
                        >
                            <div className="flex items-center gap-2 truncate">
                                <MessageSquare size={16} className="opacity-70" />
                                <span className="truncate text-sm">{session.title}</span>
                            </div>
                            <button
                                onClick={(e) => { e.stopPropagation(); deleteTeamSession(session.id); }}
                                className="opacity-0 group-hover:opacity-100 p-1 hover:text-error transition-all"
                            >
                                <Trash2 size={14} />
                            </button>
                        </div>
                    ))}
                </div>
            </div>

            {/* Main Workspace Area */}
            <div className="flex-1 flex flex-col h-full bg-base-100/50">
                {/* Header */}
                <header className="flex-none px-6 py-4 flex items-center justify-between border-b border-base-200 bg-base-100/80 backdrop-blur-md sticky top-0 z-10">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center text-primary">
                            <Users size={20} />
                        </div>
                        <div>
                            <h1 className="text-xl font-bold font-mona tracking-tight">研发团队讨论区 (Multi-Agent)</h1>
                            <p className="text-xs text-base-content/60 font-medium">需求拆解、并行执行、零干预交付</p>
                        </div>
                    </div>
                    {/* Workspace Preview Button */}
                    {activeSession?.workspace && (
                        <div className="flex items-center gap-2">
                            <button
                                onClick={handleOpenWorkspace}
                                className="btn btn-sm btn-ghost gap-2 text-base-content/70 hover:text-base-content"
                                title="在系统资源管理器中打开"
                            >
                                <ExternalLink size={14} /> 文件夹
                            </button>
                            <button
                                onClick={() => setShowFiles(!showFiles)}
                                className={`btn btn-sm gap-2 transition-all ${showFiles ? 'btn-primary' : 'btn-outline border-base-300'
                                    }`}
                            >
                                <FolderOpen size={14} /> 预览产出
                                {workspaceFiles.length > 0 && (
                                    <div className="badge badge-sm badge-neutral">{workspaceFiles.length}</div>
                                )}
                            </button>
                        </div>
                    )}
                </header>

                {/* Messages Area */}
                <main className="flex-1 overflow-y-auto px-6 py-4">
                    <div className="max-w-4xl mx-auto space-y-6">
                        {messages.length === 0 ? (
                            <div className="flex flex-col items-center justify-center h-[60vh] text-center px-4 animate-in fade-in zoom-in duration-500">
                                <div className="w-20 h-20 bg-primary/10 rounded-3xl flex items-center justify-center mb-6 shadow-lg shadow-primary/5">
                                    <Bot size={40} className="text-primary" />
                                </div>
                                <h2 className="text-2xl font-bold mb-3 tracking-tight">专属的多智能体工程团队</h2>
                                <p className="text-base-content/70 max-w-md leading-relaxed">
                                    输入您的软件需求，我们的项目经理 (林雨) 将拉通产品、架构、前后端开发与测试，在沙箱中全自动为您评估并编码。
                                </p>
                            </div>
                        ) : (
                            messages.map((msg) => (
                                <div key={msg.id} className={`flex items-start gap-4 ${msg.role === 'user' ? 'flex-row-reverse' : ''}`}>
                                    <div className="flex-none flex items-center justify-center w-10 h-10 rounded-2xl text-xl bg-white dark:bg-base-200 border border-base-200 shadow-sm shrink-0 overflow-hidden relative group">
                                        {msg.avatar ? (
                                            <img src={msg.avatar} alt="avatar" className="w-full h-full object-cover" />
                                        ) : (
                                            msg.icon
                                        )}
                                        {msg.role !== 'user' && msg.role !== 'system' && (
                                            <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center pointer-events-none">
                                                <span className="text-[10px] text-white font-bold leading-tight px-1 text-center scale-90">{getRole(msg.role)?.name?.split(' ')[0]}</span>
                                            </div>
                                        )}
                                    </div>
                                    <div className={`flex flex-col gap-1.5 max-w-[85%] ${msg.role === 'user' ? 'items-end' : ''}`}>
                                        <div className="text-xs font-semibold text-base-content/60 px-1 tracking-wide">
                                            {msg.name}
                                        </div>
                                        <div className={`
                                            relative px-5 py-3.5 rounded-2xl text-[14px] leading-relaxed
                                            ${msg.role === 'user'
                                                ? 'bg-primary text-primary-content rounded-tr-none shadow-md shadow-primary/20'
                                                : msg.role === 'system'
                                                    ? 'bg-warning/20 text-warning-content border border-warning/30 rounded-tl-none font-bold shadow-inner'
                                                    : msg.isProgress
                                                        ? 'bg-white dark:bg-base-300 animate-pulse text-base-content/70 rounded-tl-none border shadow-sm'
                                                        : 'bg-white dark:bg-base-200 border border-base-200/60 rounded-tl-none shadow-sm prose prose-sm max-w-none prose-p:leading-relaxed prose-pre:bg-base-300'}
                                        `}>
                                            {msg.isProgress ? (
                                                <div className="flex items-center gap-2.5 font-medium">
                                                    <Clock size={14} className="animate-spin text-primary" />
                                                    {msg.action}
                                                </div>
                                            ) : msg.role === 'user' ? (
                                                <div className="whitespace-pre-wrap">{msg.content}</div>
                                            ) : (
                                                <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))
                        )}
                        <div ref={messagesEndRef} />
                    </div>
                </main>

                {/* Input Area */}
                <div className="flex-none p-5 bg-base-100/80 backdrop-blur-lg border-t border-base-200 z-10">
                    <div className="max-w-4xl mx-auto">
                        <form onSubmit={handleSubmit} className="flex gap-3">
                            <input
                                type="text"
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                placeholder={isRunning ? "团队推进中..." : "告诉林雨（项目经理），您想要开发点什么..."}
                                disabled={isRunning}
                                className="input input-lg input-bordered flex-1 focus:outline-none focus:ring-2 focus:ring-primary/20 bg-white dark:bg-base-200 shadow-sm transition-shadow rounded-2xl"
                            />
                            <button
                                type="submit"
                                className="btn btn-lg btn-primary rounded-2xl shadow-lg shadow-primary/20 w-[140px]"
                                disabled={!input.trim() || isRunning}
                            >
                                <Send size={18} />
                                {isRunning ? "执行中" : "发送需求"}
                            </button>
                        </form>
                    </div>
                </div>
            </div>

            {/* Right Panel: Workspace Files & Markdown Preview */}
            {showFiles && activeSession && (
                <div className="w-[450px] flex-none border-l border-base-200 bg-base-100 flex flex-col shadow-[-10px_0_20px_-10px_rgba(0,0,0,0.05)] z-20 transition-all duration-300 transform translate-x-0">
                    <div className="flex-none px-4 py-3 flex items-center justify-between border-b border-base-200 bg-base-100">
                        <div className="flex items-center gap-2">
                            <FileText size={16} className="text-primary" />
                            <h3 className="font-bold text-sm tracking-wide">项目产出文件</h3>
                        </div>
                        <button onClick={() => setShowFiles(false)} className="btn btn-sm btn-circle btn-ghost text-base-content/60 hover:text-base-content">
                            <X size={16} />
                        </button>
                    </div>

                    <div className="flex flex-1 overflow-hidden h-full">
                        {/* File List */}
                        <div className={`flex flex-col border-r border-base-200 bg-base-100/50 ${selectedFile ? 'w-[140px] flex-none' : 'w-full'}`}>
                            <div className="p-2 text-xs font-semibold text-base-content/50 uppercase tracking-wider">
                                FILES ({workspaceFiles.length})
                            </div>
                            <div className="flex-1 overflow-y-auto p-2 space-y-1">
                                {workspaceFiles.length === 0 ? (
                                    <div className="text-center py-6 text-xs text-base-content/50 px-2 leading-relaxed">
                                        该工作区目前为空<br />(智能体正在编码...)
                                    </div>
                                ) : workspaceFiles.map((file, i) => (
                                    <div
                                        key={i}
                                        onClick={() => handleSelectFile(file.name)}
                                        className={`px-3 py-2 text-xs rounded-md cursor-pointer truncate transition-all ${selectedFile === file.name
                                            ? 'bg-primary text-primary-content font-medium shadow-sm'
                                            : 'hover:bg-base-200 text-base-content hover:text-primary'
                                            }`}
                                        title={file.name}
                                    >
                                        <div className="flex items-center gap-2">
                                            <FileText size={12} className={selectedFile === file.name ? 'opacity-90' : 'opacity-50'} />
                                            <span className="truncate">{file.name}</span>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>

                        {/* File Content Preview */}
                        {selectedFile && (
                            <div className="flex-1 flex flex-col min-w-0 bg-base-100 relative">
                                <div className="flex-none px-4 py-2 border-b border-base-200 flex justify-between items-center bg-base-200/50">
                                    <span className="text-xs font-semibold truncate text-base-content/80">{selectedFile}</span>
                                    <button
                                        className="btn btn-xs btn-ghost text-base-content/50"
                                        onClick={() => setSelectedFile(null)}
                                    >
                                        关闭
                                    </button>
                                </div>
                                <div className="flex-1 overflow-y-auto p-5">
                                    {selectedFile.endsWith('.md') ? (
                                        <div className="prose prose-sm max-w-none prose-headings:font-bold prose-headings:tracking-tight prose-a:text-primary dark:prose-invert">
                                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{fileContent}</ReactMarkdown>
                                        </div>
                                    ) : (
                                        <pre className="text-xs font-mono whitespace-pre-wrap text-base-content/80 leading-relaxed overflow-x-auto">
                                            {fileContent}
                                        </pre>
                                    )}
                                </div>
                            </div>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
}
