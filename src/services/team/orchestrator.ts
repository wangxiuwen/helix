import { invoke } from '@tauri-apps/api/core';
import { ROLES, getRole } from './roles';
import { LLMProvider } from './llm';

export class TeamOrchestrator {
    private llm = new LLMProvider();
    private history: any[] = [];

    async handleRequest(topic: string, workspaceDir: string, onEvent: (evt: any) => void) {
        onEvent({ type: 'team_start', data: topic });

        let rounds = 0;
        this.history.push({ role: 'user', content: topic });

        const workspaceContext = workspaceDir
            ? `\n\n# Workspace Directory\nALL files MUST be written EXACTLY to this directory (create it if not exists): ${workspaceDir}`
            : '';

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
