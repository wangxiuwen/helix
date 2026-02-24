import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
    Activity,
    Bot,
    Cloud,
    Cpu,
    HardDrive,
    MonitorCheck,
    Network,
    RefreshCw,
    Server,
} from 'lucide-react';
import { useDevOpsStore } from '../stores/useDevOpsStore';

// K8s 类型
interface KubeCluster {
    name: string;
    server: string;
}
interface KubeContext {
    name: string;
    cluster: string;
    user: string;
    namespace: string | null;
}
interface KubeInfo {
    clusters: KubeCluster[];
    contexts: KubeContext[];
    current_context: string | null;
    config_path: string;
    config_exists: boolean;
}

// Aliyun 类型
interface AliyunProfile {
    name: string;
    mode: string;
    access_key_hint: string;
    region_id: string;
}
interface AliyunInfo {
    profiles: AliyunProfile[];
    current: string | null;
    config_path: string;
    config_exists: boolean;
}

const REGION_LABELS: Record<string, string> = {
    'cn-beijing': '华北2 北京',
    'cn-shanghai': '华东2 上海',
    'cn-hangzhou': '华东1 杭州',
    'cn-shenzhen': '华南1 深圳',
    'cn-guangzhou': '华南3 广州',
    'cn-hongkong': '香港',
    'ap-southeast-1': '新加坡',
    'us-west-1': '美国 硅谷',
};

function Dashboard() {
    const { t } = useTranslation();
    const {
        servers,
        aiProviders,
        logs,
        checkAllServers,
    } = useDevOpsStore();

    const [refreshing, setRefreshing] = useState(false);
    const [kubeInfo, setKubeInfo] = useState<KubeInfo | null>(null);
    const [aliyunInfo, setAliyunInfo] = useState<AliyunInfo | null>(null);

    const loadCloudInfo = async () => {
        try {
            const kube = await invoke<KubeInfo>('get_kube_info', { customPath: null });
            setKubeInfo(kube);
        } catch (e) {
            console.warn('Failed to load kube info:', e);
        }
        try {
            const aliyun = await invoke<AliyunInfo>('get_aliyun_info');
            setAliyunInfo(aliyun);
        } catch (e) {
            console.warn('Failed to load aliyun info:', e);
        }
    };

    useEffect(() => {
        checkAllServers();
        loadCloudInfo();
    }, []);

    const handleRefresh = async () => {
        setRefreshing(true);
        await Promise.all([checkAllServers(), loadCloudInfo()]);
        setRefreshing(false);
    };

    const onlineServers = servers.filter((s) => s.status === 'online');
    const warningServers = servers.filter((s) => s.status === 'warning');
    const activeProviders = aiProviders.filter((p) => p.enabled);
    // Reserved for future dashboard widgets
    // const activeTasks = tasks.filter((t) => t.status === 'active');
    // const activeAlerts = alerts.filter((a) => a.enabled);
    // const errorLogs = logs.filter((l) => l.level === 'error');

    return (
        <div className="p-6 space-y-6 overflow-y-auto h-full">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-base-content">
                        {t('dashboard.title', '控制面板')}
                    </h1>
                    <p className="text-sm text-base-content/60 mt-1">
                        {t('dashboard.subtitle', 'Helix 智能助手总览')}
                    </p>
                </div>
                <button
                    className={`btn btn-primary btn-sm gap-2`}
                    onClick={handleRefresh}
                    disabled={refreshing}
                >
                    <RefreshCw size={16} className={refreshing ? 'animate-spin' : ''} />
                    {t('dashboard.refresh', '刷新')}
                </button>
            </div>

            {/* Stats Grid */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                {/* Servers */}
                <div className="card bg-base-100 shadow-md border border-base-200 hover:shadow-lg transition-shadow">
                    <div className="card-body p-5">
                        <div className="flex items-center justify-between">
                            <div className="p-2.5 rounded-xl bg-blue-500/10">
                                <Server size={22} className="text-blue-500" />
                            </div>
                            <span className="text-2xl font-bold text-base-content">
                                {onlineServers.length}
                                <span className="text-sm font-normal text-base-content/40">
                                    /{servers.length}
                                </span>
                            </span>
                        </div>
                        <p className="text-sm text-base-content/60 mt-2">
                            {t('dashboard.servers_online', '服务器在线')}
                        </p>
                        {warningServers.length > 0 && (
                            <p className="text-xs text-amber-500 mt-1">
                                ⚠️ {warningServers.length} {t('dashboard.warnings', '告警')}
                            </p>
                        )}
                    </div>
                </div>

                {/* K8s Clusters */}
                <div className="card bg-base-100 shadow-md border border-base-200 hover:shadow-lg transition-shadow">
                    <div className="card-body p-5">
                        <div className="flex items-center justify-between">
                            <div className="p-2.5 rounded-xl bg-cyan-500/10">
                                <Network size={22} className="text-cyan-500" />
                            </div>
                            <span className="text-2xl font-bold text-base-content">
                                {kubeInfo?.clusters.length ?? '–'}
                            </span>
                        </div>
                        <p className="text-sm text-base-content/60 mt-2">
                            {t('dashboard.k8s_clusters', 'K8s 集群')}
                        </p>
                        {kubeInfo?.current_context && (
                            <p className="text-xs text-cyan-500 mt-1">
                                ▸ {kubeInfo.current_context}
                            </p>
                        )}
                    </div>
                </div>

                {/* Aliyun Profiles */}
                <div className="card bg-base-100 shadow-md border border-base-200 hover:shadow-lg transition-shadow">
                    <div className="card-body p-5">
                        <div className="flex items-center justify-between">
                            <div className="p-2.5 rounded-xl bg-orange-500/10">
                                <Cloud size={22} className="text-orange-500" />
                            </div>
                            <span className="text-2xl font-bold text-base-content">
                                {aliyunInfo?.profiles.length ?? '–'}
                            </span>
                        </div>
                        <p className="text-sm text-base-content/60 mt-2">
                            {t('dashboard.aliyun_profiles', '阿里云配置')}
                        </p>
                        {aliyunInfo?.current && (
                            <p className="text-xs text-orange-500 mt-1">
                                ▸ {aliyunInfo.current}
                            </p>
                        )}
                    </div>
                </div>

                {/* AI Providers */}
                <div className="card bg-base-100 shadow-md border border-base-200 hover:shadow-lg transition-shadow">
                    <div className="card-body p-5">
                        <div className="flex items-center justify-between">
                            <div className="p-2.5 rounded-xl bg-violet-500/10">
                                <Bot size={22} className="text-violet-500" />
                            </div>
                            <span className="text-2xl font-bold text-base-content">
                                {activeProviders.length}
                                <span className="text-sm font-normal text-base-content/40">
                                    /{aiProviders.length}
                                </span>
                            </span>
                        </div>
                        <p className="text-sm text-base-content/60 mt-2">
                            {t('dashboard.ai_providers', 'AI 提供商')}
                        </p>
                    </div>
                </div>
            </div>

            {/* Cloud Details + Recent Logs */}
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* K8s Cluster Details */}
                <div className="card bg-base-100 shadow-md border border-base-200">
                    <div className="card-body">
                        <h3 className="text-lg font-semibold text-base-content flex items-center gap-2 mb-4">
                            <Network size={20} />
                            {t('dashboard.k8s_overview', 'Kubernetes 集群')}
                        </h3>
                        {!kubeInfo || !kubeInfo.config_exists ? (
                            <div className="text-center py-8 text-base-content/40">
                                <Network size={40} className="mx-auto mb-3 opacity-30" />
                                <p>{t('dashboard.no_kubeconfig', '未找到 kubeconfig')}</p>
                                <p className="text-xs mt-1">
                                    {t('dashboard.no_kubeconfig_hint', '请确保 ~/.kube/config 存在')}
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-2">
                                {kubeInfo.contexts.map((ctx) => (
                                    <div
                                        key={ctx.name}
                                        className={`flex items-center justify-between p-3 rounded-xl transition-colors ${ctx.name === kubeInfo.current_context
                                            ? 'bg-cyan-500/10 border border-cyan-500/20'
                                            : 'bg-base-200/50 hover:bg-base-200'
                                            }`}
                                    >
                                        <div className="flex items-center gap-3">
                                            <div
                                                className={`w-2.5 h-2.5 rounded-full ${ctx.name === kubeInfo.current_context
                                                    ? 'bg-cyan-500'
                                                    : 'bg-gray-400'
                                                    }`}
                                            />
                                            <div>
                                                <span className="font-medium text-base-content text-sm">
                                                    {ctx.name}
                                                </span>
                                                {ctx.name === kubeInfo.current_context && (
                                                    <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded-full bg-cyan-500/20 text-cyan-600">
                                                        当前
                                                    </span>
                                                )}
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-3 text-xs text-base-content/50">
                                            <span>集群: {ctx.cluster}</span>
                                            {ctx.namespace && (
                                                <span className="text-base-content/40">ns: {ctx.namespace}</span>
                                            )}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>

                {/* Aliyun Profile Details */}
                <div className="card bg-base-100 shadow-md border border-base-200">
                    <div className="card-body">
                        <h3 className="text-lg font-semibold text-base-content flex items-center gap-2 mb-4">
                            <Cloud size={20} />
                            {t('dashboard.aliyun_overview', '阿里云配置')}
                        </h3>
                        {!aliyunInfo || !aliyunInfo.config_exists ? (
                            <div className="text-center py-8 text-base-content/40">
                                <Cloud size={40} className="mx-auto mb-3 opacity-30" />
                                <p>{t('dashboard.no_aliyun', '未找到阿里云配置')}</p>
                                <p className="text-xs mt-1">
                                    {t('dashboard.no_aliyun_hint', '请安装 aliyun CLI 或手动创建 ~/.aliyun/config.json')}
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-2">
                                {aliyunInfo.profiles.map((profile) => (
                                    <div
                                        key={profile.name}
                                        className={`flex items-center justify-between p-3 rounded-xl transition-colors ${profile.name === aliyunInfo.current
                                            ? 'bg-orange-500/10 border border-orange-500/20'
                                            : 'bg-base-200/50 hover:bg-base-200'
                                            }`}
                                    >
                                        <div className="flex items-center gap-3">
                                            <div
                                                className={`w-2.5 h-2.5 rounded-full ${profile.name === aliyunInfo.current
                                                    ? 'bg-orange-500'
                                                    : 'bg-gray-400'
                                                    }`}
                                            />
                                            <div>
                                                <span className="font-medium text-base-content text-sm">
                                                    {profile.name}
                                                </span>
                                                {profile.name === aliyunInfo.current && (
                                                    <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded-full bg-orange-500/20 text-orange-600">
                                                        当前
                                                    </span>
                                                )}
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-3 text-xs text-base-content/50">
                                            <span className="px-1.5 py-0.5 rounded bg-base-200 font-mono">
                                                {profile.access_key_hint}
                                            </span>
                                            <span>
                                                {REGION_LABELS[profile.region_id] || profile.region_id}
                                            </span>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>
            </div>

            {/* Server List & Recent Logs */}
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* Server Quick View */}
                <div className="card bg-base-100 shadow-md border border-base-200">
                    <div className="card-body">
                        <h3 className="text-lg font-semibold text-base-content flex items-center gap-2 mb-4">
                            <MonitorCheck size={20} />
                            {t('dashboard.servers_overview', '服务器概览')}
                        </h3>
                        {servers.length === 0 ? (
                            <div className="text-center py-8 text-base-content/40">
                                <Server size={40} className="mx-auto mb-3 opacity-30" />
                                <p>{t('dashboard.no_servers', '暂未添加服务器')}</p>
                                <p className="text-xs mt-1">
                                    {t('dashboard.no_servers_hint', '前往「服务器」页面添加')}
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-2">
                                {servers.slice(0, 5).map((server) => (
                                    <div
                                        key={server.id}
                                        className="flex items-center justify-between p-3 rounded-xl bg-base-200/50 hover:bg-base-200 transition-colors"
                                    >
                                        <div className="flex items-center gap-3">
                                            <div
                                                className={`w-2.5 h-2.5 rounded-full ${server.status === 'online'
                                                    ? 'bg-emerald-500'
                                                    : server.status === 'warning'
                                                        ? 'bg-amber-500 animate-pulse'
                                                        : 'bg-gray-400'
                                                    }`}
                                            />
                                            <div>
                                                <span className="font-medium text-base-content text-sm">
                                                    {server.name}
                                                </span>
                                                <span className="text-xs text-base-content/40 ml-2">
                                                    {server.host}
                                                </span>
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-3 text-xs text-base-content/50">
                                            {server.cpu !== undefined && (
                                                <span className="flex items-center gap-1">
                                                    <Cpu size={12} /> {server.cpu}%
                                                </span>
                                            )}
                                            {server.disk !== undefined && (
                                                <span className="flex items-center gap-1">
                                                    <HardDrive size={12} /> {server.disk}%
                                                </span>
                                            )}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>

                {/* Recent Logs */}
                <div className="card bg-base-100 shadow-md border border-base-200">
                    <div className="card-body">
                        <h3 className="text-lg font-semibold text-base-content flex items-center gap-2 mb-4">
                            <Activity size={20} />
                            {t('dashboard.recent_logs', '最近日志')}
                        </h3>
                        {logs.length === 0 ? (
                            <div className="text-center py-8 text-base-content/40">
                                <Activity size={40} className="mx-auto mb-3 opacity-30" />
                                <p>{t('dashboard.no_logs', '暂无日志')}</p>
                            </div>
                        ) : (
                            <div className="space-y-1.5">
                                {logs.slice(0, 8).map((log) => (
                                    <div
                                        key={log.id}
                                        className="flex items-start gap-2 p-2 rounded-lg text-xs"
                                    >
                                        <span
                                            className={`shrink-0 mt-0.5 w-1.5 h-1.5 rounded-full ${log.level === 'error'
                                                ? 'bg-red-500'
                                                : log.level === 'warn'
                                                    ? 'bg-amber-500'
                                                    : log.level === 'debug'
                                                        ? 'bg-gray-400'
                                                        : 'bg-blue-500'
                                                }`}
                                        />
                                        <span className="text-base-content/40 shrink-0 w-14">
                                            {new Date(log.timestamp).toLocaleTimeString()}
                                        </span>
                                        <span className="text-base-content/70 truncate">
                                            {log.message}
                                        </span>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}

export default Dashboard;
