/**
 * 技能系统 — 模块化的 AI 工具集
 * 
 * 每个 Skill 包含一组相关工具，可独立启用/禁用。
 * AI 对话时只加载已启用技能的工具。
 * 支持内置技能 + 用户自定义技能。
 */

import * as aliyun from './aliyunService';
import * as k8s from './k8sService';
import { useDevOpsStore } from '../stores/useDevOpsStore';

// ========== Types ==========

export interface ToolParameter {
    type: 'string' | 'number' | 'boolean';
    description: string;
    required?: boolean;
    enum?: string[];
}

export interface OpsTool {
    name: string;
    description: string;
    dangerous?: boolean;
    parameters: Record<string, ToolParameter>;
    execute: (params: Record<string, any>) => Promise<string>;
}

export interface OpsSkill {
    id: string;
    name: string;
    description: string;
    icon: string;           // emoji or icon name
    category: 'cloud' | 'container' | 'server' | 'devops' | 'notification' | 'custom';
    builtin: boolean;       // 内置技能不可删除
    enabled: boolean;
    tools: OpsTool[];
    version?: string;
    author?: string;
    configRequired?: string[];  // 需要哪些配置才能使用
}

// ========== 内置技能定义 ==========

const aliyunSkill: OpsSkill = {
    id: 'skill-aliyun-ecs',
    name: '阿里云 ECS',
    description: '管理阿里云 ECS 实例：查看、启动、停止、重启云服务器',
    icon: '☁️',
    category: 'cloud',
    builtin: true,
    enabled: true,
    version: '1.0.0',
    author: 'helix',
    configRequired: ['aliyun.accessKeyId', 'aliyun.accessKeySecret'],
    tools: [
        {
            name: 'list_ecs_instances',
            description: '列出阿里云 ECS 实例，返回实例ID、名称、状态、IP、CPU、内存等信息',
            parameters: {
                region: { type: 'string', description: '地域ID，如 cn-beijing', required: false },
            },
            execute: async (params: Record<string, any>) => {
                const instances = await aliyun.describeInstances(params.region);
                if (instances.length === 0) return '当前地域没有 ECS 实例';
                return instances.map((i: any) =>
                    `• ${i.InstanceName} (${i.InstanceId}) | 状态: ${i.Status} | ${i.Cpu}核${i.Memory}MB | IP: ${i.PublicIpAddress?.join(',') || i.InnerIpAddress?.join(',') || '无'} | 类型: ${i.InstanceType}`
                ).join('\n');
            },
        },
        {
            name: 'start_ecs_instance',
            description: '启动一个阿里云 ECS 实例',
            dangerous: true,
            parameters: {
                instance_id: { type: 'string', description: 'ECS 实例ID', required: true },
            },
            execute: async (params: Record<string, any>) => aliyun.startInstance(params.instance_id),
        },
        {
            name: 'stop_ecs_instance',
            description: '停止一个阿里云 ECS 实例',
            dangerous: true,
            parameters: {
                instance_id: { type: 'string', description: 'ECS 实例ID', required: true },
            },
            execute: async (params: Record<string, any>) => aliyun.stopInstance(params.instance_id),
        },
        {
            name: 'reboot_ecs_instance',
            description: '重启一个阿里云 ECS 实例',
            dangerous: true,
            parameters: {
                instance_id: { type: 'string', description: 'ECS 实例ID', required: true },
            },
            execute: async (params: Record<string, any>) => aliyun.rebootInstance(params.instance_id),
        },
    ],
};

const k8sSkill: OpsSkill = {
    id: 'skill-k8s',
    name: 'Kubernetes 集群',
    description: '管理 K8s 集群：查看 Pod/Deployment/Service，读取日志，扩缩容，滚动重启',
    icon: '⚓',
    category: 'container',
    builtin: true,
    enabled: true,
    version: '1.0.0',
    author: 'helix',
    configRequired: ['k8s.apiServer', 'k8s.token'],
    tools: [
        {
            name: 'list_k8s_pods',
            description: '列出 Kubernetes Pod，返回名称、状态、就绪状态、重启次数等',
            parameters: {
                namespace: { type: 'string', description: '命名空间，默认 default，使用 _all 查看全部', required: false },
            },
            execute: async (params: Record<string, any>) => {
                const pods = await k8s.listPods(params.namespace);
                if (pods.length === 0) return '当前命名空间没有 Pod';
                return pods.map((p: any) =>
                    `• ${p.name} | 状态: ${p.status} | 就绪: ${p.ready} | 重启: ${p.restarts} | 节点: ${p.node} | IP: ${p.ip}`
                ).join('\n');
            },
        },
        {
            name: 'list_k8s_deployments',
            description: '列出 Kubernetes Deployment，返回名称、副本数、就绪状态',
            parameters: {
                namespace: { type: 'string', description: '命名空间', required: false },
            },
            execute: async (params: Record<string, any>) => {
                const deps = await k8s.listDeployments(params.namespace);
                if (deps.length === 0) return '当前命名空间没有 Deployment';
                return deps.map((d: any) =>
                    `• ${d.name} | 副本: ${d.ready}/${d.replicas} | 镜像: ${d.images.join(', ')}`
                ).join('\n');
            },
        },
        {
            name: 'list_k8s_services',
            description: '列出 Kubernetes Service，返回名称、类型、端口',
            parameters: {
                namespace: { type: 'string', description: '命名空间', required: false },
            },
            execute: async (params: Record<string, any>) => {
                const svcs = await k8s.listServices(params.namespace);
                if (svcs.length === 0) return '当前命名空间没有 Service';
                return svcs.map((s: any) =>
                    `• ${s.name} | 类型: ${s.type} | ClusterIP: ${s.clusterIP} | 端口: ${s.ports.join(', ')}`
                ).join('\n');
            },
        },
        {
            name: 'list_k8s_namespaces',
            description: '列出所有 Kubernetes 命名空间',
            parameters: {},
            execute: async () => {
                const nss = await k8s.listNamespaces();
                return nss.map((n: any) => `• ${n.name} | 状态: ${n.status} | 存活: ${n.age}`).join('\n');
            },
        },
        {
            name: 'get_pod_logs',
            description: '获取指定 Pod 的日志',
            parameters: {
                namespace: { type: 'string', description: '命名空间', required: true },
                pod_name: { type: 'string', description: 'Pod 名称', required: true },
                tail_lines: { type: 'number', description: '返回最后多少行，默认 50', required: false },
            },
            execute: async (params: Record<string, any>) => {
                const logs = await k8s.getPodLogs(params.namespace, params.pod_name, params.tail_lines || 50);
                return logs || '(日志为空)';
            },
        },
        {
            name: 'scale_k8s_deployment',
            description: '对 Kubernetes Deployment 进行扩缩容',
            dangerous: true,
            parameters: {
                namespace: { type: 'string', description: '命名空间', required: true },
                deployment_name: { type: 'string', description: 'Deployment 名称', required: true },
                replicas: { type: 'number', description: '目标副本数', required: true },
            },
            execute: async (params: Record<string, any>) => k8s.scaleDeployment(params.namespace, params.deployment_name, params.replicas),
        },
        {
            name: 'restart_k8s_deployment',
            description: '滚动重启 Kubernetes Deployment',
            dangerous: true,
            parameters: {
                namespace: { type: 'string', description: '命名空间', required: true },
                deployment_name: { type: 'string', description: 'Deployment 名称', required: true },
            },
            execute: async (params: Record<string, any>) => k8s.restartDeployment(params.namespace, params.deployment_name),
        },
        {
            name: 'delete_k8s_pod',
            description: '删除指定 Pod（会被 Deployment 自动重新创建）',
            dangerous: true,
            parameters: {
                namespace: { type: 'string', description: '命名空间', required: true },
                pod_name: { type: 'string', description: 'Pod 名称', required: true },
            },
            execute: async (params: Record<string, any>) => k8s.deletePod(params.namespace, params.pod_name),
        },
    ],
};

const serverSkill: OpsSkill = {
    id: 'skill-server-mgmt',
    name: '服务器管理',
    description: '管理已添加的服务器节点：查看列表、检查状态、连通性检测',
    icon: '🖥️',
    category: 'server',
    builtin: true,
    enabled: true,
    version: '1.0.0',
    author: 'helix',
    tools: [
        {
            name: 'list_servers',
            description: '列出所有已添加的服务器节点及其状态',
            parameters: {},
            execute: async () => {
                const { servers } = useDevOpsStore.getState();
                if (servers.length === 0) return '暂无服务器节点';
                return servers.map((s: any) =>
                    `• ${s.name} (${s.host}${s.port ? ':' + s.port : ''}) | 状态: ${s.status} | 标签: ${s.tags?.join(',') || '无'}`
                ).join('\n');
            },
        },
        {
            name: 'check_server_status',
            description: '检查所有服务器的连接状态',
            parameters: {},
            execute: async () => {
                const store = useDevOpsStore.getState();
                await store.checkAllServers();
                const { servers } = useDevOpsStore.getState();
                const online = servers.filter((s: any) => s.status === 'online').length;
                return `已检查 ${servers.length} 台服务器，${online} 台在线，${servers.length - online} 台离线`;
            },
        },
    ],
};

const cronSkill: OpsSkill = {
    id: 'skill-cron-jobs',
    name: '定时任务',
    description: '查看和管理定时任务，包括 Cron 调度和手动执行',
    icon: '⏰',
    category: 'devops',
    builtin: true,
    enabled: true,
    version: '1.0.0',
    author: 'helix',
    tools: [
        {
            name: 'list_cron_jobs',
            description: '列出所有定时任务及其状态',
            parameters: {},
            execute: async () => {
                const { tasks } = useDevOpsStore.getState();
                if (tasks.length === 0) return '暂无定时任务';
                return tasks.map((t: any) =>
                    `• ${t.name} | 调度: ${t.schedule || '手动'} | 状态: ${t.status} | 上次执行: ${t.lastRun || '未执行'} | 结果: ${t.lastResult || '未知'}`
                ).join('\n');
            },
        },
    ],
};

const notificationSkill: OpsSkill = {
    id: 'skill-notification',
    name: '消息通知',
    description: '通过飞书或钉钉 Webhook 发送通知消息',
    icon: '📢',
    category: 'notification',
    builtin: true,
    enabled: true,
    version: '1.0.0',
    author: 'helix',
    configRequired: ['botChannels'],
    tools: [
        {
            name: 'send_notification',
            description: '发送通知到飞书或钉钉群',
            parameters: {
                channel: { type: 'string', description: '通知渠道', required: true, enum: ['feishu', 'dingtalk', 'wecom'] },
                message: { type: 'string', description: '通知内容', required: true },
            },
            execute: async (params: Record<string, any>) => {
                const { botChannels } = useDevOpsStore.getState();
                const channel = botChannels?.find(
                    (c: any) => c.type === params.channel && c.enabled
                );
                if (!channel || !channel.config?.webhookUrl) return `未配置或未启用 ${params.channel === 'feishu' ? '飞书' : params.channel === 'dingtalk' ? '钉钉' : '企业微信'} 通知渠道 Webhook`;

                const res = await fetch(channel.config.webhookUrl, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(
                        params.channel === 'feishu'
                            ? { msg_type: 'text', content: { text: `[helix] ${params.message}` } }
                            : params.channel === 'dingtalk'
                                ? { msgtype: 'markdown', markdown: { title: '通知', text: `[helix] ${params.message}` } }
                                : { msgtype: 'text', text: { content: `[helix] ${params.message}` } } // WeCom format
                    ),
                });

                if (!res.ok) throw new Error(`发送通知失败: ${res.statusText}`);
                return `通知已发送到${params.channel === 'feishu' ? '飞书' : params.channel === 'dingtalk' ? '钉钉' : '企业微信'}`;
            },
        },
    ],
};

// ========== 所有内置技能 ==========

export const builtinSkills: OpsSkill[] = [
    aliyunSkill,
    k8sSkill,
    serverSkill,
    cronSkill,
    notificationSkill,
];

// ========== Skill Manager ==========

let _skills: OpsSkill[] = [...builtinSkills];

export function getAllSkills(): OpsSkill[] {
    return _skills;
}

export function getEnabledSkills(): OpsSkill[] {
    return _skills.filter(s => s.enabled);
}

export function setSkillEnabled(skillId: string, enabled: boolean): void {
    const skill = _skills.find(s => s.id === skillId);
    if (skill) skill.enabled = enabled;
}

export function addCustomSkill(skill: OpsSkill): void {
    // Prevent duplicate IDs
    _skills = _skills.filter(s => s.id !== skill.id);
    _skills.push({ ...skill, builtin: false });
}

export function removeCustomSkill(skillId: string): boolean {
    const skill = _skills.find(s => s.id === skillId);
    if (!skill || skill.builtin) return false;
    _skills = _skills.filter(s => s.id !== skillId);
    return true;
}

export function getSkillById(skillId: string): OpsSkill | undefined {
    return _skills.find(s => s.id === skillId);
}

// Sync skill states from persisted store
export function syncSkillStates(states: Record<string, boolean>): void {
    for (const [id, enabled] of Object.entries(states)) {
        setSkillEnabled(id, enabled);
    }
}

// Load custom skills from persisted store
export function loadCustomSkills(customs: Array<{
    id: string;
    name: string;
    description: string;
    icon: string;
    category: OpsSkill['category'];
    tools: Array<{
        name: string;
        description: string;
        dangerous?: boolean;
        parameters: Record<string, ToolParameter>;
        script: string; // JavaScript function body as string
    }>;
}>): void {
    for (const custom of customs) {
        const skill: OpsSkill = {
            id: custom.id,
            name: custom.name,
            description: custom.description,
            icon: custom.icon,
            category: custom.category,
            builtin: false,
            enabled: true,
            tools: custom.tools.map(t => ({
                name: t.name,
                description: t.description,
                dangerous: t.dangerous,
                parameters: t.parameters,
                execute: buildCustomExecutor(t.script),
            })),
        };
        addCustomSkill(skill);
    }
}

function buildCustomExecutor(script: string): (params: Record<string, any>) => Promise<string> {
    return async (params: Record<string, any>) => {
        try {
            // Execute the script with params in scope
            const fn = new Function('params', 'fetch', `return (async () => { ${script} })()`);
            const result = await fn(params, fetch);
            return typeof result === 'string' ? result : JSON.stringify(result, null, 2);
        } catch (err: any) {
            return `自定义技能执行失败: ${err.message}`;
        }
    };
}

// ========== For AI Function Calling (only enabled skills) ==========

export function getToolsForAI(): Array<{
    type: 'function';
    function: { name: string; description: string; parameters: any };
}> {
    const enabledTools = getEnabledSkills().flatMap(s => s.tools);
    return enabledTools.map(tool => ({
        type: 'function' as const,
        function: {
            name: tool.name,
            description: tool.description,
            parameters: {
                type: 'object',
                properties: Object.fromEntries(
                    Object.entries(tool.parameters).map(([key, param]) => [
                        key,
                        {
                            type: param.type,
                            description: param.description,
                            ...(param.enum ? { enum: param.enum } : {}),
                        },
                    ])
                ),
                required: Object.entries(tool.parameters)
                    .filter(([, p]) => p.required)
                    .map(([k]) => k),
            },
        },
    }));
}

export function findTool(name: string): OpsTool | undefined {
    return getEnabledSkills().flatMap(s => s.tools).find(t => t.name === name);
}

export async function executeTool(name: string, params: Record<string, any>): Promise<{ result: string; dangerous: boolean }> {
    const tool = findTool(name);
    if (!tool) return { result: `未知工具: ${name}`, dangerous: false };
    try {
        const result = await tool.execute(params);
        return { result, dangerous: !!tool.dangerous };
    } catch (err: any) {
        return { result: `执行失败: ${err.message}`, dangerous: false };
    }
}

// ========== Skills Prompt Injection (仿 OpenClaw formatSkillsForPrompt) ==========

export function buildSkillsPrompt(): string {
    const enabled = getEnabledSkills();
    if (enabled.length === 0) return '';

    const lines: string[] = [
        '## 可用技能',
        '',
        `当前已启用 ${enabled.length} 个技能，共 ${enabled.reduce((n, s) => n + s.tools.length, 0)} 个工具。`,
        '',
    ];

    for (const skill of enabled) {
        lines.push(`### ${skill.icon} ${skill.name}`);
        lines.push(skill.description);
        lines.push('');
        for (const tool of skill.tools) {
            const paramSig = Object.entries(tool.parameters)
                .map(([k, p]) => `${k}${p.required ? '' : '?'}: ${p.type}`)
                .join(', ');
            lines.push(`- \`${tool.name}(${paramSig})\` — ${tool.description}${tool.dangerous ? ' ⚠️危险' : ''}`);
        }
        lines.push('');
    }

    return lines.join('\n');
}

// ========== Tool Loop Detection (仿 OpenClaw tool-loop-detection.ts) ==========

export interface ToolCallRecord {
    name: string;
    argsHash: string;
    timestamp: number;
}

export type LoopDetectionResult = {
    blocked: boolean;
    warning: boolean;
    message?: string;
};

export class ToolLoopDetector {
    private history: ToolCallRecord[] = [];
    private readonly historySize: number;
    private readonly warningThreshold: number;
    private readonly blockThreshold: number;

    constructor(opts?: { historySize?: number; warningThreshold?: number; blockThreshold?: number }) {
        this.historySize = opts?.historySize ?? 30;
        this.warningThreshold = opts?.warningThreshold ?? 3;
        this.blockThreshold = opts?.blockThreshold ?? 5;
    }

    private hashArgs(args: Record<string, any>): string {
        try {
            return JSON.stringify(args);
        } catch {
            return '{}';
        }
    }

    record(name: string, args: Record<string, any>): LoopDetectionResult {
        const entry: ToolCallRecord = {
            name,
            argsHash: this.hashArgs(args),
            timestamp: Date.now(),
        };
        this.history.push(entry);
        if (this.history.length > this.historySize) {
            this.history = this.history.slice(-this.historySize);
        }

        // Check for consecutive identical calls
        const tail = this.history.slice(-this.blockThreshold);
        const identicalCount = tail.filter(
            r => r.name === name && r.argsHash === entry.argsHash
        ).length;

        if (identicalCount >= this.blockThreshold) {
            return {
                blocked: true,
                warning: true,
                message: `工具 ${name} 被连续调用 ${identicalCount} 次（相同参数），已自动中断以防止无限循环。`,
            };
        }
        if (identicalCount >= this.warningThreshold) {
            return {
                blocked: false,
                warning: true,
                message: `工具 ${name} 已连续调用 ${identicalCount} 次，可能存在循环。`,
            };
        }

        // Check ping-pong pattern (alternating between 2 tools)
        if (this.history.length >= 6) {
            const last6 = this.history.slice(-6);
            const nameA = last6[0].name;
            const nameB = last6[1].name;
            if (nameA !== nameB) {
                const isPingPong = last6.every((r, i) => r.name === (i % 2 === 0 ? nameA : nameB));
                if (isPingPong) {
                    return {
                        blocked: true,
                        warning: true,
                        message: `检测到工具 ${nameA} 和 ${nameB} 之间的乒乓循环，已自动中断。`,
                    };
                }
            }
        }

        return { blocked: false, warning: false };
    }

    reset(): void {
        this.history = [];
    }
}

// ========== Tool Event Types (仿 OpenClaw handleToolExecutionStart/End) ==========

export type ToolEventPhase = 'start' | 'result' | 'error' | 'retry' | 'loop_warning' | 'loop_blocked';

export interface ToolEvent {
    phase: ToolEventPhase;
    toolName: string;
    args?: Record<string, any>;
    result?: string;
    error?: string;
    meta?: string;       // descriptive label
    dangerous?: boolean;
    timestamp: number;
}

// ========== Custom Skill Security Scanner (仿 OpenClaw skill-scanner.ts) ==========

export type ScanSeverity = 'info' | 'warn' | 'critical';

export interface ScanFinding {
    ruleId: string;
    severity: ScanSeverity;
    message: string;
    evidence: string;
    line: number;
}

export interface ScanSummary {
    critical: number;
    warn: number;
    info: number;
    findings: ScanFinding[];
}

const SCAN_LINE_RULES: Array<{
    ruleId: string;
    severity: ScanSeverity;
    message: string;
    pattern: RegExp;
    requiresContext?: RegExp;
}> = [
        {
            ruleId: 'dangerous-exec',
            severity: 'critical',
            message: '检测到 Shell 命令执行 (child_process)',
            pattern: /\b(exec|execSync|spawn|spawnSync|execFile|execFileSync)\s*\(/,
            requiresContext: /child_process/,
        },
        {
            ruleId: 'dynamic-code-execution',
            severity: 'critical',
            message: '检测到动态代码执行',
            pattern: /\beval\s*\(|new\s+Function\s*\(/,
        },
        {
            ruleId: 'crypto-mining',
            severity: 'critical',
            message: '检测到可能的挖矿代码',
            pattern: /stratum\+tcp|stratum\+ssl|coinhive|cryptonight|xmrig/i,
        },
        {
            ruleId: 'suspicious-network',
            severity: 'warn',
            message: 'WebSocket 连接到非标准端口',
            pattern: /new\s+WebSocket\s*\(\s*["']wss?:\/\/[^"']*:(\d+)/,
        },
        {
            ruleId: 'env-access',
            severity: 'warn',
            message: '访问环境变量',
            pattern: /process\.env/,
        },
        {
            ruleId: 'fs-access',
            severity: 'warn',
            message: '检测到文件系统访问',
            pattern: /readFileSync|writeFileSync|readFile|writeFile|unlinkSync|rmSync/,
        },
        {
            ruleId: 'obfuscated-code',
            severity: 'warn',
            message: '检测到 Hex 编码字符串（可能的混淆代码）',
            pattern: /(\\x[0-9a-fA-F]{2}){6,}/,
        },
    ];

export function scanCustomSkillScript(script: string): ScanSummary {
    const findings: ScanFinding[] = [];
    const lines = script.split('\n');
    const matchedRules = new Set<string>();

    for (const rule of SCAN_LINE_RULES) {
        if (matchedRules.has(rule.ruleId)) continue;

        // Skip if context requirement not met
        if (rule.requiresContext && !rule.requiresContext.test(script)) continue;

        for (let i = 0; i < lines.length; i++) {
            if (rule.pattern.test(lines[i])) {
                findings.push({
                    ruleId: rule.ruleId,
                    severity: rule.severity,
                    message: rule.message,
                    evidence: lines[i].trim().slice(0, 120),
                    line: i + 1,
                });
                matchedRules.add(rule.ruleId);
                break;
            }
        }
    }

    return {
        critical: findings.filter(f => f.severity === 'critical').length,
        warn: findings.filter(f => f.severity === 'warn').length,
        info: findings.filter(f => f.severity === 'info').length,
        findings,
    };
}

export function getSkillEnabled(skill: OpsSkill): boolean {
    return skill.enabled;
}

// ========== Agent Skills System (SKILL.md — 仿 OpenClaw) ==========

export interface AgentSkillMetadata {
    emoji?: string;
    requires?: {
        bins?: string[];
        config?: string[];
    };
    install?: Array<{
        id?: string;
        kind: string;
        label?: string;
    }>;
}

export interface AgentSkill {
    /** Unique name from frontmatter */
    name: string;
    /** Short description from frontmatter */
    description: string;
    /** Markdown body — the AI prompt instructions */
    body: string;
    /** Parsed from metadata.openclaw in frontmatter */
    metadata: AgentSkillMetadata;
    /** Source: 'builtin' | 'user' | 'project' */
    source: 'builtin' | 'user' | 'project';
    /** Allowed tools from frontmatter */
    allowedTools?: string[];
    /** Is it enabled */
    enabled: boolean;
    /** File path of the SKILL.md */
    filePath: string;
}

/**
 * Parse a SKILL.md file: YAML frontmatter delimited by --- plus markdown body.
 * Adapted from OpenClaw's loadSkillsFromDir / pi-coding-agent parser.
 */
export function parseSkillMd(raw: string, filePath: string, source: AgentSkill['source']): AgentSkill | null {
    const fmMatch = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/);
    if (!fmMatch) return null;

    const fmBlock = fmMatch[1];
    const body = fmMatch[2].trim();

    // Simple YAML parser for flat frontmatter fields
    const fm: Record<string, string> = {};
    for (const line of fmBlock.split('\n')) {
        const m = line.match(/^(\w[\w-]*)\s*:\s*(.+)$/);
        if (m) fm[m[1]] = m[2].trim();
    }

    if (!fm['name']) return null;

    // Parse metadata JSON if present
    let metadata: AgentSkillMetadata = {};
    if (fm['metadata']) {
        try {
            const raw = fm['metadata'];
            const parsed = JSON.parse(raw);
            if (parsed?.openclaw) {
                metadata = {
                    emoji: parsed.openclaw.emoji,
                    requires: parsed.openclaw.requires,
                    install: parsed.openclaw.install,
                };
            }
        } catch {
            // metadata is optional, ignore parse errors
        }
    }

    // Parse allowed-tools
    let allowedTools: string[] | undefined;
    if (fm['allowed-tools']) {
        try {
            allowedTools = JSON.parse(fm['allowed-tools']);
        } catch {
            allowedTools = fm['allowed-tools'].split(',').map(s => s.trim().replace(/"/g, ''));
        }
    }

    // Clean description (remove YAML multiline indicators)
    let description = fm['description'] || fm['name'];
    description = description.replace(/^\|?\s*/, '').replace(/^"(.*)"$/, '$1');

    return {
        name: fm['name'],
        description,
        body,
        metadata,
        source,
        allowedTools,
        enabled: true,
        filePath,
    };
}

/**
 * Directory of built-in SKILL.md content.
 * In a real fs-based setup these would be loaded from public/skills/,
 * but since we're in a browser SPA, we embed them as a registry.
 */
const BUILTIN_SKILLS: Array<{ name: string; raw: string }> = [
    {
        name: 'ecs-ops',
        raw: `---
name: ecs-ops
description: "阿里云 ECS 实例操作指南"
metadata: { "openclaw": { "emoji": "☁️", "requires": { "config": ["aliyun.accessKeyId", "aliyun.accessKeySecret"] } } }
allowed-tools: ["list_ecs_instances", "start_ecs_instance", "stop_ecs_instance", "reboot_ecs_instance"]
---

# 阿里云 ECS 管理

使用 ECS 工具管理云服务器实例。

## 操作规范

- 查询操作直接执行，无需确认
- **停止/重启等危险操作** 必须先确认实例 ID 和当前状态
- 优先使用 \`list_ecs_instances\` 获取实例列表再操作

## 常用动作

### 列出实例

\`\`\`json
{ "tool": "list_ecs_instances", "params": { "region": "cn-hangzhou" } }
\`\`\`

### 停止实例 ⚠️

\`\`\`json
{ "tool": "stop_ecs_instance", "params": { "instanceId": "i-xxx", "forceStop": false } }
\`\`\`

- \`forceStop: true\` 仅在实例无响应时使用

### 重启实例 ⚠️

\`\`\`json
{ "tool": "reboot_ecs_instance", "params": { "instanceId": "i-xxx", "forceReboot": false } }
\`\`\`

## 安全守则

- 不在生产高峰时间执行停止/重启
- 操作前确认实例名称、ID、状态
- 批量操作逐台执行，不并行
`
    },
    {
        name: 'k8s-ops',
        raw: `---
name: k8s-ops
description: "Kubernetes 集群操作指南"
metadata: { "openclaw": { "emoji": "⚓", "requires": { "config": ["k8s.apiServer", "k8s.token"] } } }
allowed-tools: ["list_k8s_pods", "list_k8s_deployments", "list_k8s_services", "list_k8s_namespaces", "get_pod_logs", "scale_k8s_deployment", "restart_k8s_deployment", "delete_k8s_pod"]
---

# Kubernetes 集群管理

管理 K8s 集群的 Pod、Deployment、Service 等资源。

## 诊断流程

1. 先用 \`list_k8s_namespaces\` 确认命名空间
2. 用 \`list_k8s_pods\` 查看 Pod 状态，关注 \`CrashLoopBackOff\` / \`ImagePullBackOff\`
3. 异常 Pod 用 \`get_pod_logs\` 查看日志
4. 需要修复时通过 \`restart_k8s_deployment\` 滚动重启

## 扩缩容

\`\`\`json
{ "tool": "scale_k8s_deployment", "params": { "deployment": "api-server", "namespace": "production", "replicas": 5 } }
\`\`\`

- 缩容前确认当前副本数和流量
- 扩容注意节点资源是否充足

## 安全守则

- \`delete_k8s_pod\` 仅用于异常 Pod，不删除正常运行的 Pod
- 生产命名空间操作需二次确认
- 避免同时重启同一 Deployment 的所有 Pod
`
    },
    {
        name: 'server-monitor',
        raw: `---
name: server-monitor
description: "服务器状态监控与巡检指南"
metadata: { "openclaw": { "emoji": "🖥️" } }
allowed-tools: ["check_server_status", "run_server_command"]
---

# 服务器监控

通过 SSH 连接检查服务器运行状态并执行管理命令。

## 巡检流程

1. \`check_server_status\` — 获取 CPU、内存、磁盘、负载
2. 关注指标阈值:
   - CPU > 80% → 告警
   - 内存 > 90% → 告警
   - 磁盘 > 85% → 告警
   - 负载 > CPU核数 × 2 → 告警

## 命令执行 ⚠️

\`run_server_command\` 可执行远程命令，仅用于:
- 查看进程: \`ps aux | head -20\`
- 查看日志: \`tail -50 /var/log/syslog\`
- 网络检查: \`ss -tlnp\`

**禁止执行**: \`rm -rf\`、\`dd\`、\`mkfs\`、\`shutdown\`、\`reboot\` 等破坏性命令

## 汇报格式

巡检结果以表格形式呈现：服务器名 | CPU | 内存 | 磁盘 | 状态
`
    },
    {
        name: 'devops-tasks',
        raw: `---
name: devops-tasks
description: "定时任务与 CI/CD 操作指南"
metadata: { "openclaw": { "emoji": "⚙️" } }
allowed-tools: ["list_cron_jobs", "toggle_cron_job"]
---

# DevOps 定时任务管理

管理系统定时任务（cron jobs）和 CI/CD 流水线。

## 查看任务

\`\`\`json
{ "tool": "list_cron_jobs", "params": {} }
\`\`\`

## 启用/禁用

\`\`\`json
{ "tool": "toggle_cron_job", "params": { "jobId": "backup-daily", "enabled": false } }
\`\`\`

## 规范

- 禁用任务前确认影响范围
- 记录操作日志
- 紧急禁用后安排恢复计划
`
    },
    {
        name: 'notification',
        raw: `---
name: notification
description: "通知渠道操作指南—飞书/钉钉/Webhook"
metadata: { "openclaw": { "emoji": "📢" } }
allowed-tools: ["send_notification"]
---

# 通知管理

通过 \`send_notification\` 向飞书、钉钉、Webhook 等渠道发送消息。

## 使用方式

\`\`\`json
{ "tool": "send_notification", "params": { "channel": "feishu", "message": "告警：API 响应时间 > 5s", "level": "warning" } }
\`\`\`

## 渠道

| 渠道 | channel 值 | 配置 |
|------|-----------|------|
| 飞书 | feishu | webhookUrl |
| 钉钉 | dingtalk | webhookUrl |
| 企微 | wecom | webhookUrl |
| Webhook | webhook | url |

## 规范

- 告警信息包含: 时间、指标、当前值、阈值
- 避免重复发送相同告警
- 紧急告警用 \`level: "critical"\`
`
    },
];

/**
 * Load all built-in agent skills (embedded in the app).
 */
function loadBuiltinAgentSkills(): AgentSkill[] {
    const skills: AgentSkill[] = [];
    for (const entry of BUILTIN_SKILLS) {
        const skill = parseSkillMd(entry.raw, `builtin://${entry.name}/SKILL.md`, 'builtin');
        if (skill) skills.push(skill);
    }
    return skills;
}

/** In-memory cache for loaded agent skills */
let cachedAgentSkills: AgentSkill[] | null = null;

/**
 * Load all agent skills from all sources, merged by priority.
 * In browser SPA, we only have built-in skills; user/project skills
 * can be added via the store's custom skill mechanism.
 */
export function loadAllAgentSkills(): AgentSkill[] {
    if (cachedAgentSkills) return cachedAgentSkills;
    cachedAgentSkills = loadBuiltinAgentSkills();
    return cachedAgentSkills;
}

/**
 * Add a user-defined agent skill (parsed from SKILL.md content).
 */
export function addUserAgentSkill(raw: string): AgentSkill | null {
    const skill = parseSkillMd(raw, 'user://custom/SKILL.md', 'user');
    if (!skill) return null;
    // Clear cache to force reload
    cachedAgentSkills = null;
    return skill;
}

/** Reset cache (e.g., when skill states change) */
export function resetAgentSkillsCache(): void {
    cachedAgentSkills = null;
}

/**
 * Build the AI system prompt section from enabled agent skills.
 * Adapted from OpenClaw's formatSkillsForPrompt + buildWorkspaceSkillsPrompt.
 *
 * Format: each skill's markdown body is included under a header with emoji + name.
 * Limits: max 30KB total, max 150 skills.
 */
export function buildAgentSkillsPrompt(skills: AgentSkill[]): string {
    const MAX_SKILLS = 150;
    const MAX_CHARS = 30_000;

    const enabled = skills.filter(s => s.enabled);
    if (enabled.length === 0) return '';

    const limited = enabled.slice(0, MAX_SKILLS);
    const lines: string[] = [
        '## 可用技能 (Agent Skills)',
        '',
        `已加载 ${limited.length} 个技能模块。每个技能包含专业操作指南。`,
        '',
    ];

    let totalChars = lines.join('\n').length;

    for (const skill of limited) {
        const emoji = skill.metadata.emoji || '📦';
        const header = `### ${emoji} ${skill.name}`;
        const desc = `> ${skill.description}`;
        const section = `${header}\n${desc}\n\n${skill.body}\n\n---\n`;

        if (totalChars + section.length > MAX_CHARS) {
            lines.push(`\n⚠️ 技能提示已截断（已包含 ${lines.length} 个技能，总字符数达上限 ${MAX_CHARS}）`);
            break;
        }

        lines.push(section);
        totalChars += section.length;
    }

    return lines.join('\n');
}
