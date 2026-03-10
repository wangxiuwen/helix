const testCases = [
  "/memo save 这是一个非常长非常长的中文测试内容，用于验证在应用安全截断机制后，系统保存和显示备忘录列表时不会因为多字节字符被从中间被截断而发生Rust panic崩溃。",
  "/memo list",
  "/search 2026年全球人工智能技术发展大趋势",
  "/link https://www.rust-lang.org/zh-CN/",
  "给我写一篇关于Rust语言所有权机制和生命周期的简短中文介绍，包含代码示例，测试你的截断功能"
];

async function run() {
  for (let i = 0; i < testCases.length; i++) {
    console.log(\n--- [Test Case ] ---);
    console.log(Command: );
    try {
      const res = await fetch('http://localhost:9520/api/agent/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: testCases[i], account_id: 'test_truncation_123' })
      });
      const text = await res.text();
      try {
        const json = JSON.parse(text);
        console.log('Result: SUCCESS (No Crash)');
        console.log(Reply: ...);
      } catch(e) {
        console.log('JSON Parse error, Raw Body:', text.substring(0, 100));
      }
    } catch (e) {
      console.error(FAILED / CRASHED: );
    }
  }
}
run();
