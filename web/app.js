// dsh-pet 独立桌宠渲染层
// 视频精灵 + 自主随机链（待机/转向/走动/随机动作）+ 拖拽甩抛物理 + 级联动作菜单 + 气泡/对话。
const TAURI = window.__TAURI__;
const invoke = TAURI.core.invoke;
const listen = TAURI.event.listen;

const $ = (id) => document.getElementById(id);
const stage = $('stage');
const vids = [$('va'), $('vb')];
const menuRoot = $('menu');
const menuGroups = $('menu-groups');
const menuAnims = $('menu-anims');
const bubble = $('bubble');
const bubbleText = $('bubble-text');
const bubbleImg = $('bubble-img');
const chat = $('chat');
const chatLog = $('chat-log');
const chatName = $('chat-name');
const chatForm = $('chat-form');
const chatInput = $('chat-input');
const chatSend = chatForm.querySelector('button');
const chatClose = $('chat-close');
const fx = $('fx');

const CANVAS_W = 640;
const CANVAS_H = 360;
const HIT_BOX = { x0: 200, y0: 50, x1: 440, y1: 335 };
const DRAG_THRESHOLD = 6;
const MIN_THROW_SPEED = 500;
const MAX_THROW_SPEED = 3600;

let cfg = null;
let winState = null;
let front = 0;
let facing = 'left';
let currentAnim = '';
let nextTimer = null;
let moveState = null;
let moveToken = 0;
let throwState = null;
let dragState = null;
let menuOpen = false;
let chatOpen = false;
let sending = false;
let bubbleTimer = null;
let menuCloseTimer = null;
let customAssets = { animations: [], memes: [] };
const customAnimPaths = {};
const customMemePaths = {};

async function refreshCustom() {
  try {
    const c = await invoke('get_custom_assets');
    customAssets = c;
    for (const k of Object.keys(customAnimPaths)) delete customAnimPaths[k];
    for (const k of Object.keys(customMemePaths)) delete customMemePaths[k];
    for (const a of c.animations || []) {
      customAnimPaths[a.name] = await invoke('read_custom_asset', { path: a.path });
    }
    for (const a of c.memes || []) {
      customMemePaths[a.name] = await invoke('read_custom_asset', { path: a.path });
    }
    jsLog('custom assets: ' + (c.animations || []).length + ' anim / ' + (c.memes || []).length + ' meme');
  } catch (err) {
    jsLog('custom assets failed: ' + err);
  }
}

// ---------------- 基础 ----------------
function jsLog(msg) {
  invoke('js_log', { msg: String(msg) }).catch(() => {});
}
const rand = (min, max) => min + Math.random() * (max - min);
const clamp = (v, min, max) => Math.min(Math.max(v, min), max);
const pick = (arr, exclude) => {
  const src = arr && arr.length ? arr.filter((x) => x !== exclude) : [];
  const pool = src.length ? src : arr || [];
  return pool[Math.floor(Math.random() * pool.length)];
};
const normArray = (v) => (Array.isArray(v) ? v : v === undefined || v === null ? [] : [v]);

async function refreshWindow() {
  winState = await invoke('get_window_state');
  return winState;
}

function setWin(x, y) {
  if (!winState) return;
  winState.x = x;
  winState.y = y;
  invoke('set_window_pos', { x, y }).catch(() => {});
}

function bodyOffset() {
  return {
    x: stage.offsetLeft + cfg.pet.size / 2,
    y: stage.offsetTop + stageHeight() / 2,
  };
}

function bodyCenter() {
  const off = bodyOffset();
  return { x: winState.x + off.x, y: winState.y + off.y };
}

function stageHeight() {
  return (cfg.pet.size * CANVAS_H) / CANVAS_W;
}

/** 可见身体在舞台内的偏移（px）：上下各有透明余量，碰撞要按身体算才贴边 */
function bodyBox() {
  const s = cfg.pet.size / CANVAS_W;
  return {
    left: HIT_BOX.x0 * s,
    top: HIT_BOX.y0 * s,
    right: HIT_BOX.x1 * s,
    bottom: HIT_BOX.y1 * s,
  };
}

function playArea() {
  if (cfg.boundary_mode === 'work' && winState.work) return winState.work;
  return winState.monitor;
}

function layout() {
  const w = cfg.pet.size;
  const h = stageHeight();
  stage.style.width = w + 'px';
  stage.style.height = h + 'px';
  stage.style.left = (window.innerWidth - w) / 2 + 'px';
  stage.style.top = (window.innerHeight - h) / 2 + 'px';
  positionBubble();
  positionChat();
}

function applyFacing() {
  for (const v of vids) v.classList.toggle('flip', facing === 'right');
}

async function reportHitbox() {
  const rect = stage.getBoundingClientRect();
  const s = rect.width / CANVAS_W;
  await invoke('report_hitbox', {
    x: rect.left + HIT_BOX.x0 * s,
    y: rect.top + HIT_BOX.y0 * s,
    w: (HIT_BOX.x1 - HIT_BOX.x0) * s,
    h: (HIT_BOX.y1 - HIT_BOX.y0) * s,
  });
}

// ---------------- 视频播放 ----------------
function waitFirstFrame(video, timeoutMs = 90) {
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      resolve();
    };
    if (typeof video.requestVideoFrameCallback === 'function') {
      const id = video.requestVideoFrameCallback(() => finish());
      setTimeout(() => {
        try {
          video.cancelVideoFrameCallback(id);
        } catch {
          /* ignore */
        }
        finish();
      }, timeoutMs);
    } else {
      video.addEventListener('loadeddata', finish, { once: true });
      setTimeout(finish, timeoutMs);
    }
  });
}

function frontVideo() {
  return vids[front];
}

async function play(name, opts = {}) {
  const { loop = false, onEnded = null, autoNext = true } = opts;
  const idx = 1 - front;
  const back = vids[idx];
  const fore = vids[front];
  back.onended = null;
  back.loop = loop;
  back.src = customAnimPaths[name] || 'assets/webm/' + encodeURIComponent(name) + '.webm';
  back.currentTime = 0;
  try {
    await back.play();
  } catch (err) {
    jsLog('play failed: ' + name + ' ' + err);
    return false;
  }
  await waitFirstFrame(back);
  back.classList.add('front');
  fore.classList.remove('front');
  front = idx;
  currentAnim = name;
  setTimeout(() => {
    if (fore !== vids[front]) fore.pause();
  }, 180);
  if (!loop) {
    back.onended = () => {
      back.onended = null;
      back.pause();
      if (onEnded) onEnded();
      else if (autoNext) pickNext();
    };
  }
  return true;
}

function playIdle() {
  const pool = normArray(cfg.animations.idle);
  if (!pool.length) return;
  const name = pick(pool, currentAnim);
  void play(name, { loop: true, autoNext: false });
  scheduleNext(rand(4500, 8500));
}

function playEvent(name) {
  if (!name) return;
  stopMove();
  stopThrow();
  clearTimeout(nextTimer);
  void play(name, { onEnded: () => playIdle() });
}

function playClick() {
  const pool = normArray(cfg.animations.clicks);
  if (!pool.length) return playIdle();
  void play(pick(pool, currentAnim), { onEnded: () => playIdle() }).then((ok) => {
    if (ok) bounce();
  });
}

let squashTimer = null;
function bounce(strength = 1) {
  stage.classList.remove('squash');
  void stage.offsetWidth;
  stage.style.setProperty('--squash-depth', String(strength));
  stage.classList.add('squash');
  clearTimeout(squashTimer);
  squashTimer = setTimeout(() => stage.classList.remove('squash'), 480);
}

/** 点击粒子：对齐原版点击反馈的轻量版 */
function spawnBurst(x, y) {
  const count = 10;
  for (let i = 0; i < count; i++) {
    const p = document.createElement('div');
    p.className = 'fx-particle';
    const angle = (Math.PI * 2 * i) / count + Math.random() * 0.4;
    const dist = 26 + Math.random() * 40;
    p.style.left = x + 'px';
    p.style.top = y + 'px';
    p.style.setProperty('--fx-dx', Math.cos(angle) * dist + 'px');
    p.style.setProperty('--fx-dy', Math.sin(angle) * dist + 'px');
    fx.appendChild(p);
    setTimeout(() => p.remove(), 700);
  }
}

// ---------------- 随机链 ----------------
function scheduleNext(delay) {
  clearTimeout(nextTimer);
  nextTimer = setTimeout(() => {
    void pickNext();
  }, delay);
}

function weightedCategory() {
  const cats = (cfg.animations.categories || []).filter((c) => (c.actions || []).length);
  if (!cats.length) return null;
  const eligible = cats.filter((c) => !(c.noMirror && facing === 'right'));
  const pool = eligible.length ? eligible : cats;
  const total = pool.reduce((s, c) => s + (c.weight || 1), 0);
  let t = Math.random() * total;
  for (const c of pool) {
    t -= c.weight || 1;
    if (t <= 0) return c;
  }
  return pool[pool.length - 1];
}

function categoryAction() {
  const cat = weightedCategory();
  if (!cat) return null;
  return pick(cat.actions, currentAnim);
}

async function pickNext() {
  if (!cfg) return;
  if (dragState || throwState || moveState || chatOpen) {
    scheduleNext(3000);
    return;
  }
  const w = cfg.animation_weights || {};
  const idleW = (w.idle ?? 10) / 100;
  const turnW = (w.turn ?? 5) / 100;
  // 桌宠自己走动是「活泼」的主要来源：配置权重只有 5% 时太罕见，这里保底 20%
  const moveW = Math.max((w.move ?? 5) / 100, 0.2);
  const roll = Math.random();
  if (roll < idleW) {
    playIdle();
    return;
  }
  if (roll < idleW + turnW) {
    facing = facing === 'right' ? 'left' : 'right';
    applyFacing();
    const pool = normArray(cfg.animations.turn);
    if (pool.length) void play(pick(pool, currentAnim), { onEnded: () => playIdle() });
    else playIdle();
    return;
  }
  if (roll < idleW + turnW + moveW) {
    const ok = await tryWalk();
    if (!ok) {
      const act = categoryAction();
      if (act) void play(act, { onEnded: () => pickNext() });
      else playIdle();
    }
    return;
  }
  const act = categoryAction();
  if (act) void play(act, { onEnded: () => pickNext() });
  else playIdle();
}

// ---------------- 自主走动 ----------------
function isMoveAction(name) {
  const actions = cfg.animations.moves?.actions || [];
  return actions.some((a) => a.name === name);
}

function stopMove() {
  moveToken++;
  moveState = null;
}

async function tryWalk(preferredName) {
  if (moveState || throwState || dragState) return false;
  const moves = cfg.animations.moves;
  if (!moves || !Array.isArray(moves.actions) || !moves.actions.length) return false;
  const chosen = preferredName
    ? moves.actions.find((a) => a.name === preferredName)
    : moves.actions[Math.floor(Math.random() * moves.actions.length)];
  if (!chosen) return false;
  const defaults = moves.default || {};
  const mp = Object.assign({}, defaults, chosen.params || {});
  const size = cfg.pet.size;
  const scale = size / 462;
  const bodyHalf = size / 2;
  const center = bodyCenter();
  const area = playArea();
  const margin = mp.margin ?? 20;
  const b = bodyBox();
  const left = area.x + margin - b.left + bodyHalf;
  const right = area.x + area.w - margin - b.right + bodyHalf;
  // 动画本身自带方向（如「原地左转奔跑」的画面就是朝左跑），不能随机反着走
  const intrinsic = chosen.name.includes('左') ? -1 : chosen.name.includes('右') ? 1 : 0;
  let dir = intrinsic || (Math.random() < 0.5 ? -1 : 1);
  const distance = rand((mp.minDist ?? 60) * scale, (mp.maxDist ?? 240) * scale);
  let target = center.x + dir * distance;
  if (target < left || target > right) {
    if (intrinsic) {
      target = clamp(target, left, right);
    } else {
      dir = -dir;
      target = center.x + dir * distance;
    }
  }
  target = clamp(target, left, right);
  if (Math.abs(target - center.x) < 20) return false;

  facing = dir > 0 ? 'right' : 'left';
  applyFacing();
  const token = ++moveToken;
  moveState = { token };
  if (!(await play(chosen.name, { autoNext: false }))) {
    moveState = null;
    return false;
  }
  const video = frontVideo();
  const duration = Number.isFinite(video.duration) && video.duration > 0 ? video.duration : 10.09;
  const lead = mp.leadSec ?? 2;
  const tail = mp.tailSec ?? 2;
  const travel = Math.max(0.1, duration - lead - tail);
  const startX = center.x;
  const winY = winState.y;
  const offset = bodyOffset();

  await new Promise((resolve) => {
    const step = () => {
      if (moveState?.token !== token) {
        resolve();
        return;
      }
      const t = video.currentTime || 0;
      let cx;
      if (t <= lead) cx = startX;
      else if (t >= duration - tail) cx = target;
      else cx = startX + (target - startX) * ((t - lead) / travel);
      setWin(cx - offset.x, winY);
      if (t < duration - tail) requestAnimationFrame(step);
      else resolve();
    };
    requestAnimationFrame(step);
  });

  if (moveState?.token !== token) return true;
  moveState = null;
  invoke('save_position').catch(() => {});
  playIdle();
  return true;
}

// ---------------- 拖拽 / 甩抛 ----------------
function setDrag(active) {
  invoke('set_drag', { active }).catch(() => {});
}
function setUiBusy(active) {
  invoke('set_ui_busy', { active }).catch(() => {});
}

function estimateThrow(state) {
  const trail = state?.trail || [];
  const now = performance.now();
  const win = trail.filter((s) => now - s.t <= 150);
  if (win.length < 2) return null;
  const a = win[0];
  const b = win[win.length - 1];
  const dt = (b.t - a.t) / 1000;
  if (dt < 0.02) return null;
  let vx = (b.x - a.x) / dt;
  let vy = (b.y - a.y) / dt;
  const speed = Math.hypot(vx, vy);
  if (speed < MIN_THROW_SPEED) return null;
  const factor = speed > MAX_THROW_SPEED ? MAX_THROW_SPEED / speed : 1;
  vx *= factor;
  vy *= factor;
  return { vx, vy };
}

function startThrow(vel) {
  const p = cfg.physics || {};
  const gravity = p.gravity ?? 1400;
  const restitution = p.restitution ?? 0.78;
  const friction = p.groundFriction ?? 2.5;
  const power = p.throwPower ?? 1;
  const mon = playArea();
  const stageOff = { x: stage.offsetLeft, y: stage.offsetTop };
  const b = bodyBox();
  // 边界按可见身体算：身体边缘贴屏幕边才反弹，不再留透明余量造成的缝
  const bounds = {
    minX: mon.x - b.left - stageOff.x,
    maxX: mon.x + mon.w - b.right - stageOff.x,
    minY: mon.y - b.top - stageOff.y,
    maxY: mon.y + mon.h - b.bottom - stageOff.y,
  };
  throwState = {
    x: winState.x,
    y: winState.y,
    vx: vel.vx * power,
    vy: vel.vy * power,
  };
  // 飞行途中用待机姿势：拖拽动画的身体比待机高约 15px，落地再切回来会“忽然往下掉一下”
  const idlePool = normArray(cfg.animations.idle);
  if (idlePool.length) {
    void play(pick(idlePool, currentAnim), { loop: true, autoNext: false });
  }
  let last = performance.now();
  let landed = false;
  const step = (now) => {
    if (!throwState) return;
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    const s = throwState;
    s.vy += gravity * dt;
    s.x += s.vx * dt;
    s.y += s.vy * dt;
    if (s.x < bounds.minX) {
      s.x = bounds.minX;
      s.vx = -s.vx * restitution;
    } else if (s.x > bounds.maxX) {
      s.x = bounds.maxX;
      s.vx = -s.vx * restitution;
    }
    if (s.y < bounds.minY) {
      s.y = bounds.minY;
      s.vy = -s.vy * restitution;
    }
    if (s.y >= bounds.maxY) {
      s.y = bounds.maxY;
      const impact = Math.abs(s.vy);
      if (impact > 250 && !landed) {
        landed = true;
        bounce(clamp(1 - impact / 3000, 0.45, 1));
        spawnBurst(stage.offsetLeft + cfg.pet.size / 2, stage.offsetTop + stageHeight() * 0.8);
      }
      s.vy = impact < 60 ? 0 : -s.vy * restitution;
      s.vx *= Math.exp(-friction * dt);
    }
    setWin(s.x, s.y);
    const settled =
      s.y >= bounds.maxY - 0.5 && Math.abs(s.vx) < 15 && Math.abs(s.vy) < 10;
    if (settled) {
      throwState = null;
      invoke('save_position').catch(() => {});
      playIdle();
      return;
    }
    requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function stopThrow() {
  throwState = null;
}

function pointerDown(e) {
  if (!cfg || e.button !== 0 || menuOpen || chatOpen) return;
  e.preventDefault();
  invoke('focus_pet').catch(() => {});
  stopMove();
  // 飞行中也能直接抓住（原版行为）：停下抛掷，立刻进入拖拽
  const wasFlying = !!throwState;
  stopThrow();
  if (wasFlying) jsLog('grab mid-flight');
  clearTimeout(nextTimer);
  facing = e.clientX < window.innerWidth / 2 ? 'left' : 'right';
  applyFacing();
  dragState = {
    startX: e.screenX,
    startY: e.screenY,
    baseX: winState.x,
    baseY: winState.y,
    targetX: winState.x,
    targetY: winState.y,
    x: e.screenX,
    y: e.screenY,
    moved: false,
    started: false,
    pointerId: e.pointerId,
    trail: [{ t: performance.now(), x: e.screenX, y: e.screenY }],
    raf: 0,
  };
  setDrag(true);
  const tick = () => {
    if (!dragState) return;
    setWin(dragState.targetX, dragState.targetY);
    dragState.raf = requestAnimationFrame(tick);
  };
  dragState.raf = requestAnimationFrame(tick);
  try {
    stage.setPointerCapture(e.pointerId);
  } catch {
    /* ignore */
  }
}

function pointerMove(e) {
  if (!dragState) return;
  const dx = e.screenX - dragState.x;
  const dy = e.screenY - dragState.y;
  if (!dragState.started && Math.abs(dx) + Math.abs(dy) <= DRAG_THRESHOLD) return;
  if (!dragState.started) {
    dragState.started = true;
    dragState.moved = true;
    document.body.classList.add('dragging');
    const dragAnim = normArray(cfg.animations.drag)[0] || cfg.animations.drag;
    void play(dragAnim, { loop: true, autoNext: false });
  }
  dragState.x = e.screenX;
  dragState.y = e.screenY;
  // 目标位置由「按下时窗口位置 + 指针屏幕位移」直接得到；rAF 里每帧写一次窗口，不走 IPC 风暴
  dragState.targetX = dragState.baseX + (e.screenX - dragState.startX);
  dragState.targetY = dragState.baseY + (e.screenY - dragState.startY);
  const now = performance.now();
  dragState.trail.push({ t: now, x: e.screenX, y: e.screenY });
  dragState.trail = dragState.trail.filter((s) => now - s.t <= 250);
}

function resetDrag(pointerId) {
  if (!dragState) return null;
  const state = dragState;
  if (state.raf) cancelAnimationFrame(state.raf);
  dragState = null;
  document.body.classList.remove('dragging');
  setDrag(false);
  if (pointerId !== undefined) {
    try {
      stage.releasePointerCapture(pointerId);
    } catch {
      /* ignore */
    }
  }
  return state;
}

function pointerUp(e) {
  if (!dragState) return;
  const state = resetDrag(e.pointerId);
  if (state.moved) {
    const vel = estimateThrow(state);
    if (vel) {
      // 刷新一次几何：拖到另一块屏时用当前显示器的边界
      refreshWindow().then(
        () => startThrow(vel),
        () => startThrow(vel),
      );
    } else {
      invoke('save_position').catch(() => {});
      playIdle();
    }
  } else {
    spawnBurst(e.clientX, e.clientY);
    playClick();
  }
}

function pointerCancel(e) {
  if (!dragState) return;
  resetDrag(e.pointerId);
  playIdle();
}

async function onDragCancel() {
  if (!dragState) return;
  resetDrag();
  playIdle();
}

// ---------------- 菜单 ----------------
function menuItem(label, onClick, arrow = false) {
  const el = document.createElement('div');
  el.className = 'menu-item';
  const text = document.createElement('span');
  text.textContent = label;
  el.appendChild(text);
  if (arrow) {
    const a = document.createElement('span');
    a.className = 'arrow';
    a.textContent = '▸';
    el.appendChild(a);
  }
  if (onClick) el.addEventListener('click', onClick);
  return el;
}

function menuSeparator() {
  const el = document.createElement('div');
  el.className = 'menu-sep';
  return el;
}

function closeMenus() {
  menuOpen = false;
  for (const el of [menuRoot, menuGroups, menuAnims]) el.classList.add('hidden');
  setUiBusy(chatOpen);
  clearTimeout(menuCloseTimer);
}

function ensureMenuClose() {
  // 级联菜单不做“鼠标离开自动收起”：点菜单外/Esc 才关，避免从一级移到二级时被误关
  clearTimeout(menuCloseTimer);
}

for (const el of [menuRoot, menuGroups, menuAnims]) {
  el.addEventListener('mouseenter', () => clearTimeout(menuCloseTimer));
}

function placePanel(el, x, y) {
  el.classList.remove('hidden');
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  el.style.left = clamp(x, 4, window.innerWidth - w - 4) + 'px';
  el.style.top = clamp(y, 4, window.innerHeight - h - 4) + 'px';
}

function placeBeside(el, anchor) {
  el.classList.remove('hidden');
  const a = anchor.getBoundingClientRect();
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  let x = a.right - 2;
  if (x + w > window.innerWidth - 4) x = a.left - w + 2;
  let y = a.top;
  if (y + h > window.innerHeight - 4) y = window.innerHeight - h - 4;
  el.style.left = clamp(x, 4, window.innerWidth - w - 4) + 'px';
  el.style.top = clamp(y, 4, window.innerHeight - h - 4) + 'px';
}

function menuGroupsData() {
  const a = cfg.animations;
  const groups = [];
  const add = (label, arr) => {
    const items = normArray(arr).filter(Boolean);
    if (items.length) groups.push({ label, items });
  };
  add('待机', a.idle);
  add('转向', a.turn);
  add('拖拽', a.drag);
  add('点击回应', a.clicks);
  add('移动', (a.moves?.actions || []).map((m) => m.name));
  for (const cat of a.categories || []) add(cat.id, cat.actions);
  for (const [key, pool] of Object.entries(a.events || {})) {
    const names = [];
    for (const slot of normArray(pool)) names.push(...normArray(slot));
    add({ balance: '余额档位', whisper: '碎碎念', workStatus: '工作状态' }[key] || key, names);
  }
  // 0.4.5 新增的独立状态池
  if (a.sleep) {
    const names = [];
    for (const v of Object.values(a.sleep)) names.push(...normArray(v));
    add('睡觉', names);
  }
  add('干饭', a.baifan);
  if (customAssets.animations.length) {
    add('自定义动画', customAssets.animations.map((a) => a.name));
  }
  return groups;
}

function showGroups() {
  menuGroups.innerHTML = '';
  const groups = menuGroupsData();
  for (const group of groups) {
    const item = menuItem(group.label, null, true);
    item.addEventListener('mouseenter', () => showAnims(group, item));
    item.addEventListener('click', () => showAnims(group, item));
    menuGroups.appendChild(item);
  }
  placeBeside(menuGroups, menuRoot);
}

function showAnims(group, anchor) {
  menuAnims.innerHTML = '';
  for (const name of group.items) {
    menuAnims.appendChild(
      menuItem(name, () => {
        closeMenus();
        playMenuAnim(name);
      }),
    );
  }
  placeBeside(menuAnims, anchor);
}

function playMenuAnim(name) {
  stopMove();
  stopThrow();
  clearTimeout(nextTimer);
  if (isMoveAction(name)) {
    void tryWalk(name).then((ok) => {
      if (!ok) void play(name, { onEnded: () => playIdle() });
    });
    return;
  }
  if (normArray(cfg.animations.idle).includes(name)) {
    void play(name, { loop: true, autoNext: false });
    scheduleNext(rand(6000, 12000));
    return;
  }
  void play(name, { onEnded: () => playIdle() });
}

async function actionBalance() {
  try {
    await invoke('refresh_balance');
  } catch (err) {
    showBubble(String(err), 'error', 8000);
  }
}

async function actionWhisper() {
  playThinking();
  try {
    await invoke('whisper_now');
  } catch (err) {
    playIdle();
    showBubble(String(err), 'error', 8000);
  }
}

async function actionHome() {
  try {
    await invoke('reset_position');
    await refreshWindow();
    playIdle();
  } catch (err) {
    showBubble(String(err), 'error', 6000);
  }
}

async function toggleTop() {
  try {
    const value = await invoke('toggle_always_on_top');
    showBubble(value ? '已置顶' : '已取消置顶', 'chat', 3000);
  } catch (err) {
    showBubble(String(err), 'error', 6000);
  }
}

function openMenu(x, y) {
  clearTimeout(nextTimer);
  void refreshCustom();
  menuRoot.innerHTML = '';
  menuRoot.appendChild(menuItem('设置…', () => { closeMenus(); void invoke('open_settings'); }));
  menuRoot.appendChild(menuItem('刷新余额', () => { closeMenus(); void actionBalance(); }));
  menuRoot.appendChild(menuItem('碎碎念', () => { closeMenus(); void actionWhisper(); }));
  menuRoot.appendChild(menuItem('对话…', () => { closeMenus(); void openChat(); }));
  const act = menuItem('动作', null, true);
  act.addEventListener('mouseenter', showGroups);
  act.addEventListener('click', showGroups);
  menuRoot.appendChild(act);
  menuRoot.appendChild(menuSeparator());
  menuRoot.appendChild(menuItem('回家', () => { closeMenus(); void actionHome(); }));
  menuRoot.appendChild(menuItem('切换置顶', () => { closeMenus(); void toggleTop(); }));
  menuRoot.appendChild(menuSeparator());
  menuRoot.appendChild(menuItem('退出', () => { closeMenus(); void invoke('quit'); }));
  placePanel(menuRoot, x, y);
  menuGroups.classList.add('hidden');
  menuAnims.classList.add('hidden');
  menuOpen = true;
  setUiBusy(true);
}

// ---------------- 气泡 ----------------
function deepseekTier() {
  const now = new Date();
  const bj = new Date(now.getTime() + 8 * 3600 * 1000);
  const day = bj.getUTCDay();
  const hour = bj.getUTCHours();
  if (day === 0 || day === 6) return 'idle';
  return (hour >= 9 && hour < 12) || (hour >= 14 && hour < 18) ? 'peak' : 'idle';
}

function positionBubble() {
  if (bubble.classList.contains('hidden')) return;
  const left = stage.offsetLeft + cfg.pet.size / 2;
  let top = stage.offsetTop - bubble.offsetHeight - 14;
  if (top < 6) top = stage.offsetTop + stageHeight() + 14;
  bubble.style.left = left + 'px';
  bubble.style.top = top + 'px';
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function showBubble(text, kind = 'chat', duration = 10000, image = null) {
  if (!text && !image) return;
  if (kind === 'balance' && text) {
    const m = text.match(/^余额\s*(.*)$/);
    const value = m ? m[1] : text;
    const tier = deepseekTier();
    bubbleText.innerHTML =
      '<div class="bub-title">余额</div><div class="bub-val">' +
      escapeHtml(value) +
      '<span class="bub-tier ' +
      tier +
      '">' +
      (tier === 'peak' ? '峰' : '谷') +
      '</span></div>';
  } else {
    bubbleText.textContent = text || '';
  }
  if (image) {
    bubbleImg.src =
      customMemePaths[image] || 'assets/memes/' + encodeURIComponent(image) + '.png';
    bubbleImg.classList.remove('hidden');
  } else {
    bubbleImg.removeAttribute('src');
    bubbleImg.classList.add('hidden');
  }
  bubble.classList.remove('hidden');
  bubble.classList.toggle('kind-error', kind === 'error');
  bubble.style.maxWidth = image ? '240px' : '320px';
  positionBubble();
  clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => bubble.classList.add('hidden'), duration);
}

bubbleImg.addEventListener('load', positionBubble);

function playThinking() {
  if (cfg.thinking) void play(cfg.thinking, { loop: true, autoNext: false });
}

// ---------------- 对话 ----------------
function positionChat() {
  if (chat.classList.contains('hidden')) return;
  const w = chat.offsetWidth || 340;
  const h = chat.offsetHeight || 440;
  const left = clamp(stage.offsetLeft + cfg.pet.size + 16, 12, window.innerWidth - w - 12);
  const top = clamp(stage.offsetTop + 40, 12, window.innerHeight - h - 12);
  chat.style.left = left + 'px';
  chat.style.top = top + 'px';
}

function addMsg(role, text) {
  const el = document.createElement('div');
  el.className = 'msg ' + role;
  el.textContent = text;
  chatLog.appendChild(el);
  chatLog.scrollTop = chatLog.scrollHeight;
  return el;
}

function renderMemory(messages) {
  chatLog.innerHTML = '';
  if (!messages || messages.length === 0) {
    addMsg('system', '还没有对话记忆，说点什么吧～');
    return;
  }
  for (const m of messages) {
    addMsg(m.role === 'user' ? 'user' : 'assistant', String(m.content ?? ''));
  }
}

async function openChat() {
  closeMenus();
  chatOpen = true;
  chat.classList.remove('hidden');
  setUiBusy(true);
  positionChat();
  try {
    await invoke('focus_pet');
  } catch {
    /* ignore */
  }
  chatInput.focus();
  try {
    const mem = await invoke('get_memory');
    renderMemory(mem.messages);
  } catch (err) {
    addMsg('system', '读取记忆失败：' + err);
  }
}

function closeChat() {
  if (!chatOpen) return;
  chatOpen = false;
  chat.classList.add('hidden');
  setUiBusy(menuOpen);
  scheduleNext(1500);
}

chatClose.addEventListener('click', closeChat);

chatForm.addEventListener('submit', async (e) => {
  e.preventDefault();
  const text = chatInput.value.trim();
  if (!text || sending) return;
  chatInput.value = '';
  if (!chatLog.querySelector('.msg.user, .msg.assistant')) chatLog.innerHTML = '';
  addMsg('user', text);
  const typing = addMsg('system', '……');
  sending = true;
  chatSend.disabled = true;
  playThinking();
  try {
    const reply = await invoke('chat_send', { text });
    typing.remove();
    playIdle();
    addMsg('assistant', reply);
  } catch (err) {
    typing.remove();
    playIdle();
    addMsg('system', '失败：' + err);
    showBubble(String(err), 'error', 8000);
  } finally {
    sending = false;
    chatSend.disabled = false;
    chatInput.focus();
  }
});

// ---------------- 启动 / 事件 ----------------
stage.addEventListener('pointerdown', pointerDown);
stage.addEventListener('pointermove', pointerMove);
stage.addEventListener('pointerup', pointerUp);
stage.addEventListener('pointercancel', pointerCancel);

document.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  if (chatOpen) return;
  openMenu(e.clientX, e.clientY);
});

document.addEventListener('click', (e) => {
  if (!menuOpen) return;
  if (
    !menuRoot.contains(e.target) &&
    !menuGroups.contains(e.target) &&
    !menuAnims.contains(e.target)
  ) {
    closeMenus();
  }
});

document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  closeMenus();
  if (chatOpen) closeChat();
});

listen('pet://say', (e) =>
  showBubble(e.payload.text, e.payload.kind, undefined, e.payload.image || null),
).catch(() => {});
listen('pet://play', (e) => playEvent(e.payload.name)).catch(() => {});
listen('pet://config', async (e) => {
  cfg = e.payload;
  chatName.textContent = cfg.pet.name || '桌宠';
  layout();
  await refreshWindow();
  await reportHitbox();
  if (!dragState && !throwState && !moveState) playIdle();
}).catch(() => {});
listen('pet://drag-cancel', onDragCancel).catch(() => {});

window.addEventListener('resize', () => {
  if (!cfg) return;
  layout();
  reportHitbox();
});

document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    vids.forEach((v) => v.pause());
  } else if (cfg) {
    playIdle();
  }
});

async function boot() {
  cfg = await invoke('get_config');
  await refreshCustom();
  chatName.textContent = cfg.pet.name || '桌宠';
  layout();
  await refreshWindow();
  await reportHitbox();

  const pool = normArray(cfg.animations.idle);
  const first = pool[0] || '待机呼吸休闲';
  const a = vids[0];
  a.loop = true;
  a.src = 'assets/webm/' + encodeURIComponent(first) + '.webm';
  a.classList.add('front');
  currentAnim = first;
  try {
    await a.play();
  } catch (err) {
    jsLog('idle play failed: ' + err);
  }
  front = 0;
  applyFacing();

  await reportHitbox();
  await invoke('ready');
  scheduleNext(rand(4000, 8000));

  try {
    const s = await invoke('get_settings');
    if (!s.api_key_set) {
      setTimeout(() => {
        showBubble('右键 → 设置，填 API Key 就能聊天啦；也可以点「从 DSH 导入」～', 'chat', 12000);
      }, 1500);
    }
  } catch {
    /* ignore */
  }
}

boot().catch((err) => jsLog('boot failed: ' + (err && err.message ? err.message : err)));
