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

  // Tauri 2.0 invoke API (window.__TAURI__.core.invoke)
  function getInvoke() {
    const api = window.__TAURI__;
    if (!api) return null;
    if (api.core && typeof api.core.invoke === 'function') return api.core.invoke;
    if (typeof api.invoke === 'function') return api.invoke;
    return null;
  }

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
    // v0.3.7: borderRadius (px, 0〜32)
    if (s.borderRadius !== undefined) {
      caption.style.setProperty('--cap-border-radius', Number(s.borderRadius) + 'px');
    }
    if (s.paddingX !== undefined) {
      caption.style.setProperty('--cap-padding-x', Number(s.paddingX) + 'px');
    }
    if (s.paddingY !== undefined) {
      caption.style.setProperty('--cap-padding-y', Number(s.paddingY) + 'px');
    }
    if (s.blockGapTenth !== undefined) {
      caption.style.setProperty('--cap-block-gap', (Number(s.blockGapTenth) / 10) + 'em');
    }
  }

  // v0.3.7: text を <p> 段落に分割して挿入する。
  // dictation-beta は \n{2,} を段落区切り、\n は段落内の改行（line break）として送ってくる
  // （captions.js の renderTextIntoBox 同等のルール）。
  function applyParagraphs(text) {
    while (caption.firstChild) caption.removeChild(caption.firstChild);
    const blocks = String(text).split(/\n{2,}/);
    for (const block of blocks) {
      const p = document.createElement('p');
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
    const changed = text !== lastText;
    lastText = text;
    applyParagraphs(text);
    if (!changed) return;
    if (currentTransition === 'none') return;

    const cls = TRANSITION_CLASSES[currentTransition];
    if (!cls) return;
    Object.values(TRANSITION_CLASSES).forEach((c) => caption.classList.remove(c));
    void caption.offsetWidth;
    caption.classList.add(cls);
  }

  // ---- v0.3.9: Window size auto-tracking --------------------------------
  // .caption のサイズが変わったら（フォントサイズ・段落数・ブロック間隔の変化）、
  // ウィンドウもそれに追従させて「字幕の物理サイズ + 余白」になるようにする。
  // これで:
  //   ① フォント大でもウィンドウ境界でクリップされない
  //   ② クリックスルー OFF 時に「字幕より上の透明な空白領域」が無くなり、
  //      マウスイベントが下のアプリに通る
  function setupResizeObserver() {
    const invoke = getInvoke();
    if (!invoke || typeof ResizeObserver === 'undefined') return;
    let lastW = -1;
    let lastH = -1;
    let timer = null;
    const ro = new ResizeObserver(() => {
      const w = caption.offsetWidth;
      const h = caption.offsetHeight;
      if (w === 0 || h === 0) return;
      // 微小変化はスキップ（フォントの hinting で 1px 揺れる程度を吸収）
      if (Math.abs(w - lastW) < 2 && Math.abs(h - lastH) < 2) return;
      lastW = w;
      lastH = h;
      // 50ms デバウンス（連続的に伸び縮みする時に最終値だけ送る）
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        invoke('caption_resized', { width: w, height: h }).catch(() => {});
      }, 50);
    });
    ro.observe(caption);
  }

  // ---- v0.3.9: Fade-out on hide_caption ---------------------------------
  // hide_caption 受信で window.hide() する前に CSS フェードアウトを挟む。
  // 「字幕表示 ON」が 5 秒で beta から hide_caption が来るのにも合う。
  function fadeOutAndHide() {
    const invoke = getInvoke();
    if (!invoke) return;
    caption.classList.add('fading-out');
    setTimeout(() => {
      invoke('window_hide').catch(() => {});
      // hide した後、次に show されるときに opacity 0 のままにならないよう
      // クラスを剥がす
      caption.classList.remove('fading-out');
    }, 220);
  }

  // v0.3.12: max-width を CSS から撤去したため pinMaxWidthToScreen() は不要に
  // なった（旧 v0.3.10 で導入したが今は無害）。`.caption` の幅はテキスト + padding
  // に自然に追従し、ウィンドウは ResizeObserver で `.caption` のサイズに合わせる。

  function bind() {
    const api = window.__TAURI__;
    if (!api || !api.event || typeof api.event.listen !== 'function') {
      setTimeout(bind, 50);
      return;
    }
    api.event.listen('show-caption', (evt) => {
      const p = evt && evt.payload;
      if (!p) return;
      // show 直後に fade-out 残留があったら即剥がす（show が来たら即不透明に戻す）
      caption.classList.remove('fading-out');
      applySettings(p.settings);
      setText(p.text);
    });
    api.event.listen('update-style', (evt) => {
      applySettings(evt && evt.payload);
    });
    // v0.3.9: hide-caption は Rust から「フェードアウトして消す」イベントとして受信
    api.event.listen('fade-out-and-hide', () => {
      fadeOutAndHide();
    });

    setupResizeObserver();
  }

  bind();
})();
