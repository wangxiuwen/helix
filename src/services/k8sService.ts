/**
 * Kubernetes 集群管理服务
 * 通过 K8s REST API 直接调用，支持 Bearer Token 认证
 */

import { useDevOpsStore } from '../stores/useDevOpsStore';

// ========== Types ==========

export interface K8sPod {
    name: string;
    namespace: string;
    status: string;
    ready: string;         // "1/1"
    restarts: number;
    age: string;
    node: string;
    ip: string;
    containers: string[];
}

export interface K8sDeployment {
    name: string;
    namespace: string;
    replicas: number;
    available: number;
    ready: number;
    age: string;
    images: string[];
}

export interface K8sService {
    name: string;
    namespace: string;
    type: string;          // ClusterIP | NodePort | LoadBalancer
    clusterIP: string;
    ports: string[];
    age: string;
}

export interface K8sNamespace {
    name: string;
    status: string;
    age: string;
}

// ========== Helper ==========

function getK8sConfig() {
    const state = useDevOpsStore.getState();
    const config = state.cloudConfig?.k8s as { kubeconfigPath: string; context: string; namespace: string; apiServer?: string; token?: string; defaultNamespace?: string } | undefined;
    if (!config?.apiServer || !config?.token) {
        throw new Error('请先在设置中配置 K8s 集群连接信息');
    }
    return config as { kubeconfigPath: string; context: string; namespace: string; apiServer: string; token: string; defaultNamespace?: string };
}

function formatAge(dateStr: string): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const days = Math.floor(diff / 86400000);
    const hours = Math.floor((diff % 86400000) / 3600000);
    if (days > 0) return `${days}d${hours}h`;
    const mins = Math.floor((diff % 3600000) / 60000);
    return hours > 0 ? `${hours}h${mins}m` : `${mins}m`;
}

async function k8sApi(path: string, method = 'GET', body?: any): Promise<any> {
    const config = getK8sConfig();
    const url = `${config.apiServer.replace(/\/$/, '')}${path}`;
    const res = await fetch(url, {
        method,
        headers: {
            'Authorization': `Bearer ${config.token}`,
            'Content-Type': 'application/json',
            'Accept': 'application/json',
        },
        body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({ message: res.statusText }));
        throw new Error(`K8s API 错误 (${res.status}): ${err.message || res.statusText}`);
    }
    return res.json();
}

// ========== Pod ==========

export async function listPods(namespace?: string): Promise<K8sPod[]> {
    const ns = namespace || getK8sConfig().defaultNamespace || 'default';
    const path = ns === '_all'
        ? '/api/v1/pods'
        : `/api/v1/namespaces/${ns}/pods`;
    const data = await k8sApi(path);
    return (data.items || []).map((pod: any) => {
        const containers = pod.spec?.containers || [];
        const statuses = pod.status?.containerStatuses || [];
        const readyCount = statuses.filter((s: any) => s.ready).length;
        const restarts = statuses.reduce((sum: number, s: any) => sum + (s.restartCount || 0), 0);
        return {
            name: pod.metadata.name,
            namespace: pod.metadata.namespace,
            status: pod.status?.phase || 'Unknown',
            ready: `${readyCount}/${containers.length}`,
            restarts,
            age: formatAge(pod.metadata.creationTimestamp),
            node: pod.spec?.nodeName || '',
            ip: pod.status?.podIP || '',
            containers: containers.map((c: any) => c.name),
        };
    });
}

// ========== Deployment ==========

export async function listDeployments(namespace?: string): Promise<K8sDeployment[]> {
    const ns = namespace || getK8sConfig().defaultNamespace || 'default';
    const path = ns === '_all'
        ? '/apis/apps/v1/deployments'
        : `/apis/apps/v1/namespaces/${ns}/deployments`;
    const data = await k8sApi(path);
    return (data.items || []).map((dep: any) => ({
        name: dep.metadata.name,
        namespace: dep.metadata.namespace,
        replicas: dep.spec?.replicas || 0,
        available: dep.status?.availableReplicas || 0,
        ready: dep.status?.readyReplicas || 0,
        age: formatAge(dep.metadata.creationTimestamp),
        images: (dep.spec?.template?.spec?.containers || []).map((c: any) => c.image),
    }));
}

// ========== Service ==========

export async function listServices(namespace?: string): Promise<K8sService[]> {
    const ns = namespace || getK8sConfig().defaultNamespace || 'default';
    const path = ns === '_all'
        ? '/api/v1/services'
        : `/api/v1/namespaces/${ns}/services`;
    const data = await k8sApi(path);
    return (data.items || []).map((svc: any) => ({
        name: svc.metadata.name,
        namespace: svc.metadata.namespace,
        type: svc.spec?.type || 'ClusterIP',
        clusterIP: svc.spec?.clusterIP || '',
        ports: (svc.spec?.ports || []).map((p: any) =>
            `${p.port}${p.nodePort ? ':' + p.nodePort : ''}/${p.protocol || 'TCP'}`
        ),
        age: formatAge(svc.metadata.creationTimestamp),
    }));
}

// ========== Namespace ==========

export async function listNamespaces(): Promise<K8sNamespace[]> {
    const data = await k8sApi('/api/v1/namespaces');
    return (data.items || []).map((ns: any) => ({
        name: ns.metadata.name,
        status: ns.status?.phase || 'Active',
        age: formatAge(ns.metadata.creationTimestamp),
    }));
}

// ========== Pod Logs ==========

export async function getPodLogs(namespace: string, podName: string, tailLines = 100): Promise<string> {
    const config = getK8sConfig();
    const url = `${config.apiServer.replace(/\/$/, '')}/api/v1/namespaces/${namespace}/pods/${podName}/log?tailLines=${tailLines}`;
    const res = await fetch(url, {
        headers: { 'Authorization': `Bearer ${config.token}` },
    });
    if (!res.ok) throw new Error(`获取日志失败: ${res.statusText}`);
    return res.text();
}

// ========== Scale ==========

export async function scaleDeployment(namespace: string, name: string, replicas: number): Promise<string> {
    await k8sApi(
        `/apis/apps/v1/namespaces/${namespace}/deployments/${name}/scale`,
        'PATCH',
        { spec: { replicas } }
    );
    return `Deployment ${name} 扩缩容至 ${replicas} 副本`;
}

// ========== Restart ==========

export async function restartDeployment(namespace: string, name: string): Promise<string> {
    // Restart by patching the template annotation
    await k8sApi(
        `/apis/apps/v1/namespaces/${namespace}/deployments/${name}`,
        'PATCH',
        {
            spec: {
                template: {
                    metadata: {
                        annotations: {
                            'kubectl.kubernetes.io/restartedAt': new Date().toISOString(),
                        },
                    },
                },
            },
        }
    );
    return `Deployment ${name} 正在滚动重启`;
}

// ========== Delete Pod ==========

export async function deletePod(namespace: string, podName: string): Promise<string> {
    await k8sApi(`/api/v1/namespaces/${namespace}/pods/${podName}`, 'DELETE');
    return `Pod ${podName} 已删除`;
}

// ========== Connection Test ==========

export async function testConnection(): Promise<boolean> {
    try {
        await k8sApi('/api/v1/namespaces');
        return true;
    } catch {
        return false;
    }
}
