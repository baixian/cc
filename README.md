# Mac 智能网络网关 · S1 开发源码

依据 V3.0 六份基础文档与 V3.2《原型与接口对齐修订书》启动的 **S1 工程骨架**。当前为开发版，不具备系统代理或 TUN 接管能力。

## 已实现

- Tauri 2 + Vue 3 + TypeScript + Rust 工程目录和最小 capability/CSP。
- P00–P10 导航及统一状态框架；P00/P01/P05/P06 读取 Rust IPC，明确显示未知、断开和未接管；其他页面为未接入占位。P00 在 macOS 上只读检查当前代理配置的启用标记，不返回代理地址，不能据此判定 VPN 或流量路径。
- `app_get_bootstrap`、`system_get_environment`、`core_connect_external`、`core_disconnect`、`core_get_snapshot`。目前仅支持无 Secret 的本机控制器只读连接；严格限制到 `127.0.0.1` 或 `::1`，不访问其他地址，不修改网络。
- `/version` 与 `/connections` 真实读取；P01 每两秒读取连接快照并汇总最多 6 条常见链路、缺失链路数及有进程名的连接数，五秒无成功读取则显示 `UNKNOWN`。这些字段不能证明系统代理或 TUN 已接管。P05 展示连接目标、进程、规则、链路和流量，支持筛选、分页、两秒轮询及五秒过期提示；连接变化可能影响分页顺序。
- P06 提供手动只读诊断：分别读取本机控制器的 `/version`、`/connections`，显示接口可读性、耗时与原因码。外网、DNS、系统代理、TUN 均不在这次诊断范围，网络健康保持 `UNKNOWN`。
- 前端 IPC 版本校验，浏览器打开时拒绝伪造桌面数据；6 项基础单测。
- Rust DTO 使用 `ts-rs` 导出 TypeScript 类型，前端直接引用生成文件；macOS CI 会比对生成结果与仓库版本，阻断字段漂移。累计字节数超出 JavaScript 安全整数时标为未知。

## 运行

```sh
cd apps/desktop
npm install
npm run dev               # 仅浏览器 UI；会显示 DESKTOP_REQUIRED
npm run tauri dev         # 须在安装 Xcode、Rust、Tauri 依赖的 Mac 上执行
npm run build             # 前端类型检查及构建
npm run test              # 前端 IPC 错误路径测试
```

macOS 上执行 `cd apps/desktop/src-tauri && cargo test` 检查 Rust DTO 和测试；再执行 `cargo fmt --check`。GitHub Actions 的 macOS S1 编译和单测已通过（[运行记录](https://github.com/baixian/cc/actions/runs/39189843429)）。这并不证明系统网络接管、真实控制器兼容或异常恢复通过。生产打包也尚未配置签名、公证、helper、Mihomo 二进制和卸载恢复。

在 Mac 上运行 `bash scripts/check-contracts.sh` 验证 Rust 生成的前端类型；Rust DTO 改动后运行 `bash scripts/check-contracts.sh --write` 并提交 `apps/desktop/src/generated/ipc` 的更新。`fixtures/connections.json` 是无凭据的模拟响应，仅用于解析测试，并非实机采集记录。

## 下一批任务

1. MAC-01/02：Mac 上只读检查系统代理、DNS、路由及 VPN；测试 H/N 路线和恢复证据，冻结 S0 决策。
2. CON-01/BE-04：固定 Mihomo 版本，补 Keychain Secret 的原生录入与读取，并采集经过脱敏的实机 API fixtures；生成类型与 CI 契约门禁已接入。
3. BE-05/FE-04/05：P01 动态拓扑和事件流，事件断流超过 5 秒转 STALE，断线后从快照重新同步；目前 P01/P05 使用定时快照，链路汇总并非系统流量拓扑。
4. 在恢复 journal/所有权机制通过实机测试前，保持所有网络写入能力关闭。

参考资料位于用户提供的 V3.2 完整交付压缩包；源码不含节点凭据、订阅或第三方二进制。

## Mac 实机与持续集成

推送后 `.github/workflows/ci.yml` 会在 macOS runner 上执行前端测试、构建与 Rust 编译。CI 编译通过仅证明代码可构建，不等于系统代理、TUN 或恢复测试通过。S0 测试机可用 `bash scripts/mac-readonly-audit.sh /path/to/new-evidence-dir` 留存只读网络快照。报告可能含本机 IP、DNS 与 VPN 名称，分享前请检查并脱敏。
