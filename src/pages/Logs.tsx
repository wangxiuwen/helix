import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Activity, Search, Trash2, ArrowLeft } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

interface LogEntry {
    id: number;
    timestamp: number;
    level: string;
    target: string;
    message: string;
    fields: Record<string, string>;
}

function Logs() {
    const navigate = useNavigate();
    const [logs, setLogs] = useState<LogEntry[]>([]);
    const [search, setSearch] = useState('');
    const [levelFilter, setLevelFilter] = useState('all');
    const bottomRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        // Enable the debug console bridge and fetch buffered logs
        invoke('enable_debug_console').catch(() => { });
        invoke<LogEntry[]>('get_debug_console_logs').then((buffered) => {
            if (buffered && buffered.length > 0) setLogs(buffered);
        }).catch(() => { });

        // Listen for new log events from the Rust backend
        const unlisten = listen<LogEntry>('log-event', (event) => {
            setLogs((prev) => {
                const next = [...prev, event.payload];
                return next.length > 2000 ? next.slice(-2000) : next;
            });
        });

        return () => {
            unlisten.then((fn) => fn());
        };
    }, []);

    // Auto-scroll to bottom when new logs arrive
    useEffect(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [logs.length]);

    const filtered = logs.filter((log) => {
        const matchSearch = !search || log.message.toLowerCase().includes(search.toLowerCase()) || log.target.toLowerCase().includes(search.toLowerCase());
        const matchLevel = levelFilter === 'all' || log.level.toLowerCase() === levelFilter;
        return matchSearch && matchLevel;
    });

    const handleClear = () => {
        invoke('clear_debug_console_logs').catch(() => { });
        setLogs([]);
    };

    const levelColors: Record<string, { dot: string; badge: string }> = {
        INFO: { dot: 'bg-blue-500', badge: 'bg-blue-500/10 text-blue-500' },
        WARN: { dot: 'bg-amber-500', badge: 'bg-amber-500/10 text-amber-500' },
        ERROR: { dot: 'bg-red-500', badge: 'bg-red-500/10 text-red-500' },
        DEBUG: { dot: 'bg-gray-400', badge: 'bg-gray-400/10 text-gray-500' },
        TRACE: { dot: 'bg-gray-300', badge: 'bg-gray-300/10 text-gray-400' },
    };

    const formatTime = (ts: number) => {
        const d = new Date(ts);
        return d.toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
    };

    return (
        <div className="p-6 space-y-4 overflow-y-auto h-full flex flex-col">
            <div className="flex items-center justify-between shrink-0">
                <div className="flex items-center gap-3">
                    <button
                        onClick={() => navigate('/')}
                        className="p-1.5 rounded-lg hover:bg-base-200 transition-colors text-base-content/50"
                        title="返回对话"
                    >
                        <ArrowLeft size={20} />
                    </button>
                    <div>
                        <h1 className="text-2xl font-bold text-base-content">系统日志</h1>
                        <p className="text-sm text-base-content/60 mt-1">实时后端日志 · 共 {logs.length} 条</p>
                    </div>
                </div>
                <button className="btn btn-ghost btn-sm gap-2 text-red-500" onClick={handleClear}>
                    <Trash2 size={16} />清空
                </button>
            </div>

            <div className="space-y-3 shrink-0">
                <label className="input input-bordered input-sm flex items-center gap-2 w-full max-w-md">
                    <Search size={16} className="text-base-content/40" />
                    <input type="text" placeholder="搜索日志..." value={search} onChange={(e) => setSearch(e.target.value)} className="grow" />
                </label>
                <div className="flex gap-2">
                    {['all', 'info', 'warn', 'error', 'debug'].map((level) => {
                        const isActive = levelFilter === level;
                        const colors: Record<string, string> = {
                            all: isActive ? 'bg-base-content text-base-100' : '',
                            info: isActive ? 'bg-blue-500 text-white' : '',
                            warn: isActive ? 'bg-amber-500 text-white' : '',
                            error: isActive ? 'bg-red-500 text-white' : '',
                            debug: isActive ? 'bg-gray-500 text-white' : '',
                        };
                        return (
                            <button key={level}
                                className={`px-3 py-1 text-xs rounded-full transition-colors ${isActive ? colors[level] : 'bg-base-200 text-base-content/60 hover:bg-base-300'}`}
                                onClick={() => setLevelFilter(level)}>
                                {level === 'all' ? '全部' : level.toUpperCase()}
                            </button>
                        );
                    })}
                </div>
            </div>

            {filtered.length === 0 ? (
                <div className="text-center py-16 text-base-content/40 flex-1 flex flex-col items-center justify-center">
                    <Activity size={48} className="mb-3 opacity-30" />
                    <p>暂无日志</p>
                </div>
            ) : (
                <div className="flex-1 overflow-y-auto font-mono text-xs space-y-px bg-base-200/30 rounded-xl p-2">
                    {filtered.map((log) => {
                        const lc = levelColors[log.level] || levelColors.DEBUG;
                        return (
                            <div key={log.id} className={`flex items-start gap-2 px-2 py-1 rounded hover:bg-base-200/50 ${log.level === 'ERROR' ? 'bg-red-500/5' : ''}`}>
                                <span className="text-base-content/40 shrink-0 w-16">{formatTime(log.timestamp)}</span>
                                <span className={`shrink-0 text-[10px] font-semibold px-1.5 py-0.5 rounded ${lc.badge}`}>{log.level}</span>
                                <span className="text-base-content/40 shrink-0 max-w-[140px] truncate">{log.target}</span>
                                <span className="text-base-content break-all">{log.message}</span>
                            </div>
                        );
                    })}
                    <div ref={bottomRef} />
                </div>
            )}
        </div>
    );
}

export default Logs;
