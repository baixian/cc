export const pages = [
  { id: 'P01', key: 'overview', name: '网络总览', icon: '◉', detail: '运行状态、网络拓扑与流量' },
  { id: 'P02', key: 'ai', name: 'AI 专线', icon: '✦', detail: '服务策略、出口与专项检测' },
  { id: 'P03', key: 'nodes', name: '节点与订阅', icon: '◇', detail: '订阅预览、节点与策略组' },
  { id: 'P04', key: 'rules', name: '分流规则', icon: '≡', detail: '顺序匹配、模拟、差异与回滚' },
  { id: 'P05', key: 'connections', name: '实时连接', icon: '↗', detail: '连接、进程与路由证据' },
  { id: 'P06', key: 'diagnostics', name: '网络诊断', icon: '⌁', detail: '探针状态、原因与证据' },
  { id: 'P07', key: 'gatekeeper', name: '门禁与恢复', icon: '⬡', detail: '预检、所有权与恢复证明' },
  { id: 'P08', key: 'workbench', name: '诊断工作台', icon: '▤', detail: '审计、历史与脱敏导出' },
  { id: 'P09', key: 'settings', name: '偏好设置', icon: '⚙', detail: '模式、权限与更新' },
  { id: 'P00', key: 'onboarding', name: '首次使用', icon: '→', detail: '环境检查与接入向导' },
  { id: 'P10', key: 'menubar', name: '菜单栏', icon: '◌', detail: '原生菜单栏功能待 Mac 实现' },
] as const;
export type PageKey = typeof pages[number]['key'];
