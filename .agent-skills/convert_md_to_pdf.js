/**
 * Auto-generated Skill: convert_md_to_pdf
 * 将Markdown文件转换为PDF格式
 * Created at: 2026-02-26T14:25:54.811Z
 */

export const definition = {
    type: "function",
    function: {
        name: "convert_md_to_pdf",
        description: "将Markdown文件转换为PDF格式",
        parameters: {
            type: "object",
            properties: {
          "md_file": {
                    "type": "string",
                    "description": "要转换的Markdown文件路径"
          },
          "pdf_file": {
                    "type": "string",
                    "description": "输出的PDF文件路径"
          }
},
            required: ["md_file","pdf_file"]
        }
    }
};

export async function handler(args) {
    const fs = require('fs');
const { execSync } = require('child_process');
const path = require('path');

function convert_md_to_pdf(args) {
    const mdFile = args.md_file;
    const pdfFile = args.pdf_file;
    
    // 检查pandoc是否存在
    try {
        execSync('pandoc --version', { stdio: 'pipe' });
    } catch (e) {
        return 'Error: pandoc is not installed. Please install pandoc first.';
    }
    
    // 检查文件是否存在
    if (!fs.existsSync(mdFile)) {
        return `Error: Markdown file not found: ${mdFile}`;
    }
    
    try {
        // 使用pandoc转换为PDF
        const cmd = `pandoc "${mdFile}" -o "${pdfFile}" --pdf-engine=xelatex -V mainfont="SimSun" -V fontsize=12pt`;
        execSync(cmd, { stdio: 'inherit' });
        
        if (fs.existsSync(pdfFile)) {
            return `PDF created successfully: ${pdfFile}`;
        } else {
            return 'Error: PDF file was not created';
        }
    } catch (error) {
        return `Error converting to PDF: ${error.message}`;
    }
}

return convert_md_to_pdf(args);
}
