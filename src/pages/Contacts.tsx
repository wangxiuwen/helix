import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
    Search, Plus, Edit3, Trash2, Check, UserPlus, ChevronLeft
} from 'lucide-react';
import { useDevOpsStore, VirtualContact } from '../stores/useDevOpsStore';

const AVATAR_SEEDS = [
    'Felix', 'Aneka', 'Pepper', 'Missy', 'Sassy', 'Lucky', 'Buddy', 'Charlie',
    'Max', 'Oscar', 'Milo', 'Leo', 'Luna', 'Bella', 'Lily', 'Daisy', 'Ruby',
    'Coco', 'Gracie', 'Sadie', 'Molly', 'Rosie', 'Lola', 'Lucy', 'Stella',
];

const ROLE_PRESETS = [
    {
        role: '项目经理', icon: '📋', color: '#3b82f6',
        systemPrompt: `你是一位拥有 10 年以上经验的资深项目经理(PM)。

## 核心能力
- **项目规划**：精通 WBS 分解、甘特图排期、关键路径分析(CPM)，能将模糊需求转化为可执行的里程碑计划
- **风险管理**：擅长 RAID 日志管理（风险 Risk、假设 Assumption、问题 Issue、依赖 Dependency），提前识别瓶颈，制定应急预案
- **资源协调**：善于跨团队协作，平衡人力、时间与预算三角约束
- **敏捷实践**：熟练运用 Scrum / Kanban，能组织高效的站会、Sprint 评审与回顾

## 行为准则
1. 沟通时始终条理清晰、重点突出，使用结构化的"背景 → 问题 → 方案 → 行动项"框架
2. 任何计划建议都需附带时间节点、责任人和验收标准
3. 主动识别并提示项目中的风险和依赖关系
4. 定期汇总进度时使用"红黄绿"三色状态灯标注
5. 在团队讨论时扮演协调者角色，推动共识形成

## 输出规范
- 项目计划用表格呈现（任务 / 负责人 / 开始日 / 截止日 / 状态）
- 风险评估标注概率与影响等级（高 / 中 / 低）
- 会议纪要包含"决议事项"和"待办行动"两个板块`,
    },
    {
        role: '产品经理', icon: '📝', color: '#8b5cf6',
        systemPrompt: `你是一位敏锐的产品经理(Product Manager)，深耕互联网与 AI 产品领域超过 8 年。

## 核心能力
- **需求分析**：精通用户访谈、场景故事(User Story)编写、需求优先级排序(RICE / MoSCoW)
- **产品设计**：擅长绘制用户旅程地图、信息架构、用户流程图，产出高质量 PRD
- **数据驱动**：善于定义北极星指标，通过漏斗分析、A/B 测试、留存分析优化产品体验
- **商业思维**：能构建商业画布(Business Model Canvas)，分析竞品格局与市场定位
- **跨团队沟通**：擅长将业务语言翻译为技术语言，推动设计、研发、运营协同

## 行为准则
1. 始终以用户价值为出发点，拒绝"自嗨"型功能
2. 需求描述遵循 "作为【角色】，我希望【动作】，以便【价值】" 格式
3. 功能建议都需评估 ROI（投入产出比）和机会成本
4. 给出多种方案时标注各自优缺点和推荐理由
5. 主动思考边界场景、异常流程和灰度策略

## 输出规范
- PRD 包含：背景、目标用户、核心场景、功能清单、非功能性需求、数据埋点、上线计划
- 竞品分析使用结构化对比表格
- 优先级矩阵附带评分与排序依据`,
    },
    {
        role: '架构师', icon: '🏗️', color: '#06b6d4',
        systemPrompt: `你是一位顶级系统架构师(System Architect)，拥有复杂分布式系统设计的深厚经验。

## 核心能力
- **架构设计**：精通微服务、事件驱动、CQRS、领域驱动设计(DDD)、六边形架构等主流架构模式
- **高可用设计**：熟练掌握多活部署、流量调度、限流降级熔断(Sentinel/Hystrix)、灰度发布策略
- **高性能优化**：深入理解缓存策略（多级缓存/热点Key）、数据库读写分离/分库分表、异步队列削峰
- **云原生体系**：精通 Kubernetes 编排、Service Mesh(Istio)、Serverless、容器安全
- **数据架构**：熟悉 OLTP/OLAP 选型、数据湖、实时流处理(Flink/Kafka Streams)、向量数据库

## 行为准则
1. 技术选型时始终给出"选型矩阵"，从性能、成本、团队成熟度、社区生态等维度评估
2. 方案中必须包含容量评估、故障域分析和灾备策略
3. 避免过度设计——复杂度必须与业务规模匹配
4. 架构决策记录(ADR)使用"背景 → 决策 → 后果"标准格式
5. 重要设计附带时序图或系统交互图描述

## 输出规范
- 架构图使用文字描述+分层结构, 标注组件之间的调用关系与数据流向
- 性能评估包含 QPS、延迟 P99、吞吐量预估
- 方案对比用决策矩阵表格呈现`,
    },
    {
        role: '开发工程师', icon: '💻', color: '#10b981',
        systemPrompt: `你是一名资深全栈开发工程师(Full-Stack Developer)，拥有广泛而深入的技术功底。

## 核心能力
- **前端精通**：React/Vue/Next.js 生态、TypeScript、CSS 工程化、性能优化(Web Vitals)、PWA
- **后端精通**：Node.js/Python/Rust/Go、RESTful/GraphQL API 设计、ORM 框架、消息队列
- **数据库**：MySQL/PostgreSQL 调优（索引/执行计划）、Redis 缓存策略、MongoDB 文档建模
- **工程实践**：Git 工作流(Git Flow/Trunk-based)、代码审查(Code Review)、CI/CD 流水线构建
- **AI 应用开发**：LLM API 接入、Prompt Engineering、RAG 系统搭建、Agent 工具链开发

## 行为准则
1. 代码遵循 SOLID 原则和 Clean Code 规范，命名语义化、函数职责单一
2. 提供代码时必须包含必要注释和错误处理，不写"happy path only"的代码
3. 给出方案时考虑边界情况、并发安全、内存泄漏等常见陷阱
4. 重构建议附带修改前后对比和改进理由
5. 对性能敏感的场景给出时间/空间复杂度分析

## 输出规范
- 代码块标注语言和文件路径
- 复杂逻辑先用伪代码或流程图说明思路
- API 设计包含请求/响应示例和错误码定义
- 技术方案附带依赖版本和兼容性说明`,
    },
    {
        role: '测试工程师', icon: '🧪', color: '#f59e0b',
        systemPrompt: `你是一位严谨且经验丰富的质量保证(QA)工程师，以"质量守门人"的心态守护每一个版本。

## 核心能力
- **测试策略**：精通测试金字塔(单元/集成/E2E)设计、风险驱动测试、探索性测试
- **自动化测试**：熟练使用 Pytest/Jest/Playwright/Cypress 等框架，搭建 CI 中的自动化回归体系
- **性能测试**：掌握 JMeter/k6/Locust 压测工具，能做容量规划和瓶颈定位
- **安全测试**：了解 OWASP Top 10，能进行基础的 SQL 注入/XSS/CSRF 验证
- **缺陷管理**：擅长编写高质量 Bug 报告(复现步骤/期望结果/实际结果/严重等级)

## 行为准则
1. 思考问题时始终采用"破坏性思维"——主动寻找系统的薄弱点
2. 测试用例设计覆盖正常流、异常流、边界值、并发场景、幂等性验证
3. 对每个功能提出至少 3 个"如果……会怎样？"的质疑
4. 缺陷报告必须可复现，附带环境信息和截图/日志
5. 回归测试范围评估时，分析变更代码的影响半径

## 输出规范
- 测试计划包含：测试范围、测试环境、入口/出口标准、风险评估
- 用例以表格呈现：编号 / 模块 / 前置条件 / 测试步骤 / 期望结果 / 优先级
- 测试报告包含：执行概要 / 通过率 / 遗留缺陷清单 / 上线建议`,
    },
    {
        role: '教研专家', icon: '📚', color: '#0d9488',
        systemPrompt: `你是一位资深的教研专家(Instructional Designer)，兼具教育学理论功底和一线教学实践经验。

## 核心能力
- **课程设计**：精通 ADDIE/SAM 教学设计模型，能设计从启蒙到进阶的完整课程体系
- **认知科学**：深入理解布鲁姆教育目标分类(记忆→理解→应用→分析→评价→创造)，合理设计认知梯度
- **教学法**：熟练运用 PBL(项目式学习)、翻转课堂、脚手架教学、差异化教学等
- **评价体系**：擅长设计形成性评价与总结性评价，通过多维度评价指标衡量学习效果
- **教育技术**：了解 AI 辅助教学、自适应学习系统、游戏化设计在教育中的应用

## 行为准则
1. 课程设计始终遵循"学习目标→教学活动→评价方式"三位一体对齐原则
2. 知识点讲解采用"概念→示例→练习→反馈"四步循环
3. 内容表达深入浅出，善用类比和生活案例降低理解门槛
4. 为不同水平的学习者提供分层学习路径
5. 重视学习动机设计，在课程中嵌入成就感和好奇心激发机制

## 输出规范
- 课程大纲含：模块划分 / 课时安排 / 学习目标 / 核心知识点 / 教学活动 / 评价方式
- 教案含：导入(5min) → 新授(15min) → 练习(10min) → 总结(5min) 结构
- 习题设计标注难度等级和对应的布鲁姆层级`,
    },
    {
        role: '设计师', icon: '🎨', color: '#ec4899',
        systemPrompt: `你是一位才华横溢的 UI/UX 设计师(Product Designer)，拥有多个千万级用户产品的设计经验。

## 核心能力
- **视觉设计**：精通设计系统(Design System)搭建、色彩理论、排版规则、动效设计原则
- **交互设计**：擅长交互原型设计、微交互细节打磨、手势交互、响应式布局适配
- **用户研究**：善用可用性测试、眼动追踪分析、A/B 测试洞察用户行为模式
- **设计规范**：精通 Material Design 3、Apple HIG、Ant Design 等主流设计语言
- **工具链**：Figma 组件库搭建、Design Token 管理、设计-开发交付(Handoff)最佳实践
- **前沿趋势**：紧跟 Glassmorphism、Bento Grid、3D 插画、AI 辅助设计等设计潮流

## 行为准则
1. 设计建议始终以用户心智模型和认知负荷为出发点
2. 颜色建议提供具体 HEX/HSL 值，字体建议指定字号/字重/行高
3. 交互方案描述状态转换：默认态→悬停态→点击态→加载态→完成态→异常态
4. 对比方案时从视觉层次、可访问性(WCAG AA)、开发成本三维度评估
5. 始终关注一致性——遵守组件复用和约定的设计语言

## 输出规范
- 设计建议含：布局结构 / 色彩方案(主色/辅助色/中性色) / 字体层级 / 间距系统
- 交互说明含：触发条件 / 动画时长(ms) / 缓动曲线 / 状态流转
- 设计评审清单：对齐 / 一致性 / 可访问性 / 响应式适配 / 暗色模式`,
    },
    {
        role: '运维工程师', icon: '🔧', color: '#6366f1',
        systemPrompt: `你是一位资深的 DevOps/SRE 工程师，以"稳定性就是生命线"为信条守护生产环境。

## 核心能力
- **CI/CD 流水线**：精通 GitHub Actions/GitLab CI/Jenkins 流水线设计，构建 → 测试 → 扫描 → 部署全流程自动化
- **容器化与编排**：深度使用 Docker 多阶段构建、Kubernetes(HPA/PDB/NetworkPolicy)、Helm Charts 管理
- **可观测性体系**：搭建 Prometheus + Grafana 监控、ELK/Loki 日志聚合、Jaeger/SkyWalking 分布式链路追踪
- **基础设施即代码(IaC)**：Terraform/Pulumi 管理云资源、Ansible 配置管理
- **故障排查**：精通 Linux 性能分析工具链(top/htop/iotop/strace/perf)，能快速定位 CPU/内存/IO/网络瓶颈
- **安全加固**：防火墙策略、证书管理(Let's Encrypt)、密钥管理(Vault)、镜像漏洞扫描(Trivy)

## 行为准则
1. 任何操作建议都必须附带回滚方案和验证步骤
2. 脚本提供幂等性保证——重复执行不产生副作用
3. 故障处理遵循 "止血 → 定位 → 修复 → 复盘" 四步法
4. 变更操作标注影响范围和推荐的执行窗口
5. 监控告警设计遵循分级(P0-P3)和升级策略

## 输出规范
- 运维方案含：操作步骤(带序号) / 前置检查 / 回滚步骤 / 验证命令
- 脚本代码附带注释和错误处理(set -euo pipefail)
- 故障复盘含：时间线 / 影响范围 / 根因分析 / 改进措施`,
    },
    {
        role: '数据分析师', icon: '📊', color: '#14b8a6',
        systemPrompt: `你是一位资深的数据分析师(Data Analyst)，用数据洞察驱动商业决策。

## 核心能力
- **数据查询**：精通 SQL（窗口函数/CTE/递归查询）、能优化慢查询和处理大规模数据集
- **统计分析**：掌握假设检验、回归分析、聚类分析、时间序列预测等统计方法
- **可视化**：擅长用 ECharts/matplotlib/Tableau/Power BI 制作直观有力的数据图表
- **Python 数据栈**：精通 Pandas/NumPy/Scikit-learn 数据处理和建模流程
- **业务理解**：能深入理解电商、SaaS、教育、金融等行业的核心业务指标体系
- **AB 实验**：精通实验设计、样本量计算、显著性检验和结果解读

## 行为准则
1. 分析结论必须"用数据说话"——引用具体数字、百分比和趋势
2. 区分"相关性"与"因果性"，避免过度解读
3. 数据图表选择遵循：趋势用折线、对比用条形、占比用饼图、分布用直方图
4. 给出商业建议时衡量实施成本与预期收益
5. 分析报告结构：摘要 → 关键发现 → 详细分析 → 建议行动

## 输出规范
- SQL 查询附带注释和预期输出样例
- 指标定义精确到计算公式（分子/分母/筛选条件/时间粒度）
- 分析报告使用"数据发现 + So What + Now What"三段式结构
- 图表建议标注坐标轴含义、单位和数据来源`,
    },
    {
        role: '安全专家', icon: '🛡️', color: '#ef4444',
        systemPrompt: `你是一位顶级的网络安全专家(Security Engineer)，兼具攻击者思维和防御者视角。

## 核心能力
- **渗透测试**：精通 Web 渗透(OWASP Top 10)、API 安全测试、移动端安全、社工攻击识别
- **代码审计**：擅长发现注入漏洞(SQL/NoSQL/命令注入)、反序列化、SSRF、逻辑缺陷等安全问题
- **安全架构**：精通零信任架构、OAuth2/OIDC 认证体系、密钥管理、数据加密策略(静态/传输/使用中)
- **应急响应**：掌握安全事件分类分级、取证分析、IoC 提取、攻击溯源
- **合规与治理**：了解等保 2.0、GDPR、SOC2 等合规要求，能设计安全治理流程
- **云安全**：精通 AWS/Azure/阿里云安全最佳实践、CSPM、容器安全、密钥管理(KMS)

## 行为准则
1. 安全建议按风险等级(严重/高/中/低)排序，关键风险优先处理
2. 每个漏洞描述包含：漏洞类型 / 影响范围 / 利用条件 / 修复建议 / 验证方法
3. 安全方案兼顾安全性与可用性——拒绝"因噎废食"式的过度管控
4. 密码和密钥管理建议采用业界最佳实践(bcrypt/argon2, 密钥轮换)
5. 主动提醒容易被忽视的攻击面：日志注入、时序攻击、枚举攻击等

## 输出规范
- 安全评估报告含：风险矩阵 / 漏洞详情 / 修复优先级 / 临时缓解措施
- 安全加固清单按"检查项 / 当前状态 / 建议配置 / 参考标准"格式
- 代码安全审查标注具体代码行和修复示例`,
    },
];

function Contacts() {
    const { t } = useTranslation();
    const { contacts, lanPeers, addContact, updateContact, removeContact } = useDevOpsStore();
    const [searchQuery, setSearchQuery] = useState('');
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const [showAddForm, setShowAddForm] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);

    // Form state
    const [form, setForm] = useState({
        name: '', icon: '🤖', avatar: '', color: '#3b82f6', role: '',
        description: '', systemPrompt: '',
    });

    const allContacts = [...contacts, ...lanPeers];
    const selected = allContacts.find(c => c.id === selectedId) || null;

    const filtered = allContacts.filter(c =>
        !searchQuery || c.name.includes(searchQuery) || c.role.includes(searchQuery) || (c.device && c.device.includes(searchQuery))
    );

    // Group contacts by role
    const grouped = filtered.reduce<Record<string, VirtualContact[]>>((acc, c) => {
        const key = c.role || '其他';
        if (!acc[key]) acc[key] = [];
        acc[key].push(c);
        return acc;
    }, {});

    const openAddForm = () => {
        const seed = AVATAR_SEEDS[Math.floor(Math.random() * AVATAR_SEEDS.length)];
        setForm({
            name: '', icon: '🤖',
            avatar: `https://api.dicebear.com/9.x/micah/svg?seed=${seed}`,
            color: '#3b82f6', role: '', description: '', systemPrompt: '',
        });
        setShowAddForm(true);
        setEditingId(null);
    };

    const openEditForm = (c: VirtualContact) => {
        setForm({
            name: c.name, icon: c.icon, avatar: c.avatar, color: c.color,
            role: c.role, description: c.description || '', systemPrompt: c.systemPrompt,
        });
        setEditingId(c.id);
        setShowAddForm(true);
    };

    const handleSave = () => {
        if (!form.name.trim() || !form.role.trim()) return;
        if (editingId) {
            updateContact(editingId, { ...form });
        } else {
            const id = addContact({
                name: form.name, icon: form.icon, avatar: form.avatar,
                color: form.color, role: form.role, description: form.description,
                systemPrompt: form.systemPrompt,
            });
            setSelectedId(id);
        }
        setShowAddForm(false);
        setEditingId(null);
    };

    const handleDelete = (id: string) => {
        if (selectedId === id) setSelectedId(null);
        removeContact(id);
    };

    // Auto-select first contact on mount
    useEffect(() => {
        if (!selectedId && allContacts.length > 0) setSelectedId(allContacts[0].id);
    }, [allContacts.length, selectedId]);

    // Detail / form right panel content
    const renderRightPanel = () => {
        if (showAddForm) {
            return (
                <div className="flex-1 flex flex-col bg-white dark:bg-[#2a2a2a] overflow-y-auto">
                    {/* Header */}
                    <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-700/50 flex items-center gap-3 shrink-0">
                        <button onClick={() => { setShowAddForm(false); setEditingId(null); }} className="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
                            <ChevronLeft size={16} className="text-gray-400" />
                        </button>
                        <h3 className="text-[14px] font-semibold text-gray-800 dark:text-gray-200">
                            {editingId ? '编辑联系人' : '添加新成员'}
                        </h3>
                    </div>
                    {/* Form body */}
                    <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">
                        {/* Avatar + name row */}
                        <div className="flex items-start gap-5">
                            <div
                                className="w-20 h-20 rounded-2xl overflow-hidden shadow-md shrink-0 flex items-center justify-center"
                                style={{ background: `linear-gradient(135deg, ${form.color}33, ${form.color}66)`, border: `2px solid ${form.color}44` }}
                            >
                                {form.avatar ? (
                                    <img src={form.avatar} alt="" className="w-full h-full object-cover" />
                                ) : (
                                    <span className="text-3xl">{form.icon}</span>
                                )}
                            </div>
                            <div className="flex-1 space-y-3 pt-1">
                                <div>
                                    <label className="text-[11px] text-gray-400 font-medium mb-1 block">姓名 *</label>
                                    <input
                                        className="w-full bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors"
                                        placeholder="输入姓名"
                                        value={form.name}
                                        onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                                    />
                                </div>
                                <div>
                                    <label className="text-[11px] text-gray-400 font-medium mb-1 block">简介</label>
                                    <input
                                        className="w-full bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors"
                                        placeholder="一句话描述"
                                        value={form.description}
                                        onChange={e => setForm(f => ({ ...f, description: e.target.value }))}
                                    />
                                </div>
                            </div>
                        </div>

                        {/* Role presets */}
                        <div>
                            <label className="text-[11px] text-gray-400 font-medium mb-2 block">角色 *</label>
                            <div className="flex flex-wrap gap-1.5">
                                {ROLE_PRESETS.map(p => (
                                    <button
                                        key={p.role}
                                        className={`px-3 py-1.5 rounded-full text-[11px] transition-all border ${form.role === p.role
                                            ? 'bg-[#07c160]/10 text-[#07c160] border-[#07c160]/30 font-medium shadow-sm'
                                            : 'bg-gray-50 dark:bg-gray-800 text-gray-500 dark:text-gray-400 border-gray-200 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700'
                                            }`}
                                        onClick={() => setForm(f => ({ ...f, role: p.role, icon: p.icon, color: p.color, systemPrompt: p.systemPrompt || f.systemPrompt }))}
                                    >
                                        {p.icon} {p.role}
                                    </button>
                                ))}
                            </div>
                            <input
                                className="w-full mt-2.5 bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors"
                                placeholder="或自定义角色名称"
                                value={form.role}
                                onChange={e => setForm(f => ({ ...f, role: e.target.value }))}
                            />
                        </div>

                        {/* Color picker */}
                        <div>
                            <label className="text-[11px] text-gray-400 font-medium mb-2 block">主题色</label>
                            <div className="flex gap-2.5">
                                {['#3b82f6', '#8b5cf6', '#06b6d4', '#10b981', '#f59e0b', '#ef4444', '#ec4899', '#6366f1', '#0d9488', '#f97316'].map(c => (
                                    <button
                                        key={c}
                                        className={`w-7 h-7 rounded-full transition-all ${form.color === c ? 'scale-125 ring-2 ring-offset-2 ring-gray-300 dark:ring-gray-600 dark:ring-offset-[#2a2a2a]' : 'hover:scale-110'}`}
                                        style={{ backgroundColor: c }}
                                        onClick={() => setForm(f => ({ ...f, color: c }))}
                                    />
                                ))}
                            </div>
                        </div>

                        {/* System Prompt */}
                        <div>
                            <label className="text-[11px] text-gray-400 font-medium mb-1 block">系统提示词 (可选)</label>
                            <textarea
                                className="w-full bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-2.5 text-[13px] text-gray-800 dark:text-gray-200 outline-none border border-gray-200 dark:border-gray-600 focus:border-[#07c160] transition-colors min-h-[100px] resize-none"
                                placeholder="自定义 AI 行为... (留空使用默认)"
                                value={form.systemPrompt}
                                onChange={e => setForm(f => ({ ...f, systemPrompt: e.target.value }))}
                            />
                        </div>
                    </div>
                    {/* Footer */}
                    <div className="px-6 py-4 border-t border-gray-100 dark:border-gray-700/50 flex justify-end gap-2 shrink-0">
                        <button
                            onClick={() => { setShowAddForm(false); setEditingId(null); }}
                            className="px-5 py-2 text-[12px] text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                        >
                            取消
                        </button>
                        <button
                            onClick={handleSave}
                            disabled={!form.name.trim() || !form.role.trim()}
                            className="px-5 py-2 text-[12px] bg-[#07c160] hover:bg-[#06ad56] disabled:opacity-40 text-white rounded-lg transition-colors flex items-center gap-1.5"
                        >
                            <Check size={13} /> {editingId ? '保存' : '添加'}
                        </button>
                    </div>
                </div>
            );
        }

        if (selected) {
            return (
                <div className="flex-1 flex flex-col bg-white dark:bg-[#2a2a2a]">
                    {/* Info section */}
                    <div className="flex-1 overflow-y-auto">
                        {/* Top: avatar + name area */}
                        <div className="px-8 pt-8 pb-6 flex items-start gap-5">
                            <div
                                className="w-16 h-16 rounded-2xl overflow-hidden shadow-md shrink-0 flex items-center justify-center"
                                style={{ background: `linear-gradient(135deg, ${selected.color}33, ${selected.color}66)`, border: `2px solid ${selected.color}44` }}
                            >
                                {selected.avatar ? (
                                    <img src={selected.avatar} alt="" className="w-full h-full object-cover" />
                                ) : (
                                    <span className="text-2xl">{selected.icon}</span>
                                )}
                            </div>
                            <div className="flex-1 pt-1">
                                <h2 className="text-[18px] font-semibold text-gray-800 dark:text-gray-200 leading-tight">{selected.name}</h2>
                                <div className="flex items-center gap-1.5 mt-1">
                                    <span className="text-[13px]">{selected.icon}</span>
                                    <span className="text-[13px] text-gray-500 dark:text-gray-400">{selected.role}</span>
                                </div>
                                {selected.description && (
                                    <p className="text-[12px] text-gray-400 mt-2 leading-relaxed">{selected.description}</p>
                                )}
                            </div>
                        </div>

                        {/* Info rows */}
                        <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50">
                            <div className="py-4 flex items-center gap-3">
                                <span className="text-[12px] text-gray-400 w-16 shrink-0">角色</span>
                                <span className="text-[13px] text-gray-700 dark:text-gray-300 flex items-center gap-1.5">
                                    <span
                                        className="inline-block w-2.5 h-2.5 rounded-full"
                                        style={{ backgroundColor: selected.color }}
                                    />
                                    {selected.role}
                                </span>
                            </div>
                        </div>
                        {selected.device && (
                            <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50">
                                <div className="py-4 flex items-center gap-3">
                                    <span className="text-[12px] text-gray-400 w-16 shrink-0">IP</span>
                                    <span className="text-[13px] text-gray-700 dark:text-gray-300 font-mono">
                                        {selected.ip}:{selected.port}
                                    </span>
                                </div>
                            </div>
                        )}
                        {selected.systemPrompt && !selected.isLan && (
                            <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50">
                                <div className="py-4">
                                    <span className="text-[12px] text-gray-400 block mb-2">系统提示词</span>
                                    <div className="text-[12px] text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-800/50 rounded-lg p-3 whitespace-pre-wrap leading-relaxed max-h-[200px] overflow-y-auto">
                                        {selected.systemPrompt}
                                    </div>
                                </div>
                            </div>
                        )}

                        {/* Actions */}
                        {!selected.isLan && (
                            <div className="mx-6 border-t border-gray-100 dark:border-gray-700/50 py-4 flex gap-2">
                                <button
                                    onClick={() => openEditForm(selected)}
                                    className="flex-1 py-2.5 text-[12px] bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-xl hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors flex items-center justify-center gap-1.5"
                                >
                                    <Edit3 size={13} /> 编辑资料
                                </button>
                                <button
                                    onClick={() => handleDelete(selected.id)}
                                    className="px-5 py-2.5 text-[12px] text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-colors flex items-center gap-1.5"
                                >
                                    <Trash2 size={13} /> 删除
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            );
        }

        return (
            <div className="flex-1 flex items-center justify-center bg-white dark:bg-[#2a2a2a]">
                <div className="text-center text-gray-400">
                    <UserPlus size={36} className="mx-auto mb-3 opacity-20" />
                    <p className="text-[13px]">{t('contacts.select_hint', '选择一个联系人查看详情')}</p>
                    <button
                        onClick={openAddForm}
                        className="mt-4 px-5 py-2 text-[12px] bg-[#07c160] hover:bg-[#06ad56] text-white rounded-full transition-colors"
                    >
                        + 添加虚拟角色
                    </button>
                </div>
            </div>
        );
    };

    return (
        <div className="flex flex-1 w-full h-full bg-[#f0f0f0] dark:bg-[#1e1e1e]">
            {/* Left: Contact List */}
            <div className="w-[240px] shrink-0 bg-[#e8e8e8] dark:bg-[#252525] border-r border-black/[0.06] dark:border-white/[0.06] flex flex-col">
                {/* Search + Add */}
                <div className="px-3 pt-3 pb-2 flex items-center gap-2" style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}>
                    <div className="flex-1 relative" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
                        <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                        <input
                            className="w-full bg-white/60 dark:bg-white/5 rounded-md pl-8 pr-3 py-1.5 text-[12px] text-gray-700 dark:text-gray-300 placeholder:text-gray-400 outline-none border border-transparent focus:border-[#07c160]/30 transition-colors"
                            placeholder={t('contacts.search', '搜索联系人...')}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                        />
                    </div>
                    <button
                        onClick={openAddForm}
                        className="w-7 h-7 flex items-center justify-center rounded-md bg-[#07c160] hover:bg-[#06ad56] text-white transition-colors shrink-0"
                        title={t('contacts.add', '添加联系人')}
                        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
                    >
                        <Plus size={14} />
                    </button>
                </div>

                {/* Contact list */}
                <div className="flex-1 overflow-y-auto">
                    {Object.entries(grouped).map(([role, members]) => (
                        <div key={role}>
                            <div className="px-4 py-1.5 text-[10px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider sticky top-0 bg-[#e8e8e8] dark:bg-[#252525]">
                                {role} ({members.length})
                            </div>
                            {members.map(c => (
                                <div
                                    key={c.id}
                                    className={`flex items-center gap-2.5 px-4 py-2.5 cursor-pointer transition-colors ${selectedId === c.id
                                        ? 'bg-black/[0.08] dark:bg-white/[0.08]'
                                        : 'hover:bg-black/[0.04] dark:hover:bg-white/[0.04]'
                                        }`}
                                    onClick={() => { setSelectedId(c.id); setShowAddForm(false); }}
                                >
                                    <div
                                        className="w-9 h-9 rounded-lg overflow-hidden shrink-0 flex items-center justify-center"
                                        style={{ background: `linear-gradient(135deg, ${c.color}22, ${c.color}44)`, border: `1.5px solid ${c.color}33` }}
                                    >
                                        {c.avatar ? (
                                            <img src={c.avatar} alt="" className="w-full h-full object-cover" />
                                        ) : (
                                            <span className="text-base">{c.icon}</span>
                                        )}
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="text-[13px] font-medium text-gray-800 dark:text-gray-200 truncate">{c.name}</div>
                                        <div className="text-[11px] text-gray-400 truncate">{c.role}</div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    ))}
                    {filtered.length === 0 && (
                        <div className="text-center py-12 text-gray-400">
                            <UserPlus size={28} className="mx-auto mb-2 opacity-20" />
                            <p className="text-[11px]">
                                {searchQuery ? t('contacts.no_results', '未找到联系人') : t('contacts.empty', '暂无联系人')}
                            </p>
                        </div>
                    )}
                </div>
            </div>

            {/* Right: Detail / Form */}
            {renderRightPanel()}
        </div>
    );
}

export default Contacts;
