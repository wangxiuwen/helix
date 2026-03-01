import { useState, useEffect, useCallback } from 'react';
import { Clock, Play, Plus, Trash2, Pause, RefreshCw, CheckCircle, XCircle, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface CronTask {
    id: string;
    name: string;
    description: string;
    schedule: string;
    script: string;
    status: 'active' | 'paused';
    type: 'cron' | 'manual';
    last_run: string | null;
    next_run: string | null;
    run_count: number;
    notify_channel: 'feishu' | 'dingtalk' | 'wecom' | null;
}

interface CronRun {
    id: string;
    task_id: string;
    started_at: string;
    finished_at: string | null;
    status: 'success' | 'failed' | 'running';
    output: string | null;
}

function CronJobs() {
    const [tasks, setTasks] = useState<CronTask[]>([]);
    const [loading, setLoading] = useState(true);
    const [selected, setSelected] = useState<CronTask | null>(null);
    const [showAdd, setShowAdd] = useState(false);
    const [runningTasks, setRunningTasks] = useState<Set<string>>(new Set());
    const [taskRuns, setTaskRuns] = useState<Record<string, CronRun[]>>({});
    const [form, setForm] = useState({
        name: '', description: '', schedule: '', script: '',
        type: 'cron' as 'cron' | 'manual',
        notifyChannel: null as 'feishu' | 'dingtalk' | 'wecom' | null,
    });

    const loadTasks = useCallback(async () => {
        try {
            const result = await invoke<CronTask[]>('cron_list_tasks');
            setTasks(result);
            if (result.length > 0 && (!selected || !result.find(t => t.id === selected.id))) {
                setSelected(result[0]);
            } else if (selected) {
                const updated = result.find(t => t.id === selected.id);
                if (updated) setSelected(updated);
            }
        } catch (e) {
            console.error('Failed to load tasks:', e);
        } finally {
            setLoading(false);
        }
    }, [selected]);

    useEffect(() => { loadTasks(); }, [loadTasks]);

    const handleAdd = async () => {
        if (!form.name) return;
        try {
            await invoke<CronTask>('cron_create_task', {
                input: { name: form.name, description: form.description || undefined, type: form.type, schedule: form.schedule || undefined, script: form.script || undefined, notify_channel: form.notifyChannel },
            });
            setForm({ name: '', description: '', schedule: '', script: '', type: 'cron', notifyChannel: null });
            setShowAdd(false);
            await loadTasks();
        } catch (e) { console.error('Failed to create task:', e); }
    };

    const handleDelete = async (id: string) => {
        try {
            await invoke('cron_delete_task', { id });
            if (selected?.id === id) setSelected(null);
            await loadTasks();
        } catch (e) { console.error('Failed to delete task:', e); }
    };

    const handleToggleStatus = async (task: CronTask) => {
        const newStatus = task.status === 'active' ? 'paused' : 'active';
        try {
            await invoke('cron_update_task', { id: task.id, input: { status: newStatus } });
            await loadTasks();
        } catch (e) { console.error('Failed to update task:', e); }
    };

    const handleRun = async (id: string) => {
        setRunningTasks(prev => new Set(prev).add(id));
        try {
            await invoke<CronRun>('cron_run_task', { id });
            await loadTasks();
            await loadRuns(id);
        } catch (e) { console.error('Failed to run task:', e); }
        finally { setRunningTasks(prev => { const next = new Set(prev); next.delete(id); return next; }); }
    };

    const loadRuns = async (taskId: string) => {
        try {
            const runs = await invoke<CronRun[]>('cron_get_runs', { taskId, limit: 10 });
            setTaskRuns(prev => ({ ...prev, [taskId]: runs }));
        } catch (e) { console.error('Failed to load runs:', e); }
    };

    const formatDate = (dateStr: string | null) => {
        if (!dateStr) return '-';
        try { return new Date(dateStr).toLocaleString(); } catch { return dateStr; }
    };

    if (loading) {
        return <div className="flex-1 flex items-center justify-center"><Loader2 className="animate-spin text-gray-400" size={32} /></div>;
    }

    return (
        <>
            {/* Left: Task list */}
            <div className="w-[250px] shrink-0 bg-[#f7f7f7] dark:bg-[#252525] flex flex-col border-r border-black/5 dark:border-white/5">
                <div className="px-3 pt-4 pb-2 flex items-center justify-between" data-tauri-drag-region>
                    <span className="text-xs text-gray-400">{tasks.length} 个任务</span>
                    <div className="flex items-center gap-1">
                        <button onClick={loadTasks} className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 text-gray-400" title="刷新">
                            <RefreshCw className="w-3.5 h-3.5" />
                        </button>
                        <button onClick={() => setShowAdd(!showAdd)} className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 text-gray-400" title="新建">
                            <Plus className="w-3.5 h-3.5" />
                        </button>
                    </div>
                </div>

                <div className="flex-1 overflow-y-auto">
                    {tasks.length === 0 ? (
                        <div className="px-4 py-12 text-center text-gray-400 text-xs">暂无定时任务</div>
                    ) : (
                        tasks.map(task => (
                            <div
                                key={task.id}
                                onClick={() => { setSelected(task); loadRuns(task.id); }}
                                className={`flex items-center px-3 py-3 cursor-pointer transition-colors group ${selected?.id === task.id ? 'bg-[#c9c9c9] dark:bg-[#383838]' : 'hover:bg-[#ebebeb] dark:hover:bg-[#303030]'
                                    }`}
                            >
                                <div className="w-10 h-10 rounded-lg bg-gray-200 dark:bg-[#404040] flex items-center justify-center shrink-0 mr-3">
                                    <Clock size={18} className={task.status === 'active' ? 'text-[#07c160]' : 'text-gray-400'} />
                                </div>
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center justify-between">
                                        <span className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">{task.name}</span>
                                        <span className={`text-[10px] px-1.5 py-0.5 rounded ${task.status === 'active' ? 'bg-[#07c160]/10 text-[#07c160]' : 'bg-gray-200 dark:bg-gray-700 text-gray-400'}`}>
                                            {task.status === 'active' ? '运行中' : '已暂停'}
                                        </span>
                                    </div>
                                    <p className="text-xs text-gray-400 truncate mt-0.5">{task.schedule || task.type}</p>
                                </div>
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* Right: Task detail */}
            <div className="flex-1 flex flex-col min-w-0 bg-[#f5f5f5] dark:bg-[#1e1e1e]">
                <div className="h-14 px-5 flex items-center justify-between border-b border-black/5 dark:border-white/5 shrink-0">
                    <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200">{selected ? selected.name : '定时任务'}</h3>
                </div>

                {/* Add form */}
                {showAdd && (
                    <div className="px-5 py-4 border-b border-black/5 dark:border-white/5">
                        <div className="max-w-lg space-y-2">
                            <input value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} placeholder="任务名称"
                                className="w-full px-3 py-2 text-sm bg-white dark:bg-[#2e2e2e] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400" />
                            <input value={form.schedule} onChange={e => setForm({ ...form, schedule: e.target.value })} placeholder="Cron 表达式 (如 0 */5 * * * *)"
                                className="w-full px-3 py-2 text-sm bg-white dark:bg-[#2e2e2e] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400 font-mono" />
                            <textarea value={form.script} onChange={e => setForm({ ...form, script: e.target.value })} placeholder="执行脚本" rows={3}
                                className="w-full px-3 py-2 text-sm bg-white dark:bg-[#2e2e2e] rounded-md border-0 outline-none resize-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400 font-mono" />
                            <div className="flex gap-2 mt-2">
                                <select value={form.notifyChannel || ''} onChange={e => setForm({ ...form, notifyChannel: e.target.value ? e.target.value as any : null })}
                                    className="w-full px-3 py-2 text-sm bg-white dark:bg-[#2e2e2e] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200">
                                    <option value="">不发送通知</option>
                                    <option value="feishu">飞书</option>
                                    <option value="dingtalk">钉钉</option>
                                    <option value="wecom">企业微信</option>
                                </select>
                            </div>
                            <div className="flex gap-2">
                                <button onClick={handleAdd} disabled={!form.name} className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md disabled:opacity-50">创建</button>
                                <button onClick={() => setShowAdd(false)} className="px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md">取消</button>
                            </div>
                        </div>
                    </div>
                )}

                {selected ? (
                    <div className="flex-1 overflow-y-auto px-8 py-6">
                        <div className="max-w-2xl">
                            {/* Info */}
                            <div className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] mb-4 space-y-3">
                                <div className="flex items-center justify-between">
                                    <span className="text-xs text-gray-400">状态</span>
                                    <span className={`text-xs font-medium ${selected.status === 'active' ? 'text-[#07c160]' : 'text-gray-400'}`}>{selected.status === 'active' ? '运行中' : '已暂停'}</span>
                                </div>
                                {selected.schedule && <div className="flex items-center justify-between"><span className="text-xs text-gray-400">Cron</span><span className="text-xs font-mono text-gray-600 dark:text-gray-300">{selected.schedule}</span></div>}
                                {selected.description && <div className="flex items-center justify-between"><span className="text-xs text-gray-400">描述</span><span className="text-xs text-gray-600 dark:text-gray-300">{selected.description}</span></div>}
                                <div className="flex items-center justify-between"><span className="text-xs text-gray-400">上次运行</span><span className="text-xs text-gray-600 dark:text-gray-300">{formatDate(selected.last_run)}</span></div>
                                <div className="flex items-center justify-between"><span className="text-xs text-gray-400">下次运行</span><span className="text-xs text-gray-600 dark:text-gray-300">{formatDate(selected.next_run)}</span></div>
                                <div className="flex items-center justify-between"><span className="text-xs text-gray-400">执行次数</span><span className="text-xs text-gray-600 dark:text-gray-300">{selected.run_count}</span></div>
                            </div>

                            {/* Actions */}
                            <div className="flex items-center gap-2 mb-6">
                                <button onClick={() => handleRun(selected.id)} disabled={runningTasks.has(selected.id)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md disabled:opacity-50">
                                    {runningTasks.has(selected.id) ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Play className="w-3.5 h-3.5" />}执行
                                </button>
                                <button onClick={() => handleToggleStatus(selected)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 text-xs bg-white dark:bg-[#2e2e2e] text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-600 rounded-md">
                                    <Pause className="w-3.5 h-3.5" />{selected.status === 'active' ? '暂停' : '恢复'}
                                </button>
                                <button onClick={() => handleDelete(selected.id)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-md">
                                    <Trash2 className="w-3.5 h-3.5" />删除
                                </button>
                            </div>

                            {/* Run history */}
                            <h4 className="text-xs font-medium text-gray-400 mb-2">执行历史</h4>
                            <div className="space-y-2">
                                {(taskRuns[selected.id] || []).length === 0 ? (
                                    <p className="text-xs text-gray-400 py-4 text-center">暂无执行记录</p>
                                ) : (
                                    (taskRuns[selected.id] || []).map(run => (
                                        <div key={run.id} className="p-3 rounded-lg bg-white dark:bg-[#2e2e2e]">
                                            <div className="flex items-center justify-between mb-1">
                                                <span className="flex items-center gap-1.5 text-xs">
                                                    {run.status === 'success' ? <CheckCircle size={12} className="text-[#07c160]" /> : run.status === 'running' ? <Loader2 size={12} className="animate-spin text-blue-400" /> : <XCircle size={12} className="text-red-400" />}
                                                    <span className="text-gray-600 dark:text-gray-300">{run.status}</span>
                                                </span>
                                                <span className="text-[10px] text-gray-400">{formatDate(run.started_at)}</span>
                                            </div>
                                            {run.output && <pre className="text-[11px] text-gray-500 font-mono whitespace-pre-wrap max-h-24 overflow-y-auto mt-1 p-2 bg-[#f7f7f7] dark:bg-[#1e1e1e] rounded">{run.output.slice(0, 500)}</pre>}
                                        </div>
                                    ))
                                )}
                            </div>
                        </div>
                    </div>
                ) : (
                    <div className="flex-1 flex items-center justify-center text-gray-400">
                        <div className="text-center">
                            <Clock className="w-12 h-12 mx-auto mb-3 opacity-20" />
                            <p className="text-sm">选择一个任务查看详情</p>
                        </div>
                    </div>
                )}
            </div>
        </>
    );
}

export default CronJobs;
