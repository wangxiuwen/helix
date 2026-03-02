/**
 * Auto-generated Skill: folder_scanner_pdf
 * 扫描指定文件夹，分析其结构、文件类型、大小等信息，并生成PDF格式的详细报告
 * Created at: 2026-02-26T14:54:28.059Z
 */

export const definition = {
    type: "function",
    function: {
        name: "folder_scanner_pdf",
        description: "扫描指定文件夹，分析其结构、文件类型、大小等信息，并生成PDF格式的详细报告",
        parameters: {
            type: "object",
            properties: {
          "dirPath": {
                    "type": "string",
                    "description": "要扫描的文件夹路径，默认为当前目录"
          },
          "outputPath": {
                    "type": "string",
                    "description": "PDF报告的输出路径，默认为report.pdf"
          }
},
            required: []
        }
    }
};

export async function handler(args) {
    
const fs = require('fs');
const path = require('path');

function scanDirectory(dirPath, basePath = dirPath) {
  const result = {
    name: path.basename(dirPath),
    path: dirPath,
    files: [],
    directories: [],
    totalSize: 0,
    fileCount: 0,
    dirCount: 0
  };

  try {
    const items = fs.readdirSync(dirPath);
    
    for (const item of items) {
      if (item.startsWith('.') && item !== '.') continue; // 跳过隐藏文件
      
      const fullPath = path.join(dirPath, item);
      const stat = fs.statSync(fullPath);
      
      if (stat.isDirectory()) {
        const subDir = scanDirectory(fullPath, basePath);
        result.directories.push(subDir);
        result.totalSize += subDir.totalSize;
        result.fileCount += subDir.fileCount;
        result.dirCount += subDir.dirCount + 1;
      } else {
        const ext = path.extname(item).toLowerCase();
        result.files.push({
          name: item,
          size: stat.size,
          modified: stat.mtime,
          extension: ext || '无扩展名'
        });
        result.totalSize += stat.size;
        result.fileCount++;
      }
    }
  } catch (error) {
    result.error = error.message;
  }

  return result;
}

function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function generateHTMLReport(data, title) {
  const now = new Date().toLocaleString('zh-CN');
  
  function renderTree(node, level = 0) {
    let html = '';
    const indent = '  '.repeat(level);
    
    // 渲染文件
    for (const file of node.files) {
      html += `${indent}<div class="file">
        <span class="icon">📄</span>
        <span class="name">${file.name}</span>
        <span class="size">${formatBytes(file.size)}</span>
      </div>\n`;
    }
    
    // 渲染子目录
    for (const dir of node.directories) {
      html += `${indent}<div class="directory">
        <div class="dir-header">
          <span class="icon">📁</span>
          <span class="name">${dir.name}/</span>
          <span class="info">(${dir.fileCount} 个文件, ${formatBytes(dir.totalSize)})</span>
        </div>
        <div class="dir-content">
          ${renderTree(dir, level + 1)}
        </div>
      </div>\n`;
    }
    
    return html;
  }
  
  function getFileTypeStats(node, stats = {}) {
    for (const file of node.files) {
      const ext = file.extension;
      if (!stats[ext]) {
        stats[ext] = { count: 0, size: 0 };
      }
      stats[ext].count++;
      stats[ext].size += file.size;
    }
    for (const dir of node.directories) {
      getFileTypeStats(dir, stats);
    }
    return stats;
  }
  
  const fileTypeStats = getFileTypeStats(data);
  const sortedTypes = Object.entries(fileTypeStats)
    .sort((a, b) => b[1].size - a[1].size)
    .slice(0, 10);
  
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <title>${title}</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      line-height: 1.6;
      color: #333;
      max-width: 1200px;
      margin: 0 auto;
      padding: 40px;
      background: #f5f5f5;
    }
    .container {
      background: white;
      padding: 40px;
      border-radius: 8px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.1);
    }
    h1 {
      color: #2c3e50;
      border-bottom: 3px solid #3498db;
      padding-bottom: 15px;
      margin-bottom: 30px;
    }
    h2 {
      color: #34495e;
      margin: 30px 0 15px 0;
      font-size: 1.4em;
    }
    .summary {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 20px;
      margin-bottom: 30px;
    }
    .stat-card {
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      color: white;
      padding: 20px;
      border-radius: 8px;
      text-align: center;
    }
    .stat-card h3 {
      font-size: 0.9em;
      opacity: 0.9;
      margin-bottom: 10px;
    }
    .stat-card .value {
      font-size: 2em;
      font-weight: bold;
    }
    .file-types {
      margin: 20px 0;
    }
    .file-type-item {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 12px 15px;
      background: #f8f9fa;
      margin: 8px 0;
      border-radius: 6px;
      border-left: 4px solid #3498db;
    }
    .tree-view {
      margin-top: 20px;
      font-family: 'Consolas', monospace;
      font-size: 14px;
    }
    .directory {
      margin: 5px 0;
    }
    .dir-header {
      padding: 8px 12px;
      background: #ecf0f1;
      border-radius: 4px;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .dir-header:hover {
      background: #d5dbdb;
    }
    .file {
      padding: 5px 12px 5px 40px;
      display: flex;
      align-items: center;
      gap: 10px;
      border-bottom: 1px solid #ecf0f1;
    }
    .file:hover {
      background: #f8f9fa;
    }
    .icon { font-size: 16px; }
    .name { flex: 1; }
    .size { 
      color: #7f8c8d;
      font-size: 0.9em;
    }
    .info {
      color: #7f8c8d;
      font-size: 0.85em;
    }
    .footer {
      margin-top: 40px;
      padding-top: 20px;
      border-top: 1px solid #ecf0f1;
      color: #95a5a6;
      font-size: 0.9em;
      text-align: center;
    }
    @media print {
      body { background: white; padding: 20px; }
      .container { box-shadow: none; padding: 0; }
    }
  </style>
</head>
<body>
  <div class="container">
    <h1>📂 ${title}</h1>
    
    <div class="summary">
      <div class="stat-card">
        <h3>总文件数</h3>
        <div class="value">${data.fileCount.toLocaleString()}</div>
      </div>
      <div class="stat-card">
        <h3>总文件夹数</h3>
        <div class="value">${data.dirCount.toLocaleString()}</div>
      </div>
      <div class="stat-card">
        <h3>总大小</h3>
        <div class="value">${formatBytes(data.totalSize)}</div>
      </div>
    </div>
    
    <h2>📊 文件类型统计 (Top 10)</h2>
    <div class="file-types">
      ${sortedTypes.map(([ext, info]) => `
        <div class="file-type-item">
          <span><strong>${ext === '无扩展名' ? '(无扩展名)' : ext}</strong> - ${info.count} 个文件</span>
          <span>${formatBytes(info.size)}</span>
        </div>
      `).join('')}
    </div>
    
    <h2>🌲 目录结构</h2>
    <div class="tree-view">
      <div class="dir-header">
        <span class="icon">📁</span>
        <span class="name"><strong>${data.name}/</strong></span>
        <span class="info">(${data.fileCount} 个文件, ${data.dirCount} 个文件夹, ${formatBytes(data.totalSize)})</span>
      </div>
      <div class="dir-content" style="margin-left: 20px;">
        ${renderTree(data)}
      </div>
    </div>
    
    <div class="footer">
      <p>报告生成时间: ${now}</p>
      <p>扫描路径: ${data.path}</p>
    </div>
  </div>
</body>
</html>`;
}

// 主函数
const dirPath = args.dirPath || '.';
const outputPath = args.outputPath || 'folder_report.html';

console.log(`🔍 正在扫描文件夹: ${path.resolve(dirPath)}`);

const scanData = scanDirectory(dirPath);
const htmlContent = generateHTMLReport(scanData, '文件夹扫描报告');

fs.writeFileSync(outputPath, htmlContent, 'utf8');

return {
  success: true,
  message: `✅ 报告已生成: ${path.resolve(outputPath)}`,
  summary: {
    totalFiles: scanData.fileCount,
    totalDirs: scanData.dirCount,
    totalSize: formatBytes(scanData.totalSize),
    outputFile: path.resolve(outputPath)
  }
};

}
