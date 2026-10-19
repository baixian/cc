<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { gatewayApi, GatewayIpcError } from './ipc';
import type { BootstrapDto, CoreSnapshotDto, EnvironmentDto } from './generated/contracts';
import { pages, type PageKey } from './pages';

const active = ref<PageKey>('overview');
const boot = ref<BootstrapDto | null>(null);
const environment = ref<EnvironmentDto | null>(null);
const snapshot = ref<CoreSnapshotDto | null>(null);
const endpoint = ref('http://127.0.0.1:9090');
const error = ref('');
const coreError = ref('');
const loading = ref(false);
const current = computed(() => pages.find(p => p.key === active.value)!);
function explain(cause: unknown, fallback: string) {
  return cause instanceof GatewayIpcError ? `${cause.code}：${cause.message}` : fallback;
}
async function refresh() {
  loading.value = true;
  error.value = '';
  try {
    const [b, e] = await Promise.all([gatewayApi.bootstrap(), gatewayApi.environment()]);
    boot.value = b;
    environment.value = e;
    snapshot.value = null;
    coreError.value = '';
    if (b.coreMode === 'external') {
      try { snapshot.value = await gatewayApi.coreSnapshot(); }
      catch (cause) { coreError.value = explain(cause, '内核快照读取失败'); }
    }
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
  try { await gatewayApi.disconnect(); await refresh(); }
  catch (cause) { error.value = explain(cause, '断开失败'); }
  finally { loading.value = false; }
}
onMounted(refresh);
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
      <header class="topbar"><div class="breadcrumb">工作台 <span>/</span> {{ current.name }}</div><div class="top-actions"><span class="mode">{{ boot?.coreMode ?? '未选择模式' }}</span><span class="status">{{ coreError ? '状态未知' : boot?.coreState ?? '未连接' }}</span><button class="refresh" :disabled="loading" @click="refresh">{{ loading ? '读取中' : '刷新状态' }}</button></div></header>
      <div class="content">
        <div class="heading"><div><div class="eyebrow">{{ current.id }} / NETWORK CONTROL</div><h1>{{ current.name }}</h1><p>{{ current.detail }}</p></div><span class="badge">开发中 · 无网络写入能力</span></div>
        <div v-if="error" class="alert" role="alert">{{ error }}</div>
        <div v-if="coreError" class="alert" role="alert">{{ coreError }}。上次连接不代表内核现在可用。</div>
        <template v-if="active === 'overview'">
          <div class="cards"><div class="card"><div class="card-label">内核状态</div><strong>{{ snapshot?.state ?? '未知' }}</strong><small>来源：{{ snapshot ? 'Mihomo API' : '尚无有效快照' }}</small></div><div class="card"><div class="card-label">网络入口</div><strong>{{ boot?.ingress ?? '未知' }}</strong><small>未验证 TUN / 系统代理</small></div><div class="card"><div class="card-label">实时连接</div><strong>{{ snapshot?.connectionCount ?? '未知' }}</strong><small>{{ snapshot?.observedAt ?? '等待读取' }}</small></div><div class="card"><div class="card-label">网络健康</div><strong class="unknown">UNKNOWN</strong><small>连接数不代表健康检测通过</small></div></div>
          <div class="panel topology"><div class="panel-title"><strong>网络拓扑</strong><span>{{ snapshot ? `内核 ${snapshot.coreVersion}` : '真实连接采集待接入' }}</span></div><div class="empty"><div class="empty-mark">◎</div><b>{{ snapshot ? `当前观测到 ${snapshot.connectionCount} 条连接` : '暂无可验证的连接数据' }}</b><p>连接计数来自本机控制器；进程识别、路由链路与动态拓扑将在采集器实现后展示。</p></div></div>
        </template>
        <template v-else-if="active === 'onboarding'"><div class="panel"><div class="panel-title"><strong>首次使用 · 环境扫描</strong><span>只读接入</span></div><div class="check-row"><span>操作系统</span><strong>{{ environment?.osVersion ?? '未知' }}</strong></div><div class="check-row"><span>CPU 架构</span><strong>{{ environment?.architecture ?? '未知' }}</strong></div><div class="check-row"><span>网络检查</span><strong class="unknown">{{ environment?.networkStatus ?? 'UNKNOWN' }}</strong></div><form class="connect-form" @submit.prevent="connect"><label for="endpoint">本机 Mihomo 控制器地址（无需 Secret 的控制器）</label><div><input id="endpoint" v-model="endpoint" placeholder="http://127.0.0.1:9090" :disabled="loading"/><button :disabled="loading" type="submit">连接只读内核</button><button v-if="boot?.coreMode === 'external'" :disabled="loading" type="button" @click="disconnect">断开</button></div><small>仅连接 127.0.0.1 或 ::1；不修改现有 Clash 配置、系统代理或 TUN。需要 Secret 的控制器会明确报错。</small></form><div class="notice">托管模式的配置预览、预检、系统授权及读回健康尚未实现，当前不能完成托管向导。</div></div></template>
        <template v-else><div class="panel"><div class="panel-title"><strong>{{ current.name }}</strong><span>接口接入待完成</span></div><div class="empty"><div class="empty-mark">◇</div><b>尚无真实数据</b><p>此页已纳入导航与状态框架。{{ current.detail }}将在相应 Rust IPC 和实机验收通过后开放。</p></div></div></template>
        <div class="footnote">{{ environment?.observedAt ? `环境观测时间：${environment.observedAt}` : '尚无有效环境快照' }} · 当前版本不修改代理、DNS 或路由。</div>
      </div>
    </main>
  </div>
</template>
