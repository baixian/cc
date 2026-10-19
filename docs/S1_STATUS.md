# S1 开发记录 · 2026-10-18

## 已有闭环

1. P00 接受用户显式输入的本机控制器地址，仅允许 `http://127.0.0.1:port` 或 `http://[::1]:port`；macOS 上只读观察代理、DNS 和默认路由配置摘要，不返回对应地址。读取失败或无法识别时显示 UNKNOWN。
2. Rust 请求 `/version` 确认控制器响应；拒绝重定向、代理环境变量、非本机 IP、路径、查询参数、凭据和无端口地址。
3. 连接成功后 `/connections` 取真实连接数量及可用的累计字节字段；失败显示 `UNKNOWN` 与具体错误，不把旧会话当作可用。
4. E 模式仅只读。无 Secret 的控制器可以使用；需要 Secret 的控制器返回 `CORE_AUTH_REQUIRED`。尚未实现 Keychain 中的 Secret 原生输入。
5. P00–P10 导航完整；P00/P01/P05/P06 有真实 IPC，其他业务内容仍为待接入状态。P01 汇总本机控制器链路证据，P05 支持只读连接筛选与分页；两页都使用定时快照，过期或断线显示未知。P06 仅手动检测本机控制器接口。链路证据不是 macOS 系统路由证明。

## 验证

- `npm run build`：通过。
- `npm test`：6 项通过（浏览器阻断、版本冲突、后端错误、只读连接参数、连接分页参数、诊断参数）。
- [macOS CI](https://github.com/baixian/cc/actions)：Rust 单测、IPC 生成类型比对与桌面编译；连接解析使用无凭据模拟响应测试。实机 API 兼容尚未验证。
- macOS 真实设备上的系统观察、控制器兼容及网络接管：仍未做实机验证。CI runner 的命令与解析测试不代替用户测试机的 S0 证据。

## 下一步优先级

| 顺序 | 任务 | 准入条件 |
|---|---|---|
| 1 | Mac 实机 S0：环境只读探测、TUN/系统代理 POC、恢复方案 | 独立测试机与管理员测试授权 |
| 2 | 固定 Mihomo 构建版本，录制并脱敏 `/version`、`/configs`、`/connections` fixtures | 可访问的测试内核 |
| 3 | 原生 Secret/Keychain、安全能力矩阵；Rust DTO→TS 生成门禁已接入 | Mac 编译链 |
| 4 | 实时 WS 采集、断流 STALE 与 P01 拓扑 | 固定 API fixture 与状态定义 |
| 5 | 订阅、规则及 AI 策略事务；最后是 helper/TUN 接管和实机恢复 | S0 安全路线确定 |

## GitHub

用户已指定公开仓库 `https://github.com/baixian/cc`。本仓库只含开发源码、无节点凭据或订阅。提交记录不表示 Rust 编译、Mac 实机网络测试或正式发布已通过。
