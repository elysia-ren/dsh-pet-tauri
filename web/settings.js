const TAURI = window.__TAURI__;
const invoke = TAURI.core.invoke;

const $ = (id) => document.getElementById(id);

function setStatus(text, isError = false) {
  $('save_status').textContent = text;
  $('save_status').style.color = isError ? '#ff8f8f' : '#8b93a3';
}

async function load() {
  const s = await invoke('get_settings');
  $('base_url').value = s.base_url ?? '';
  $('model').value = s.model ?? '';
  $('temperature').value = s.temperature ?? 1;
  $('pet_name').value = s.pet_name ?? '';
  $('pet_size').value = s.pet_size ?? 462;
  $('corner').value = s.corner ?? 'top-right';
  $('margin_x').value = s.margin_x ?? 24;
  $('margin_y').value = s.margin_y ?? 100;
  $('always_on_top').checked = !!s.always_on_top;
  $('autostart').checked = !!s.autostart;
  $('balance_enabled').checked = !!s.balance_enabled;
  $('balance_interval_sec').value = s.balance_interval_sec ?? 1800;
  $('balance_full').value = s.balance_full ?? 20;
  $('whisper_enabled').checked = !!s.whisper_enabled;
  $('whisper_interval_sec').value = s.whisper_interval_sec ?? 300;
  $('whisper_prompt').value = s.whisper_prompt ?? '';
  $('whisper_image_enabled').checked = !!s.whisper_image_enabled;
  $('chat_image_enabled').checked = !!s.chat_image_enabled;
  $('notifications_enabled').checked = !!s.notifications_enabled;
  $('chat_memory_rounds').value = s.chat_memory_rounds ?? 5;
  $('meme_count').textContent = (s.meme_count ?? 0) + ' 张';
  const ph = s.physics ?? {};
  $('ph_gravity').value = ph.gravity ?? 1400;
  $('ph_restitution').value = ph.restitution ?? 0.78;
  $('ph_friction').value = ph.groundFriction ?? 2.5;
  $('ph_power').value = ph.throwPower ?? 1;
  $('ph_ceiling').checked = ph.ceilingBounce !== false;
  $('ph_boundary').value = s.boundary_mode ?? 'screen';
  $('settings_path').textContent = s.settings_path ?? '';
  $('memory_path').textContent = s.memory_path ?? '';
  $('version').textContent = s.version ?? '';
  updateKeyStatus(!!s.api_key_set);
  try {
    const c = await invoke('get_custom_assets');
    $('custom_assets').textContent =
      (c.animations || []).length + ' 段动画 / ' + (c.memes || []).length + ' 张表情（animations/、memes/）';
  } catch {
    /* ignore */
  }
  await refreshMemory();
}

function updateKeyStatus(set) {
  $('key_status').textContent = set ? '已设置（本机加密保存）' : '未设置';
  $('key_status').style.color = set ? '#7fd6a4' : '#ff8f8f';
}

function collect() {
  return {
    base_url: $('base_url').value.trim(),
    model: $('model').value.trim(),
    temperature: Number($('temperature').value) || 0,
    pet_name: $('pet_name').value.trim() || '桌宠',
    pet_size: Number($('pet_size').value) || 462,
    corner: $('corner').value,
    margin_x: Number($('margin_x').value) || 0,
    margin_y: Number($('margin_y').value) || 0,
    always_on_top: $('always_on_top').checked,
    autostart: $('autostart').checked,
    whisper_prompt: $('whisper_prompt').value,
    chat_memory_rounds: Number($('chat_memory_rounds').value) || 0,
    whisper_enabled: $('whisper_enabled').checked,
    whisper_interval_sec: Number($('whisper_interval_sec').value) || 300,
    whisper_image_enabled: $('whisper_image_enabled').checked,
    chat_image_enabled: $('chat_image_enabled').checked,
    notifications_enabled: $('notifications_enabled').checked,
    balance_enabled: $('balance_enabled').checked,
    balance_interval_sec: Number($('balance_interval_sec').value) || 1800,
    balance_full: Number($('balance_full').value) || 20,
    boundary_mode: $('ph_boundary').value,
    physics: {
      gravity: Number($('ph_gravity').value) || 1400,
      restitution: Math.min(Math.max(Number($('ph_restitution').value) || 0, 0), 1),
      groundFriction: Math.max(Number($('ph_friction').value) || 0, 0),
      throwPower: Math.max(Number($('ph_power').value) || 0.1, 0.1),
      ceilingBounce: $('ph_ceiling').checked,
      petCollision: false,
    },
    // 占位：Rust 侧会保留旧密文 / memes / 位置，不会用这些字段覆盖
    api_key_enc: '',
    memes: {},
    pos_x: null,
    pos_y: null,
  };
}

async function save() {
  try {
    await invoke('save_settings', { incoming: collect() });
    setStatus('已保存 · ' + new Date().toLocaleTimeString());
    await load();
  } catch (err) {
    setStatus('保存失败：' + err, true);
  }
}

async function saveKey() {
  const key = $('api_key').value.trim();
  if (!key) {
    setStatus('API Key 不能为空', true);
    return;
  }
  try {
    await invoke('set_api_key', { key });
    $('api_key').value = '';
    updateKeyStatus(true);
    setStatus('API Key 已保存（DPAPI 加密）');
  } catch (err) {
    setStatus('保存 Key 失败：' + err, true);
  }
}

async function importDshKey() {
  try {
    await invoke('import_dsh_key');
    updateKeyStatus(true);
    setStatus('已从 DSH 凭证导入 DEEPSEEK_API_KEY（DPAPI 加密保存）');
  } catch (err) {
    setStatus('导入 Key 失败：' + err, true);
  }
}

async function clearKey() {
  if (!confirm('清除已保存的 API Key？')) return;
  try {
    await invoke('clear_api_key');
    updateKeyStatus(false);
    setStatus('API Key 已清除');
  } catch (err) {
    setStatus('清除失败：' + err, true);
  }
}

async function testLlm() {
  const btn = $('test_llm');
  btn.disabled = true;
  $('test_status').textContent = '测试中…';
  try {
    const reply = await invoke('test_llm');
    $('test_status').textContent = '连接成功：' + reply;
    $('test_status').style.color = '#7fd6a4';
  } catch (err) {
    $('test_status').textContent = '失败：' + err;
    $('test_status').style.color = '#ff8f8f';
  } finally {
    btn.disabled = false;
  }
}

async function refreshMemory() {
  try {
    const mem = await invoke('get_memory');
    $('memory_count').textContent = String(mem.count ?? 0);
  } catch {
    /* ignore */
  }
}

async function showMemory() {
  const mem = await invoke('get_memory');
  const list = (mem.messages ?? []).slice(-20);
  const text = list
    .map((m) => `${m.role === 'user' ? '我' : '桌宠'}：${m.content}`)
    .join('\n');
  const pre = $('memory_preview');
  pre.textContent = text || '（还没有对话记忆）';
  pre.classList.remove('hidden');
}

async function clearMemory() {
  if (!confirm('清空全部对话记忆？此操作不可撤销。')) return;
  try {
    await invoke('clear_memory');
    await refreshMemory();
    $('memory_preview').classList.add('hidden');
    setStatus('对话记忆已清空');
  } catch (err) {
    setStatus('清空失败：' + err, true);
  }
}

async function resetPosition() {
  try {
    await invoke('reset_position');
    setStatus('位置已重置为停靠位');
  } catch (err) {
    setStatus('重置位置失败：' + err, true);
  }
}

async function exportMemory() {
  try {
    const path = await invoke('export_memory');
    $('export_path').textContent = '已导出：' + path;
    setStatus('记忆已导出');
  } catch (err) {
    setStatus('导出失败：' + err, true);
  }
}

async function importDsh() {
  try {
    await invoke('import_dsh_config');
    await load();
    setStatus('已从 DSH 配置导入（尺寸/位置/人设/间隔）');
  } catch (err) {
    setStatus('导入失败：' + err, true);
  }
}

$('save').addEventListener('click', save);
document.querySelectorAll('.preset').forEach((btn) => {
  btn.addEventListener('click', () => {
    $('base_url').value = btn.dataset.base;
    $('model').value = btn.dataset.model;
    setStatus('已填入 ' + btn.textContent + ' 端点，记得保存');
  });
});
$('save_key').addEventListener('click', saveKey);
$('import_key').addEventListener('click', importDshKey);
$('clear_key').addEventListener('click', clearKey);
$('test_llm').addEventListener('click', testLlm);
$('show_memory').addEventListener('click', showMemory);
$('export_memory').addEventListener('click', exportMemory);
$('clear_memory').addEventListener('click', clearMemory);
$('reset_position').addEventListener('click', resetPosition);
$('import_dsh').addEventListener('click', importDsh);
$('open_dir').addEventListener('click', () => {
  invoke('open_app_dir').catch((err) => setStatus('打开文件夹失败：' + err, true));
});
$('close').addEventListener('click', () => invoke('close_settings'));

async function refreshCustomCount() {
  try {
    const c = await invoke('get_custom_assets');
    $('custom_assets').textContent =
      (c.animations || []).length + ' 段动画 / ' + (c.memes || []).length + ' 张表情（animations/、memes/）';
  } catch {
    /* ignore */
  }
}
setInterval(refreshCustomCount, 5000);

load().catch((err) => setStatus('读取设置失败：' + err, true));
