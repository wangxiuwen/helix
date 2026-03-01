import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
    Plug,
    Plus,
    Trash2,
    RefreshCw,
    Search,
    ToggleLeft,
    ToggleRight,
} from 'lucide-react';

interface MCPClient {
    name: string;
    transport: string;
    command?: string;
    args?: string[];
    url?: string;
    env: Record<string, string>;
    enabled: boolean;
}

function MCP() {
    const [clients, setClients] = useState<MCPClient[]>([]);
    const [showCreate, setShowCreate] = useState(false);
    const [search, setSearch] = useState('');
    const [newClient, setNewClient] = useState<MCPClient>({
        name: '', transport: 'stdio', command: '', args: [], url: '', env: {}, enabled: true
    });

    const loadClients = useCallback(async () => {
        try {
            setClients(await invoke<MCPClient[]>('mcp_list'));
        } catch (e) { console.error('mcp_list', e); }
    }, []);

    useEffect(() => { loadClients(); }, [loadClients]);

    const handleCreate = async () => {
        try {
            await invoke<MCPClient>('mcp_create', { client: newClient });
            setShowCreate(false);
            setNewClient({ name: '', transport: 'stdio', command: '', args: [], url: '', env: {}, enabled: true });
            loadClients();
        } catch (e) { alert(String(e)); }
    };

    const handleToggle = async (name: string) => {
        try { await invoke<MCPClient>('mcp_toggle', { name }); loadClients(); } catch (e) { console.error(e); }
    };

    const handleDelete = async (name: string) => {
        try { await invoke('mcp_delete', { name }); loadClients(); } catch (e) { console.error(e); }
    };

    const filtered = clients.filter(c =>
        !search || c.name.toLowerCase().includes(search.toLowerCase())
    );

    return (
        <div className="flex-1 flex flex-col h-full bg-[#FAFBFC] dark:bg-base-300">
            {/* Header */}
            <div className="shrink-0 px-6 pt-6 pb-4">
                <div className="flex items-center justify-between mb-4">
                    <div>
                        <h1 className="text-lg font-bold text-gray-800 dark:text-white flex items-center gap-2">
                            <Plug size={20} className="text-[#07c160]" />
                            MCP 客户端
                        </h1>
                        <p className="text-xs text-gray-400 mt-1">管理 Model Context Protocol 连接，扩展 Agent 能力</p>
                    </div>
                    <div className="flex items-center gap-2">
                        <button className="p-2 rounded-lg hover:bg-black/5 dark:hover:bg-white/5 text-gray-400 transition-colors" onClick={loadClients} title="刷新">
                            <RefreshCw size={16} />
                        </button>
                        <button
                            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#07c160] hover:bg-[#06ad56] text-white text-sm rounded-lg transition-colors"
                            onClick={() => setShowCreate(!showCreate)}
                        >
                            <Plus size={14} />{showCreate ? '取消' : '添加'}
                        </button>
                    </div>
                </div>

                {/* Search */}
                <div className="relative">
                    <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                    <input
                        className="w-full pl-9 pr-3 py-2 text-sm bg-white dark:bg-[#2e2e2e] rounded-lg border-0 outline-none text-gray-700 dark:text-gray-200 placeholder:text-gray-400"
                        placeholder="搜索 MCP 客户端..."
                        value={search}
                        onChange={e => setSearch(e.target.value)}
                    />
                </div>
            </div>

            {/* Create Form */}
            {showCreate && (
                <div className="mx-6 mb-4 p-4 bg-white dark:bg-[#2e2e2e] rounded-xl space-y-3">
                    <input
                        className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-lg border-0 outline-none"
                        placeholder="名称 (如 tavily_mcp)"
                        value={newClient.name}
                        onChange={e => setNewClient({ ...newClient, name: e.target.value })}
                    />
                    <select
                        className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-lg border-0 outline-none text-gray-700 dark:text-gray-200"
                        value={newClient.transport}
                        onChange={e => setNewClient({ ...newClient, transport: e.target.value })}
                    >
                        <option value="stdio">stdio (本地命令)</option>
                        <option value="sse">SSE (远程服务)</option>
                    </select>
                    {newClient.transport === 'stdio' ? (
                        <input
                            className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-lg border-0 outline-none"
                            placeholder="命令 (如 npx -y @tavily/mcp)"
                            value={newClient.command || ''}
                            onChange={e => setNewClient({ ...newClient, command: e.target.value })}
                        />
                    ) : (
                        <input
                            className="w-full px-3 py-2 text-sm bg-[#f7f7f7] dark:bg-[#3a3a3a] rounded-lg border-0 outline-none"
                            placeholder="URL (如 http://localhost:3001/sse)"
                            value={newClient.url || ''}
                            onChange={e => setNewClient({ ...newClient, url: e.target.value })}
                        />
                    )}
                    <button
                        className="px-4 py-2 text-sm bg-[#07c160] hover:bg-[#06ad56] text-white rounded-lg disabled:opacity-40"
                        onClick={handleCreate}
                        disabled={!newClient.name || (newClient.transport === 'stdio' ? !newClient.command : !newClient.url)}
                    >
                        创建
                    </button>
                </div>
            )}

            {/* Client List */}
            <div className="flex-1 overflow-y-auto px-6 pb-6">
                {filtered.length > 0 ? (
                    <div className="space-y-3">
                        {filtered.map(client => (
                            <div key={client.name} className="p-4 rounded-xl bg-white dark:bg-[#2e2e2e] transition-colors hover:shadow-sm">
                                <div className="flex items-center justify-between mb-2">
                                    <div className="flex items-center gap-3">
                                        <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${client.enabled ? 'bg-[#07c160]/10' : 'bg-gray-100 dark:bg-gray-700'}`}>
                                            <Plug size={16} className={client.enabled ? 'text-[#07c160]' : 'text-gray-400'} />
                                        </div>
                                        <div>
                                            <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{client.name}</span>
                                            <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded bg-blue-50 dark:bg-blue-900/30 text-blue-500">{client.transport}</span>
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        <button
                                            onClick={() => handleToggle(client.name)}
                                            className={`p-1 rounded transition-colors ${client.enabled ? 'text-[#07c160]' : 'text-gray-400 hover:text-gray-600'}`}
                                            title={client.enabled ? '禁用' : '启用'}
                                        >
                                            {client.enabled ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
                                        </button>
                                        <button
                                            onClick={() => handleDelete(client.name)}
                                            className="p-1 rounded text-gray-400 hover:text-red-500 transition-colors"
                                            title="删除"
                                        >
                                            <Trash2 size={14} />
                                        </button>
                                    </div>
                                </div>
                                <p className="text-xs text-gray-400 break-all ml-11">
                                    {client.transport === 'stdio' ? client.command : client.url}
                                </p>
                            </div>
                        ))}
                    </div>
                ) : (
                    <div className="flex flex-col items-center justify-center h-64 text-gray-400">
                        <Plug size={40} className="mb-3 opacity-30" />
                        <p className="text-sm">{search ? '没有匹配的 MCP 客户端' : '暂无 MCP 客户端'}</p>
                        <p className="text-xs mt-1">点击"添加"连接 MCP 服务</p>
                    </div>
                )}
            </div>
        </div>
    );
}

export default MCP;
