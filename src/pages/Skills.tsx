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
    ChevronRight,
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

    // Auto-clear notifications
    useEffect(() => {
        if (toast) { const t = setTimeout(() => setToast(''), 3000); return () => clearTimeout(t); }
    }, [toast]);
    useEffect(() => {
        if (error) { const t = setTimeout(() => setError(''), 5000); return () => clearTimeout(t); }
    }, [error]);

    // Load skills
    const loadSkills = useCallback(async () => {
        setLoading(true);
        try {
            const list = await invoke<Skill[]>('skills_list');
            setSkills(list);
            // Auto-select first or keep current
            if (list.length > 0 && (!selected || !list.find(s => s.name === selected.name))) {
                setSelected(list[0]);
            } else if (selected) {
                // Refresh selected detail
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

    // Toggle skill
    const handleToggle = async (skill: Skill) => {
        try {
            await invoke('skills_toggle', { name: skill.name, enabled: !skill.enabled });
            const updated = { ...skill, enabled: !skill.enabled };
            setSkills(prev => prev.map(s => s.name === skill.name ? updated : s));
            if (selected?.name === skill.name) setSelected(updated);
            setToast(`${skill.icon} ${skill.name} ${!skill.enabled ? '已启用' : '已禁用'}`);
        } catch (e: any) {
            setError(String(e));
        }
    };

    // Open skills directory
    const handleOpenDir = async () => {
        try {
            await invoke('skills_open_dir');
        } catch (e: any) {
            setError(String(e));
        }
    };

    // Create new skill
    const handleCreate = async () => {
        if (!newSkillName.trim()) return;
        const safeName = newSkillName.trim().toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
        try {
            await invoke<string>('skills_create', { name: safeName });
            setToast(`技能 "${safeName}" 已创建`);
            setShowCreateModal(false);
            setNewSkillName('');
            await loadSkills();
        } catch (e: any) {
            setError(String(e));
        }
    };

    // Install from Git
    const handleInstallGit = async () => {
        if (!gitUrl.trim()) return;
        setLoading(true);
        try {
            const name = await invoke<string>('skills_install_git', { url: gitUrl.trim() });
            setToast(`技能 "${name}" 安装成功`);
            setShowInstallModal(false);
            setGitUrl('');
            await loadSkills();
        } catch (e: any) {
            setError(String(e));
        } finally {
            setLoading(false);
        }
    };

    // Uninstall skill
    const handleUninstall = async (skill: Skill) => {
        if (!confirm(`确定要卸载技能 "${skill.name}" 吗？这将删除技能文件夹。`)) return;
        try {
            await invoke('skills_uninstall', { name: skill.name });
            setToast(`技能 "${skill.name}" 已卸载`);
            if (selected?.name === skill.name) setSelected(null);
            await loadSkills();
        } catch (e: any) {
            setError(String(e));
        }
    };

    // Filter skills
    const filtered = skills.filter(s =>
        !search ||
        s.name.toLowerCase().includes(search.toLowerCase()) ||
        s.description.toLowerCase().includes(search.toLowerCase()) ||
        s.tags.some(t => t.toLowerCase().includes(search.toLowerCase()))
    );

    const enabledCount = skills.filter(s => s.enabled).length;

    return (
        <div className="flex-1 flex flex-col h-full overflow-hidden">
            {/* Header */}
            <div className="px-8 py-5 border-b border-gray-200/60 dark:border-base-200/60">
                <div className="max-w-7xl mx-auto flex items-center justify-between">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 bg-gradient-to-br from-violet-400 to-purple-600 rounded-xl flex items-center justify-center shadow-lg shadow-purple-500/20">
                            <Puzzle className="w-5 h-5 text-white" />
                        </div>
                        <div>
                            <h1 className="text-xl font-bold text-gray-900 dark:text-white">技能管理</h1>
                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                {skills.length} 个技能 · {enabledCount} 个已启用
                            </p>
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={handleOpenDir}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-600 dark:text-gray-400 hover:text-purple-600 dark:hover:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20 rounded-lg transition-all"
                            title={skillsDir}
                        >
                            <FolderOpen className="w-4 h-4" />
                            打开目录
                        </button>
                        <button
                            onClick={() => setShowInstallModal(true)}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-600 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded-lg transition-all"
                        >
                            <GitBranch className="w-4 h-4" />
                            Git 安装
                        </button>
                        <button
                            onClick={() => setShowCreateModal(true)}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-purple-500 hover:bg-purple-600 text-white rounded-lg transition-all shadow-sm"
                        >
                            <Plus className="w-4 h-4" />
                            新建技能
                        </button>
                        <button
                            onClick={loadSkills}
                            disabled={loading}
                            className="p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-base-200 transition-all"
                            title="刷新"
                        >
                            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
                        </button>
                    </div>
                </div>
            </div>

            {/* Notifications */}
            {error && (
                <div className="mx-8 mt-3 max-w-7xl mx-auto">
                    <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm">
                        <AlertCircle className="w-4 h-4 flex-shrink-0" />
                        {error}
                    </div>
                </div>
            )}
            {toast && (
                <div className="mx-8 mt-3 max-w-7xl mx-auto">
                    <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-green-50 dark:bg-green-900/20 text-green-600 dark:text-green-400 text-sm">
                        <CheckCircle2 className="w-4 h-4 flex-shrink-0" />
                        {toast}
                    </div>
                </div>
            )}

            {/* Main Content: Left list + Right detail */}
            <div className="flex-1 flex overflow-hidden">
                {/* Left Sidebar: Skill List */}
                <div className="w-80 border-r border-gray-200/60 dark:border-base-200/60 flex flex-col bg-gray-50/30 dark:bg-base-100/30">
                    {/* Search */}
                    <div className="px-4 py-3 border-b border-gray-200/40 dark:border-base-200/40">
                        <div className="relative">
                            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                            <input
                                type="text"
                                value={search}
                                onChange={e => setSearch(e.target.value)}
                                placeholder="搜索技能..."
                                className="w-full pl-9 pr-3 py-2 text-sm bg-white dark:bg-base-200 border border-gray-200 dark:border-base-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-purple-500/30 focus:border-purple-400 text-gray-900 dark:text-white"
                            />
                        </div>
                    </div>

                    {/* Skill list */}
                    <div className="flex-1 overflow-y-auto">
                        {filtered.length === 0 ? (
                            <div className="p-8 text-center text-gray-400 dark:text-gray-500">
                                <Puzzle className="w-10 h-10 mx-auto mb-2 opacity-30" />
                                <p className="text-sm">没有找到技能</p>
                            </div>
                        ) : (
                            filtered.map(skill => (
                                <button
                                    key={skill.name}
                                    onClick={() => setSelected(skill)}
                                    className={`w-full px-4 py-3 flex items-start gap-3 text-left transition-all border-b border-gray-100/60 dark:border-base-200/40 hover:bg-white/80 dark:hover:bg-base-200/50 ${selected?.name === skill.name
                                            ? 'bg-white dark:bg-base-200 shadow-sm border-l-2 border-l-purple-500'
                                            : 'border-l-2 border-l-transparent'
                                        }`}
                                >
                                    <span className="text-xl mt-0.5 flex-shrink-0">{skill.icon}</span>
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium text-sm text-gray-900 dark:text-white truncate">{skill.name}</span>
                                            <span className="text-xs text-gray-400 dark:text-gray-500">v{skill.version}</span>
                                        </div>
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 line-clamp-2">{skill.description}</p>
                                        <div className="flex items-center gap-1.5 mt-1.5">
                                            <span className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium ${skill.enabled
                                                    ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                                                    : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-500'
                                                }`}>
                                                {skill.enabled ? '已启用' : '已禁用'}
                                            </span>
                                            {skill.tags.slice(0, 2).map(tag => (
                                                <span key={tag} className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-purple-50 dark:bg-purple-900/20 text-purple-600 dark:text-purple-400">
                                                    {tag}
                                                </span>
                                            ))}
                                        </div>
                                    </div>
                                    <ChevronRight className="w-4 h-4 text-gray-300 dark:text-gray-600 mt-1 flex-shrink-0" />
                                </button>
                            ))
                        )}
                    </div>
                </div>

                {/* Right: Skill Detail */}
                <div className="flex-1 overflow-y-auto">
                    {selected ? (
                        <div className="p-8 max-w-3xl">
                            {/* Skill header */}
                            <div className="flex items-start gap-4 mb-6">
                                <span className="text-4xl">{selected.icon}</span>
                                <div className="flex-1">
                                    <div className="flex items-center gap-3 mb-1">
                                        <h2 className="text-2xl font-bold text-gray-900 dark:text-white">{selected.name}</h2>
                                        <span className="text-sm text-gray-400 dark:text-gray-500">v{selected.version}</span>
                                    </div>
                                    <p className="text-gray-600 dark:text-gray-400 mb-3">{selected.description}</p>

                                    {/* Meta info */}
                                    <div className="flex items-center gap-4 text-sm text-gray-500 dark:text-gray-400">
                                        <span className="flex items-center gap-1">
                                            <User className="w-3.5 h-3.5" />
                                            {selected.author}
                                        </span>
                                        {selected.tags.length > 0 && (
                                            <span className="flex items-center gap-1">
                                                <Tag className="w-3.5 h-3.5" />
                                                {selected.tags.join(', ')}
                                            </span>
                                        )}
                                        {selected.homepage && (
                                            <a
                                                href={selected.homepage}
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                className="flex items-center gap-1 text-purple-500 hover:text-purple-600"
                                            >
                                                <ExternalLink className="w-3.5 h-3.5" />
                                                主页
                                            </a>
                                        )}
                                    </div>
                                </div>
                            </div>

                            {/* Action buttons */}
                            <div className="flex items-center gap-3 mb-6 pb-6 border-b border-gray-200/60 dark:border-base-200/60">
                                <button
                                    onClick={() => handleToggle(selected)}
                                    className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all ${selected.enabled
                                            ? 'bg-green-500 hover:bg-green-600 text-white shadow-sm shadow-green-500/20'
                                            : 'bg-gray-200 dark:bg-base-300 hover:bg-gray-300 dark:hover:bg-base-200 text-gray-700 dark:text-gray-300'
                                        }`}
                                >
                                    {selected.enabled ? <ToggleRight className="w-4 h-4" /> : <ToggleLeft className="w-4 h-4" />}
                                    {selected.enabled ? '已启用' : '点击启用'}
                                </button>
                                <button
                                    onClick={() => handleUninstall(selected)}
                                    className="flex items-center gap-1.5 px-3 py-2 text-sm text-red-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-all"
                                >
                                    <Trash2 className="w-4 h-4" />
                                    卸载
                                </button>
                            </div>

                            {/* File path */}
                            <div className="flex items-center gap-2 px-3 py-2 mb-6 rounded-lg bg-gray-50 dark:bg-base-200/50 text-xs text-gray-500 dark:text-gray-400 font-mono">
                                <FileText className="w-3.5 h-3.5 flex-shrink-0" />
                                <span className="truncate">{selected.path}</span>
                            </div>

                            {/* Skill body (Markdown rendered as plain text for now) */}
                            <div className="prose prose-sm dark:prose-invert max-w-none">
                                <div className="whitespace-pre-wrap text-sm text-gray-700 dark:text-gray-300 leading-relaxed">
                                    {selected.body.split('\n').map((line, i) => {
                                        if (line.startsWith('# ')) {
                                            return <h2 key={i} className="text-lg font-bold text-gray-900 dark:text-white mt-6 mb-3">{line.slice(2)}</h2>;
                                        }
                                        if (line.startsWith('## ')) {
                                            return <h3 key={i} className="text-base font-semibold text-gray-800 dark:text-gray-200 mt-5 mb-2">{line.slice(3)}</h3>;
                                        }
                                        if (line.startsWith('### ')) {
                                            return <h4 key={i} className="text-sm font-semibold text-gray-700 dark:text-gray-300 mt-4 mb-1">{line.slice(4)}</h4>;
                                        }
                                        if (line.startsWith('- ')) {
                                            return <div key={i} className="flex gap-2 ml-2"><span className="text-purple-400">•</span><span>{line.slice(2)}</span></div>;
                                        }
                                        if (line.startsWith('```')) {
                                            return <div key={i} className="text-xs font-mono text-purple-400">{line}</div>;
                                        }
                                        if (line.trim() === '') {
                                            return <div key={i} className="h-2" />;
                                        }
                                        return <p key={i} className="mb-1">{line}</p>;
                                    })}
                                </div>
                            </div>
                        </div>
                    ) : (
                        <div className="flex flex-col items-center justify-center h-full text-gray-400 dark:text-gray-500">
                            <Puzzle className="w-16 h-16 mb-4 opacity-20" />
                            <p className="text-lg">选择一个技能查看详情</p>
                            <p className="text-sm mt-1">技能存储在 <code className="text-xs bg-gray-100 dark:bg-base-200 px-2 py-0.5 rounded">{skillsDir || '~/.helix/skills/'}</code></p>
                        </div>
                    )}
                </div>
            </div>

            {/* Install Modal */}
            {showInstallModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-white dark:bg-base-100 rounded-2xl shadow-2xl w-[440px] p-6">
                        <h3 className="text-lg font-bold text-gray-900 dark:text-white mb-4 flex items-center gap-2">
                            <GitBranch className="w-5 h-5 text-blue-500" />
                            从 Git 安装技能
                        </h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
                            输入包含 SKILL.md 的 Git 仓库 URL
                        </p>
                        <input
                            type="text"
                            value={gitUrl}
                            onChange={e => setGitUrl(e.target.value)}
                            placeholder="https://github.com/user/skill-name.git"
                            className="w-full px-4 py-2.5 text-sm bg-white dark:bg-base-200 border border-gray-200 dark:border-base-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500/30 focus:border-blue-400 text-gray-900 dark:text-white mb-4"
                            onKeyDown={e => e.key === 'Enter' && handleInstallGit()}
                        />
                        <div className="flex justify-end gap-2">
                            <button
                                onClick={() => { setShowInstallModal(false); setGitUrl(''); }}
                                className="px-4 py-2 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-base-200 transition-all"
                            >
                                取消
                            </button>
                            <button
                                onClick={handleInstallGit}
                                disabled={!gitUrl.trim() || loading}
                                className="px-4 py-2 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded-lg disabled:opacity-50 transition-all flex items-center gap-2"
                            >
                                {loading && <Loader2 className="w-4 h-4 animate-spin" />}
                                安装
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Create Modal */}
            {showCreateModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-white dark:bg-base-100 rounded-2xl shadow-2xl w-[440px] p-6">
                        <h3 className="text-lg font-bold text-gray-900 dark:text-white mb-4 flex items-center gap-2">
                            <Plus className="w-5 h-5 text-purple-500" />
                            新建自定义技能
                        </h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
                            在 <code className="text-xs bg-gray-100 dark:bg-base-200 px-2 py-0.5 rounded">{skillsDir}</code> 创建技能模板
                        </p>
                        <input
                            type="text"
                            value={newSkillName}
                            onChange={e => setNewSkillName(e.target.value)}
                            placeholder="技能名称 (英文, 如 my-skill)"
                            className="w-full px-4 py-2.5 text-sm bg-white dark:bg-base-200 border border-gray-200 dark:border-base-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-purple-500/30 focus:border-purple-400 text-gray-900 dark:text-white mb-4"
                            onKeyDown={e => e.key === 'Enter' && handleCreate()}
                        />
                        <div className="flex justify-end gap-2">
                            <button
                                onClick={() => { setShowCreateModal(false); setNewSkillName(''); }}
                                className="px-4 py-2 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-base-200 transition-all"
                            >
                                取消
                            </button>
                            <button
                                onClick={handleCreate}
                                disabled={!newSkillName.trim()}
                                className="px-4 py-2 text-sm bg-purple-500 hover:bg-purple-600 text-white rounded-lg disabled:opacity-50 transition-all"
                            >
                                创建
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
