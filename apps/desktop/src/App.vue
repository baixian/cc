<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { gatewayApi, GatewayIpcError } from './ipc';
import type { BootstrapDto, ConnectionPageDto, CoreSnapshotDto, DiagnosticReportDto, EnvironmentDto, ProxyCatalogDto, RunningRulesDto, RuleSimulationDto } from './generated/contracts';
import { pages, type PageKey } from './pages';

const active = ref<PageKey>('overview');
const boot = ref<BootstrapDto | null>(null);
const environment = ref<EnvironmentDto | null>(null);
const snapshot = ref<CoreSnapshotDto | null>(null);
const endpoint = ref('http://127.0.0.1:9090');
const error = ref('');
const coreError = ref('');
const loading = ref(false);
const connectionPage = ref<ConnectionPageDto | null>(null);
const connectionError = ref('');
const connectionLoading = ref(false);
const diagnosticReport = ref<DiagnosticReportDto | null>(null);
const diagnosticError = ref('');
const diagnosticLoading = ref(false);
const catalog = ref<ProxyCatalogDto | null>(null);
const catalogError = ref('');
const catalogLoading = ref(false);
const runningRules = ref<RunningRulesDto | null>(null);
const rulesError = ref('');
const rulesLoading = ref(false);
const simulateHost = ref('');
const simulation = ref<RuleSimulationDto | null>(null);
const simulationError = ref('');
const simulationLoading = ref(false);
const connectionFilter = ref('');
const appliedFilter = ref('');
const cursor = ref<string | null>(null);
const cursorHistory = ref<(string | null)[]>([]);
const clock = ref(Date.now());
let poller: ReturnType<typeof setInterval> | undefined;
let generation = 0;
let snapshotGeneration = 0;
let diagnosticGeneration = 0;
let catalogGeneration = 0;
let rulesGeneration = 0;
const connectionStale = computed(() => !connectionPage.value || clock.value - Date.parse(connectionPage.value.observedAt) > 5000);
const snapshotStale = computed(() => !snapshot.value || clock.value - Date.parse(snapshot.value.observedAt) > 5000);
const overviewValid = computed(() => !coreError.value && !snapshotStale.value);
const current = computed(() => pages.find(p => p.key === active.value)!);
const proxyLabel = computed(() => {
  if (environment.value?.proxyState === 'ENABLED_FLAG_OBSERVED') return '发现启用标记';
  if (environment.value?.proxyState === 'NO_ENABLED_FLAG_OBSERVED') return '未见启用标记';
  return 'UNKNOWN';
});
const dnsLabel = computed(() => environment.value?.dnsState === 'DNS_ENTRY_OBSERVED' ? '发现 DNS 条目' : 'UNKNOWN');
const routeLabel = computed(() => environment.value?.defaultRouteState === 'DEFAULT_ROUTE_OBSERVED' ? '发现默认路由' : 'UNKNOWN');
function explain(cause: unknown, fallback: string) {
  return cause instanceof GatewayIpcError ? `${cause.code}：${cause.message}` : fallback;
}
async function refresh() {
  loading.value = true;
  error.value = '';
  snapshotGeneration++;
  try {
    const [b, e] = await Promise.all([gatewayApi.bootstrap(), gatewayApi.environment()]);
    boot.value = b;
    environment.value = e;
    snapshot.value = null;
    coreError.value = '';
    if (b.coreMode === 'external') {
      await loadOverviewSnapshot();
    }
    if (active.value === 'connections') void loadConnections();
    if (active.value === 'nodes') void loadCatalog();
    if (active.value === 'rules') void loadRules();
  } catch (cause) {
    boot.value = null;
    environment.value = null;
    snapshot.value = null;
    error.value = explain(cause, '读取系统状态失败');
  } finally { loading.value = false; }
}
async function connect() {
  loading.value = true;
  error.value = '';
  try { await gatewayApi.connectExternal(endpoint.value.trim()); await refresh(); }
  catch (cause) { error.value = explain(cause, '连接失败'); }
  finally { loading.value = false; }
}
async function disconnect() {
  loading.value = true;
  generation++;
  snapshotGeneration++;
  diagnosticGeneration++;
  catalogGeneration++;
  rulesGeneration++;
  diagnosticReport.value = null;
  diagnosticError.value = '';
  connectionPage.value = null;
  snapshot.value = null;
  catalog.value = null;
  runningRules.value = null;
  simulation.value = null;
  try { await gatewayApi.disconnect(); await refresh(); }
  catch (cause) { error.value = explain(cause, '断开失败'); }
  finally { loading.value = false; }
}
async function loadCatalog() {
  if (boot.value?.coreMode !== 'external') return;
  const request = ++catalogGeneration;
  catalogLoading.value = true;
  try {
    const result = await gatewayApi.nodesList();
    if (request !== catalogGeneration || boot.value?.coreMode !== 'external') return;
    catalog.value = result;
    catalogError.value = '';
  } catch (cause) {
    if (request === catalogGeneration) catalogError.value = explain(cause, '代理目录读取失败');
  } finally { if (request === catalogGeneration) catalogLoading.value = false; }
}
async function loadRules() {
  if (boot.value?.coreMode !== 'external') return;
  const request = ++rulesGeneration;
  rulesLoading.value = true;
  simulation.value = null;
  try {
    const result = await gatewayApi.rulesGet();
    if (request !== rulesGeneration || boot.value?.coreMode !== 'external') return;
    runningRules.value = result;
    rulesError.value = '';
  } catch (cause) {
    if (request === rulesGeneration) rulesError.value = explain(cause, '规则读取失败');
  } finally { if (request === rulesGeneration) rulesLoading.value = false; }
}
async function simulateRule() {
  if (boot.value?.coreMode !== 'external' || simulationLoading.value) return;
  const request = rulesGeneration;
  simulationLoading.value = true;
  simulation.value = null;
  simulationError.value = '';
  try {
    const result = await gatewayApi.rulesSimulate(simulateHost.value.trim());
    if (request === rulesGeneration) simulation.value = result;
  } catch (cause) {
    if (request === rulesGeneration) simulationError.value = explain(cause, '规则模拟失败');
  } finally { simulationLoading.value = false; }
}
async function runDiagnostics() {
  if (diagnosticLoading.value || boot.value?.coreMode !== 'external') return;
  const request = ++diagnosticGeneration;
  diagnosticLoading.value = true;
  diagnosticError.value = '';
  try {
    const result = await gatewayApi.diagnosticsRun();
    if (request === diagnosticGeneration && boot.value?.coreMode === 'external') diagnosticReport.value = result;
  } catch (cause) {
    if (request === diagnosticGeneration) diagnosticError.value = explain(cause, '诊断读取失败');
  } finally { diagnosticLoading.value = false; }
}
function diagnosticReason(code: string | null) {
  if (!code) return '控制器接口可读';
  const labels: Record<string, string> = {
    CORE_UNAVAILABLE: '本机控制器未响应', CORE_AUTH_REQUIRED: '控制器需要 Secret',
    CORE_API_ERROR: '控制器返回异常状态', CORE_RESPONSE_INVALID: '响应缺少必要字段',
    CORE_RESPONSE_TOO_LARGE: '响应超过读取上限', CORE_CLIENT_ERROR: '本机连接创建失败',
  };
  return labels[code] ?? code;
}
async function loadOverviewSnapshot() {
  if (boot.value?.coreMode !== 'external') return;
  const request = ++snapshotGeneration;
  try {
    const data = await gatewayApi.coreSnapshot();
    if (request !== snapshotGeneration || boot.value?.coreMode !== 'external') return;
    snapshot.value = data;
    coreError.value = '';
    clock.value = Date.now();
  } catch (cause) {
    if (request === snapshotGeneration) coreError.value = explain(cause, '内核快照读取失败');
  }
}
async function loadConnections() {
  if (connectionLoading.value || active.value !== 'connections') return;
  const request = generation;
  connectionLoading.value = true;
  try {
    const page = await gatewayApi.connectionsList(cursor.value, appliedFilter.value);
    if (request !== generation || active.value !== 'connections') return;
    connectionPage.value = page;
    connectionError.value = '';
    clock.value = Date.now();
  } catch (cause) {
    if (request !== generation || active.value !== 'connections') return;
    connectionError.value = explain(cause, '连接数据读取失败');
  } finally { connectionLoading.value = false; }
}
function filterConnections() {
  generation++;
  appliedFilter.value = connectionFilter.value.trim();
  cursor.value = null;
  cursorHistory.value = [];
  connectionPage.value = null;
  void loadConnections();
}
function nextConnections() {
  if (!connectionPage.value?.nextCursor) return;
  cursorHistory.value.push(cursor.value);
  cursor.value = connectionPage.value.nextCursor;
  connectionPage.value = null;
  generation++;
  void loadConnections();
}
function previousConnections() {
  if (!cursorHistory.value.length) return;
  cursor.value = cursorHistory.value.pop() ?? null;
  connectionPage.value = null;
  generation++;
  void loadConnections();
}
function displayBytes(value: number | null) {
  if (value === null) return '未知';
  return `${(value / 1024).toFixed(1)} KiB`;
}
watch(active, page => {
  generation++;
  if (page === 'connections') void loadConnections();
  if (page === 'overview') void loadOverviewSnapshot();
  if (page === 'nodes') void loadCatalog();
  if (page === 'rules') void loadRules();
});
onMounted(() => {
  void refresh();
  poller = setInterval(() => {
    clock.value = Date.now();
    if (active.value === 'connections' && boot.value?.coreMode === 'external') void loadConnections();
    if (active.value === 'overview' && boot.value?.coreMode === 'external') void loadOverviewSnapshot();
  }, 2000);
});
onUnmounted(() => { if (poller) clearInterval(poller); generation++; snapshotGeneration++; diagnosticGeneration++; catalogGeneration++; rulesGeneration++; });
</script>

<template>
  <div class="shell">
    <aside class="sidebar">
      <div class="brand"><span class="logo">M</span><div><strong>Mac 智能网关</strong><small>V3.2 · 开发版</small></div></div>
      <div class="nav-label">网络工作台</div>
      <button v-for="p in pages" :key="p.id" class="nav" :class="{ selected: active === p.key }" @click="active = p.key"><span class="icon">{{ p.icon }}</span>{{ p.name }}<small>{{ p.id }}</small></button>
      <div class="sidebar-bottom"><span class="dot off"></span> {{ boot?.coreMode === 'external' ? '外部内核 · 只读' : '未接管系统网络' }}</div>
    </aside>
    <main class="main">
      <header class="topbar"><div class="breadcrumb">工作台 <span>/</span> {{ current.name }}</div><div class="top-actions"><span class="mode">{{ boot?.coreMode ?? '未选择模式' }}</span><span class="status">{{ boot?.coreMode === 'external' && !overviewValid ? '状态未知' : boot?.coreState ?? '未连接' }}</span><button class="refresh" :disabled="loading" @click="refresh">{{ loading ? '读取中' : '刷新状态' }}</button></div></header>
      <div class="content">
        <div class="heading"><div><div class="eyebrow">{{ current.id }} / NETWORK CONTROL</div><h1>{{ current.name }}</h1><p>{{ current.detail }}</p></div><span class="badge">开发中 · 无网络写入能力</span></div>
        <div v-if="error" class="alert" role="alert">{{ error }}</div>
        <div v-if="coreError" class="alert" role="alert">{{ coreError }}。上次连接不代表内核现在可用。</div>
        <template v-if="active === 'overview'">
          <div class="cards"><div class="card"><div class="card-label">内核状态</div><strong>{{ overviewValid ? snapshot?.state : 'UNKNOWN' }}</strong><small>来源：{{ overviewValid ? 'Mihomo API 快照' : '无新鲜快照' }}</small></div><div class="card"><div class="card-label">网络入口</div><strong>{{ boot?.ingress ?? '未知' }}</strong><small>未验证 TUN / 系统代理</small></div><div class="card"><div class="card-label">实时连接</div><strong>{{ overviewValid ? snapshot?.connectionCount : '未知' }}</strong><small>{{ snapshot?.observedAt ?? '等待读取' }}</small></div><div class="card"><div class="card-label">网络健康</div><strong class="unknown">UNKNOWN</strong><small>连接数不代表健康检测通过</small></div></div>
          <div class="panel topology"><div class="panel-title"><strong>连接链路证据</strong><span>{{ overviewValid ? `内核 ${snapshot?.coreVersion} · ${snapshot?.observedAt}` : 'UNKNOWN / 无新鲜快照' }}</span></div><div v-if="overviewValid && snapshot" class="evidence"><p>本机控制器观测到 {{ snapshot.connectionCount }} 条连接，其中 {{ snapshot.identifiedProcessCount }} 条含进程名，{{ snapshot.unknownRouteCount }} 条未提供链路。下面仅展示最多 6 条常见链路。</p><div v-for="route in snapshot.routeEvidence" :key="route.chain" class="route-row"><span>{{ route.chain }}</span><div class="route-track"><div :style="{ width: `${Math.max(2, route.connectionCount / Math.max(1, snapshot.connectionCount) * 100)}%` }"></div></div><strong>{{ route.connectionCount }}</strong></div><div v-if="!snapshot.routeEvidence.length" class="empty compact">控制器未提供可汇总的链路数据。</div><small>链路字段是 Mihomo 的连接记录，不证明 macOS 流量经过本应用，也不代表系统代理或 TUN 已接管。</small></div><div v-else class="empty"><div class="empty-mark">◎</div><b>暂无新鲜的连接证据</b><p>连接控制器后会自动读取；读取失败或超过 5 秒未成功刷新即显示 UNKNOWN。</p></div></div>
        </template>
        <template v-else-if="active === 'onboarding'"><div class="panel"><div class="panel-title"><strong>首次使用 · 环境扫描</strong><span>只读接入</span></div><div class="check-row"><span>操作系统</span><strong>{{ environment?.osVersion ?? '未知' }}</strong></div><div class="check-row"><span>CPU 架构</span><strong>{{ environment?.architecture ?? '未知' }}</strong></div><div class="check-row"><span>系统代理配置标记</span><strong :class="{ unknown: proxyLabel === 'UNKNOWN' }">{{ proxyLabel }}<br/><small>{{ environment?.proxyReasonCode ?? '尚未观测' }}</small></strong></div><div class="check-row"><span>DNS 配置</span><strong :class="{ unknown: dnsLabel === 'UNKNOWN' }">{{ dnsLabel }}<br/><small>{{ environment?.dnsReasonCode ?? '尚未观测' }}</small></strong></div><div class="check-row"><span>默认路由</span><strong :class="{ unknown: routeLabel === 'UNKNOWN' }">{{ routeLabel }}<br/><small>{{ environment?.defaultRouteReasonCode ?? '尚未观测' }}</small></strong></div><div class="check-row"><span>网络检查</span><strong class="unknown">{{ environment?.networkStatus ?? 'UNKNOWN' }}</strong></div><p class="table-note">仅展示 macOS 当前配置中的标记，不返回代理、DNS 或网关地址。观察到配置不代表流量路径、外网连通性或网络健康；未见代理标记也不排除 VPN 与应用内代理。</p><form class="connect-form" @submit.prevent="connect"><label for="endpoint">本机 Mihomo 控制器地址（无需 Secret 的控制器）</label><div><input id="endpoint" v-model="endpoint" placeholder="http://127.0.0.1:9090" :disabled="loading"/><button :disabled="loading" type="submit">连接只读内核</button><button v-if="boot?.coreMode === 'external'" :disabled="loading" type="button" @click="disconnect">断开</button></div><small>仅连接 127.0.0.1 或 ::1；不修改现有 Clash 配置、系统代理或 TUN。需要 Secret 的控制器会明确报错。</small></form><div class="notice">托管模式的配置预览、预检、系统授权及读回健康尚未实现，当前不能完成托管向导。</div></div></template>
        <template v-else-if="active === 'connections'"><div class="panel"><div class="panel-title"><strong>实时连接 · 本机 Mihomo</strong><span>{{ connectionError || connectionStale ? 'UNKNOWN / 数据已过期' : '已观测' }} · {{ connectionPage?.observedAt ?? '尚未观测' }}</span></div>
          <form class="connections-toolbar" @submit.prevent="filterConnections"><input v-model="connectionFilter" maxlength="128" placeholder="筛选域名、IP、进程或规则" aria-label="筛选连接"/><button type="submit">筛选</button><button type="button" @click="loadConnections">刷新</button><span>{{ connectionLoading ? '读取中' : `匹配 ${connectionPage?.total ?? '未知'} 条` }}</span></form>
          <div v-if="connectionError" class="alert" role="alert">{{ connectionError }}。已有列表仅作为历史快照，不代表当前连接状态。</div>
          <div v-if="!boot || boot.coreMode !== 'external'" class="empty"><div class="empty-mark">◇</div><b>尚未连接本机控制器</b><p>在首次使用页连接后才能读取真实连接。</p></div>
          <div v-else-if="!connectionPage" class="empty"><div class="empty-mark">◇</div><b>{{ connectionLoading ? '正在读取连接' : '暂无有效连接快照' }}</b></div>
          <template v-else><div class="table-wrap"><table><thead><tr><th>目标</th><th>进程</th><th>规则</th><th>链路</th><th>上传 / 下载</th></tr></thead><tbody><tr v-for="item in connectionPage.items" :key="item.id"><td>{{ item.host || item.destinationIp || '未知' }}<small v-if="item.host && item.destinationIp">{{ item.destinationIp }}</small></td><td>{{ item.processName || '未知' }}</td><td>{{ item.rule || '未知' }}</td><td>{{ item.chains.length ? item.chains.join(' → ') : '未知' }}</td><td>{{ displayBytes(item.uploadBytes) }} / {{ displayBytes(item.downloadBytes) }}</td></tr></tbody></table></div><div v-if="!connectionPage.items.length" class="empty compact">当前筛选下没有连接。</div><div class="pagination"><span>第 {{ cursorHistory.length + 1 }} 页 · 跳过无效记录 {{ connectionPage.skippedCount }} 条</span><div><button :disabled="!cursorHistory.length" @click="previousConnections">上一页</button><button :disabled="!connectionPage.hasMore" @click="nextConnections">下一页</button></div></div><p class="table-note">每页重新读取实时快照；连接变化可能改变分页顺序。超过 5 秒无成功读取时状态标记为 UNKNOWN。</p></template>
        </div></template>
        <template v-else-if="active === 'nodes'"><div class="panel"><div class="panel-title"><strong>代理节点与策略组 · 运行中配置</strong><span>{{ catalog?.observedAt ?? '尚未观测' }}</span></div>
          <p class="table-note">从当前本机 Mihomo 控制器读取；此页不切换节点或写入订阅。</p><button class="diag-run" :disabled="catalogLoading || boot?.coreMode !== 'external'" @click="loadCatalog">{{ catalogLoading ? '读取中…' : '刷新目录' }}</button>
          <div v-if="catalogError" class="alert" role="alert">{{ catalogError }}。已有数据仅是历史快照。</div><div v-if="boot?.coreMode !== 'external'" class="empty compact">请先在首次使用页连接本机控制器。</div>
          <template v-else-if="catalog"><div class="section-heading">策略组 · {{ catalog.groups.length }}</div><div class="table-wrap"><table><thead><tr><th>名称</th><th>类型</th><th>当前选择</th><th>候选节点（最多 200 个）</th></tr></thead><tbody><tr v-for="group in catalog.groups" :key="group.name"><td>{{ group.name }}</td><td>{{ group.kind }}</td><td>{{ group.selected ?? '未知' }}</td><td>{{ group.candidates.join('、') || '未知' }}</td></tr></tbody></table></div><div v-if="!catalog.groups.length" class="empty compact">未返回策略组。</div>
          <div class="section-heading">节点 · {{ catalog.nodes.length }}</div><div class="table-wrap"><table><thead><tr><th>名称</th><th>类型</th><th>内核存活字段</th><th>最近一次历史延迟</th></tr></thead><tbody><tr v-for="node in catalog.nodes" :key="node.name"><td>{{ node.name }}</td><td>{{ node.kind }}</td><td>{{ node.alive === null ? '未知' : node.alive ? '是' : '否' }}</td><td>{{ node.lastDelayMs === null ? '未知' : node.lastDelayMs + ' ms' }}</td></tr></tbody></table></div><div v-if="!catalog.nodes.length" class="empty compact">未返回节点。</div>
          <div class="section-heading">内核代理提供者 · {{ catalog.providers.length }}</div><p v-if="catalog.providerErrorCode" class="table-note unknown">提供者接口不可用：{{ catalog.providerErrorCode }}；不代表没有订阅。</p><div v-for="provider in catalog.providers" :key="provider.name" class="check-row"><span>{{ provider.name }}</span><strong>{{ provider.nodeCount }} 个节点</strong></div><p class="table-note">目录截取遗漏 {{ catalog.omittedCount }} 项；延迟来自内核历史记录，不是本应用新发起的测速。</p></template>
          <div v-else class="empty compact">{{ catalogLoading ? '正在读取目录…' : '暂无有效目录快照' }}</div></div></template>
        <template v-else-if="active === 'rules'"><div class="panel"><div class="panel-title"><strong>运行中规则 · 顺序匹配</strong><span>{{ runningRules?.observedAt ?? '尚未观测' }}</span></div><p class="table-note">读取当前内核的有序规则。静态域名模拟仅支持 DOMAIN、DOMAIN-SUFFIX、DOMAIN-KEYWORD 和 MATCH；前置规则无法解析时停止并显示“不支持模拟”。</p><button class="diag-run" :disabled="rulesLoading || boot?.coreMode !== 'external'" @click="loadRules">{{ rulesLoading ? '读取中…' : '刷新规则' }}</button>
          <form class="connections-toolbar" @submit.prevent="simulateRule"><input v-model="simulateHost" maxlength="253" placeholder="输入 ASCII 域名，如 example.com" aria-label="模拟域名" :disabled="boot?.coreMode !== 'external'"/><button :disabled="simulationLoading || boot?.coreMode !== 'external'" type="submit">{{ simulationLoading ? '模拟中…' : '模拟首个命中' }}</button></form>
          <div v-if="rulesError" class="alert" role="alert">{{ rulesError }}。已有规则仅是历史快照。</div><div v-if="simulationError" class="alert" role="alert">{{ simulationError }}</div><div v-if="simulation" class="simulation-result"><strong>{{ simulation.state === 'MATCHED' ? '首个命中' : simulation.state === 'NO_MATCH' ? '未命中' : '不支持模拟' }}</strong><span>{{ simulation.rule ? '#' + simulation.rule.index + ' ' + simulation.rule.kind + ' ' + simulation.rule.payload + ' → ' + simulation.rule.proxy : simulation.reasonCode ?? '无匹配规则' }}</span><small>范围：{{ simulation.scope }} · {{ simulation.observedAt }}；不代表实际流量路径。</small></div>
          <div v-if="boot?.coreMode !== 'external'" class="empty compact">请先在首次使用页连接本机控制器。</div><template v-else-if="runningRules"><div class="section-heading">规则 · {{ runningRules.rules.length }}</div><div class="table-wrap"><table><thead><tr><th>顺序</th><th>类型</th><th>内容</th><th>目标策略</th><th>状态</th></tr></thead><tbody><tr v-for="rule in runningRules.rules" :key="rule.index"><td>{{ rule.index }}</td><td>{{ rule.kind }}</td><td>{{ rule.payload || '—' }}</td><td>{{ rule.proxy }}</td><td>{{ rule.disabled ? '禁用' : '启用' }}</td></tr></tbody></table></div><div v-if="!runningRules.rules.length" class="empty compact">未返回规则。</div><p class="table-note">超过上限遗漏 {{ runningRules.omittedCount }} 条。规则编辑、校验、预览、确认、应用及读回仍需完成。</p></template><div v-else class="empty compact">{{ rulesLoading ? '正在读取规则…' : '暂无有效规则快照' }}</div></div></template>
        <template v-else-if="active === 'diagnostics'"><div class="panel"><div class="panel-title"><strong>本机控制器诊断</strong><span>只读 · 手动运行</span></div><p class="diag-intro">检测当前连接的 Mihomo 控制器接口是否可读。系统 DNS、外网连通性、代理和 TUN 均未检测，网络健康仍为 UNKNOWN。</p><button class="diag-run" :disabled="diagnosticLoading || boot?.coreMode !== 'external'" @click="runDiagnostics">{{ diagnosticLoading ? '正在读取…' : '运行控制器诊断' }}</button><div v-if="diagnosticError" class="alert" role="alert">{{ diagnosticError }}</div><div v-if="diagnosticReport" class="diagnostic-results"><div class="check-row" v-for="probe in diagnosticReport.probes" :key="probe.name"><span>{{ probe.name === 'version' ? '版本接口 /version' : '连接接口 /connections' }}</span><strong :class="{ unknown: probe.status !== 'AVAILABLE' }">{{ probe.status === 'AVAILABLE' ? '可读' : '不可读' }} · {{ probe.latencyMs }} ms<br/><small>{{ diagnosticReason(probe.reasonCode) }}</small></strong></div><p>观测时间：{{ diagnosticReport.observedAt }} · 本次报告仅限本机控制器；诊断结果不会自动保留为网络健康结论。</p></div><div v-else-if="boot?.coreMode !== 'external'" class="empty compact">请先在首次使用页连接本机控制器。</div></div></template>
        <template v-else><div class="panel"><div class="panel-title"><strong>{{ current.name }}</strong><span>接口接入待完成</span></div><div class="empty"><div class="empty-mark">◇</div><b>尚无真实数据</b><p>此页已纳入导航与状态框架。{{ current.detail }}将在相应 Rust IPC 和实机验收通过后开放。</p></div></div></template>
        <div class="footnote">{{ environment?.observedAt ? `环境观测时间：${environment.observedAt}` : '尚无有效环境快照' }} · 当前版本不修改代理、DNS 或路由。</div>
      </div>
    </main>
  </div>
</template>
