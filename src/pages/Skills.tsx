import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    Puzzle,
    Search,
    FolderOpen,
    Plus,
    RefreshCw,
    GitBranch,
    Trash2,
    ToggleLeft,
    ToggleRight,
    Tag,
    User,
    ExternalLink,
    Loader2,
    AlertCircle,
    CheckCircle2,
    FileText,
} from 'lucide-react';

interface Skill {
    name: string;
    description: string;
    icon: string;
    version: string;
    author: string;
    tags: string[];
    path: string;
    enabled: boolean;
    body: string;
    homepage: string;
}

export default function Skills() {
    const [skills, setSkills] = useState<Skill[]>([]);
    const [selected, setSelected] = useState<Skill | null>(null);
    const [search, setSearch] = useState('');
    const [loading, setLoading] = useState(false);
    const [toast, setToast] = useState('');
    const [error, setError] = useState('');
    const [skillsDir, setSkillsDir] = useState('');
    const [showInstallModal, setShowInstallModal] = useState(false);
    const [showCreateModal, setShowCreateModal] = useState(false);
    const [gitUrl, setGitUrl] = useState('');
    const [newSkillName, setNewSkillName] = useState('');

    useEffect(() => {
        if (toast) { const t = setTimeout(() => setToast(''), 3000); return () => clearTimeout(t); }
    }, [toast]);
    useEffect(() => {
        if (error) { const t = setTimeout(() => setError(''), 5000); return () => clearTimeout(t); }
    }, [error]);

    const loadSkills = useCallback(async () => {
        setLoading(true);
        try {
            const list = await invoke<Skill[]>('skills_list');
            setSkills(list);
            if (list.length > 0 && (!selected || !list.find(s => s.name === selected.name))) {
                setSelected(list[0]);
            } else if (selected) {
                const updated = list.find(s => s.name === selected.name);
                if (updated) setSelected(updated);
            }
        } catch (e: any) {
            setError(String(e));
        } finally {
            setLoading(false);
        }
    }, [selected]);

    useEffect(() => {
        loadSkills();
        invoke<string>('skills_get_dir').then(setSkillsDir).catch(() => { });
    }, []);

    const handleToggle = async (skill: Skill) => {
        try {
            await invoke('skills_toggle', { name: skill.name, enabled: !skill.enabled });
            const updated = { ...skill, enabled: !skill.enabled };
            setSkills(prev => prev.map(s => s.name === skill.name ? updated : s));
            if (selected?.name === skill.name) setSelected(updated);
            setToast(`${skill.icon} ${skill.name} ${!skill.enabled ? '已启用' : '已禁用'}`);
        } catch (e: any) { setError(String(e)); }
    };

    const handleOpenDir = async () => {
        try { await invoke('skills_open_dir'); } catch (e: any) { setError(String(e)); }
    };

    const handleCreate = async () => {
        if (!newSkillName.trim()) return;
        const safeName = newSkillName.trim().toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
        try {
            await invoke<string>('skills_create', { name: safeName });
            setToast(`技能 "${safeName}" 已创建`);
            setShowCreateModal(false);
            setNewSkillName('');
            await loadSkills();
        } catch (e: any) { setError(String(e)); }
    };

    const handleInstallGit = async () => {
        if (!gitUrl.trim()) return;
        setLoading(true);
        try {
            const name = await invoke<string>('skills_install_git', { url: gitUrl.trim() });
            setToast(`技能 "${name}" 安装成功`);
            setShowInstallModal(false);
            setGitUrl('');
            await loadSkills();
        } catch (e: any) { setError(String(e)); } finally { setLoading(false); }
    };

    const handleUninstall = async (skill: Skill) => {
        if (!confirm(`确定要卸载技能 "${skill.name}" 吗？`)) return;
        try {
            await invoke('skills_uninstall', { name: skill.name });
            setToast(`技能 "${skill.name}" 已卸载`);
            if (selected?.name === skill.name) setSelected(null);
            await loadSkills();
        } catch (e: any) { setError(String(e)); }
    };

    const filtered = skills.filter(s =>
        !search || s.name.toLowerCase().includes(search.toLowerCase()) ||
        s.description.toLowerCase().includes(search.toLowerCase()) ||
        s.tags.some(t => t.toLowerCase().includes(search.toLowerCase()))
    );

    const enabledCount = skills.filter(s => s.enabled).length;

    return (
        <>
            {/* Left: Skill list (same width/style as chat session list) */}
            <div className="w-[250px] shrink-0 bg-[#f7f7f7] dark:bg-[#252525] flex flex-col border-r border-black/5 dark:border-white/5">
                {/* Header */}
                <div className="px-3 pt-4 pb-1">
                    <div className="flex items-center justify-between mb-2">
                        <span className="text-xs text-gray-400">{skills.length} 个技能 · {enabledCount} 已启用</span>
                        <div className="flex items-center gap-1">
                            <button onClick={loadSkills} disabled={loading} className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 text-gray-400" title="刷新">
                                <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
                            </button>
                            <button onClick={() => setShowCreateModal(true)} className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 text-gray-400" title="新建技能">
                                <Plus className="w-3.5 h-3.5" />
                            </button>
                        </div>
                    </div>
                    <div className="relative">
                        <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                        <input
                            type="text"
                            value={search}
                            onChange={e => setSearch(e.target.value)}
                            placeholder="搜索技能..."
                            className="w-full pl-8 pr-3 py-1.5 text-xs bg-white dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400"
                        />
                    </div>
                </div>

                {/* Skill list */}
                <div className="flex-1 overflow-y-auto">
                    {filtered.length === 0 ? (
                        <div className="px-4 py-12 text-center text-gray-400 text-xs">没有找到技能</div>
                    ) : (
                        filtered.map(skill => (
                            <div
                                key={skill.name}
                                onClick={() => setSelected(skill)}
                                className={`flex items-center px-3 py-3 cursor-pointer transition-colors group ${selected?.name === skill.name
                                    ? 'bg-[#c9c9c9] dark:bg-[#383838]'
                                    : 'hover:bg-[#ebebeb] dark:hover:bg-[#303030]'
                                    }`}
                            >
                                <div className="w-10 h-10 rounded-lg bg-gray-200 dark:bg-[#404040] flex items-center justify-center shrink-0 mr-3 text-lg">
                                    {skill.icon || '🧩'}
                                </div>
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center justify-between">
                                        <span className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">{skill.name}</span>
                                        <span className="text-[10px] text-gray-400 shrink-0 ml-2">v{skill.version}</span>
                                    </div>
                                    <div className="flex items-center gap-1.5 mt-0.5">
                                        <p className="text-xs text-gray-400 truncate flex-1">{skill.description}</p>
                                        <span className={`text-[10px] px-1.5 py-0.5 rounded ${skill.enabled ? 'bg-[#07c160]/10 text-[#07c160]' : 'bg-gray-200 dark:bg-gray-700 text-gray-400'}`}>
                                            {skill.enabled ? '启用' : '禁用'}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* Right: Skill detail */}
            <div className="flex-1 flex flex-col min-w-0 bg-[#f5f5f5] dark:bg-[#1e1e1e]">
                {/* Notifications */}
                {(error || toast) && (
                    <div className="px-5 pt-3">
                        {error && (
                            <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-red-50 dark:bg-red-900/20 text-red-500 text-xs mb-2">
                                <AlertCircle className="w-3.5 h-3.5 shrink-0" />{error}
                            </div>
                        )}
                        {toast && (
                            <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-green-50 dark:bg-green-900/20 text-[#07c160] text-xs">
                                <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />{toast}
                            </div>
                        )}
                    </div>
                )}

                {/* Header bar */}
                <div className="h-14 px-5 flex items-center justify-between border-b border-black/5 dark:border-white/5 shrink-0" data-tauri-drag-region>
                    <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200">
                        {selected ? selected.name : '技能管理'}
                    </h3>
                    <div className="flex items-center gap-1">
                        <button onClick={handleOpenDir} className="px-2 py-1 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/10 rounded transition-colors flex items-center gap-1" title={skillsDir}>
                            <FolderOpen className="w-3.5 h-3.5" />打开目录
                        </button>
                        <button onClick={() => setShowInstallModal(true)} className="px-2 py-1 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/10 rounded transition-colors flex items-center gap-1">
                            <GitBranch className="w-3.5 h-3.5" />Git 安装
                        </button>
                    </div>
                </div>

                {selected ? (
                    <div className="flex-1 overflow-y-auto px-8 py-6">
                        <div className="max-w-2xl">
                            {/* Skill header */}
                            <div className="flex items-start gap-4 mb-5">
                                <span className="text-4xl">{selected.icon}</span>
                                <div className="flex-1">
                                    <div className="flex items-center gap-2 mb-1">
                                        <h2 className="text-xl font-bold text-gray-800 dark:text-gray-100">{selected.name}</h2>
                                        <span className="text-xs text-gray-400">v{selected.version}</span>
                                    </div>
                                    <p className="text-sm text-gray-500 dark:text-gray-400 mb-2">{selected.description}</p>
                                    <div className="flex items-center gap-3 text-xs text-gray-400">
                                        <span className="flex items-center gap-1"><User className="w-3 h-3" />{selected.author}</span>
                                        {selected.tags.length > 0 && <span className="flex items-center gap-1"><Tag className="w-3 h-3" />{selected.tags.join(', ')}</span>}
                                        {selected.homepage && (
                                            <a href={selected.homepage} target="_blank" rel="noopener noreferrer" className="flex items-center gap-1 text-[#07c160] hover:underline">
                                                <ExternalLink className="w-3 h-3" />主页
                                            </a>
                                        )}
                                    </div>
                                </div>
                            </div>

                            {/* Actions */}
                            <div className="flex items-center gap-3 mb-5 pb-5 border-b border-black/5 dark:border-white/5">
                                <button
                                    onClick={() => handleToggle(selected)}
                                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${selected.enabled
                                        ? 'bg-[#07c160] text-white hover:bg-[#06ad56]'
                                        : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600'
                                        }`}
                                >
                                    {selected.enabled ? <ToggleRight className="w-4 h-4" /> : <ToggleLeft className="w-4 h-4" />}
                                    {selected.enabled ? '已启用' : '点击启用'}
                                </button>
                                <button onClick={() => handleUninstall(selected)} className="flex items-center gap-1 px-3 py-1.5 text-xs text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-md transition-colors">
                                    <Trash2 className="w-3.5 h-3.5" />卸载
                                </button>
                            </div>

                            {/* File path */}
                            <div className="flex items-center gap-2 px-3 py-2 mb-5 rounded-md bg-white dark:bg-[#2e2e2e] text-[11px] text-gray-400 font-mono">
                                <FileText className="w-3 h-3 shrink-0" /><span className="truncate">{selected.path}</span>
                            </div>

                            {/* Skill body */}
                            <div className="text-sm text-gray-600 dark:text-gray-300 leading-relaxed">
                                {selected.body.split('\n').map((line, i) => {
                                    if (line.startsWith('# ')) return <h2 key={i} className="text-base font-bold text-gray-800 dark:text-white mt-5 mb-2">{line.slice(2)}</h2>;
                                    if (line.startsWith('## ')) return <h3 key={i} className="text-sm font-semibold text-gray-700 dark:text-gray-200 mt-4 mb-1.5">{line.slice(3)}</h3>;
                                    if (line.startsWith('### ')) return <h4 key={i} className="text-sm font-medium text-gray-600 dark:text-gray-300 mt-3 mb-1">{line.slice(4)}</h4>;
                                    if (line.startsWith('- ')) return <div key={i} className="flex gap-2 ml-2"><span className="text-[#07c160]">•</span><span>{line.slice(2)}</span></div>;
                                    if (line.trim() === '') return <div key={i} className="h-2" />;
                                    return <p key={i} className="mb-1">{line}</p>;
                                })}
                            </div>
                        </div>
                    </div>
                ) : (
                    <div className="flex-1 flex items-center justify-center text-gray-400">
                        <div className="text-center">
                            <Puzzle className="w-12 h-12 mx-auto mb-3 opacity-20" />
                            <p className="text-sm">选择一个技能查看详情</p>
                        </div>
                    </div>
                )}
            </div>

            {/* Install Modal */}
            {showInstallModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-white dark:bg-[#2e2e2e] rounded-xl shadow-xl w-[400px] p-5">
                        <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-1 flex items-center gap-2"><GitBranch className="w-4 h-4 text-[#07c160]" />从 Git 安装技能</h3>
                        <p className="text-xs text-gray-400 mb-3">输入包含 SKILL.md 的 Git 仓库 URL</p>
                        <input type="text" value={gitUrl} onChange={e => setGitUrl(e.target.value)} placeholder="https://github.com/user/skill.git"
                            className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 mb-3"
                            onKeyDown={e => e.key === 'Enter' && handleInstallGit()} />
                        <div className="flex justify-end gap-2">
                            <button onClick={() => { setShowInstallModal(false); setGitUrl(''); }} className="px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md">取消</button>
                            <button onClick={handleInstallGit} disabled={!gitUrl.trim() || loading} className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md disabled:opacity-50 flex items-center gap-1">
                                {loading && <Loader2 className="w-3 h-3 animate-spin" />}安装
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Create Modal */}
            {showCreateModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-white dark:bg-[#2e2e2e] rounded-xl shadow-xl w-[400px] p-5">
                        <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-1 flex items-center gap-2"><Plus className="w-4 h-4 text-[#07c160]" />新建自定义技能</h3>
                        <p className="text-xs text-gray-400 mb-3">在 <code className="text-[11px] bg-gray-100 dark:bg-gray-700 px-1.5 py-0.5 rounded">{skillsDir}</code> 创建模板</p>
                        <input type="text" value={newSkillName} onChange={e => setNewSkillName(e.target.value)} placeholder="技能名称 (英文, 如 my-skill)"
                            className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-md border-0 outline-none text-gray-700 dark:text-gray-200 mb-3"
                            onKeyDown={e => e.key === 'Enter' && handleCreate()} />
                        <div className="flex justify-end gap-2">
                            <button onClick={() => { setShowCreateModal(false); setNewSkillName(''); }} className="px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md">取消</button>
                            <button onClick={handleCreate} disabled={!newSkillName.trim()} className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-md disabled:opacity-50">创建</button>
                        </div>
                    </div>
                </div>
            )}
        </>
    );
}
