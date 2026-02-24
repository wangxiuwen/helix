import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, RefreshCw, Server, Trash2, Wifi, WifiOff, X } from 'lucide-react';
import { useDevOpsStore } from '../stores/useDevOpsStore';

function AddServerDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
    const { t } = useTranslation();
    const { addServer } = useDevOpsStore();
    const [name, setName] = useState('');
    const [host, setHost] = useState('');
    const [port, setPort] = useState('');
    const [tags, setTags] = useState('');

    const handleSubmit = () => {
        if (!name.trim() || !host.trim()) return;
        addServer({
            name: name.trim(),
            host: host.trim(),
            port: port ? parseInt(port) : undefined,
            tags: tags ? tags.split(',').map((t) => t.trim()).filter(Boolean) : [],
        });
        setName(''); setHost(''); setPort(''); setTags('');
        onClose();
    };

    if (!open) return null;
    return (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="card bg-base-100 w-full max-w-md shadow-2xl">
                <div className="card-body">
                    <div className="flex items-center justify-between mb-4">
                        <h3 className="text-lg font-semibold">{t('servers.add_title', '添加服务器')}</h3>
                        <button className="btn btn-ghost btn-sm btn-circle" onClick={onClose}><X size={18} /></button>
                    </div>
                    <div className="space-y-3">
                        <div className="form-control">
                            <label className="label"><span className="label-text">名称 *</span></label>
                            <input className="input input-bordered input-sm w-full" placeholder="e.g. Web Server" value={name} onChange={(e) => setName(e.target.value)} />
                        </div>
                        <div className="form-control">
                            <label className="label"><span className="label-text">主机地址 *</span></label>
                            <input className="input input-bordered input-sm w-full" placeholder="e.g. 192.168.1.100" value={host} onChange={(e) => setHost(e.target.value)} />
                        </div>
                        <div className="form-control">
                            <label className="label"><span className="label-text">端口</span></label>
                            <input className="input input-bordered input-sm w-full" placeholder="22" type="number" value={port} onChange={(e) => setPort(e.target.value)} />
                        </div>
                        <div className="form-control">
                            <label className="label"><span className="label-text">标签 (逗号分隔)</span></label>
                            <input className="input input-bordered input-sm w-full" placeholder="production, web" value={tags} onChange={(e) => setTags(e.target.value)} />
                        </div>
                    </div>
                    <div className="modal-action">
                        <button className="btn btn-ghost btn-sm" onClick={onClose}>取消</button>
                        <button className="btn btn-primary btn-sm" onClick={handleSubmit} disabled={!name.trim() || !host.trim()}>添加</button>
                    </div>
                </div>
            </div>
        </div>
    );
}

function Servers() {
    useTranslation();
    const { servers, removeServer, checkAllServers, checkServerStatus } = useDevOpsStore();
    const [showAdd, setShowAdd] = useState(false);
    const [search, setSearch] = useState('');
    const [refreshing, setRefreshing] = useState(false);

    useEffect(() => { checkAllServers(); }, []);

    const handleRefresh = async () => { setRefreshing(true); await checkAllServers(); setRefreshing(false); };

    const filtered = servers.filter((s) =>
        !search || s.name.toLowerCase().includes(search.toLowerCase()) || s.host.toLowerCase().includes(search.toLowerCase())
    );

    return (
        <div className="p-6 space-y-6 overflow-y-auto h-full">
            <AddServerDialog open={showAdd} onClose={() => setShowAdd(false)} />
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-base-content">服务器管理</h1>
                    <p className="text-sm text-base-content/60 mt-1">管理和监控你的服务器节点</p>
                </div>
                <div className="flex gap-2">
                    <button className="btn btn-primary btn-sm gap-2" onClick={() => setShowAdd(true)}><Plus size={16} />添加</button>
                    <button className="btn btn-ghost btn-sm" onClick={handleRefresh} disabled={refreshing}><RefreshCw size={16} className={refreshing ? 'animate-spin' : ''} /></button>
                </div>
            </div>
            <input className="input input-bordered input-sm w-full max-w-xs" placeholder="搜索服务器..." value={search} onChange={(e) => setSearch(e.target.value)} />
            {filtered.length === 0 ? (
                <div className="text-center py-16 text-base-content/40">
                    <Server size={48} className="mx-auto mb-3 opacity-30" />
                    <p>{servers.length === 0 ? '暂未添加服务器' : '没有匹配结果'}</p>
                    {servers.length === 0 && <button className="btn btn-primary btn-sm mt-3" onClick={() => setShowAdd(true)}>添加第一台</button>}
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    {filtered.map((server) => (
                        <div key={server.id} className="card bg-base-100 shadow-md border border-base-200 hover:shadow-lg transition-all">
                            <div className="card-body p-5">
                                <div className="flex items-center justify-between mb-2">
                                    <div className="flex items-center gap-3">
                                        <div className={`p-2 rounded-lg ${server.status === 'online' ? 'bg-emerald-500/10' : 'bg-gray-500/10'}`}>
                                            {server.status === 'online' ? <Wifi size={18} className="text-emerald-500" /> : <WifiOff size={18} className="text-gray-400" />}
                                        </div>
                                        <div>
                                            <h3 className="font-semibold text-sm">{server.name}</h3>
                                            <span className="text-xs text-base-content/40">{server.host}{server.port ? `:${server.port}` : ''}</span>
                                        </div>
                                    </div>
                                    <div className={`w-2.5 h-2.5 rounded-full ${server.status === 'online' ? 'bg-emerald-500 animate-pulse' : 'bg-gray-400'}`} />
                                </div>
                                {server.tags && server.tags.length > 0 && (
                                    <div className="flex flex-wrap gap-1 mb-2">
                                        {server.tags.map((tag) => <span key={tag} className="text-xs px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-500">{tag}</span>)}
                                    </div>
                                )}
                                <div className="flex items-center justify-between mt-2">
                                    <span className="text-xs text-base-content/30">{server.lastCheck ? new Date(server.lastCheck).toLocaleTimeString() : ''}</span>
                                    <div className="flex gap-1">
                                        <button className="btn btn-ghost btn-xs" onClick={() => checkServerStatus(server.id)}><RefreshCw size={12} /></button>
                                        <button className="btn btn-ghost btn-xs text-red-500" onClick={() => removeServer(server.id)}><Trash2 size={12} /></button>
                                    </div>
                                </div>
                            </div>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}

export default Servers;
