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
    // v0.3.7: borderRadius (px, 0〜32)、CSS 変数経由で .caption に反映
    if (s.borderRadius !== undefined) {
      caption.style.setProperty('--cap-border-radius', Number(s.borderRadius) + 'px');
    }
    // v0.3.7: paddingX / paddingY（px）
    if (s.paddingX !== undefined) {
      caption.style.setProperty('--cap-padding-x', Number(s.paddingX) + 'px');
    }
    if (s.paddingY !== undefined) {
      caption.style.setProperty('--cap-padding-y', Number(s.paddingY) + 'px');
    }
    // v0.3.7: blockGapTenth（em x 10、0〜25）→ em 単位の段落間 margin
    if (s.blockGapTenth !== undefined) {
      caption.style.setProperty('--cap-block-gap', (Number(s.blockGapTenth) / 10) + 'em');
    }
  }

  // v0.3.7: text を <p> 段落に分割して挿入する。
  // dictation-beta は \n{2,} を段落区切り、\n は段落内の改行（line break）として送ってくる
  // （captions.js の renderTextIntoBox 同等のルール）。
  function applyParagraphs(text) {
    // 全 child を退避してから新規挿入（DOM 直接操作で innerHTML 経由を避ける = XSS 対策）
    while (caption.firstChild) caption.removeChild(caption.firstChild);
    const blocks = String(text).split(/\n{2,}/);
    for (const block of blocks) {
      const p = document.createElement('p');
      // 段落内の \n は <br> に
      const lines = block.split('\n');
      lines.forEach((line, i) => {
        if (i > 0) p.appendChild(document.createElement('br'));
        if (line.length > 0) p.appendChild(document.createTextNode(line));
      });
      caption.appendChild(p);
    }
  }

  function setText(text) {
    if (typeof text !== 'string') return;
    // 同じテキストの再送（スタイルだけ変えたい時など）はアニメーション不要
    const changed = text !== lastText;
    lastText = text;
    applyParagraphs(text);
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
