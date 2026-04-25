'use strict';

const HOST = 'com.bayashi.dictation_overlay';
const status = document.getElementById('status');
const position = document.getElementById('position');
const textEl = document.getElementById('text');

let port = null;
let lastMonitors = [];
let intentionalClose = false;
let lastGoodbyeReason = null;

function log(line) {
  const t = new Date().toLocaleTimeString();
  status.textContent = `[${t}] ${line}\n` + status.textContent;
}

function describePosition(msg) {
  // どのモニタにあるか + そのモニタ内ローカル座標 + CSS px
  const m = lastMonitors.find(
    mi => msg.x >= mi.x && msg.x < mi.x + mi.width
       && msg.y >= mi.y && msg.y < mi.y + mi.height
  );
  if (!m) {
    return `仮想 (${msg.x}, ${msg.y}) ${msg.width}×${msg.height}`;
  }
  const lx = msg.x - m.x;
  const ly = msg.y - m.y;
  const sf = m.scale_factor || 1;
  const cssW = Math.round(msg.width / sf);
  const cssH = Math.round(msg.height / sf);
  return `モニタ #${m.index} (${Math.round(lx / sf)}, ${Math.round(ly / sf)} CSSpx) ${cssW}×${cssH}`;
}

function connect() {
  if (port) { log('既に接続中'); return; }
  intentionalClose = false;
  lastGoodbyeReason = null;
  try {
    port = chrome.runtime.connectNative(HOST);
    log('connectNative() 呼び出し');
    port.onMessage.addListener((msg) => {
      if (!msg || !msg.type) return;
      switch (msg.type) {
        case 'ready':
          log('← ready ' + (msg.version || '') + ' caps=' + JSON.stringify(msg.capabilities || []));
          break;
        case 'monitor_list':
          lastMonitors = Array.isArray(msg.monitors) ? msg.monitors : [];
          log('← monitor_list:');
          lastMonitors.forEach((m) => {
            const star = m.is_primary ? '★' : ' ';
            log(`  ${star} #${m.index} ${m.name || ''} ${m.width}x${m.height} @ (${m.x},${m.y}) scale=${m.scale_factor}`);
          });
          break;
        case 'position_changed':
          // ログを汚さないよう専用エリアに最新値だけ表示
          position.textContent = describePosition(msg);
          break;
        case 'goodbye':
          intentionalClose = true;
          lastGoodbyeReason = msg.reason || 'unknown';
          log('← goodbye reason=' + lastGoodbyeReason + ' (意図的終了)');
          break;
        default:
          log('← ' + JSON.stringify(msg));
      }
    });
    port.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError;
      const errMsg = err && err.message ? err.message : '';
      if (intentionalClose) {
        log(`切断: ${lastGoodbyeReason || 'goodbye'} ✓ 計画的`);
      } else {
        log(`切断（予期せず）: ${errMsg || 'normal'}`);
      }
      port = null;
      intentionalClose = false;
      lastGoodbyeReason = null;
    });
  } catch (e) {
    log('connectNative 失敗: ' + (e && e.message ? e.message : String(e)));
    port = null;
  }
}

function disconnect() {
  if (!port) { log('未接続'); return; }
  try { port.disconnect(); } catch (e) { log('disconnect 失敗: ' + e); }
  port = null;
  log('disconnect() 呼び出し');
}

function send(msg) {
  if (!port) { log('未接続。先に [接続] を押してください'); return; }
  try {
    port.postMessage(msg);
    log('→ ' + JSON.stringify(msg));
  } catch (e) {
    log('送信失敗: ' + (e && e.message ? e.message : String(e)));
  }
}

document.getElementById('connect').addEventListener('click', connect);
document.getElementById('disconnect').addEventListener('click', disconnect);

document.getElementById('show').addEventListener('click', () => {
  const text = textEl.value || 'テスト字幕';
  send({
    type: 'show_caption',
    text,
    settings: {
      fontSize: 64,
      fontFamily: "'Noto Sans JP', sans-serif",
      fontWeight: 600,
      color: '#ffffff',
      bgColor: '#000000',
      bgAlpha: 70,
      shadowOn: true,
      shadowColor: '#000000',
      shadowBlur: 6,
    },
  });
});

document.getElementById('hide').addEventListener('click', () => {
  send({ type: 'hide_caption' });
});

document.getElementById('ping').addEventListener('click', () => {
  send({ type: 'ping' });
});

document.getElementById('ct-on').addEventListener('click', () => {
  send({ type: 'set_click_through', enabled: true });
});
document.getElementById('ct-off').addEventListener('click', () => {
  send({ type: 'set_click_through', enabled: false });
});

document.getElementById('list-monitors').addEventListener('click', () => {
  send({ type: 'list_monitors' });
});
document.getElementById('mon-0').addEventListener('click', () => {
  send({ type: 'set_monitor', index: 0 });
});
document.getElementById('mon-1').addEventListener('click', () => {
  send({ type: 'set_monitor', index: 1 });
});
document.getElementById('mon-2').addEventListener('click', () => {
  send({ type: 'set_monitor', index: 2 });
});

document.getElementById('exit').addEventListener('click', () => {
  send({ type: 'exit' });
});
