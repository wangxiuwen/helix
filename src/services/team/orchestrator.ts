import { invoke } from '@tauri-apps/api/core';
import { ROLES, getRole } from './roles';
import { LLMProvider } from './llm';

export class TeamOrchestrator {
    private llm = new LLMProvider();
    private history: any[] = [];

    async handleRequest(topic: string, workspaceDir: string, onEvent: (evt: any) => void, mentionedRoles?: Array<{ role: string, name: string, systemPrompt: string }>, isImplicitBroadcast?: boolean) {
        onEvent({ type: 'team_start', data: topic });

        let initialDiscussionSummary = "";

        // If specific members were @mentioned (or all members in group chat), each responds
        if (mentionedRoles && mentionedRoles.length > 0) {
            let discussionContext = `用户说: ${topic}`;
            for (const mr of mentionedRoles) {
                const roleId = Object.entries(ROLES).find(([_, r]) => r.name === mr.name)?.[0] || 'developer';
                onEvent({ type: 'progress', data: { role: roleId, name: mr.name, action: `${mr.name} 正在思考...` } });
                try {
                    let output = '';
                    if (isImplicitBroadcast) {
                        const sysPrompt = mr.systemPrompt || `你是团队中的【${mr.name}】(${mr.role})。
请针对当前用户的需求或当前的讨论进度，直接发表你的专业观点、建议或反驳。
要求：
1. 简洁有力，不废话，不要客套。
2. 密切关注前面几位成员的发言，如果有不同意见或需要补充，请直接指出。
3. 如果大家已经达成一致，请确认你的职责部分。`;
                        const res = await this.llm.chat([
                            { role: 'system', content: sysPrompt + (workspaceDir ? `\n\n相关目录: ${workspaceDir}` : '') },
                            { role: 'user', content: discussionContext }
                        ]);
                        output = res.content || '';
                    } else {
                        const subResult: any = await invoke('spawn_subagent', {
                            task: discussionContext + (workspaceDir ? `\n\nIMPORTANT: Use this directory for ALL file outputs (create it if needed): ${workspaceDir}` : ''),
                            systemPrompt: mr.systemPrompt || `你是团队中的【${mr.name}】(${mr.role})。完成用户的任务。`,
                            maxRounds: 5
                        });
                        output = subResult.output;
                    }

                    onEvent({ type: 'result', data: { role: roleId, name: mr.name, content: output } });
                    // Accumulate context so next member sees previous responses
                    discussionContext += `\n\n[${mr.name}]: ${output}`;
                    initialDiscussionSummary += `\n\n[${mr.name}]: ${output}`;
                } catch (err: any) {
                    onEvent({ type: 'result', data: { role: roleId, name: mr.name, content: `执行失败: ${err.message || err}` } });
                }
            }

            // If it's a specific @mention (not implicit), we end here.
            // But for general group chat, we CONTINUE to the PM coordination loop.
            if (!isImplicitBroadcast) {
                onEvent({ type: 'team_done' });
                return;
            }
        }

        let rounds = 0;
        this.history.push({ role: 'user', content: topic });

        // If we had an initial broadcast discussion, tell the PM about it
        if (initialDiscussionSummary) {
            this.history.push({
                role: 'assistant',
                content: `大家已经完成了初步表态：${initialDiscussionSummary}\n\n作为 PM，我将根据以上讨论引导后续行动或进一步探讨。`
            });
        }

        let workspaceContext = workspaceDir
            ? `\n\n# Workspace Directory\nALL files MUST be written EXACTLY to this directory (create it if not exists): ${workspaceDir}`
            : '';

        try {
            const agContext = await invoke<string>('get_antigravity_context', { workspace: workspaceDir || null });
            if (agContext) {
                workspaceContext += `\n\n# Persistent Context\n${agContext}`;
            }
        } catch (e) {
            console.error('Failed to load antigravity context', e);
        }

        while (rounds < 30) {
            rounds++;
            onEvent({ type: 'progress', data: { role: 'pm', name: ROLES.pm.name, action: `PM 思考中 (Round ${rounds})...` } });

            let msg;
            try {
                msg = await this.llm.chat([
                    { role: 'system', content: ROLES.pm.systemPrompt + workspaceContext },
                    ...this.history
                ], [
                    {
                        type: 'function',
                        function: {
                            name: 'group_discuss',
                            description: 'Initiate a group discussion among team members.',
                            parameters: {
                                type: 'object',
                                properties: {
                                    topic: { type: 'string' },
                                    participants: {
                                        type: 'array',
                                        items: { type: 'string', enum: ['product', 'architect', 'developer', 'tester', 'teaching'] }
                                    }
                                },
                                required: ['topic', 'participants']
                            }
                        }
                    },
                    {
                        type: 'function',
                        function: {
                            name: 'delegate_to',
                            description: 'Delegate a specific task to a specialist team member.',
                            parameters: {
                                type: 'object',
                                properties: {
                                    role: { type: 'string', enum: ['product', 'architect', 'developer', 'tester', 'teaching'] },
                                    task: { type: 'string' }
                                },
                                required: ['role', 'task']
                            }
                        }
                    }
                ]);
            } catch (err: any) {
                onEvent({ type: 'error', data: `PM Error: ${err.message}` });
                break;
            }

            if (msg.tool_calls) {
                this.history.push(msg); // push assistant message with tool calls

                for (const tc of msg.tool_calls) {
                    let args;
                    try {
                        args = JSON.parse(tc.function.arguments);
                    } catch (e) {
                        args = {};
                    }

                    let resultStr = "";
                    if (tc.function.name === 'group_discuss') {
                        onEvent({ type: 'group_start', data: { topic: args.topic } });
                        resultStr = await this.runGroupDiscuss(args.topic, args.participants || [], onEvent);
                    } else if (tc.function.name === 'delegate_to') {
                        const roleDef = getRole(args.role);
                        onEvent({ type: 'progress', data: { role: args.role, name: roleDef?.name, action: `正在执行任务... (${args.task})` } });

                        try {
                            const subResult: any = await invoke('spawn_subagent', {
                                task: args.task + (workspaceDir ? `\n\nIMPORTANT: Use this directory for ALL file outputs (create it if needed): ${workspaceDir}` : ''),
                                systemPrompt: roleDef?.systemPrompt || `You are ${args.role}`,
                                maxRounds: 30
                            });
                            resultStr = subResult.output;
                            onEvent({ type: 'result', data: { role: args.role, name: roleDef?.name, content: resultStr } });
                        } catch (subErr: any) {
                            resultStr = `执行失败: ${subErr}`;
                            onEvent({ type: 'result', data: { role: args.role, name: roleDef?.name, content: resultStr } });
                            onEvent({ type: 'error', data: `[${roleDef?.name}] Error: ${subErr}` });
                        }
                    } else {
                        resultStr = "Unknown tool";
                    }

                    this.history.push({
                        role: 'tool',
                        tool_call_id: tc.id,
                        name: tc.function.name,
                        content: resultStr
                    });
                }
            } else {
                onEvent({ type: 'result', data: { role: 'pm', name: ROLES.pm.name, content: msg.content } });
                break;
            }
        }
        onEvent({ type: 'team_done' });
    }

    private async runGroupDiscuss(topic: string, participants: string[], onEvent: any): Promise<string> {
        let summary = `【讨论主题】: ${topic}\n`;
        for (const roleId of participants) {
            const role = getRole(roleId);
            if (!role) continue;

            onEvent({ type: 'progress', data: { role: roleId, name: role.name, action: `发表观点中...` } });

            try {
                const pMsg = await this.llm.chat([
                    { role: 'system', content: `你是团队中的【${role.name}】。根据你的角色利益直白地发表观点。避免长篇大论，口语化直接反驳不合理的地方。` },
                    { role: 'user', content: summary }
                ]);
                const text = pMsg.content || '';
                summary += `\n[${role.name}]: ${text}`;
                onEvent({ type: 'result', data: { role: roleId, name: role.name, content: text } });
            } catch (err: any) {
                const text = `⚠️ (连接异常，无法发表观点): ${err.message || String(err)}`;
                summary += `\n[${role.name}]: ${text}`;
                onEvent({ type: 'result', data: { role: roleId, name: role.name, content: text } });
            }
        }
        return summary;
    }
}
