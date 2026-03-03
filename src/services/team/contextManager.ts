import { LLMProvider } from './llm';

const CHARS_PER_TOKEN = 3;

// Layer 1
const TOOL_PROTECTION_THRESHOLD = 30000;
const MIN_PRUNABLE_THRESHOLD = 15000;
const MAX_TOOL_RESULT_CHARS = 8000;
const PREVIEW_HEAD_CHARS = 500;
const PREVIEW_TAIL_CHARS = 500;

// Layer 2
const COMPRESSION_TOKEN_THRESHOLD = 0.45;
const COMPRESSION_PRESERVE_RATIO = 0.35;

// Layer 3
const OVERFLOW_SAFETY_MARGIN = 0.85;

export function estimateTokens(text: string): number {
    if (!text) return 0;
    return Math.ceil(text.length / CHARS_PER_TOKEN);
}

export function estimateMessageTokens(msg: any): number {
    let text = msg.content || '';
    if (msg.tool_calls) {
        text += JSON.stringify(msg.tool_calls);
    }
    return estimateTokens(text);
}

export function estimateMessagesTokens(messages: any[]): number {
    return messages.reduce((acc, msg) => acc + estimateMessageTokens(msg), 0);
}

// ----------------------------------------------------------------------------
// Layer 1: Tool Output Masking
// ----------------------------------------------------------------------------
export function maskToolOutputs(messages: any[]): any[] {
    let accumulatedToolTokens = 0;
    let prunableTokens = 0;
    const itemsToPrune: any[] = [];
    const newMessages = [...messages];

    // Reverse scan to protect recent tool outputs
    for (let i = newMessages.length - 1; i >= 0; i--) {
        const msg = newMessages[i];
        if (msg.role === 'tool') {
            const contentStr = typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content);
            const tokens = estimateTokens(contentStr);

            if (accumulatedToolTokens < TOOL_PROTECTION_THRESHOLD) {
                // Inside protection window
                accumulatedToolTokens += tokens;
            } else {
                // Outside protection window
                if (contentStr.length > MAX_TOOL_RESULT_CHARS) {
                    prunableTokens += tokens;
                    itemsToPrune.push({ index: i, originalContent: contentStr });
                }
            }
        }
    }

    // Only execute if batch threshold is met
    if (prunableTokens >= MIN_PRUNABLE_THRESHOLD) {
        for (const item of itemsToPrune) {
            const originalContent: string = item.originalContent;
            const head = originalContent.substring(0, PREVIEW_HEAD_CHARS);
            const tail = originalContent.substring(originalContent.length - PREVIEW_TAIL_CHARS);

            const omittedLines = originalContent.substring(PREVIEW_HEAD_CHARS, originalContent.length - PREVIEW_TAIL_CHARS).split('\n').length;
            const approxKB = Math.round(originalContent.length / 1024);

            newMessages[item.index].content = `[Tool output truncated — original: ~${omittedLines} lines, ~${approxKB}KB]\n${head}\n...\n[${omittedLines} lines omitted]\n...\n${tail}`;
        }
        console.log(`[ContextManager] Masked ${itemsToPrune.length} tool outputs, saved ~${prunableTokens} tokens.`);
    }

    return newMessages;
}

// ----------------------------------------------------------------------------
// Layer 2: Chat Compression
// ----------------------------------------------------------------------------
export async function compressChat(messages: any[], llm: LLMProvider, contextLimit: number = 131072): Promise<{ compressed: boolean, messages: any[] }> {
    const currentTokens = estimateMessagesTokens(messages);
    const threshold = contextLimit * COMPRESSION_TOKEN_THRESHOLD;

    if (currentTokens <= threshold) {
        return { compressed: false, messages };
    }

    // Attempt compression
    // Split messages at approx 65% / 35% ratio
    const preserveRatioTokens = currentTokens * COMPRESSION_PRESERVE_RATIO;

    let splitIndex = messages.length - 1;
    let accumulatedFromEnd = 0;

    // Find the split point going backwards to ensure we preserve the latest chunks
    while (splitIndex >= 0 && accumulatedFromEnd < preserveRatioTokens) {
        accumulatedFromEnd += estimateMessageTokens(messages[splitIndex]);
        splitIndex--;
    }

    // Ensure split index lands on a 'user' message to not break tool_call -> tool_result pairs
    while (splitIndex >= 0 && messages[splitIndex].role !== 'user') {
        splitIndex--;
    }

    if (splitIndex <= 0) {
        // Cannot split safely
        return { compressed: false, messages };
    }

    const messagesToCompress = messages.slice(0, splitIndex);
    const messagesToPreserve = messages.slice(splitIndex);

    const prompt = `You are a context compression assistant. Summarize the following dialogue history.
Rules:
1. Retain all file paths, function names, variable names, error messages, tool results, and technical decisions.
2. Retain user preferences, constraints, and explicit requirements.
3. Record which tools were used and key findings.
4. Track the current state of ongoing tasks.
5. Be concise but preserve executable context.
6. Output in the language of the conversation.
7. Wrap your output in <state_snapshot> tags.

History to compress:
${JSON.stringify(messagesToCompress, null, 2)}`;

    try {
        const response = await llm.chat([{ role: 'user', content: prompt }]);
        const snapshot = response.content || '';

        const newMessages = [
            {
                role: 'user',
                content: `[Context Snapshot — compressed from ${messagesToCompress.length} messages]\n\n${snapshot}`
            },
            {
                role: 'assistant',
                content: '明白，我已掌握之前的上下文。请继续。'
            },
            ...messagesToPreserve
        ];

        const afterTokens = estimateMessagesTokens(newMessages);

        // Deflation check
        if (afterTokens >= currentTokens) {
            console.warn('[ContextManager] Compression inflated context, reverting.');
            return { compressed: false, messages };
        }

        console.log(`[ContextManager] Compressed History: ${currentTokens} -> ${afterTokens} tokens.`);
        return { compressed: true, messages: newMessages };
    } catch (e) {
        console.error('[ContextManager] Compression failed', e);
        return { compressed: false, messages };
    }
}

// ----------------------------------------------------------------------------
// Layer 3: Overflow Prevention
// ----------------------------------------------------------------------------
export function checkOverflow(messages: any[], systemPrompt: string, contextLimit: number = 131072): { safe: boolean, totalTokens: number, limit: number, usagePercent: number } {
    const sysTokens = estimateTokens(systemPrompt);
    const msgTokens = estimateMessagesTokens(messages);
    const totalTokens = sysTokens + msgTokens;

    const hardLimit = contextLimit * OVERFLOW_SAFETY_MARGIN;
    const usagePercent = Math.round((totalTokens / contextLimit) * 100);

    return {
        safe: totalTokens < hardLimit,
        totalTokens,
        limit: contextLimit,
        usagePercent
    };
}
