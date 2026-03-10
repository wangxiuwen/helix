import { invoke } from '@tauri-apps/api/core';
// roles.ts still available if needed for backward compat
import { LLMProvider } from './llm';
import { maskToolOutputs, checkOverflow, compressChat } from './contextManager';

export interface SessionMember {
    id: string;
    name: string;
    role: string;
    systemPrompt: string;
    icon?: string;
    avatar?: string;
}

const BUTLER_SYSTEM_PROMPT = `你是 **Helix 大管家**，一个全能的 AI 团队协调者。

# 你的角色
你是团队的核心协调人。你的职责是：接收用户需求，协调团队成员完成任务。

# 工作流程

## 1: 需求分析
收到新需求后，先分析需求，确定需要哪些成员参与。
如果需求复杂，可以 CALL **group_discuss** 发起团队讨论。

## 2: 任务分配
分析完需求后，使用 **delegate_to** 将具体任务分配给合适的成员。
等待结果，跟踪进度。

## 3: 结果交付
所有任务完成后，输出最终总结报告。

# 工具
- **group_discuss**: 发起团队讨论，让相关成员发表观点。
- **delegate_to**: 将具体任务委派给团队成员执行。

# 规则
- 所有回复使用中文。
- 简洁高效，注重交付。
- 根据成员的专长合理分配任务。`;

export class TeamOrchestrator {
    private llm = new LLMProvider();
    private history: any[] = [];

    async handleRequest(
        topic: string,
        workspaceDir: string,
        onEvent: (evt: any) => void,
        sessionMembers: SessionMember[],
        mentionedRoles?: Array<{ role: string, name: string, systemPrompt: string }>,
        isImplicitBroadcast?: boolean
    ) {
        onEvent({ type: 'team_start', data: topic });

        let initialDiscussionSummary = "";

        // Build dynamic member list for tools
        const memberNameMap = Object.fromEntries(sessionMembers.map(m => [m.id, m]));

        // If specific members were @mentioned (or all members in group chat), each responds
        if (mentionedRoles && mentionedRoles.length > 0) {
            let discussionContext = `用户说: ${topic}`;
            for (const mr of mentionedRoles) {
                const member = sessionMembers.find(m => m.name === mr.name);
                const roleId = member?.id || 'assistant';
                onEvent({ type: 'progress', data: { role: roleId, name: mr.name, action: `${mr.name} 正在思考...` } });
                try {
                    let output = '';
                    if (isImplicitBroadcast) {
                        const sysPrompt = mr.systemPrompt || `你是团队中的【${mr.name}】(${mr.role})。
请针对当前用户的需求或当前的讨论进度，直接发表你的专业观点、建议或反驳。
要求：
1. 简洁有力，不废话，不要客套。
2. 密切关注前面几位成员的发言，如果有不同意见或需要补充，请直接指出。
3. 如果大家已经达成一致，请确认你的职责部分。
4. 如果用户的发言是闲聊、玩笑或与已有项目无关的脑筋急转弯（如：大象装冰箱），请以你的角色性格自然、幽默地回应，切勿凭空捏造虚假的项目方案（如A方案/单体架构等废话）。

## 讨论规则
- 如果你同意前面某位成员的观点，请简要说"同意XX的观点"并补充你的专业角度
- 如果你不同意，请给出具体理由和替代方案，不要含糊其辞
- 每次发言结尾，用一句话总结你的核心观点和建议的行动项
- 主动向其他角色提问，推动讨论深入（如："架构师觉得这个方案的性能瓶颈在哪？"）`;
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
                    discussionContext += `\n\n[${mr.name}]: ${output}`;
                    initialDiscussionSummary += `\n\n[${mr.name}]: ${output}`;
                } catch (err: any) {
                    onEvent({ type: 'result', data: { role: roleId, name: mr.name, content: `执行失败: ${err.message || err}` } });
                }
            }

            if (!isImplicitBroadcast) {
                onEvent({ type: 'team_done' });
                return;
            }
        }

        let rounds = 0;
        this.history.push({ role: 'user', content: topic });

        if (initialDiscussionSummary) {
            this.history.push({
                role: 'assistant',
                content: `大家已经完成了初步表态：${initialDiscussionSummary}\n\n作为大管家，我将根据以上讨论引导后续行动或进一步探讨。`
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

        // Build dynamic tool definitions based on actual session members (excluding butler)
        const delegatableMembers = sessionMembers.filter(m => m.id !== 'c-butler');
        const memberEnumIds = delegatableMembers.map(m => m.id);
        const memberDescriptions = delegatableMembers.map(m => `${m.id}: ${m.name} (${m.role})`).join(', ');

        const butlerPrompt = BUTLER_SYSTEM_PROMPT + `\n\n# 当前团队成员\n${delegatableMembers.map(m => `- **${m.name}** (${m.role}): ${m.systemPrompt || '无特殊说明'}`).join('\n')}`;

        const tools: any[] = [];
        if (memberEnumIds.length > 0) {
            tools.push(
                {
                    type: 'function',
                    function: {
                        name: 'group_discuss',
                        description: `Initiate a group discussion among team members. Available: ${memberDescriptions}`,
                        parameters: {
                            type: 'object',
                            properties: {
                                topic: { type: 'string' },
                                participants: {
                                    type: 'array',
                                    items: { type: 'string', enum: memberEnumIds }
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
                        description: `Delegate a specific task to a team member. Available: ${memberDescriptions}`,
                        parameters: {
                            type: 'object',
                            properties: {
                                member_id: { type: 'string', enum: memberEnumIds, description: 'ID of the member to delegate to' },
                                task: { type: 'string' }
                            },
                            required: ['member_id', 'task']
                        }
                    }
                }
            );
        }

        while (rounds < 30) {
            rounds++;

            this.history = maskToolOutputs(this.history);

            if (rounds > 1 && rounds % 5 === 0) {
                const compResult = await compressChat(this.history, this.llm);
                if (compResult.compressed) {
                    this.history = compResult.messages;
                    onEvent({ type: 'progress', data: { role: 'c-butler', name: 'Helix 大管家', action: `[系统] 对话已成功压缩，节省上下文空间` } });
                }
            }

            const overflowCheck = checkOverflow(this.history, butlerPrompt + workspaceContext);
            onEvent({ type: 'loop_info', data: `[Loop ${rounds}] Messages: ${this.history.length} | Tokens: ~${overflowCheck.totalTokens} | Context: ${overflowCheck.usagePercent}%` });

            if (!overflowCheck.safe) {
                onEvent({ type: 'progress', data: { role: 'c-butler', name: '系统警报', action: `🚨 上下文已满 (${overflowCheck.usagePercent}%)。正在紧急阻断，请开启新对话...` } });
                this.history = this.history.slice(Math.floor(this.history.length * 0.7));
                this.history.unshift({ role: 'assistant', content: '[Earlier context was truncated due to context window overflow]' });
            }

            onEvent({ type: 'progress', data: { role: 'c-butler', name: 'Helix 大管家', action: `大管家思考中 (Round ${rounds})...` } });

            let msg;
            try {
                msg = await this.llm.chat([
                    { role: 'system', content: butlerPrompt + workspaceContext },
                    ...this.history
                ], tools.length > 0 ? tools : undefined);
            } catch (err: any) {
                onEvent({ type: 'error', data: `大管家 Error: ${err.message}` });
                break;
            }

            if (msg.tool_calls) {
                this.history.push(msg);

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
                        resultStr = await this.runGroupDiscuss(args.topic, args.participants || [], sessionMembers, onEvent);
                    } else if (tc.function.name === 'delegate_to') {
                        const memberId = args.member_id || args.role; // backward compat
                        const member = memberNameMap[memberId] || sessionMembers.find(m => m.name === memberId);
                        const memberName = member?.name || memberId;
                        const memberRole = member?.role || 'assistant';
                        onEvent({ type: 'progress', data: { role: memberId, name: memberName, action: `正在执行任务... (${args.task})` } });

                        try {
                            const subResult: any = await invoke('spawn_subagent', {
                                task: args.task + (workspaceDir ? `\n\nIMPORTANT: Use this directory for ALL file outputs (create it if needed): ${workspaceDir}` : ''),
                                systemPrompt: member?.systemPrompt || `你是${memberName}（${memberRole}），完成用户的任务。`,
                                maxRounds: 10
                            });
                            resultStr = subResult.output;
                            onEvent({ type: 'result', data: { role: memberId, name: memberName, content: resultStr } });
                        } catch (subErr: any) {
                            resultStr = `执行失败: ${subErr}`;
                            onEvent({ type: 'result', data: { role: memberId, name: memberName, content: resultStr } });
                            onEvent({ type: 'error', data: `[${memberName}] Error: ${subErr}` });
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
                onEvent({ type: 'result', data: { role: 'c-butler', name: 'Helix 大管家', content: msg.content } });
                break;
            }
        }
        onEvent({ type: 'team_done' });
    }

    private async runGroupDiscuss(topic: string, participantIds: string[], sessionMembers: SessionMember[], onEvent: any): Promise<string> {
        let summary = `【讨论主题】: ${topic}\n`;
        for (const memberId of participantIds) {
            const member = sessionMembers.find(m => m.id === memberId);
            if (!member) continue;

            onEvent({ type: 'progress', data: { role: memberId, name: member.name, action: `发表观点中...` } });

            try {
                const pMsg = await this.llm.chat([
                    { role: 'system', content: `你是团队中的【${member.name}】(${member.role})。根据你的角色利益直白地发表观点。避免长篇大论，口语化直接反驳不合理的地方。` },
                    { role: 'user', content: summary }
                ]);
                const text = pMsg.content || '';
                summary += `\n[${member.name}]: ${text}`;
                onEvent({ type: 'result', data: { role: memberId, name: member.name, content: text } });
            } catch (err: any) {
                const text = `⚠️ (连接异常，无法发表观点): ${err.message || String(err)}`;
                summary += `\n[${member.name}]: ${text}`;
                onEvent({ type: 'result', data: { role: memberId, name: member.name, content: text } });
            }
        }
        return summary;
    }
}
