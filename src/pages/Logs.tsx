import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Search, Trash2, AlertTriangle, Info, Bug } from 'lucide-react';

interface LogEntry {
    id: number;
    level: string;
    message: string;
    timestamp: string;
    target: string;
}

function Logs() {
    const [logs, setLogs] = useState<LogEntry[]>([]);
    const [search, setSearch] = useState('');
    const [levelFilter, setLevelFilter] = useState('all');
    const bottomRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        invoke('enable_debug_console').then(() => {
            invoke<LogEntry[]>('get_debug_console_logs').then(setLogs).catch(console.error);
        });

        const unlisten = listen<LogEntry>('log-event', (event) => {
            setLogs(prev => [...prev, event.payload]);
        });
        return () => {
            unlisten.then(fn => fn());
            invoke('disable_debug_console').catch(console.error);
        };
    }, []);

    useEffect(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [logs]);

    const handleClear = async () => {
        try {
            await invoke('clear_debug_console_logs');
            setLogs([]);
        } catch (e) { console.error(e); }
    };

    const filtered = logs.filter(log => {
        if (levelFilter !== 'all' && log.level.toLowerCase() !== levelFilter) return false;
        if (search && !log.message.toLowerCase().includes(search.toLowerCase()) && !log.target.toLowerCase().includes(search.toLowerCase())) return false;
        return true;
    });

    const levelIcon = (level: string) => {
        const lower = level.toLowerCase();
        if (lower.includes('err')) return <AlertTriangle size={12} className="text-red-400 shrink-0" />;
        if (lower.includes('warn')) return <AlertTriangle size={12} className="text-yellow-400 shrink-0" />;
        if (lower.includes('debug')) return <Bug size={12} className="text-[#07c160] shrink-0" />; // User mentioned Debug showed up for Info, swapped colors to match standard console
        return <Info size={12} className="text-blue-500 shrink-0" />;
    };

    const levelColor = (level: string) => {
        const lower = level.toLowerCase();
        if (lower.includes('err')) return 'text-red-400';
        if (lower.includes('warn')) return 'text-yellow-500';
        if (lower.includes('debug')) return 'text-[#07c160]';
        return 'text-blue-500';
    };

    return (
        <>
            {/* Left: Filter panel */}
            <div className="w-[250px] shrink-0 bg-[#f7f7f7] dark:bg-[#252525] flex flex-col border-r border-black/5 dark:border-white/5">
                <div className="px-3 pt-4 pb-2">
                    <div className="flex items-center justify-between mb-2">
                        <span className="text-xs text-gray-400">共 {logs.length} 条日志</span>
                        <button onClick={handleClear} className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 text-red-400" title="清空"><Trash2 className="w-3.5 h-3.5" /></button>
                    </div>
                    <div className="relative mb-2">
                        <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                        <input type="text" value={search} onChange={e => setSearch(e.target.value)} placeholder="搜索日志..."
                            className="w-full pl-8 pr-3 py-1.5 text-xs bg-white dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400" />
                    </div>
                </div>

                {/* Level filters */}
                <div className="px-3 pb-3 space-y-0.5">
                    {[
                        { key: 'all', label: '全部', count: logs.length },
                        { key: 'info', label: 'Info', count: logs.filter(l => l.level.toLowerCase() === 'info').length },
                        { key: 'warn', label: 'Warn', count: logs.filter(l => l.level.toLowerCase() === 'warn').length },
                        { key: 'error', label: 'Error', count: logs.filter(l => l.level.toLowerCase() === 'error').length },
                        { key: 'debug', label: 'Debug', count: logs.filter(l => l.level.toLowerCase() === 'debug').length },
                    ].map(item => (
                        <button
                            key={item.key}
                            onClick={() => setLevelFilter(item.key)}
                            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors ${levelFilter === item.key
                                ? 'bg-white dark:bg-[#383838] text-gray-800 dark:text-white font-medium'
                                : 'text-gray-500 dark:text-gray-400 hover:bg-black/5 dark:hover:bg-white/5'
                                }`}
                        >
                            <span>{item.label}</span>
                            <span className="text-[10px] text-gray-400">{item.count}</span>
                        </button>
                    ))}
                </div>

                <div className="flex-1" />
            </div>

            {/* Right: Log stream */}
            <div className="flex-1 flex flex-col min-w-0 bg-[#f5f5f5] dark:bg-[#1e1e1e]">
                <div className="h-14 px-5 flex items-center border-b border-black/5 dark:border-white/5 shrink-0" data-tauri-drag-region>
                    <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200">系统日志</h3>
                    <span className="text-xs text-gray-400 ml-2">实时</span>
                </div>

                <div className="flex-1 overflow-y-auto font-mono text-xs">
                    {filtered.length === 0 ? (
                        <div className="flex items-center justify-center h-full text-gray-400 text-sm">暂无日志</div>
                    ) : (
                        <div className="p-3 space-y-0.5">
                            {filtered.map((log) => (
                                <div key={log.id} className="flex items-start gap-2 px-2 py-1 rounded hover:bg-white/50 dark:hover:bg-white/5 transition-colors">
                                    {levelIcon(log.level)}
                                    <span className="text-[10px] text-gray-400 shrink-0 w-[52px]">
                                        {new Date(log.timestamp).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false })}
                                    </span>
                                    <span className={`text-[10px] font-bold uppercase w-[40px] shrink-0 ${levelColor(log.level)}`}>{log.level.slice(0, 5)}</span>
                                    <span className="text-[10px] text-gray-400 shrink-0 max-w-[120px] truncate">{log.target}</span>
                                    <span className="text-[11px] text-gray-600 dark:text-gray-300 break-all">{log.message}</span>
                                </div>
                            ))}
                            <div ref={bottomRef} />
                        </div>
                    )}
                </div>
            </div>
        </>
    );
}

export default Logs;
