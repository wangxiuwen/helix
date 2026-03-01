import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    Puzzle,
    Search,
    FolderOpen,
    Plus,
    RefreshCw,
    Trash2,
    Tag,
    User,
    ExternalLink,
    Loader2,
    AlertCircle,
    CheckCircle2,
    FileText,
    Download,
    X,
    Eye,
    Copy,
    Globe,
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

// Built-in curated skill registry for Skills Hub (matching CoPaw's hub)
const HUB_SKILLS = [
    {
        name: 'web-search', description: '增强的网页搜索与内容抓取能力',
        icon: '🔍', author: 'helix-team', version: '1.0.0', tags: ['search', 'web'],
        url: 'https://github.com/helix-ai/skills',
        readme: '使 Agent 能够搜索网页并提取关键信息。支持多种搜索引擎和内容解析格式。',
    },
    {
        name: 'code-review', description: '自动代码审查与优化建议',
        icon: '🔬', author: 'helix-team', version: '1.0.0', tags: ['code', 'review'],
        url: 'https://github.com/helix-ai/skills',
        readme: '对代码进行全面审查，提供安全性、性能和可读性方面的改进建议。',
    },
    {
        name: 'git-assistant', description: 'Git 提交、分支、PR 管理助手',
        icon: '🌿', author: 'helix-team', version: '1.0.0', tags: ['git', 'devops'],
        url: 'https://github.com/helix-ai/skills',
        readme: '帮助管理 Git 工作流，包括提交信息生成、分支策略、PR 描述等。',
    },
    {
        name: 'docker-ops', description: 'Docker 容器管理与部署',
        icon: '🐳', author: 'helix-team', version: '1.0.0', tags: ['docker', 'devops'],
        url: 'https://github.com/helix-ai/skills',
        readme: '管理 Docker 容器和镜像，支持 Compose 文件生成和容器编排。',
    },
    {
        name: 'sql-assistant', description: 'SQL 查询生成与数据库分析',
        icon: '🗄️', author: 'helix-team', version: '1.0.0', tags: ['sql', 'database'],
        url: 'https://github.com/helix-ai/skills',
        readme: '自动生成复杂 SQL 查询，分析数据库结构并提供优化建议。',
    },
    {
        name: 'api-tester', description: 'HTTP API 测试与调试工具',
        icon: '🌐', author: 'helix-team', version: '1.0.0', tags: ['api', 'test'],
        url: 'https://github.com/helix-ai/skills',
        readme: '测试和调试 HTTP API，自动生成测试用例并分析响应结果。',
    },
    {
        name: 'markdown-writer', description: '文档写作与 Markdown 格式化',
        icon: '📝', author: 'helix-team', version: '1.0.0', tags: ['writing', 'docs'],
        url: 'https://github.com/helix-ai/skills',
        readme: '辅助创建高质量文档，自动格式化 Markdown 并生成目录结构。',
    },
    {
        name: 'data-analysis', description: '数据分析与可视化报告生成',
        icon: '📊', author: 'helix-team', version: '1.0.0', tags: ['data', 'analysis'],
        url: 'https://github.com/helix-ai/skills',
        readme: '分析数据集并生成可视化报告，支持 CSV、JSON 等多种数据格式。',
    },
    {
        name: 'translate', description: '多语言翻译与本地化',
        icon: '🌍', author: 'helix-team', version: '1.0.0', tags: ['i18n', 'translate'],
        url: 'https://github.com/helix-ai/skills',
        readme: '支持 100+ 种语言的高质量翻译，保留原文格式和语义。',
    },
];

const SUPPORTED_URL_PREFIXES = [
    'https://skills.sh/',
    'https://clawhub.ai/',
    'https://skillsmp.com/',
    'https://github.com/',
];

type TabKey = 'local' | 'hub';

export default function Skills() {
    const [tab, setTab] = useState<TabKey>('local');

    // Local skills
    const [skills, setSkills] = useState<Skill[]>([]);
    const [loading, setLoading] = useState(false);
    const [toast, setToast] = useState('');
    const [error, setError] = useState('');
    const [skillsDir, setSkillsDir] = useState('');
    const [search, setSearch] = useState('');

    // Drawer
    const [drawerSkill, setDrawerSkill] = useState<Skill | null>(null);
    const [drawerOpen, setDrawerOpen] = useState(false);
    const [showPreview, setShowPreview] = useState(true);

    // Hub
    const [hubSearch, setHubSearch] = useState('');
    const [hubSelected, setHubSelected] = useState<typeof HUB_SKILLS[0] | null>(null);

    // Modals
    const [showCreateModal, setShowCreateModal] = useState(false);
    const [newSkillName, setNewSkillName] = useState('');
    const [importModalOpen, setImportModalOpen] = useState(false);
    const [importUrl, setImportUrl] = useState('');
    const [importUrlError, setImportUrlError] = useState('');
    const [importing, setImporting] = useState(false);

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
            if (drawerSkill) {
                const updated = list.find(s => s.name === drawerSkill.name);
                if (updated) setDrawerSkill(updated);
            }
        } catch (e: unknown) { setError(String(e)); }
        finally { setLoading(false); }
    }, [drawerSkill]);

    useEffect(() => {
        loadSkills();
        invoke<string>('skills_get_dir').then(setSkillsDir).catch(() => { });
        let unlisten: (() => void) | null = null;
        import('@tauri-apps/api/event').then(({ listen }) => {
            listen<unknown>('skills-changed', () => loadSkills()).then(fn => { unlisten = fn; });
        });
        return () => { if (unlisten) unlisten(); };
    }, []);

    const handleToggle = async (skill: Skill, e?: React.MouseEvent) => {
        e?.stopPropagation();
        try {
            await invoke('skills_toggle', { name: skill.name, enabled: !skill.enabled });
            const updated = { ...skill, enabled: !skill.enabled };
            setSkills(prev => prev.map(s => s.name === skill.name ? updated : s));
            if (drawerSkill?.name === skill.name) setDrawerSkill(updated);
            setToast(`${skill.icon} ${skill.name} ${!skill.enabled ? '已启用' : '已禁用'}`);
        } catch (e: unknown) { setError(String(e)); }
    };

    const handleDelete = async (skill: Skill, e?: React.MouseEvent) => {
        e?.stopPropagation();
        if (!confirm(`确定要卸载技能 "${skill.name}" 吗？`)) return;
        try {
            await invoke('skills_uninstall', { name: skill.name });
            setToast(`技能 "${skill.name}" 已卸载`);
            if (drawerSkill?.name === skill.name) { setDrawerOpen(false); setDrawerSkill(null); }
            await loadSkills();
        } catch (e: unknown) { setError(String(e)); }
    };

    const handleOpenDir = async () => { try { await invoke('skills_open_dir'); } catch (e: unknown) { setError(String(e)); } };

    const handleCreate = async () => {
        if (!newSkillName.trim()) return;
        const safeName = newSkillName.trim().toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
        try {
            await invoke<string>('skills_create', { name: safeName });
            setToast(`技能 "${safeName}" 已创建`);
            setShowCreateModal(false);
            setNewSkillName('');
            await loadSkills();
        } catch (e: unknown) { setError(String(e)); }
    };

    const isSupportedUrl = (url: string) => SUPPORTED_URL_PREFIXES.some(p => url.startsWith(p));

    const handleImportUrlChange = (val: string) => {
        setImportUrl(val);
        const trimmed = val.trim();
        setImportUrlError(trimmed && !isSupportedUrl(trimmed)
            ? '不支持该来源，请使用 skills.sh / clawhub.ai / skillsmp.com / github.com' : '');
    };

    const handleHubInstall = async () => {
        if (importing) return;
        const trimmed = importUrl.trim();
        if (!trimmed || !isSupportedUrl(trimmed)) return;
        setImporting(true);
        try {
            const result = await invoke<{ installed: boolean; name: string }>('skills_hub_install', { bundleUrl: trimmed });
            if (result?.installed) {
                setToast(`技能 "${result.name}" 安装成功`);
                setImportModalOpen(false);
                setImportUrl('');
                setImportUrlError('');
                setTab('local');
                await loadSkills();
            } else { setError('安装失败'); }
        } catch (e: unknown) { setError(String(e)); }
        finally { setImporting(false); }
    };

    const handleCopyBody = (body: string) => { navigator.clipboard.writeText(body); setToast('已复制到剪贴板'); };

    const openDrawer = (skill: Skill) => { setDrawerSkill(skill); setDrawerOpen(true); };

    const filtered = skills.filter(s =>
        !search || s.name.toLowerCase().includes(search.toLowerCase()) ||
        s.description.toLowerCase().includes(search.toLowerCase()) ||
        s.tags.some(t => t.toLowerCase().includes(search.toLowerCase()))
    );
    const sorted = [...filtered].sort((a, b) => {
        if (a.enabled && !b.enabled) return -1;
        if (!a.enabled && b.enabled) return 1;
        return a.name.localeCompare(b.name);
    });

    const filteredHub = HUB_SKILLS.filter(s =>
        !hubSearch || s.name.toLowerCase().includes(hubSearch.toLowerCase()) ||
        s.description.toLowerCase().includes(hubSearch.toLowerCase()) ||
        s.tags.some(t => t.toLowerCase().includes(hubSearch.toLowerCase()))
    );

    const enabledCount = skills.filter(s => s.enabled).length;

    return (
        <div className="flex-1 flex flex-col min-w-0 h-full bg-[#FAFBFC] dark:bg-base-300 relative">
            {/* Notifications */}
            {(error || toast) && (
                <div className="absolute top-4 right-4 z-40 max-w-sm space-y-2">
                    {error && (
                        <div className="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-red-50 dark:bg-red-900/30 text-red-500 text-xs shadow-lg">
                            <AlertCircle className="w-4 h-4 shrink-0" />{error}
                        </div>
                    )}
                    {toast && (
                        <div className="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-green-50 dark:bg-green-900/30 text-[#07c160] text-xs shadow-lg">
                            <CheckCircle2 className="w-4 h-4 shrink-0" />{toast}
                        </div>
                    )}
                </div>
            )}

            {/* Header */}
            <div className="shrink-0 px-8 pt-8 pb-4" data-tauri-drag-region>
                <div className="flex items-center justify-between mb-4">
                    <div>
                        <h1 className="text-xl font-bold text-gray-800 dark:text-white">Skills</h1>
                        <p className="text-xs text-gray-400 mt-0.5">管理 Agent 技能与能力扩展 · {skills.length} 个技能 · {enabledCount} 已启用</p>
                    </div>
                    <div className="flex items-center gap-2">
                        <button onClick={handleOpenDir} className="px-3 py-1.5 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-black/5 dark:hover:bg-white/10 rounded-lg transition-colors flex items-center gap-1.5" title={skillsDir}>
                            <FolderOpen size={14} />打开目录
                        </button>
                        <button onClick={() => setImportModalOpen(true)} className="flex items-center gap-1.5 px-3 py-1.5 bg-white dark:bg-[#2e2e2e] hover:bg-gray-50 dark:hover:bg-[#383838] text-gray-700 dark:text-gray-200 text-xs rounded-lg transition-colors border border-gray-200 dark:border-gray-700">
                            <Download size={14} />导入技能
                        </button>
                        <button onClick={() => setShowCreateModal(true)} className="flex items-center gap-1.5 px-3 py-1.5 bg-[#07c160] hover:bg-[#06ad56] text-white text-xs rounded-lg transition-colors">
                            <Plus size={14} />创建技能
                        </button>
                    </div>
                </div>

                {/* Tabs */}
                <div className="flex items-center gap-1 border-b border-gray-200 dark:border-gray-700">
                    <button
                        onClick={() => setTab('local')}
                        className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors -mb-px ${tab === 'local' ? 'border-[#07c160] text-[#07c160]' : 'border-transparent text-gray-400 hover:text-gray-600 dark:hover:text-gray-300'}`}
                    >
                        本地技能
                    </button>
                    <button
                        onClick={() => setTab('hub')}
                        className={`flex items-center gap-1.5 px-4 py-2 text-sm font-medium border-b-2 transition-colors -mb-px ${tab === 'hub' ? 'border-[#07c160] text-[#07c160]' : 'border-transparent text-gray-400 hover:text-gray-600 dark:hover:text-gray-300'}`}
                    >
                        <Globe size={14} />Skills Hub
                    </button>
                </div>
            </div>

            {/* Tab content */}
            {tab === 'local' ? (
                <>
                    <div className="px-8 pb-3">
                        <div className="flex items-center gap-2">
                            <div className="relative flex-1 max-w-xs">
                                <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                                <input type="text" value={search} onChange={e => setSearch(e.target.value)} placeholder="搜索技能..."
                                    className="w-full pl-9 pr-3 py-2 text-sm bg-white dark:bg-[#2e2e2e] rounded-lg border border-gray-200 dark:border-gray-700 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400 focus:border-[#07c160] transition-colors" />
                            </div>
                            <button onClick={loadSkills} disabled={loading} className="p-2 rounded-lg hover:bg-black/5 dark:hover:bg-white/5 text-gray-400 transition-colors" title="刷新">
                                <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
                            </button>
                        </div>
                    </div>

                    <div className="flex-1 overflow-y-auto px-8 pb-8">
                        {sorted.length === 0 ? (
                            <div className="flex flex-col items-center justify-center h-64 text-gray-400">
                                <Puzzle className="w-12 h-12 mb-3 opacity-20" />
                                <p className="text-sm">{search ? '没有匹配的技能' : '暂无技能'}</p>
                                <p className="text-xs mt-1 opacity-70">点击"创建技能"或"导入技能"开始</p>
                            </div>
                        ) : (
                            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                {sorted.map(skill => (
                                    <div key={skill.name} onClick={() => openDrawer(skill)}
                                        className="group relative p-4 rounded-xl bg-white dark:bg-[#2e2e2e] border border-gray-100 dark:border-gray-800 hover:shadow-md hover:border-gray-200 dark:hover:border-gray-700 cursor-pointer transition-all">
                                        <div className="flex items-start justify-between mb-3">
                                            <div className="flex items-center gap-2.5">
                                                <div className="w-9 h-9 rounded-lg bg-gray-100 dark:bg-[#404040] flex items-center justify-center text-lg shrink-0">{skill.icon || '🧩'}</div>
                                                <div className="min-w-0">
                                                    <h3 className="text-sm font-semibold text-gray-800 dark:text-gray-100 truncate">{skill.name}</h3>
                                                    <span className="text-[10px] text-gray-400">v{skill.version}</span>
                                                </div>
                                            </div>
                                            <div className={`flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full shrink-0 ${skill.enabled ? 'bg-[#07c160]/10 text-[#07c160]' : 'bg-gray-100 dark:bg-gray-700 text-gray-400'}`}>
                                                <span className={`w-1.5 h-1.5 rounded-full ${skill.enabled ? 'bg-[#07c160]' : 'bg-gray-400'}`} />
                                                {skill.enabled ? '启用' : '禁用'}
                                            </div>
                                        </div>
                                        <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-2 mb-3 min-h-[2rem]">{skill.description || '暂无描述'}</p>
                                        {skill.tags.length > 0 && (
                                            <div className="flex flex-wrap gap-1 mb-3">
                                                {skill.tags.slice(0, 3).map(tag => (
                                                    <span key={tag} className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400">{tag}</span>
                                                ))}
                                                {skill.tags.length > 3 && <span className="text-[10px] text-gray-400">+{skill.tags.length - 3}</span>}
                                            </div>
                                        )}
                                        <div className="text-[10px] text-gray-400 font-mono truncate mb-3">{skill.path}</div>
                                        <div className="flex items-center justify-between pt-2 border-t border-gray-100 dark:border-gray-700">
                                            <span className="text-[10px] text-gray-400 flex items-center gap-1"><User size={10} />{skill.author}</span>
                                            <div className="flex items-center gap-2">
                                                <button onClick={(e) => handleDelete(skill, e)} className="text-gray-400 hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100" title="卸载"><Trash2 size={12} /></button>
                                                <button onClick={(e) => handleToggle(skill, e)}
                                                    className={`text-xs px-2 py-0.5 rounded transition-colors ${skill.enabled ? 'text-blue-500 hover:bg-blue-50 dark:hover:bg-blue-900/20' : 'text-[#07c160] hover:bg-green-50 dark:hover:bg-green-900/20'}`}>
                                                    {skill.enabled ? '禁用' : '启用'}
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </>
            ) : (
                /* Skills Hub tab */
                <div className="flex flex-1 min-h-0">
                    {/* Hub sidebar list */}
                    <div className="w-[260px] shrink-0 border-r border-gray-200 dark:border-gray-700 flex flex-col">
                        <div className="px-4 py-3">
                            <div className="relative">
                                <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                                <input type="text" value={hubSearch} onChange={e => setHubSearch(e.target.value)} placeholder="搜索..."
                                    className="w-full pl-8 pr-3 py-1.5 text-xs bg-white dark:bg-[#2e2e2e] rounded-lg border border-gray-200 dark:border-gray-700 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400" />
                            </div>
                        </div>
                        <div className="flex-1 overflow-y-auto">
                            {filteredHub.map(skill => (
                                <div key={skill.name} onClick={() => setHubSelected(skill)}
                                    className={`flex items-center px-4 py-3 cursor-pointer transition-colors ${hubSelected?.name === skill.name ? 'bg-[#07c160]/5 border-l-2 border-[#07c160]' : 'hover:bg-gray-50 dark:hover:bg-[#303030] border-l-2 border-transparent'}`}>
                                    <span className="text-xl mr-3 shrink-0">{skill.icon}</span>
                                    <div className="min-w-0">
                                        <p className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">{skill.name}</p>
                                        <p className="text-xs text-gray-400 truncate">{skill.description}</p>
                                    </div>
                                </div>
                            ))}
                            {/* Import from URL */}
                            <div className="px-4 py-3 mt-2 border-t border-gray-200 dark:border-gray-700">
                                <button onClick={() => setImportModalOpen(true)}
                                    className="w-full flex items-center justify-center gap-2 py-3 rounded-lg border-2 border-dashed border-gray-300 dark:border-gray-600 text-gray-400 hover:text-[#07c160] hover:border-[#07c160] transition-colors text-xs">
                                    <Download size={14} />从 URL 导入
                                </button>
                            </div>
                        </div>
                    </div>

                    {/* Hub detail */}
                    <div className="flex-1 overflow-y-auto">
                        {hubSelected ? (
                            <div className="px-10 py-8 max-w-2xl">
                                <div className="flex items-start gap-4 mb-6">
                                    <span className="text-5xl">{hubSelected.icon}</span>
                                    <div>
                                        <h2 className="text-xl font-bold text-gray-800 dark:text-white mb-1">{hubSelected.name}</h2>
                                        <p className="text-sm text-gray-500 dark:text-gray-400 mb-2">{hubSelected.description}</p>
                                        <div className="flex items-center gap-3 text-xs text-gray-400">
                                            <span className="flex items-center gap-1"><User size={11} />{hubSelected.author}</span>
                                            <span>v{hubSelected.version}</span>
                                            <span className="flex items-center gap-1"><Tag size={11} />{hubSelected.tags.join(', ')}</span>
                                        </div>
                                    </div>
                                </div>

                                <p className="text-sm text-gray-600 dark:text-gray-300 mb-6 p-4 bg-gray-50 dark:bg-[#2e2e2e] rounded-xl leading-relaxed">{hubSelected.readme}</p>

                                <div className="flex items-center gap-3">
                                    <button onClick={() => { setImportUrl(hubSelected.url); setImportModalOpen(true); }}
                                        className="flex items-center gap-2 px-5 py-2.5 bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg text-sm font-medium transition-colors">
                                        <Download size={15} />安装技能
                                    </button>
                                    {hubSelected.url && (
                                        <a href={hubSelected.url} target="_blank" rel="noopener noreferrer"
                                            className="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-[#2e2e2e] hover:bg-gray-50 dark:hover:bg-[#383838] text-gray-700 dark:text-gray-200 rounded-lg text-sm border border-gray-200 dark:border-gray-700 transition-colors">
                                            <ExternalLink size={14} />查看源码
                                        </a>
                                    )}
                                </div>

                                <div className="mt-6 text-[11px] text-gray-400 p-3 bg-blue-50 dark:bg-blue-900/10 rounded-lg">
                                    <p>支持从以下来源安装：skills.sh · clawhub.ai · skillsmp.com · github.com</p>
                                </div>
                            </div>
                        ) : (
                            <div className="flex flex-col items-center justify-center h-full text-gray-400">
                                <Globe className="w-12 h-12 mb-3 opacity-20" />
                                <p className="text-sm">从左侧选择技能查看详情</p>
                                <p className="text-xs mt-1 opacity-70">或者从 URL 导入自定义技能</p>
                                <button onClick={() => setImportModalOpen(true)}
                                    className="mt-4 flex items-center gap-2 px-4 py-2 bg-[#07c160] hover:bg-[#06ad56] text-white text-sm rounded-lg transition-colors">
                                    <Download size={14} />从 URL 导入
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            )}

            {/* Skill detail Drawer */}
            {drawerOpen && drawerSkill && (
                <>
                    <div className="fixed inset-0 z-40 bg-black/20" onClick={() => setDrawerOpen(false)} />
                    <div className="fixed right-0 top-0 bottom-0 z-50 w-[520px] bg-white dark:bg-[#1e1e1e] shadow-2xl flex flex-col" style={{ animation: 'slideInRight 0.22s ease-out' }}>
                        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-100 dark:border-gray-800">
                            <div className="flex items-center gap-3">
                                <span className="text-2xl">{drawerSkill.icon}</span>
                                <div>
                                    <h2 className="text-sm font-bold text-gray-800 dark:text-white">{drawerSkill.name}</h2>
                                    <span className="text-[10px] text-gray-400">v{drawerSkill.version} · {drawerSkill.author}</span>
                                </div>
                            </div>
                            <button onClick={() => setDrawerOpen(false)} className="p-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/10"><X size={18} className="text-gray-400" /></button>
                        </div>
                        <div className="flex-1 overflow-y-auto px-6 py-5">
                            <div className="space-y-3 mb-5">
                                <div>
                                    <label className="text-[10px] text-gray-400 uppercase tracking-wider mb-1 block">描述</label>
                                    <p className="text-sm text-gray-700 dark:text-gray-200">{drawerSkill.description || '暂无描述'}</p>
                                </div>
                                {drawerSkill.tags.length > 0 && (
                                    <div>
                                        <label className="text-[10px] text-gray-400 uppercase tracking-wider mb-1 block">标签</label>
                                        <div className="flex flex-wrap gap-1">
                                            {drawerSkill.tags.map(tag => (
                                                <span key={tag} className="flex items-center gap-1 text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400"><Tag size={10} />{tag}</span>
                                            ))}
                                        </div>
                                    </div>
                                )}
                                {drawerSkill.homepage && (
                                    <div>
                                        <label className="text-[10px] text-gray-400 uppercase tracking-wider mb-1 block">主页</label>
                                        <a href={drawerSkill.homepage} target="_blank" rel="noopener noreferrer" className="text-xs text-[#07c160] hover:underline flex items-center gap-1"><ExternalLink size={12} />{drawerSkill.homepage}</a>
                                    </div>
                                )}
                                <div>
                                    <label className="text-[10px] text-gray-400 uppercase tracking-wider mb-1 block">路径</label>
                                    <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-gray-50 dark:bg-[#2e2e2e] text-[11px] text-gray-400 font-mono"><FileText size={12} /><span className="truncate">{drawerSkill.path}</span></div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-center justify-between mb-2">
                                    <label className="text-[10px] text-gray-400 uppercase tracking-wider">内容</label>
                                    <div className="flex items-center gap-2">
                                        <button onClick={() => handleCopyBody(drawerSkill.body)} className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors" title="复制"><Copy size={14} /></button>
                                        <button onClick={() => setShowPreview(!showPreview)}
                                            className={`flex items-center gap-1 text-[10px] px-2 py-1 rounded transition-colors ${showPreview ? 'bg-[#07c160]/10 text-[#07c160]' : 'bg-gray-100 dark:bg-gray-700 text-gray-400'}`}>
                                            <Eye size={10} />预览
                                        </button>
                                    </div>
                                </div>
                                {showPreview ? (
                                    <div className="p-4 rounded-lg bg-gray-50 dark:bg-[#2e2e2e] text-sm text-gray-600 dark:text-gray-300 leading-relaxed max-h-[400px] overflow-y-auto">
                                        {drawerSkill.body.split('\n').map((line, i) => {
                                            if (line.startsWith('# ')) return <h2 key={i} className="text-base font-bold text-gray-800 dark:text-white mt-4 mb-2">{line.slice(2)}</h2>;
                                            if (line.startsWith('## ')) return <h3 key={i} className="text-sm font-semibold text-gray-700 dark:text-gray-200 mt-3 mb-1.5">{line.slice(3)}</h3>;
                                            if (line.startsWith('- ')) return <div key={i} className="flex gap-2 ml-2"><span className="text-[#07c160]">•</span><span>{line.slice(2)}</span></div>;
                                            if (line.trim() === '') return <div key={i} className="h-2" />;
                                            return <p key={i} className="mb-1">{line}</p>;
                                        })}
                                    </div>
                                ) : (
                                    <pre className="p-4 rounded-lg bg-gray-50 dark:bg-[#2e2e2e] text-xs text-gray-600 dark:text-gray-300 font-mono whitespace-pre-wrap max-h-[400px] overflow-y-auto">{drawerSkill.body}</pre>
                                )}
                            </div>
                        </div>
                        <div className="flex items-center justify-between px-6 py-4 border-t border-gray-100 dark:border-gray-800">
                            <button onClick={() => handleDelete(drawerSkill)} className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"><Trash2 size={14} />卸载</button>
                            <button onClick={() => handleToggle(drawerSkill)}
                                className={`flex items-center gap-1.5 px-4 py-2 text-xs rounded-lg font-medium transition-colors ${drawerSkill.enabled ? 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-300' : 'bg-[#07c160] text-white hover:bg-[#06ad56]'}`}>
                                {drawerSkill.enabled ? '禁用' : '启用'}
                            </button>
                        </div>
                    </div>
                </>
            )}

            {/* Import Modal */}
            {importModalOpen && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-white dark:bg-[#2e2e2e] rounded-xl shadow-xl w-[520px] p-6">
                        <div className="flex items-center justify-between mb-4">
                            <h3 className="text-sm font-bold text-gray-800 dark:text-white flex items-center gap-2"><Download className="w-4 h-4 text-[#07c160]" />导入技能</h3>
                            <button onClick={() => { if (!importing) { setImportModalOpen(false); setImportUrl(''); setImportUrlError(''); } }} className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"><X size={16} className="text-gray-400" /></button>
                        </div>
                        <div className="mb-4 p-3 rounded-lg bg-[#f7f7f7] dark:bg-[#3a3a3a] text-xs text-gray-500 dark:text-gray-400">
                            <p className="font-medium text-gray-600 dark:text-gray-300 mb-1.5">支持的 URL 来源：</p>
                            <ul className="space-y-0.5 ml-1">
                                <li>• https://skills.sh/</li><li>• https://clawhub.ai/</li><li>• https://skillsmp.com/</li><li>• https://github.com/</li>
                            </ul>
                            <p className="font-medium text-gray-600 dark:text-gray-300 mt-2.5 mb-1.5">URL 示例：</p>
                            <ul className="space-y-0.5 ml-1 text-[11px]">
                                <li>• https://skills.sh/vercel-labs/skills/find-skills</li>
                                <li>• https://github.com/anthropics/skills/tree/main/skills/skill-creator</li>
                            </ul>
                        </div>
                        <input type="text" value={importUrl} onChange={e => handleImportUrlChange(e.target.value)} placeholder="输入技能 URL..." disabled={importing}
                            className="w-full px-3 py-2.5 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-lg border border-gray-200 dark:border-gray-700 outline-none text-gray-700 dark:text-gray-200 mb-2 focus:border-[#07c160] transition-colors"
                            onKeyDown={e => e.key === 'Enter' && handleHubInstall()} />
                        {importUrlError && <p className="text-xs text-red-500 mb-2 flex items-center gap-1"><AlertCircle size={12} />{importUrlError}</p>}
                        {importing && <p className="text-xs text-gray-400 mb-2 flex items-center gap-1"><Loader2 size={12} className="animate-spin" />正在导入...</p>}
                        <div className="flex justify-end gap-2 mt-3">
                            <button onClick={() => { if (!importing) { setImportModalOpen(false); setImportUrl(''); setImportUrlError(''); } }} disabled={importing} className="px-4 py-2 text-xs text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg disabled:opacity-40">取消</button>
                            <button onClick={handleHubInstall} disabled={importing || !importUrl.trim() || !!importUrlError}
                                className="px-4 py-2 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg disabled:opacity-40 flex items-center gap-1">
                                {importing && <Loader2 size={12} className="animate-spin" />}导入技能
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Create Modal */}
            {showCreateModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
                    <div className="bg-white dark:bg-[#2e2e2e] rounded-xl shadow-xl w-[400px] p-5">
                        <h3 className="text-sm font-bold text-gray-800 dark:text-white mb-1 flex items-center gap-2"><Plus className="w-4 h-4 text-[#07c160]" />创建技能</h3>
                        <p className="text-xs text-gray-400 mb-3">在 <code className="text-[11px] bg-gray-100 dark:bg-gray-700 px-1.5 py-0.5 rounded">{skillsDir}</code> 创建模板</p>
                        <input type="text" value={newSkillName} onChange={e => setNewSkillName(e.target.value)} placeholder="技能名称 (英文, 如 my-skill)"
                            className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-lg border border-gray-200 dark:border-gray-700 outline-none text-gray-700 dark:text-gray-200 mb-3 focus:border-[#07c160] transition-colors"
                            onKeyDown={e => e.key === 'Enter' && handleCreate()} />
                        <div className="flex justify-end gap-2">
                            <button onClick={() => { setShowCreateModal(false); setNewSkillName(''); }} className="px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg">取消</button>
                            <button onClick={handleCreate} disabled={!newSkillName.trim()} className="px-3 py-1.5 text-xs bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg disabled:opacity-50">创建</button>
                        </div>
                    </div>
                </div>
            )}

            <style>{`@keyframes slideInRight { from { transform: translateX(100%); } to { transform: translateX(0); } }`}</style>
        </div>
    );
}
