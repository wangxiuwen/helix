const dgram = require('dgram');
const http = require('http');

const MULTICAST_IP = '224.0.0.167';
const PORT = 53317;
const HTTP_PORT = 8888; // Ensure uniqueness

const payload = JSON.stringify({
    alias: "虚拟网络小助手",
    version: "2.0",
    deviceModel: "Mock Desktop",
    deviceType: "desktop",
    fingerprint: "mock-" + Date.now(),
    port: HTTP_PORT,
    protocol: "http",
    download: true,
    announce: true,
    announcement: true
});

const client = dgram.createSocket('udp4');

// Send initial multicast and broadcast
client.bind(() => {
    client.setBroadcast(true);
    client.setMulticastTTL(128);
    console.log(`[LAN Mock] Started broadcaster...`);
    setInterval(() => {
        const buf = Buffer.from(payload);
        client.send(buf, 0, buf.length, PORT, MULTICAST_IP, (err) => {
            if (err) console.error("Multicast err:", err);
        });
        client.send(buf, 0, buf.length, PORT, '255.255.255.255', (err) => {
            // Ignore broadcast EACCES errors if any
        });
    }, 3000);
});

// Setup a small HTTP server to receive LAN messages representing the mock client UI
const server = http.createServer((req, res) => {
    if (req.method === 'POST') {
        let body = '';
        req.on('data', chunk => body += chunk.toString());
        req.on('end', () => {
            console.log(`\n======================================================`);
            console.log(`[LAN Mock HTTP] Incoming POST from Helix: ${req.url}`);
            try {
                const json = JSON.parse(body);
                console.log(`[LAN Mock HTTP] Message details:`);
                console.log(`  Session ID: ${json.session_id}`);
                console.log(`  Sender: ${json.name} (${json.role})`);
                console.log(`  Content: ${json.content}`);

                // We could even simulate a reply!
                setTimeout(() => {
                    sendReply(json.session_id, json.content);
                }, 2000);
            } catch (e) {
                console.log(body);
            }
            console.log(`======================================================\n`);
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ success: true }));
        });
    } else {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({}));
    }
});

function sendReply(sessionId, userMessage) {
    console.log(`[LAN Mock HTTP] Simulating reply to session ${sessionId}...`);

    let replyText = "收到你的消息啦！我是模拟的局域网客户端回复～";
    if (userMessage.includes("你好") || userMessage.includes("hello")) {
        replyText = "你好！很高兴认识你，我现在是通过局域网跟你聊天哦！";
    } else if (userMessage.includes("测试") || userMessage.includes("test")) {
        replyText = "局域网通信测试成功！收发全部正常 ✅";
    } else if (userMessage.includes("文件")) {
        replyText = "发送文件功能听说还在开发中，我现在只能收发文本消息呢。";
    } else if (userMessage.includes("谁")) {
        replyText = "我是由 Node.js 脚本模拟出来的一个局域网虚拟用户呀！";
    } else {
        const responses = [
            "哈哈，有意思！",
            "原来如此~",
            "我在同一局域网下听得清清楚楚。",
            "局域网聊天速度就是快！完全没有延迟感。",
            `你刚才说 "${userMessage}" 是吗？我觉得挺好的！`
        ];
        replyText = responses[Math.floor(Math.random() * responses.length)];
    }

    const options = {
        hostname: '127.0.0.1',
        port: 53317,
        path: '/api/helix/v1/message',
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        }
    };

    const req = http.request(options, (res) => {
        res.on('data', () => { });
    });

    req.on('error', e => { });
    req.write(JSON.stringify({
        session_id: sessionId,
        role: "assistant", // Using assistant role prevents "user" right alignment in some UIs
        name: "虚拟网络小助手",
        content: replyText,
        reply_to: null
    }));
    req.end();
}

server.listen(HTTP_PORT, '0.0.0.0', () => {
    console.log(`[LAN Mock HTTP] Listening on port ${HTTP_PORT} to receive Helix chats...`);
});
