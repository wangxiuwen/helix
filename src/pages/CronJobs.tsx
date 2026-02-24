import { useState, useEffect, useCallback } from 'react';
import { Clock, Play, Plus, Trash2, Pause, History, Bell, RefreshCw, ChevronDown, ChevronUp, CheckCircle, XCircle, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface CronTask {
    id: string;
    name: string;
    description: string;
    type: string;
    schedule: string | null;
    script: string | null;
    status: string;
    notify_channel: string | null;
    created_at: string;
    updated_at: string;
    last_run: string | null;
    last_result: string | null;
    next_run: string | null;
}

interface CronRun {
    id: number;
    task_id: string;
    started_at: string;
    finished_at: string | null;
    result: string;
    output: string;
}

function CronJobs() {
    const [tasks, setTasks] = useState<CronTask[]>([]);
    const [loading, setLoading] = useState(true);
    const [showAdd, setShowAdd] = useState(false);
    const [runningTasks, setRunningTasks] = useState<Set<string>>(new Set());
    const [expandedTask, setExpandedTask] = useState<string | null>(null);
    const [taskRuns, setTaskRuns] = useState<Record<string, CronRun[]>>({});
    const [form, setForm] = useState({
        name: '', description: '', schedule: '', script: '',
        type: 'cron' as 'cron' | 'manual',
        notifyChannel: null as 'feishu' | 'dingtalk' | null,
    });

    const loadTasks = useCallback(async () => {
        try {
            const result = await invoke<CronTask[]>('cron_list_tasks');
            setTasks(result);
        } catch (e) {
            console.error('Failed to load tasks:', e);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        loadTasks();
    }, [loadTasks]);

    const handleAdd = async () => {
        if (!form.name) return;
        try {
            await invoke<CronTask>('cron_create_task', {
                input: {
                    name: form.name,
                    description: form.description || undefined,
                    type: form.type,
                    schedule: form.schedule || undefined,
                    script: form.script || undefined,
                    notify_channel: form.notifyChannel,
                },
            });
            setForm({ name: '', description: '', schedule: '', script: '', type: 'cron', notifyChannel: null });
            setShowAdd(false);
            await loadTasks();
        } catch (e) {
            console.error('Failed to create task:', e);
        }
    };

    const handleDelete = async (id: string) => {
        try {
            await invoke('cron_delete_task', { id });
            await loadTasks();
        } catch (e) {
            console.error('Failed to delete task:', e);
        }
    };

    const handleToggleStatus = async (task: CronTask) => {
        const newStatus = task.status === 'active' ? 'paused' : 'active';
        try {
            await invoke('cron_update_task', { id: task.id, input: { status: newStatus } });
            await loadTasks();
        } catch (e) {
            console.error('Failed to update task:', e);
        }
    };

    const handleRun = async (id: string) => {
        setRunningTasks(prev => new Set(prev).add(id));
        try {
            await invoke<CronRun>('cron_run_task', { id });
            await loadTasks();
            // Refresh run history if expanded
            if (expandedTask === id) {
                await loadRuns(id);
            }
        } catch (e) {
            console.error('Failed to run task:', e);
        } finally {
            setRunningTasks(prev => {
                const next = new Set(prev);
                next.delete(id);
                return next;
            });
        }
    };

    const loadRuns = async (taskId: string) => {
        try {
            const runs = await invoke<CronRun[]>('cron_get_runs', { taskId, limit: 10 });
            setTaskRuns(prev => ({ ...prev, [taskId]: runs }));
        } catch (e) {
            console.error('Failed to load runs:', e);
        }
    };

    const toggleExpand = async (taskId: string) => {
        if (expandedTask === taskId) {
            setExpandedTask(null);
        } else {
            setExpandedTask(taskId);
            await loadRuns(taskId);
        }
    };

    const formatDate = (dateStr: string | null) => {
        if (!dateStr) return null;
        try {
            return new Date(dateStr).toLocaleString();
        } catch {
            return dateStr;
        }
    };

    if (loading) {
        return (
            <div className="p-6 flex items-center justify-center h-full">
                <Loader2 className="animate-spin" size={32} />
            </div>
        );
    }

    return (
        <div className="p-6 space-y-6 overflow-y-auto h-full max-w-4xl mx-auto">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-base-content">定时任务</h1>
                    <p className="text-sm text-base-content/60 mt-1">管理定时执行的自动化任务（后端持久化）</p>
                </div>
                <div className="flex gap-2">
                    <button className="btn btn-ghost btn-sm" onClick={loadTasks} title="刷新">
                        <RefreshCw size={16} />
                    </button>
                    <button className="btn btn-primary btn-sm gap-2" onClick={() => setShowAdd(!showAdd)}>
                        <Plus size={16} />{showAdd ? '取消' : '创建任务'}
                    </button>
                </div>
            </div>

            {/* Add Form */}
            {showAdd && (
                <div className="card bg-base-100 shadow-md border border-base-200">
                    <div className="card-body space-y-3">
                        <h3 className="font-semibold">新建任务</h3>
                        <div className="grid grid-cols-2 gap-3">
                            <div>
                                <label className="text-xs text-base-content/50">任务名称</label>
                                <input className="input input-bordered input-sm w-full" placeholder="如：每日备份" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
                            </div>
                            <div>
                                <label className="text-xs text-base-content/50">任务类型</label>
                                <select className="select select-bordered select-sm w-full" value={form.type} onChange={(e) => setForm({ ...form, type: e.target.value as 'cron' | 'manual' })}>
                                    <option value="cron">定时 (Cron)</option>
                                    <option value="manual">手动触发</option>
                                </select>
                            </div>
                        </div>
                        {form.type === 'cron' && (
                            <div>
                                <label className="text-xs text-base-content/50">Cron 表达式</label>
                                <input className="input input-bordered input-sm w-full" placeholder="0 2 * * * (每天凌晨2点)" value={form.schedule} onChange={(e) => setForm({ ...form, schedule: e.target.value })} />
                                <p className="text-xs text-base-content/40 mt-1">格式: 分 时 日 月 星期</p>
                            </div>
                        )}
                        <div>
                            <label className="text-xs text-base-content/50">执行命令 / AI 指令</label>
                            <textarea className="textarea textarea-bordered textarea-sm w-full" rows={2} placeholder="如：list_k8s_pods 或 shell 命令" value={form.script} onChange={(e) => setForm({ ...form, script: e.target.value })} />
                        </div>
                        <div>
                            <label className="text-xs text-base-content/50">描述</label>
                            <input className="input input-bordered input-sm w-full" placeholder="任务说明（可选）" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} />
                        </div>
                        <div>
                            <label className="text-xs text-base-content/50">执行完通知</label>
                            <select className="select select-bordered select-sm w-full" value={form.notifyChannel || ''} onChange={(e) => setForm({ ...form, notifyChannel: (e.target.value || null) as any })}>
                                <option value="">不通知</option>
                                <option value="feishu">飞书</option>
                                <option value="dingtalk">钉钉</option>
                            </select>
                        </div>
                        <button className="btn btn-primary btn-sm" onClick={handleAdd} disabled={!form.name}>创建</button>
                    </div>
                </div>
            )}

            {/* Task List */}
            {tasks.length === 0 && !showAdd ? (
                <div className="card bg-base-100 shadow-md border border-base-200">
                    <div className="card-body text-center py-12 text-base-content/40">
                        <Clock size={48} className="mx-auto mb-4 opacity-30" />
                        <p className="text-lg">暂无定时任务</p>
                        <p className="text-sm mt-1">点击「创建任务」开始配置</p>
                    </div>
                </div>
            ) : (
                <div className="space-y-3">
                    {tasks.map((task) => (
                        <div key={task.id} className="card bg-base-100 shadow-md border border-base-200">
                            <div className="card-body p-5">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3 cursor-pointer" onClick={() => toggleExpand(task.id)}>
                                        <div className={`w-2.5 h-2.5 rounded-full ${task.status === 'active' ? 'bg-emerald-500' : task.status === 'error' ? 'bg-red-500' : 'bg-gray-400'}`} />
                                        <div>
                                            <h3 className="font-semibold text-sm text-base-content">{task.name}</h3>
                                            {task.description && <p className="text-xs text-base-content/50">{task.description}</p>}
                                        </div>
                                        {expandedTask === task.id ? <ChevronUp size={14} className="text-base-content/40" /> : <ChevronDown size={14} className="text-base-content/40" />}
                                    </div>
                                    <div className="flex items-center gap-2">
                                        {task.notify_channel && (
                                            <span className="text-xs px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-500 flex items-center gap-1">
                                                <Bell size={10} />{task.notify_channel === 'feishu' ? '飞书' : '钉钉'}
                                            </span>
                                        )}
                                        <button className="btn btn-ghost btn-xs" onClick={() => handleRun(task.id)} title="手动执行" disabled={runningTasks.has(task.id)}>
                                            {runningTasks.has(task.id) ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} className="text-emerald-500" />}
                                        </button>
                                        <button className="btn btn-ghost btn-xs" onClick={() => handleToggleStatus(task)} title={task.status === 'active' ? '暂停' : '恢复'}>
                                            <Pause size={14} className={task.status === 'active' ? 'text-amber-500' : 'text-emerald-500'} />
                                        </button>
                                        <button className="btn btn-ghost btn-xs text-red-500" onClick={() => handleDelete(task.id)}>
                                            <Trash2 size={14} />
                                        </button>
                                    </div>
                                </div>
                                <div className="flex items-center gap-4 mt-2 text-xs text-base-content/50">
                                    {task.schedule && (
                                        <span className="flex items-center gap-1"><Clock size={12} />{task.schedule}</span>
                                    )}
                                    {task.next_run && (
                                        <span className="flex items-center gap-1 text-blue-500">下次: {formatDate(task.next_run)}</span>
                                    )}
                                    {task.last_run && (
                                        <span className="flex items-center gap-1">
                                            <History size={12} />上次: {formatDate(task.last_run)}
                                            {task.last_result && (
                                                <span className={task.last_result === 'success' ? 'text-emerald-500' : 'text-red-500'}>
                                                    ({task.last_result === 'success' ? '成功' : '失败'})
                                                </span>
                                            )}
                                        </span>
                                    )}
                                    <span className="px-2 py-0.5 rounded bg-base-200 text-base-content/60">
                                        {task.type === 'cron' ? '定时' : '手动'}
                                    </span>
                                </div>
                                {task.script && (
                                    <div className="mt-2 p-2 bg-base-200/50 rounded text-xs font-mono text-base-content/60 truncate">
                                        {task.script}
                                    </div>
                                )}

                                {/* Run History */}
                                {expandedTask === task.id && (
                                    <div className="mt-3 border-t border-base-200 pt-3">
                                        <h4 className="text-xs font-semibold text-base-content/60 mb-2">执行历史</h4>
                                        {!taskRuns[task.id] || taskRuns[task.id].length === 0 ? (
                                            <p className="text-xs text-base-content/40">暂无执行记录</p>
                                        ) : (
                                            <div className="space-y-2 max-h-60 overflow-y-auto">
                                                {taskRuns[task.id].map((run) => (
                                                    <div key={run.id} className="p-2 bg-base-200/30 rounded text-xs">
                                                        <div className="flex items-center justify-between">
                                                            <div className="flex items-center gap-2">
                                                                {run.result === 'success' ? (
                                                                    <CheckCircle size={12} className="text-emerald-500" />
                                                                ) : run.result === 'running' ? (
                                                                    <Loader2 size={12} className="animate-spin text-blue-500" />
                                                                ) : (
                                                                    <XCircle size={12} className="text-red-500" />
                                                                )}
                                                                <span className="text-base-content/60">{formatDate(run.started_at)}</span>
                                                            </div>
                                                            {run.finished_at && (
                                                                <span className="text-base-content/40">{formatDate(run.finished_at)}</span>
                                                            )}
                                                        </div>
                                                        {run.output && (
                                                            <pre className="mt-1 p-1.5 bg-base-300/50 rounded text-[10px] font-mono text-base-content/50 max-h-32 overflow-auto whitespace-pre-wrap">
                                                                {run.output}
                                                            </pre>
                                                        )}
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}

export default CronJobs;
