# Mac 智能网络网关 · S1 开发源码

依据 V3.0 六份基础文档与 V3.2《原型与接口对齐修订书》启动的 **S1 工程骨架**。当前为开发版，不具备系统代理或 TUN 接管能力。

## 已实现

- Tauri 2 + Vue 3 + TypeScript + Rust 工程目录和最小 capability/CSP。
- P00–P10 导航及统一状态框架；P00/P01 读取 Rust IPC，明确显示未知、断开和未接管；其他页面为未接入占位。
- `app_get_bootstrap`、`system_get_environment`、`core_connect_external`、`core_disconnect`、`core_get_snapshot`。目前仅支持无 Secret 的本机控制器只读连接；严格限制到 `127.0.0.1` 或 `::1`，不访问其他地址，不修改网络。
- `/version` 与 `/connections` 真实读取，首页显示连接数量；接口失败或返回字段缺失时显示错误/未知，不展示健康假象。
- 前端 IPC 版本校验，浏览器打开时拒绝伪造桌面数据；4 项基础单测。
- Rust DTO 上的 `ts-rs` 导出注解。当前 TS 契约快照需要在有 Rust 的环境执行 `cargo test` 后核对生成文件，尚未完成自动契约门禁。

## 运行

```sh
cd apps/desktop
npm install
npm run dev               # 仅浏览器 UI；会显示 DESKTOP_REQUIRED
npm run tauri dev         # 须在安装 Xcode、Rust、Tauri 依赖的 Mac 上执行
npm run build             # 前端类型检查及构建
npm run test              # 前端 IPC 错误路径测试
```

macOS 上执行 `cd apps/desktop/src-tauri && cargo test` 检查 Rust DTO 和测试；再执行 `cargo fmt --check`。此交付环境为 Linux，未安装 Rust/Xcode，不能宣称 Mac 构建和实机网络测试通过。生产打包也尚未配置签名、公证、helper、Mihomo 二进制和卸载恢复。

## 下一批任务

1. MAC-01/02：Mac 上只读检查系统代理、DNS、路由及 VPN；测试 H/N 路线和恢复证据，冻结 S0 决策。
2. CON-01/BE-04：固定 Mihomo 版本，补 Keychain Secret 的原生录入与读取、API fixtures、Rust DTO → TS 自动生成与 CI 契约门禁；完成 Rust 侧编译和契约测试。
3. BE-05/FE-04/05：真实连接/流量快照与 P01 拓扑，事件断流超过 5 秒转 STALE，断线后从快照重新同步。
4. 在恢复 journal/所有权机制通过实机测试前，保持所有网络写入能力关闭。

参考资料位于用户提供的 V3.2 完整交付压缩包；源码不含节点凭据、订阅或第三方二进制。
