import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
    Search, Plus, Edit3, Trash2, Check, UserPlus, ChevronLeft
} from 'lucide-react';
import { useDevOpsStore, VirtualContact } from '../stores/useDevOpsStore';

const AVATAR_SEEDS = [
    'Felix', 'Aneka', 'Pepper', 'Missy', 'Sassy', 'Lucky', 'Buddy', 'Charlie',
    'Max', 'Oscar', 'Milo', 'Leo', 'Luna', 'Bella', 'Lily', 'Daisy', 'Ruby',
    'Coco', 'Gracie', 'Sadie', 'Molly', 'Rosie', 'Lola', 'Lucy', 'Stella',
];

const ROLE_PRESETS = [
    { role: '项目经理', icon: '📋', color: '#3b82f6' },
    { role: '产品经理', icon: '📝', color: '#8b5cf6' },
    { role: '架构师', icon: '🏗️', color: '#06b6d4' },
    { role: '开发工程师', icon: '💻', color: '#10b981' },
    { role: '测试工程师', icon: '🧪', color: '#f59e0b' },
    { role: '教研专家', icon: '📚', color: '#0d9488' },
    { role: '设计师', icon: '🎨', color: '#ec4899' },
    { role: '运维工程师', icon: '🔧', color: '#6366f1' },
    { role: '数据分析师', icon: '📊', color: '#14b8a6' },
    { role: '安全专家', icon: '🛡️', color: '#ef4444' },
];

function Contacts() {
    const { t } = useTranslation();
    const { contacts, addContact, updateContact, removeContact } = useDevOpsStore();
    const [searchQuery, setSearchQuery] = useState('');
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const [showAddForm, setShowAddForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);

    // Form state
    const [form, setForm] = useState({
        name: '', icon: '🤖', avatar: '', color: '#3b82f6', role: '',
        description: '', systemPrompt: '',
    });

    const selected = contacts.find(c => c.id === selectedId) || null;

    const filtered = contacts.filter(c =>
        !searchQuery || c.name.includes(searchQuery) || c.role.includes(searchQuery)
    );

    // Group contacts by role
    const grouped = filtered.reduce<Record<string, VirtualContact[]>>((acc, c) => {
        const key = c.role || '其他';
        if (!acc[key]) acc[key] = [];
        acc[key].push(c);
        return acc;
    }, {});

    const openAddForm = () => {
        const seed = AVATAR_SEEDS[Math.floor(Math.random() * AVATAR_SEEDS.length)];
        setForm({
            name: '', icon: '🤖',
            avatar: `https://api.dicebear.com/9.x/micah/svg?seed=${seed}`,
            color: '#3b82f6', role: '', description: '', systemPrompt: '',
        });
        setShowAddForm(true);
        setEditingId(null);
    };

    const openEditForm = (c: VirtualContact) => {
        setForm({
            name: c.name, icon: c.icon, avatar: c.avatar, color: c.color,
            role: c.role, description: c.description || '', systemPrompt: c.systemPrompt,
        });
        setEditingId(c.id);
        setShowAddForm(true);
    };

    const handleSave = () => {
        if (!form.name.trim() || !form.role.trim()) return;
        if (editingId) {
            updateContact(editingId, { ...form });
        } else {
            const id = addContact({
                name: form.name, icon: form.icon, avatar: form.avatar,
                color: form.color, role: form.role, description: form.description,
                systemPrompt: form.systemPrompt,
            });
            setSelectedId(id);
        }
        setShowAddForm(false);
        setEditingId(null);
    };

    const handleDelete = (id: string) => {
        if (selectedId === id) setSelectedId(null);
        removeContact(id);
    };

    // Auto-select first contact on mount
    useEffect(() => {
        if (!selectedId && contacts.length > 0) setSelectedId(contacts[0].id);
    }, [contacts.length]);

    // Detail / form right panel content
    const renderRightPanel = () => {
        if (showAddForm) {
            return (
                <div className="flex-1 flex flex-col bg-white dark:bg-[#2a2a2a] overflow-y-auto">
                    {/* Header */}
                    <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-700/50 flex items-center gap-3 shrink-0">
                        <button onClick={() => { setShowAddForm(false); setEditingId(null); }} className="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
                            <ChevronLeft size={16} className="text-gray-400" />
                        </button>
                        <h3 className="text-[14px] font-semibold text-gray-800 dark:text-gray-200">
                            {editingId ? '编辑联系人' : '添加新成员'}
                        </h3>
                    </div>
                    {/* Form body */}
                    <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">
                        {/* Avatar + name row */}
                        <div className="flex items-start gap-5">
                            <div
                                className="w-20 h-20 rounded-2xl overflow-hidden shadow-md shrink-0 flex items-center justify-center"
                                style={{ background: `linear-gradient(135deg, ${form.color}33, ${form.color}66)`, border: `2px solid ${form.color}44` }}
                            >
                                {form.avatar ? (
                                    <img src={form.avatar} alt="" className="w-full h-full object-cover" />
                                ) : (
                                    <span className="text-3xl">{form.icon}</span>
                                )}
                            </div>
                            <div className="flex-1 space-y-3 pt-1">
                                <div>
                                    <label className="text-[11px] text-gray-400 font-medium mb-1 block">姓名 *</label>
                                    <input
                                        className="w-full bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors"
                                        placeholder="输入姓名"
                                        value={form.name}
                                        onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                                    />
                                </div>
                                <div>
                                    <label className="text-[11px] text-gray-400 font-medium mb-1 block">简介</label>
                                    <input
                                        className="w-full bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors"
                                        placeholder="一句话描述"
                                        value={form.description}
                                        onChange={e => setForm(f => ({ ...f, description: e.target.value }))}
                                    />
                                </div>
                            </div>
                        </div>

                        {/* Role presets */}
                        <div>
                            <label className="text-[11px] text-gray-400 font-medium mb-2 block">角色 *</label>
                            <div className="flex flex-wrap gap-1.5">
                                {ROLE_PRESETS.map(p => (
                                    <button
                                        key={p.role}
                                        className={`px-3 py-1.5 rounded-full text-[11px] transition-all border ${form.role === p.role
                                            ? 'bg-[#07c160]/10 text-[#07c160] border-[#07c160]/30 font-medium shadow-sm'
                                            : 'bg-gray-50 dark:bg-gray-800 text-gray-500 dark:text-gray-400 border-gray-200 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700'
                                            }`}
                                        onClick={() => setForm(f => ({ ...f, role: p.role, icon: p.icon, color: p.color }))}
                                    >
                                        {p.icon} {p.role}
                                    </button>
                                ))}
                            </div>
                            <input
                                className="w-full mt-2.5 bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors"
                                placeholder="或自定义角色名称"
                                value={form.role}
                                onChange={e => setForm(f => ({ ...f, role: e.target.value }))}
                            />
                        </div>

                        {/* Color picker */}
                        <div>
                            <label className="text-[11px] text-gray-400 font-medium mb-2 block">主题色</label>
                            <div className="flex gap-2.5">
                                {['#3b82f6', '#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899', '#6366f1', '#0d9488', '#f97316'].map(c => (
                                    <button
                                        key={c}
                                        className={`w-7 h-7 rounded-full transition-all ${form.color === c ? 'scale-125 ring-2 ring-offset-2 ring-gray-300 dark:ring-gray-600 dark:ring-offset-[#2a2a2a]' : 'hover:scale-110'}`}
                                        style={{ backgroundColor: c }}
                                        onClick={() => setForm(f => ({ ...f, color: c }))}
                                    />
                                ))}
                            </div>
                        </div>

                        {/* System Prompt */}
                        <div>
                            <label className="text-[11px] text-gray-400 font-medium mb-1 block">系统提示词 (可选)</label>
                            <textarea
                                className="w-full bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors min-h-[100px] resize-none"
                                placeholder="自定义 AI 行为... (留空使用默认)"
                                value={form.systemPrompt}
                                onChange={e => setForm(f => ({ ...f, systemPrompt: e.target.value }))}
                            />
                        </div>
                    </div>
                    {/* Footer */}
                    <div className="px-6 py-4 border-t border-gray-100 dark:border-gray-700/50 flex justify-end gap-2 shrink-0">
                        <button
                            onClick={() => { setShowAddForm(false); setEditingId(null); }}
                            className="px-5 py-2 text-[12px] text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                        >
                            取消
                        </button>
                        <button
                            onClick={handleSave}
                            disabled={!form.name.trim() || !form.role.trim()}
                            className="px-5 py-2 text-[12px] bg-[#07c160] hover:bg-[#06ad56] disabled:opacity-40 text-white rounded-lg transition-colors flex items-center gap-1.5"
                        >
                            <Check size={13} /> {editingId ? '保存' : '添加'}
                        </button>
                    </div>
                </div>
            );
        }

        if (selected) {
            return (
                <div className="flex-1 flex flex-col bg-white dark:bg-[#2a2a2a]">
                    {/* Info section */}
                    <div className="flex-1 overflow-y-auto">
                        {/* Top: avatar + name area */}
                        <div className="px-8 pt-8 pb-6 flex items-start gap-5">
                            <div
                                className="w-16 h-16 rounded-2xl overflow-hidden shadow-md shrink-0 flex items-center justify-center"
                                style={{ background: `linear-gradient(135deg, ${selected.color}33, ${selected.color}66)`, border: `2px solid ${selected.color}44` }}
                            >
                                {selected.avatar ? (
                                    <img src={selected.avatar} alt="" className="w-full h-full object-cover" />
                                ) : (
                                    <span className="text-2xl">{selected.icon}</span>
                                )}
                            </div>
                            <div className="flex-1 pt-1">
                                <h2 className="text-[18px] font-semibold text-gray-800 dark:text-gray-200 leading-tight">{selected.name}</h2>
                                <div className="flex items-center gap-1.5 mt-1">
                                    <span className="text-[13px]">{selected.icon}</span>
                                    <span className="text-[13px] text-gray-500 dark:text-gray-400">{selected.role}</span>
                                </div>
                                {selected.description && (
                                    <p className="text-[12px] text-gray-400 mt-2 leading-relaxed">{selected.description}</p>
                                )}
                            </div>
                        </div>

                        {/* Info rows */}
                        <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50">
                            <div className="py-4 flex items-center gap-3">
                                <span className="text-[12px] text-gray-400 w-16 shrink-0">角色</span>
                                <span className="text-[13px] text-gray-700 dark:text-gray-300 flex items-center gap-1.5">
                                    <span
                                        className="inline-block w-2.5 h-2.5 rounded-full"
                                        style={{ backgroundColor: selected.color }}
                                    />
                                    {selected.role}
                                </span>
                            </div>
                        </div>
                        {selected.systemPrompt && (
                            <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50">
                                <div className="py-4">
                                    <span className="text-[12px] text-gray-400 block mb-2">系统提示词</span>
                                    <div className="text-[12px] text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-800/50 rounded-lg p-3 whitespace-pre-wrap leading-relaxed max-h-[200px] overflow-y-auto">
                                        {selected.systemPrompt}
                                    </div>
                                </div>
                            </div>
                        )}

                        {/* Actions */}
                        <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50 py-4 flex gap-2">
                            <button
                                onClick={() => openEditForm(selected)}
                                className="flex-1 py-2.5 text-[12px] bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-xl hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors flex items-center justify-center gap-1.5"
                            >
                                <Edit3 size={13} /> 编辑资料
                            </button>
                            <button
                                onClick={() => handleDelete(selected.id)}
                                className="px-5 py-2.5 text-[12px] text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-colors flex items-center gap-1.5"
                            >
                                <Trash2 size={13} /> 删除
                            </button>
                        </div>
                    </div>
                </div>
            );
        }

        return (
            <div className="flex-1 flex items-center justify-center bg-white dark:bg-[#2a2a2a]">
                <div className="text-center text-gray-400">
                    <UserPlus size={36} className="mx-auto mb-3 opacity-20" />
                    <p className="text-[13px]">{t('contacts.select_hint', '选择一个联系人查看详情')}</p>
                    <button
                        onClick={openAddForm}
                        className="mt-4 px-5 py-2 text-[12px] bg-[#07c160] hover:bg-[#06ad56] text-white rounded-full transition-colors"
                    >
                        + 添加虚拟角色
                    </button>
                </div>
            </div>
        );
    };

    return (
        <div className="flex flex-1 w-full h-full bg-[#f0f0f0] dark:bg-[#1e1e1e]">
            {/* Left: Contact List */}
            <div className="w-[240px] shrink-0 bg-[#e8e8e8] dark:bg-[#252525] border-r border-black/[0.06] dark:border-white/[0.06] flex flex-col">
                {/* Search + Add */}
                <div className="px-3 pt-3 pb-2 flex items-center gap-2" style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}>
                    <div className="flex-1 relative" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
                        <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                        <input
                            className="w-full bg-white/60 dark:bg-white/5 rounded-md pl-8 pr-3 py-1.5 text-[12px] text-gray-700 dark:text-gray-300 placeholder:text-gray-400 outline-none border border-transparent focus:border-[#07c160]/30 transition-colors"
                            placeholder={t('contacts.search', '搜索联系人...')}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                        />
                    </div>
                    <button
                        onClick={openAddForm}
                        className="w-7 h-7 flex items-center justify-center rounded-md bg-[#07c160] hover:bg-[#06ad56] text-white transition-colors shrink-0"
                        title={t('contacts.add', '添加联系人')}
                        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                    >
                        <Plus size={14} />
                    </button>
                </div>

                {/* Contact list */}
                <div className="flex-1 overflow-y-auto">
                    {Object.entries(grouped).map(([role, members]) => (
                        <div key={role}>
                            <div className="px-4 py-1.5 text-[10px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider sticky top-0 bg-[#e8e8e8] dark:bg-[#252525]">
                                {role} ({members.length})
                            </div>
                            {members.map(c => (
                                <div
                                    key={c.id}
                                    className={`flex items-center gap-2.5 px-4 py-2.5 cursor-pointer transition-colors ${selectedId === c.id
                                        ? 'bg-black/[0.08] dark:bg-white/[0.08]'
                                        : 'hover:bg-black/[0.04] dark:hover:bg-white/[0.04]'
                                        }`}
                                    onClick={() => { setSelectedId(c.id); setShowAddForm(false); }}
                                >
                                    <div
                                        className="w-9 h-9 rounded-lg overflow-hidden shrink-0 flex items-center justify-center"
                                        style={{ background: `linear-gradient(135deg, ${c.color}22, ${c.color}44)`, border: `1.5px solid ${c.color}33` }}
                                    >
                                        {c.avatar ? (
                                            <img src={c.avatar} alt="" className="w-full h-full object-cover" />
                                        ) : (
                                            <span className="text-base">{c.icon}</span>
                                        )}
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="text-[13px] font-medium text-gray-800 dark:text-gray-200 truncate">{c.name}</div>
                                        <div className="text-[11px] text-gray-400 truncate">{c.role}</div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    ))}
                    {filtered.length === 0 && (
                        <div className="text-center py-12 text-gray-400">
                            <UserPlus size={28} className="mx-auto mb-2 opacity-20" />
                            <p className="text-[11px]">
                                {searchQuery ? t('contacts.no_results', '未找到联系人') : t('contacts.empty', '暂无联系人')}
                            </p>
                        </div>
                    )}
                </div>
            </div>

            {/* Right: Detail / Form */}
            {renderRightPanel()}
        </div>
    );
}

export default Contacts;
