(function () {
  'use strict';

  const caption = document.getElementById('caption');
  if (!caption) return;

  // ---- Transition (v0.3.6 〜) -------------------------------------------
  // dictation-beta のストリームモードで text が 150〜800ms 間隔で連発される。
  // テキスト全更新時に CSS アニメーションで「入ってくる感」を出す。
  // settings.transition で 5 種類の挙動を選べる。
  const TRANSITION_CLASSES = {
    'fade': 'anim-fade',
    'slide-right': 'anim-slide-right',
    'slide-left': 'anim-slide-left',
    'scroll-up': 'anim-scroll-up',
  };
  let currentTransition = 'none'; // backward compat: default OFF
  let lastText = '';

  function hexToRgba(hex, alpha) {
    const m = /^#?([0-9a-fA-F]{6})$/.exec(String(hex || ''));
    if (!m) return `rgba(0,0,0,${alpha})`;
    const n = parseInt(m[1], 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`;
  }

  function applySettings(s) {
    if (!s || typeof s !== 'object') return;
    if (s.fontSize) caption.style.fontSize = s.fontSize + 'px';
    if (s.fontFamily) caption.style.fontFamily = s.fontFamily;
    if (s.fontWeight !== undefined) caption.style.fontWeight = s.fontWeight;
    if (s.color) caption.style.color = s.color;
    if (s.bgColor || s.bgAlpha !== undefined) {
      const alpha = (s.bgAlpha !== undefined ? Number(s.bgAlpha) : 70) / 100;
      caption.style.background = hexToRgba(s.bgColor || '#000000', alpha);
    }
    if (s.shadowOn) {
      const blur = s.shadowBlur !== undefined ? Number(s.shadowBlur) : 6;
      caption.style.textShadow = `0 0 ${blur}px ${s.shadowColor || '#000000'}`;
    } else if (s.shadowOn === false) {
      caption.style.textShadow = 'none';
    }
    if (s.strokeOn && s.strokeColor && s.strokeWidth) {
      caption.style.webkitTextStroke = `${s.strokeWidth}px ${s.strokeColor}`;
    } else if (s.strokeOn === false) {
      caption.style.webkitTextStroke = '0';
    }
    if (s.lineHeightTenth) {
      caption.style.lineHeight = (Number(s.lineHeightTenth) / 10).toString();
    }
    // v0.3.6: transition kind
    if (typeof s.transition === 'string') {
      if (s.transition === 'none' || TRANSITION_CLASSES[s.transition]) {
        currentTransition = s.transition;
      }
    }
  }

  function setText(text) {
    if (typeof text !== 'string') return;
    // 同じテキストの再送（スタイルだけ変えたい時など）はアニメーション不要
    const changed = text !== lastText;
    lastText = text;
    caption.textContent = text;
    if (!changed) return;
    if (currentTransition === 'none') return;

    const cls = TRANSITION_CLASSES[currentTransition];
    if (!cls) return;
    // 全アニメーションクラスを一旦剥がす
    Object.values(TRANSITION_CLASSES).forEach((c) => caption.classList.remove(c));
    // reflow を強制してアニメーションを再トリガ
    void caption.offsetWidth;
    caption.classList.add(cls);
  }

  function bind() {
    const api = window.__TAURI__;
    if (!api || !api.event || typeof api.event.listen !== 'function') {
      // Tauri runtime not ready yet — retry once.
      setTimeout(bind, 50);
      return;
    }
    api.event.listen('show-caption', (evt) => {
      const p = evt && evt.payload;
      if (!p) return;
      // settings → text の順で適用（transition フィールドを反映してから text 差分判定したい）
      applySettings(p.settings);
      setText(p.text);
    });
    api.event.listen('update-style', (evt) => {
      applySettings(evt && evt.payload);
    });
  }

  bind();
})();
