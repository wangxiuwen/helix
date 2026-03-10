import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    BarChart3, TrendingUp, Zap, DollarSign,
    Download, RefreshCw, ArrowUpRight, ArrowDownRight,
    Database, Clock
} from 'lucide-react';

// ============================================================================
// Types (mirror backend usage.rs)
// ============================================================================

interface UsageTotals {
    total_requests: number;
    total_prompt_tokens: number;
    total_completion_tokens: number;
    total_tokens: number;
    total_cost_usd: number;
}

interface ModelUsage {
    model: string;
    provider: string;
    request_count: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    cost_usd: number;
}

interface DailyUsage {
    date: string;
    request_count: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    cost_usd: number;
}

interface UsageEntry {
    id: number;
    session_key: string;
    model: string;
    provider: string;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    cost_usd: number;
    source: string;
    created_at: string;
}

interface UsageDashboard {
    totals: UsageTotals;
    today: UsageTotals;
    by_model: ModelUsage[];
    daily: DailyUsage[];
    recent: UsageEntry[];
}

// ============================================================================
// Helpers
// ============================================================================

function formatTokens(n: number): string {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
    return n.toString();
}

function formatCost(usd: number): string {
    if (usd < 0.01) return '$' + usd.toFixed(4);
    return '$' + usd.toFixed(2);
}

function sourceLabel(src: string): { label: string; color: string } {
    switch (src) {
        case 'agent': return { label: 'Agent', color: 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400' };
        case 'team_chat': return { label: 'Team', color: 'bg-violet-500/15 text-violet-600 dark:text-violet-400' };
        case 'auto_reply': return { label: 'Auto', color: 'bg-amber-500/15 text-amber-600 dark:text-amber-400' };
        case 'compaction': return { label: 'Compact', color: 'bg-slate-500/15 text-slate-600 dark:text-slate-400' };
        default: return { label: src, color: 'bg-gray-500/15 text-gray-600 dark:text-gray-400' };
    }
}

// ============================================================================
// Mini SVG Bar Chart
// ============================================================================

function DailyChart({ data }: { data: DailyUsage[] }) {
    if (!data.length) return <div className="text-sm text-gray-400 text-center py-8">暂无每日数据</div>;

    const sorted = [...data].sort((a, b) => a.date.localeCompare(b.date)).slice(-14); // last 14 days
    const maxTokens = Math.max(...sorted.map(d => d.total_tokens), 1);

    return (
        <div className="flex items-end gap-1.5 h-32 px-2">
            {sorted.map((d, i) => {
                const h = Math.max((d.total_tokens / maxTokens) * 100, 4);
                const promptH = Math.max((d.prompt_tokens / maxTokens) * 100, 0);
                return (
                    <div key={i} className="flex-1 flex flex-col items-center gap-1 group relative">
                        {/* Tooltip */}
                        <div className="absolute -top-20 left-1/2 -translate-x-1/2 bg-gray-900 text-white text-[10px] p-2 rounded-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10 shadow-lg">
                            <div className="font-medium">{d.date}</div>
                            <div>{formatTokens(d.total_tokens)} tokens · {d.request_count} reqs</div>
                            <div>{formatCost(d.cost_usd)}</div>
                        </div>
                        {/* Bar */}
                        <div className="w-full rounded-t-sm overflow-hidden flex flex-col-reverse" style={{ height: `${h}%` }}>
                            <div className="w-full bg-emerald-500/70 rounded-t-sm" style={{ height: `${promptH}%` }} />
                            <div className="w-full bg-violet-500/60 flex-1" />
                        </div>
                        {/* Label */}
                        <span className="text-[9px] text-gray-400 truncate w-full text-center">
                            {d.date.slice(5)}
                        </span>
                    </div>
                );
            })}
        </div>
    );
}

// ============================================================================
// Main Component
// ============================================================================

export default function Usage() {
    const [dashboard, setDashboard] = useState<UsageDashboard | null>(null);
    const [loading, setLoading] = useState(true);
    const [exporting, setExporting] = useState(false);
    const [tab, setTab] = useState<'models' | 'recent'>('models');

    const load = useCallback(async () => {
        setLoading(true);
        try {
            const data = await invoke<UsageDashboard>('usage_dashboard', {
                recentLimit: 50,
                dailyDays: 30,
            });
            setDashboard(data);
        } catch (e) {
            console.error('usage_dashboard', e);
        }
        setLoading(false);
    }, []);

    useEffect(() => { load(); }, [load]);

    const handleExport = async () => {
        setExporting(true);
        try {
            const data = await invoke<any>('usage_export', { days: 90 });
            const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `helix-usage-${new Date().toISOString().slice(0, 10)}.json`;
            a.click();
            URL.revokeObjectURL(url);
        } catch (e) {
            console.error('usage_export', e);
        }
        setExporting(false);
    };

    const totals = dashboard?.totals;
    const today = dashboard?.today;

    // Rate of change indicator (today vs lifetime average)
    const avgDailyTokens = totals && dashboard?.daily?.length
        ? totals.total_tokens / Math.max(dashboard.daily.length, 1)
        : 0;
    const todayDelta = today && avgDailyTokens > 0
        ? ((today.total_tokens - avgDailyTokens) / avgDailyTokens * 100)
        : 0;

    return (
        <div className="flex-1 flex flex-col h-full overflow-hidden bg-[#FAFBFC] dark:bg-base-300">
            {/* Header */}
            <div className="shrink-0 px-6 pt-5 pb-3 flex items-center justify-between border-b border-black/5 dark:border-white/5">
                <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-violet-500 to-indigo-600 flex items-center justify-center shadow-lg shadow-violet-500/25">
                        <BarChart3 size={18} className="text-white" />
                    </div>
                    <div>
                        <h1 className="text-base font-bold text-gray-800 dark:text-white">用量统计</h1>
                        <p className="text-[11px] text-gray-400 mt-0.5">Token 消耗 · 请求次数 · 费用估算</p>
                    </div>
                </div>
                <div className="flex items-center gap-2">
                    <button
                        onClick={handleExport}
                        disabled={exporting}
                        className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium
                            bg-white dark:bg-[#2e2e2e] border border-black/10 dark:border-white/10
                            rounded-lg hover:bg-gray-50 dark:hover:bg-[#353535]
                            text-gray-600 dark:text-gray-300 transition-colors
                            disabled:opacity-50"
                    >
                        <Download size={14} />
                        {exporting ? '导出中...' : '导出 JSON'}
                    </button>
                    <button
                        onClick={load}
                        disabled={loading}
                        className="p-2 rounded-lg hover:bg-black/5 dark:hover:bg-white/5 text-gray-400 transition-colors disabled:opacity-50"
                    >
                        <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
                    </button>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">
                {loading && !dashboard ? (
                    <div className="flex items-center justify-center h-64 text-gray-400 text-sm">
                        <RefreshCw size={20} className="animate-spin mr-2" /> 加载中...
                    </div>
                ) : (
                    <>
                        {/* Summary Cards */}
                        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
                            {/* Total Requests */}
                            <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5">
                                <div className="flex items-center justify-between mb-2">
                                    <div className="w-8 h-8 rounded-lg bg-blue-500/10 flex items-center justify-center">
                                        <Zap size={16} className="text-blue-500" />
                                    </div>
                                    <span className="text-[10px] text-gray-400 flex items-center gap-0.5">
                                        今日 {today?.total_requests ?? 0}
                                    </span>
                                </div>
                                <p className="text-2xl font-bold text-gray-800 dark:text-white">
                                    {totals?.total_requests?.toLocaleString() ?? '0'}
                                </p>
                                <p className="text-[11px] text-gray-400 mt-0.5">总请求次数</p>
                            </div>

                            {/* Total Tokens */}
                            <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5">
                                <div className="flex items-center justify-between mb-2">
                                    <div className="w-8 h-8 rounded-lg bg-emerald-500/10 flex items-center justify-center">
                                        <Database size={16} className="text-emerald-500" />
                                    </div>
                                    <span className="text-[10px] text-gray-400">
                                        ↑{formatTokens(totals?.total_prompt_tokens ?? 0)} ↓{formatTokens(totals?.total_completion_tokens ?? 0)}
                                    </span>
                                </div>
                                <p className="text-2xl font-bold text-gray-800 dark:text-white">
                                    {formatTokens(totals?.total_tokens ?? 0)}
                                </p>
                                <p className="text-[11px] text-gray-400 mt-0.5">总 Token 消耗</p>
                            </div>

                            {/* Today Tokens */}
                            <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5">
                                <div className="flex items-center justify-between mb-2">
                                    <div className="w-8 h-8 rounded-lg bg-violet-500/10 flex items-center justify-center">
                                        <TrendingUp size={16} className="text-violet-500" />
                                    </div>
                                    {todayDelta !== 0 && (
                                        <span className={`text-[10px] flex items-center gap-0.5 ${todayDelta > 0 ? 'text-rose-500' : 'text-emerald-500'}`}>
                                            {todayDelta > 0 ? <ArrowUpRight size={12} /> : <ArrowDownRight size={12} />}
                                            {Math.abs(todayDelta).toFixed(0)}%
                                        </span>
                                    )}
                                </div>
                                <p className="text-2xl font-bold text-gray-800 dark:text-white">
                                    {formatTokens(today?.total_tokens ?? 0)}
                                </p>
                                <p className="text-[11px] text-gray-400 mt-0.5">今日 Token</p>
                            </div>

                            {/* Total Cost */}
                            <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5">
                                <div className="flex items-center justify-between mb-2">
                                    <div className="w-8 h-8 rounded-lg bg-amber-500/10 flex items-center justify-center">
                                        <DollarSign size={16} className="text-amber-500" />
                                    </div>
                                    <span className="text-[10px] text-gray-400">
                                        今日 {formatCost(today?.total_cost_usd ?? 0)}
                                    </span>
                                </div>
                                <p className="text-2xl font-bold text-gray-800 dark:text-white">
                                    {formatCost(totals?.total_cost_usd ?? 0)}
                                </p>
                                <p className="text-[11px] text-gray-400 mt-0.5">估计总费用 (USD)</p>
                            </div>
                        </div>

                        {/* Daily Chart */}
                        <div className="p-5 rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5">
                            <div className="flex items-center justify-between mb-4">
                                <h3 className="text-sm font-semibold text-gray-800 dark:text-white">每日趋势</h3>
                                <div className="flex items-center gap-3 text-[10px] text-gray-400">
                                    <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-sm bg-emerald-500/70" />Prompt</span>
                                    <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-sm bg-violet-500/60" />Completion</span>
                                </div>
                            </div>
                            <DailyChart data={dashboard?.daily ?? []} />
                        </div>

                        {/* Tabs: Models / Recent */}
                        <div>
                            <div className="flex items-center gap-1 mb-3">
                                <button
                                    onClick={() => setTab('models')}
                                    className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${tab === 'models'
                                        ? 'bg-gray-900 text-white dark:bg-white dark:text-gray-900'
                                        : 'text-gray-500 hover:bg-black/5 dark:hover:bg-white/5'
                                        }`}
                                >
                                    按模型
                                </button>
                                <button
                                    onClick={() => setTab('recent')}
                                    className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${tab === 'recent'
                                        ? 'bg-gray-900 text-white dark:bg-white dark:text-gray-900'
                                        : 'text-gray-500 hover:bg-black/5 dark:hover:bg-white/5'
                                        }`}
                                >
                                    最近调用
                                </button>
                            </div>

                            {tab === 'models' ? (
                                <div className="rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5 overflow-hidden">
                                    <table className="w-full text-xs">
                                        <thead>
                                            <tr className="border-b border-black/5 dark:border-white/5 text-gray-400 text-left">
                                                <th className="px-4 py-3 font-medium">模型</th>
                                                <th className="px-4 py-3 font-medium text-right">请求数</th>
                                                <th className="px-4 py-3 font-medium text-right">Prompt</th>
                                                <th className="px-4 py-3 font-medium text-right">Completion</th>
                                                <th className="px-4 py-3 font-medium text-right">总 Token</th>
                                                <th className="px-4 py-3 font-medium text-right">费用</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {(dashboard?.by_model ?? []).map((m, i) => (
                                                <tr key={i} className="border-b border-black/3 dark:border-white/3 last:border-0 hover:bg-black/[0.02] dark:hover:bg-white/[0.02]">
                                                    <td className="px-4 py-3">
                                                        <span className="font-medium text-gray-800 dark:text-gray-200">{m.model}</span>
                                                        <span className="ml-1.5 text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-400">{m.provider}</span>
                                                    </td>
                                                    <td className="px-4 py-3 text-right text-gray-600 dark:text-gray-300 font-mono">{m.request_count.toLocaleString()}</td>
                                                    <td className="px-4 py-3 text-right text-gray-500 font-mono">{formatTokens(m.prompt_tokens)}</td>
                                                    <td className="px-4 py-3 text-right text-gray-500 font-mono">{formatTokens(m.completion_tokens)}</td>
                                                    <td className="px-4 py-3 text-right text-gray-800 dark:text-gray-200 font-mono font-medium">{formatTokens(m.total_tokens)}</td>
                                                    <td className="px-4 py-3 text-right text-amber-600 dark:text-amber-400 font-mono">{formatCost(m.cost_usd)}</td>
                                                </tr>
                                            ))}
                                            {(dashboard?.by_model ?? []).length === 0 && (
                                                <tr><td colSpan={6} className="text-center py-8 text-gray-400">暂无模型数据</td></tr>
                                            )}
                                        </tbody>
                                    </table>
                                </div>
                            ) : (
                                <div className="rounded-xl bg-white dark:bg-[#2e2e2e] border border-black/5 dark:border-white/5 overflow-hidden">
                                    <table className="w-full text-xs">
                                        <thead>
                                            <tr className="border-b border-black/5 dark:border-white/5 text-gray-400 text-left">
                                                <th className="px-4 py-3 font-medium">时间</th>
                                                <th className="px-4 py-3 font-medium">来源</th>
                                                <th className="px-4 py-3 font-medium">模型</th>
                                                <th className="px-4 py-3 font-medium text-right">Prompt</th>
                                                <th className="px-4 py-3 font-medium text-right">Completion</th>
                                                <th className="px-4 py-3 font-medium text-right">费用</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {(dashboard?.recent ?? []).map((e, i) => {
                                                const s = sourceLabel(e.source);
                                                return (
                                                    <tr key={i} className="border-b border-black/3 dark:border-white/3 last:border-0 hover:bg-black/[0.02] dark:hover:bg-white/[0.02]">
                                                        <td className="px-4 py-2.5 text-gray-500 font-mono flex items-center gap-1.5">
                                                            <Clock size={12} className="text-gray-300" />
                                                            {e.created_at?.slice(5, 16).replace('T', ' ') ?? '-'}
                                                        </td>
                                                        <td className="px-4 py-2.5">
                                                            <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${s.color}`}>
                                                                {s.label}
                                                            </span>
                                                        </td>
                                                        <td className="px-4 py-2.5 text-gray-700 dark:text-gray-300 font-medium truncate max-w-[160px]">{e.model}</td>
                                                        <td className="px-4 py-2.5 text-right text-gray-500 font-mono">{formatTokens(e.prompt_tokens)}</td>
                                                        <td className="px-4 py-2.5 text-right text-gray-500 font-mono">{formatTokens(e.completion_tokens)}</td>
                                                        <td className="px-4 py-2.5 text-right text-amber-600 dark:text-amber-400 font-mono">{formatCost(e.cost_usd)}</td>
                                                    </tr>
                                                );
                                            })}
                                            {(dashboard?.recent ?? []).length === 0 && (
                                                <tr><td colSpan={6} className="text-center py-8 text-gray-400">暂无调用记录</td></tr>
                                            )}
                                        </tbody>
                                    </table>
                                </div>
                            )}
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
