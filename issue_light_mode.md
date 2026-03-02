## 问题描述

AI 对话页面的代码块在白天（Light）模式下，文字颜色与背景色过于接近，几乎无法阅读。

## 复现步骤

1. 在设置中切换到白天模式（Light Theme）
2. 在 AI 对话中让 AI 返回包含代码块的回复
3. 代码块中的文字几乎不可见（浅灰色文字在浅色背景上）

## 截图

代码块区域箭头所指位置的文字几乎看不清：

> 代码块背景为浅灰色，文字也是浅色，对比度极低

## 建议修复

为 Light 模式单独设置代码块样式，确保文字与背景有足够对比度。例如：

```css
/* Light mode code block fix */
[data-theme="light"] pre code,
.light pre code {
    color: #1f2937;  /* dark text */
    background-color: #f3f4f6;  /* light gray bg */
}
```

## 环境

- OS: Windows
- Helix: v0.3.0
- 主题: Light Mode
