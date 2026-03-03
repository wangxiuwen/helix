import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Brain, Plus, Search, Trash2, Save, Clock } from 'lucide-react';

interface KnowledgeItem {
    id: string;
    topic: string;
    summary: string;
    source: string;
    created_at: string;
    updated_at: string;
    artifacts: string[];
}

export default function KnowledgeItems() {
    const [items, setItems] = useState<KnowledgeItem[]>([]);
    const [selectedItem, setSelectedItem] = useState<KnowledgeItem | null>(null);
    const [searchQuery, setSearchQuery] = useState('');
    const [loading, setLoading] = useState(false);

    // Editor state
    const [editorTopic, setEditorTopic] = useState('');
    const [editorSummary, setEditorSummary] = useState('');
    const [editorContent, setEditorContent] = useState('');
    const [isSaving, setIsSaving] = useState(false);
    const [isCreatingNew, setIsCreatingNew] = useState(false);

    const loadItems = useCallback(async () => {
        setLoading(true);
        try {
            const list: KnowledgeItem[] = await invoke('brain_list_knowledge_items');
            list.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
            setItems(list);
        } catch (e) {
            console.error("Failed to load knowledge items", e);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        loadItems();
    }, [loadItems]);

    const handleSelectItem = async (item: KnowledgeItem) => {
        setSelectedItem(item);
        setEditorTopic(item.topic);
        setEditorSummary(item.summary);
        setIsCreatingNew(false);
        try {
            const content: string = await invoke('brain_get_knowledge_item', { id: item.id });
            setEditorContent(content);
        } catch (e) {
            console.error("Failed to load KI content", e);
            setEditorContent("Error loading content.");
        }
    };

    const handleCreateNew = () => {
        setSelectedItem(null);
        setEditorTopic('');
        setEditorSummary('');
        setEditorContent('# New Knowledge Item\n\nWrite your notes here...');
        setIsCreatingNew(true);
    };

    const handleSave = async () => {
        if (!editorTopic.trim()) return;
        setIsSaving(true);
        try {
            if (isCreatingNew) {
                await invoke('brain_knowledge_item_crud_action', {
                    action: 'create',
                    topic: editorTopic,
                    summary: editorSummary,
                    content: editorContent
                });
                setIsCreatingNew(false);
            } else if (selectedItem) {
                await invoke('brain_knowledge_item_crud_action', {
                    action: 'update',
                    id: selectedItem.id,
                    topic: editorTopic,
                    summary: editorSummary,
                    content: editorContent
                });
            }
            await loadItems();

            // Re-select the updated item if it wasn't a new creation
            // If it was new, we might need to find its ID by topic/timestamp, but for now we can just leave it selected or reload
            if (!isCreatingNew && selectedItem) {
                const updatedList = await invoke<KnowledgeItem[]>('brain_list_knowledge_items');
                const updatedItem = updatedList.find(i => i.id === selectedItem.id);
                if (updatedItem) setSelectedItem(updatedItem);
            }

        } catch (e) {
            console.error("Failed to save KI", e);
        } finally {
            setIsSaving(false);
        }
    };

    const handleDelete = async (id: string, e: React.MouseEvent) => {
        e.stopPropagation();
        if (!confirm('Are you sure you want to delete this knowledge item?')) return;
        try {
            await invoke('brain_delete_knowledge_item', { id });
            if (selectedItem?.id === id) {
                setSelectedItem(null);
                setEditorTopic('');
                setEditorSummary('');
                setEditorContent('');
            }
            await loadItems();
        } catch (err) {
            console.error("Failed to delete KI", err);
        }
    };

    const filteredItems = items.filter(i =>
        i.topic.toLowerCase().includes(searchQuery.toLowerCase()) ||
        i.summary.toLowerCase().includes(searchQuery.toLowerCase())
    );

    const formatDate = (dateStr: string) => {
        const d = new Date(dateStr);
        return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    };

    return (
        <div className="flex-1 flex overflow-hidden bg-[#fafafa] dark:bg-[#1e1e1e]">
            {/* Left sidebar: List of Knowledge Items */}
            <div className="w-[300px] flex flex-col border-r border-black/[0.06] dark:border-white/[0.06] bg-[#f5f5f5] dark:bg-[#252525]">
                {/* Header & Actions */}
                <div className="p-4 border-b border-black/[0.06] dark:border-white/[0.06]">
                    <div className="flex items-center justify-between mb-4">
                        <div className="flex items-center gap-2">
                            <Brain className="text-purple-600 dark:text-purple-400" size={20} />
                            <h2 className="font-semibold text-gray-800 dark:text-gray-200">Knowledge Base</h2>
                        </div>
                        <button
                            onClick={handleCreateNew}
                            className="p-1.5 rounded-md text-gray-600 hover:text-purple-600 hover:bg-purple-100 dark:text-gray-400 dark:hover:text-purple-400 dark:hover:bg-purple-500/20 transition-colors"
                            title="New Knowledge Item"
                        >
                            <Plus size={18} />
                        </button>
                    </div>
                    {/* Search */}
                    <div className="relative">
                        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                        <input
                            type="text"
                            placeholder="Search knowledge..."
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                            className="w-full pl-9 pr-3 py-1.5 bg-white dark:bg-[#1e1e1e] border border-black/10 dark:border-white/10 rounded-lg text-[13px] outline-none focus:border-purple-500 transition-colors"
                        />
                    </div>
                </div>

                {/* List */}
                <div className="flex-1 overflow-y-auto p-2 space-y-1">
                    {loading && items.length === 0 ? (
                        <div className="text-[13px] text-gray-400 text-center py-8">Loading...</div>
                    ) : filteredItems.length === 0 ? (
                        <div className="text-[13px] text-gray-400 text-center py-8">
                            {searchQuery ? 'No matching items' : 'No knowledge items yet'}
                        </div>
                    ) : (
                        filteredItems.map(item => (
                            <div
                                key={item.id}
                                onClick={() => handleSelectItem(item)}
                                className={`group p-3 rounded-lg cursor-pointer transition-colors ${selectedItem?.id === item.id
                                    ? 'bg-purple-100 dark:bg-purple-500/20 border border-purple-200 dark:border-purple-500/30'
                                    : 'hover:bg-black/5 dark:hover:bg-white/5 border border-transparent'
                                    }`}
                            >
                                <div className="flex justify-between items-start mb-1">
                                    <h3 className="text-[14px] font-medium text-gray-800 dark:text-gray-200 truncate pr-2">
                                        {item.topic}
                                    </h3>
                                    <button
                                        onClick={(e) => handleDelete(item.id, e)}
                                        className="opacity-0 group-hover:opacity-100 text-gray-400 hover:text-red-500 transition-opacity p-1 -mr-1 -mt-1"
                                    >
                                        <Trash2 size={14} />
                                    </button>
                                </div>
                                <p className="text-[12px] text-gray-500 dark:text-gray-400 line-clamp-2 mb-2 leading-relaxed">
                                    {item.summary || 'No summary'}
                                </p>
                                <div className="flex items-center justify-between text-[11px] text-gray-400">
                                    <div className="flex items-center gap-1">
                                        <Clock size={10} />
                                        <span>{formatDate(item.updated_at)}</span>
                                    </div>
                                    {item.source && (
                                        <span className="px-1.5 py-0.5 rounded bg-black/5 dark:bg-white/5 truncate max-w-[80px]">
                                            {item.source}
                                        </span>
                                    )}
                                </div>
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* Right pane: Editor */}
            <div className="flex-1 flex flex-col bg-white dark:bg-[#1e1e1e]">
                {(selectedItem || isCreatingNew) ? (
                    <>
                        {/* Editor Header */}
                        <div className="px-6 py-4 border-b border-black/[0.06] dark:border-white/[0.06] flex items-start justify-between gap-4">
                            <div className="flex-1 space-y-3">
                                <input
                                    type="text"
                                    value={editorTopic}
                                    onChange={(e) => setEditorTopic(e.target.value)}
                                    placeholder="Knowledge Topic"
                                    className="w-full text-lg font-semibold bg-transparent border-none outline-none text-gray-800 dark:text-gray-200"
                                />
                                <input
                                    type="text"
                                    value={editorSummary}
                                    onChange={(e) => setEditorSummary(e.target.value)}
                                    placeholder="Brief summary..."
                                    className="w-full text-[13px] bg-transparent border border-black/10 dark:border-white/10 rounded-md px-3 py-1.5 outline-none text-gray-600 dark:text-gray-300 focus:border-purple-500 transition-colors"
                                />
                            </div>
                            <button
                                onClick={handleSave}
                                disabled={isSaving || !editorTopic.trim()}
                                className="shrink-0 flex items-center gap-1.5 px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded-lg text-[13px] font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                <Save size={16} />
                                {isSaving ? 'Saving...' : 'Save'}
                            </button>
                        </div>

                        {/* Markdown Editor Area */}
                        <div className="flex-1 flex flex-col p-6 overflow-hidden">
                            <textarea
                                value={editorContent}
                                onChange={(e) => setEditorContent(e.target.value)}
                                placeholder="Markdown content..."
                                className="flex-1 w-full resize-none bg-transparent border-none outline-none text-[14px] leading-relaxed text-gray-800 dark:text-gray-200 font-mono"
                            />
                        </div>
                    </>
                ) : (
                    <div className="flex-1 flex flex-col items-center justify-center text-gray-400 gap-3">
                        <Brain size={48} className="opacity-20" />
                        <p>Select a knowledge item or create a new one</p>
                    </div>
                )}
            </div>
        </div>
    );
}
