'use strict';

const HOST = 'com.bayashi.dictation_overlay';
const status = document.getElementById('status');
const textEl = document.getElementById('text');

let port = null;

function log(line) {
  const t = new Date().toLocaleTimeString();
  status.textContent = `[${t}] ${line}\n` + status.textContent;
}

function connect() {
  if (port) { log('既に接続中'); return; }
  try {
    port = chrome.runtime.connectNative(HOST);
    log('connectNative() 呼び出し');
    port.onMessage.addListener((msg) => {
      log('← ' + JSON.stringify(msg));
    });
    port.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError;
      log('切断: ' + (err && err.message ? err.message : 'normal'));
      port = null;
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

document.getElementById('exit').addEventListener('click', () => {
  send({ type: 'exit' });
});
